# PLAN.md - M51 Dynamic Reallocation for Failed Agent

## Context

Идем по `docs_raw/DRONE_A.17.md`: после M50 следующий шаг в выбранной Ветке 6 - **M51 Dynamic Reallocation for Failed Agent**.

Цель M51 - добавить минимальный failure/reallocation behavior, нужный для будущего multi-agent SITL. Это не broad Algorithm Depth и не переписывание CBBA/allocator stack. Нужен узкий, детерминированный контракт:

1. heartbeat timeout помечает агента lost;
2. незавершенные задачи lost агента возвращаются в unassigned pool;
3. оставшиеся alive агенты получают эти задачи;
4. метрики и event log явно показывают failure/reallocation;
5. task ownership остается уникальным.

Важный текущий статус: часть механики уже есть в коде. В `swarm-runtime` есть `FailureDetector`, `TaskRegistry::release_agent_tasks`, `Coordinator::process_tick`, `AgentNode::process_inbox_and_allocate` и внутренний `allocate_unassigned`. В `swarm-sim` уже есть `reallocation_time_ticks`/`avg_reallocation_ticks`. В `agent_process` уже есть coarse `reallocation_count`. M51 должен не изобретать это заново, а сделать поведение явным, проверяемым и пригодным для multi-agent SITL foundation в M52.

`PLAN.md` в рабочем дереве на момент планирования отсутствовал, поэтому этот файл создается как новый planning artifact. `INVESTIGATION.md` отсутствует.

## Investigation context

Были изучены:

- `.agent-io/inbox.txt`;
- `docs_raw/DRONE_A.17.md`;
- последние локальные коммиты, включая M50;
- `crates/swarm-runtime/src/failure.rs`;
- `crates/swarm-runtime/src/task_registry.rs`;
- `crates/swarm-runtime/src/coordinator.rs`;
- `crates/swarm-runtime/src/node.rs`;
- `crates/swarm-examples/src/sitl_observability.rs`;
- `crates/swarm-examples/src/sitl_report.rs`;
- `crates/swarm-examples/src/bin/agent_process.rs`;
- `crates/swarm-examples/src/bin/multiprocess_scenario.rs`;
- `crates/swarm-metrics/src/metrics.rs`;
- `crates/swarm-sim/src/runner.rs`;
- README/SITL docs context from M50.

Ключевые findings:

- `FailureDetector::detect()` уже делает heartbeat timeout -> failed agent.
- `TaskRegistry::release_agent_tasks()` уже возвращает assigned/in-progress tasks lost агента в `Unassigned` и очищает `assigned_to`.
- `CoordinatorOutput` сейчас содержит `newly_failed` и `released_tasks`, но не связывает released tasks с конкретным failed agent и не сообщает, какие задачи реально были reassigned.
- `AgentNode::process_inbox_and_allocate()` уже вызывает `allocate_unassigned()` после release/expiry/unassigned/idle-agent conditions, но `allocate_unassigned()` возвращает только conflict count. Поэтому M51 не может явно отчитаться о `reassignment_count`, `tasks_recovered` и latency без небольшой доработки output model.
- `swarm-sim` уже считает `reallocation_time_ticks`, но это simulation-level metric. Для M51 нужен runtime/SITL-facing contract, чтобы будущий multi-agent SITL мог использовать те же события.
- `sitl_observability.rs` сейчас логирует upload/lifecycle/telemetry events, но не имеет событий agent lost / task released / task reassigned / reallocation completed.
- `SitlRunReport` сейчас single-agent oriented. Для M51 лучше не ломать single-agent report semantics; сначала добавить runtime metrics + event-log events. Report fields можно добавить как optional/default только если они действительно используются в CLI/report path.
- `agent_process`/`multiprocess_scenario` уже дают локальный multi-process failure smoke, но он пишет coarse `reallocation_count` как количество released tasks, а не как фактическое количество recovered/reassigned tasks.

## Affected components

- `crates/swarm-runtime/src/task_registry.rs`
  - уточнить/покрыть release semantics: только `Assigned`/`InProgress`, не `Completed`/`Failed`, owner очищается.
- `crates/swarm-runtime/src/coordinator.rs`
  - расширить `CoordinatorOutput` или добавить small struct для per-agent release details: failed agent, released tasks, detection tick.
- `crates/swarm-runtime/src/node.rs`
  - заменить внутренний `allocate_unassigned()` return value с `u64 conflicts` на outcome struct: assigned/reassigned task pairs + conflicts;
  - расширить `NodeTickOutput` runtime-facing reallocation fields.
- `crates/swarm-examples/src/sitl_observability.rs`
  - добавить event variants для lost/reallocation path и summary counters.
- `crates/swarm-examples/src/bin/agent_process.rs`
  - обновить serialized metrics: `reassignment_count`, `tasks_recovered`, `reallocation_latency_ticks` или эквивалентные поля.
- `crates/swarm-examples/src/bin/multiprocess_scenario.rs`
  - читать новые metrics fields и проверять invariants без зависимости от real PX4.
- `crates/swarm-examples/src/sitl_report.rs`
  - только если нужно: добавить optional/default reallocation fields, не ломая существующий single-agent JSON report.
- `crates/swarm-examples/tests/*`
  - добавить/расширить tests для SITL event log and docs sanity.
- `README.md`
  - обязательно актуализировать: добавить M51 status/description, объяснить mock/fake-only scope и что real multi-agent SITL остается M52.
- `docs/SITL_SETUP.md`
  - желательно обновить boundary: M51 покрывает failure/reallocation только mock/fake/runtime level, а не live PX4 multi-agent.

## Implementation steps

1. Зафиксировать модель runtime reallocation output.

   В `crates/swarm-runtime/src/coordinator.rs` добавить small data model, например:

   - `FailureRelease { failed_agent_id, released_tasks, detected_at_tick }`;
   - сохранить совместимость удобных полей `newly_failed`/`released_tasks`, если это снижает blast radius.

   В `crates/swarm-runtime/src/node.rs` добавить small outcome model для allocation after release:

   - `AssignmentChange { task_id, agent_id }`;
   - `ReallocationOutcome { reassigned_tasks, conflicting_assignments }`;
   - в `NodeTickOutput`: `reassignment_count`, `tasks_recovered`, `reallocation_latency_ticks`, `reassigned_tasks` или близкие по смыслу поля.

   Семантика должна быть явной:

   - `released_tasks` - tasks, снятые с failed agent;
   - `reassigned_tasks` - tasks, которым allocator назначил нового alive owner в этом tick;
   - `tasks_recovered` - subset released tasks, которые получили нового owner;
   - `reassignment_count` - `tasks_recovered.len()`;
   - `reallocation_latency_ticks` - latency from detection tick to assignment tick. Для immediate same-tick reallocation это `0`.

2. Сохранить существующий heartbeat timeout path.

   Не переписывать `FailureDetector`. Использовать текущий поток:

   - heartbeats попадают в `MembershipView`;
   - `FailureDetector::detect()` возвращает timed-out alive agents;
   - `Coordinator::process_tick()` делает `mark_dead`;
   - `TaskRegistry::release_agent_tasks()` возвращает unfinished tasks в pool.

   Добавить tests, которые закрепляют это как контракт M51.

3. Сделать allocation outcome observable.

   Сейчас `allocate_unassigned()` возвращает только conflicts. Нужно вернуть список successful assignments. При этом нельзя считать pre-existing unassigned tasks как recovered from failed agent. Для recovered count нужно пересечь successful assignments с `released_tasks`.

   Важно:

   - не назначать задачи dead agents;
   - не допускать duplicate ownership;
   - если нет surviving agents или allocator не может назначить задачу, task остается unassigned и не считается recovered.

4. Добавить metrics/report fields.

   Минимальный путь:

   - расширить `NodeTickOutput` как source of truth;
   - расширить `AgentMetrics` в `agent_process.rs`:
     - `reassignment_count`;
     - `tasks_recovered`;
     - `reallocation_latency_ticks`;
     - возможно `reassigned_tasks`.

   Если используется `SitlRunReport`, добавлять только optional/default fields, чтобы не ломать существующие M48/M49 reports:

   - `#[serde(default)] reassignment_count: u64`;
   - `#[serde(default)] tasks_recovered: Vec<TaskId>` или count-only;
   - `#[serde(default)] reallocation_latency_ticks: Option<u64>`.

   Если report path пока не подключен к multi-agent, оставить report update вне M51 implementation и зафиксировать в docs, что M51 exports metrics/event-log, not single-agent report.

5. Отразить reallocation in event log.

   В `crates/swarm-examples/src/sitl_observability.rs` добавить события, например:

   - `AgentLost { step, agent_id }`;
   - `TaskReleased { step, task_id, previous_agent_id }`;
   - `TaskReassigned { step, task_id, from_agent_id, to_agent_id, latency_ticks }`;
   - `ReallocationCompleted { step, failed_agent_id, reassignment_count, tasks_recovered, latency_ticks }`.

   Обновить summary:

   - count lost agents;
   - count released tasks;
   - count reassigned/recovered tasks;
   - expose final/first reallocation latency.

   На M51 достаточно fake/mock path: helper или recorder method, который принимает `NodeTickOutput`/runtime reallocation summary и пишет events. Не нужно подключать real PX4.

6. Добавить deterministic runtime tests.

   Основной тест должен быть в `crates/swarm-runtime/src/node.rs` или отдельном integration test:

   - создать 2-3 агента и задачи;
   - назначить часть задач agent-0;
   - перестать получать heartbeat agent-0;
   - tick after timeout marks agent-0 dead;
   - tasks agent-0 released;
   - surviving agent получает tasks;
   - `tasks_recovered`/`reassignment_count` корректны;
   - в registry нет duplicate owners.

   Добавить negative/edge cases:

   - no survivors -> tasks released but not recovered;
   - completed task failed agent не возвращается в pool;
   - pre-existing unassigned task can be assigned but не считается recovered;
   - two failed agents same tick release independent task sets.

7. Добавить event-log tests.

   В `sitl_observability.rs` или `crates/swarm-examples/tests/replay_cli.rs`:

   - event log serialization roundtrip с reallocation events;
   - summary counts `agent_lost`, `task_released`, `task_reassigned`, `reallocation_completed`;
   - summary latency field;
   - replay CLI summary, если `replay --sitl-summary` выводит эти counters.

8. Обновить README и SITL docs.

   В `README.md`:

   - добавить M51 в current status;
   - пояснить, что это deterministic mock/fake/runtime reallocation contract;
   - не обещать real multi-agent PX4 SITL до M52;
   - добавить команды проверки M51 tests.

   В `docs/SITL_SETUP.md`:

   - в CI/manual boundary добавить, что M51 failure/reallocation проверяется на runtime/mock/fake level;
   - live PX4 multi-agent failure остается manual/future M52+.

   Расширить `sitl_docs` sanity test, если он уже проверяет README/SITL anchors.

9. Не расширять scope.

   Не делать:

   - hierarchical coordination;
   - communication-aware scoring;
   - broad CBBA rewrite;
   - real multi-agent PX4 SITL;
   - hardware/HIL tests.

10. Verification commands for implementation.

   Для будущей реализации выполнить с hard timeout:

   - `timeout 300s cargo fmt --all`;
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-runtime reallocation`;
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-runtime failure`;
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_observability`;
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`;
   - `timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings`.

   Если меняются `agent_process`/`multiprocess_scenario`, добавить targeted test/command, но не делать live PX4 и не запускать долгие прогоны.

## Testing strategy

### 1. Tests that need no refactoring

Эти автотесты запланированы вместе с main implementation:

- `TaskRegistry::release_agent_tasks`:
  - releases `Assigned` tasks;
  - releases `InProgress` tasks;
  - does not release `Completed` tasks;
  - does not release `Failed` tasks;
  - clears `assigned_to`;
  - returns released task ids deterministically enough for assertions after sorting.
- `Coordinator::process_tick`:
  - heartbeat timeout produces `newly_failed`;
  - failed agent is marked dead;
  - assigned/in-progress tasks are returned to pool;
  - no false release when heartbeat is fresh.
- `AgentNode` deterministic reallocation:
  - lost agent returns tasks to pool;
  - surviving agent receives recovered tasks;
  - `reassignment_count == tasks_recovered.len()`;
  - `reallocation_latency_ticks == Some(0)` for same-tick reallocation;
  - duplicate assignment invariant: every task has at most one owner.
- Negative path:
  - no surviving agents -> tasks released but not recovered;
  - allocator conflicts do not create duplicate ownership;
  - pre-existing unassigned tasks are assigned normally but not counted as recovered from failed agent.
- Edge cases:
  - two agents fail on same tick;
  - completed task of failed agent remains completed;
  - stale heartbeat/generation does not resurrect dead owner unexpectedly.
- SITL event log:
  - reallocation events serialize/deserialize in `snake_case`;
  - summary counts lost agents, released tasks, reassigned tasks and completed reallocation;
  - latency appears in summary.
- Docs sanity:
  - `README.md` contains M51 status and test commands;
  - `docs/SITL_SETUP.md` says M51 reallocation is mock/fake/runtime-level, not live PX4 multi-agent readiness.

### 2. Tests that need light refactoring

Эти тесты стоит добавить, если implementation начинает дублировать setup:

- helper для deterministic fake heartbeat stream:
  - record heartbeat for survivor;
  - omit heartbeat for lost agent;
  - advance tick past timeout.
- helper для multi-agent runtime fixture:
  - agents;
  - assigned tasks;
  - `InMemAgentTransport`;
  - `GreedyAllocator`.
- helper для unique ownership assertions.
- helper или adapter для записи `NodeTickOutput`/reallocation summary в `SitlEventRecorder`.
- `agent_process` metrics serialization test через extracted pure function, если текущая private `write_metrics` плохо тестируется напрямую.
- `multiprocess_scenario` fixture cleanup, если текущий `/tmp/swarm-v03` путь мешает portable tests. Автотесты не должны зависеть от фиксированного `/tmp` state outside test control.

### 3. Tests that need heavy refactoring

Не включать в M51 default scope:

- real multi-agent PX4 SITL failure integration;
- CI-managed PX4 container with several vehicles;
- HIL/real hardware lost-agent tests;
- CBBA broad failure/reallocation rewrite;
- communication-aware scoring under partitions;
- end-to-end supervisor process with multiple real MAVLink endpoints.

Эти проверки относятся к M52+ или отдельной ветке, потому что требуют multi-agent SITL foundation and external simulator orchestration.

## Risks and tradeoffs

- `NodeTickOutput` является публичным runtime-facing типом. Расширять его лучше добавлением полей, а не ломать существующие consumers.
- Важно не смешать `released_tasks`, `reassigned_tasks` и `tasks_recovered`: pre-existing unassigned tasks могут быть assigned на том же tick, но не должны считаться recovered from failed agent.
- Latency нужно определить однозначно. Для M51 рекомендуем runtime latency от detection tick до reassignment tick. Simulation-level latency from scheduled failure tick уже существует отдельно.
- Event log schema расширится. Нужно сохранить `snake_case` serialization и roundtrip tests.
- Existing simulation metrics уже имеют `reallocation_time_ticks`; не нужно переименовывать или ломать benchmark exports ради M51.
- CBBA/distributed path может требовать отдельной логики, потому что current centralized `allocate_unassigned()` skipped when `cbba.is_some()`. На M51 допустимо явно ограничить deterministic test Greedy/mock path и не обещать broad CBBA repair.
- `agent_process` и `multiprocess_scenario` используют реальные UDP sockets locally. Для M51 unit/integration tests лучше держать in-memory/fake transport default, а UDP scenario оставить manual or targeted smoke.
- README не должен обещать multi-agent PX4 readiness: M51 только runtime/mock/fake failure/reallocation contract.

## Open questions

1. Должны ли reallocation fields попасть в `SitlRunReport`, или на M51 достаточно `NodeTickOutput` + `AgentMetrics` + SITL event log?
2. Какой latency считать canonical для M51: от detection tick или от фактического missed heartbeat/failure tick? Рекомендация: runtime field = detection -> assignment, simulation metrics остаются как есть.
3. Нужно ли M51 покрывать CBBA path минимальным test, или оставить CBBA за scope как "broad CBBA rewrite"?
4. Где лучше разместить conversion from runtime output to SITL event log: в `sitl_observability.rs` helper или в будущий multi-agent SITL supervisor?
5. Нужно ли обновлять `multiprocess_scenario` сейчас, если M52 все равно будет делать полноценный multi-agent SITL foundation?
