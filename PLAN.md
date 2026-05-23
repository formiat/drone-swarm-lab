# PLAN — Stage 4: Infrastructure Inspection (M16)

## Context

Этот план описывает реализацию **Milestone 16** (M16) — reference mission для обследования линейной инфраструктуры (ЛЭП, трубопроводы, периметр). В отличие от предыдущих миссий (coverage по ячейкам, SAR по grid, emergency mesh по ролям), агенты здесь покрывают **граф рёбер** — непрерывные линейные сегменты. Каждое ребро требует физического прохождения агентом от точки `from` к точке `to`. Агентам назначаются наборы рёбер (в TSP-порядке), движение включено (`enable_movement=true`), а метрики отражают долю покрытых рёбер, пропуски, повторные проходы и эффективность маршрута.

**Текущее состояние кодовой базы:**
- Workspace из 9 crate'ов: `swarm-types`, `swarm-comms`, `swarm-sim`, `swarm-runtime`, `swarm-alloc`, `swarm-metrics`, `swarm-replay`, `swarm-scenarios`, `swarm-examples`.
- M1–M15 реализованы. Последняя миссия — SAR v2 (M14) + CBBA Robustness (M15).
- Существует единый benchmark harness (`strategy_comparison`) с поддержкой `--mission coverage|emergency-mesh|sar|all` и `--scenario-suite <json>`.
- Сценарии загружаются через `ScenarioSuite` JSON (Mission DSL v0.12).
- Метрики централизованы в `swarm-metrics` (`RunMetrics`, `AggregateMetrics`).
- Движение агентов реализовано в `MembershipView::apply_movement` (M8); kinematic model включает `speed`, `battery_drain_rate`.
- SAR использует `GridState` в `RunConfig` для отслеживания scan progress; аналогичный подход применим для `InspectionState`.

**Orchestrator docs:** файлы `docs/DRONE_A.7.md`, `docs/DRONE_B.7.md`, `docs/DRONE_B.8.md` в директории оркестратора отсутствуют; требования получены напрямую из inbox (план Stage 4).

## Investigation context

Файл `INVESTIGATION.md` в `workspace_root` отсутствует. Анализ кодовой базы проведён путём чтения ключевых модулей:
- `swarm-types/src/task.rs` — структура `Task` с `grid_cell`, подходит для добавления `edge_id`.
- `swarm-sim/src/runner.rs` — основной цикл `ScenarioRunner`, точка интеграции `InspectionState` и подсчёта метрик.
- `swarm-metrics/src/metrics.rs` — `RunMetrics`/`AggregateMetrics`, требуются новые поля с `#[serde(default)]`.
- `swarm-scenarios/src/sar_scenario.rs`, `emergency_mesh.rs` — паттерны для `build_*_scenario` и профилей.
- `swarm-examples/src/bin/strategy_comparison.rs` — точка интеграции новой миссии в CLI.
- `swarm-sim/src/report_export.rs` — CSV/JSON export, требуется расширение `ReportRow`.

## Affected components

| Crate | Файлы | Изменения |
|-------|-------|-----------|
| `swarm-types` | `src/task.rs`, `src/lib.rs`, новый `src/edge.rs` | `EdgeId`, `InspectionEdge`, `InspectionGraph`, генераторы графов |
| `swarm-scenarios` | `src/inspection.rs`, `src/lib.rs` | `InspectionConfig`, `InspectionProfile`, `build_inspection_scenario` |
| `swarm-metrics` | `src/metrics.rs` | 4 новых поля в `RunMetrics` и `AggregateMetrics` |
| `swarm-sim` | `src/runner.rs`, `src/report_export.rs`, `src/benchmark.rs`, `src/lib.rs` | `InspectionState`, `RunConfig.inspection_state`, логика покрытия рёбер, export, `Display` таблицы |
| `swarm-examples` | `src/bin/strategy_comparison.rs` | `Mission::Inspection`, builder, `--mission inspection` |
| Root | `scenarios/inspection.*.json`, `README.md` | JSON suite'ы, документация |

## Implementation steps

### 1. Типы данных графа рёбер (`swarm-types`)

**Файлы:** `crates/swarm-types/src/edge.rs` (новый), `crates/swarm-types/src/task.rs`, `crates/swarm-types/src/lib.rs`.

1.1. Создать `edge.rs`:
- `EdgeId` — newtype `String` с derive (`Display`, `Serialize`, `Deserialize`, `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash`).
- `InspectionEdge { id: EdgeId, from: Pose, to: Pose, length_m: f64, priority: u8 }`.
- `InspectionGraph { edges: Vec<InspectionEdge>, depot: Pose }`.

1.2. Реализовать генераторы как `impl InspectionGraph`:
- `linear_route(n_segments: u32, segment_length_m: f64) -> Self` — прямая линия вдоль оси X от `(0,0)`; `n_segments` рёбер длины `segment_length_m`.
- `grid_perimeter(width: u32, height: u32, cell_size_m: f64) -> Self` — периметр прямоугольной сетки: 4 стороны, замкнутый контур, `depot` в `(0,0)`.
- `random_graph(n_nodes: u32, seed: u64) -> Self` — случайный геометрический граф: `n_nodes` точек в квадрате `[0,100]×[0,100]`, рёбра между парами узлов с расстоянием `< 30 м`. `depot` = первый узел.

1.3. Расширить `Task` в `task.rs`:
- Добавить `edge_id: Option<EdgeId>` с `#[serde(default, skip_serializing_if = "Option::is_none")]` (аналогично `grid_cell`).
- Обновить unit-тесты `task.rs` (конструктор `task("t")` не сломается благодаря `serde(default)` и named fields).

1.4. Экспортировать новые типы из `swarm-types/src/lib.rs`.

### 2. Сценарий обследования (`swarm-scenarios`)

**Файлы:** `crates/swarm-scenarios/src/inspection.rs` (новый), `crates/swarm-scenarios/src/lib.rs`.

2.1. Создать `inspection.rs`:
- `InspectionConfig { graph: InspectionGraph, agent_count: u32, battery_constraint: f64, require_role: Option<Role>, seed: u64, max_ticks: u64 }`.
- `InspectionProfile` enum: `Linear`, `Perimeter`, `Random`.
- `InspectionStandardProfiles` со списком имён профилей.
- `build_inspection_scenario(cfg: &InspectionConfig) -> (Scenario, RunConfig)`:
  - Создать агентов ( Scout по умолчанию; если `require_role` задан — соответствующая роль ).
  - Позиции агентов: случайные или равномерно распределённые вокруг `depot`.
  - Для каждого `InspectionEdge` создать `Task`:
    - `id = TaskId::from(format!("edge-{}", edge.id))`
    - `pose = Some(edge.to)`
    - `edge_id = Some(edge.id.clone())`
    - `priority = edge.priority`
    - `required_role = cfg.require_role`
  - `Scenario` с `name = "inspection"`.
  - `RunConfig`:
    - `enable_movement = true`
    - `max_ticks = cfg.max_ticks`
    - `inspection_state = Some(InspectionState::new(cfg.graph.clone()))`
    - Если `battery_constraint > 0.0`: установить `battery_drain_rate` у агентов и `max_range` так, чтобы агенты могли исчерпать батарею при длительных маршрутах.

2.2. Экспортировать из `swarm-scenarios/src/lib.rs`.

### 3. Метрики (`swarm-metrics`)

**Файлы:** `crates/swarm-metrics/src/metrics.rs`.

3.1. Добавить в `RunMetrics` (все с `#[serde(default)]`):
- `edge_coverage_rate: f64`
- `missed_edges: u64`
- `revisit_count: u64`
- `route_efficiency: f64`

3.2. Добавить в `AggregateMetrics`:
- `avg_edge_coverage_rate: f64`
- `avg_missed_edges: f64`
- `avg_revisit_count: f64`
- `avg_route_efficiency: f64`

3.3. Обновить `AggregateMetrics::from_runs` — суммирование и усреднение новых полей.

3.4. Обновить `Display for AggregateMetrics` — добавить 4 строки.

3.5. Обновить `mod tests` — конструктор `run(...)` должен инициализировать новые поля нулями.

### 4. Состояние обследования и runner (`swarm-sim`)

**Файлы:** `crates/swarm-sim/src/runner.rs`, `crates/swarm-sim/src/lib.rs`.

4.1. Добавить `InspectionState` в `runner.rs` (или в новый `src/inspection_state.rs`):
```rust
pub struct InspectionState {
    pub graph: InspectionGraph,
    pub covered: HashSet<EdgeId>,
    pub visit_counts: HashMap<EdgeId, u32>,
    pub depot: Pose,
}
```

4.2. Добавить `inspection_state: Option<InspectionState>` в `RunConfig` с `#[serde(default)]`.

4.3. Интегрировать логику покрытия в основной цикл `ScenarioRunner::run_internal`:
- После фазы movement (или в той же фазе, где обновляются позиции), для каждого живого агента:
  - Получить назначенные задачи (`assigned_to == agent_id`).
  - Для задач с `edge_id.is_some()`:
    - Проверить расстояние от позиции агента до `task.pose` (threshold = `1.0` м или `edge.length_m * 0.1`, min `0.5` м).
    - Если агент достиг точки `to`:
      - `visit_counts[edge_id] += 1`.
      - Если `edge_id` уже был в `covered` → `revisit_count += 1`.
      - Иначе → `covered.insert(edge_id)`.
      - Пометить задачу как выполненную / освободить (`release_task`), чтобы агент мог получить следующее ребро.

4.4. В конце прогона вычислить:
- `edge_coverage_rate = covered.len() as f64 / total_edges as f64`
- `missed_edges = total_edges - covered.len()`
- `route_efficiency = if total_distance_travelled > 0.0 { sum_covered_edge_lengths / total_distance_travelled } else { 0.0 }`

4.5. Условие завершения миссии (`break` в цикле):
- Добавить `inspection_complete`: если `inspection_state` задан и все рёбра покрыты (`covered.len() == total_edges`), то миссия считается завершённой (наряду с `all_tasks_assigned` и другими условиями).

4.6. Экспортировать `InspectionState` из `swarm-sim/src/lib.rs` (чтобы `swarm-scenarios` мог создавать его).

### 5. Export и benchmark таблица (`swarm-sim`)

**Файлы:** `crates/swarm-sim/src/report_export.rs`, `crates/swarm-sim/src/benchmark.rs`.

5.1. В `report_export.rs`:
- Добавить 4 поля в `ReportRow`.
- Добавить заголовки CSV и заполнение JSON/CSV.

5.2. В `benchmark.rs`:
- Обновить `ComparisonReport` `Display` — добавить 4 колонки в markdown-таблицу: `EdgeCoverage`, `MissedEdges`, `Revisits`, `RouteEfficiency`.
- *Примечание:* таблица станет шире; это приемлемый tradeoff для единообразия. Альтернатива — вынести inspection-метрики в отдельную секцию, но это усложняет `Display`.

### 6. Интеграция в benchmark CLI (`swarm-examples`)

**Файлы:** `crates/swarm-examples/src/bin/strategy_comparison.rs`.

6.1. Добавить `Mission::Inspection` в enum.

6.2. Обновить `parse_mission` и `mission_name`.

6.3. В `main()`, в ветке `match mission`, добавить `Mission::Inspection`:
- `profile_names` = `InspectionStandardProfiles::profile_names()`.
- `builder` — замыкание, вызывающее `build_inspection_scenario` с профилем.
- `mission_options.mission_name = "inspection"`.

6.4. В `run_from_suite` новая миссия поддерживается автоматически через `entry.mission` строку, но убедиться, что `mission_names` корректно прокидываются в отчёт.

### 7. JSON scenario suite'ы

**Файлы:** `scenarios/inspection.linear.json`, `scenarios/inspection.perimeter.json`, `scenarios/inspection.random.json`.

7.1. `inspection.linear.json`:
- `name`: "Inspection Linear"
- 1 scenario: `mission = "inspection"`, `profile = "linear"`
- `InspectionGraph::linear_route(10, 10.0)` → 10 рёбер по 10 м, 3 агента.
- `max_ticks: 500`, `enable_movement: true`.

7.2. `inspection.perimeter.json`:
- `name`: "Inspection Perimeter"
- `profile = "perimeter"`
- `grid_perimeter(10, 10, 10.0)` → периметр 10×10 м, 4 агента.
- `battery_constraint: 0.3` (30% от полного заряда — агенты должны успеть до исчерпания).

7.3. `inspection.random.json`:
- `name`: "Inspection Random"
- `profile = "random"`
- `random_graph(15, seed)` → случайный граф, 5 агентов.

### 8. Тесты

#### 8.1 Unit tests (Category 1 — без рефакторинга)

**`swarm-types/src/edge.rs` (новый модуль):**
- `linear_route_n_edges` — `graph.edges.len() == n_segments`.
- `linear_route_total_length` — сумма `length_m` == `n_segments * segment_length_m`.
- `grid_perimeter_closed` — последнее ребро заканчивается в `depot`.
- `random_graph_no_panic` — при `n_nodes` от 2 до 50 не паникует; `edges` не пусты.
- `grid_perimeter_count` — для `w×h` периметр содержит `2*(w+h)` рёбер.

**`swarm-scenarios/src/inspection.rs`:**
- `build_inspection_scenario_tasks_match_edges` — `scenario.tasks.len() == config.graph.edges.len()`.
- `build_inspection_scenario_edge_id_set` — все `task.edge_id` уникальны и не `None`.

#### 8.2 Integration tests (Category 2 — лёгкий рефакторинг сценария)

**`crates/swarm-sim/tests/` или в `runner.rs` `mod tests`:**
- `inspection_linear_3_agents_coverage_above_90`:
  - Создать `InspectionConfig` с `linear_route(10, 10.0)`, 3 агента.
  - Запустить `ScenarioRunner::run`.
  - `assert!(metrics.edge_coverage_rate > 0.9)`.
  - `assert!(metrics.success)`.

- `inspection_perimeter_battery_constraint_no_exhaustion`:
  - `grid_perimeter(10, 10, 10.0)`, 4 агента, `battery_constraint=0.3`.
  - Запустить.
  - `assert_eq!(metrics.agents_exhausted, 0)`.
  - `assert!(metrics.edge_coverage_rate > 0.8)`.

**`crates/swarm-examples/src/bin/strategy_comparison.rs` (run via cargo test интеграционно):**
- Добавить тестовый benchmark run для `Mission::Inspection` с 10 seeds, проверить что `ComparisonReport` содержит ключи для inspection профилей.

#### 8.3 Property-based tests (Category 3 — тяжёлый, но изолированный)

**`crates/swarm-scenarios/tests/proptest_generators.rs` (или новый файл):**
- `proptest: random_inspection_graph_and_agent_count`:
  - Стратегия: `n_nodes in 2..30u32`, `agent_count in 1..10u32`, `seed in any::<u64>()`.
  - Создать `InspectionGraph::random_graph(n_nodes, seed)`.
  - Создать `InspectionConfig`, запустить `ScenarioRunner::run`.
  - Invariants:
    - не паникует;
    - `metrics.edge_coverage_rate` в `[0.0, 1.0]`;
    - `metrics.missed_edges + covered_edges == total_edges` (консистентность).
  - Запускать с `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1`.

### 9. Актуализация README

**Файл:** `README.md`.

9.1. Добавить раздел **Milestone 16 — Infrastructure Inspection** после M15:
- Описание: edge coverage mission, граф рёбер, TSP-упорядоченные назначения, battery-aware约束.
- Команды запуска:
  ```bash
  cargo run -p swarm-examples --bin strategy_comparison --mission inspection
  cargo run -p swarm-examples --bin strategy_comparison --scenario-suite scenarios/inspection.linear.json --json inspection.json
  ```
- Benchmark-таблица (markdown) с примером результатов (10 seeds, quick mode) — добавить после существующих таблиц.

9.2. Обновить список mission'ов в разделе "Run Examples" и "Workspace Layout" (добавить `inspection` в описание `swarm-scenarios`).

## Testing strategy

| Категория | Что тестируем | Где | Как запускать |
|-----------|---------------|-----|---------------|
| 1 (unit, no refactor) | Генераторы графов, конструктор сценария | `swarm-types`, `swarm-scenarios` | `cargo test -p swarm-types`, `cargo test -p swarm-scenarios` |
| 1 (unit, no refactor) | Метрики — сериализация / агрегация | `swarm-metrics` | `cargo test -p swarm-metrics` |
| 2 (integration, light) | Linear scenario coverage > 0.9 | `swarm-sim` | `cargo test -p swarm-sim inspection_linear` |
| 2 (integration, light) | Perimeter + battery, no exhaustion | `swarm-sim` | `cargo test -p swarm-sim inspection_perimeter` |
| 2 (integration, light) | CLI benchmark включает inspection | `swarm-examples` | `cargo test -p swarm-examples --bin strategy_comparison` (или интеграционный тест) |
| 3 (proptest, heavy) | Случайный граф + случайное число агентов | `swarm-scenarios` или `swarm-sim` | `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-scenarios proptest_inspection` |
| 2 (integration, light) | JSON suite roundtrip | `swarm-sim` | `cargo test -p swarm-sim load_inspection_suite` |

**Gaps (явно зафиксированные):**
- TSP-оптимизация порядка рёбер в bundle не покрывается автотестом как отдельный алгоритм, потому что в текущей архитектуре агенты получают задачи по одной через стандартный allocator. TSP-order эмулируется приоритетами/позициями. Если в будущем появится `EdgeBundleAllocator`, для него потребуется отдельный unit test.
- Визуальная валидация маршрутов (plots) невозможна в headless CI; покрывается метрикой `route_efficiency`.

## Risks and tradeoffs

### Ширина markdown-таблицы benchmark
Добавление 4 колонок в `ComparisonReport` Display сделает таблицу очень широкой (>140 символов). Это не ломает функциональность, но снижает читаемость в терминале. Tradeoff: единообразие всех метрик в одной таблице vs. читаемость. Решение — добавить колонки; JSON/CSV остаются основным машиночитаемым форматом.

### Производительность runner
Проверка покрытия рёбер на каждом тике добавляет `O(agents × edges_per_agent)` операций. При типичных значениях (`agents ≤ 10`, `edges ≤ 50`) это несущественно. Если графы станут большими (>>1000 рёбер), потребуется spatial index. На данном этапе — не нужен.

### Совместимость JSON сценариев
Добавление `edge_id` в `Task` и `inspection_state` в `RunConfig` использует `#[serde(default)]`, поэтому старые JSON-сценарии без этих полей продолжают десериализоваться корректно.

### Battery constraint = 0.0
Требуется корректная интерпретация: `0.0 = без ограничений`. В `build_inspection_scenario` при `battery_constraint == 0.0` агенты получают `battery_drain_rate: 0.0` и `max_range: f64::INFINITY` (или достаточно большое значение). При `> 0.0` — `battery_drain_rate` пропорционален constraint.

## Что могло сломаться

| Риск | Компонент | Проверка |
|------|-----------|----------|
| **Десериализация старых `Task` JSON** — добавление `edge_id` ломает `serde` для ручно-созданных JSON без default | `swarm-types` | `cargo test -p swarm-sim` (есть тест `load_coverage_example_scenario`) + запуск `strategy_comparison --scenario-suite scenarios/coverage.ideal.json` |
| **Десериализация старых `RunConfig` JSON** — добавление `inspection_state` | `swarm-sim` | Тест `run_config_json_defaults_work` + `load_scenario_suite` для существующих JSON |
| **Нарушение инварианта `all_tasks_assigned`** — inspection задачи освобождаются (`release_task`) при покрытии ребра, что может привести к ложному `all_tasks_assigned=false` в конце прогона | `swarm-sim` | Убедиться, что `release_task` корректно переводит статус в `Completed`, или настроить `all_tasks_assigned` проверку на учёт `Completed` задач. `runner.rs` уже использует `all_assigned_or_completed()`. Проверить интеграционным тестом `inspection_linear_3_agents_coverage_above_90`. |
| **Регрессия в `report_export` / CSV** — новые поля добавлены, но старые отчёты без них не импортируются обратно | `swarm-sim` | CSV export — добавление колонок в конец таблицы не ломает парсинг по именам. JSON export — `#[serde(default)]` на читающей стороне решит проблему. Проверить: запустить старый `coverage` benchmark и убедиться, что CSV/JSON экспорт без ошибок. |
| **Регрессия в `AggregateMetrics::from_runs`** — поля инициализируются нулями в пустом runs | `swarm-metrics` | Unit test `aggregate_sar_fields_empty` уже покрывает пустой вектор; добавить аналогичный для inspection полей. |
| **Battery exhaustion при `enable_movement=true`** — агенты inspection mission могут исчерпать батарею раньше, чем в других миссиях, из-за длинных маршрутов | `swarm-scenarios` | Интеграционный тест `inspection_perimeter_battery_constraint_no_exhaustion` + проверка `agents_exhausted == 0`. |
| **Конфликт `inspection_state` с `grid_state`** — оба поля в `RunConfig` опциональны, но если по ошибке заданы оба, runner должен обрабатывать только один | `swarm-sim` | В `run_internal` использовать `if let Some(ref mut grid_state) = grid_state` и отдельно `if let Some(ref mut inspection_state) = inspection_state`. Они независимы. Добавить debug assertion или лог, если оба заданы. |
| **Производительность `random_graph`** при `n_nodes > 100` — квадратичная сложность генерации рёбер | `swarm-types` | Ограничить `n_nodes` в proptest до 50; задокументировать в коде. |

## Open questions

1. **TSP-order vs. existing allocators:** Требуется ли создавать отдельный `EdgeBundleAllocator`, или достаточно эмулировать TSP через приоритеты задач? В плане выбран подход «каждое ребро = отдельная Task» с последовательным назначением через существующие allocators. Если ревью покажет, что нужен явный bundle allocator, потребуется дополнительный шаг в `swarm-alloc`.

2. **Threshold для покрытия ребра:** Фиксированный `1.0` м или пропорциональный длине? В плане используется `max(1.0, length_m * 0.1)`. Нужно ли сделать это конфигурируемым через `InspectionConfig`?

3. **Замыкание периметра (`grid_perimeter`):** Должно ли последнее ребро возвращаться в `depot` (замкнутый контур)? В плане — да, для согласованности с реальным периметром.

4. **Как интерпретировать `battery_constraint` численно:** `0.3` = 30% от `max_range` или `battery_drain_rate` увеличен в 3×? В плане выбран подход: `battery_constraint` масштабирует `max_range` и `battery_drain_rate` так, чтобы полный маршрут превышал запас хода. Конкретная формула будет уточнена при реализации (`max_range = battery_constraint * estimated_total_route_length / agent_count`).
