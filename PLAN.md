# PLAN: M30 — New Mission Prototype (Wildfire / Flood Mapping)

## Контекст

M29 завершён: введён `RegressionSuite`, `ThresholdChecker`, `Baseline`, `RegressionRunner`,
`default_suites()` и `regression_runner` binary. Система умеет запускать regression
для coverage, SAR, inspection, emergency-mesh и safety.

Цель M30 — проверить, что система реально расширяется на новую миссию, а не только
поддерживает уже реализованные сценарии. Выбрана миссия **Wildfire / Flood Mapping**
потому что:

- ближе к текущим SAR/coverage primitives;
- можно переиспользовать BeliefMap-like model для hazard map;
- естественно появляются risk zones и changing priorities;
- хорошо проверяются DSL, semantics, allocation, safety и replay;
- не нужно сразу вводить сложную динамику moving targets или pickup/dropoff dependencies.

## Investigation Context

`INVESTIGATION.md` отсутствует. Ниже — ключевые наблюдения из инспекции кода.

**Текущая архитектура миссий** (`swarm-types/src/task.rs`, `swarm-types/src/mission.rs`):

- `TaskKind` перечисляет: `CoverageCell`, `SarScan`, `SarConfirmationScan`, `InspectionEdge`, `RelayPlacement`, `Waypoint`.
- `MissionAdapter` trait предоставляет `task_kind`, `route_cost`, `is_completed`, `score`.
- `RunState` содержит `scanned_cells`, `covered_edges`, `completed_tasks`.
- Новый `TaskKind` добавляется без изменений ядра, если `MissionAdapter` реализован отдельно.

**Текущий scenario builder** (`swarm-scenarios/src/`):

- Каждая миссия имеет свой builder: `build_coverage_scenario`, `build_sar_scenario`,
  `build_inspection_scenario`, `build_emergency_mesh_scenario`.
- Каждый builder принимает конфиг и возвращает `(Scenario, RunConfig)`.
- Профили группируются в `*StandardProfiles` структуры.

**Текущий replay** (`swarm-replay/src/event_log.rs`):

- `Event` enum содержит `SarScan`, `SarDetection`, `EdgeVisited`, `SafetyViolation`, `CbbaConverged`, `CbbaBundleUpdated`.
- Добавление нового event type требует обновления `Event` enum + builder.

**Текущий DSL** (`swarm-sim/src/dsl.rs`):

- `ScenarioSuite` хранит `mission: String`, `profile: String`, `scenario: Scenario`, `run_config: RunConfig`.
- Валидация проверяет `schema_version == "0.1"` и непустые поля.
- Новая миссия не требует изменений DSL ядра, если `mission` string произвольная.

**Текущий benchmark harness** (`swarm-sim/src/benchmark.rs`):

- `BenchmarkHarness::run_smoke/run_quick/run_full` принимают `ScenarioBuilder` и `StrategyFactory`.
- Новая миссия интегрируется через добавление builder в `strategy_comparison.rs` / `regression_runner.rs`.

## Affected Components

| Компонент | Файл | Тип изменения |
|---|---|---|
| `swarm-types` | `src/task.rs` | добавить `TaskKind::MappingZone` |
| `swarm-types` | `src/pose.rs` | добавить `Aabb` (axis-aligned bounding box) |
| `swarm-types` | `src/mission.rs` | добавить `wildfire_scanned_zones` в `RunState` (или использовать `completed_tasks`) |
| `swarm-scenarios` | `src/wildfire.rs` (новый) | `WildfireConfig`, `HazardZone`, `WildfireProfile`, `build_wildfire_scenario` |
| `swarm-scenarios` | `src/lib.rs` | re-export wildfire модулей |
| `swarm-scenarios` | `src/profiles.rs` | добавить `WildfireStandardProfiles` (опционально) |
| `swarm-sim` | `src/runner.rs` | обработка `MappingZone` tasks в `run_internal` (обнаружение → обновление hazard map → re-prioritization) |
| `swarm-sim` | `src/dsl.rs` | `validate_mission_specific` — добавить wildfire-валидацию |
| `swarm-replay` | `src/event_log.rs` | добавить `HazardMapUpdated`, `AgentObservation` events |
| `swarm-metrics` | `src/metrics.rs` | добавить `hazard_zones_mapped`, `avg_threat_level_final`, `priority_updates` |
| `swarm-examples` | `src/bin/strategy_comparison.rs` | добавить `Mission::Wildfire` + builder |
| `swarm-examples` | `src/bin/regression_runner.rs` | добавить wildfire suites в `default_suites()` (опционально, post-M30) |
| `swarm-examples` | `tests/wildfire.rs` (новый) | интеграционные тесты для wildfire mission |
| `README.md` | — | документация по Wildfire/Flood Mapping |

## Implementation Steps

### Шаг 1: Domain model (`swarm-types`)

**Файл:** `crates/swarm-types/src/pose.rs`

```rust
/// Axis-aligned bounding box in 2D.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Aabb {
    pub fn contains(&self, pose: &Pose) -> bool {
        pose.x >= self.min_x && pose.x <= self.max_x &&
        pose.y >= self.min_y && pose.y <= self.max_y
    }

    pub fn center(&self) -> Pose {
        Pose {
            x: (self.min_x + self.max_x) / 2.0,
            y: (self.min_y + self.max_y) / 2.0,
        }
    }

    pub fn area(&self) -> f64 {
        (self.max_x - self.min_x) * (self.max_y - self.min_y)
    }
}
```

**Файл:** `crates/swarm-types/src/task.rs`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    CoverageCell,
    SarScan,
    SarConfirmationScan,
    InspectionEdge,
    RelayPlacement,
    Waypoint,
    MappingZone, // M30
}
```

**Файл:** `crates/swarm-types/src/mission.rs`

Добавить в `RunState`:
```rust
pub struct RunState {
    pub scanned_cells: HashSet<(u32, u32)>,
    pub covered_edges: HashSet<EdgeId>,
    pub completed_tasks: HashSet<TaskId>,
    pub mapped_zones: HashSet<String>, // M30
}
```

### Шаг 2: Scenario builder (`swarm-scenarios`)

**Файл:** `crates/swarm-scenarios/src/wildfire.rs`

```rust
use swarm_sim::{RunConfig, Scenario};
use swarm_types::{Aabb, Agent, AgentId, Capability, Health, Pose, Role, Task, TaskId, TaskStatus, TaskKind};

pub struct HazardZone {
    pub id: String,
    pub bounds: Aabb,
    pub threat_level: f64, // 0.0..1.0
    pub priority: u8,
}

pub struct WildfireConfig {
    pub seed: u64,
    pub agent_count: u32,
    pub zones: Vec<HazardZone>,
    pub update_interval_ticks: u64,
    pub max_ticks: u64,
    pub enable_dynamic_threat: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WildfireProfile {
    SmallStatic,   // 4x4, 2 agents, static hazard map
    MediumDynamic, // 8x8, 4 agents, 2 update events
}

impl WildfireProfile {
    pub fn config(&self, seed: u64) -> WildfireConfig {
        match self {
            Self::SmallStatic => WildfireConfig {
                seed,
                agent_count: 2,
                zones: vec![
                    HazardZone {
                        id: "zone-a".to_owned(),
                        bounds: Aabb { min_x: 0.0, min_y: 0.0, max_x: 20.0, max_y: 20.0 },
                        threat_level: 0.7,
                        priority: 5,
                    },
                    HazardZone {
                        id: "zone-b".to_owned(),
                        bounds: Aabb { min_x: 20.0, min_y: 20.0, max_x: 40.0, max_y: 40.0 },
                        threat_level: 0.3,
                        priority: 3,
                    },
                ],
                update_interval_ticks: 999,
                max_ticks: 200,
                enable_dynamic_threat: false,
            },
            Self::MediumDynamic => WildfireConfig {
                seed,
                agent_count: 4,
                zones: vec![
                    HazardZone {
                        id: "zone-a".to_owned(),
                        bounds: Aabb { min_x: 0.0, min_y: 0.0, max_x: 20.0, max_y: 20.0 },
                        threat_level: 0.5,
                        priority: 4,
                    },
                    HazardZone {
                        id: "zone-b".to_owned(),
                        bounds: Aabb { min_x: 20.0, min_y: 0.0, max_x: 40.0, max_y: 20.0 },
                        threat_level: 0.2,
                        priority: 2,
                    },
                    HazardZone {
                        id: "zone-c".to_owned(),
                        bounds: Aabb { min_x: 0.0, min_y: 20.0, max_x: 20.0, max_y: 40.0 },
                        threat_level: 0.4,
                        priority: 3,
                    },
                    HazardZone {
                        id: "zone-d".to_owned(),
                        bounds: Aabb { min_x: 20.0, min_y: 20.0, max_x: 40.0, max_y: 40.0 },
                        threat_level: 0.1,
                        priority: 1,
                    },
                ],
                update_interval_ticks: 50,
                max_ticks: 400,
                enable_dynamic_threat: true,
            },
        }
    }
}

pub fn build_wildfire_scenario(config: &WildfireConfig) -> (Scenario, RunConfig) {
    // Build agents
    // Build tasks: one MappingZone task per HazardZone
    // Task.pose = zone.bounds.center()
    // Task.priority = zone.priority
    // Task.kind = Some(TaskKind::MappingZone)
    // Task.required_capabilities = vec![Capability::from("thermal".to_owned())]
}
```

### Шаг 3: Mission adapter (`swarm-alloc` или `swarm-scenarios`)

**Файл:** `crates/swarm-scenarios/src/wildfire.rs` (impl block)

```rust
use swarm_alloc::AllocationAgent;
use swarm_types::{MissionAdapter, RunState, Pose, Task, TaskKind};

pub struct WildfireAdapter;

impl MissionAdapter for WildfireAdapter {
    fn task_kind(&self, _task: &Task) -> TaskKind {
        TaskKind::MappingZone
    }

    fn route_cost(&self, from: Pose, task: &Task) -> f64 {
        let to = task.pose.unwrap_or(from);
        from.distance_to(&to)
    }

    fn is_completed(&self, task: &Task, state: &RunState) -> bool {
        state.mapped_zones.contains(&task.id.0)
    }

    fn score(&self, agent: &AllocationAgent, task: &Task) -> f64 {
        // Higher priority = higher score; thermal capability = bonus
        let priority_bonus = task.priority as f64;
        let capability_bonus = if agent.capabilities.iter().any(|c| c.0 == "thermal") {
            10.0
        } else {
            0.0
        };
        priority_bonus + capability_bonus
    }
}
```

### Шаг 4: Runner integration (`swarm-sim`)

**Файл:** `crates/swarm-sim/src/runner.rs`

В `run_internal`, в цикле по tick:
- Если agent достигает pose задачи с `kind == MappingZone`:
  1. Добавить zone id в `mapped_zones` (внутри `RunState` или аналогичной структуре).
  2. Записать `Event::AgentObservation { agent_id, zone_id, tick }`.
  3. Если `enable_dynamic_threat` и `tick % update_interval_ticks == 0`:
     - Обновить `threat_level` зон (например, увеличить на 0.1).
     - Пересчитать `priority` задач.
     - Записать `Event::HazardMapUpdated { zone_id, new_threat_level, tick }`.

**Важно:** `RunState` не хранится в `ScenarioRunner` напрямую. Нужно добавить
`wildfire_state: Option<WildfireState>` в `RunConfig` или внутрь runner.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WildfireState {
    pub zones: Vec<HazardZone>,
    pub mapped_zone_ids: HashSet<String>,
    pub update_interval_ticks: u64,
}
```

Добавить в `RunConfig`:
```rust
pub wildfire_state: Option<WildfireState>,
```

### Шаг 5: Replay events

**Файл:** `crates/swarm-replay/src/event_log.rs`

```rust
pub enum Event {
    // ... existing variants ...
    AgentObservation {
        agent_id: AgentId,
        zone_id: String,
        tick: u64,
    },
    HazardMapUpdated {
        zone_id: String,
        new_threat_level: f64,
        new_priority: u8,
        tick: u64,
    },
    TaskPriorityUpdated {
        task_id: TaskId,
        old_priority: u8,
        new_priority: u8,
        tick: u64,
    },
}
```

### Шаг 6: Metrics

**Файл:** `crates/swarm-metrics/src/metrics.rs`

Добавить в `RunMetrics`:
```rust
#[serde(default)]
pub hazard_zones_mapped: u64,
#[serde(default)]
pub priority_updates: u64,
#[serde(default)]
pub final_avg_threat_level: f64,
```

Добавить в `AggregateMetrics`:
```rust
#[serde(default)]
pub avg_hazard_zones_mapped: f64,
#[serde(default)]
pub avg_priority_updates: f64,
#[serde(default)]
pub avg_final_threat_level: f64,
```

Обновить `AggregateMetrics::from_runs`.

### Шаг 7: DSL validation

**Файл:** `crates/swarm-sim/src/dsl.rs`

В `validate_mission_specific` (или создать новую функцию):
```rust
fn validate_wildfire(entry: &ScenarioSuiteEntry) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    // Check that all tasks have kind == MappingZone
    // Check that zones have valid threat_level (0.0..1.0)
    // Check that poses are within zone bounds
    errors
}
```

### Шаг 8: CLI integration

**Файл:** `crates/swarm-examples/src/bin/strategy_comparison.rs`

Добавить:
```rust
enum Mission {
    Coverage,
    EmergencyMesh,
    Sar,
    Inspection,
    Wildfire, // M30
}
```

В `parse_mission`:
```rust
"wildfire" => vec![Mission::Wildfire],
```

В цикл `for mission in &cli.missions`:
```rust
Mission::Wildfire => {
    let profiles: Vec<String> = vec!["small-static".to_owned(), "medium-dynamic".to_owned()];
    let builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
        let profile = WildfireProfile::from_str(profile_name).unwrap_or(WildfireProfile::SmallStatic);
        build_wildfire_scenario(&profile.config(seed))
    });
    (profiles, builder)
}
```

### Шаг 9: Benchmark

**Файл:** `crates/swarm-examples/src/bin/strategy_comparison.rs`

Baseline comparison между `greedy`, `auction`, `cbba` на `SmallStatic` scenario.

```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --smoke --mission wildfire --output-dir results/wildfire_smoke/
```

### Шаг 10: Актуализация README

Добавить разделы:
- "Wildfire / Flood Mapping" в Current Status
- `M30` в Milestones Overview
- Описание semantics и ограничений

## Testing Strategy

### Категория 1 — Без рефакторинга (unit + integration)

**Unit: DSL parse/validation**
- Файл: `crates/swarm-sim/src/dsl.rs` (в `#[cfg(test)]`)
- `validate_wildfire` с корректным entry → пустой errors.
- `validate_wildfire` с `threat_level > 1.0` → error.
- `validate_wildfire` с task без `kind` → error.

**Unit: Task generation**
- Файл: `crates/swarm-scenarios/src/wildfire.rs`
- `build_wildfire_scenario` создаёт по одному `Task` на `HazardZone`.
- Каждый task имеет `kind == MappingZone`, `pose` внутри `bounds`.

**Unit: Mission adapter**
- Файл: `crates/swarm-scenarios/src/wildfire.rs`
- `WildfireAdapter::is_completed` → true когда zone id в `mapped_zones`.
- `WildfireAdapter::score` → thermal capability даёт bonus.

**Unit: Replay event serialization**
- Файл: `crates/swarm-replay/src/event_log.rs`
- `AgentObservation`, `HazardMapUpdated`, `TaskPriorityUpdated` roundtrip через serde.

**Integration: Benchmark smoke test**
- Файл: `crates/swarm-examples/tests/wildfire.rs`
- Запуск `--smoke --mission wildfire`.
- Assert: exit code 0, `results.json` содержит `avg_hazard_zones_mapped`.

### Категория 2 — Лёгкий рефакторинг (integration)

**Integration: Hazard map builders**
- `WildfireProfile::SmallStatic` и `MediumDynamic` builders возвращают корректные `Aabb`.
- `Aabb::contains` работает для pose внутри и снаружи bounds.

**Integration: Fake observation/update helpers**
- `WildfireState::update_threat_levels` корректно увеличивает threat level.
- `priority` пересчитывается пропорционально threat level.

**Integration: Reusable benchmark fixtures**
- `make_wildfire_scenario_builder` helper для тестов.

**Integration: Priority update assertions**
- После `HazardMapUpdated` задачи с обновлёнными зонами имеют повышенный priority.
- `auction` allocator перераспределяет агентов на высокоприоритетные зоны.

### Категория 3 — Тяжёлый рефакторинг

**Property test: Dynamic re-prioritization**
- Файл: `crates/swarm-examples/tests/wildfire_proptest.rs`
- Для random seed и random threat level updates:
  - `priority` монотонно не убывает при увеличении threat level.
  - `mapped_zones` не уменьшается.

**Multi-seed stability**
- `SmallStatic` scenario с seeds 0..100:
  - `success_rate > 0.9` (все зоны мапятся).
  - `avg_hazard_zones_mapped` == число зон.

**Comparative strategy test**
- `greedy`, `auction`, `cbba` на `SmallStatic`:
  - Все стратегии достигают `success_rate == 1.0`.
  - `auction` имеет минимальное `total_distance_travelled` (не строго, но тенденция).

## Risks and Tradeoffs

1. **Dynamic threat model complexity** — обновление приоритетов на каждом tick может быть дорогим.
   Mitigation: обновлять только по `update_interval_ticks`, не на каждом tick.

2. **Backward compatibility** — новые поля в `RunConfig` и `RunMetrics` ломают десериализацию старых JSON.
   Mitigation: использовать `#[serde(default)]` для всех новых полей.

3. **TaskKind proliferation** — каждая новая миссия добавляет variant в `TaskKind`.
   Mitigation: M31+ рассмотреть переход на string-based `TaskKind` или plugin system.

4. **Runner bloat** — `run_internal` уже ~1400 строк. Добавление wildfire-логики увеличит его.
   Mitigation: вынести mission-specific логику в `MissionAdapter::tick` callback.

5. **Flaky thresholds** — dynamic scenarios имеют высокую вариативность.
   Mitigation: regression suites для wildfire использовать `Quick` (10 seeds) вместо `Smoke`.

## Open Questions

1. **Нужен ли отдельный `WildfireAllocator`?**
   Нет — `greedy` и `auction` достаточно, если `Task.priority` обновляется динамически.
   CBBA не поддерживает динамические приоритеты (known limitation), маркируем `unsupported`.

2. **Как моделировать spread (распространение) пожара/наводнения?**
   M30 ограничен static/dynamic threat levels. Spread (соседние зоны заражаются) — M32+.

3. **Нужен ли графический replay для hazard map?**
   ASCII replay через `swarm-replay` достаточен для M30. Heatmap viz — M33+.

4. **Как часто обновлять baseline для wildfire?**
   После стабилизации thresholds — один раз. Динамические scenarios менее стабильны,
   baseline может дрейфовать чаще.

5. **Поддержка flood отличается от wildfire?**
   M30 использует unified "hazard mapping" model. Различия (water vs fire) —
   косметические (названия, цвета в replay). Физика spread отличается, но это M32+.
