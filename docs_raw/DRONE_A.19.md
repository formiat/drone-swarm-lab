# DRONE_A.19 - итоговый план после сравнения A.18 и B.18

Дата: 2026-05-30

## Контекст

Этот документ сравнивает два варианта дальнейшего развития:

- `docs_raw/DRONE_A.18.md`;
- `docs_raw/DRONE_B.18.md`.

Итоговый выбор учитывает ограничение: **пока не идем в реальное физическое
железо / аппаратку**. Все, что ниже, остается в границах local PX4 SITL / PX4
SIH, mock/fake tests, replay, regression and documentation. HIL, real hardware,
flight certification and physical safety process не входят в этот план.

## Короткое сравнение A.18 и B.18

### Что лучше в A.18

A.18 лучше как основной стратегический вектор.

Он точнее попадает в главный текущий gap проекта:

- `sitl_agent` уже умеет single-agent PX4/SIH execute;
- `sitl_supervisor --mock` уже умеет heartbeat timeout and reallocation;
- multi-agent PX4/SIH уже проверен как upload-only;
- но нет live multi-agent execute supervisor;
- и нет live PX4/SIH failure/reallocation flow.

A.18 правильно предлагает не начинать с новых миссий, benchmark publication или
platform/API, а сначала закрыть live multi-agent PX4/SIH supervisor gap.

Главная сильная идея A.18: **Supervisor Controller Boundary**. Перед тем как
добавлять live PX4 controller, нужно отделить supervisor state machine from
agent implementation. Иначе `sitl_supervisor` быстро станет большим смешением
CLI, MAVLink, runtime coordinator, metrics, event log and failure policy.

### Что лучше в B.18

B.18 полезнее по деталям конкретных implementation tasks.

Из него стоит взять:

- `sitl_supervisor --connection` как понятный live/SIH режим;
- connection strings from `sitl.multi-agent.config.json`;
- per-agent telemetry aggregation;
- per-agent no-progress timeout;
- sequential/parallel launch option, default sequential;
- common multi-agent run report;
- event types:
  - `SitlMultiAgentRunStarted`;
  - `SitlMultiAgentRunFinished`;
  - supplementary upload started/completed;
- `--reupload-on-failure` / supplementary upload idea;
- detailed troubleshooting/docs requirements;
- `docs/EXTENSION_GUIDE.md` как полезный post-SITL stabilization milestone;
- New Mission candidates as future branch material.

### Что хуже в B.18

B.18 хуже как линейный план, потому что смешивает две независимые траектории:

- B: Live Multi-Agent PX4;
- C: Disaster Mapping v2 / Platform / New Mission.

Это расширяет scope раньше времени. Сейчас главный риск проекта не в flood, не
в extension guide and not in a new mission. Главный риск - live multi-agent
PX4/SIH supervisor still partial.

Также B.18 использует формулировку "аппаратная готовность". Для текущего этапа
это лучше не использовать: она может звучать как движение к физическому железу.
Правильная формулировка: **local PX4/SIH supervised workflow**, not hardware
readiness.

## Итоговый выбор

Итоговый план должен быть линейным:

```
M57 Supervisor Controller Boundary
  -> M58 Live Multi-Agent PX4/SIH Execute Orchestration
  -> M59 Live PX4/SIH Failure & Reallocation
  -> M60 PX4/SIH Supervisor Hardening
  -> M61 Platform / API Stabilization
  -> M62 Benchmark / Baseline Refresh
  -> M63 Next Branch Decision
```

Главная формула:

> Сначала превратить multi-agent PX4/SIH из upload-only artifact в supervised
> execute workflow, затем перенести failure/reallocation из mock в controlled
> live/SIH flow, потом стабилизировать extension/API and refresh benchmark.

## Scope boundary

### Входит

- local PX4 SITL / SIH;
- multiple PX4 local instances;
- mock/fake controller tests;
- manual/ignored local integration tests;
- supervisor orchestration;
- replay/event log;
- run reports;
- regression and benchmark artifacts;
- documentation.

### Не входит

- real physical drones;
- HIL as required gate;
- Gazebo as required gate;
- flight certification;
- production ground control station;
- onboard distributed autonomy;
- runtime collision avoidance;
- hardware-specific failsafe tuning.

Gazebo/HIL/hardware can be future options after this plan, but they are not part
of M57-M63.

---

## M57 - Supervisor Controller Boundary

### Цель

Отделить `sitl_supervisor` как orchestrator/state machine от конкретной
реализации агента.

Сейчас `sitl_supervisor --mock` уже выполняет полезный workflow:

- manifest loading/building;
- mock waypoints;
- simulated heartbeat;
- agent lost timeout;
- task release;
- runtime reallocation;
- event log;
- supervisor metrics.

Но этот код не должен напрямую разрастаться в live PX4 implementation. Перед
M58 нужно ввести внутреннюю границу:

```text
Supervisor
  owns: run lifecycle, runtime coordinator, task ownership, event log, metrics

AgentController
  owns: one agent connection/lifecycle/progress/abort/final state
```

### Предлагаемые типы

Имена могут измениться при реализации, но смысл должен сохраниться:

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

Минимальные реализации:

- `MockAgentController` - сохраняет текущее `--mock` поведение;
- `FakeAgentController` for unit/integration tests;
- `Px4AgentController` появляется только в M58.

### Что сделать

1. Вынести supervisor state machine из `crates/swarm-examples/src/bin/sitl_supervisor.rs`.
2. Оставить CLI совместимым:
   - `--dry-run`;
   - `--mock`;
   - `--manifest`;
   - `--replay-log`;
   - `--fail-agent`;
   - `--fail-after-ticks`;
   - `--heartbeat-timeout-ticks`;
   - `--max-ticks`.
3. Сделать `SupervisorMetrics` тестируемой структурой, не привязанной к stdout.
4. Сохранить current mock reallocation behavior byte-for-byte where practical.
5. Добавить unit tests для supervisor transitions через fake controllers.
6. Добавить точку расширения для future `Px4AgentController`, но не реализовывать
   live PX4 in M57.

### Не делать

- Не добавлять live PX4 controller.
- Не менять MAVLink upload protocol.
- Не менять runtime reallocation semantics.
- Не трогать hardware boundary.
- Не стабилизировать публичный API.

### Done criteria

- `sitl_supervisor --mock` работает как раньше.
- Deterministic mock reallocation test остается green.
- Supervisor state machine покрыта хотя бы базовыми tests без subprocess.
- Кодовая граница для M58 понятна.
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
- Cross-check replay events against final task registry state.
- Full state-machine model tests for generated controller responses.

---

## M58 - Live Multi-Agent PX4/SIH Execute Orchestration

### Цель

Получить первый local PX4/SIH multi-agent execute workflow under `sitl_supervisor`.

Это следующий шаг после upload-only artifact:

- M55 доказал, что две PX4 SIH instance могут принять разные waypoint subsets;
- M58 должен доказать, что supervisor может запустить их как один общий
  multi-agent run.

### Минимальный сценарий

- 2 agents.
- `scenarios/sitl.multi-agent.json`.
- `scenarios/sitl.multi-agent.config.json`.
- разные MAVLink endpoints.
- разные `system_id`.
- lifecycle: `execute`.
- disjoint task subsets.
- common `run_id`.
- common event log.
- common final report.

### Что сделать

1. Добавить live/SIH режим в `sitl_supervisor`.

   Возможный CLI:

   ```bash
   cargo run -p swarm-examples --features mavlink-transport --bin sitl_supervisor -- \
     --connection \
     --scenario scenarios/sitl.multi-agent.json \
     --config scenarios/sitl.multi-agent.config.json \
     --execute \
     --replay-log results/m58_multi_agent_px4_sih_execute_YYYY-MM-DD/run.sitl-log.json \
     --run-report results/m58_multi_agent_px4_sih_execute_YYYY-MM-DD/report.json \
     --timeout 120 \
     --telemetry-timeout 30 \
     --no-progress-timeout 45
   ```

   Точный CLI можно изменить. Важно:

   - live/SIH режим должен быть явно отделен от `--mock`;
   - без `mavlink-transport` должна быть typed/actionable error;
   - physical hardware не должен случайно включаться.

2. Реализовать `Px4AgentController`.

   Он должен переиспользовать уже существующую logic из `sitl_agent`:

   - mission plan extraction;
   - safety validation;
   - mission upload;
   - arm/takeoff/start;
   - telemetry progress;
   - abort on bounded failures.

   Предпочтительный вариант - library/controller reuse, не subprocess spawning.
   Subprocess допустим как fallback, но хуже для metrics, event merge and M59.

3. Per-agent lifecycle.

   Минимальные состояния:

   - `Pending`;
   - `Uploaded`;
   - `Started`;
   - `InProgress`;
   - `Completed`;
   - `Failed`;
   - `Aborted`.

4. Launch policy.

   Взять из B.18:

   - support `start_delay_ms` from config;
   - default sequential launch;
   - optional `--parallel` later if easy;
   - sequential should be default because it is easier to debug and safer for
     local SIH experiments.

5. Per-agent telemetry aggregation.

   Нужно хранить:

   - `(agent_id, seq) -> task_id`;
   - heartbeat/progress per agent;
   - completed tasks per agent;
   - no-progress timeout per agent;
   - final status per agent.

6. Event log.

   Existing SITL events should correctly include `agent_id`.

   Add or emulate:

   - `SitlMultiAgentRunStarted { agent_count, scenario }`;
   - `SitlMultiAgentRunFinished { overall_status }`.

7. Final multi-agent report.

   Report fields:

   - `schema_version`;
   - `run_id`;
   - `scenario`;
   - `config`;
   - `mode`;
   - per-agent connection/system/component;
   - per-agent lifecycle;
   - per-agent mission item count;
   - per-agent completed task count;
   - total completed tasks;
   - failed/aborted agents;
   - overall status;
   - event log path;
   - known limitations.

8. Captured artifact.

   Create `results/m58_multi_agent_px4_sih_execute_YYYY-MM-DD/` with:

   - README;
   - exact commands;
   - PX4 path/version/commit if available;
   - endpoints/system ids;
   - stdout/stderr snippets;
   - run report;
   - event log;
   - replay summary.

### Не делать

- Не делать failure/reallocation.
- Не делать real hardware.
- Не делать Gazebo/HIL required validation.
- Не делать distributed onboard coordination.
- Не делать UI.

### Done criteria

- Two local PX4/SIH agents execute disjoint task subsets under one supervisor.
- Final report contains per-agent statuses.
- Event log is readable via replay summary.
- Duplicate ownership is rejected before upload.
- Mock/dry-run paths remain portable and do not require PX4.
- Docs state this is local PX4/SIH, not hardware readiness.

### Тесты

#### Tests that need no refactoring

- Multi-agent config parse/validation.
- Duplicate ownership rejection.
- Command generation for per-agent standalone commands.
- Replay summary roundtrip for synthetic multi-agent execute events.
- CLI conflicting `--mock` / `--connection` mode rejection.

#### Tests that need light refactoring

- Fake `Px4AgentController` execute lifecycle:
  - upload success;
  - start success;
  - progress ticks;
  - completion.
- Per-agent telemetry mapping test: `(agent_id, seq) -> task_id`.
- Supervisor report aggregation from two fake controllers.
- Per-agent event log ordering and `agent_id` presence.

#### Tests that need heavy refactoring

- Manual/ignored two-instance PX4/SIH execute integration.
- Parallel launch smoke.
- Time-bounded live SITL smoke not included in default CI.

---

## M59 - Live PX4/SIH Failure & Reallocation

### Цель

Перенести уже доказанный mock/runtime reallocation flow в controlled local
PX4/SIH supervisor flow.

После M59 проект сможет честно сказать:

- reallocation работает в runtime;
- reallocation работает в mock supervisor;
- controlled live/SIH supervisor flow тоже умеет lost agent -> release ->
  reassign -> survivor mission update.

### Минимальный failure scope

Не надо доказывать all possible failures. Для первой версии достаточно одного
контролируемого failure mode:

1. Supervisor перестает получать progress/heartbeat from one agent.
2. Agent marked lost after timeout.
3. Unfinished tasks from lost agent are released.
4. Runtime reallocation assigns recoverable tasks to survivor.
5. Survivor receives updated mission.
6. Event log records the chain.

### Failure injection options

Допустимые варианты:

- stop/kill one local PX4 process during execute;
- close one endpoint;
- use supervisor flag that simulates controller disconnect after N ticks;
- use one fake failing controller plus one real survivor as an intermediate step.

Рекомендация: start with controlled controller failure if process kill is flaky,
then capture one manual local SIH process-kill artifact if feasible.

### Survivor mission update policy

Нужно выбрать минимальную корректную стратегию.

#### Option A - mission replacement

- Stop/clear current survivor mission.
- Upload new combined remaining mission.
- Continue/start again.
- Easier to reason about and document.
- More intrusive for PX4 state.

#### Option B - supplementary upload

- Upload recovered waypoints as additional/supplementary mission work.
- Less disruptive conceptually.
- Harder to make correct across PX4 states.

Итоговая рекомендация: implement **Option A first**, but design names/events so
`supplementary upload` can be added later. If B.18's `--reupload-on-failure`
turns out simple with existing MAVLink upload path, it can be used as the user
facing flag. Internally it may still do mission replacement in M59.

### Что сделать

1. Live lost-agent detection:
   - heartbeat timeout;
   - telemetry no-progress timeout;
   - controller disconnect/error.
2. Stop tracking failed agent.
3. Release unfinished tasks assigned to failed agent.
4. Call runtime reallocation.
5. Compute recovered task ids and target survivor.
6. Build survivor mission update/replacement plan.
7. Upload mission update/replacement to survivor.
8. Continue survivor progress tracking where practical.
9. Record metrics:
   - `lost_agents`;
   - `released_tasks`;
   - `reassigned_tasks`;
   - `reassignment_count`;
   - `reallocation_latency_ticks`;
   - `tasks_recovered`;
   - `survivor_mission_updates`;
   - `final_completed_after_reallocation`.
10. Event log:
   - `agent_lost`;
   - `task_released`;
   - `task_reassigned`;
   - `reallocation_completed`;
   - optional `supplementary_upload_started`;
   - optional `supplementary_upload_completed`.
11. Captured artifact in `results/m59_px4_sih_failure_reallocation_YYYY-MM-DD/`.

### Не делать

- Не заявлять hardware readiness.
- Не делать all failure modes.
- Не делать distributed onboard reallocation.
- Не делать collision avoidance.
- Не делать production failover.

### Done criteria

- Fake/controller test: one agent fails, survivor receives recovered task.
- Manual/controlled local PX4/SIH artifact exists for at least one failure path.
- Replay summary shows:
  - `agent_lost=1`;
  - `task_released>=1`;
  - `task_reassigned>=1`;
  - `reallocation_completed=1`;
  - non-empty recovered tasks list.
- Final report has lost/reassigned/recovered metrics.
- Docs clearly state controlled local PX4/SIH only.

### Тесты

#### Tests that need no refactoring

- Existing runtime reallocation tests.
- Existing mock supervisor failure test.
- Replay event roundtrip for reallocation events.
- Task registry release/reassign tests.

#### Tests that need light refactoring

- Fake live controller failure:
  - fail before start;
  - fail during progress;
  - fail after completing one task.
- Mission replacement/supplementary plan construction test.
- Final report metrics aggregation test.
- Replay summary test for live-style failure events.

#### Tests that need heavy refactoring

- Manual/ignored two-PX4/SIH failure integration.
- Process-control harness that starts/kills PX4 instance deterministically.
- Property test: no duplicate ownership after arbitrary failure timing.

---

## M60 - PX4/SIH Supervisor Hardening

### Цель

Сделать live/SIH supervisor workflow достаточно устойчивым для повторяемых
research runs.

M58-M59 могут сначала быть narrow happy path and one failure case. M60 должен
закрыть инженерные шероховатости, иначе каждый ручной прогон будет требовать
разбора stdout and local state.

### Что сделать

1. Typed supervisor errors:
   - bad config;
   - invalid lifecycle combination;
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
3. Report schema hardening:
   - `schema_version`;
   - `run_id`;
   - `mode`;
   - `agents`;
   - `task_ownership`;
   - `events_summary`;
   - `final_status`;
   - `limitations`.
4. Output behavior:
   - create directories automatically;
   - avoid accidental overwrite unless explicit `--force` or unique run id;
   - stable result layout.
5. Docs:
   - exact local PX4/SIH startup commands;
   - multi-instance endpoint setup;
   - troubleshooting for port conflicts, heartbeat timeout, wrong system id;
   - interpreting reallocation artifacts;
   - explicit not-hardware statement.
6. Regression:
   - mock/fake paths remain default CI-safe;
   - live PX4/SIH remains manual/ignored.

### Не делать

- Не делать hardware checklist expansion beyond existing boundary.
- Не делать HIL/Gazebo required validation.
- Не делать public API semver promise.
- Не делать benchmark publication in this milestone.

### Done criteria

- Bad user inputs produce actionable errors.
- Partial agent failure produces structured report.
- Docs and tests agree on current behavior.
- Manual run artifacts are reproducible enough for another local run.
- No claim of hardware readiness appears in docs.

### Тесты

#### Tests that need no refactoring

- CLI rejects missing values and conflicting modes.
- Config validation errors include agent/task context.
- Replay summary handles failure reports.

#### Tests that need light refactoring

- Fake controller error matrix.
- Report schema snapshot-style tests.
- Output path behavior tests using temp directories.

#### Tests that need heavy refactoring

- End-to-end supervisor harness with multiple fake agents and randomized errors.
- Manual/ignored live PX4/SIH negative cases.

---

## M61 - Platform / API Stabilization

### Цель

После закрытия live/SIH supervisor gap задокументировать extension points проекта.

Это полезная часть из B.18, но ее лучше делать **после** M57-M60, а не до них.
Пока live multi-agent PX4/SIH partial, extension guide не закрывает главный
технический риск. После M57-M60 он становится логичным следующим шагом.

### Что сделать

1. Создать `docs/EXTENSION_GUIDE.md`.

   Документ должен покрывать:

   **Как добавить новую миссию:**

   - add/choose `TaskKind`;
   - implement `MissionAdapter`;
   - add scenario builder;
   - add scenario JSON/DSL;
   - define completion semantics;
   - add metrics;
   - add replay events if needed;
   - add regression smoke or explicitly mark unsupported.

   **Как добавить новую стратегию:**

   - implement allocator trait;
   - register in CLI/benchmark matrix;
   - document support matrix;
   - add regression/benchmark coverage.

   **Как добавить метрику:**

   - add field in run metrics;
   - aggregate it if needed;
   - export JSON/CSV/Markdown;
   - update docs/tests.

2. Document crate boundaries:

   - which crates are stable-ish extension points;
   - which are internal;
   - what should not be used by an external mission/strategy.

3. Schema version policy:

   - scenario schema;
   - replay schema;
   - report schema.

4. Add one test-only minimal extension path:

   - minimal mission adapter or fake mission fixture;
   - runner path;
   - replay/report path where practical.

### Не делать

- Не обещать stable public API/semver yet.
- Не publish crate.
- Не add new real mission here.

### Done criteria

- `docs/EXTENSION_GUIDE.md` exists.
- It explains mission, strategy and metrics extension paths.
- Crate boundaries and schema version policy are documented.
- At least one test validates the documented extension path where practical.

### Тесты

#### Tests that need no refactoring

- Docs test for required guide sections.
- Scenario/replay schema version presence tests if already available.

#### Tests that need light refactoring

- Minimal test mission fixture.
- Extension guide compliance smoke.
- Schema version validation helper.

#### Tests that need heavy refactoring

- External strategy harness.
- Cross-version schema compatibility tests.

---

## M62 - Benchmark / Baseline Refresh

### Цель

Обновить simulation benchmark claims after current technical direction is stable.

Benchmark до M57-M60 был бы premature: он улучшил бы simulator evidence, но не
закрыл бы `Real multi-agent PX4 is partial`. После M57-M61 benchmark refresh
становится полезным:

- live/SIH evidence есть;
- extension boundaries понятнее;
- regression deterministic;
- fresh benchmark can represent current HEAD.

### Что сделать

1. Decide benchmark scope:
   - supported mission-strategy pairs only;
   - unsupported pairs remain explicitly unsupported;
   - realism profiles either excluded or marked experimental.
2. Choose seed count:
   - 500 for cheaper refresh;
   - 1000 for publication-like artifact.
3. Run release build.
4. Fix manifest:
   - git commit;
   - seed range;
   - jobs count;
   - scenario versions;
   - machine summary if desired.
5. Generate:
   - JSON;
   - CSV;
   - Markdown table;
   - summary report.
6. Update:
   - `docs/BENCHMARK_RESULTS.md`;
   - README benchmark status;
   - `docs/STATUS.md`.

### Не делать

- Не использовать simulation benchmark как substitute for PX4/SIH evidence.
- Не включать unsupported pairs as success claims.
- Не делать paper-level statistical analysis unless explicitly chosen.

### Done criteria

- Fresh benchmark artifact exists for current HEAD.
- Historical benchmark docs no longer look current if stale.
- Regression runner remains green after benchmark-related changes.

### Тесты

#### Tests that need no refactoring

- Existing benchmark export tests.
- Regression runner default suite.
- Manifest/report identity tests.

#### Tests that need light refactoring

- Benchmark pack validation helper.
- Compare-baseline smoke on new artifact.
- Docs test for benchmark artifact paths.

#### Tests that need heavy refactoring

- Confidence interval tooling tests.
- Large-run reproducibility harness across machines.
- Statistical delta report validation.

---

## M63 - Next Branch Decision

### Цель

После M57-M62 снова выбрать стратегическое направление.

На этом этапе проект должен иметь:

- simulation foundation;
- deterministic regression gate;
- single-agent PX4/SIH execute evidence;
- multi-agent PX4/SIH execute evidence;
- controlled live/SIH failure/reallocation evidence;
- extension guide;
- refreshed benchmark baseline if M62 is executed.

Тогда выбор следующей ветки будет существенно лучше обоснован.

### Возможные направления после M63

#### Option A - Continue Real SITL / PX4/SIH

Без физического железа это может означать:

- better PX4 process orchestration;
- richer SIH/Gazebo optional validation;
- better timeout/abort policies;
- more mission shapes under SIH;
- repeatable local integration harness.

HIL/real hardware still remains separate future decision.

#### Option B - Disaster Mapping v2

Взять из B.18:

- flood scope decision;
- wildfire priority -> allocation scoring;
- success semantics hardening;
- wildfire/flood metrics.

Это стоит делать, если хочется развивать domain depth.

#### Option C - New Mission

Взять из B.18 кандидатов:

- Multi-target Pursuit;
- Logistics / Delivery.

Это стоит делать после M61, чтобы проверить extension path на реальной новой
миссии.

#### Option D - Algorithm Depth

Вернуться к:

- communication-aware allocation;
- mission-specific planner modes;
- hierarchical coordination;
- stronger CBBA under loss/failure.

Это лучше делать вместе с benchmark methodology.

#### Option E - Replay / Visualization

Развить:

- richer replay schema;
- run timeline;
- map view;
- comparison view.

Это полезно после live/SIH artifacts, потому что появятся более интересные logs.

## Почему не начинать сейчас с Disaster Mapping / Platform / New Mission

B.18 предлагает последовательность `M56 -> M57 -> M54 -> M55 -> M58`, потому что
Disaster Mapping and Platform work are more isolated. Это разумно с точки зрения
риска, но хуже с точки зрения главной цели проекта сейчас.

Главный открытый пункт в `docs/STATUS.md`:

> Real multi-agent PX4 is partial.

Новые mission/domain/platform tasks не закрывают этот пункт. Поэтому они должны
идти после M57-M60, а не перед ними.

## Почему не идти в физическое железо

Даже после M58/M59 проект останется local PX4/SIH research workflow. Для
physical drones нужны отдельные процессы и гарантии:

- physical safety review;
- operator procedure;
- geofence;
- emergency stop;
- hardware-specific failsafe tuning;
- legal/regulatory compliance;
- controlled flight environment.

Ничего из этого не входит в текущий проектный план. Текущая граница:

> local PX4/SIH and mock/fake validation only.

## Итоговый линейный план

1. M57 - Supervisor Controller Boundary.
2. M58 - Live Multi-Agent PX4/SIH Execute Orchestration.
3. M59 - Live PX4/SIH Failure & Reallocation.
4. M60 - PX4/SIH Supervisor Hardening.
5. M61 - Platform / API Stabilization.
6. M62 - Benchmark / Baseline Refresh.
7. M63 - Next Branch Decision.

## Итоговая рекомендация

Основным планом считать A.18, но усилить его деталями из B.18:

- live `sitl_supervisor --connection`;
- per-agent telemetry aggregation;
- sequential/parallel launch policy;
- multi-agent run started/finished events;
- supplementary upload / mission replacement after failure;
- extension guide as post-SITL milestone;
- new mission candidates as post-M63 branch material.

Не брать из B.18:

- смешивание live PX4/SIH with Disaster Mapping before closing supervisor gap;
- порядок, где platform/domain work идет раньше live multi-agent supervisor;
- wording that implies hardware readiness.

Практический первый шаг: **M57 Supervisor Controller Boundary**.
