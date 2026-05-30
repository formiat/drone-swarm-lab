# DRONE_B.18 — План развития: Траектории B + C

Дата фиксации: 2026-05-30

**Горизонт:** M54–M58. Последний завершённый плановый milestone — M53.

Основа: анализ текущего состояния кода, коммитов, README, STATUS.md и
BRANCHES.md по состоянию на 2026-05-30.

---

## Контекст

Ветка 6 (Real SITL / PX4) полностью завершена: M43–M53. Помимо неё, есть два
неформальных артефакта результатов:

- `results/m55_multi_agent_px4_sih_2026-05-30/` — двухэкземплярный
  upload-only PX4 SIH check в рамках M52;
- `results/m56_regression_determinism_2026-05-30/` — детерминизм sweep
  regression в рамках M39a.

Эти директории не являются самостоятельными плановыми milestones — они
вспомогательные артефакты, зафиксированные при выполнении M52 и M39a.
Следующий плановый milestone — **M54**.

---

## Выбранный фокус: Траектории B + C

**Траектория B — Аппаратная готовность (Live Multi-Agent PX4):**

Сейчас `sitl_supervisor --mock` реализует полную реаллокацию при потере агента
и тестируется детерминированно. `sitl_agent --connection --execute` выполняет
одиночный PX4 SITL run. Следующий логический шаг — связать эти два слоя: дать
supervisor реальные MAVLink connection'ы для нескольких агентов и добавить live
failure handling.

**Траектория C — Расширение платформы:**

Три самостоятельных направления, которые делают платформу более универсальной:

1. Закрыть технический долг по Disaster Mapping (Ветка 2 из BRANCHES.md):
   flood scope decision и priority → allocation.
2. Стабилизировать extension points (Ветка 7): задокументировать, как добавить
   миссию или стратегию без изменений ядра.
3. Добавить принципиально новый класс миссий (Ветка 8): задачи с динамическими
   целями или precedence constraints, которых сейчас нет.

**Почему B и C совместимы:**

B и C независимы по зависимостям кода. Внутри B: M54 → M55 (последовательно).
Внутри C: M56 → M57 → M58 (M57 идеально до M58, но не жёстко). B и C можно
выполнять параллельно или в любом порядке между собой.

---

## Линейный план

```
B:  M54  Live Multi-Agent PX4 Execute Orchestration
      -> M55  Live PX4 Failure & Reallocation in Supervisor

C:  M56  Disaster Mapping v2
      -> M57  Platform / API Stabilization
        -> M58  New Mission
```

---

## M54 — Live Multi-Agent PX4 Execute Orchestration

**Цель:** запустить несколько реальных PX4 SITL инстансов через `sitl_supervisor`
с полным lifecycle для каждого агента.

**Суть:** сейчас `sitl_supervisor --mock` работает, но только в памяти.
`sitl_agent --connection --execute` умеет запускать полный lifecycle для одного
агента. M54 соединяет их: supervisor получает реальные MAVLink connection'ы,
запускает upload + arm + takeoff + execute для каждого агента, собирает telemetry
от всех агентов и строит итоговый multi-agent report.

### Что сделать

1. Добавить режим `--connection` в `sitl_supervisor`:
   - feature-gated `mavlink-transport`;
   - connection strings берутся из `sitl.multi-agent.config.json` (поле уже есть);
   - без feature → понятная ошибка с инструкцией по сборке.

2. Для каждого агента из конфига:
   - создать `MavlinkTransport::new(connection_string, agent_id)`;
   - выполнить safety validation перед upload;
   - выполнить mission upload handshake (`MISSION_CLEAR_ALL`, `MISSION_COUNT`,
     `MISSION_REQUEST_INT`, `MISSION_ITEM_INT`, `MISSION_ACK`);
   - отправить arm / takeoff / set_auto_mode команды;
   - запустить telemetry loop (`MISSION_CURRENT`, `MISSION_ITEM_REACHED`).

3. Параллельность и порядок:
   - поддержать per-agent `start_delay_ms` из конфига (уже есть в структуре);
   - выбор между последовательным и параллельным запуском через CLI flag
     (`--sequential` / `--parallel`, default sequential для безопасности);
   - `--upload-only` режим (уже реализован в `sitl_agent`), перенести на
     уровень supervisor.

4. Multi-agent telemetry aggregation:
   - отдельный канал событий на каждого агента;
   - mapping: `(agent_id, seq)` → `task_id` → `TaskStatus`;
   - no-progress timeout per agent;
   - disconnect per agent → abort того агента, не всего supervisor run.

5. Final multi-agent run report:
   - scenario;
   - per-agent: agent_id, connection_string, mission item count, completed count,
     final status, error if any;
   - total completed / total failed / overall status.

6. Расширить SITL event log:
   - события уже имеют `agent_id` поле — его нужно заполнять корректно для каждого
     агента в multi-agent run;
   - добавить событие `SitlMultiAgentRunStarted { agent_count, scenario }`;
   - добавить событие `SitlMultiAgentRunFinished { overall_status }`.

7. Документировать tested setup в `docs/SITL_SETUP.md`:
   - как запустить два PX4 SIH инстанса с разными system_id и портами;
   - connection strings для каждого;
   - troubleshooting: port conflicts, heartbeat timeout, wrong system_id.

### Done criteria

- `sitl_supervisor --connection --scenario ... --config ...` запускает upload
  для двух PX4 SIH инстансов и получает accepted MISSION_ACK от каждого;
- с `--execute` оба агента проходят arm/takeoff/start и telemetry loop;
- final report содержит per-agent статус;
- mock/dry-run пути остаются portable и не требуют PX4;
- captured run artifact в `results/`.

### Тесты

#### Без рефакторинга

- Multi-agent run report serialization roundtrip test.
- `SitlMultiAgentRunStarted` / `SitlMultiAgentRunFinished` event roundtrip.
- CLI: `--connection` без feature → typed error с понятным сообщением.
- CLI: conflicting `--mock` и `--connection` → typed error.
- Per-agent telemetry mapping: `(agent_id, seq)` → task_id test с fake events.

#### Лёгкий рефакторинг

- Fake multi-agent MAVLink connection fixture (два независимых fake streams).
- Multi-agent run report builder helper.
- Per-agent telemetry channel test helper.

#### Тяжёлый рефакторинг

- Real two-instance PX4 SIH execute integration test (ручной / feature-gated).
- Parallel launch smoke (requires real PX4 or heavy fake).

---

## M55 — Live PX4 Failure & Reallocation in Supervisor

**Цель:** детектировать потерю агента в live multi-agent PX4 run и перераспределить
его незавершённые задачи на оставшихся агентов.

**Суть:** в mock режиме реаллокация уже работает и покрыта тестами. M55 переносит
этот flow в live connection path: supervisor теряет heartbeat от реального PX4
инстанса, освобождает задачи упавшего агента через runtime reallocation, определяет
можно ли дать оставшимся агентам эти задачи через supplementary upload, фиксирует
всё это в event log.

### Что сделать

1. Heartbeat monitoring в live connection loop:
   - per-agent heartbeat timeout (из конфига или CLI, default совпадает с mock);
   - при отсутствии heartbeat → агент считается lost;
   - вызов `AgentNode::process_inbox_and_allocate` с отсутствующими heartbeats
     уже триггерит `failure_releases` — этот path переиспользовать.

2. При потере агента в live run:
   - остановить telemetry loop для упавшего агента;
   - вызвать `reallocate_failed_agent` через runtime (уже существует);
   - определить reallocated tasks → проверить, возможен ли supplementary upload
     оставшимся агентам (`--reupload-on-failure` flag);
   - если `--reupload-on-failure`: загрузить дополнительные waypoints на каждый
     оставшийся агент через upload handshake;
   - если нет: финишировать с partial completion.

3. Обновить final report:
   - `lost_agents: Vec<AgentId>`;
   - `reassignment_count: u64`;
   - `tasks_recovered: Vec<TaskId>`;
   - `reallocation_latency_ticks: Option<u64>`.

4. Event log:
   - `SitlAgentLost { agent_id, tick }` (уже есть схема);
   - `SitlTaskReleased { task_id, from_agent_id, tick }` (уже есть схема);
   - `SitlTaskReassigned { task_id, from_agent_id, to_agent_id, tick }` (уже есть схема);
   - `SitlSupplementaryUploadStarted { agent_id, new_waypoint_count, tick }` — новое;
   - `SitlSupplementaryUploadCompleted { agent_id, accepted, tick }` — новое.

5. Зафиксировать captured live test:
   - запустить два PX4 SIH, убить один вручную во время execute;
   - сохранить event log и report в `results/`;
   - задокументировать процедуру в `docs/SITL_SETUP.md`.

### Done criteria

- Live test: один из двух PX4 инстансов убит во время execute → supervisor
  детектирует heartbeat timeout, логирует agent_lost, reallocates tasks,
  финишируют с partial/recovered статусом;
- с `--reupload-on-failure` оставшийся агент получает supplementary upload и
  продолжает;
- event log содержит agent_lost, task_released, task_reassigned события из live run;
- final report содержит `lost_agents`, `reassignment_count`, `tasks_recovered`;
- mock path не меняется и остаётся portable.

### Тесты

#### Без рефакторинга

- `SitlSupplementaryUploadStarted` / `SitlSupplementaryUploadCompleted` roundtrip.
- Final report с `lost_agents` field serialization test.
- Unit: supplementary waypoint list construction для reallocated tasks.
- Mock supervisor: `--reupload-on-failure` mode с fake connection.

#### Лёгкий рефакторинг

- Fake heartbeat timeout trigger в fake MAVLink connection.
- Supplementary upload fixture (partial waypoint list).
- Failure scenario config builder (какой агент убиваем, через сколько тиков).

#### Тяжёлый рефакторинг

- Real two-instance PX4 SIH failure integration test (ручной, с kill одного
  инстанса).
- Property test: при любом порядке agent failures все assignable задачи в конечном
  счёте получают агента или помечаются unrecoverable.

---

## M56 — Disaster Mapping v2

**Цель:** закрыть технический долг по Ветке 2 (BRANCHES.md): flood scope decision
и priority → allocation.

**Суть:** есть два самостоятельных пункта:

A. **Flood scope decision.** Название "wildfire / flood mapping" в коде, docs и
   README обещает flood, которого нет. Это нужно явно закрыть.

B. **Priority → allocation.** Сейчас wildfire priority updates обновляют поле
   `priority` в задаче и пишут replay events, но не влияют на allocation scoring
   в текущем тике. `GreedyAllocator` и другие стратегии не используют `priority`
   при выборе следующей задачи.

### Что сделать

#### A. Flood scope decision (выбрать один вариант)

**Вариант A — Cleanup (рекомендуется если flood не является приоритетом):**

- Убрать "flood" из названия билдера/функций/docs там, где нет реализации:
  `WildfireConfig` doc comment, `docs/SITL_SETUP.md`, README Quick Start,
  wildfire scenario file headers.
- Явно задокументировать flood как future work в BRANCHES.md или отдельном файле.
- Wildfire остаётся единственной disaster mapping миссией.

**Вариант B — Minimal Flood Implementation:**

- Новый `FloodConfig` (отдельно от `WildfireConfig`): `flooded_zones`,
  `water_spread_rate`, `critical_zones`, `rescue_priority_tasks`.
- Scenario files: `scenarios/flood.small-static.json`,
  `scenarios/flood.medium-dynamic.json`.
- Metrics: `flooded_zones_mapped`, `critical_zones_mapped`,
  `time_to_first_critical`, `final_risk_level`.
- `TaskKind::MappingZone` переиспользуется — новый adapter
  `FloodMappingAdapter`.
- Replay events: `FloodZoneUpdated { zone_id, water_level, tick }`,
  `CriticalZoneDetected { zone_id, tick }`.
- Regression smoke как experimental.

#### B. Priority → allocation scoring

1. Добавить `priority_weight: f64` в `WildfireScoringConfig` (или аналогичный
   механизм через `MissionAdapter::score`).
2. Передавать `task.priority` в scoring: `score = base_score + priority_weight *
   task.priority as f64`.
3. Адаптер `WildfireMappingAdapter::score` уже существует — расширить, не
   переписывать.
4. Тест: два zone tasks с одинаковым расстоянием, разным приоритетом → высокий
   приоритет получает более высокий score → назначается первым.
5. Benchmark: сравнить `time_to_map_first_high_risk` с и без priority_weight.

#### C. Success semantics hardening

- Зафиксировать документированный success rule для `small-static`:
  `task_completion_rate >= threshold` → `success = true`.
- Зафиксировать для `medium-dynamic`: то же самое, но threshold ниже из-за
  динамики.
- Тест: `success` и `task_completion_rate >= threshold` согласованы для
  каждого сценарного файла.

### Done criteria

- Flood вопрос закрыт: либо явно out-of-scope в docs, либо реализован.
- `priority_weight` влияет на assignment order: высокоприоритетный zone
  назначается раньше при равных расстояниях.
- Wildfire success semantics задокументированы и покрыты тестами.
- `time_to_map_first_high_risk` метрика меняется при включении priority_weight.

### Тесты

#### Без рефакторинга

- Priority scoring: high_priority zone vs low_priority zone с одинаковым
  расстоянием → high назначается первым.
- Success semantics consistency: `success == (task_completion_rate >= threshold)`
  для `small-static` и `medium-dynamic`.
- Wildfire scenario load tests (уже есть — проверить что не сломаны).
- Если Вариант B: `FloodConfig` parse test, `FloodMappingAdapter` completion test,
  replay event roundtrip для flood events.

#### Лёгкий рефакторинг

- Wildfire scenario fixture с контролируемыми приоритетами.
- Scoring comparison helper: top-scored task при разных приоритетах.
- `time_to_map_first_high_risk` assertion helper.

#### Тяжёлый рефакторинг

- Property tests для dynamic priority updates под случайными seed'ами.
- Multi-seed comparison benchmark: с/без priority_weight.
- Если Вариант B: flood disaster mapping abstraction, flood regression suite.

---

## M57 — Platform / API Stabilization

**Цель:** задокументировать extension points так, чтобы новая миссия или стратегия
добавлялись без изменений ядра.

**Суть:** сейчас `MissionAdapter`, `TaskKind`, `MissionAdapter::score/route_cost/
is_completed` уже существуют как trait, но нет документированного path для
внешнего разработчика. M58 (New Mission) требует чистого extension path — M57
создаёт и верифицирует его.

### Что сделать

1. Создать `docs/EXTENSION_GUIDE.md`:

   **Как добавить новую миссию:**
   - добавить `TaskKind` вариант;
   - реализовать `MissionAdapter` (score, route_cost, is_completed);
   - добавить scenario builder (Rust-сторона);
   - добавить scenario JSON файл (DSL-сторона);
   - добавить метрики в `RunMetrics` / `AggregateMetrics`;
   - добавить replay events;
   - добавить regression smoke suite.

   **Как добавить новую стратегию:**
   - реализовать `Allocator` trait;
   - зарегистрировать в `AdapterRegistry` или CLI parser;
   - добавить в benchmark matrix;
   - добавить в regression suite (или явно пометить unsupported).

   **Как добавить метрику:**
   - добавить поле в `RunMetrics`;
   - добавить в `AggregateMetrics` (если нужна агрегация);
   - обновить JSON/CSV/Markdown export;
   - добавить в manifest schema.

2. Stable crate boundaries:
   - задокументировать какие crates публичные (`swarm-types`, `swarm-sim`,
     `swarm-scenarios`, `swarm-comms`), какие internal (`swarm-runtime`,
     `swarm-alloc`);
   - проверить что internal crates не переиспользуются напрямую в extension
     path.

3. Schema version policy:
   - scenario files: `schema_version: "0.1"` уже есть — задокументировать
     что значит minor vs major bump;
   - replay log format: задокументировать текущий schema, backward compatibility
     policy.

4. Integration test — minimal new mission без изменений ядра:
   - создать `MinimalTestMission` в test-only модуле;
   - реализует `MissionAdapter`;
   - строится через DSL fixture;
   - проходит через `runner`, `AdapterRegistry`, replay.

### Done criteria

- `docs/EXTENSION_GUIDE.md` покрывает mission, strategy и metrics paths.
- Integration test добавляет `MinimalTestMission` без изменений в
  `swarm-types`, `swarm-runtime`, `swarm-alloc`, `swarm-sim`.
- Crate boundaries задокументированы.
- Schema version policy зафиксирована в docs.

### Тесты

#### Без рефакторинга

- `MinimalTestMission` DSL parse → runner → replay roundtrip test.
- Schema version field присутствует в scenario и replay fixtures.
- Replay JSON backward compatibility: старые event форматы читаются текущим
  parser'ом (если schema stable).

#### Лёгкий рефакторинг

- `MinimalTestMission` fixture builder.
- Shared extension guide compliance assertion (проверяет что mission реализует
  нужные точки).
- Schema version bump detection helper.

#### Тяжёлый рефакторинг

- External strategy harness (отдельный binary добавляет стратегию без patch
  к ядру).
- Schema compatibility tests across versions.

---

## M58 — New Mission

**Цель:** добавить принципиально новый класс миссий, который проверяет
coordination mechanics, недоступные в существующих миссиях.

**Суть:** существующие миссии (Coverage, SAR, Inspection, Wildfire) устроены по
одному паттерну: агент получает набор точек/зон/рёбер и посещает их. Ни одна из
них не требует задач с динамическими целями или precedence constraints. M58
добавляет один из двух кандидатов.

### Кандидаты

#### Кандидат 1 — Multi-target Pursuit

Движущиеся цели. Агенты перехватывают (capture) или сопровождают (escort).

**Механика:**
- цели движутся по заданным траекториям или по простой модели уклонения;
- задача агента: войти в proximity radius → цель считается captured / сопровождаемой;
- задачи динамически появляются (новая цель) и исчезают (цель захвачена);
- при потере агента → его цели возвращаются в unassigned без предсказуемой позиции
  (в отличие от waypoint).

**Domain model:**
- `TaskKind::Pursuit { target_id, mode: CaptureOrEscort, proximity_radius }`;
- `PursuitTarget { id, trajectory: Vec<Pose>, speed }`;
- `RunState::active_targets: HashMap<TargetId, Pose>`.

**Метрики:**
- `capture_rate`: доля целей captured за время миссии;
- `time_to_intercept`: среднее время от появления задачи до capture;
- `targets_lost`: цели, которые не были перехвачены за `max_ticks`;
- `total_pursuit_distance`: суммарное расстояние, пройденное агентами в pursuit.

**Почему интересно алгоритмически:**
- CBBA vs greedy под высокой динамикой пересмотра задач;
- time-dependent scoring: задача с быстро уходящей целью должна иметь higher
  urgency penalty;
- появление новых задач каждый тик тестирует allocation latency.

**Выбрать если:** цель — проверить алгоритмическую реактивность и динамику
пересмотра задач.

#### Кандидат 2 — Logistics / Delivery

Задачи с precedence constraints и capacity limits.

**Механика:**
- pickup и dropoff локации: нельзя доставить не забрав;
- depot как база с бесконечным запасом;
- capacity: агент несёт ограниченный груз (`max_load`);
- deadlines: некоторые задачи имеют time window (`latest_delivery_tick`).

**Domain model:**
- `TaskKind::Pickup { item_id, location }`;
- `TaskKind::Dropoff { item_id, location, requires_pickup: TaskId }`;
- `AgentState::cargo: Vec<ItemId>`;
- `RunState::delivered_items: HashSet<ItemId>`.

**Метрики:**
- `delivery_rate`: доля items delivered в срок;
- `late_deliveries`: items delivered после deadline;
- `capacity_violations`: попытки взять больше `max_load`;
- `total_route_cost`: суммарная длина маршрутов;
- `unserved_deliveries`: items не доставленные вообще.

**Почему интересно алгоритмически:**
- впервые задачи имеют precedence constraints → allocator должен учитывать
  dependency graph;
- capacity-aware CBBA vs centralized solver;
- проверяет обобщаемость DSL за пределы "набор точек".

**Выбрать если:** цель — проверить расширяемость DSL и task dependency handling.

### Что сделать (общее для обоих кандидатов)

1. Domain model: ключевые типы (`TaskKind` вариант, state fields, target/item
   structs).
2. DSL extension: новые поля в scenario JSON, параметры генерации, DSL validation.
3. `MissionAdapter` impl: `score`, `route_cost`, `is_completed`.
4. `RunState` extension: минимально необходимые поля.
5. Allocation compatibility: проверить какие стратегии поддерживаются, какие нет
   (support matrix).
6. Replay events: mission-specific события.
7. Метрики: добавить в `RunMetrics` / `AggregateMetrics` / export schema.
8. Scenarios: `scenarios/pursuit.small.json` + `scenarios/pursuit.medium.json`
   (или `delivery.*`).
9. Regression smoke: experimental threshold для new mission.
10. `docs/EXTENSION_GUIDE.md` пополнить примером.

### Done criteria

- Новая миссия описывается через DSL без изменений в `swarm-runtime`, `swarm-alloc`.
- Минимум два сценарных файла (small / medium).
- Benchmark запускается для stable стратегий.
- Replay содержит события, специфичные для новой миссии.
- Support matrix задокументирована: какие стратегии stable, unsupported, experimental.
- Regression smoke проходит.

### Тесты

#### Без рефакторинга

- DSL parse/validation tests для нового scenario type.
- Task generation unit test: генерируются корректные `TaskKind` варианты.
- Completion semantics test: задача completed при выполнении условия.
- Replay event serialization roundtrip.
- Benchmark smoke для small scenario.

#### Лёгкий рефакторинг

- Mission-specific scenario fixture builder.
- Reusable mission benchmark fixture.
- Outcome assertion helpers (capture_rate / delivery_rate).

#### Тяжёлый рефакторинг

- Property tests: динамическое появление/исчезновение задач (Pursuit) или
  precedence constraint satisfaction (Logistics).
- Multi-seed mission stability tests.
- Comparative strategy tests по всем stable стратегиям.

---

## Сводная таблица

| Milestone | Траектория | Результат | Зависит от |
|---|---|---|---|
| M54 | B | Live multi-agent PX4 execute: upload + arm + telemetry для N агентов | M53 |
| M55 | B | Live PX4 failure detection + реаллокация + supplementary upload | M54 |
| M56 | C | Flood scope closed; priority → allocation scoring; success semantics | M43–M53 |
| M57 | C | Extension guide, crate boundaries, schema policy, integration test | M56 (желательно) |
| M58 | C | Новый класс миссий (Pursuit или Logistics) через extension path | M57 |

**B и C независимы между собой.** M54 → M55 последовательно. M56 → M57 → M58
последовательно внутри C.

---

## Рекомендации по порядку

**Если ресурсы позволяют работать параллельно:**

- Запускать M54 и M56 одновременно — они не пересекаются по коду.
- M55 начинать после M54; M57 начинать после M56.
- M58 начинать после M57.

**Если работа последовательная:**

Рекомендуемый порядок: M56 → M57 → M54 → M55 → M58.

Почему:
- M56 и M57 — более изолированные изменения, меньше риск;
- M54 требует стенда с несколькими PX4 инстансами — лучше иметь стабильную
  платформу (M56, M57) перед live PX4 работой;
- M58 последним, потому что использует extension path из M57.

---

## Что не включаем сейчас

- **Algorithm Depth (Ветка 1)**: communication-aware scoring и hierarchical
  coordination. Требуют Research Benchmark (Ветка 3) для валидации — это отдельный
  и более длинный цикл.
- **Research Benchmark (Ветка 3)**: 1000-seed runs, confidence intervals.
  Зависит от алгоритмических улучшений из Ветки 1.
- **Replay / Visualization (Ветка 5)**: ASCII overlay, interactive UI. Ценно,
  но не разблокирует другие ветки.
- **HIL / Real hardware**: за пределами M55 boundary. Требует физического
  стенда и отдельного safety review.
