# M34 — Planner Correctness v2

## Context

M33 закрыт. Mission Semantics Integration реализовано: 6 concrete adapters, `AdapterRegistry`, adapter-driven completion/scoring.

Следующий шаг по линейному плану DRONE_A.14.linear.md — M34 Planner Correctness v2.

## Investigation context

`INVESTIGATION.md` отсутствует. Анализ кода показал:

- `RoutePlanner` trait (`crates/swarm-alloc/src/route_planner.rs:20-27`) определён с 2 методами: `order` и `is_feasible`
- `NearestNeighbourPlanner` — greedy TSP, `is_feasible` всегда возвращает `true`
- `TwoOptPlanner` — local search, `is_feasible` всегда возвращает `true`
- `BatteryAwarePlanner` — оборачивает inner planner, но имеет **критический баг**:
  - `order()` вызывает `self.inner.order(start, tasks, agent)`, получает ordered list
  - затем проверяет `is_feasible(start, tasks, agent)` — но проверяет **оригинальный `tasks`**, а не ordered subset!
  - `ordered.pop()` удаляет задачи из ordered list, но `is_feasible` всегда проверяет полный набор
  - итог: `BatteryAwarePlanner::order` никогда не удаляет задачи, feasibility не работает
- `is_feasible` использует `agent.battery_drain_rate` (legacy v1), игнорирует `battery_model` v2
- `route_cost` считает distance, но не учитывает altitude (z coordinate)
- Runner (`crates/swarm-sim/src/runner.rs:1367-1370`) устанавливает планнер-метрики:
  - `avg_route_length = bundle_travel_distance` (от CBBA)
  - `avg_wasted_travel = 0.0` (hardcoded!)
  - `avg_return_reserve = final_battery_min` (не reserve, а минимальный остаток)
  - `infeasible_routes = 0` (hardcoded!)
- Regression (`crates/swarm-sim/src/regression.rs:207-209`) имеет `avg_route_length` baseline test

## Affected components

| Компонент | Путь | Что меняется |
|---|---|---|
| BatteryAwarePlanner | `crates/swarm-alloc/src/route_planner.rs` | Исправить `order()` и `is_feasible()` |
| RoutePlanner trait | `crates/swarm-alloc/src/route_planner.rs` | Возможно расширение (wasted travel, reserve) |
| Runner metrics | `crates/swarm-sim/src/runner.rs` | Вычислять wasted travel, infeasible routes, return reserve |
| CBBA | `crates/swarm-alloc/src/cbba.rs` | Использовать BatteryAwarePlanner с v2 battery model |
| Regression | `crates/swarm-sim/src/regression.rs` | Обновить baseline для route metrics |
| README | `README.md` | Обновить Current Status |

## Implementation steps

### 1. Исправить BatteryAwarePlanner::order

Файл: `crates/swarm-alloc/src/route_planner.rs`

**Баг:** `order()` проверяет feasibility на `tasks` (оригинальный набор), а не на ordered subset.

**Исправление:**
```rust
fn order(&self, start: Pose, tasks: &[Task], agent: &Agent) -> Vec<TaskId> {
    let mut ordered = self.inner.order(start, tasks, agent);
    // Build ordered task list from ids
    let task_by_id: HashMap<TaskId, &Task> = tasks.iter().map(|t| (t.id.clone(), t)).collect();
    let mut ordered_tasks: Vec<&Task> = ordered
        .iter()
        .map(|id| task_by_id.get(id).copied().unwrap())
        .collect();
    // Drop from the END of the ordered route until feasible
    while !ordered_tasks.is_empty() && !self.is_feasible(start, &ordered_tasks, agent) {
        ordered_tasks.pop();
        ordered.pop();
    }
    ordered
}
```

### 2. Обновить BatteryAwarePlanner::is_feasible для battery model v2

Файл: `crates/swarm-alloc/src/route_planner.rs`

**Текущий код:** использует `agent.battery_drain_rate` (legacy).

**Исправление:**
- Если `agent.battery_model` is `Some`, использовать `hover_drain_per_tick`, `climb_drain_per_meter`, `cruise_drain_per_meter`
- Horizontal distance → `cruise_drain_per_meter`
- Vertical distance (|dz|) → `climb_drain_per_meter`
- Reserve fraction → из `battery_model.reserve_fraction`
- Если `battery_model` is `None`, fallback на legacy `battery_drain_rate`

### 3. Вычислить meaningful route metrics в runner

Файл: `crates/swarm-sim/src/runner.rs`

**Текущий код (hardcoded):**
```rust
avg_route_length: bundle_travel_distance,
avg_wasted_travel: 0.0,
avg_return_reserve: final_battery_min,
infeasible_routes: 0,
```

**Исправление:**
- `avg_route_length` — оставить `bundle_travel_distance` (корректно для CBBA)
- `avg_wasted_travel` — вычислить как `bundle_travel_distance - nn_route_cost` (разница между фактическим и greedy NN оптимальным)
- `avg_return_reserve` — вычислить `final_battery - required_battery_for_return` (сколько осталось бы после возврата)
- `infeasible_routes` — считать, сколько агентов получили infeasible bundle (батарея < required)

### 4. Добавить route metrics для centralized planner

Файл: `crates/swarm-alloc/src/centralized.rs`

Сейчас centralized planner не использует `RoutePlanner`. Добавить:
- Опциональный `route_planner` поле
- При allocation вызывать `planner.order()` для ordering tasks per agent
- Вычислять `bundle_travel_distance` для каждого агента

### 5. Обновить regression baseline

Файл: `crates/swarm-sim/src/regression.rs`

- Обновить `avg_route_length` baseline (был 0.0, теперь будет non-zero)
- Добавить `avg_wasted_travel` baseline
- Добавить `avg_return_reserve` baseline
- Добавить `avg_infeasible_routes` baseline

### 6. Обновить README

Файл: `README.md`
- Обновить Planner Quality в Current Status (M34)
- Убрать из Known Limitations пункт про planner metrics

## Testing strategy

### Категория 1 — без рефакторинга

- **Unit test**: `BatteryAwarePlanner::order` с infeasible route → должен drop задачи
  ```rust
  let tasks = [t0, t1, t2]; // total distance > battery
  let ordered = planner.order(start, &tasks, &agent);
  assert!(ordered.len() < tasks.len());
  ```
- **Unit test**: `BatteryAwarePlanner::is_feasible` с battery model v2
  ```rust
  let agent = Agent { battery_model: Some(BatteryModel { ... }), .. };
  assert!(planner.is_feasible(start, &tasks, &agent));
  ```
- **Unit test**: `route_cost` с 3D pose (z coordinate)
  ```rust
  let start = Pose { x: 0.0, y: 0.0, z: 0.0 };
  let task = Pose { x: 3.0, y: 4.0, z: 5.0 };
  assert_eq!(route_cost(start, &[&task]), (3*3 + 4*4 + 5*5).sqrt());
  ```
- **Smoke test**: `--smoke --mission coverage --planner battery-aware` проходит
- **Smoke test**: `--smoke --mission coverage --planner two-opt` проходит

### Категория 2 — лёгкий рефакторинг

- **Route fixture builders**: `fn infeasible_route_fixture() -> (Vec<Task>, Agent)`
- **Battery model v2 fixtures**: `fn agent_with_battery_model() -> Agent`
- **Benchmark parser helper**: извлечь `avg_wasted_travel` из JSON report

### Категория 3 — тяжёлый рефакторинг

- **Planner comparison property test**: для random task sets, two-opt cost <= NN cost <= battery-aware dropped cost
- **Dynamic replanning**: после task release, planner reorder должен оставаться feasible
- **Long-run comparison**: `--full --mission coverage --planner {nn,two-opt,battery-aware}`

## Risks and tradeoffs

| Риск | Вероятность | Влияние | Митигация |
|---|---|---|---|
| BatteryAwarePlanner::order fix ломает CBBA bundle construction | Средняя | Высокое | Сохранить old behavior как `BatteryAwarePlannerV1`; новый как default |
| Battery model v2 integration несовместима со старыми агентами | Низкая | Среднее | Fallback на `battery_drain_rate` при `battery_model: None` |
| Wasted travel calculation expensive | Низкая | Низкое | Вычислять только в runner, не на каждом tick |
| Regression baseline сломается | Высокая | Среднее | Обновить baseline после fix |

## Open questions

1. **Как вычислять wasted travel?**
   - Вариант A: `actual_distance - nn_distance` (по сравнению с greedy NN)
   - Вариант B: `actual_distance - optimal_distance` (но optimal TSP невычислим быстро)
   - Рекомендуется A для практичности

2. **Как вычислять return reserve?**
   - Вариант A: `final_battery - distance_to_base * drain_rate`
   - Вариант B: `final_battery - BatteryModel::reserve_fraction * 100`
   - Рекомендуется A (физически корректнее)

3. **Нужен ли centralized planner route ordering?**
   - Centralized allocator назначает задачи, но не определяет порядок
   - Добавление route ordering требует изменения `CentralizedPlanner`
   - Рекомендуется опциональное поле `route_planner` в `CentralizedPlanner`

4. **Как интегрировать с SAR?**
   - SAR centralized не поддерживается (static pre-plan incompatible)
   - SAR CBBA может использовать BatteryAwarePlanner с mission-aware scoring
   - Рекомендуется документировать unsupported статусы в тестах

## Что могло сломаться

- **Поведение**: `BatteryAwarePlanner::order` теперь корректно drop задачи. CBBA bundles могут стать короче (fewer tasks per agent), что изменит success rate и coverage.
- **API/контракты**: `BatteryAwarePlanner::is_feasible` теперь использует `battery_model` v2. Агенты без `battery_model` продолжают работать (fallback на `battery_drain_rate`).
- **Данные**: `RunMetrics` теперь содержит non-zero `avg_wasted_travel` и `infeasible_routes`. Старые JSON десериализуются (serde default).
- **Интеграции**: Regression baseline для `avg_route_length` может измениться. Нужно обновить baseline.
- **Производительность**: `BatteryAwarePlanner::order` теперь строит `task_by_id` HashMap. Незначительный overhead.

## Критерии готовности

- [ ] `cargo test --workspace` проходит (включая новые planner tests).
- [ ] `cargo clippy --all-targets -- -D warnings` проходит.
- [ ] `cargo fmt --all` не меняет код.
- [ ] `BatteryAwarePlanner::order` корректно drop задачи из infeasible route.
- [ ] `BatteryAwarePlanner::is_feasible` поддерживает battery model v2.
- [ ] Runner вычисляет non-zero `avg_wasted_travel` и `infeasible_routes`.
- [ ] Regression baseline обновлён.
- [ ] README обновлён (Current Status).
- [ ] Локальный commit сделан.
