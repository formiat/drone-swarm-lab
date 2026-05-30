# DRONE_A.18 - detailed next development vector after M43-M56

Дата: 2026-05-30

## Короткий вывод

Следующий лучший вектор развития проекта: **Live Multi-Agent PX4 Supervisor**.

Проект уже вышел за рамки чистого симулятора: есть scenario DSL, runtime,
allocators, regression harness, replay, single-agent PX4 SIH golden path,
multi-agent manifest/config, mock supervisor reallocation and deterministic
regression sweep. Но главный оставшийся технический разрыв находится не в
документации и не в бенчмарках:

- multi-agent PX4 сейчас проверен только как **upload-only**;
- live execute orchestration для нескольких PX4 instance отсутствует;
- failure/reallocation уже доказан в runtime/mock supervisor, но не в live PX4
  flow;
- hardware boundary оформлен, но hardware readiness не заявляется и не должна
  заявляться.

Поэтому следующий этап должен не распыляться на новые миссии, UI или публикацию,
а соединить уже реализованные части в один воспроизводимый live SITL workflow:

```
multi-agent config
  -> supervisor orchestration
  -> PX4 SIH agents
  -> execute lifecycle
  -> telemetry progress
  -> failure detection
  -> runtime reallocation
  -> survivor mission update
  -> replay/result artifact
```

## Почему план начинается с M57

M43-M53 были планом Ветки 6 Real SITL / PX4.

После них в репозитории появились дополнительные result milestones:

- M54 - mock multi-agent supervisor with reallocation artifact;
- M55 - two-instance PX4 SIH upload-only artifact;
- M56 - regression determinism sweep.

Чтобы не путать исторические result artifacts с новым планом, дальнейшую
нумерацию разумно начать с **M57**.

## Текущее состояние, от которого отталкиваемся

### Готово

- `sitl_agent` умеет dry-run, mock, upload-only, execute lifecycle для
  single-agent PX4 SITL.
- MAVLink mission upload protocol реализован через настоящий upload handshake.
- Есть pre-upload safety validation and hardware-candidate guard.
- Есть telemetry-to-task progress mapping.
- Есть SITL event log and replay summary.
- Есть `multi_sitl.v1` config and manifest.
- Есть duplicate ownership rejection.
- Есть mock/fake `sitl_supervisor` workflow:
  - heartbeat/progress tracking;
  - timeout;
  - agent lost;
  - task release;
  - runtime reallocation;
  - reallocation events;
  - supervisor metrics.
- Есть captured local PX4 SIH artifacts:
  - single-agent execute path;
  - two-agent upload-only path.
- Default regression determinism sweep passed for `jobs=1/4/14`.

### Не готово

- `sitl_supervisor` не умеет управлять несколькими real PX4 agents in execute
  mode.
- Нет live PX4 multi-agent flow, где несколько agents одновременно выполняют
  свои task subsets под одним supervisor run.
- Нет live PX4 failure/reallocation flow.
- Нет live supervisor policy для partial startup, partial upload failure,
  partial execute failure, per-agent abort and cleanup.
- Нет captured artifact, который доказывает:
  - 2 PX4 SIH agents;
  - both execute;
  - telemetry progress collected per agent;
  - common event log;
  - no duplicate ownership;
  - final multi-agent run report.
- Нет captured artifact, который доказывает live failure:
  - один PX4 agent lost/stalled/disconnected;
  - его unfinished tasks released;
  - survivor receives recovered task;
  - reallocation visible in event log.

## Главная цель следующего блока

Сделать проект не просто "simulation + PX4 bridge", а воспроизводимым
experimental SITL harness для multi-agent missions.

Важно: это всё еще **не hardware-ready product**. Цель блока - live PX4 SIH /
SITL proof, а не реальные дроны, не flight certification, не production ground
control station.

## Общий порядок

```
M57 Supervisor Controller Boundary
  -> M58 Live Multi-Agent PX4 Execute
  -> M59 Live PX4 Failure / Reallocation
  -> M60 PX4 Supervisor Hardening
  -> M61 Benchmark / Baseline Refresh
  -> M62 Next Branch Decision
```

M57-M60 - техническое продолжение Ветки 6.
M61 - возврат к research/benchmark only after live SITL gap is closed.
M62 - точка выбора новой специализации.

---

## M57 - Supervisor Controller Boundary

### Цель

Отделить `sitl_supervisor` как supervisor/orchestrator от конкретного способа
управления агентом.

Сейчас `crates/swarm-examples/src/bin/sitl_supervisor.rs` уже полезен, но он
смешивает:

- CLI parsing;
- manifest generation;
- mock runtime setup;
- agent heartbeat simulation;
- task completion simulation;
- reallocation;
- metrics;
- event log writing.

Для live PX4 execute path это станет хрупким. Перед добавлением real PX4
controller нужно ввести явную границу:

```text
Supervisor
  owns run lifecycle, metrics, event log, task ownership, runtime coordinator

AgentController
  owns one agent lifecycle: upload, execute, poll progress, abort, final status
```

### Предлагаемая архитектура

Начать внутри `swarm-examples`, не выносить преждевременно в публичный crate.

Возможные модули:

- `crates/swarm-examples/src/sitl_supervisor.rs`
- `crates/swarm-examples/src/sitl_controller.rs`
- или оставить рядом с binary, но вынести тестируемую логику из `main`.

Минимальные типы:

```rust
trait AgentController {
    fn agent_id(&self) -> &str;
    fn lifecycle(&self) -> MultiAgentLifecycle;
    fn upload(&mut self, plan: &AgentMissionPlan) -> Result<AgentStep, SitlError>;
    fn start(&mut self) -> Result<AgentStep, SitlError>;
    fn poll(&mut self, tick: u64) -> Result<AgentProgress, SitlError>;
    fn abort(&mut self, reason: &str) -> Result<AgentStep, SitlError>;
}
```

Точные имена можно поменять при реализации. Важно не имя trait, а разделение
ответственности.

### Что сделать

1. Вынести из `sitl_supervisor.rs` чистую supervisor state machine.
2. Ввести `MockAgentController`, который сохраняет текущее поведение
   `--mock`.
3. Оставить CLI behavior совместимым:
   - `--dry-run`;
   - `--mock`;
   - `--manifest`;
   - `--replay-log`;
   - `--fail-agent`;
   - `--fail-after-ticks`;
   - `--heartbeat-timeout-ticks`;
   - `--max-ticks`.
4. Сделать supervisor metrics отдельной структурой, которую можно тестировать
   без запуска binary.
5. Сохранить текущий deterministic failure/reallocation test.
6. Добавить тесты на state machine напрямую, если это возможно без большого
   рефакторинга.

### Не делать в M57

- Не добавлять PX4 real controller.
- Не менять MAVLink protocol.
- Не менять runtime reallocation semantics.
- Не делать hardware path.
- Не стабилизировать публичный API.

### Done criteria

- Текущее `sitl_supervisor --mock` поведение не сломано.
- Существующий mock reallocation artifact можно воспроизвести.
- Supervisor logic тестируется без subprocess там, где это практично.
- В коде появилась понятная точка расширения для `Px4AgentController`.
- `cargo test -p swarm-examples --test sitl_agent` проходит.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  проходит.

### Тесты

#### Tests that need no refactoring

- Existing subprocess tests for `sitl_supervisor --mock`.
- Duplicate ownership rejection.
- Missing/invalid supervisor CLI args.
- Replay summary contains reallocation events.

#### Tests that need light refactoring

- Unit tests for supervisor state transitions with fake controllers.
- Metrics aggregation tests without spawning binary.
- Deterministic failure schedule tests:
  - fail before upload;
  - fail after upload;
  - fail during progress.

#### Tests that need heavy refactoring

- Property tests over arbitrary failure schedules.
- Multi-agent state-machine model tests where controller responses are generated.
- Cross-check replay events against final task registry state.

---

## M58 - Live Multi-Agent PX4 Execute

### Цель

Получить первый live PX4 SIH multi-agent execute workflow.

Это следующий качественный шаг после M55. M55 доказал, что две PX4 SIH instance
могут принять разные task subsets. M58 должен доказать, что они могут быть
запущены как один supervised run.

### Минимальный сценарий

- 2 agents.
- `scenarios/sitl.multi-agent.json`.
- `scenarios/sitl.multi-agent.config.json`.
- разные MAVLink endpoints.
- разные `system_id`.
- lifecycle: `execute`.
- each agent receives disjoint task subset.
- common run id.
- common event log.
- common final report.

### Что сделать

1. Добавить `Px4AgentController`.
2. Переиспользовать существующий `sitl_agent` logic where possible:
   - mission plan extraction;
   - safety validation;
   - mission upload;
   - arm/takeoff/start;
   - telemetry progress mapping;
   - final report fields.
3. Решить, как не дублировать код:
   - либо выделить reusable library functions из `sitl_agent.rs`;
   - либо на первом шаге supervisor запускает несколько `sitl_agent`
     subprocesses.
4. Предпочтительный вариант: **library/controller**, а не subprocess.
   Subprocess проще, но хуже для failure/reallocation, event merge and metrics.
5. Supervisor должен вести per-agent lifecycle:
   - `Pending`;
   - `Uploaded`;
   - `Started`;
   - `InProgress`;
   - `Completed`;
   - `Failed`;
   - `Aborted`.
6. Event log должен включать `agent_id` для всех relevant events.
7. Final run report должен показывать:
   - agents count;
   - per-agent lifecycle result;
   - per-agent completed tasks;
   - total completed tasks;
   - failed agents;
   - aborts;
   - run duration;
   - scenario/config path;
   - git commit if already available via existing report helpers.
8. Сделать ручной PX4 SIH прогон and store artifact in `results/...`.

### CLI sketch

Команда может выглядеть примерно так:

```bash
cargo run -p swarm-examples --features mavlink-transport --bin sitl_supervisor -- \
  --connection-execute \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.config.json \
  --replay-log results/m58_multi_agent_px4_execute_YYYY-MM-DD/run.sitl-log.json \
  --run-report results/m58_multi_agent_px4_execute_YYYY-MM-DD/report.json \
  --timeout 120 \
  --telemetry-timeout 30 \
  --no-progress-timeout 45 \
  --allow-hardware-candidate
```

Exact CLI может быть другим, но важно не перегрузить `--mock`. Должно быть ясно:
это live PX4 path, а не portable mock.

### Не делать в M58

- Не делать failure/reallocation.
- Не делать hardware.
- Не делать Gazebo/HIL as required gate.
- Не делать complex distributed coordination между PX4 agents.
- Не делать UI.

### Done criteria

- Два PX4 SIH agents execute disjoint task subsets.
- Supervisor завершает run and writes common report.
- Event log читается `replay --summary`.
- No duplicate ownership remains enforced before upload.
- Документация честно говорит: local PX4 SIH execute, not hardware.
- Есть result artifact directory.

### Риски

- PX4 SIH может вести себя иначе для двух instance, чем upload-only.
- MAVLink endpoint ownership может конфликтовать с PX4 normal/onboard mode.
- Concurrent mission protocol per endpoint может потребовать аккуратного polling.
- Telemetry progress per agent может быть noisy; timeouts нужно подбирать
  консервативно.

### Тесты

#### Tests that need no refactoring

- Multi-agent config parse/validation.
- Duplicate ownership rejection.
- Command generation for per-agent standalone commands.
- Replay summary works for synthetic multi-agent execute events.

#### Tests that need light refactoring

- Fake `Px4AgentController` execute lifecycle:
  - upload success;
  - start success;
  - progress ticks;
  - completion.
- Supervisor report aggregation from two fake controllers.
- Per-agent event log ordering and agent_id presence.

#### Tests that need heavy refactoring

- Manual/ignored real PX4 SIH integration test.
- Full multi-agent execute artifact validation.
- Time-bounded live SITL smoke that can be run locally but not in default CI.

---

## M59 - Live PX4 Failure / Reallocation

### Цель

Перенести доказанный mock/runtime reallocation flow в live PX4 supervisor.

Это главный milestone всего следующего блока. После него можно будет честно
сказать, что project has an experimental live PX4 SIH multi-agent reallocation
workflow.

### Failure modes для первой версии

Не нужно сразу моделировать все возможные аварии. Минимально достаточно одного
контролируемого failure mode:

1. Supervisor перестает получать progress/heartbeat from agent.
2. Agent marked lost after timeout.
3. Unfinished tasks assigned to lost agent are released.
4. Runtime reallocation assigns recoverable tasks to survivor.
5. Survivor receives mission update.
6. Event log records the full chain.

Для manual run можно использовать один из способов:

- остановить один PX4 process;
- закрыть его endpoint;
- использовать supervisor test flag `--fail-agent <id>` that simulates
  controller disconnect after N ticks while the other PX4 instance remains live;
- start with simulated controller failure over one real survivor if full process
  kill is too unstable.

### Важное проектное решение

Нужно выбрать, как survivor получает recovered tasks:

**Option A - mission replacement.**

- Stop/clear current mission on survivor.
- Upload new combined remaining mission.
- Restart/continue.
- Simpler to reason about.
- More intrusive for PX4 state.

**Option B - append/partial update.**

- Add recovered waypoints after current mission.
- Less disruptive.
- Harder to make correct and portable.

Рекомендация: **Option A for M59**. Для research/SITL foundation важнее
детерминированность и прозрачность, чем production-like minimal disturbance.

### Что сделать

1. Extend supervisor task registry mapping:
   - task id -> agent;
   - task id -> waypoint sequence;
   - task status per agent.
2. Add live lost-agent detection:
   - heartbeat timeout;
   - telemetry no-progress timeout;
   - controller disconnect/error.
3. Wire lost-agent event into runtime reallocation.
4. Convert recovered task ids into survivor mission plan.
5. Upload replacement/updated mission to survivor.
6. Continue survivor telemetry tracking.
7. Record metrics:
   - `lost_agents`;
   - `released_tasks`;
   - `reassigned_tasks`;
   - `reassignment_count`;
   - `reallocation_latency_ticks`;
   - `tasks_recovered`;
   - `survivor_mission_updates`;
   - `final_completed_after_reallocation`.
8. Add result artifact:
   - README;
   - command;
   - PX4 version/commit;
   - endpoints/system ids;
   - event log;
   - replay summary;
   - final report;
   - note whether failure was process kill, endpoint loss, or controlled
     simulated controller failure.

### Не делать в M59

- Не заявлять hardware readiness.
- Не делать robust production failover.
- Не гарантировать collision avoidance.
- Не пытаться доказать all failure modes.
- Не делать distributed onboard reallocation; supervisor remains central.

### Done criteria

- Есть deterministic fake test: one live-style controller fails, survivor gets
  recovered tasks.
- Есть manual PX4 SIH artifact for at least one live/simulated failure path.
- Event log summary shows:
  - `agent_lost=1`;
  - `task_released>=1`;
  - `task_reassigned>=1`;
  - `reallocation_completed=1`;
  - non-empty recovered tasks list.
- Docs clearly state what was and was not proven.

### Риски

- Mission replacement may disrupt PX4 mission state.
- If survivor is already mid-flight, replacing mission can create confusing
  telemetry semantics.
- If the lost agent fails after completing its task but before final telemetry,
  supervisor must not reassign already completed tasks.
- Real PX4 failure may be harder to make deterministic than mock failure.

### Тесты

#### Tests that need no refactoring

- Runtime reallocation tests already present.
- Mock supervisor failure test already present.
- Replay event roundtrip for reallocation events.
- Task registry release/reassign tests.

#### Tests that need light refactoring

- Fake live controller failure:
  - fail before start;
  - fail during progress;
  - fail after completing one task.
- Mission replacement plan test for survivor.
- Final report metrics aggregation test.
- Replay summary test for live-style failure events.

#### Tests that need heavy refactoring

- Manual/ignored two-PX4 SIH failure integration.
- Process-control harness that starts/kills PX4 instance deterministically.
- Property tests for no duplicate ownership after arbitrary failure timing.

---

## M60 - PX4 Supervisor Hardening

### Цель

Сделать live supervisor не одноразовым экспериментом, а достаточно надежным
research workflow.

M58-M59 могут сначала быть narrow happy path плюс one failure case. M60 должен
закрыть инженерные шероховатости, которые иначе будут мешать каждому следующему
прогону.

### Что сделать

1. Typed supervisor errors:
   - bad config;
   - endpoint unavailable;
   - heartbeat timeout;
   - mission upload failed;
   - command rejected;
   - progress timeout;
   - abort failed;
   - partial run failed.
2. Consistent exit codes:
   - config/CLI error;
   - safety validation error;
   - PX4 unavailable;
   - mission rejected;
   - runtime failure after start.
3. Better report schema:
   - `schema_version`;
   - `run_id`;
   - `mode`;
   - `agents`;
   - `task_ownership`;
   - `events_summary`;
   - `final_status`;
   - `limitations`.
4. Idempotent output directories:
   - create directories automatically;
   - avoid overwriting unless `--force` or unique run id.
5. Documentation:
   - exact local PX4 SIH startup commands;
   - troubleshooting for multi-instance endpoints;
   - how to interpret reallocation artifacts;
   - what is out of scope.
6. Regression:
   - mock path remains default CI-safe;
   - live PX4 remains manual/ignored.

### Не делать в M60

- Не делать public API stabilization.
- Не делать hardware checklist expansion beyond existing boundary unless a
  concrete hardware experiment is planned.
- Не делать benchmark publication.

### Done criteria

- Bad user inputs produce actionable errors.
- Partial agent failure produces structured report, not ambiguous stdout only.
- Docs and tests agree on current behavior.
- Manual run artifacts are reproducible enough for another local run.

### Тесты

#### Tests that need no refactoring

- CLI rejects missing values and conflicting modes.
- Config validation errors include agent/task context.
- Replay summary handles failure reports.

#### Tests that need light refactoring

- Fake controller error matrix.
- Report schema snapshot-ish tests.
- Output path behavior tests using temp directories.

#### Tests that need heavy refactoring

- End-to-end supervisor harness with multiple fake agents and randomized errors.
- Manual/ignored live PX4 negative cases.

---

## M61 - Benchmark / Baseline Refresh

### Цель

После закрытия live SITL gap можно обновить simulation benchmark claims.

До M57-M60 большой benchmark был бы полезен, но не закрывал бы главный
архитектурный вопрос. После M57-M60 проект уже будет иметь более сильную
позицию: simulation benchmark плюс live PX4 SIH evidence.

### Что сделать

1. Decide exact benchmark scope:
   - default supported mission-strategy pairs only;
   - unsupported pairs remain explicit unsupported;
   - realism profiles either included as experimental or excluded.
2. Run agreed seed count:
   - 500 if time/cost matters;
   - 1000 if publication-like artifact is desired.
3. Use release build.
4. Fix manifest:
   - git commit;
   - seed range;
   - jobs count;
   - machine summary if desired;
   - scenario versions.
5. Generate:
   - JSON;
   - CSV;
   - Markdown table;
   - summary report.
6. Update:
   - `docs/BENCHMARK_RESULTS.md`;
   - README benchmark status;
   - `docs/STATUS.md`.

### Не делать в M61

- Не использовать benchmark as substitute for live PX4 evidence.
- Не включать unsupported pairs as success claims.
- Не делать paper-level statistical analysis unless explicitly chosen.

### Done criteria

- Fresh benchmark artifact exists for current HEAD.
- Historical benchmark docs no longer look current if they are stale.
- Regression runner remains green after benchmark-related changes.

### Тесты

#### Tests that need no refactoring

- Existing benchmark export tests.
- Regression runner default suite.
- Manifest/report identity tests.

#### Tests that need light refactoring

- Benchmark pack validation helper.
- Compare baseline smoke on new artifact.
- Docs test for benchmark artifact paths.

#### Tests that need heavy refactoring

- Confidence interval tooling tests.
- Large-run reproducibility harness across machines.
- Statistical delta report validation.

---

## M62 - Next Branch Decision

### Цель

После M57-M61 снова выбрать стратегическое направление.

На этом этапе проект будет иметь:

- simulation foundation;
- deterministic regression gate;
- single-agent PX4 SIH execute evidence;
- multi-agent PX4 SIH execute evidence;
- live/mock reallocation evidence;
- benchmark refresh.

Это хорошая точка для осознанной развилки.

### Варианты после M62

#### Option A - Continue Real SITL / PX4

Дальше идти в:

- Gazebo validation;
- HIL boundary;
- better PX4 process orchestration;
- richer safety policies;
- supervised hardware experiment checklist.

Выбирать, если цель - hardware-adjacent research harness.

#### Option B - Research Benchmark

Дальше идти в:

- confidence intervals;
- degradation curves;
- strategy comparison report;
- communication loss / scale curves;
- publication-quality results.

Выбирать, если цель - исследовательский артефакт and reproducible paper-like
benchmark.

#### Option C - Algorithm Depth

Дальше идти в:

- communication-aware allocation;
- mission-specific planner modes;
- hierarchical coordination;
- stronger CBBA under loss/failure.

Выбирать, если цель - улучшить сами алгоритмы, а не инфраструктуру.

#### Option D - Platform / API

Дальше идти в:

- library API cleanup;
- stable scenario/runner API;
- examples;
- semantic versioning;
- package hygiene.

Выбирать, если цель - сделать проект удобным для внешних пользователей.

#### Option E - Replay / Visualization

Дальше идти в:

- richer replay schema;
- timeline viewer;
- map visualization;
- compare two runs visually.

Выбирать, если цель - анализ и демонстрация поведения.

### Рекомендация на момент DRONE_A.18

Не выбирать новую ветку прямо сейчас. Сначала закрыть M57-M60. Потом сделать M61
только если нужен свежий benchmark claim. После этого вернуться к выбору ветки.

## Почему не начинать сейчас с benchmark/publication

Большие simulation runs полезны, но они не закрывают главное ограничение в
`docs/STATUS.md`: real multi-agent PX4 is partial. Если сейчас сделать еще один
1000-seed benchmark, проект станет лучше документирован как simulator, но не
станет сильнее как SITL/PX4 workflow.

Benchmark стоит делать после M57-M60, потому что тогда можно честно сказать:

- simulation behavior measured;
- regression deterministic;
- single-agent PX4 SIH verified;
- multi-agent PX4 SIH execute verified;
- failure/reallocation path demonstrated at least in controlled SIH/manual form.

## Почему не начинать сейчас с hardware

Hardware readiness explicitly out of scope. Текущий `docs/HARDWARE_READINESS.md`
правильно отделяет local SITL from hardware candidate connections. До hardware
нужны:

- stable live multi-agent PX4 execute;
- stable live failure/reallocation semantics;
- better abort/cleanup policy;
- operator procedure outside the codebase;
- physical safety process.

Прыжок к hardware сейчас был бы преждевременным.

## Почему не начинать сейчас с новых миссий

Новые миссии вроде flood/disaster mapping v2 могут быть интересны, но они не
используют главный свежий прогресс по PX4. Если сейчас уйти в новые missions,
проект снова расползется в simulation breadth, а live SITL gap останется.

## Практический первый шаг

Начинать с **M57 Supervisor Controller Boundary**.

Это небольшой, безопасный и нужный этап:

- он не требует PX4;
- сохраняет текущие mock tests;
- уменьшает риск M58/M59;
- делает код supervisor расширяемым;
- быстро покажет, насколько текущий `sitl_agent` можно переиспользовать как
  library logic.

После M57 можно будет принимать более точное решение по реализации M58:

- in-process `Px4AgentController`;
- or subprocess-based first version;
- or hybrid.

Но M57 должен быть сделан так, чтобы этот выбор не ломал public behavior.

## Итоговый линейный план

1. M57 - Supervisor Controller Boundary.
2. M58 - Live Multi-Agent PX4 Execute.
3. M59 - Live PX4 Failure / Reallocation.
4. M60 - PX4 Supervisor Hardening.
5. M61 - Benchmark / Baseline Refresh.
6. M62 - Next Branch Decision.

Главная формула следующего этапа:

> Сначала превратить multi-agent PX4 из upload-only artifact в supervised execute
> workflow, затем перенести failure/reallocation из mock в live PX4 SIH, и только
> после этого возвращаться к benchmark/publication or new branch choice.
