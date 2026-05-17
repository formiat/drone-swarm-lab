# PLAN: Milestone 9 — SAR v1 (Search and Rescue)

## Context

Milestones 1-8 построили полноценный coordination runtime с физической моделью:
- Milestones 1-4: membership, failure detection, task allocation, gossip convergence, partitions.
- Milestone 5: connectivity-aware allocation, relay placement, network availability.
- Milestone 6: strategy comparison platform (Greedy, Auction, ConnectivityAware, CentralizedPlanner).
- Milestone 7: experiment infrastructure (replay, JSON/CSV export, proptest).
- Milestone 8: kinematics + battery (movement, battery drain, capability gate, movement metrics).

**Milestone 9 (v0.9)** переводит проект от абстрактной coordination-проверки к первой настоящей reference mission — Search and Rescue. Агенты ищут скрытые цели на дискретизированной карте, используя ролевые сенсоры с вероятностью обнаружения. Это первый сценарий, где success зависит не только от allocation correctness, но и от физического покрытия области и sensor performance.

**Источники контекста:** `DRONE_A.3.md` (SAR: grid, hidden targets, roles, PoD, time_to_find), `DRONE_B.3.md` (SAR + kinematic model как первый содержательный benchmark). INVESTIGATION.md отсутствует.

**Текущее состояние (v0.8):**
- `Agent` имеет `speed`, `max_range`, `battery_drain_rate` — агенты двигаются к задачам.
- `Role` enum: `Scout`, `Relay`, `Mapper`, `Inspector`, `Carrier`.
- `Task` имеет `pose`, `required_role`, `preferred_role`.
- `ConnectivityModel` вычисляет связность по `comms_range` и динамическим `pose`.
- `ScenarioRunner` поддерживает `RunConfig` с `enable_movement`, `tick_duration_ms`.
- Метрики: `final_battery_min`, `avg_distance_travelled`, `agents_exhausted`, `mission_completion_ticks`, `time_to_first_exhaustion`, `network_availability`.

**Критерий готовности:**
1. Есть `SearchGrid` — дискретизированная область поиска (cells).
2. Есть `HiddenTarget` — цели размещены в ячейках, неизвестны агентам до сканирования.
3. Роли влияют на поиск: `Scout` (стандартный PoD), `Thermal` (повышенный PoD), `Relay` (не ищет, поддерживает связь).
4. `SensorModel` — вероятность обнаружения при сканировании ячейки (PoD зависит от роли).
5. Метрики: `time_to_find` (тик нахождения первой цели), `coverage_over_time` (доля просканированных ячеек по времени), `probability_of_detection` (фактическая доля найденных целей).
6. Сценарий SAR: N агентов, M целей на сетке, агенты назначаются на ячейки, двигаются, сканируют, расходуют батарею.
7. Все существующие тесты проходят (backward compat).

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из DRONE_A.3.md и DRONE_B.3.md:

- DRONE_A.3.md: SAR — это перевод от абстрактной проверки к reference mission. Состав: grid/area, hidden target, scout/thermal/relay roles, probability of detection, time_to_find, coverage over time, network availability.
- DRONE_B.3.md: SAR + kinematic model — первый настоящий benchmark, который нельзя "сломать" trivially. Закрывает gap в "критерии не-песочницы".
- Оба документа сходятся: после kinematics/battery (Milestone 8) SAR — логичный следующий шаг. CBBA (Milestone 10) строится поверх SAR как алгоритмический benchmark.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-types/src/grid.rs` | **NEW** — `SearchGrid`, `GridCell`, `CellState`, `HiddenTarget`, `SensorModel` |
| `crates/swarm-types/src/lib.rs` | Re-export новых типов |
| `crates/swarm-types/src/task.rs` | Добавить `TaskType` enum (`ScanCell`, `Relay`, `Transport`) или `scan_required: bool` |
| `crates/swarm-runtime/src/node.rs` | Интеграция scan action в tick loop: после `apply_movement` проверить достижение ячейки и вызвать `scan_cell()` |
| `crates/swarm-runtime/src/grid_state.rs` | **NEW** — `GridState`: mutable grid scan progress, target placement, scan results |
| `crates/swarm-sim/src/runner.rs` | `SarRunConfig` или расширение `RunConfig` grid-полями; SAR-специфичная логика в `run_with()` |
| `crates/swarm-sim/src/sar_scenario.rs` | **NEW** — `SarScenario`: builder для SAR миссии |
| `crates/swarm-metrics/src/metrics.rs` | Новые поля: `time_to_find`, `coverage_over_time`, `probability_of_detection`, `targets_found`, `targets_total`, `scan_count` |
| `crates/swarm-scenarios/src/lib.rs` | Re-export `sar_scenario` |
| `crates/swarm-examples/src/bin/sar_scenario.rs` | **NEW** — runnable SAR binary |
| `README.md` | Обновить статус до Milestone 9, описать SAR миссию |

---

## Implementation Steps

### Шаг 1 — Типы сетки и цели (`swarm-types`)

Файл: `crates/swarm-types/src/grid.rs` (новый)

```rust
/// Discrete search area divided into cells.
pub struct SearchGrid {
    pub width: u32,       // cells in x
    pub height: u32,      // cells in y
    pub cell_size: f64,   // meters per cell
}

impl SearchGrid {
    pub fn cell_center(&self, x: u32, y: u32) -> Pose { ... }
    pub fn cell_at_pose(&self, pose: &Pose) -> Option<(u32, u32)> { ... }
    pub fn total_cells(&self) -> u32 { self.width * self.height }
}

/// State of a single grid cell.
#[derive(Debug, Clone, PartialEq)]
pub enum CellState {
    Unvisited,
    Visited { scanned_by: Vec<AgentId>, scan_tick: u64 },
    TargetFound { target_id: String, found_by: AgentId, found_at_tick: u64 },
}

/// Hidden target placed on the grid.
pub struct HiddenTarget {
    pub id: String,
    pub cell_x: u32,
    pub cell_y: u32,
    pub pose: Pose, // center of cell
}

/// Probability-of-Detection model based on agent role.
pub struct SensorModel {
    pub scout_pod: f64,    // base PoD for Scout role
    pub thermal_pod: f64,  // elevated PoD for Thermal role
    pub relay_pod: f64,    // reduced PoD for Relay (if they scan at all)
}

impl SensorModel {
    pub fn probability(&self, role: Role) -> f64 {
        match role {
            Role::Scout => self.scout_pod,
            Role::Thermal => self.thermal_pod,
            _ => self.relay_pod,
        }
    }
}
```

**Тесты (категория 1):**
- `search_grid_cell_count` — total_cells correct
- `cell_center_roundtrip` — cell_center → cell_at_pose roundtrips
- `sensor_model_scout_vs_thermal` — thermal_pod > scout_pod

---

### Шаг 2 — GridState: mutable scan progress (`swarm-runtime`)

Файл: `crates/swarm-runtime/src/grid_state.rs` (новый)

```rust
pub struct GridState {
    pub grid: SearchGrid,
    pub cells: Vec<CellState>,
    pub targets: Vec<HiddenTarget>,
    pub sensor: SensorModel,
    pub targets_found: u32,
    pub first_find_tick: Option<u64>,
    pub scan_count: u32,
}

impl GridState {
    pub fn new(grid: SearchGrid, targets: Vec<HiddenTarget>, sensor: SensorModel) -> Self { ... }

    /// Scan a cell when an agent arrives at its center.
    /// Returns true if a target was found in this scan.
    pub fn scan_cell(&mut self, agent_id: AgentId, cell_idx: usize, role: Role, current_tick: u64) -> bool { ... }

    pub fn coverage_fraction(&self) -> f64 {
        let visited = self.cells.iter().filter(|c| !matches!(c, CellState::Unvisited)).count();
        visited as f64 / self.cells.len() as f64
    }

    pub fn all_targets_found(&self) -> bool {
        self.targets_found == self.targets.len() as u32
    }
}
```

Логика `scan_cell`:
1. Если cell уже `Visited` или `TargetFound` — idempotent (не сканируем повторно, или повторное сканирование не даёт новой информации).
2. Проверить, есть ли target в этой ячейке.
3. Если target есть: сгенерировать случайное число, сравнить с `sensor.probability(role)`. Если >= PoD — target found.
4. Обновить `CellState`, `targets_found`, `first_find_tick`.
5. Вернуть `true` если target найден в этом сканировании.

**Детерминизм:** `scan_cell` принимает `rng: &mut impl Rng` для воспроизводимости. В `ScenarioRunner` передаётся seeded RNG.

**Тесты (категория 1):**
- `scan_finds_target_when_pod_is_one` — PoD=1.0 → всегда находит
- `scan_misses_target_when_pod_is_zero` — PoD=0.0 → никогда не находит
- `scan_coverage_fraction` — 2 из 4 ячеек scanned → coverage=0.5
- `scan_idempotent` — повторное сканирование той же ячейки не меняет state

---

### Шаг 3 — Связать задачи с ячейками сетки

Файл: `crates/swarm-types/src/task.rs`

Добавить к `Task`:

```rust
pub struct Task {
    // ... existing fields ...
    /// If set, this task represents scanning a specific grid cell.
    pub grid_cell: Option<(u32, u32)>,
}
```

Или альтернатива (менее инвазивная): использовать `pose` задачи как center of cell, а в `GridState` мапить `pose → cell`. Предпочтительнее добавить `grid_cell` для явности.

**Backward compat:** `grid_cell: None` по умолчанию (`#[serde(default)]`).

---

### Шаг 4 — Интеграция scan в tick loop

Файл: `crates/swarm-runtime/src/node.rs`

В `process_inbox_and_allocate()` после `apply_movement` (если `enable_movement`):

```rust
// Step 4a: scan cells when agents arrive at task poses
if let Some(ref mut grid_state) = self.grid_state {
    for (agent_id, task_id) in &self.coordinator.registry.agent_assignments() {
        if let Some(task) = self.coordinator.registry.get_task(task_id) {
            if let Some((cell_x, cell_y)) = task.grid_cell {
                // Check if agent pose is within cell center threshold
                if let Some(entry) = self.coordinator.membership.get_agent(agent_id) {
                    let cell_pose = grid_state.grid.cell_center(cell_x, cell_y);
                    let dist = entry.pose.distance_to(&cell_pose);
                    if dist < ARRIVAL_THRESHOLD {
                        let cell_idx = (cell_y * grid_state.grid.width + cell_x) as usize;
                        let found = grid_state.scan_cell(
                            agent_id.clone(),
                            cell_idx,
                            entry.role,
                            self.tick_count,
                            &mut self.rng, // seeded RNG
                        );
                        if found {
                            // Optional: trigger event for replay/metrics
                        }
                    }
                }
            }
        }
    }
}
```

`ARRIVAL_THRESHOLD` — допустимое отклонение от центра ячейки (например, 0.1 м).

**Тесты (категория 2):**
- `agent_scans_cell_when_arrives` — интеграционный: агент назначен на задачу с grid_cell, двигается к центру, scan_cell вызывается.

---

### Шаг 5 — SAR Scenario Builder

Файл: `crates/swarm-scenarios/src/sar_scenario.rs` (новый)

```rust
pub struct SarScenarioConfig {
    pub grid: SearchGrid,
    pub target_count: u32,
    pub scout_count: u32,
    pub thermal_count: u32,
    pub relay_count: u32,
    pub sensor: SensorModel,
    pub enable_movement: bool,
    pub tick_duration_ms: u64,
    pub max_ticks: u64,
    pub seed: u64,
}

pub fn build_sar_scenario(config: &SarScenarioConfig) -> (Vec<Agent>, Vec<Task>, Vec<HiddenTarget>) { ... }
```

Логика:
1. Создать `SearchGrid`.
2. Случайно разместить `target_count` целей в ячейках (seeded RNG для детерминизма).
3. Создать задачи: одна задача на каждую ячейку (`Task { grid_cell: Some((x,y)), pose: grid.cell_center(x,y), required_role: None, ... }`).
4. Создать агентов: scouts, thermals, relays с разными ролями и capabilities.
   - Scouts: `role: Scout`, `speed: 5.0`, `comms_range: 15.0`.
   - Thermals: `role: Thermal` (нужно добавить в `Role` enum?), `speed: 4.0`, `comms_range: 12.0`, capability `thermal`.
   - Relays: `role: Relay`, `speed: 3.0`, `comms_range: 20.0`.

**Примечание:** `Role::Thermal` — новый вариант в `Role` enum. Если не добавлять, можно использовать `Role::Scout` + `Capability::Thermal`. Предпочтительнее добавить `Role::Thermal` в enum для явности.

Файл: `crates/swarm-types/src/agent.rs`

```rust
pub enum Role {
    Scout,
    Relay,
    Mapper,
    Inspector,
    Carrier,
    Thermal, // NEW
}
```

**Тесты (категория 1):**
- `sar_scenario_creates_correct_agent_count` — scout_count + thermal_count + relay_count агентов
- `sar_scenario_targets_within_grid` — все цели внутри сетки
- `sar_scenario_one_task_per_cell` — задач = width * height

---

### Шаг 6 — SAR-специфичные метрики

Файл: `crates/swarm-metrics/src/metrics.rs`

Новые поля в `RunMetrics`:
```rust
#[serde(default)]
pub time_to_find: Option<u64>,          // tick when first target found
#[serde(default)]
pub coverage_over_time: Vec<f64>,       // coverage fraction per tick (time series)
#[serde(default)]
pub probability_of_detection: f64,      // targets_found / targets_total
#[serde(default)]
pub targets_found: u32,
#[serde(default)]
pub targets_total: u32,
#[serde(default)]
pub scan_count: u32,
```

Агрегация в `ScenarioRunner::run_with()`:
- `time_to_find` = `grid_state.first_find_tick`
- `targets_found` = `grid_state.targets_found`
- `targets_total` = `grid_state.targets.len()`
- `probability_of_detection` = `targets_found / targets_total`
- `scan_count` = `grid_state.scan_count`
- `coverage_over_time` — записывать `coverage_fraction()` каждый тик

**Тесты (категория 1):**
- `sar_metrics_populated` — после SAR-симуляции метрики содержат ненулевые значения
- `coverage_over_time_monotonic` — coverage не уменьшается (или non-decreasing)

---

### Шаг 7 — ScenarioRunner поддержка SAR

Файл: `crates/swarm-sim/src/runner.rs`

Добавить в `RunConfig` (или создать `SarRunConfig`):
```rust
pub struct RunConfig {
    // ... existing fields ...
    // SAR-specific (optional)
    pub grid_state: Option<GridState>,
}
```

Или лучше: `ScenarioRunner` принимает `Option<GridState>` отдельно от `RunConfig`.

В `run_with()`:
1. Если `grid_state` передан — инициализировать `grid_state` на каждом `AgentNode`.
2. В конце симуляции — собрать SAR-метрики из `grid_state`.
3. Success criteria для SAR: `targets_found == targets_total` (или `targets_found >= min_required`).

**Тесты (категория 2):**
- `sar_scenario_finds_all_targets_with_pod_one` — PoD=1.0, все цели найдены за finite ticks
- `sar_scenario_fails_with_pod_zero` — PoD=0.0, success=false (цели не найдены)
- `sar_thermal_faster_than_scout` — thermal (PoD=0.8) находит быстрее, чем scout (PoD=0.3) при тех же условиях

---

### Шаг 8 — Runnable SAR binary

Файл: `crates/swarm-examples/src/bin/sar_scenario.rs` (новый)

```rust
fn main() {
    let config = SarScenarioConfig {
        grid: SearchGrid { width: 10, height: 10, cell_size: 10.0 },
        target_count: 3,
        scout_count: 3,
        thermal_count: 1,
        relay_count: 1,
        sensor: SensorModel { scout_pod: 0.3, thermal_pod: 0.8, relay_pod: 0.1 },
        enable_movement: true,
        tick_duration_ms: 1000,
        max_ticks: 500,
        seed: 42,
    };

    let (agents, tasks, targets) = build_sar_scenario(&config);
    let grid_state = GridState::new(config.grid.clone(), targets, config.sensor.clone());

    let runner = ScenarioRunner::new(agents, tasks, RunConfig {
        // ... standard config ...
        enable_movement: config.enable_movement,
        tick_duration_ms: config.tick_duration_ms,
    });

    let result = runner.run_with(|nodes, tick| {
        // Optional: print coverage every 50 ticks
    });

    println!("Targets found: {}/{}\n", result.metrics.targets_found, result.metrics.targets_total);
    println!("Time to first find: {:?}\n", result.metrics.time_to_find);
    println!("Final coverage: {:.2}\n", result.metrics.coverage_over_time.last().unwrap_or(&0.0));
    println!("PoD: {:.2}\n", result.metrics.probability_of_detection);

    assert!(result.metrics.targets_found > 0, "At least one target should be found");
}
```

---

### Шаг 9 — Обновить README.md

Добавить раздел **Milestone 9**:
- SAR v1: grid search, hidden targets, role-based sensors (Scout/Thermal/Relay).
- Probability of Detection model: thermal has higher PoD than scout.
- Metrics: `time_to_find`, `coverage_over_time`, `probability_of_detection`.
- Команда запуска: `cargo run -p swarm-examples --bin sar_scenario`.

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run -p swarm-examples --bin sar_scenario
cargo run -p swarm-examples --bin coverage_with_failure
cargo run -p swarm-examples --bin emergency_mesh_scenario
cargo run -p swarm-examples --bin strategy_comparison
```

---

## Testing Strategy

### Категория 1 — Без рефакторинга (unit тесты)

**`swarm-types` — grid и sensor (3 теста):**
- `search_grid_cell_count`
- `cell_center_roundtrip`
- `sensor_model_scout_vs_thermal`

**`swarm-runtime` — GridState scan (4 теста):**
- `scan_finds_target_when_pod_is_one`
- `scan_misses_target_when_pod_is_zero`
- `scan_coverage_fraction`
- `scan_idempotent`

**`swarm-scenarios` — SAR builder (3 теста):**
- `sar_scenario_creates_correct_agent_count`
- `sar_scenario_targets_within_grid`
- `sar_scenario_one_task_per_cell`

**`swarm-metrics` — SAR metrics (2 теста):**
- `sar_metrics_populated`
- `coverage_over_time_monotonic`

**`swarm-types` — Role enum (1 тест):**
- `role_thermal_serde_roundtrip`

**Регрессия:** все существующие ~147 тестов должны пройти (backward compat через `grid_cell: None`, `Role::Thermal` как новый enum variant).

### Категория 2 — Лёгкий рефакторинг (интеграционные)

- **`agent_scans_cell_when_arrives`** — интеграция: AgentNode с enable_movement, grid_state, агент двигается к cell center, scan срабатывает.
- **`sar_scenario_finds_all_targets_with_pod_one`** — end-to-end: PoD=1.0 → все цели найдены.
- **`sar_scenario_fails_with_pod_zero`** — end-to-end: PoD=0.0 → success=false.
- **`sar_thermal_faster_than_scout`** — сравнение: thermal (высокий PoD) vs scout (низкий PoD) на одной seed.

### Категория 3 — Тяжёлый (не для v0.9)

- **Multi-target SAR с battery exhaustion** — агенты исчерпывают батарею до покрытия всей сетки. Требует tuning max_range и grid size.
- **CBBA on SAR** — отложен до Milestone 10.
- **Dynamic target injection** — цели появляются во время миссии. Требует изменения task lifecycle.

### Покрытие gap

- **Gap**: нет модели помех (false positives). Scan может "найти" target там, где его нет. Для v0.9 — не критично.
- **Gap**: нет prioritization ячеек (все ячейки равнозначны). Для v0.9 — приемлемо; можно добавить heat map в v0.10.
- **Gap**: `coverage_over_time` — Vec<f64> на каждый тик может быть большим для long runs. Можно сэмплировать каждые N тиков. Для v0.9 — записывать каждый тик, оптимизировать позже.

---

## Risks and Tradeoffs

**1. Новый enum variant `Role::Thermal`**

Breaking change для deserialизации старых JSON с ролью "thermal" (если такие были). Но "thermal" раньше был только `Capability`, не `Role`. `#[serde(default)]` на Role не применим (enum). Риск: старые JSON с `role: "thermal"` (если создавались вручную) не десериализуются. Вероятность низкая — thermal не использовался как Role.

**2. `grid_cell` в `Task`**

Новое поле `grid_cell: Option<(u32, u32)>` добавляется в `Task`. Все конструкторы `Task` обновляются. `#[serde(default)]` обеспечивает backward compat для старых JSON-конфигов.

**3. GridState в tick loop**

`GridState::scan_cell` вызывается каждый тик для каждого агента с назначенной задачей. Если агент стоит на месте (доехал до цели), scan будет idempotent (ячейка уже Visited). Производительность: O(agents) на тик, приемлемо.

**4. Детерминизм PoD**

`scan_cell` использует seeded RNG. Важно: один и тот же seed должен давать один и тот же результат. Проверить через `sar_scenario_finds_all_targets_with_pod_one` на фиксированном seed.

**5. ARRIVAL_THRESHOLD**

Порог прибытия к центру ячейки (0.1 м) — магическая константа. Если `cell_size` маленький (1.0 м), threshold может быть слишком большим. Для v0.9: `ARRIVAL_THRESHOLD = cell_size * 0.1` (10% от размера ячейки).

**6. Task = one per cell**

В SAR-сценарии количество задач = width * height. Для сетки 10×10 это 100 задач. При 5 агентах coverage займёт ~20 тиков (последовательно) или меньше (параллельно). Для battery model: каждая ячейка 10×10 м, расстояние между соседними ячейками ~10 м. При speed=5 м/с и tick_duration=1 с: агент проходит 5 м/тик. До соседней ячейки — 2 тика. Battery drain: 2 тика * 5 м * drain_rate. При max_range=500 м — ~100 ячеек на полной батарее. Для 10×10 сетки хватает.

**7. Coverage_over_time Vec<f64>**

Для `max_ticks=500` — Vec из 500 элементов (~4KB). Для `max_ticks=10000` — ~80KB. Приемлемо для in-process, но для JSON export может раздуть отчёт. Можно сэмплировать каждые 10 тиков. Для v0.9 — записывать каждый тик, оптимизировать в v0.10 если нужно.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| Новый enum variant `Role::Thermal` | Старые JSON с `role: "thermal"` (если были) не десериализуются | `cargo test --workspace` (serde тесты) |
| `grid_cell` в `Task` | Все конструкторы Task (в тестах, сценариях) не компилируются | `cargo check --workspace` |
| `#[serde(default)]` для `grid_cell` | Старые JSON-конфиги для agent_process не десериализуются | Тест `task_serde_default` |
| `GridState` в tick loop | Performance regression: O(agents) scan per tick | Benchmark: `cargo test` время выполнения не должно вырасти >20% |
| `Role::Thermal` в allocator | Агенты с Thermal не назначаются на задачи без required_role | Тест `thermal_agent_allocatable_on_generic_task` |
| `coverage_over_time` Vec | Memory usage growth для long runs | `sar_scenario` с max_ticks=1000 не паникует по памяти |
| `Cargo.lock` изменился | Должен быть включён в commit | `git diff --stat` |

---

## Open Questions

1. **Нужен ли `Role::Thermal` или достаточно `Capability::Thermal`?**
   - `Role::Thermal` — явный, интуитивный для SAR.
   - `Capability::Thermal` — не требует изменения enum, но менее явный.
   - Рекомендация: добавить `Role::Thermal` в enum (breaking change минимальный, т.к. thermal не использовался как Role ранее).

2. **`coverage_over_time` — Vec или HashMap<tick, fraction>?**
   - Vec: простой, плотный, но большой для long runs.
   - HashMap: разреженный, но теряет ordering guarantee.
   - Рекомендация: Vec для v0.9; оптимизировать сэмплированием в v0.10.

3. **Нужен ли `min_targets_required` вместо `all_targets_found`?**
   - Для SAR может быть достаточно найти 1 из 3 целей (partial success).
   - Для v0.9: success = all_targets_found. Добавить `min_targets_required` в v0.10.

4. **Как обрабатывать scan агентом без назначенной задачи?**
   - В v0.9: scan только при назначении на задачу с `grid_cell`. Агент без задачи не сканирует.
   - Альтернатива: "area search" — агент сканирует ячейку под собой каждый тик. Более реалистично, но сложнее (нужна autonomy без allocation). Для v0.10.

5. **Нужна ли GridState сериализация для replay?**
   - Да: `GridState` должен сериализоваться в `EventLog` для deterministic replay. Добавить `GridSnapshot` event.
   - Для v0.9: достаточно сериализовать `SearchGrid`, `HiddenTarget` placement, и `seed`. Replay пересоздаёт `GridState` и воспроизводит scan логику.
