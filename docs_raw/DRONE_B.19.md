# DRONE_B.19 — Итоговый план: Траектории B + C (без hardware)

Дата фиксации: 2026-05-30

**Горизонт:** M57–M65. Последний завершённый плановый milestone — M53.

Основа: синтез `DRONE_A.18.md` и `DRONE_B.18.md`.

---

## Контекст

Ветка 6 (Real SITL / PX4) завершена: M43–M53. Неформальные result-артефакты:

- `results/m55_multi_agent_px4_sih_2026-05-30/` — двухэкземплярный
  upload-only check в рамках M52;
- `results/m56_regression_determinism_2026-05-30/` — determinism sweep
  в рамках M39a.

Это директории результатов, не плановые milestones. Следующий плановый
milestone — **M57** (чтобы не путать с result-артефактами m54/m55/m56).

---

## Чем отличались A.18 и B.18

### Главное расхождение

`DRONE_A.18.md` сосредоточен исключительно на Trajectory B (live multi-agent
PX4 SITL) и вводит критический промежуточный шаг: **M57 Supervisor Controller
Boundary** — рефакторинг `sitl_supervisor` до добавления реального
`Px4AgentController`. Без этого шага добавление live PX4 controller в текущий
supervisor превращает его в неподдерживаемую лапшу: CLI, mock runtime,
heartbeat simulation, task completion, реаллокация, метрики и event log сейчас
смешаны в одном файле.

`DRONE_B.18.md` пропускает этот шаг и сразу переходит к live PX4, зато
включает Trajectory C целиком: Disaster Mapping v2, Platform/API, New Mission.

### Что взято из каждого

Из `DRONE_A.18.md`:
- нумерация с M57;
- M57 Supervisor Controller Boundary как обязательный prerequisite;
- архитектурные детали live PX4 (per-agent lifecycle state machine, выбор
  survivor mission update policy);
- M60 PX4 Supervisor Hardening (typed errors, exit codes, report schema);
- M64 Benchmark / Baseline Refresh;
- M65 Algorithm Depth Decision Point.

Из `DRONE_B.18.md`:
- M61 Disaster Mapping v2 (flood scope decision, priority → allocation);
- M62 Platform / API Stabilization (extension guide, crate boundaries);
- M63 New Mission (Pursuit или Logistics/Delivery).

### Что исключено

- Hardware / HIL — явно вне scope этого плана. `docs/HARDWARE_READINESS.md`
  и `--allow-hardware-candidate` guard уже оформляют эту границу; идти дальше
  не планируется.
- Gazebo validation — не включается как gate.

---

## Принцип плана

B (live multi-agent PX4) и C (расширение платформы) независимы по коду.
C можно начинать параллельно с M58/M59. Внутри B: M57 → M58 → M59 → M60
строго последовательно. Внутри C: M61 → M62 → M63 последовательно.

Hardware явно исключён. Все live PX4 milestones работают с локальным PX4 SIH,
не с реальными дронами.

---

## Линейный план

```
B:  M57  Supervisor Controller Boundary
      -> M58  Live Multi-Agent PX4 Execute
        -> M59  Live PX4 Failure / Reallocation
          -> M60  PX4 Supervisor Hardening

C:  M61  Disaster Mapping v2              (независима, можно параллельно с M58/M59)
      -> M62  Platform / API Stabilization
        -> M63  New Mission

    M64  Benchmark / Baseline Refresh     (после закрытия B и C)
    M65  Algorithm Depth Decision Point   (стратегическая развилка)
```

---

## M57 — Supervisor Controller Boundary

**Цель:** отделить supervisor state machine от конкретного способа управления
агентом до добавления реального PX4 controller.

**Суть:** `sitl_supervisor.rs` сейчас смешивает CLI parsing, manifest
generation, mock runtime setup, heartbeat simulation, task completion,
реаллокацию, метрики и event log writing. Для live PX4 execute path это
становится хрупким. Нужна явная граница:

```text
Supervisor
  owns: run lifecycle, metrics, event log, task ownership, runtime coordinator

AgentController
  owns: один agent lifecycle: upload, execute, poll progress, abort, final status
```

### Предлагаемая архитектура

Начинать внутри `swarm-examples`, не выносить преждевременно в публичный crate.

```rust
trait AgentController {
    fn agent_id(&self) -> &str;
    fn upload(&mut self, plan: &AgentMissionPlan) -> Result<AgentStep, SitlError>;
    fn start(&mut self) -> Result<AgentStep, SitlError>;
    fn poll(&mut self, tick: u64) -> Result<AgentProgress, SitlError>;
    fn abort(&mut self, reason: &str) -> Result<AgentStep, SitlError>;
}
```

`MockAgentController` сохраняет текущее поведение `--mock`. `Px4AgentController`
появится в M58.

### Что сделать

1. Вынести из `sitl_supervisor.rs` чистую supervisor state machine.
2. Ввести `MockAgentController`, который сохраняет текущее поведение.
3. Оставить CLI behaviour совместимым: `--dry-run`, `--mock`, `--manifest`,
   `--replay-log`, `--fail-agent`, `--fail-after-ticks`,
   `--heartbeat-timeout-ticks`, `--max-ticks`.
4. Supervisor metrics выделить в отдельную структуру, тестируемую без binary.
5. Сохранить текущий deterministic failure/reallocation test без изменений.

### Не делать в M57

- Не добавлять реальный PX4 controller.
- Не менять MAVLink protocol.
- Не менять runtime reallocation semantics.
- Не стабилизировать публичный API.

### Done criteria

- `sitl_supervisor --mock` поведение не сломано.
- Существующий mock reallocation artifact воспроизводим.
- Supervisor logic тестируется без subprocess там, где практично.
- В коде появилась понятная точка расширения для `Px4AgentController`.
- `cargo test -p swarm-examples --test sitl_agent` проходит.

### Тесты

#### Без рефакторинга

- Существующие subprocess тесты `sitl_supervisor --mock`.
- Duplicate ownership rejection.
- Missing/invalid CLI args.
- Replay summary содержит reallocation events.

#### Лёгкий рефакторинг

- Unit тесты supervisor state transitions с fake controllers.
- Metrics aggregation без запуска binary.
- Deterministic failure schedule тесты:
  - fail before upload;
  - fail after upload;
  - fail during progress.

#### Тяжёлый рефакторинг

- Property tests над произвольными failure schedules.
- Cross-check replay events against final task registry state.

---

## M58 — Live Multi-Agent PX4 Execute

**Цель:** первый live PX4 SIH multi-agent execute workflow.

**Суть:** M55 (upload-only artifact) доказал, что два PX4 SIH инстанса
принимают разные task subsets. M58 должен доказать, что они выполняются как
один supervised run с общим lifecycle, event log и final report.

### Минимальный сценарий

- 2 агента, `scenarios/sitl.multi-agent.json` /
  `scenarios/sitl.multi-agent.config.json`;
- разные MAVLink endpoints, разные `system_id`;
- lifecycle: `execute` (upload → arm → takeoff → start → telemetry → complete);
- disjoint task subsets, единый `run_id`, единый event log, единый final report.

### Что сделать

1. Добавить `Px4AgentController` (feature-gated `mavlink-transport`):
   - переиспользовать logic из `sitl_agent.rs` через library functions,
     не subprocess;
   - subprocess проще, но хуже для failure/reallocation и event merge.
2. Per-agent lifecycle state machine:
   `Pending → Uploaded → Started → InProgress → Completed / Failed / Aborted`.
3. Supervisor ведёт telemetry aggregation по всем агентам:
   - отдельный канал на каждого;
   - mapping `(agent_id, seq)` → `task_id` → `TaskStatus`;
   - per-agent no-progress timeout;
   - disconnect одного агента → abort только этого агента.
4. Event log: поле `agent_id` корректно заполняется для каждого агента.
5. Final multi-agent run report:
   - `agents_count`, `run_id`, `scenario`, `config`;
   - per-agent: `agent_id`, `connection_string`, `mission_item_count`,
     `completed_count`, `final_status`, `error`;
   - `total_completed`, `total_failed`, `overall_status`, `duration`.
6. CLI sketch:

```bash
cargo run -p swarm-examples --features mavlink-transport \
  --bin sitl_supervisor -- \
  --connection-execute \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.config.json \
  --replay-log results/.../run.sitl-log.json \
  --run-report results/.../report.json \
  --timeout 120 --telemetry-timeout 30 --no-progress-timeout 45 \
  --allow-hardware-candidate
```

7. Captured PX4 SIH artifact в `results/`.
8. Задокументировать tested setup в `docs/SITL_SETUP.md`:
   - как запустить два PX4 SIH инстанса;
   - expected ports, system IDs;
   - troubleshooting: port conflicts, heartbeat timeout, wrong system_id.

### Не делать в M58

- Не делать failure/reallocation (это M59).
- Не делать hardware.
- Не делать complex distributed coordination между PX4 agents.

### Done criteria

- Два PX4 SIH агента выполняют disjoint task subsets.
- Supervisor завершает run, пишет общий report.
- Event log читается `replay --summary`.
- Duplicate ownership отклоняется до upload.
- Есть result artifact directory.
- Docs честно говорят: local PX4 SIH, not hardware.

### Риски

- PX4 SIH может вести себя иначе для двух инстансов, чем upload-only.
- Concurrent MAVLink polling per endpoint требует аккуратного timing.
- Telemetry progress может быть noisy; timeouts нужно подбирать консервативно.

### Тесты

#### Без рефакторинга

- Multi-agent config parse/validation.
- Duplicate ownership rejection.
- Replay summary работает для synthetic multi-agent execute events.
- CLI: `--connection-execute` без feature → typed error.

#### Лёгкий рефакторинг

- Fake `Px4AgentController`: upload success, start success, progress ticks,
  completion.
- Supervisor report aggregation из двух fake controllers.
- Per-agent event log ordering и `agent_id` presence.

#### Тяжёлый рефакторинг

- Manual/ignored real PX4 SIH integration test.
- Time-bounded live SITL smoke (локальный, не в default CI).

---

## M59 — Live PX4 Failure / Reallocation

**Цель:** перенести доказанный mock/runtime reallocation flow в live PX4
supervisor.

**Суть:** mock supervisor уже детектирует потерю агента, реаллоцирует задачи,
пишет события в event log — всё это покрыто детерминированными тестами. M59
переносит этот flow в live connection path.

### Failure modes для первой версии

Минимально достаточно одного контролируемого failure mode:

1. Supervisor перестаёт получать progress/heartbeat от агента.
2. Агент помечается lost после timeout.
3. Незавершённые задачи released через runtime reallocation.
4. Survivor получает recovered tasks.
5. Event log записывает полную цепочку.

Для manual run: остановить один PX4 process, или закрыть его endpoint, или
использовать `--fail-agent <id>` с контролируемым disconnect одного real агента
при живом survivor.

### Survivor mission update policy

**Option A — Mission replacement (рекомендуется для M59):**

- Stop/clear текущую mission у survivor.
- Upload новой объединённой remaining mission.
- Restart/continue.
- Проще рассуждать, детерминированнее, прозрачнее.
- Более intrusive для PX4 state — приемлемо для research/SITL.

**Option B — Append/partial update:** менее разрушительно, но сложнее сделать
корректно и portable. Отложить на будущее.

### Что сделать

1. Live lost-agent detection в supervisor connection loop:
   - per-agent heartbeat timeout;
   - telemetry no-progress timeout;
   - controller disconnect/error.
2. Wire lost-agent event в runtime reallocation (уже существует в mock path).
3. Конвертировать recovered task ids в survivor mission plan.
4. Upload replacement mission на survivor (Option A).
5. Продолжить telemetry tracking у survivor.
6. Метрики в final report:
   - `lost_agents`, `released_tasks`, `reassigned_tasks`;
   - `reassignment_count`, `reallocation_latency_ticks`;
   - `tasks_recovered`, `survivor_mission_updates`;
   - `final_completed_after_reallocation`.
7. Новые event types:
   - `SitlSupplementaryUploadStarted { agent_id, new_waypoint_count, tick }`;
   - `SitlSupplementaryUploadCompleted { agent_id, accepted, tick }`.
8. Captured artifact: README, command, PX4 version, event log, replay summary,
   final report.

### Не делать в M59

- Не заявлять hardware readiness.
- Не делать robust production failover.
- Не гарантировать collision avoidance.
- Supervisor остаётся централизованным.

### Done criteria

- Детерминированный fake тест: один live-style controller fails, survivor
  получает recovered tasks.
- Manual PX4 SIH artifact для хотя бы одного live/simulated failure path.
- Event log summary: `agent_lost=1`, `task_released>=1`, `task_reassigned>=1`,
  `reallocation_completed=1`.
- Docs чётко описывают, что доказано, а что нет.

### Риски

- Mission replacement может disrupting PX4 mission state у survivor.
- Если survivor уже mid-flight — replace может создавать confusing telemetry.
- Lost agent может упасть после завершения задачи, но до final telemetry:
  supervisor не должен реаллоцировать уже completed задачи.

### Тесты

#### Без рефакторинга

- Runtime reallocation tests (уже есть).
- Mock supervisor failure test (уже есть).
- Replay event roundtrip для reallocation events.
- Task registry release/reassign tests.

#### Лёгкий рефакторинг

- Fake live controller failure: fail before start / during progress /
  after completing one task.
- Mission replacement plan test для survivor.
- Final report metrics aggregation.
- Replay summary test для live-style failure events.

#### Тяжёлый рефакторинг

- Manual/ignored two-PX4 SIH failure integration.
- Property tests: no duplicate ownership after arbitrary failure timing.

---

## M60 — PX4 Supervisor Hardening

**Цель:** сделать live supervisor не одноразовым экспериментом, а достаточно
надёжным research workflow.

**Суть:** M58–M59 могут сначала быть narrow happy path + один failure case.
M60 закрывает инженерные шероховатости, которые иначе мешают каждому
следующему прогону.

### Что сделать

1. Typed supervisor errors:
   - `BadConfig`, `EndpointUnavailable`, `HeartbeatTimeout`;
   - `MissionUploadFailed`, `CommandRejected`, `ProgressTimeout`;
   - `AbortFailed`, `PartialRunFailed`.
2. Consistent exit codes:
   - config/CLI error; safety validation error; PX4 unavailable;
   - mission rejected; runtime failure after start.
3. Structured report schema:
   - `schema_version`, `run_id`, `mode`, `agents`;
   - `task_ownership`, `events_summary`, `final_status`, `limitations`.
4. Idempotent output directories:
   - создавать автоматически;
   - не перезаписывать без `--force` или unique run id.
5. Документация:
   - точные команды запуска локального PX4 SIH в multi-instance режиме;
   - troubleshooting для multi-instance endpoints;
   - как интерпретировать reallocation artifacts;
   - что out of scope.
6. Regression:
   - mock path остаётся default CI-safe;
   - live PX4 остаётся manual/ignored.

### Done criteria

- Bad user inputs дают actionable errors.
- Partial agent failure даёт structured report, а не ambiguous stdout.
- Docs и тесты согласованы с текущим поведением.
- Manual run artifacts воспроизводимы для повторного локального прогона.

### Тесты

#### Без рефакторинга

- CLI rejects missing values и conflicting modes.
- Config validation errors включают agent/task context.
- Replay summary handles failure reports.

#### Лёгкий рефакторинг

- Fake controller error matrix.
- Report schema snapshot-ish тесты.
- Output path behaviour тесты с temp directories.

#### Тяжёлый рефакторинг

- End-to-end supervisor harness с несколькими fake agents и randomized errors.
- Manual/ignored live PX4 negative cases.

---

## M61 — Disaster Mapping v2

**Цель:** закрыть технический долг по Ветке 2 (BRANCHES.md): flood scope
decision и priority → allocation.

**Независима от B пути.** Можно начинать параллельно с M58 или M59.

### Пункт A — Flood scope decision

Название «wildfire / flood mapping» в коде, docs и README обещает flood,
которого нет. Нужно явно закрыть.

**Вариант A — Cleanup (рекомендуется, если flood не является приоритетом):**

- Убрать «flood» из `WildfireConfig` doc comment, README Quick Start,
  wildfire scenario file headers, `docs/SITL_SETUP.md`.
- Явно задокументировать flood как future work.
- Wildfire остаётся единственной disaster mapping миссией.

**Вариант B — Minimal Flood Implementation:**

- `FloodConfig`: `flooded_zones`, `water_spread_rate`, `critical_zones`,
  `rescue_priority_tasks`.
- Scenario files: `scenarios/flood.small-static.json`,
  `scenarios/flood.medium-dynamic.json`.
- Metrics: `flooded_zones_mapped`, `critical_zones_mapped`,
  `time_to_first_critical`, `final_risk_level`.
- `TaskKind::MappingZone` переиспользуется — новый `FloodMappingAdapter`.
- Replay events: `FloodZoneUpdated`, `CriticalZoneDetected`.
- Regression smoke как experimental.

### Пункт B — Priority → allocation scoring

Сейчас wildfire priority updates обновляют поле и пишут replay events, но
allocation scoring в текущем тике не использует `priority`. Нужно исправить.

1. Добавить `priority_weight: f64` в конфиг wildfire scoring.
2. Расширить `WildfireMappingAdapter::score`:
   `score = base_score + priority_weight * task.priority as f64`.
3. Тест: два zone tasks с одинаковым расстоянием, разным приоритетом →
   высокоприоритетный назначается первым.
4. Benchmark delta: `time_to_map_first_high_risk` с и без `priority_weight`.

### Пункт C — Success semantics hardening

- Зафиксировать задокументированный success rule для `small-static` и
  `medium-dynamic`.
- Тест: `success == (task_completion_rate >= threshold)` для каждого сценария.

### Done criteria

- Flood вопрос закрыт: либо явно out-of-scope в docs, либо реализован.
- `priority_weight` влияет на assignment order.
- Wildfire success semantics задокументированы и покрыты тестами.

### Тесты

#### Без рефакторинга

- Priority scoring: high vs low priority при равном расстоянии.
- Success semantics consistency для `small-static` и `medium-dynamic`.
- Существующие wildfire scenario load tests.
- (Вариант B) `FloodConfig` parse, `FloodMappingAdapter` completion,
  replay event roundtrip для flood events.

#### Лёгкий рефакторинг

- Wildfire fixture с контролируемыми приоритетами.
- Scoring comparison helper.
- `time_to_map_first_high_risk` assertion helper.

#### Тяжёлый рефакторинг

- Property tests для dynamic priority updates.
- Multi-seed comparison benchmark с/без `priority_weight`.
- (Вариант B) flood regression suite.

---

## M62 — Platform / API Stabilization

**Цель:** задокументировать extension points так, чтобы новая миссия или
стратегия добавлялись без изменений ядра.

**Суть:** M63 (New Mission) требует чистого extension path. M62 создаёт и
верифицирует его.

### Что сделать

1. Создать `docs/EXTENSION_GUIDE.md`:

   **Как добавить миссию:**
   - `TaskKind` вариант;
   - `MissionAdapter` impl (score, route_cost, is_completed);
   - scenario builder (Rust) + scenario JSON (DSL);
   - метрики в `RunMetrics` / `AggregateMetrics` / export schema;
   - replay events;
   - regression smoke suite.

   **Как добавить стратегию:**
   - `Allocator` trait impl;
   - регистрация в CLI / `AdapterRegistry`;
   - добавление в benchmark matrix;
   - regression suite или явный unsupported.

   **Как добавить метрику:**
   - поле в `RunMetrics`;
   - `AggregateMetrics`;
   - JSON/CSV/Markdown export;
   - manifest schema.

2. Stable crate boundaries: задокументировать публичные
   (`swarm-types`, `swarm-sim`, `swarm-scenarios`, `swarm-comms`) и
   internal (`swarm-runtime`, `swarm-alloc`).

3. Schema version policy:
   - scenario files: `schema_version: "0.1"` уже есть, задокументировать
     minor vs major bump;
   - replay log format: backward compatibility policy.

4. Integration test — `MinimalTestMission` без изменений ядра:
   - реализует `MissionAdapter`;
   - строится через DSL fixture;
   - проходит через runner, `AdapterRegistry`, replay.

### Done criteria

- `docs/EXTENSION_GUIDE.md` покрывает mission, strategy, metrics paths.
- Integration test добавляет `MinimalTestMission` без изменений в
  `swarm-types`, `swarm-runtime`, `swarm-alloc`, `swarm-sim`.
- Crate boundaries задокументированы.
- Schema version policy зафиксирована.

### Тесты

#### Без рефакторинга

- `MinimalTestMission` DSL parse → runner → replay roundtrip.
- Schema version field присутствует в scenario и replay fixtures.

#### Лёгкий рефакторинг

- `MinimalTestMission` fixture builder.
- Extension guide compliance assertion.

#### Тяжёлый рефакторинг

- External strategy harness (отдельный binary без patch к ядру).
- Schema compatibility tests across versions.

---

## M63 — New Mission

**Цель:** добавить принципиально новый класс миссий, который проверяет
coordination mechanics, недоступные в существующих миссиях.

**Суть:** Coverage, SAR, Inspection, Wildfire устроены по одному паттерну:
агент получает набор точек/зон/рёбер и посещает их. M63 добавляет один из
двух кандидатов через extension path из M62.

### Кандидат 1 — Multi-target Pursuit

Движущиеся цели. Агенты перехватывают или сопровождают.

**Domain model:**
- `TaskKind::Pursuit { target_id, mode: CaptureOrEscort, proximity_radius }`;
- `PursuitTarget { id, trajectory: Vec<Pose>, speed }`;
- `RunState::active_targets: HashMap<TargetId, Pose>`.

**Механика:** цели движутся по заданным траекториям или по простой модели
уклонения. Задача completed, когда агент входит в `proximity_radius`. Задачи
динамически появляются и исчезают.

**Метрики:** `capture_rate`, `time_to_intercept`, `targets_lost`,
`total_pursuit_distance`.

**Выбирать, если:** цель — проверить алгоритмическую реактивность и динамику
пересмотра задач.

### Кандидат 2 — Logistics / Delivery

Задачи с precedence constraints и capacity limits.

**Domain model:**
- `TaskKind::Pickup { item_id, location }`;
- `TaskKind::Dropoff { item_id, location, requires_pickup: TaskId }`;
- `AgentState::cargo: Vec<ItemId>`;
- `RunState::delivered_items: HashSet<ItemId>`.

**Механика:** нельзя доставить не забрав. Capacity: агент несёт ограниченный
груз. Deadlines: time window для некоторых задач.

**Метрики:** `delivery_rate`, `late_deliveries`, `capacity_violations`,
`total_route_cost`, `unserved_deliveries`.

**Выбирать, если:** цель — проверить обобщаемость DSL и task dependency
handling.

### Что сделать (общее)

1. Domain model: `TaskKind` вариант, state fields, target/item structs.
2. DSL: новые поля в scenario JSON, параметры генерации, DSL validation.
3. `MissionAdapter` impl: score, route_cost, is_completed.
4. `RunState` extension: минимально необходимые поля.
5. Allocation compatibility: какие стратегии stable, experimental, unsupported.
6. Replay events: mission-specific события.
7. Метрики: `RunMetrics` / `AggregateMetrics` / export.
8. Scenarios: `scenarios/pursuit.small.json` + `scenarios/pursuit.medium.json`
   (или `delivery.*`).
9. Regression smoke: experimental threshold.
10. Пополнить `docs/EXTENSION_GUIDE.md` живым примером.

### Done criteria

- Новая миссия добавлена без изменений в `swarm-runtime`, `swarm-alloc`.
- Минимум два сценария (small / medium).
- Benchmark запускается для stable стратегий.
- Replay содержит mission-specific события.
- Support matrix задокументирована.
- Regression smoke проходит.

### Тесты

#### Без рефакторинга

- DSL parse/validation для нового scenario type.
- Task generation unit test.
- Completion semantics test.
- Replay event serialization roundtrip.
- Benchmark smoke для small scenario.

#### Лёгкий рефакторинг

- Mission-specific fixture builder.
- Outcome assertion helpers.

#### Тяжёлый рефакторинг

- Property tests: динамическое появление/исчезновение задач (Pursuit) или
  precedence constraint satisfaction (Logistics).
- Multi-seed stability tests.
- Comparative strategy tests.

---

## M64 — Benchmark / Baseline Refresh

**Цель:** обновить simulation benchmark claims после закрытия B и C.

**Суть:** до M57–M63 большой benchmark был бы полезен, но не закрывал бы
главный оставшийся разрыв. После B+C у проекта сильная позиция: simulation
benchmark + live PX4 SIH evidence + новая миссия.

### Что сделать

1. Определить scope: только supported mission-strategy пары; unsupported
   остаются явно unsupported; realism profiles — experimental или excluded.
2. Согласовать seed count: 500 если важно время/стоимость, 1000 для
   publication-like артефакта.
3. Release build.
4. Manifest: git commit, seed range, jobs count, schema version.
5. Артефакты: JSON, CSV, Markdown table, summary report.
6. Обновить `docs/BENCHMARK_RESULTS.md`, README, `docs/STATUS.md`.

### Не делать в M64

- Не использовать benchmark как substitute для live PX4 evidence.
- Не включать unsupported пары как success claims.
- Не делать paper-level statistical analysis — это отдельное решение.

### Done criteria

- Fresh benchmark artifact для текущего HEAD.
- Historical benchmark docs больше не выглядят актуальными, если устарели.
- Regression runner остаётся зелёным.

### Тесты

#### Без рефакторинга

- Существующие benchmark export tests.
- Regression runner default suite.
- Manifest/report identity tests.

#### Лёгкий рефакторинг

- Benchmark pack validation helper.
- Compare baseline smoke на новом артефакте.

#### Тяжёлый рефакторинг

- Confidence interval tooling tests.
- Statistical delta report validation.

---

## M65 — Algorithm Depth Decision Point

**Цель:** осознанно выбрать следующее стратегическое направление.

После M57–M64 проект будет иметь:

- simulation foundation + deterministic regression gate;
- single-agent PX4 SIH execute evidence (M48);
- multi-agent PX4 SIH execute evidence (M58);
- live failure/reallocation (M59);
- disaster mapping v2 (M61);
- extension guide + new mission (M62–M63);
- benchmark refresh (M64).

Это хорошая точка для осознанной развилки.

### Варианты после M65

**Option A — Algorithm Depth (Ветка 1 из BRANCHES.md):**

- Communication-aware allocation: `message_budget` как soft penalty;
  benchmark CBBA vs greedy по message count / success tradeoff.
- Mission-specific planner modes: SAR greedy-by-uncertainty, wildfire
  priority-weighted nearest neighbour.
- Hierarchical coordination для 8+ агентов.

Выбирать, если цель — улучшить сами алгоритмы и получить измеримые
преимущества конкретных стратегий.

**Option B — Research Benchmark (Ветка 3 из BRANCHES.md):**

- 1000-seed runs с confidence intervals.
- Degradation curves по packet loss, числу агентов, размеру сетки.
- Strategy comparison report: где CBBA выигрывает у greedy, где проигрывает.
- Publication-quality results.

Выбирать после Option A или если алгоритмы уже достаточно зрелые.

**Option C — Replay / Visualization (Ветка 5 из BRANCHES.md):**

- Wildfire/realism события в replay summary.
- ASCII overlay для SAR belief grid, wildfire hazard grid.
- Timeline viewer (egui или Bevy) — опционально.

Выбирать, если цель — анализ и демонстрация поведения миссий.

**Hardware путь исключён** из вариантов этого плана.

---

## Сводная таблица

| Milestone | Траектория | Результат | Зависит от | Параллельность |
|---|---|---|---|---|
| M57 | B | `AgentController` trait, `MockAgentController`, точка расширения | M53 | — |
| M58 | B | Live multi-agent PX4 execute: upload + arm + telemetry для N агентов | M57 | — |
| M59 | B | Live PX4 failure detection + реаллокация + mission replacement | M58 | — |
| M60 | B | Typed errors, exit codes, report schema, hardening | M59 | — |
| M61 | C | Flood scope closed; priority → allocation; success semantics | M53 | Параллельно с M58/M59 |
| M62 | C | Extension guide, crate boundaries, schema policy | M61 (желательно) | После M61 |
| M63 | C | Новый класс миссий (Pursuit или Logistics) | M62 | После M62 |
| M64 | B+C | Benchmark refresh для текущего HEAD | M60, M63 | После B и C |
| M65 | — | Algorithm Depth Decision Point | M64 | — |

---

## Рекомендуемый старт

**Первый шаг — M57 Supervisor Controller Boundary.**

Почему:
- не требует PX4;
- сохраняет текущие mock tests без изменений;
- существенно снижает риск M58/M59;
- быстро покажет, насколько `sitl_agent` можно переиспользовать как library;
- после M57 можно принять более точное решение по реализации M58
  (in-process `Px4AgentController` vs subprocess vs hybrid).

**Параллельно** можно начинать **M61 Disaster Mapping v2** — он независим
по коду от B пути и закрывает давно открытый технический долг.
