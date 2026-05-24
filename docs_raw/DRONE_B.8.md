# DRONE_B.8 — Детализированный гибридный roadmap

Дата фиксации: 2026-05-20

## Исходная точка

**Что завершено:**

- M11 Hardening: `mission`/`scenario` в JSON/CSV, mission-aware `benchmark_run_id`,
  proptest distributed CBBA, README с реальными числами.
- Mission DSL (v0.12): `ScenarioSuite`, `load_scenario_suite`, флаг `--scenario-suite`,
  три примера в `scenarios/`, 10 unit-тестов.

**Что есть в коде:**

- `AgentNode<T: Transport>` — транспорт подменяем.
- `Transport` trait (`swarm-comms/src/transport.rs`) — in-memory и UDP реализации.
- `Agent`: `battery`, `speed`, `max_range`, `battery_drain_rate`, `comms_range`.
- `Task`: `pose`, `grid_cell`, `required_capabilities`, `required_role`.
- SAR: `SearchGrid`, `SensorModel`, `HiddenTarget`, `BeliefMap` в `swarm-types`.
- Benchmark matrix: 5 стратегий × профили × миссии, JSON/CSV/Markdown export.
- `GridState` в runtime.

---

## Итоговый план

```
Stage 1: Safety Layer
Stage 2: SAR v2 / Uncertainty Map
Stage 3: CBBA Robustness
Stage 4: Infrastructure Inspection
Stage 5: SITL (PX4 / MAVLink)
```

Каждый stage завершается milestone-коммитом с номером версии.

---

## Stage 1 — Safety Layer (M13)

### Цель

Добавить слой физических и операционных ограничений поверх аллокатора и runtime.
Стратегии начнут учитывать, что не все задачи достижимы и не все позиции допустимы.

### Что строить

**1.1 Новый крейт `swarm-safety`**

Типы:

```rust
pub struct Geofence { ... }        // допустимая область (AABB или выпуклый полигон)
pub struct NoFlyZone { ... }       // список запрещённых ячеек или полигонов
pub struct SeparationConstraint {  // минимальное расстояние между агентами
    pub min_distance_m: f64,
}
pub struct SafetyConfig {
    pub geofence: Option<Geofence>,
    pub no_fly_zones: Vec<NoFlyZone>,
    pub separation: Option<SeparationConstraint>,
}
pub struct SafetyViolation { ... } // диагностика нарушений
```

Публичный API:

```rust
pub fn check_agent(config: &SafetyConfig, agent: &Agent, others: &[Agent])
    -> Vec<SafetyViolation>;
pub fn is_task_reachable(config: &SafetyConfig, agent: &Agent, task: &Task) -> bool;
pub fn filter_safe_tasks<'a>(
    config: &SafetyConfig,
    agent: &Agent,
    tasks: &'a [Task],
) -> Vec<&'a Task>;
```

**1.2 Safety-aware аллокация**

Добавить `SafetyConfig` в `AllocationContext` (или отдельный параметр аллокаторов).
Все аллокаторы (Greedy, CBBA, Auction, Centralized, ConnectivityAware) перед назначением
фильтруют задачи через `filter_safe_tasks`.

**1.3 Runtime enforcement**

В `AgentNode::tick`: перед каждым движением проверять `check_agent`.
При нарушении агент останавливается и логирует `SafetyViolation` в `EventLog`.

**1.4 Benchmark профили с safety**

Добавить `safety_config: Option<SafetyConfig>` в `RunConfig` (с serde).
Добавить scenario JSON-файлы с geofence/no-fly zones в `scenarios/`.

### Тесты

- `swarm-safety`: unit-тесты check_agent, is_task_reachable, filter_safe_tasks.
- Proptest: случайные позиции агентов + случайные geofence — violation rate не должен
  давать NaN/panic.
- Integration: coverage scenario с no-fly зоной в центре сетки — агенты не посещают
  запрещённые ячейки.

### Done criteria

- `cargo test -p swarm-safety` — проходит.
- Coverage scenario с no-fly зоной: 0 нарушений в event log.
- `--scenario-suite scenarios/coverage.safety.json` работает.
- Benchmark export содержит `safety_violations` метрику.

---

## Stage 2 — SAR v2 / Uncertainty Map (M14)

### Цель

Сделать SAR-миссию исследовательски содержательной: агенты работают с вероятностной
картой уверенности, повторно сканируют ячейки с высокой неопределённостью, учитывают
ложные срабатывания.

### Что строить

**2.1 `BeliefMap` — полноценная реализация**

Уже есть в `swarm-types`. Расширить:

```rust
pub struct BeliefCell {
    pub prior: f64,            // начальная вероятность наличия цели
    pub posterior: f64,        // текущий belief после наблюдений
    pub scan_count: u32,       // сколько раз ячейка сканировалась
    pub last_scan_tick: u64,
}

pub struct BeliefMap {
    cells: Vec<Vec<BeliefCell>>,
}

impl BeliefMap {
    pub fn update(&mut self, cell: (u32,u32), detection: bool, sensor: &SensorModel);
    pub fn entropy(&self, cell: (u32,u32)) -> f64;
    pub fn highest_uncertainty_cells(&self, n: usize) -> Vec<(u32,u32)>;
    pub fn probability_of_detection(&self) -> f64;
}
```

Байесовское обновление при каждом скане:

```
P(target | detection) = P(detection | target) * P(target) / P(detection)
```

**2.2 `SensorModel` v2**

Расширить существующий `SensorModel` (`swarm-types`):

```rust
pub struct SensorModel {
    pub detection_probability: f64,   // P(detect | target present)
    pub false_positive_rate: f64,     // P(detect | no target)
    pub range_m: f64,                 // радиус сканирования
}
```

`false_positive_rate` уже есть структурно, убедиться что используется в Байесе.

**2.3 Uncertainty-driven task prioritization**

Новый аллокатор или расширение существующих:
задачи-сканы получают динамический приоритет на основе entropy ячейки.

```rust
pub fn sar_task_priority(belief: &BeliefMap, cell: (u32,u32)) -> f64 {
    belief.entropy(cell) * belief.cells[cell].posterior
}
```

Агент предпочитает высокоэнтропийные ячейки.

**2.4 Повторные сканирования**

Если `posterior > threshold` после первого скана — ячейка остаётся в пуле задач для
повторного сканирования (confirmation scan). Добавить `TaskKind::ConfirmationScan` или
флаг `rescan: bool` в `Task`.

**2.5 Новые метрики**

Добавить в `AggregateMetrics`:

- `avg_belief_entropy_final` — среднее entropy карты в конце прогона;
- `avg_false_positive_rate` — доля ложных срабатываний;
- `avg_confirmation_scans` — среднее число повторных сканов на цель;
- `avg_pod` — probability of detection (уже есть, уточнить расчёт с учётом belief).

**2.6 SAR v2 сценарии**

Обновить `scenarios/sar.ideal.json` и добавить `sar.uncertain.json`,
`sar.noisy.json` (высокий false positive rate).

### Тесты

- Unit: `BeliefMap::update` — корректный Байес для одного скана.
- Unit: `BeliefMap::entropy` — H=0 при posterior=0/1, максимум при posterior=0.5.
- Unit: `sar_task_priority` — высокоэнтропийные ячейки получают больший приоритет.
- Integration: после 200 тиков в `sar.ideal` — `avg_pod > 0.8`.
- Integration: `sar.noisy` — `avg_false_positive_rate` соответствует заданному `SensorModel`.
- Proptest: случайные `detection_probability` / `false_positive_rate` — Байес не даёт
  posterior за пределами [0,1].

### Done criteria

- Benchmark по SAR v2 отличается от SAR v1 в метриках (belief entropy, PoD).
- `--scenario-suite scenarios/sar.uncertain.json` работает и экспортирует CSV.
- Стратегии начинают отличаться не только allocation quality, но и поведением под
  неопределённостью.

---

## Stage 3 — CBBA Robustness (M15)

### Цель

Понять пределы CBBA: где алгоритм сходится быстро, где деградирует, как ведёт себя
при потере пакетов и сетевых партициях. Получить 1000-seed analysis с выводами.

### Что строить

**3.1 Расширенный proptest suite**

В `crates/swarm-sim/tests/proptest_cbba.rs`:

- Случайные топологии (erdos-renyi граф агентов, случайный `comms_range`).
- Случайный `packet_loss_rate` в [0.0, 0.5].
- Измерение тиков до сходимости CBBA.
- Проверка инвариантов: нет конфликтующих назначений, нет незавершённых bids после
  сходимости.

**3.2 Convergence time distribution**

Новая метрика в `AggregateMetrics`:

```rust
pub convergence_ticks_p50: f64,
pub convergence_ticks_p95: f64,
pub convergence_ticks_max: f64,
```

Измерять: с какого тика все агенты согласованы (нет conflicting assignments).

**3.3 TSP-ordering в task bundles**

В `CbbaAllocator`: если агент получает список задач (bundle), их порядок посещения
оптимизировать жадным nearest-neighbour TSP.

```rust
fn order_bundle_tsp(agent_pose: Pose, tasks: &[Task]) -> Vec<TaskId>;
```

Метрика: `avg_bundle_travel_distance` — суммарный путь агента по bundle.

**3.4 Retransmission policy**

При `packet_loss_rate > threshold` агент повторяет bid с экспоненциальным backoff.
Конфигурируется в `CbbaConfig`:

```rust
pub retransmit_max_attempts: u32,   // default: 3
pub retransmit_backoff_ticks: u64,  // default: 2
```

**3.5 Partition healing**

Расширить `PartitionEvent` — добавить `heal_at_tick: Option<u64>`.
После healing CBBA должен повторно сойтись. Тест: partition на тиках 50-100,
heal на 100 — к тику 150 все агенты согласованы.

**3.6 1000-seed publishable benchmark**

Отдельный `--scenario-suite scenarios/cbba_stress.json` со 1000 seeds.
В README: таблица с p50/p95/max convergence time по packet loss rate.

### Тесты

- Proptest (уже есть) — расширить параметрический диапазон.
- Integration: `cbba_stress` с packet_loss=0.3 — `convergence_ticks_p95 < 100`.
- Integration: partition + heal — после heal все задачи назначены без конфликтов.
- Unit: `order_bundle_tsp` — возвращает ближайшего соседа как первый шаг.

### Done criteria

- `--scenario-suite scenarios/cbba_stress.json` завершается за разумное время.
- README содержит convergence distribution таблицу из 1000 seeds.
- TSP-ordering даёт измеримое снижение `avg_bundle_travel_distance`.

---

## Stage 4 — Infrastructure Inspection (M16)

### Цель

Добавить reference mission для обследования линейной инфраструктуры (ЛЭП, трубопроводы,
периметр). Агенты покрывают граф рёбер, а не ячейки сетки.

### Что строить

**4.1 Новый тип задачи: `EdgeTask`**

Расширить `Task` или добавить в `swarm-types`:

```rust
pub struct InspectionEdge {
    pub id: EdgeId,
    pub from: Pose,
    pub to: Pose,
    pub length_m: f64,
    pub priority: u8,
}
```

Аллокация: агент получает набор рёбер, TSP-порядок по ним.

**4.2 `InspectionGraph`**

```rust
pub struct InspectionGraph {
    pub edges: Vec<InspectionEdge>,
    pub depot: Pose,   // базовая точка старта/финиша
}
```

Генераторы:

- `linear_route(n_segments, segment_length_m)` — прямая линия (ЛЭП);
- `grid_perimeter(width, height, cell_size_m)` — периметр сетки;
- `random_graph(n_nodes, seed)` — случайный граф для stress-тестов.

**4.3 `InspectionMission` в `swarm-scenarios`**

Новый модуль `crates/swarm-scenarios/src/inspection.rs`:

```rust
pub struct InspectionConfig {
    pub graph: InspectionGraph,
    pub agent_count: u32,
    pub battery_constraint: f64,   // 0.0 = без ограничений
    pub require_role: Option<Role>,
    pub seed: u64,
    pub max_ticks: u64,
}

pub fn build_inspection_scenario(cfg: &InspectionConfig) -> (Scenario, RunConfig);
```

**4.4 Новые метрики**

```rust
pub avg_edge_coverage_rate: f64,    // доля покрытых рёбер
pub avg_missed_edges: f64,          // среднее число пропущенных рёбер
pub avg_revisit_count: f64,         // среднее число повторных визитов одного ребра
pub avg_route_efficiency: f64,      // покрытое расстояние / общий путь агента
```

**4.5 Benchmark и сценарии**

- `scenarios/inspection.linear.json` — прямая ЛЭП, 3 агента.
- `scenarios/inspection.perimeter.json` — периметр 10×10, 4 агента, battery constraint.
- `scenarios/inspection.random.json` — случайный граф, 5 агентов.

### Тесты

- Unit: `linear_route` — n рёбер, суммарная длина = n * segment_length_m.
- Integration: `inspection.linear`, 3 агента — `avg_edge_coverage_rate > 0.9`.
- Integration: `inspection.perimeter` с battery=30% — агенты не исчерпывают батарею.
- Proptest: случайный `InspectionGraph`, случайный `agent_count` — нет паники,
  `avg_edge_coverage_rate` в [0,1].

### Done criteria

- `--scenario-suite scenarios/inspection.linear.json` работает, экспортирует CSV.
- Стратегии отличаются по `avg_edge_coverage_rate` и `avg_missed_edges`.
- README содержит inspection benchmark таблицу.

---

## Stage 5 — SITL / MAVLink (M17)

### Цель

Подключить один агент к PX4 SITL через MAVLink. Валидировать, что координационные
алгоритмы, разработанные в симуляторе, работают поверх реального autopilot стека.

### Предусловия

- Safety Layer завершён (Stage 1).
- `Transport` trait стабилен.
- Хотя бы один inspection или coverage сценарий воспроизводимо работает через DSL.

### Что строить

**5.1 `MavlinkTransport`**

Новый крейт `swarm-mavlink` (или модуль в `swarm-comms`):

```rust
pub struct MavlinkTransport {
    conn: MavlinkConnection,
    agent_id: AgentId,
}

impl Transport for MavlinkTransport {
    type Error = MavlinkError;
    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error>;
    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error>;
}
```

Зависимость: `mavlink` crate (rust-mavlink).

Сначала реализовать `MockMavlinkTransport` (записывает команды в Vec)
для unit-тестов без PX4.

**5.2 Mapping: миссионные задачи → MAVLink команды**

```rust
pub fn task_to_mavlink_waypoint(task: &Task) -> mavlink::MavMessage;
pub fn mavlink_status_to_task_status(msg: &mavlink::MavMessage) -> Option<TaskStatus>;
```

`Task` с `pose` → `MAV_CMD_NAV_WAYPOINT`.
Статус полёта → `TaskStatus::InProgress` / `TaskStatus::Completed`.

**5.3 Single-agent SITL runner**

Новый binary в `swarm-examples`:

```bash
cargo run --bin sitl_agent -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/coverage.ideal.json \
  --agent-id agent-0
```

Один агент получает задачи из `Scenario`, конвертирует их в waypoints и отправляет
через `MavlinkTransport`.

**5.4 PX4 SITL integration тест (manual)**

Документация в `docs/SITL_SETUP.md`:

- Установка PX4 и Gazebo.
- Команды для запуска SITL.
- Ожидаемый вывод при успешном полёте по waypoints.

Автоматизировать нельзя (требует PX4 SITL окружения), но описать воспроизводимо.

**5.5 Multi-agent SITL**

После успешного single-agent:
- Запустить N агентов, каждый с отдельным `MavlinkTransport` (разные UDP порты).
- Coordinator работает через in-memory transport.
- Каждый `AgentNode` отправляет своё движение в SITL через `MavlinkTransport`.

### Тесты

- Unit: `MockMavlinkTransport` — send + poll roundtrip.
- Unit: `task_to_mavlink_waypoint` — корректный MAV_CMD для Task с Pose.
- Unit: `mavlink_status_to_task_status` — корректный маппинг статусов.
- Manual: single-agent SITL по coverage сценарию.

### Done criteria

- `sitl_agent` компилируется и запускается без PX4 (с MockMavlink).
- С запущенным PX4 SITL: агент пролетает по waypoints из coverage.ideal.json.
- Документация воспроизводима: другой человек может повторить SITL запуск.

---

## Зависимости между stages

```
Stage 1 (Safety Layer)
    ↓ (SafetyConfig в RunConfig — нужен для всех)
Stage 2 (SAR v2)    ←→    Stage 3 (CBBA Robustness)
    ↓                              ↓
Stage 4 (Infrastructure Inspection)
    ↓
Stage 5 (SITL)
```

Stage 2 и Stage 3 независимы — можно делать параллельно или в любом порядке.
Stage 4 после Stage 1 (использует safety-aware аллокацию).
Stage 5 после всех предыдущих (опирается на стабильность Transport и DSL).

---

## Версионирование

| Stage | Версия | Содержание |
|-------|--------|------------|
| 1 | v0.13 | Safety Layer |
| 2 | v0.14 | SAR v2 / Uncertainty Map |
| 3 | v0.15 | CBBA Robustness |
| 4 | v0.16 | Infrastructure Inspection |
| 5 | v0.17 | SITL / MAVLink |

---

## Cross-cutting: Visualization / Replay UI

Не выделен в отдельный stage, но пригодится после Stage 2 или Stage 4.

Минимальный вариант: CLI-утилита `cargo run --bin replay -- --log run.jsonl`
которая рисует ASCII grid с позициями агентов по тикам.

Полноценный вариант (Bevy или egui): после Stage 4, когда есть
InspectionGraph и BeliefMap для визуализации.
