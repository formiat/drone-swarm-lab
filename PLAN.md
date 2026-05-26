# PLAN: M27 — Mission Semantics Layer

## Контекст

M27 вводит явный семантический слой задач вместо generic allocation tasks. Сейчас аллокаторы
(CBBA, Greedy, Auction) принимают `Task` без понимания её типа — scoring и маршрутная стоимость
захардкожены в самих аллокаторах и не учитывают семантику задачи (SAR-scan, inspection, coverage).
Это корень части алгоритмических проблем M25: CBBA не отличает ячейку SAR-сетки от Inspection-ребра
при построении bundle.

M27 исправляет это: вводит `TaskKind` enum, трейт `MissionAdapter`, валидацию в DSL loader и
обновление всех builderов сценариев.

## Investigation Context

`INVESTIGATION.md` отсутствует. Ниже — ключевые наблюдения из инспекции кода.

**Текущее состояние `Task`** (`crates/swarm-types/src/task.rs`):

```
Task { id, status, assigned_to, priority, required_capabilities, required_role,
       preferred_role, expires_at, pose, grid_cell, edge_id }
```

Поле `kind` отсутствует. Тип задачи неявно выводится по наличию `grid_cell` / `edge_id` / `pose`.

**Текущий scoring:**

- `CbbaAllocator::marginal_score` (`swarm-alloc/src/cbba.rs:130`) —
  `−distance * weight + battery * weight − position_penalty`; нет учёта типа задачи.
- `AuctionAllocator::cost` (`swarm-alloc/src/allocator.rs`) —
  аналогично: distance + battery + role_bonus; тип задачи не учитывается.

**DSL loader** (`swarm-sim/src/dsl.rs`) уже содержит `validate_mission_specific`, которая
проверяет наличие `grid_cell` у SAR-задач по строковому полю `entry.mission`.
После M27 валидация должна работать через `task.kind`, а не только через `entry.mission`.

**Scenario builders** создают задачи без явного `kind`:

- `build_sar_scenario` (`swarm-scenarios/src/sar_scenario.rs:160`) — `grid_cell = Some((x, y))`
- `build_inspection_scenario` (`swarm-scenarios/src/inspection.rs:71`) — `edge_id = Some(...)`
- `build_coverage_scenario` (`swarm-scenarios/src/coverage.rs:16`) — `pose = None, grid_cell = None`

**Тип `RunState`**: в кодовой базе отсутствует. `GridState` (`swarm-runtime/src/grid_state.rs`)
и `InspectionState` (`swarm-sim/src/runner.rs`) хранят состояние выполнения миссий отдельно.
Для трейта `MissionAdapter::is_completed` потребуется ввести облегчённый `RunState` в `swarm-types`,
агрегирующий только ту информацию, которая нужна для check completion.

**Тип `AllocationAgent`** (`swarm-alloc/src/allocator.rs`): подмножество `Agent`
(id, pose, battery, capabilities, role, comms_range). Сигнатура `MissionAdapter::score`
должна принимать `&AllocationAgent`, а не `&Agent`, — только этот тип доступен аллокаторам.

## Affected Components

| Компонент | Файл | Тип изменения |
|---|---|---|
| `swarm-types` | `src/task.rs` | добавить `TaskKind` enum, поле `kind` в `Task` |
| `swarm-types` | `src/mission.rs` (новый) | трейт `MissionAdapter`, тип `RunState` |
| `swarm-types` | `src/lib.rs` | re-export нового модуля |
| `swarm-scenarios` | `src/adapter.rs` (новый) | 4 реализации `MissionAdapter` |
| `swarm-scenarios` | `src/lib.rs` | re-export адаптеров |
| `swarm-scenarios` | `src/sar_scenario.rs` | `kind: Some(TaskKind::SarScan)` в задачах |
| `swarm-scenarios` | `src/inspection.rs` | `kind: Some(TaskKind::InspectionEdge)` |
| `swarm-scenarios` | `src/coverage.rs` | `kind: Some(TaskKind::CoverageCell)` |
| `swarm-alloc` | `src/cbba.rs` | опциональное делегирование scoring через адаптер |
| `swarm-alloc` | `src/allocator.rs` | опциональное делегирование cost через адаптер |
| `swarm-sim` | `src/dsl.rs` | валидация `TaskKind` vs полей задачи |
| `README.md` | — | актуализация архитектурного описания |

## Implementation Steps

### Шаг 1: `TaskKind` enum в `swarm-types`

**Файл:** `crates/swarm-types/src/task.rs`

Добавить enum перед `struct Task`:

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
}
```

Добавить поле в `Task` (после `edge_id`):

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub kind: Option<TaskKind>,
```

Backward compat обеспечивается через `#[serde(default)]`: старые JSON без поля `kind`
десериализуются с `kind = None`.

### Шаг 2: `RunState` и `MissionAdapter` в `swarm-types`

**Файл:** `crates/swarm-types/src/mission.rs` (новый)

`RunState` — облегчённая агрегация состояния выполнения, не зависящая от `swarm-runtime`/`swarm-sim`:

```rust
pub struct RunState {
    /// Ячейки, которые были просканированы (SAR).
    pub scanned_cells: HashSet<(u32, u32)>,
    /// Рёбра, которые были покрыты (Inspection).
    pub covered_edges: HashSet<EdgeId>,
    /// Задачи, помеченные как завершённые (Coverage, Waypoint).
    pub completed_tasks: HashSet<TaskId>,
}
```

Трейт `MissionAdapter` (подпись `score` принимает `&AllocationAgent`, не `&Agent`
— это тип, доступный аллокаторам):

```rust
pub trait MissionAdapter: Send + Sync {
    fn task_kind(&self, task: &Task) -> TaskKind;
    fn route_cost(&self, from: Pose, task: &Task) -> f64;
    fn is_completed(&self, task: &Task, state: &RunState) -> bool;
    fn score(&self, agent: &AllocationAgent, task: &Task) -> f64;
}
```

**Файл:** `crates/swarm-types/src/lib.rs`  
Добавить `pub mod mission;` и re-exports `MissionAdapter`, `RunState`.

`swarm-alloc` уже зависит от `swarm-types` → трейт доступен аллокаторам без новых зависимостей.

### Шаг 3: Реализации адаптеров в `swarm-scenarios`

**Файл:** `crates/swarm-scenarios/src/adapter.rs` (новый)

**`CoverageAdapter`:**

- `task_kind` → `TaskKind::CoverageCell`
- `route_cost(from, task)` → euclidean distance от `from` до `task.pose.unwrap_or_default()`
- `is_completed(task, state)` → `state.completed_tasks.contains(&task.id)`
- `score(agent, task)` → `-distance + agent.battery * 0.01`

**`SarAdapter`:**

- `task_kind(task)` → по `task.kind` если задан, иначе `TaskKind::SarScan`
- `route_cost(from, task)` → euclidean distance до cell center
  (из `task.grid_cell`, cell_size_m — поле конфига адаптера)
- `is_completed(task, state)` → `state.scanned_cells.contains(&cell)`
- `score(agent, task)` → приоритет задачи + бонус для Scout/Thermal роли

**`InspectionAdapter`:**

- `task_kind` → `TaskKind::InspectionEdge`
- `route_cost(from, task)` → расстояние до ближайшей точки ребра
  (из `task.edge_id` + словарь рёбер в адаптере)
- `is_completed(task, state)` → `state.covered_edges.contains(&edge_id)`
- `score(agent, task)` → бонус для Inspector роли; штраф при низком battery

**`WaypointAdapter`:**

- `task_kind` → `TaskKind::Waypoint`
- `route_cost(from, task)` → euclidean distance до `task.pose`
- `is_completed(task, state)` → `state.completed_tasks.contains(&task.id)`
- `score(agent, task)` → `-distance`

**Файл:** `crates/swarm-scenarios/src/lib.rs`  
Добавить `pub mod adapter;` и re-exports.

### Шаг 4: Делегирование scoring в аллокаторах

**Файл:** `crates/swarm-alloc/src/allocator.rs`

Добавить в трейт `Allocator` метод с дефолтной реализацией (не ломает существующие impl):

```rust
fn allocate_with_adapter(
    &mut self,
    tasks: &[AllocationTask<'_>],
    agents: &[AllocationAgent],
    adapter: &dyn MissionAdapter,
) -> Vec<(TaskId, AgentId)> {
    self.allocate(tasks, agents)
}
```

Добавить поле в `AuctionAllocator`:

```rust
pub mission_adapter: Option<Box<dyn MissionAdapter + Send + Sync>>,
```

Обновить `AuctionAllocator::cost`: если `self.mission_adapter` задан →
использовать `adapter.route_cost(agent.pose, task)` и `adapter.score(agent, task)`;
иначе — старая логика (backward compat).

**Файл:** `crates/swarm-alloc/src/cbba.rs`

Добавить поле в `CbbaAllocator`:

```rust
pub mission_adapter: Option<Box<dyn MissionAdapter + Send + Sync>>,
```

Обновить `CbbaAllocator::marginal_score`: если `self.mission_adapter` задан →
использовать `adapter.route_cost` и `adapter.score` как базу;
иначе — старая логика.

### Шаг 5: Валидация `TaskKind` в DSL loader

**Файл:** `crates/swarm-sim/src/dsl.rs`

В `validate_mission_specific` (и/или отдельную функцию `validate_task_kind_fields`)
добавить проверки для каждой задачи с ненулевым `kind`:

- `TaskKind::SarScan | SarConfirmationScan` → `task.grid_cell.is_some()` обязательно
- `TaskKind::InspectionEdge` → `task.edge_id.is_some()` обязательно
- `TaskKind::Waypoint` → `task.pose.is_some()` обязательно
- `kind = None` → проверка пропускается (legacy compat)

Ошибки добавляются в `Vec<ValidationError>` с полем `"task.kind"` или `"task.grid_cell"`.

### Шаг 6: Обновление scenario builders

**`crates/swarm-scenarios/src/sar_scenario.rs`** — `build_sar_scenario`:

```rust
kind: Some(TaskKind::SarScan),
```

При наличии confirmation-scan tasks (если добавятся в будущем):
`kind: Some(TaskKind::SarConfirmationScan)`.

**`crates/swarm-scenarios/src/inspection.rs`** — `build_inspection_scenario`:

```rust
kind: Some(TaskKind::InspectionEdge),
```

**`crates/swarm-scenarios/src/coverage.rs`** — `build_coverage_scenario`:

```rust
kind: Some(TaskKind::CoverageCell),
```

### Шаг 7: Актуализация README.md

В `README.md`:

- Добавить раздел «Mission Semantics» с описанием `TaskKind` и `MissionAdapter`
- Описать четыре адаптера и правило `kind → required fields`
- Обновить список компонентов (`swarm-scenarios` — теперь содержит `adapter.rs`)
- Добавить пример: `AuctionAllocator` с `SarAdapter`

## Testing Strategy

### Категория 1 — без рефакторинга

Реализовать вместе с основными изменениями:

1. **`TaskKind` serde roundtrip** (`swarm-types/src/task.rs`):
   - Все шесть вариантов `TaskKind` сериализуются в snake_case и десериализуются обратно.
   - `Task` с `kind: None` (поле отсутствует в JSON) десериализуется без ошибок.
   - `Task` с `kind: Some(SarScan)` roundtrip корректен.

2. **Unit тесты адаптеров** (`swarm-scenarios/src/adapter.rs`):
   - `CoverageAdapter::route_cost` — корректный euclidean distance.
   - `SarAdapter::route_cost` — корректное расстояние до cell center.
   - `InspectionAdapter::route_cost` — ≥ 0 для произвольного ребра.
   - `WaypointAdapter::route_cost` — корректный euclidean distance.
   - `SarAdapter::is_completed` — true когда cell в `scanned_cells`.
   - `InspectionAdapter::is_completed` — true когда edge в `covered_edges`.
   - `CoverageAdapter::is_completed` — true когда task в `completed_tasks`.
   - Все `score` методы возвращают finite value для valid inputs.
   - `task_kind` каждого адаптера возвращает ожидаемый вариант.

3. **DSL validation** (`swarm-sim/src/dsl.rs`):
   - `SarScan` task без `grid_cell` → `ValidationError`.
   - `InspectionEdge` task без `edge_id` → `ValidationError`.
   - `Waypoint` task без `pose` → `ValidationError`.
   - Task с `kind: None` (legacy) → нет ошибки при валидации.
   - Task с корректными полями → нет ошибки.

4. **Scenario builder тесты** (`swarm-scenarios`):
   - `build_sar_scenario` → все задачи имеют `kind = Some(SarScan)`.
   - `build_inspection_scenario` → все задачи имеют `kind = Some(InspectionEdge)`.
   - `build_coverage_scenario` → все задачи имеют `kind = Some(CoverageCell)`.

5. **Allocator backward compat** (`swarm-alloc`):
   - `CbbaAllocator` без адаптера (`mission_adapter = None`) даёт идентичный результат
     существующим тестам (marginal_score не изменился для None-пути).
   - `AuctionAllocator` без адаптера — аналогично.

### Категория 2 — лёгкий рефакторинг

Требуют незначительного обновления существующих тестов или фикстур:

6. **Параметрический тест адаптеров**:
   - Все четыре адаптера на наборе заранее заданных Task/Agent не паникуют.
   - `route_cost` всегда ≥ 0 для всех входных данных в наборе.
   - `score` всегда `f64::is_finite()` для valid agent/task.

7. **Интеграционный тест: allocator + adapter**:
   - `AuctionAllocator::allocate_with_adapter` с `SarAdapter` отдаёт приоритет Scout-агентам
     перед Relay при прочих равных.
   - `CbbaAllocator` с `InspectionAdapter` предпочитает Inspector-агентов.

8. **Regression: существующие сценарные тесты** (`swarm-scenarios`, `swarm-sim`):
   - Обновить существующие тесты, проверяющие поля `Task`, чтобы учитывать новое поле `kind`.
   - `sar_scenario_one_task_per_cell` — добавить проверку `kind = Some(SarScan)`.
   - `build_inspection_scenario_tasks_match_edges` — добавить проверку `kind`.

### Категория 3 — тяжёлый рефакторинг

Требуют введения proptest или значительного рефакторинга:

9. **Property-based тесты** (`swarm-scenarios`):
   - `adapter.score(agent, task)` всегда `f64::is_finite()` (proptest).
   - `adapter.route_cost(from, task)` всегда `≥ 0.0` (proptest).
   - Для `SarAdapter`: `route_cost` → 0 когда `from == cell_center`.

### Gap / coverage notes

- `SarConfirmationScan` не создаётся ни одним builder'ом в текущей кодовой базе.
  Автотест `task_kind = SarConfirmationScan` будет только unit serde; интеграционный —
  явный gap до появления соответствующего builder'а.
- `RelayPlacement` аналогично: нет builder'а → только serde roundtrip.

## Risks and Tradeoffs

1. **`&AllocationAgent` vs `&Agent` в `MissionAdapter::score`**: prompt указывает `&Agent`,
   но allocators работают с `AllocationAgent`. Предлагается `&AllocationAgent` в трейте.
   Trade-off: реализации адаптеров не видят `battery_drain_rate`, `max_range` из `Agent`.
   Если нужен более точный scoring — потребуется расширить `AllocationAgent` или
   добавить конвертацию.

2. **Два пути scoring одновременно**: до полной миграции в аллокаторах будет
   две ветки (adapter/no-adapter). Риск расхождения поведения. Ограничено флагом
   `Option<Box<dyn MissionAdapter>>`.

3. **`MissionAdapter: Send + Sync`**: адаптеры должны быть thread-safe для хранения
   в структурах аллокаторов. Это ограничивает реализации (нельзя хранить `Rc`, `RefCell`).

4. **`RunState` → заполнение из runtime**: `RunState` в `swarm-types` не знает о `GridState`
   и `InspectionState`. Runtime должен конвертировать их в `RunState` перед вызовом
   `is_completed`. Конвертеры (`From<&GridState> for RunState` и т.п.) нужно разместить
   в `swarm-runtime`/`swarm-sim` — это дополнительные точки интеграции.

## Что могло сломаться

| Риск | Компонент | Как проверить |
|---|---|---|
| Десериализация Task из старых JSON | `swarm-types` | тест: roundtrip без поля `kind` → `kind = None` |
| Компиляция аллокаторов после добавления поля в `CbbaAllocator` | `swarm-alloc` | `cargo check -p swarm-alloc` |
| Существующие тесты, создающие `Task` литералом, не компилируются (новое поле без default) | `swarm-*` | `cargo check --workspace` |
| DSL validation ломает легаси-сценарии без `kind` | `swarm-sim` | тест: legacy task с `kind = None` → нет ValidationError |
| Интеграционные тесты runner (sar/inspection) | `swarm-sim`, `swarm-scenarios` | `cargo test -p swarm-sim -p swarm-scenarios` |
| `MissionAdapter` не реализует `Send + Sync` → compile error при хранении в allocator | `swarm-alloc` | `cargo check -p swarm-alloc` |
| Существующие тесты CBBA сломаны из-за изменения `marginal_score` | `swarm-alloc` | `cargo test -p swarm-alloc` |

Все точки проверяются командой `cargo check --workspace` сразу после Шага 1,
до реализации остальных шагов.

## Open Questions

1. **`AllocationAgent` vs `Agent` в adapter**: принять `&AllocationAgent` в трейте
   или расширить `AllocationAgent` полями из `Agent` (battery_drain_rate, max_range)?
   Текущий plan: `&AllocationAgent` — минимальные изменения.

2. **Как заполнять `RunState` из runtime?** Нужен ли `impl From<&GridState> for RunState`
   в `swarm-runtime`? Или достаточно ручного маппинга в `Coordinator`?

3. **`SarConfirmationScan`: когда builder?** Текущий `build_sar_scenario` не создаёт
   confirmation scan tasks. Планировать builder в M27 или отложить?

4. **Adapter в allocator: поле vs параметр вызова?** Текущий plan: поле в структуре
   (backward compat через `Option`). Альтернатива: параметр `allocate_with_adapter` —
   более явно, но требует обновления всех точек вызова.
