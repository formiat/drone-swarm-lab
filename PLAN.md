# PLAN: M28 — Planner Quality Upgrade

## Контекст

M28 улучшает качество маршрутного планирования для task bundles. Сейчас CBBA использует жадный
nearest-neighbour TSP (`order_bundle_tsp` в `swarm-alloc/src/cbba.rs:61`) для упорядочивания задач
в bundle. Этот алгоритм:

1. **Не оптимален** — NN TSP даёт маршруты в среднем на 25% длиннее оптимальных.
2. **Не учитывает батарею** — агент может получить bundle, который физически невозможно выполнить
   из-за ограничений батареи (особенно в constrained сценариях inspection perimeter и SAR battery-constrained).
3. **Не изолирован** — логика ordering захардкожена в CBBA, хотя та же проблема актуальна и для
   других allocators (centralized, greedy при multi-task assignment).

M28 вводит `RoutePlanner` trait с несколькими реализациями и общую функцию `route_cost`,
что позволяет:
- заменить NN на 2-opt для лучших маршрутов;
- добавить battery-aware feasibility check перед формированием bundle;
- собирать новые метрики (route length, wasted travel, return reserve).

## Investigation Context

`INVESTIGATION.md` отсутствует. Ниже — ключевые наблюдения из инспекции кода.

**Текущий route ordering** (`swarm-alloc/src/cbba.rs:61`):

```rust
pub fn order_bundle_tsp(agent_pose: Pose, bundle: &[TaskId], tasks: &[Task]) -> Vec<TaskId>
```

Реализует greedy nearest-neighbour: на каждом шаге выбирается ближайшая непосещённая задача.
Сложность O(n²), результат — permutation задач.

**Текущий scoring в CBBA** (`swarm-alloc/src/cbba.rs:132`):

```rust
pub fn marginal_score(&self, agent: &AllocationAgent, task: &Task, bundle: &[TaskId]) -> f64
```

Учитывает расстояние от агента до задачи, батарею и position penalty (длина bundle),
но не проверяет feasibility по суммарному пути.

**Battery constraint** уже существует в inspection (`swarm-scenarios/src/inspection.rs:79`):

```rust
let (battery, battery_drain_rate, max_range) = if config.battery_constraint > 0.0 { ... }
```

Но allocator не проверяет, что агент сможет выполнить все задачи в bundle и вернуться на базу.

**Metrics** (`swarm-metrics/src/lib.rs`): сейчас собираются `success_rate`, `total_ticks`,
`agents_exhausted`, `edge_coverage_rate`, `avg_distance_travelled`. Новые метрики M28
(`avg_route_length`, `avg_wasted_travel`, `avg_return_reserve`, `avg_infeasible_routes`)
должны дополнить существующие.

**Inspection benchmark** (`swarm-sim/src/benchmark.rs`): уже есть `BenchmarkHarness`,
который может запускать inspection linear и собирать метрики. M28 добавит сравнение
NN vs 2-opt в этот harness.

## Affected Components

| Компонент | Файл | Тип изменения |
|---|---|---|
| `swarm-alloc` | `src/route_planner.rs` (новый) | `RoutePlanner` trait + 3 реализации |
| `swarm-alloc` | `src/lib.rs` | re-export `RoutePlanner`, `route_cost` |
| `swarm-alloc` | `src/cbba.rs` | использовать `RoutePlanner` вместо `order_bundle_tsp` |
| `swarm-alloc` | `src/centralized.rs` | опционально: `RoutePlanner` для ordering |
| `swarm-sim` | `src/metrics.rs` / `swarm-metrics` | 4 новые метрики |
| `swarm-sim` | `src/benchmark.rs` | benchmark NN vs 2-opt для inspection |
| `swarm-examples` | `src/bin/strategy_comparison.rs` | вывод новых метрик в JSON/CSV |
| `README.md` | — | актуализация статуса M28 |

## Implementation Steps

### Шаг 1: `RoutePlanner` trait и `route_cost`

**Файл:** `crates/swarm-alloc/src/route_planner.rs`

```rust
use swarm_types::{Agent, AgentId, Pose, Task, TaskId};

/// Общая функция стоимости маршрута: суммарное евклидово расстояние.
pub fn route_cost(start: Pose, tasks: &[&Task]) -> f64 {
    let mut total = 0.0;
    let mut current = start;
    for task in tasks {
        if let Some(pose) = task.pose {
            total += current.distance_to(&pose);
            current = pose;
        }
    }
    total
}

pub trait RoutePlanner: Send + Sync {
    /// Вернуть упорядоченный список TaskId для выполнения.
    fn order(&self, start: Pose, tasks: &[Task], agent: &Agent) -> Vec<TaskId>;
    /// Проверить, что агент может выполнить все задачи и вернуться на базу.
    fn is_feasible(&self, start: Pose, tasks: &[Task], agent: &Agent) -> bool;
}

/// Greedy nearest-neighbour (текущий алгоритм).
pub struct NearestNeighbourPlanner;

impl RoutePlanner for NearestNeighbourPlanner {
    fn order(&self, start: Pose, tasks: &[Task], _agent: &Agent) -> Vec<TaskId> {
        // ... реализация из order_bundle_tsp
    }
    fn is_feasible(&self, _start: Pose, _tasks: &[Task], _agent: &Agent) -> bool {
        true // NN не проверяет feasibility
    }
}

/// 2-opt local search для улучшения маршрута.
pub struct TwoOptPlanner {
    pub max_iterations: usize,
}

impl Default for TwoOptPlanner {
    fn default() -> Self {
        Self { max_iterations: 1000 }
    }
}

impl RoutePlanner for TwoOptPlanner {
    fn order(&self, start: Pose, tasks: &[Task], _agent: &Agent) -> Vec<TaskId> {
        // 1. Начать с NN ordering
        // 2. Пытаться swap двух рёбер: если уменьшает route_cost — принять
        // 3. Остановка при отсутствии улучшения или достижении max_iterations
    }
    fn is_feasible(&self, _start: Pose, _tasks: &[Task], _agent: &Agent) -> bool {
        true
    }
}

/// Battery-aware feasibility check + NN ordering.
pub struct BatteryAwarePlanner {
    pub reserve_fraction: f64,
    pub inner: Box<dyn RoutePlanner>,
}

impl Default for BatteryAwarePlanner {
    fn default() -> Self {
        Self {
            reserve_fraction: 0.2,
            inner: Box::new(NearestNeighbourPlanner),
        }
    }
}

impl RoutePlanner for BatteryAwarePlanner {
    fn order(&self, start: Pose, tasks: &[Task], agent: &Agent) -> Vec<TaskId> {
        // 1. Получить ordering от inner planner
        // 2. Проверить feasibility: если нет — отбросить последнюю задачу и повторить
        let ordered = self.inner.order(start, tasks, agent);
        let mut feasible = ordered.clone();
        while !feasible.is_empty() && !self.is_feasible(start, tasks, agent) {
            feasible.pop();
        }
        feasible
    }

    fn is_feasible(&self, start: Pose, tasks: &[Task], agent: &Agent) -> bool {
        // Суммарный путь + возврат на базу ≤ max_range * (1 - reserve_fraction)
        // Учитывать battery_drain_rate
        let total_distance = route_cost(start, tasks);
        let return_distance = if let Some(last) = tasks.last() {
            last.pose.map(|p| p.distance_to(&start)).unwrap_or(0.0)
        } else {
            0.0
        };
        let required = (total_distance + return_distance) * agent.battery_drain_rate;
        required <= agent.battery * (1.0 - self.reserve_fraction)
    }
}
```

**Файл:** `crates/swarm-alloc/src/lib.rs`

```rust
pub mod route_planner;
pub use route_planner::{route_cost, RoutePlanner};
```

### Шаг 2: Заменить `order_bundle_tsp` на `RoutePlanner` в CBBA

**Файл:** `crates/swarm-alloc/src/cbba.rs`

- Добавить поле `pub route_planner: Box<dyn RoutePlanner>` в `CbbaAllocator`.
- Default: `Box::new(NearestNeighbourPlanner)` для backward compatibility.
- В `allocate()` (строка ~310) заменить `order_bundle_tsp(...)` на:
  ```rust
  let agent = ...;
  let ordered = self.route_planner.order(agent.pose, bundle_tasks, agent);
  *bundle = ordered;
  ```
- Удалить `order_bundle_tsp` и `bundle_travel_distance` (функциональность переезжает в `route_planner`).

### Шаг 3: Новые метрики

**Файл:** `crates/swarm-metrics/src/lib.rs` (или `crates/swarm-sim/src/metrics.rs`)

Добавить поля в `RunMetrics`:

```rust
pub avg_route_length: f64,
pub avg_wasted_travel: f64,      // путь без полезной работы (между задачами без выполнения)
pub avg_return_reserve: f64,     // остаток батареи при возврате на базу
pub avg_infeasible_routes: f64,  // сколько раз bundle был отклонён
```

**Файл:** `crates/swarm-sim/src/runner.rs`

- После завершения run вычислять `avg_route_length` как сумму расстояний между assigned tasks.
- `avg_wasted_travel` = `avg_route_length` − сумма расстояний от pose задачи до базы (или другая метрика).
- `avg_return_reserve` = батарея агента − drain на выполнение bundle.
- `avg_infeasible_routes` = счётчик отказов `BatteryAwarePlanner`.

### Шаг 4: Benchmark NN vs 2-opt

**Файл:** `crates/swarm-sim/src/benchmark.rs`

Добавить метод в `BenchmarkHarness`:

```rust
pub fn run_planner_comparison(
    &self,
    planner_a: Box<dyn RoutePlanner>,
    planner_b: Box<dyn RoutePlanner>,
) -> (BenchmarkResult, BenchmarkResult)
```

Запускать inspection linear с обоими планировщиками, сравнивать `avg_route_length`.

**Файл:** `crates/swarm-examples/src/bin/strategy_comparison.rs`

Добавить CLI флаг `--planner {nn|two-opt|battery-aware}`.

### Шаг 5: Актуализация README

Добавить M28 в таблицу Milestones Overview и Current Status.

## Testing Strategy

### Категория 1 — Без рефакторинга (unit + integration)

**Unit: 2-opt не ухудшает маршрут**
- Файл: `crates/swarm-alloc/src/route_planner.rs` (в `#[cfg(test)]`)
- Генерировать случайный набор задач (5–10 штук).
- `route_cost(nn_ordering)` ≥ `route_cost(two_opt_ordering)`.
- Запуск: `cargo test -p swarm-alloc route_planner`.

**Unit: battery-aware отклоняет нефeasible bundle**
- Агент с `battery = 10.0`, `battery_drain_rate = 1.0`, `max_range = 5.0`.
- Bundle из 3 задач на расстоянии 10.0 каждая.
- `BatteryAwarePlanner::is_feasible` → `false`.
- `BatteryAwarePlanner::order` возвращает подмножество задач (или пустой вектор).

**Integration: inspection linear, 2-opt vs NN**
- Файл: `crates/swarm-scenarios/tests/inspection.rs` (дополнить существующий)
- Запустить `InspectionProfile::Linear` с `TwoOptPlanner` и `NearestNeighbourPlanner`.
- Утверждение: `avg_route_length` для 2-opt ≤ NN.

### Категория 2 — Лёгкий рефакторинг (proptest)

**Proptest: 2-opt корректность**
- Файл: `crates/swarm-alloc/tests/proptest_route_planner.rs`
- Стратегия: случайный `Pose` (0..100, 0..100), случайный `Agent` (battery 10..100).
- Свойства:
  1. Результат `order()` — permutation входных `TaskId` (нет дубликатов, все присутствуют).
  2. `is_feasible()` не паникует на любых входах.
  3. Для `BatteryAwarePlanner`: returned tasks ⊆ input tasks.

### Категория 3 — Тяжёлый рефакторинг

Не требуется. M28 не меняет структуры данных и не требует миграций.

## Risks and Tradeoffs

1. **2-opt O(n²) может замедлить CBBA** на больших bundles (max_bundle_size = 5 → n = 5,
   O(25) итераций, negligible). Если в будущем max_bundle_size вырастет — потребуется
   более быстрый планировщик (Lin-Kernighan или ортогональная декомпозиция).

2. **Battery-aware отбрасывает задачи** — это может уменьшить `success_rate` в обмен на
   `agents_exhausted = 0`. Tradeoff: консервативный планировщик безопаснее, но может
   оставлять задачи невыполненными.

3. **Обратная совместимость CBBA** — поле `route_planner: Box<dyn RoutePlanner>` требует
   изменения конструктора `CbbaAllocator`. Все существующие вызовы (`swarm-sim/src/runner.rs`,
   `swarm-examples/src/bin/strategy_comparison.rs`) нужно обновить.

4. **Метрики `avg_wasted_travel`** — определение "wasted" субъективно (путь между задачами
   vs путь до базы). Нужно зафиксировать формулу в комментариях.

## Open Questions

1. Нужно ли добавить `RoutePlanner` в `GreedyAllocator` и `AuctionAllocator`?
   Сейчас они назначают по одной задаче за раз — ordering неактуален. Но если в будущем
   добавим multi-task greedy — trait уже готов.

2. Как учитывать `MissionAdapter::route_cost` (M27) в `route_cost` функции?
   Возможно, `RoutePlanner` должен принимать `&dyn MissionAdapter` для mission-specific
   costing. Оставлено за рамками M28 — можно добавить в M29.

3. Нужен ли `ThreeOptPlanner` или `LinKernighanPlanner`?
   Для bundle size ≤ 5 2-opt достаточно. Если inspection перейдёт на большие графы —
   рассмотреть в M30+.
