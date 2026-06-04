# PLAN: M80 — Mission Command IR

## Контекст

M80 — первый шаг в цепочке M80→M89 (docs_raw/DRONE_A.25.md). Цель: создать
аппаратно-независимый intermediate representation (IR) для команд реальной дроно-миссии.

```text
MissionIntent -> MissionCommand IR -> backend compiler (M81+)
```

Без этого шага MAVLink-код из M81+ будет утекать PX4/ArduPilot-специфику прямо в
логику миссии. M80 только типы + валидация; MAVLink-сериализация, PX4-поведение и
реальное выполнение — за рамками этого milestone.

Текущее состояние кодовой базы (M79):
- `swarm-comms/src/mavlink/types.rs`: `MissionItem`, `Waypoint` — MAVLink-gated,
  используют feature `mavlink-transport`.
- `swarm-sim/src/urban/`: `UrbanPlannedRoute`, `UrbanNode` — локальные координаты.
- `swarm-examples/src/sitl_plan.rs`: `SitlDryRunArtifact`, `SitlPlan` — существующий
  dry-run артефакт без command IR.
- `swarm-replay/src/event_log.rs`: `Event` — событийный лог без command lifecycle.
- `swarm-safety/src/preflight.rs`: `SafetyValidationReport` — preflight gate без
  command-level проверок.

## Затронутые компоненты

| Компонент | Изменение |
|---|---|
| `crates/swarm-mission-ir/` | **Новый крейт** — core IR типы + валидация |
| `Cargo.toml` (workspace) | Добавить `swarm-mission-ir` в `members` и `workspace.dependencies` |
| `crates/swarm-sim/Cargo.toml` | Добавить `swarm-mission-ir` зависимость |
| `crates/swarm-sim/src/urban/mod.rs` | Добавить `urban_route_to_follow_route()` |
| `crates/swarm-examples/Cargo.toml` | Добавить `swarm-mission-ir` зависимость |
| `crates/swarm-examples/src/sitl_plan.rs` | Добавить `command_ir_summary: Option<MissionCommandSummary>` в `SitlDryRunArtifact` |
| `crates/swarm-examples/src/sitl_agent_runtime/runtime.rs` | Собрать IR и summary при `--dry-run` |
| `docs/MISSION_COMMAND_IR.md` | **Новый документ** — описание IR |
| `README.md` | Добавить M80 в Milestones + Workspace Layout |
| `docs/STATUS.md` | Добавить M80 milestone status |
| `docs/EXTENSION_GUIDE.md` | Упомянуть command IR как extension point |

## Шаги реализации

### Шаг 1. Создать крейт `swarm-mission-ir`

**Файлы:**
- `crates/swarm-mission-ir/Cargo.toml` — создать
- `crates/swarm-mission-ir/src/lib.rs` — создать

**Cargo.toml:**
```toml
[package]
name    = "swarm-mission-ir"
version = "0.1.0"
edition = "2021"
license.workspace = true

[dependencies]
derive_more = { workspace = true }
serde       = { workspace = true }
thiserror   = { workspace = true }
```

Нет зависимостей на другие swarm-* крейты — IR должен быть независимым фундаментом.

**Результат:** крейт компилируется, `cargo check -p swarm-mission-ir` проходит.

---

### Шаг 2. Добавить крейт в workspace

**Файл:** `Cargo.toml` (workspace root)

Добавить в `members`:
```toml
"crates/swarm-mission-ir",
```

Добавить в `workspace.dependencies`:
```toml
swarm-mission-ir = { path = "crates/swarm-mission-ir" }
```

**Результат:** `cargo build --workspace` компилирует новый крейт.

---

### Шаг 3. ID-типы: `CommandId`, `MissionId`, `RouteId`

**Файл:** `crates/swarm-mission-ir/src/ids.rs`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize,
         AsRef, Deref, DerefMut, From, Into)]
pub struct CommandId(String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize,
         AsRef, Deref, DerefMut, From, Into)]
pub struct MissionId(String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize,
         AsRef, Deref, DerefMut, From, Into)]
pub struct RouteId(String);
```

Все поля приватные. Derives: `AsRef, Deref, DerefMut, From, Into` из `derive_more`.

**Результат:** ID-типы используемы из lib.rs, проходят serde roundtrip-тест.

---

### Шаг 4. Типы координатной системы и высоты

**Файл:** `crates/swarm-mission-ir/src/frame.rs`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateFrame {
    /// WGS84 geodetic coordinates (lat/lon/alt).
    Wgs84,
    /// Local NED frame relative to a reference origin.
    LocalNed,
    /// Local ENU frame relative to a reference origin.
    LocalEnu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AltitudeReference {
    /// Above mean sea level.
    Amsl,
    /// Above ground level.
    Agl,
    /// Relative to takeoff/home position.
    RelativeHome,
    /// WGS84 ellipsoid height.
    Ellipsoid,
}
```

**Файл:** `crates/swarm-mission-ir/src/position.rs`

```rust
/// Geographic position in WGS84.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeoPosition {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub alt_m: f64,
}

/// Local position in metric units relative to a reference origin.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalPosition {
    pub x_m: f64,
    pub y_m: f64,
    pub z_m: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Position {
    Geo(GeoPosition),
    Local(LocalPosition),
}
```

**Результат:** все типы сериализуются детерминированно.

---

### Шаг 5. Типы политик и семантики

**Файл:** `crates/swarm-mission-ir/src/policy.rs`

```rust
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutAction {
    Abort,
    ReturnToLaunch,
    Hold,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeoutPolicy {
    /// value: `(command_timeout_secs, completion_timeout_secs, on_timeout)`
    pub command_timeout_secs: f64,
    pub completion_timeout_secs: f64,
    pub on_timeout: TimeoutAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalState {
    Landed,
    Hovering,
    AtWaypoint,
    OrbitComplete,
    RouteComplete,
    Aborted,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompletionTolerance {
    pub position_m: f64,
    pub altitude_m: f64,
}
```

**Результат:** типы сериализуются, доступны из lib.rs.

---

### Шаг 6. Тип направления орбиты

**Файл:** `crates/swarm-mission-ir/src/orbit.rs`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrbitDirection {
    Clockwise,
    CounterClockwise,
}
```

**Результат:** тип доступен в `MissionCommand::Orbit`.

---

### Шаг 7. Тип waypoint для route

**Файл:** `crates/swarm-mission-ir/src/waypoint.rs`

```rust
/// A single waypoint within a mission route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissionWaypoint {
    pub position: Position,
    /// Optional acceptance radius override for this waypoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_radius_m: Option<f64>,
}
```

**Результат:** `FollowRoute` может нести непустой список waypoints.

---

### Шаг 8. Основной enum `MissionCommand` (13 вариантов)

**Файл:** `crates/swarm-mission-ir/src/command.rs`

```rust
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum MissionCommand {
    Arm,
    Disarm,
    Takeoff { altitude_m: f64 },
    Hold { duration_secs: f64 },
    Land,
    ReturnToLaunch,
    GoTo { position: Position },
    FollowRoute {
        route_id: RouteId,
        waypoints: Vec<MissionWaypoint>,
    },
    LoiterTime { duration_secs: f64 },
    Orbit {
        center: Position,
        radius_m: f64,
        turns: f64,
        direction: OrbitDirection,
    },
    Pause,
    Resume,
    Abort,
}

impl MissionCommand {
    /// Returns the kebab-case kind name for display/logging.
    pub fn kind_name(&self) -> &'static str { ... }
}
```

Примечание: `duration_secs: f64` вместо `std::time::Duration` — Duration не реализует
`serde::Serialize` без включения feature. Используем f64 секунды для сериализации, что
явно соответствует единицам через имя поля.

**Результат:** все 13 примитивов определены, сериализуются через `serde_json`.

---

### Шаг 9. `MissionCommandEntry` и `MissionCommandPlan`

**Файл:** `crates/swarm-mission-ir/src/plan.rs`

```rust
/// A single command in a mission sequence with identity and source metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissionCommandEntry {
    pub command_id: CommandId,
    pub command: MissionCommand,
    /// Optional source task id from which this command was derived.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_task_id: Option<String>,
    /// Optional source route id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_route_id: Option<String>,
    /// Optional source agent id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_agent_id: Option<String>,
}

/// A complete hardware-agnostic mission command plan (the IR).
///
/// This is NOT a MAVLink plan. It is an intermediate representation that a
/// backend compiler (M81+) translates into protocol-specific command sequences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissionCommandPlan {
    pub schema_version: String,
    pub mission_id: MissionId,
    pub coordinate_frame: CoordinateFrame,
    pub altitude_reference: AltitudeReference,
    pub timeout_policy: TimeoutPolicy,
    pub expected_terminal_state: TerminalState,
    pub completion_tolerance: CompletionTolerance,
    pub commands: Vec<MissionCommandEntry>,
}
```

`schema_version` = `"mission_command_ir.v1"`.

**Результат:** полный план сериализуется/десериализуется roundtrip.

---

### Шаг 10. `MissionCommandSummary` (для dry-run артефакта)

**Файл:** `crates/swarm-mission-ir/src/summary.rs`

```rust
/// Compact summary of a MissionCommandPlan for inclusion in dry-run artifacts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MissionCommandSummary {
    pub mission_id: String,
    pub command_count: usize,
    /// key: command kind name, value: count
    pub commands_by_kind: std::collections::BTreeMap<String, usize>,
    pub coordinate_frame: String,
    pub altitude_reference: String,
    pub total_waypoints: usize,
}

impl MissionCommandSummary {
    pub fn from_plan(plan: &MissionCommandPlan) -> Self { ... }
}
```

`BTreeMap` гарантирует детерминированный порядок ключей в JSON.

**Результат:** `MissionCommandSummary::from_plan(&plan)` строит summary без паники.

---

### Шаг 11. Типизированные ошибки и функция валидации

**Файл:** `crates/swarm-mission-ir/src/error.rs`

```rust
#[derive(Debug, thiserror::Error)]
pub enum MissionIrError {
    #[error("duplicate command id '{0}'")]
    DuplicateCommandId(String),
    #[error("takeoff altitude must be positive, got {altitude_m}")]
    InvalidTakeoffAltitude { altitude_m: f64 },
    #[error("hold/loiter duration must be positive, got {duration_secs}s")]
    InvalidDuration { duration_secs: f64 },
    #[error("orbit radius must be positive, got {radius_m}m")]
    InvalidOrbitRadius { radius_m: f64 },
    #[error("orbit turns must be positive, got {turns}")]
    InvalidOrbitTurns { turns: f64 },
    #[error("follow_route command has no waypoints (route_id = '{route_id}')")]
    EmptyRoute { route_id: String },
    #[error("non-finite coordinate in {context}: ({x}, {y}, {z})")]
    NonFiniteCoordinate { context: &'static str, x: f64, y: f64, z: f64 },
    #[error("position kind '{kind}' is ambiguous for coordinate frame '{frame}'")]
    AmbiguousCoordinateFrame { kind: String, frame: String },
}
```

**Файл:** `crates/swarm-mission-ir/src/validation.rs`

```rust
/// Validates a MissionCommandPlan.
///
/// Checks: duplicate command ids, negative altitude, zero/negative duration,
/// non-finite coordinates, impossible orbit params, empty route, frame/position
/// kind consistency.
pub fn validate(plan: &MissionCommandPlan) -> Result<(), MissionIrError> { ... }
```

Правила валидации из Scope п.3:
- `Takeoff { altitude_m }`: altitude_m > 0 (не просто ≠ 0 — отрицательная тоже невалидна)
- `Hold { duration_secs }` / `LoiterTime { duration_secs }`: > 0
- `FollowRoute { waypoints, route_id }`: waypoints непустые; каждый waypoint — конечные координаты
- `Orbit { radius_m, turns, center }`: radius_m > 0, turns > 0, center — конечные координаты
- `GoTo { position }`: конечные координаты
- Все `CommandId` в плане — уникальные
- `CoordinateFrame::Wgs84` + `Position::Local` → `AmbiguousCoordinateFrame`
- `CoordinateFrame::LocalNed/LocalEnu` + `Position::Geo` → `AmbiguousCoordinateFrame`

**Результат:** `validate()` возвращает `Ok(())` для валидных планов и типизированную ошибку для каждого нарушения.

---

### Шаг 12. Публичный API в `lib.rs`

**Файл:** `crates/swarm-mission-ir/src/lib.rs`

Re-export всех публичных типов через `pub use` из подмодулей.
Добавить `pub use validation::validate;`.

**Результат:** пользователи крейта импортируют типы через `swarm_mission_ir::MissionCommand` etc.

---

### Шаг 13. Добавить `swarm-mission-ir` в `swarm-sim` и реализовать Urban bridge

**Файл:** `crates/swarm-sim/Cargo.toml`

```toml
swarm-mission-ir = { workspace = true }
```

**Файл:** `crates/swarm-sim/src/urban/mod.rs` (или отдельный `urban_bridge.rs`)

Добавить публичную функцию:

```rust
/// Converts a planned Urban route loop into a `MissionCommand::FollowRoute`.
///
/// Each node in the route becomes a `MissionWaypoint` with a `LocalPosition`
/// derived from the node's simulation pose (x, y) and the given altitude.
///
/// Returns `None` if the route has no nodes.
pub fn urban_route_to_follow_route(
    route: &UrbanPlannedRoute,
    route_id: swarm_mission_ir::RouteId,
    altitude_m: f64,
) -> Option<swarm_mission_ir::MissionCommand> { ... }
```

Функция не зависит от MAVLink. Позиции — `Position::Local(LocalPosition { x_m, y_m, z_m: altitude_m })`.

**Результат:** Urban маршрут можно представить как `follow_route` без MAVLink-полей; тест демонстрирует roundtrip.

---

### Шаг 14. Расширить `SitlDryRunArtifact` — command IR summary

**Файл:** `crates/swarm-examples/Cargo.toml`

```toml
swarm-mission-ir = { workspace = true }
```

**Файл:** `crates/swarm-examples/src/sitl_plan.rs`

Добавить поле в `SitlDryRunArtifact`:
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub command_ir_summary: Option<swarm_mission_ir::MissionCommandSummary>,
```

Поле `skip_serializing_if = "Option::is_none"` сохраняет обратную совместимость для
всех существующих тестов и артефактов — старые JSON-файлы продолжают проходить десериализацию.

**Файл:** `crates/swarm-examples/src/sitl_agent_runtime/runtime.rs` (или `sitl_plan.rs`)

В функции `dry_run_artifact()`:
- Построить `MissionCommandPlan` из `SitlPlan.waypoints` как последовательность
  `GoTo` команд (для waypoint-миссий) или `FollowRoute` (для urban-route экспорта).
- Установить `command_ir_summary: Some(MissionCommandSummary::from_plan(&plan))`.

Логика построения IR из SitlPlan:
- `export_kind == "urban_route"` → один `FollowRoute` (вызов `urban_route_to_follow_route`)
- иначе → `Arm` + `Takeoff` + `GoTo` per waypoint + `Land`

**Результат:** `sitl_agent --dry-run --dry-run-artifact out.json` включает поле `command_ir_summary` в JSON.

---

### Шаг 15. Документация `docs/MISSION_COMMAND_IR.md`

**Файл:** `docs/MISSION_COMMAND_IR.md` — создать новый

Разделы:
1. **IR is mission intent, not hardware execution** — объяснение назначения и границ M80
2. **Command primitives** — таблица 13 команд с описанием параметров и семантики
3. **Explicit semantics** — coordinate frame, altitude reference, timeout policy
4. **Validation rules** — перечень всех проверок
5. **Urban route as `follow_route`** — пример конвертации Urban маршрута
6. **What this IR is NOT** — no MAVLink, no PX4-specific behavior, no hardware execution
7. **Next steps** — M81 MAVLink Common Compiler

Этот doc содержит smoke-test anchor для теста `sitl_docs`.

**Результат:** файл существует, содержит фразу "mission intent, not hardware execution".

---

### Шаг 16. Обновить README.md, docs/STATUS.md, docs/EXTENSION_GUIDE.md

**README.md:**
- Добавить `swarm-mission-ir` в таблицу Workspace Layout:
  `| \`swarm-mission-ir\` | Hardware-agnostic mission command IR: 13 primitives, explicit semantics, typed validation. |`
- Добавить M80 в Milestones Overview:
  `| M80 | ✅ | Mission Command IR: hardware-agnostic command primitives, validation, Urban route as follow_route, dry-run artifact integration |`
- Обновить "Test coverage": `380+ → 410+` (примерно, после M80 тестов)

**docs/STATUS.md:**
- Добавить строку M80 в таблицу Milestone Status

**docs/EXTENSION_GUIDE.md:**
- Добавить раздел о `swarm-mission-ir` как новом extension point для backend compiler

**Результат:** все три файла содержат M80 milestone entry.

---

### Что могло сломаться

**Поведение:**
- `SitlDryRunArtifact` получает новое поле. Существующие тесты, которые
  десериализуют артефакт из JSON, не сломаются — поле `skip_serializing_if =
  "Option::is_none"` обеспечивает обратную совместимость при чтении. Тесты,
  которые сравнивают полную JSON-строку (`assert_eq!(json_str, expected)`),
  могут упасть — нужно проверить и обновить.
- `swarm-sim` получает новую зависимость `swarm-mission-ir`. Это не должно
  изменить поведение существующего кода, но увеличит время компиляции.
- `urban_route_to_follow_route()` — новая функция, не изменяет существующую логику.

**API/контракты:**
- `SitlDryRunArtifact::schema_version` остаётся `"sitl_dry_run_artifact.v1"` — поле
  `command_ir_summary` является аддитивным расширением.
- `MissionCommandPlan::schema_version` = `"mission_command_ir.v1"` — новый тип,
  не конфликтует с существующими schema версиями.

**Интеграции:**
- `artifact_validator` не проверяет `command_ir_summary` в M80 — поле опциональное.
  Будущий M81+ может добавить правило валидатора.

**Производительность:**
- Добавление `swarm-mission-ir` в `swarm-sim` и `swarm-examples` добавит зависимость
  в граф, но крейт маленький (только типы + валидация) — незначительно.

## Стратегия тестирования

### Категория 1: без рефакторинга (реализовать вместе с основными изменениями)

Файл тестов: `crates/swarm-mission-ir/src/` (в `#[cfg(test)]` в соответствующих модулях)

1. **Serialization roundtrip** для каждого из 13 примитивов:
   `serde_json::from_str(serde_json::to_string(&cmd).unwrap()).unwrap() == cmd`

2. **Невалидная высота Takeoff**: `altitude_m = -5.0` → `MissionIrError::InvalidTakeoffAltitude`

3. **Невалидная высота Takeoff (нулевая)**: `altitude_m = 0.0` → ошибка валидации

4. **Невалидная duration для Hold**: `duration_secs = 0.0` → `MissionIrError::InvalidDuration`

5. **Невалидная duration для Hold (отрицательная)**: `duration_secs = -1.0` → ошибка

6. **Невалидный orbit radius**: `radius_m = 0.0` → `MissionIrError::InvalidOrbitRadius`

7. **Невалидный orbit turns**: `turns = -1.0` → `MissionIrError::InvalidOrbitTurns`

8. **Non-finite координата в GoTo**: `x: f64::NAN` → `MissionIrError::NonFiniteCoordinate`

9. **Non-finite координата в Orbit center**: `lat_deg: f64::INFINITY` → ошибка

10. **FollowRoute с пустым waypoints**: → `MissionIrError::EmptyRoute`

11. **Дублирующиеся CommandId**: план с двумя записями, одинаковый `CommandId` → `MissionIrError::DuplicateCommandId`

12. **Стабильный порядок команд**: после serde roundtrip порядок `commands` сохраняется

13. **Порядок waypoints в FollowRoute**: после serde roundtrip порядок waypoints сохраняется

14. **Амбигуитет фрейма**: `CoordinateFrame::Wgs84` + `Position::Local` → `AmbiguousCoordinateFrame`

15. **Docs smoke test**: `docs/MISSION_COMMAND_IR.md` содержит строку "mission intent, not hardware execution"
    (в `crates/swarm-examples/tests/sitl_docs.rs` или отдельный тест)

16. **`MissionCommandSummary::from_plan`**: команды подсчитываются корректно

17. **urban_route_to_follow_route с непустым маршрутом**: возвращает `Some(MissionCommand::FollowRoute { .. })`

18. **urban_route_to_follow_route с пустым маршрутом**: возвращает `None`

### Категория 2: лёгкий рефакторинг

19. **Scenario DSL fixture с command sequence**: тест в `swarm-sim` или `swarm-scenarios`,
    строящий `MissionCommandPlan` из сценария и проверяющий структуру.
    Потребует расширения существующего DSL или добавления фикстуры.

20. **Dry-run артефакт включает command IR summary**: интеграционный тест в
    `swarm-examples/tests/sitl_agent/`, проверяющий, что поле `command_ir_summary`
    присутствует в JSON-артефакте после `--dry-run --dry-run-artifact`.

21. **Preflight validation использует данные из command IR**: тест проверяет, что
    preflight gate видит route/altitude из `MissionCommandPlan`. Требует лёгкой
    интеграции `swarm-safety` с `swarm-mission-ir`.

22. **Replay event fixture для command lifecycle**: тест показывает, что `Event::CommandDispatched`
    (или аналог) может быть сконструирован из `MissionCommandEntry`. Потребует добавления
    новых event variants в `swarm-replay`.

### Категория 3: тяжёлый рефакторинг

23. **Shared mission schema versioning** across DSL, replay и SITL artifacts:
    единый реестр schema_version, совместно проверяемый во всех трёх точках.
    Требует серьёзного рефакторинга нескольких крейтов.

24. **Typed mission/command id registry**: централизованный реестр `CommandId`,
    проверяющий уникальность глобально. Требует singleton или thread-local хранилища.

25. **Reusable backend executor trait**: trait `MissionBackendCompiler`, реализуемый
    M81+. Требует стабилизации API обоих сторон.

## Риски и компромиссы

| Риск | Вероятность | Смягчение |
|---|---|---|
| `SitlDryRunArtifact` тесты сравнивают полный JSON строкой | Средняя | Найти и обновить такие тесты; добавление `None`-поля не изменяет JSON при `skip_serializing_if` |
| `duration_secs: f64` вместо `std::time::Duration` — потеря типовой безопасности | Низкая | Явные имена полей (`duration_secs`, `command_timeout_secs`) устраняют ambiguity; Duration не имеет built-in serde |
| `Position` как enum с тегом может ломать существующие JSON-схемы | Н/А | Новый тип, не заменяет старые `Waypoint` / `Pose` |
| swarm-sim компиляция замедлится | Низкая | swarm-mission-ir — маленький крейт только из типов |
| `CoordinateFrame` конфликт с `SitlCoordinateFrame` в sitl_plan.rs | Средняя | Разные типы с разными именами, импортируются разными путями; документировать различие |

## Открытые вопросы

1. **`duration_secs: f64` vs chrono/std**: `std::time::Duration` требует serde feature
   (`chrono` или `serde-std` crate). Текущий план использует f64 с суффиксом `_secs`.
   Если chrono добавится в `swarm-mission-ir`, нужно будет добавить в workspace.dependencies.
   Для M80 f64 достаточно.

2. **Gravity of "ambiguous frame" validation**: сейчас `GoTo { position: Position::Local }` +
   `coordinate_frame: CoordinateFrame::Wgs84` считается ошибкой. Но в ранних тестах
   всегда используется локальная симуляция. Обсудить: возможно для M80 достаточно
   предупреждения, а не ошибки?

3. **IR summary в dry-run для primitive missions**: логика "построить MissionCommandPlan
   из SitlPlan.waypoints" — детали маппинга (какой `CommandId` присваивать каждому
   waypoint, какие `Arm/Takeoff/Land` добавлять) нужно согласовать при реализации.
   Для M80 упрощённый вариант (`GoTo` per waypoint) достаточен.
