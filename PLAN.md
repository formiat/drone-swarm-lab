# Plan: M86 — MAVLink Safety / FC Contract

## Context

M86 добавляет слой планирования аппаратной безопасности MAVLink: загрузку
геозабора на FC и управление параметрами FC. Реализация не требует подключённого
дрона — только формирование артефактов dry-run и валидации.

Предшествующие milestone-ы завершены:
- M80 (Command IR): `MissionCommand`, `MissionCommandPlan` — `swarm-mission-ir`
- M81 (MAVLink Compiler): `compile_mavlink_common_plan`, `MavlinkCommonPlan` —
  `swarm-comms`
- M82 (Capability Profiles): `MavlinkCapabilityProfile`, PX4/ArduPilot профили —
  `swarm-comms`
- M83–M85 (примитивные миссии, Urban, деконфликт) — реализованы

Ключевые точки входа в код:
- `swarm-comms/src/mavlink_common_plan.rs:25` — `compile_mavlink_common_plan`
- `swarm-comms/src/mavlink_common_plan.rs:109` — `geofence_prelude: Option<Vec<MavlinkCommonMissionItem>>` уже зарезервировано
- `swarm-comms/src/mavlink_capability_profile.rs:201` — `geofence_support`
- `swarm-comms/src/mavlink_capability_profile.rs:203` — `parameter_support`
- `swarm-safety/src/lib.rs:24` — `Geofence`, `NoFlyZone`, `SafetyConfig`
- `swarm-sim/src/preflight.rs:9` — `run_preflight`

`swarm-comms` не зависит от `swarm-safety`; мост между ними живёт в `swarm-sim`
(который зависит от обоих).

## Investigation context

Нет `INVESTIGATION.md`; исследование проведено в рамках планирования.

Ключевые находки:
1. `MavlinkCommonPlan.geofence_prelude: Option<Vec<MavlinkCommonMissionItem>>`
   уже объявлено и всегда `None` — M86 заполняет это поле.
2. `MavlinkCapabilityProfile` имеет `geofence_support` и `parameter_support`,
   оба = `UnknownUntilSitlOrHardware` — M86 детализирует их.
3. `MavlinkCapabilityProfile` — `static const` без Serialize, так что добавление
   нового поля требует обновить все три статических инициализатора профилей.
4. `swarm-comms/Cargo.toml` не включает `swarm-safety`; добавлять зависимость
   не нужно — мост AABB→FcGeofenceItem реализуется в `swarm-sim`.
5. `MavlinkCommonPlanOptions` не имеет Serialize, добавление полей обратно
   совместимо через `..Default::default()`.
6. `FcParamId` — новый newtype: внутреннее поле приватное, derives AsRef/Deref/
   DerefMut/From/Into (CLAUDE.md: Newtype Wrappers).

## Affected components

| Крейт / Файл | Тип изменения |
|---|---|
| `swarm-comms/src/mavlink_geofence.rs` | Новый файл |
| `swarm-comms/src/mavlink_parameters.rs` | Новый файл |
| `swarm-comms/src/mavlink_fc_contract.rs` | Новый файл |
| `swarm-comms/src/mavlink_common_plan.rs` | Расширение (enum, struct, опции) |
| `swarm-comms/src/mavlink_capability_profile.rs` | Расширение профилей |
| `swarm-comms/src/lib.rs` | Добавить pub use из новых модулей |
| `swarm-sim/src/preflight.rs` (или новый `swarm-sim/src/fc_bridge.rs`) | Мост SafetyConfig→FcGeofenceItem |
| `docs/FC_CONTRACT.md` | Новый файл |
| `docs/MAVLINK_CAPABILITY_PROFILES.md` | Обновление (геозабор/параметры) |
| `docs/PREFLIGHT_SAFETY.md` | Обновление (FC contract как дополнение) |
| `docs/STATUS.md` | Статус M86 |
| `README.md` | Статус M86 |

## Implementation steps

---

### Шаг 1. Расширить `MavlinkCommonCommandName` командами геозабора

**Файл:** `swarm-comms/src/mavlink_common_plan.rs`

Добавить варианты в enum `MavlinkCommonCommandName` (строка ~176):

```rust
/// `MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION`
#[serde(rename = "MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION")]
FenceCircleInclusion,
/// `MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION`
#[serde(rename = "MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION")]
FenceCircleExclusion,
/// `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION`
#[serde(rename = "MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION")]
FencePolygonVertexInclusion,
/// `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION`
#[serde(rename = "MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION")]
FencePolygonVertexExclusion,
/// `MAV_CMD_DO_FENCE_ENABLE`
#[serde(rename = "MAV_CMD_DO_FENCE_ENABLE")]
DoFenceEnable,
```

Обновить `as_str()` — добавить соответствующие ветки.

**Результат:** новые варианты доступны для fence compiler и capability profile;
существующие тесты не ломаются (аддитивное изменение).

---

### Шаг 2. Создать `swarm-comms/src/mavlink_geofence.rs`

Новый модуль для FC-facing геозабора. Типы:

```rust
/// Kind of a single FC geofence item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FcGeofenceItemKind {
    CircleInclusion,
    CircleExclusion,
    PolygonInclusion,
    PolygonExclusion,
}

/// FC geofence item shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FcGeofenceShape {
    Circle { center_lat_e7: i32, center_lon_e7: i32, radius_m: f64 },
    /// value: `(lat_e7, lon_e7)` per vertex
    Polygon { vertices: Vec<(i32, i32)> },
}

/// One geofence item for FC upload planning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcGeofenceItem {
    pub id: String,
    pub kind: FcGeofenceItemKind,
    pub shape: FcGeofenceShape,
}

/// Compiled fence plan: mission items + optional enable command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MavlinkFencePlan {
    pub items: Vec<FcGeofenceItem>,
    /// If true, DoFenceEnable command appended to plan.
    pub enable_fence: bool,
}

/// Human-readable fence summary in a dry-run artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MavlinkFenceArtifact {
    pub item_count: usize,
    pub inclusion_count: usize,
    pub exclusion_count: usize,
    pub has_polygon: bool,
    pub has_circle: bool,
    pub profile_classification: MavlinkCompatibilityClass,
    pub caveats: Vec<String>,
}
```

Публичные функции:

```rust
/// Compile FcGeofenceItems to MavlinkCommonMissionItems.
/// Polygon with N vertices → N items (params[0] = vertex count).
/// Circle → 1 item (params[0] = radius_m).
pub fn compile_fence_items(
    plan: &MavlinkFencePlan,
    profile: &MavlinkCapabilityProfile,
) -> Result<(Vec<MavlinkCommonMissionItem>, Option<MavlinkCommonCommand>), FenceCompilerError>

/// Build a fence artifact summary from a compiled fence plan.
pub fn fence_artifact(
    plan: &MavlinkFencePlan,
    profile: &MavlinkCapabilityProfile,
) -> MavlinkFenceArtifact
```

Ошибки (`thiserror`):

```rust
#[derive(Debug, thiserror::Error)]
pub enum FenceCompilerError {
    #[error("fence item kind '{kind}' is not supported by profile '{profile}'")]
    UnsupportedByProfile { kind: FcGeofenceItemKind, profile: MavlinkCapabilityProfileId },
    #[error("polygon fence requires at least 3 vertices, got {count}")]
    PolygonTooFewVertices { count: usize },
    #[error("polygon fence exceeds MAVLink limit of {limit} vertices, got {count}")]
    PolygonTooManyVertices { count: usize, limit: usize },
    #[error("fence item '{id}' contains non-finite coordinate")]
    NonFiniteCoordinate { id: String },
}
```

MAVLink polygon limit = 70 вершин на polygon (ограничение MAVLink Common).

Формат `MavlinkCommonMissionItem` для вершины полигона:
- `command` = `FencePolygonVertexInclusion` / `FencePolygonVertexExclusion`
- `params[0]` = общее количество вершин полигона
- `lat_e7` / `lon_e7` = координата вершины
- `frame` = `"MAV_FRAME_GLOBAL"` (fence items используют абсолютные координаты)
- `relative_alt_m` = 0.0
- `autocontinue` = false, `current` = false

Для круга:
- `command` = `FenceCircleInclusion` / `FenceCircleExclusion`
- `params[0]` = radius_m
- `lat_e7` / `lon_e7` = центр

**Результат:** модуль скомпилируется, юнит-тесты внутри файла проходят.

---

### Шаг 3. Создать `swarm-comms/src/mavlink_parameters.rs`

Типы параметров FC:

```rust
/// Opaque FC parameter identifier (e.g., "EKF2_AID_MASK").
#[derive(AsRef, Deref, DerefMut, From, Into,
         Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FcParamId(String);

/// MAVLink parameter value type.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FcParamValue {
    Int32(i32),
    Float(f32),
}

/// Requirement range for a single FC parameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FcParamRange {
    ExactInt(i32),
    ExactFloat(f32),
    IntBounds { min: i32, max: i32 },
    FloatBounds { min: f32, max: f32 },
}

/// One required parameter with validation range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamRequirement {
    pub param_id: FcParamId,
    pub required_range: FcParamRange,
    pub reason: String,
}

/// Point-in-time parameter snapshot (dry-run or optional pre-flight capture).
/// key: `param_id`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamSnapshot {
    pub params: std::collections::HashMap<FcParamId, FcParamValue>,
    pub description: String,
}

/// Metadata for a known FC parameter.
pub struct FcKnownParam {
    pub id: &'static str,
    pub stack: MavlinkCapabilityProfileId,
    pub units: &'static str,
    pub range: Option<FcParamRange>,
    pub default_value: Option<FcParamValue>,
    pub caveats: &'static [&'static str],
}

/// Plan to read a set of parameters before mission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamReadPlan {
    pub param_ids: Vec<FcParamId>,
    pub rationale: String,
}

/// Plan to write/verify parameters before mission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamWritePlan {
    /// value: `(param_id, required_value)`
    pub writes: Vec<(FcParamId, FcParamValue)>,
    pub rationale: String,
}

/// Aggregate result of validating requirements against snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamValidationResult {
    pub violations: Vec<FcParamViolation>,
    pub checked_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FcParamViolation {
    RequiredParamMissing { param_id: FcParamId },
    ParamOutOfRange { param_id: FcParamId, actual: FcParamValue, range_description: String },
}
```

Статические реестры:

```rust
pub static FC_KNOWN_PARAMS_PX4: &[FcKnownParam] = &[
    FcKnownParam {
        id: "GF_ACTION",
        stack: MavlinkCapabilityProfileId::Px4,
        units: "enum",
        range: Some(FcParamRange::IntBounds { min: 0, max: 5 }),
        default_value: Some(FcParamValue::Int32(1)),
        caveats: &["0=None, 1=Warning, 2=Hold, 3=RTL, 4=Terminate, 5=Land"],
    },
    FcKnownParam {
        id: "GF_MAX_HOR_DIST",
        stack: MavlinkCapabilityProfileId::Px4,
        units: "m",
        range: Some(FcParamRange::FloatBounds { min: 0.0, max: 10000.0 }),
        default_value: Some(FcParamValue::Float(0.0)),
        caveats: &["0=disabled"],
    },
    FcKnownParam {
        id: "COM_ARM_WO_GPS",
        stack: MavlinkCapabilityProfileId::Px4,
        units: "bool",
        range: Some(FcParamRange::IntBounds { min: 0, max: 1 }),
        default_value: Some(FcParamValue::Int32(0)),
        caveats: &["Allows arming without GPS; use with caution in geofenced missions"],
    },
    FcKnownParam {
        id: "EKF2_AID_MASK",
        stack: MavlinkCapabilityProfileId::Px4,
        units: "bitmask",
        range: None,
        default_value: None,
        caveats: &["Controls EKF2 sensor fusion; required bits depend on mission"],
    },
];

pub static FC_KNOWN_PARAMS_ARDUPILOT: &[FcKnownParam] = &[
    FcKnownParam {
        id: "FENCE_ACTION",
        stack: MavlinkCapabilityProfileId::ArduPilot,
        units: "enum",
        range: Some(FcParamRange::IntBounds { min: 0, max: 4 }),
        default_value: Some(FcParamValue::Int32(0)),
        caveats: &["0=Report, 1=RTL, 2=Hold, 3=SmartRTL, 4=Brake"],
    },
    FcKnownParam {
        id: "FENCE_ALT_MAX",
        stack: MavlinkCapabilityProfileId::ArduPilot,
        units: "m",
        range: Some(FcParamRange::FloatBounds { min: 10.0, max: 1000.0 }),
        default_value: Some(FcParamValue::Float(100.0)),
        caveats: &["Maximum altitude for ArduPilot altitude fence"],
    },
    FcKnownParam {
        id: "FENCE_RADIUS",
        stack: MavlinkCapabilityProfileId::ArduPilot,
        units: "m",
        range: Some(FcParamRange::FloatBounds { min: 30.0, max: 10000.0 }),
        default_value: Some(FcParamValue::Float(300.0)),
        caveats: &["Circular radius fence; 0=disabled"],
    },
];
```

Публичные функции:

```rust
/// Validate one parameter requirement against a snapshot.
pub fn check_param_requirement(
    snapshot: &FcParamSnapshot,
    req: &FcParamRequirement,
) -> Result<(), FcParamViolation>

/// Validate all requirements; returns aggregate result.
pub fn validate_param_requirements(
    snapshot: &FcParamSnapshot,
    requirements: &[FcParamRequirement],
) -> FcParamValidationResult

/// Build a read plan covering all required param IDs.
pub fn read_plan_from_requirements(
    requirements: &[FcParamRequirement],
    rationale: impl Into<String>,
) -> FcParamReadPlan
```

**Результат:** типы и валидация параметров доступны, JSON roundtrip проходит.

---

### Шаг 4. Создать `swarm-comms/src/mavlink_fc_contract.rs`

FC contract объединяет fence plan и param requirements, валидирует их совместно
и блокирует старт миссии при нарушениях.

```rust
/// Combined FC contract: fence upload plan + parameter requirements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcContract {
    pub profile: MavlinkCapabilityProfileId,
    pub fence_plan: Option<MavlinkFencePlan>,
    pub param_requirements: Vec<FcParamRequirement>,
}

/// Result of validating a FcContract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcContractValidationResult {
    pub violations: Vec<FcContractViolation>,
    /// True when any violation blocks mission start.
    pub blocks_mission_start: bool,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FcContractViolation {
    /// Fence item kind not supported by selected profile.
    UnsupportedFenceType {
        profile_id: MavlinkCapabilityProfileId,
        fence_kind: FcGeofenceItemKind,
        reason: String,
    },
    /// Parameter value outside required range.
    ParamOutOfRange {
        param_id: FcParamId,
        actual: FcParamValue,
        range_description: String,
    },
    /// Required parameter absent from snapshot.
    RequiredParamMissing { param_id: FcParamId },
}
```

Функция:

```rust
/// Validate an FC contract; param_snapshot is optional (None = dry-run only,
/// param violations skipped when snapshot absent).
pub fn validate_fc_contract(
    contract: &FcContract,
    param_snapshot: Option<&FcParamSnapshot>,
) -> FcContractValidationResult
```

Логика: для каждого `FcGeofenceItem` проверяет `profile.fence_item_support`; если
classification `blocks_hardware_facing_success()` — добавляет `UnsupportedFenceType`.
Если `param_snapshot` предоставлен — запускает `validate_param_requirements` и
конвертирует `FcParamViolation` в `FcContractViolation`. Если хоть одно нарушение
есть — `blocks_mission_start = true`.

**Результат:** `validate_fc_contract` возвращает структурированный результат;
`blocks_mission_start` корректно блокирует старт миссии.

---

### Шаг 5. Расширить `MavlinkCapabilityProfile` поддержкой per-kind fence

**Файл:** `swarm-comms/src/mavlink_capability_profile.rs`

Добавить в `mavlink_capability_profile.rs` (импортировать из `mavlink_geofence`):

```rust
/// Support rule for a specific fence item kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FenceItemSupportRule {
    pub kind: FcGeofenceItemKind,
    pub classification: MavlinkCompatibilityClass,
    pub caveats: &'static [&'static str],
}
```

Добавить поле в `MavlinkCapabilityProfile`:

```rust
/// Per-kind fence item support.
pub fence_item_support: &'static [FenceItemSupportRule],
```

Обновить три статических профиля:
- `MAVLINK_COMMON_GENERIC_PROFILE`: всё → `Supported` (syntax-level)
- `PX4_PROFILE`:
  - `PolygonInclusion` → `SupportedWithCaveats` (SIH evidence; breach action via GF_ACTION)
  - `PolygonExclusion` → `SupportedWithCaveats`
  - `CircleInclusion` → `UnknownUntilSitlOrHardware`
  - `CircleExclusion` → `UnknownUntilSitlOrHardware`
  - Обновить `geofence_support` → `SupportedWithCaveats`
- `ARDUPILOT_PROFILE`: всё → `UnknownUntilSitlOrHardware`

Обновить `COMPATIBILITY_MATRIX_ROWS` добавив строки для `geofence_support`
и `parameter_support` с новыми классификациями.

Добавить публичную функцию:
```rust
/// Lookup fence item support rule for a profile and kind.
pub fn fence_item_support_rule(
    profile: &MavlinkCapabilityProfile,
    kind: FcGeofenceItemKind,
) -> Option<&FenceItemSupportRule>
```

**Результат:** профили имеют per-kind классификацию; матрица совместимости
обновлена; существующие тесты проходят с обновлёнными инициализаторами.

---

### Шаг 6. Расширить `MavlinkCommonPlanOptions` и `MavlinkCommonPlan`

**Файл:** `swarm-comms/src/mavlink_common_plan.rs`

В `MavlinkCommonPlanOptions` добавить:
```rust
/// Optional geofence plan to compile into geofence_prelude.
pub fence_plan: Option<MavlinkFencePlan>,
/// FC parameter requirements for contract validation.
pub param_requirements: Vec<FcParamRequirement>,
/// Optional pre-captured parameter snapshot for validation.
pub param_snapshot: Option<FcParamSnapshot>,
```

В `MavlinkCommonPlan` добавить:
```rust
/// Fence upload summary (populated when fence_plan provided).
#[serde(skip_serializing_if = "Option::is_none")]
pub fence_summary: Option<MavlinkFenceArtifact>,
/// FC contract validation result (populated when fence_plan or param_requirements present).
#[serde(skip_serializing_if = "Option::is_none")]
pub fc_contract_result: Option<FcContractValidationResult>,
```

В `compile_mavlink_common_plan`:
1. Если `options.fence_plan` задан — вызвать `compile_fence_items` и заполнить `geofence_prelude` и `fence_summary`.
2. Если fence plan или param requirements заданы — создать `FcContract`, вызвать
   `validate_fc_contract` и сохранить в `fc_contract_result`.
3. Ошибки компиляции fence оборачивать в новый вариант `MavlinkCommonCompilerError::FenceCompilation { source: FenceCompilerError }`.

В `MavlinkCommonCompilerError` добавить:
```rust
#[error("fence compilation failed: {source}")]
FenceCompilation { source: FenceCompilerError },
```

**Результат:** `compile_mavlink_common_plan` с fence plan → заполненный
`geofence_prelude` и `fence_summary`; `fc_contract_result` отражает прохождение FC contract.

---

### Шаг 7. Добавить мост `SafetyConfig → FcGeofenceItem` в `swarm-sim`

**Файл:** `swarm-sim/src/fc_bridge.rs` (новый)

```rust
use swarm_comms::{FcGeofenceItem, FcGeofenceItemKind, FcGeofenceShape, MavlinkFencePlan};
use swarm_safety::SafetyConfig;

/// Convert AABB geofence + no-fly zones from SafetyConfig to FC fence items.
/// AABB inclusion → 4-vertex polygon inclusion item.
/// AABB no-fly zones → 4-vertex polygon exclusion items.
/// Requires a WGS84 origin to convert local AABB coordinates.
pub fn safety_config_to_fence_plan(
    config: &SafetyConfig,
    origin: &MavlinkCoordinateOrigin,
    enable_fence: bool,
) -> Result<MavlinkFencePlan, FcBridgeError>
```

Конвертация: AABB (min_x, max_x, min_y, max_y) → 4 вершины полигона в WGS84
через `local_to_mavlink_int` (уже есть в `swarm-comms`).

`FcBridgeError` с thiserror:
- `CoordinateConversionError { source: MavlinkCoordinateError }`

Добавить `fc_bridge` в `swarm-sim/src/lib.rs`.

**Результат:** сценарии с `SafetyConfig` могут автоматически строить fence plan для FC.

---

### Шаг 8. Обновить экспорты `swarm-comms/src/lib.rs`

Добавить:
```rust
pub mod mavlink_geofence;
pub mod mavlink_parameters;
pub mod mavlink_fc_contract;

pub use mavlink_geofence::{
    compile_fence_items, fence_artifact, FcGeofenceItem, FcGeofenceItemKind,
    FcGeofenceShape, FenceCompilerError, MavlinkFenceArtifact, MavlinkFencePlan,
};
pub use mavlink_parameters::{
    check_param_requirement, read_plan_from_requirements, validate_param_requirements,
    FcKnownParam, FcParamId, FcParamRange, FcParamReadPlan, FcParamRequirement,
    FcParamSnapshot, FcParamValidationResult, FcParamValue, FcParamViolation,
    FcParamWritePlan, FC_KNOWN_PARAMS_ARDUPILOT, FC_KNOWN_PARAMS_PX4,
};
pub use mavlink_fc_contract::{
    validate_fc_contract, FcContract, FcContractViolation, FcContractValidationResult,
};
```

Обновить `pub use mavlink_capability_profile::...` добавив `FenceItemSupportRule`,
`fence_item_support_rule`.

**Результат:** публичный API `swarm-comms` охватывает все новые типы M86.

---

### Шаг 9. Написать автотесты (категория 1)

#### В `swarm-comms/src/mavlink_geofence.rs`:

```rust
#[cfg(test)]
mod tests {
    // circular_fence_compiles_to_expected_item:
    // FcGeofenceItem{kind=CircleInclusion, shape=Circle{center, radius}} →
    //   compile → 1 MavlinkCommonMissionItem с command=FenceCircleInclusion,
    //   params[0]=radius, lat_e7/lon_e7=center.
    
    // polygon_fence_compiles_to_n_vertex_items:
    // FcGeofenceItem{kind=PolygonInclusion, shape=Polygon{4 vertices}} →
    //   compile → 4 items с command=FencePolygonVertexInclusion,
    //   params[0]=4 для каждого.
    
    // fence_enable_command_present:
    // MavlinkFencePlan{enable_fence=true} → compile → Some(MavlinkCommonCommand)
    //   с command=DoFenceEnable, params[0]=1.0.
    
    // fence_enable_absent_when_disabled:
    // MavlinkFencePlan{enable_fence=false} → compile → None для enable command.
    
    // polygon_too_few_vertices_returns_error:
    // FcGeofenceItem{Polygon{2 vertices}} → FenceCompilerError::PolygonTooFewVertices.
    
    // polygon_too_many_vertices_returns_error:
    // FcGeofenceItem{Polygon{71 vertices}} → FenceCompilerError::PolygonTooManyVertices.
    
    // unsupported_profile_returns_structured_error:
    // ProfileId где данный kind → Unsupported → FenceCompilerError::UnsupportedByProfile.
}
```

#### В `swarm-comms/src/mavlink_parameters.rs`:

```rust
#[cfg(test)]
mod tests {
    // param_requirement_passes_within_int_bounds:
    // FcParamRequirement{range=IntBounds{0,5}} + snapshot{value=3} → Ok(()).
    
    // param_requirement_fails_outside_int_bounds:
    // FcParamRequirement{range=IntBounds{0,5}} + snapshot{value=10} → Err(ParamOutOfRange).
    
    // param_requirement_missing_returns_error:
    // FcParamRequirement{param_id="X"} + empty snapshot → Err(RequiredParamMissing).
    
    // param_snapshot_roundtrip_json:
    // FcParamSnapshot → serde_json::to_string → from_str → equal.
    
    // exact_int_requirement_passes:
    // FcParamRequirement{range=ExactInt(2)} + snapshot{value=Int32(2)} → Ok(()).
    
    // exact_int_requirement_fails:
    // FcParamRequirement{range=ExactInt(2)} + snapshot{value=Int32(3)} → Err.
}
```

#### В `swarm-comms/src/mavlink_fc_contract.rs`:

```rust
#[cfg(test)]
mod tests {
    // fc_contract_no_violations_does_not_block:
    // FcContract{supported polygon, param within range} → blocks_mission_start=false.
    
    // unsupported_fence_type_blocks_mission:
    // FcContract{Unsupported fence kind} → UnsupportedFenceType violation, blocks=true.
    
    // param_out_of_range_blocks_mission:
    // FcContract{param requirement} + snapshot{out of range} → blocks=true.
    
    // no_snapshot_skips_param_violations:
    // FcContract{param requirements} + snapshot=None → no param violations, blocks=false.
}
```

#### В `swarm-comms/src/mavlink_common_plan.rs`:

```rust
#[cfg(test)]
mod tests {
    // geofence_prelude_populated_when_fence_plan_provided:
    // options.fence_plan=Some(polygon) → compiled plan.geofence_prelude=Some([...]).
    
    // fence_summary_in_plan_when_fence_provided:
    // options.fence_plan=Some(...) → plan.fence_summary=Some(MavlinkFenceArtifact).
    
    // fc_contract_result_in_plan:
    // options с param_requirements → plan.fc_contract_result=Some(...).
    
    // no_fence_plan_geofence_prelude_none:
    // options без fence_plan → plan.geofence_prelude=None.
}
```

**Результат:** все тесты категории 1 имплементируются вместе с основным кодом.
Запускаются: `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms`.

---

### Шаг 10. Написать автотесты (категория 2, light refactoring)

Выносятся в `swarm-comms/tests/` или как `#[cfg(test)]` helpers:

- **Shared fence item assertion helper** — `assert_fence_item(item, expected_kind, expected_seq)`:
  переиспользуемая функция для проверки скомпилированных fence items в тестах.

- **Mock transport fence/param capture helper** — утилита, принимающая
  `MavlinkFencePlan` и `Vec<FcParamRequirement>` и возвращающая captured artifact
  для проверки (без реального транспорта).

- **Dry-run artifact fence/param assertion helper** — проверяет что
  `MavlinkCommonPlan.fence_summary` не `None` и содержит правильный `item_count`,
  `fc_contract_result.blocks_mission_start` корректен.

- **Preflight-to-FC-contract integration test** в `swarm-sim/tests/`:
  `SafetyConfig` с geofence → `run_preflight` → `safety_config_to_fence_plan`
  → `compile_mavlink_common_plan` → plan содержит fence prelude.
  Требует лёгкого рефакторинга preflight для экспорта helper-функции.

**Результат:** вспомогательный код для тестирования выделен; интеграционный тест
связывает preflight → FC contract.
Запускаются: `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms -p swarm-sim`.

---

### Шаг 11. Обновить документацию

1. **`docs/FC_CONTRACT.md`** (новый) — опишет слой FC contract:
   - что такое FC contract vs software preflight;
   - supported types (fence, params);
   - dry-run / execute semantics;
   - как блокируется старт миссии;
   - примеры артефактов.

2. **`docs/MAVLINK_CAPABILITY_PROFILES.md`** — добавить секцию:
   - таблицу `fence_item_support` для каждого профиля;
   - таблицу `parameter_support` с примерами known params.

3. **`docs/PREFLIGHT_SAFETY.md`** — добавить секцию FC contract:
   - software preflight остаётся авторитетным;
   - FC contract — дополнительный слой, не замена;
   - порядок вызовов.

4. **`docs/STATUS.md`** — пометить M86 как завершённый.

5. **`README.md`** — обновить список выполненных milestone-ов.

**Результат:** документация отражает M86; существующие docs smoke tests проходят.

---

### Шаг 12. Запустить итоговые проверки и сделать commit

```bash
cargo fmt --all
make clippy   # или cargo clippy --all-targets -- -D warnings
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim
cargo check
```

После прохождения: `git commit` с сообщением вида `Plan M86 MAVLink Safety / FC Contract`.

**Результат:** всё компилируется и тесты проходят; commit создан.

## Testing strategy

### Категория 1 — без рефакторинга (реализовать совместно с основным кодом)

| Тест | Файл | Покрываемая логика |
|---|---|---|
| `circular_fence_compiles_to_expected_item` | `mavlink_geofence.rs` | circular fence → 1 item |
| `polygon_fence_compiles_to_n_vertex_items` | `mavlink_geofence.rs` | polygon → N items |
| `fence_enable_command_present` | `mavlink_geofence.rs` | enable flag → command |
| `fence_enable_absent_when_disabled` | `mavlink_geofence.rs` | enable=false → None |
| `polygon_too_few_vertices_returns_error` | `mavlink_geofence.rs` | validation |
| `polygon_too_many_vertices_returns_error` | `mavlink_geofence.rs` | validation |
| `unsupported_profile_returns_structured_error` | `mavlink_geofence.rs` | profile error |
| `param_requirement_passes_within_int_bounds` | `mavlink_parameters.rs` | happy path |
| `param_requirement_fails_outside_int_bounds` | `mavlink_parameters.rs` | out-of-range |
| `param_requirement_missing_returns_error` | `mavlink_parameters.rs` | missing param |
| `param_snapshot_roundtrip_json` | `mavlink_parameters.rs` | serde roundtrip |
| `exact_int_requirement_passes` | `mavlink_parameters.rs` | exact match |
| `exact_int_requirement_fails` | `mavlink_parameters.rs` | exact mismatch |
| `fc_contract_no_violations_does_not_block` | `mavlink_fc_contract.rs` | happy path |
| `unsupported_fence_type_blocks_mission` | `mavlink_fc_contract.rs` | profile block |
| `param_out_of_range_blocks_mission` | `mavlink_fc_contract.rs` | param block |
| `no_snapshot_skips_param_violations` | `mavlink_fc_contract.rs` | dry-run mode |
| `geofence_prelude_populated_when_fence_plan_provided` | `mavlink_common_plan.rs` | compiler |
| `fence_summary_in_plan_when_fence_provided` | `mavlink_common_plan.rs` | artifact |
| `no_fence_plan_geofence_prelude_none` | `mavlink_common_plan.rs` | backward compat |

### Категория 2 — лёгкий рефакторинг

- Shared fence item assertion helper (выделить из `mavlink_geofence.rs`)
- Mock transport fence/param capture helper
- Dry-run artifact fence/param assertion helper
- Preflight-to-FC-contract integration test в `swarm-sim/tests/`

### Категория 3 — тяжёлый рефакторинг (не реализуются в M86)

- Локальный PX4/SIH ручной тест на принятие геозабора — требует PX4 SITL/SIH
- ArduPilot legacy fence path tests — требует ArduPilot SITL
- read-all-params large fixture — требует большой фикстуры параметров
- Version-specific param registry — требует интеграции с metadata базой ArduPilot/PX4

Gap: тесты `FcParamRange::FloatBounds` для граничных значений (edge case f32) не
покрываются в категории 1; нужно добавить 2-3 edge-case теста для float range
при реализации.

## Risks and tradeoffs

1. **Добавление поля в `MavlinkCapabilityProfile`** — структура `static const`,
   все три инициализатора обновятся; существующие тесты с `COMPATIBILITY_MATRIX_ROWS`
   нужно обновить. Риск низкий, но требует аккуратности.

2. **`MavlinkCommonCommandName` расширяется** — `classify_mission_item` в профиле
   вернёт `Unsupported` для новых fence команд, пока не добавлены правила.
   Нужно добавить правила для fence команд в `command_rules` профилей OR исключить
   fence items из стандартного compatibility pass (они проходят отдельный fence pass).
   Рекомендация: не пропускать fence items через `classify_mavlink_plan_compatibility`
   (она работает с mission items, не с fence items); документировать явно.

3. **`geofence_prelude` типизирован как `Vec<MavlinkCommonMissionItem>`** —
   fence items имеют другую семантику (`frame = MAV_FRAME_GLOBAL`), поле `current`
   и `autocontinue` неприменимы. Это технический долг, но не меняется в M86
   (тип уже зафиксирован в артефактах).

4. **Мост `SafetyConfig → FcGeofenceItem`** использует локальные координаты AABB
   с WGS84 origin. Если origin не задан — `safety_config_to_fence_plan` вернёт
   ошибку. В dry-run сценариях без реального origin нужен явный `MavlinkCoordinateOrigin`.

5. **`FcParamSnapshot` опционален** — в dry-run snapshot = None, param violations
   не проверяются. Это корректно, но означает что в dry-run `fc_contract_result`
   может показывать `blocks_mission_start=false` даже если параметры неверны.
   Документировать явно в `docs/FC_CONTRACT.md`.

## Что могло сломаться

| Область | Потенциальная регрессия | Как проверить |
|---|---|---|
| `MavlinkCommonCommandName` enum | Pattern match `_` в тестах → теперь не exhaustive | `cargo check` |
| `MavlinkCapabilityProfile` static | Добавление поля — все 3 инициализатора должны обновиться | `cargo build` |
| `COMPATIBILITY_MATRIX_ROWS` | Docs tests ожидают конкретные строки → нужно добавить новые | `cargo test -p swarm-comms` |
| `MavlinkCommonPlan` serde | Новые `skip_serializing_if=None` поля — backwards compatible | JSON roundtrip тест |
| `compile_mavlink_common_plan` | Новые поля в Options — `..Default::default()` покрывает | Тест без fence_plan |
| `MavlinkCommonPlanOptions::Default` | Надо добавить новые поля в `Default::default()` | `cargo check` |

## Open questions

1. **Обработка fence items в `classify_mavlink_plan_compatibility`**: нужно ли
   запускать compatibility pass по `geofence_prelude`? Текущий подход — отдельный
   fence pass через `validate_fc_contract`. Если нет — geofence_prelude items
   не появятся в `MavlinkCompatibilityReport`. Решение: оставить fence pass отдельным,
   документировать в профильном docs.

2. **`FcGeofenceShape::Polygon` координаты**: AABB конвертируется через origin;
   что делать с полигонами, заданными напрямую в WGS84 без origin? Ответ: прямой
   WGS84 полигон принимается через `scaled_coordinate` (уже есть в `mavlink_coords`).

3. **Fence sequence numbers**: `geofence_prelude` items имеют `seq` поле — должны
   ли они нумероваться независимо от mission items? Ответ: да, fence items имеют
   свой счётчик sequence начиная с 0 (MAVLink MISSION_TYPE_FENCE).
