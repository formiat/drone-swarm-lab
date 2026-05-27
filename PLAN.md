# M33 — Mission Semantics Integration

## Context

M32 и M32b закрыты. Benchmark output теперь корректно экспортирует per-row mission/scenario, mission-scoped profiles и merged `all` benchmark id.

Следующий шаг по линейному плану DRONE_A.14.linear.md — M33 Mission Semantics Integration.

## Investigation context

`INVESTIGATION.md` отсутствует. Анализ кода показал:

- `MissionAdapter` trait (`crates/swarm-types/src/mission.rs:30-42`) определён с 4 методами:
  - `task_kind(&self, task: &Task) -> TaskKind`
  - `route_cost(&self, from: Pose, task: &Task) -> f64`
  - `is_completed(&self, task: &Task, state: &RunState) -> bool`
  - `score(&self, agent: &AllocationAgent, task: &Task) -> f64`
- `RunState` (`crates/swarm-types/src/mission.rs:13-23`) — структура с `scanned_cells`, `covered_edges`, `completed_tasks`, `mapped_zones`
- `TaskKind` (`crates/swarm-types/src/task.rs:39-47`) — enum с 7 вариантами: `CoverageCell`, `SarScan`, `SarConfirmationScan`, `InspectionEdge`, `RelayPlacement`, `Waypoint`, `MappingZone`
- `Allocator::allocate_with_adapter` (`crates/swarm-alloc/src/allocator.rs:50-57`) — default implementation просто делегирует в `allocate`, не использует adapter
- Большинство задач в сценариях имеют `kind: None`. Исключение: wildfire (`TaskKind::MappingZone`)
- Runner (`crates/swarm-sim/src/runner.rs`) использует `TaskKind::MappingZone` напрямую для wildfire-логики, не через adapter
- DSL validation (`crates/swarm-sim/src/dsl.rs`) не проверяет `task.kind` и не требует mission-specific fields (`grid_cell`, `edge_id`, `pose`)

## Affected components

| Компонент | Путь | Что меняется |
|---|---|---|
| MissionAdapter trait | `crates/swarm-types/src/mission.rs` | Возможно расширение trait (validation hook) |
| Adapters | `crates/swarm-types/src/adapters.rs` (new) | Concrete adapters |
| Task builders | `crates/swarm-scenarios/src/*` | Установка `kind` в задачи |
| Allocator | `crates/swarm-alloc/src/allocator.rs` | Использование adapter в `allocate_with_adapter` |
| Runner | `crates/swarm-sim/src/runner.rs` | Adapter-driven completion, scoring, route_cost |
| DSL validation | `crates/swarm-sim/src/dsl.rs` | Валидация task kind и required fields |
| README | `README.md` | Обновление Current Status |

## Implementation steps

### 1. Создать concrete adapters

Файл: `crates/swarm-types/src/adapters.rs` (new module)

Реализовать 4 обязательных adapter (по критериям готовности):

```rust
pub struct CoverageAdapter;
pub struct SarAdapter;
pub struct InspectionAdapter;
pub struct WildfireAdapter;
```

Каждый реализует `MissionAdapter`:

**CoverageAdapter:**
- `task_kind` → `TaskKind::CoverageCell`
- `is_completed` → task.id есть в `state.completed_tasks`
- `route_cost` → Euclidean distance до `task.pose` (или 0.0 если pose отсутствует)
- `score` → базовый score с учётом distance, battery, role (как в AuctionAllocator)

**SarAdapter:**
- `task_kind` → `TaskKind::SarScan` (или `SarConfirmationScan` если `task.grid_cell` уже scanned)
- `is_completed` → `task.grid_cell` есть в `state.scanned_cells`
- `route_cost` → distance до `task.pose`
- `score` → entropy-based priority bonus (higher entropy → better score)

**InspectionAdapter:**
- `task_kind` → `TaskKind::InspectionEdge`
- `is_completed` → `task.edge_id` есть в `state.covered_edges`
- `route_cost` → distance до `task.pose`
- `score` → distance + edge coverage priority

**WildfireAdapter:**
- `task_kind` → `TaskKind::MappingZone`
- `is_completed` → `task.id.to_string()` есть в `state.mapped_zones` (или по zone name)
- `route_cost` → distance до `task.pose`
- `score` → threat level priority (higher threat → better score)

Опциональные adapters (`RelayAdapter`, `WaypointAdapter`) — если время позволяет.

### 2. Обновить task builders в scenarios

Файлы:
- `crates/swarm-scenarios/src/coverage.rs` — `kind: Some(TaskKind::CoverageCell)`
- `crates/swarm-scenarios/src/sar_scenario.rs` — `kind: Some(TaskKind::SarScan)`
- `crates/swarm-scenarios/src/inspection.rs` — `kind: Some(TaskKind::InspectionEdge)`
- `crates/swarm-scenarios/src/wildfire.rs` — уже `MappingZone`, оставить
- `crates/swarm-scenarios/src/emergency_mesh.rs` — `kind: Some(TaskKind::RelayPlacement)`

### 3. Провести adapter path через runner

Файл: `crates/swarm-sim/src/runner.rs`

Текущий runner строит `RunState` неявно (через `GridState`, `InspectionState`, `WildfireState`). Нужно:

1. **Build RunState** — конвертировать runtime state в `RunState` перед adapter call
2. **Call `is_completed`** — заменить текущие проверки completion на adapter-driven:
   - SAR: вместо `grid_state.scanned_cells.contains(...)` → `adapter.is_completed(task, &run_state)`
   - Inspection: вместо прямой проверки edge coverage → adapter
   - Wildfire: уже есть логика, но переделать через adapter
3. **Call `route_cost`** — использовать в allocator для mission-aware distance cost
4. **Use `score`** — в `allocate_with_adapter` для mission-aware assignment priority

### 4. Обновить allocator для adapter-driven allocation

Файл: `crates/swarm-alloc/src/allocator.rs`

Обновить `GreedyAllocator.allocate_with_adapter`:
- Группировать задачи по `adapter.task_kind()`
- Для каждой группы применять `adapter.score()` при выборе агента
- Fallback на базовый `allocate` для задач без kind или без adapter

Обновить `AuctionAllocator.allocate_with_adapter`:
- Использовать `adapter.route_cost()` вместо Euclidean distance
- Применять `adapter.score()` как бонус к cost function

### 5. Обновить DSL validation

Файл: `crates/swarm-sim/src/dsl.rs`

Добавить validation rules:
- `TaskKind::SarScan`/`SarConfirmationScan` → требовать `grid_cell`
- `TaskKind::InspectionEdge` → требовать `edge_id`
- `TaskKind::CoverageCell`/`Waypoint`/`RelayPlacement`/`MappingZone` → требовать `pose`
- Legacy scenarios без `kind` → оставить compatible (no validation error)

### 6. Обновить README

Файл: `README.md`
- Добавить Mission Semantics Integration в Current Status (M33 in work)
- Обновить Known Limitations: убрать пункт про adapter integration

## Testing strategy

### Категория 1 — без рефакторинга

- **Unit test для каждого adapter's `task_kind`**:
  - `CoverageAdapter::task_kind` возвращает `CoverageCell`
  - `SarAdapter::task_kind` возвращает `SarScan`
  - `InspectionAdapter::task_kind` возвращает `InspectionEdge`
  - `WildfireAdapter::task_kind` возвращает `MappingZone`
- **Unit test для adapter completion**:
  - `SarAdapter::is_completed` → true когда `grid_cell` в `scanned_cells`
  - `InspectionAdapter::is_completed` → true когда `edge_id` в `covered_edges`
  - `WildfireAdapter::is_completed` → true когда zone в `mapped_zones`
- **Validation tests**:
  - SAR task без `grid_cell` → validation error
  - Inspection task без `edge_id` → validation error
- **Serialization tests**:
  - `TaskKind` round-trip через serde_json

### Категория 2 — лёгкий рефакторинг

- **Shared task builders by kind**: helper `fn task_of_kind(kind: TaskKind) -> Task`
- **Reusable RunState fixtures**: `fn run_state_with_scanned(cells) -> RunState`
- **Mission lifecycle helpers**: `fn run_mission_with_adapter(adapter, tasks, agents) -> RunMetrics`

### Категория 3 — тяжёлый рефакторинг

- **Full pipeline test**: DSL → adapter → allocation → runner → report
- **Property test**: для любого `TaskKind` валидная задача проходит validation
- **Compatibility suite**: legacy scenarios без `kind` продолжают работать

## Risks and tradeoffs

| Риск | Вероятность | Влияние | Митигация |
|---|---|---|---|
| Adapter trait слишком generic | Средняя | Среднее | Начать с 4 обязательных adapters, расширять по мере необходимости |
| Runner refactoring сложный | Высокая | Высокое | Сохранить текущий completion path как fallback; adapter path — opt-in через `task.kind` |
| Scenarios с `kind: None` сломаются | Средняя | Среднее | Default adapter (generic coverage) для задач без kind; legacy compatibility mode |
| Performance regression | Низкая | Среднее | Adapter lookup через HashMap<TaskKind, Box<dyn MissionAdapter>>; кеширование при необходимости |
| Allocator API изменение | Средняя | Среднее | `allocate_with_adapter` уже существует; изменить только default implementation |

## Open questions

1. **Как выбирать adapter для mixed-mission runs?**
   - Вариант A: один adapter per mission, runner передаёт правильный adapter
   - Вариант B: registry/map adapters по `TaskKind`, runner выбирает автоматически
   - Рекомендуется B для `--mission all`

2. **Как обрабатывать задачи без `kind`?**
   - Вариант A: treat как `CoverageCell` (default)
   - Вариант B: skip adapter path, use legacy allocation
   - Рекомендуется B для backward compatibility

3. **Нужен ли `RelayAdapter` и `WaypointAdapter` сейчас?**
   - Relay используется в emergency-mesh, но allocator уже обрабатывает relay через `required_role`
   - Waypoint используется в SITL, но там специфическая логика
   - Рекомендуется отложить до M34+ если не критично для критериев готовности

4. **Как интегрировать adapter scoring с CBBA?**
   - CBBA использует `score` метод в bundle construction
   - `SarAdapter::score` может conflict с CBBA's internal scoring
   - Рекомендуется adapter scoring как optional override

## Что могло сломаться

- **Поведение**: tasks без `kind` теперь используют legacy path вместо adapter path. Если legacy path меняется (например, `allocate_with_adapter` больше не делегирует в `allocate`), старые сценарии могут сломаться.
- **API/контракты**: `MissionAdapter` trait может потребовать новых методов (например, `validate_task`). Это сломает любые external реализации trait.
- **Данные**: scenario JSON файлы без `kind` полей должны десериализоваться с `kind: None` (благодаря `#[serde(default)]`). Это работает.
- **Интеграции**: `runner.rs` wildfire-логика переносится в `WildfireAdapter`. Если adapter не используется, wildfire completion может сломаться.
- **Производительность**: adapter lookup на каждый tick может добавить overhead. Митигация: кешировать adapter registry.

## Критерии готовности

- [ ] `cargo test --workspace` проходит (включая новые adapter tests).
- [ ] `cargo clippy --all-targets -- -D warnings` проходит.
- [ ] `cargo fmt --all` не меняет код.
- [ ] Есть хотя бы 4 concrete adapters: Coverage, SAR, Inspection, Wildfire.
- [ ] Runner использует adapters для completion checks.
- [ ] DSL validation ловит missing `grid_cell` / `edge_id` / `pose` для соответствующих kinds.
- [ ] Legacy scenarios без `kind` остаются compatible.
- [ ] README обновлён (Current Status, Known Limitations).
- [ ] Локальный commit сделан.
