# PLAN: Safety Layer (M13)

## Context

Milestone 11 (hardening) и Mission DSL (v0.12) завершены. Платформа имеет 5 стратегий аллокации, 3 миссии, JSON/CSV export и декларативные сценарии. Следующий шаг по roadmap из `docs/DRONE_B.8.md` — Safety Layer (M13).

**Safety Layer** добавляет физические и операционные ограничения поверх аллокатора и runtime: геозоны, no-fly зоны, separation constraints. Это необходимый слой перед SITL/реальными дронами и полезный для constrained planning benchmark.

**Источники контекста:** `docs/DRONE_A.7.md`, `docs/DRONE_B.7.md`, `docs/DRONE_B.8.md`. INVESTIGATION.md отсутствует.

**Текущее состояние (Mission DSL v0.12 complete):**
- `Agent` — `pose`, `battery`, `speed`, `max_range`, `comms_range`, `battery_drain_rate`.
- `Task` — `pose`, `grid_cell`, `required_capabilities`, `required_role`.
- 5 аллокаторов: Greedy, Auction, CBBA, Centralized, ConnectivityAware.
- `RunConfig` с serde, `ScenarioSuite` с JSON сценариями.
- `AgentNode::tick` — движение к задаче, drain battery.
- `RunMetrics` / `AggregateMetrics` — расширяемые метрики.

**Критерий готовности:**
1. Новый крейт `swarm-safety` с `SafetyConfig`, `Geofence`, `NoFlyZone`, `SeparationConstraint`, `SafetyViolation`.
2. Все 5 аллокаторов фильтруют задачи через `filter_safe_tasks` перед назначением.
3. `AgentNode::tick` проверяет `check_agent` перед движением, логирует нарушения.
4. `RunConfig` содержит `safety_config: Option<SafetyConfig>` (с serde).
5. `RunMetrics` содержит `safety_violations` метрику.
6. JSON-сценарий `scenarios/coverage.safety.json` с no-fly зоной.
7. README обновлён с документацией Safety Layer.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_B.8.md`:
- Safety Layer — Stage 1 гибридного roadmap.
- Нужен перед SITL/реальными дронами.
- Полезен как constrained planning benchmark для всех веток.
- Детали типов и API определены в DRONE_B.8.md.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-safety/` (новый) | Крейт: Geofence, NoFlyZone, SeparationConstraint, SafetyConfig, SafetyViolation, filter_safe_tasks, check_agent, is_task_reachable |
| `crates/swarm-alloc/src/allocator.rs` | Добавить `SafetyConfig` в `AllocationContext` или параметр allocate |
| `crates/swarm-alloc/src/allocator.rs` | Greedy/Auction/CBBA/Centralized/ConnectivityAware вызывают `filter_safe_tasks` |
| `crates/swarm-runtime/src/node.rs` | `AgentNode::tick` проверяет `check_agent` перед движением |
| `crates/swarm-sim/src/runner.rs` | `RunConfig` добавляет `safety_config: Option<SafetyConfig>` |
| `crates/swarm-metrics/src/metrics.rs` | `RunMetrics` добавляет `safety_violations: u64`; `AggregateMetrics` добавляет `avg_safety_violations: f64` |
| `crates/swarm-sim/src/report_export.rs` | Добавить `safety_violations` в JSON/CSV export |
| `scenarios/coverage.safety.json` | **NEW** — сценарий с no-fly зоной в центре сетки |
| `README.md` | Документировать Safety Layer, примеры сценариев, метрики |

---

## Implementation Steps

### Шаг 1 — Новый крейт `swarm-safety`

Файл: `crates/swarm-safety/Cargo.toml` (новый), `crates/swarm-safety/src/lib.rs` (новый)

Типы:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Geofence {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoFlyZone {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeparationConstraint {
    pub min_distance_m: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SafetyConfig {
    pub geofence: Option<Geofence>,
    pub no_fly_zones: Vec<NoFlyZone>,
    pub separation: Option<SeparationConstraint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SafetyViolation {
    pub agent_id: AgentId,
    pub violation_type: ViolationType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViolationType {
    GeofenceExited,
    NoFlyZoneEntered,
    SeparationBreached { other_agent_id: AgentId },
}
```

Публичный API:

```rust
pub fn check_agent(
    config: &SafetyConfig,
    agent: &Agent,
    others: &[Agent],
) -> Vec<SafetyViolation>;

pub fn is_task_reachable(config: &SafetyConfig, agent: &Agent, task: &Task) -> bool;

pub fn filter_safe_tasks<'a>(
    config: &SafetyConfig,
    agent: &Agent,
    tasks: &'a [Task],
) -> Vec<&'a Task>;
```

**Тесты (категория 1):**
- `check_agent_no_violations_outside_nofly` — агент вне no-fly → пустой vec
- `check_agent_nofly_violation` — агент внутри no-fly → violation
- `check_agent_geofence_exited` — агент за пределами geofence → violation
- `check_agent_separation_breached` — два агента ближе min_distance → violation
- `is_task_reachable_nofly_blocked` — задача в no-fly → false
- `filter_safe_tasks_excludes_nofly` — задачи в no-fly исключаются

### Шаг 2 — Safety-aware аллокация

Файл: `crates/swarm-alloc/src/allocator.rs`

Добавить `safety_config: Option<&SafetyConfig>` в `AllocationContext` (или как отдельный параметр trait `Allocator` через default-параметр для backward compat):

```rust
pub trait Allocator {
    fn allocate(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)>;

    // ... existing methods ...
}
```

В каждом аллокаторе (Greedy, Auction, CBBA, Centralized, ConnectivityAware) перед основной логикой:
1. Если `safety_config` передан через `RunConfig` → фильтровать задачи через `filter_safe_tasks`.
2. Сохранять оригинальный пул задач для fallback (если все задачи unsafe, аллокатор получает empty pool и возвращает empty assignments — это корректное поведение).

Для интеграции с `ScenarioRunner`: `RunConfig.safety_config` передаётся в `allocate()` через обёртку.

**Тесты (категория 2):**
- `greedy_skips_nofly_tasks` — Greedy не назначает задачи в no-fly
- `auction_skips_nofly_tasks` — Auction фильтрует перед bidding
- `cbba_skips_nofly_tasks` — CBBA bundle не включает no-fly задачи
- `centralized_skips_nofly_tasks` — Centralized исключает из matching

### Шаг 3 — Runtime enforcement

Файл: `crates/swarm-runtime/src/node.rs`

В `AgentNode::tick` (или в методе движения `apply_movement`):

```rust
// Before moving toward task
if let Some(ref safety) = self.safety_config {
    let violations = swarm_safety::check_agent(safety, &self.agent_state, &other_agents);
    if !violations.is_empty() {
        // Stop movement, log violations
        self.log_safety_violations(&violations);
        return; // Do not move this tick
    }
}
```

`AgentNode` получает поле `safety_config: Option<SafetyConfig>` (устанавливается из `RunConfig` при создании node).

**Тесты (категория 2):**
- `agent_stops_at_nofly_boundary` — агент не входит в no-fly
- `agent_logs_geofence_exit` — violation записывается в EventLog

### Шаг 4 — Метрики и export

Файл: `crates/swarm-metrics/src/metrics.rs`

```rust
// RunMetrics
#[serde(default)]
pub safety_violations: u64,

// AggregateMetrics
#[serde(default)]
pub avg_safety_violations: f64,
```

Файл: `crates/swarm-sim/src/report_export.rs`

Добавить `safety_violations` в `ReportRow` и CSV headers.

Файл: `crates/swarm-sim/src/runner.rs`

В `run_internal`:
- Счётчик `safety_violations` увеличивается при каждом `check_agent` violation.
- Суммируется по всем агентам и тикам.

### Шаг 5 — Scenario JSON и README

Файл: `scenarios/coverage.safety.json` (новый)

Сценарий coverage с no-fly зоной в центре сетки (5×5, no-fly 2×2 в центре). 5 агентов, 3 задачи вне no-fly.

Файл: `README.md`

Добавить раздел **M13 — Safety Layer**:
- Описание Geofence, NoFlyZone, SeparationConstraint
- Пример JSON с safety_config
- Команда `--scenario-suite scenarios/coverage.safety.json`
- Метрика `safety_violations` в benchmark output

---

## Testing Strategy

### Категория 1 — unit тесты (swarm-safety)

- `check_agent_no_violations_outside_nofly`
- `check_agent_nofly_violation`
- `check_agent_geofence_exited`
- `check_agent_separation_breached`
- `is_task_reachable_nofly_blocked`
- `is_task_reachable_safe_task`
- `filter_safe_tasks_excludes_nofly`
- `filter_safe_tasks_preserves_safe`

### Категория 2 — integration

- `greedy_skips_nofly_tasks` — аллокатор получает задачу в no-fly, не назначает
- `auction_skips_nofly_tasks` — Auction фильтрует перед bidding
- `agent_stops_at_nofly_boundary` — runner: агент не входит в no-fly
- `coverage_safety_scenario_zero_violations` — JSON-сценарий: 0 violations в metrics
- `run_config_safety_config_serde_roundtrip` — SafetyConfig сериализуется/десериализуется

### Категория 3 — proptest / stress

- `check_agent_no_panic_random_positions` — случайные позиции + случайные geofence/no-fly → no panic, no NaN
- `filter_safe_tasks_no_panic_random` — случайные агенты/задачи/config → no panic

---

## Risks and Tradeoffs

**1. Performance overhead safety checks**

`check_agent` вызывается на каждый tick для каждого агента. Сложность O(N×M) где N — агенты, M — no-fly зоны. Для небольших сценариев (≤20 агентов) — незаметно. Для масштабных — нужна spatial index (R-tree). Митигация: пока no-fly зон мало, используем линейный поиск.

**2. Backward compatibility Allocator trait**

Добавление `SafetyConfig` в `allocate()` сигнатуру ломает все impl. Митигация: добавляем отдельный метод `allocate_with_safety(..., Option<&SafetyConfig>)` с default impl, вызывающим `allocate()`. Или передаём через `AllocationContext`.

**3. All tasks filtered out**

Если все задачи попадают в no-fly, аллокатор получает empty pool → no assignments → success=false. Это корректное поведение, но может сбивать с толку при анализе. Митигация: логировать warning при empty filtered pool.

**4. Interaction with movement**

Агент движется к задаче, но check_agent останавливает его перед no-fly. Агент может "застрять" на границе. Митигация: аллокатор уже не назначает задачи в no-fly, так что застревание маловероятно для coverage/sar. Для edge cases — агент ждёт переназначения.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| `Allocator` trait изменён | Все 5 impl + Strategy trait + call sites в runner | `cargo clippy`, `cargo test --workspace` |
| `RunConfig` новое поле `safety_config` | JSON roundtrip существующих сценариев | `cargo test` swarm-sim::dsl tests |
| `AgentNode` новое поле | Конструкторы в runner, multiprocess_scenario | `cargo test` + `cargo run --bin partition_scenario` |
| `RunMetrics` новое поле | AggregateMetrics::from_runs, export | `cargo test` swarm-metrics + report_export tests |
| `swarm-safety` crate не в workspace | Компиляция fails | `cargo build --workspace` |
| No-fly зона ломает SAR grid scan | GridState scan_cell проверяет pose, не safety | Integration test coverage.safety.json |

---

## Open Questions

1. **AABB vs произвольный полигон для no-fly?** — AABB для v0.13 (проще, быстрее). Произвольный полигон для v0.14+.
2. **Separation constraint — pairwise или global?** — Pairwise O(N²) для v0.13. Global spatial index если N > 20.
3. **Safety violation — fatal или recoverable?** — v0.13: recoverable (остановка + лог). v0.14+: configurable (stop / abort mission / reroute).
4. **Как safety влияет на CBBA distributed?** — CBBA агенты фильтруют локально, не обмениваются safety info. Это корректно, если все агенты видят одинаковый SafetyConfig (загружен из JSON).

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo test -p swarm-safety
cargo run -p swarm-examples --bin strategy_comparison -- \
  --scenario-suite scenarios/coverage.safety.json --json /tmp/safety.json
```
