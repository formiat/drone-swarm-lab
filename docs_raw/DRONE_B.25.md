# DRONE_B.25 — Итоговые майлстоуны: Urban, MAVLink Common и рой

Дата фиксации: 2026-06-03

Источник: синтез `DRONE_A.24`, `DRONE_B.24`, `DRONE_C.24`.

Этот документ — итоговый план следующей фазы после M70–M79. Он заменяет три
предыдущих варианта A/B/C.24 как консолидированная рекомендация.

Что было взято из каждого:

- **A.24 и C.24**: порядок работ — сначала IR/compiler/profiles как
  архитектурный фундамент, потом packs, потом swarm. Без фундамента все
  последующие задачи становятся одноразовыми скриптами.
- **C.24**: отдельный Primitive Mission Pack перед Urban (быстрая
  сквозная валидация архитектуры), детальный Urban Real Mission Pack,
  Swarm Topologies как отдельный milestone, Evidence Pack в конце.
- **B.24**: конкретные технические задачи — geo-referenced Urban граф,
  multi-agent deconfliction, geofence upload, FC parameter management,
  ArduPilot compatibility, synchronized swarm, mothership/carrier,
  DroneLink transport abstraction.

## Контекст

M70–M79 подняли проект до `hardware-integration candidate`: Urban routes
экспортируются в MAVLink waypoints, preflight gate останавливает плохие
миссии, artifacts machine-checkable, fault injection покрыт тестами,
operational runbooks определяют go/no-go gates.

Следующий шаг — другой уровень:

```text
M70–M79: mission research platform, готовая к SITL и первому hardware experiment.
M80–M88: mission/supervisor platform, способная управлять реальным роем дронов
          в реальной городской среде через стандартный MAVLink-стек.
```

## Архитектурная граница

PX4/ArduPilot owns:

- stabilization, attitude/rate control, motor physics;
- low-level waypoint following;
- EKF/local position estimate;
- onboard failsafes;
- vehicle-specific mode implementation;
- real airframe-specific tuning.

This project owns:

- mission intent and command IR;
- MAVLink command/mission/geofence/parameter planning;
- PX4/ArduPilot capability profiles;
- preflight safety and invariant validation;
- supervisor lifecycle, abort and replacement logic;
- Urban route planning and mission-level decisions;
- swarm roles, command coordination and ownership;
- replay, metrics, artifacts and evidence.

Ключевое правило MAVLink:

```text
Use MAVLink Common as the default command representation.
Model PX4/ArduPilot differences as explicit capability profiles.
Never let stack-specific hidden behavior creep into mission primitives.
```

## Non-Goals

До и вовремя этого плана не делать как основной workstream:

- MCU/driver code for an unknown board.
- Direct motor control or control-loop logic.
- Vendor SDK as the central abstraction.
- Real lidar/SLAM/CV/perception.
- Certified obstacle avoidance.
- Real RF mesh without chosen radio hardware.
- Hardware-readiness claims from dry-run or simulation artifacts.
- PX4-only or ArduPilot-only behavior hidden in generic mission primitives.
- Production API/semver before command/backend boundary stabilizes.
- Long benchmark reruns without new behavior or new interpretation questions.

## Milestone Chain

```text
M80 Mission Command IR
  -> M81 MAVLink Common Compiler
    -> M82 PX4 / ArduPilot Capability Profiles
      -> M83 Primitive Real Mission Pack
        -> M84 Urban Real Mission Pack
          -> M85 MAVLink Extensions: Geofence + FC Parameters
            -> M86 Swarm Command Plane
              -> M87 Swarm Topologies, Mothership + Transport Abstraction
                -> M88 SITL Dual-Stack + Hardware-Entry Evidence Pack
```

Почему такой порядок:

1. Без IR mission logic немедленно начинает захватывать MAVLink-специфику.
2. Без compiler primitive missions остаются simulation-only описаниями.
3. Без capability profiles PX4/ArduPilot совместимость станет скрытыми
   предположениями.
4. Primitive Pack быстро доказывает что M80-M82 работают сквозь всю систему.
5. Urban Pack делает из этой архитектуры реальный прикладной полигон.
6. MAVLink Extensions (geofence, params) нужны для hardware-adjacent экспериментов
   и дополняют Urban/primitives path.
7. Swarm Command Plane поднимает проект от команд одному дрону до координации
   нескольких.
8. Swarm Topologies делает паттерны mothership/relay/P2P явными, не
   hand-waving.
9. Evidence Pack создаёт дисциплину финального hardware entry.

---

## M80 — Mission Command IR

### Goal

Создать hardware-agnostic промежуточное представление команд для реальных
drone-операций.

Это не симуляторный API и не MAVLink. Это stable intermediate representation:

```text
operator intent
  -> MissionPrimitive (what should happen)
  -> MissionCommand IR (how it's expressed independently of hardware)
  -> backend compiler (MAVLink Common, dry-run, mock, future native)
```

Без этого слоя каждый backend придётся писать заново, и mission logic
неизбежно начнёт содержать PX4-ные или ArduPilot-ные особенности.

### Scope

1. Core command primitives:
   ```rust
   #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "cmd")]
   pub enum MissionCommand {
       Arm,
       Disarm,
       Takeoff(TakeoffCmd),
       Land(LandCmd),
       ReturnToLaunch(RtlCmd),
       GoTo(GoToCmd),
       FollowRoute(FollowRouteCmd),
       LoiterTime(LoiterTimeCmd),
       Orbit(OrbitCmd),
       ChangeSpeed(ChangeSpeedCmd),
       Pause,
       Resume,
       Abort(AbortCmd),
   }
   ```

2. Explicit intent structs:
   ```rust
   pub struct TakeoffCmd {
       pub altitude_m: f32,
       pub frame: AltitudeFrame,
       pub timeout: CommandTimeout,
   }

   pub struct LoiterTimeCmd {
       pub hold_seconds: f32,
       pub position: Option<CommandPosition>, // None = current position
       pub radius_m: f32,                     // 0 = autopilot default
   }

   pub struct OrbitCmd {
       pub center: CommandPosition,
       pub radius_m: f32,
       pub turns: f32,
       pub direction: OrbitDirection,
       pub altitude_m: f32,
       pub frame: AltitudeFrame,
   }

   pub struct FollowRouteCmd {
       pub route_id: String,
       pub waypoints: Vec<CommandPosition>,
       pub speed_m_per_s: Option<f32>,
   }

   pub struct AbortCmd {
       pub reason: String,
       pub abort_action: AbortAction,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum AltitudeFrame {
       RelativeHome,
       AbsoluteAmsl,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum OrbitDirection {
       Ccw,
       Cw,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum AbortAction {
       ReturnToLaunch,
       Land,
       Hover,
   }

   /// value: `(x_m, y_m, z_m)` in local frame or `(lat_deg, lon_deg, alt_m)` global.
   #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "frame")]
   pub enum CommandPosition {
       Local { x: f64, y: f64, z: f64 },
       Global { lat_deg: f64, lon_deg: f64, alt_m: f32 },
   }

   #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
   pub struct CommandTimeout {
       pub seconds: f32,
       pub on_timeout: AbortAction,
   }
   ```

3. Sequence container:
   ```rust
   pub struct MissionCommandSeq {
       pub id: String,
       pub vehicle_id: String,
       pub commands: Vec<MissionCommand>,
       pub abort_policy: AbortCmd,
   }
   ```

4. Validation rules:
   - altitude_m > 0 для `Takeoff`, `Orbit`;
   - hold_seconds > 0 для `LoiterTime`;
   - turns > 0 для `Orbit`;
   - radius_m >= 0 для `Orbit` и `LoiterTime`;
   - `FollowRoute` содержит хотя бы один waypoint;
   - все координаты конечны (не NaN, не Inf);
   - `CommandTimeout.seconds` > 0;
   - дублирующихся command id нет.

5. Integration с существующим кодом:
   - Urban route export может производить `FollowRouteCmd`;
   - preflight safety может валидировать altitude/coordinates из command seq;
   - Scenario DSL может содержать `command_seq: Option<MissionCommandSeq>`;
   - replay может записывать command lifecycle события;
   - сухой прогон (dry-run) artifact может сохранять command IR summary.

### Non-Goals

- Нет MAVLink byte serialization.
- Нет PX4/ArduPilot-специфичных mode semantics.
- Нет hardware execution.
- Нет raw vendor SDK обёртки.
- Нет real-time command streaming.

### Done Criteria

- `MissionCommand` и все intent structs определены, сериализуются
  snake_case без потерь.
- Validation реализована и покрыта тестами.
- Urban route export может вернуть `FollowRouteCmd` из planned route.
- Dry-run artifact может включать command seq summary.
- Docs объясняют что IR — mission intent, не hardware execution.

### Automated Tests

#### Tests That Need No Refactoring

- `mission_command_serde_roundtrip_all_variants`: все варианты enum
  сериализуются/десериализуются без потерь.
- `takeoff_rejects_zero_altitude`: `altitude_m=0.0` → validation error.
- `loiter_time_rejects_zero_duration`: `hold_seconds=0.0` → error.
- `orbit_rejects_zero_turns`: `turns=0.0` → error.
- `orbit_rejects_zero_radius`: `radius_m=0.0` → error.
- `follow_route_rejects_empty_waypoints`: пустой список → error.
- `non_finite_coordinate_rejected`: NaN/Inf в position → error.
- `command_seq_preserves_order`: порядок команд не меняется при serde.
- `docs_smoke_mission_intent_not_hardware_execution`.

#### Tests That Need Light Refactoring

- Shared command seq fixture builder.
- Urban route → `FollowRouteCmd` adapter test.
- Dry-run artifact command IR summary assertion helper.

#### Tests That Need Heavy Refactoring

- Schema versioning across command IR and Scenario DSL.
- Typed mission/command id registry.
- Reusable backend executor trait driven by `MissionCommandSeq`.

---

## M81 — MAVLink Common Compiler

### Goal

Компилировать `MissionCommandSeq` в детерминированный MAVLink Common план
с ordered command sequence, expected ACKs, timeout policy и abort plan.

```text
MissionCommandSeq -> MavlinkCommonPlan
```

Этот milestone создаёт реальное hardware-facing представление без реального
железа. Если primitive не поддерживается в Common dialect — результат
structured compiler error, не молчаливый fallback.

### Scope

1. Plan output type:
   ```rust
   pub struct MavlinkCommonPlan {
       pub source_mission_id: String,
       pub vehicle_id: String,
       pub phases: Vec<MavlinkPlanPhase>,
       pub expected_ack_sequence: Vec<ExpectedAck>,
       pub timeout_policy: Vec<CommandTimeout>,
       pub abort_plan: Vec<MavlinkAbortStep>,
       pub unsupported: Vec<UnsupportedPrimitive>,
       pub warnings: Vec<CompilerWarning>,
   }

   pub enum MavlinkPlanPhase {
       PreludeCommands(Vec<MavlinkCmd>),
       MissionItems(Vec<MavlinkMissionItem>),
       StartCommand(MavlinkCmd),
       PostMissionCommands(Vec<MavlinkCmd>),
   }

   pub struct MavlinkCmd {
       pub command: u16,         // MAV_CMD enum value
       pub params: [f32; 7],
       pub description: String,  // human-readable
   }

   pub struct MavlinkMissionItem {
       pub seq: u16,
       pub command: u16,
       pub frame: u8,
       pub params: [f32; 7],
       pub position: Option<CommandPosition>,
       pub description: String,
   }

   pub struct ExpectedAck {
       pub for_command: u16,
       pub accept_result: Vec<u8>, // MAV_RESULT values
       pub timeout: CommandTimeout,
   }

   pub struct UnsupportedPrimitive {
       pub command: String,
       pub reason: String,
   }
   ```

2. Supported MAVLink Common mappings:
   - `Arm` → `MAV_CMD_COMPONENT_ARM_DISARM(1)` prelude command;
   - `Disarm` → `MAV_CMD_COMPONENT_ARM_DISARM(0)` prelude command;
   - `Takeoff` → `MAV_CMD_NAV_TAKEOFF` mission item;
   - `Land` → `MAV_CMD_NAV_LAND` mission item;
   - `ReturnToLaunch` → `MAV_CMD_NAV_RETURN_TO_LAUNCH` mission item или
     post-mission command;
   - `GoTo` / `FollowRoute` waypoints → `MISSION_ITEM_INT`
     с `MAV_CMD_NAV_WAYPOINT`;
   - `LoiterTime` → `MISSION_ITEM_INT` с `MAV_CMD_NAV_LOITER_TIME`;
   - `Orbit` → `MISSION_ITEM_INT` с `MAV_CMD_NAV_LOITER_TURNS`
     или waypoint-approximation fallback (если profile разрешает);
   - `ChangeSpeed` → `MAV_CMD_DO_CHANGE_SPEED` prelude command;
   - `Abort` → `MAV_CMD_NAV_RETURN_TO_LAUNCH` или `MAV_CMD_NAV_LAND`
     abort plan.

3. Orbit fallback:
   - если capability profile помечает orbit как
     `supported_via_fallback` — компилировать как N waypoints по окружности;
   - записать `radius_m`, `turns`, `actual_waypoint_count` в artifact;
   - если profile `unsupported` — вернуть `UnsupportedPrimitive`.

4. Expected ACK contract:
   - каждый prelude command получает `ExpectedAck` с
     `accept_result: [MAV_RESULT_ACCEPTED]`;
   - mission upload phase ожидает `MISSION_ACK` с
     `MAV_MISSION_ACCEPTED`;
   - timeout policy записывается в план.

5. Dry-run artifact extension:
   - `sitl_dry_run_artifact.v1.json` расширяется секцией
     `mavlink_plan: Option<MavlinkPlanSummary>`;
   - `MavlinkPlanSummary` содержит: phase count, mission item count,
     prelude command ids, unsupported list, warning list, profile name.

6. Integration с existующим кодом:
   - `sitl_plan.rs` может вызвать compiler на `MissionCommandSeq`;
   - artifact validator проверяет что `mavlink_plan` присутствует если
     mission type требует;
   - `--dry-run` вывод включает compiled plan summary.

### Non-Goals

- Нет actual serial/UDP transport (это M85+).
- Нет polling/telemetry loop.
- Нет stream-based offboard control.
- Нет полного MAVLink dialect implementation.
- Нет capability profile enforcement (это M82).

### Done Criteria

- Все listed primitives компилируются в детерминированные MAVLink планы.
- Неподдерживаемые примитивы → `UnsupportedPrimitive`, не panic/silent fallback.
- Orbit waypoint fallback детерминирован и записывает approximation metadata.
- `MavlinkCommonPlan` сериализуется без потерь.
- `sitl_dry_run_artifact.v1.json` содержит `mavlink_plan` summary.
- Docs перечисляют точно поддерживаемые Common команды.

### Automated Tests

#### Tests That Need No Refactoring

- `compile_takeoff_produces_nav_takeoff_item`.
- `compile_land_produces_nav_land_item`.
- `compile_loiter_time_produces_nav_loiter_time_item`.
- `compile_follow_route_produces_ordered_mission_items`.
- `compile_unsupported_command_returns_structured_error`.
- `compile_orbit_without_profile_returns_unsupported`.
- `orbit_fallback_waypoints_are_deterministic`.
- `expected_ack_arm_command_has_timeout`.
- `mavlink_plan_serde_roundtrip`.
- `dry_run_artifact_contains_mavlink_plan_summary`.

#### Tests That Need Light Refactoring

- Artifact validator check для `mavlink_plan` секции.
- CLI dry-run emits plan summary.
- Preflight report links violations к command ids в плане.

#### Tests That Need Heavy Refactoring

- Backend-neutral MAVLink message model.
- Streaming mission upload state machine tests.
- Golden artifact schema versioning для plan output.

---

## M82 — PX4 / ArduPilot Capability Profiles

### Goal

Сделать совместимость с hardware-стеками явной. PX4 и ArduPilot оба
говорят на MAVLink, но не каждая команда работает идентично:

- arm/mode sequence различается;
- takeoff semantics различаются;
- orbit нативен в PX4, в ArduPilot — через GUIDED mode;
- mission start sequence различается;
- supported frames и command parameters различаются.

Проект не должен делать скрытых предположений ни о каком стеке.

### Scope

1. Profile type:
   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum AutopilotStack {
       MavlinkCommonGeneric,
       Px4Multicopter,
       ArduPilotCopter,
   }

   pub struct CapabilityProfile {
       pub stack: AutopilotStack,
       pub commands: HashMap<String, CommandSupport>,
       pub mode_sequence: ModeSequence,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum CommandSupport {
       Supported,
       SupportedWithCaveats { caveat: String },
       SupportedViaFallback { fallback: String },
       RequiresStackSpecificMapping { note: String },
       Unsupported { reason: String },
       UnknownUntilSitl,
   }
   ```

2. Mode sequence для arm/takeoff/execute:
   - **PX4**: arm → `MAV_CMD_NAV_TAKEOFF` → `MAV_CMD_MISSION_START`.
   - **ArduPilot**: `MAV_CMD_DO_SET_MODE(GUIDED)` → arm →
     `MAV_CMD_NAV_TAKEOFF` → `MAV_CMD_DO_SET_MODE(AUTO)`.
   - **Generic**: только upload + arm + start, без mode transitions.

3. Compatibility matrix (минимум для начала):

   | Primitive | Common Generic | PX4 Multicopter | ArduPilot Copter |
   |-----------|----------------|-----------------|------------------|
   | arm/disarm | supported | supported | supported |
   | takeoff | supported | supported | supported_with_caveats |
   | land | supported | supported | supported |
   | return_to_launch | supported | supported | supported |
   | go_to / waypoint | supported | supported | supported |
   | loiter_time | supported | supported | supported |
   | orbit | supported_via_fallback | supported | requires_stack_specific_mapping |
   | change_speed | supported | supported | supported |
   | abort | supported_via_fallback | supported | supported |

   Каждая запись должна иметь: caveat/fallback text, MAVLink command id,
   test coverage статус.

4. Как compiler использует profile:
   - compiler принимает `&CapabilityProfile` как аргумент;
   - для `Unsupported` → `UnsupportedPrimitive` в плане;
   - для `SupportedWithCaveats` → `CompilerWarning` в плане;
   - для `SupportedViaFallback` → используется описанный fallback;
   - profile не может молчаливо изменить mission semantics.

5. Autodecection из heartbeat:
   - `CapabilityProfile::detect_from_heartbeat(hb: &HEARTBEAT_DATA)` →
     `Option<AutopilotStack>`;
   - `MAV_AUTOPILOT_PX4` → `Px4Multicopter`;
   - `MAV_AUTOPILOT_ARDUPILOTMEGA` → `ArduPilotCopter`;
   - unknown → None.

### Non-Goals

- Нет exhaustive autopilot certification.
- Нет vendor-specific SDK integration.
- Нет unsupported command shims, притворяющихся success.
- Нет versioned profiles для каждой версии прошивки (пока).

### Done Criteria

- Три initial profiles существуют как данные, не только комментарии.
- Compiler принимает profile и применяет его к каждому primitive.
- Profile differences отражены в artifacts (warnings, caveats, unsupported list).
- `detect_from_heartbeat` возвращает правильный stack для PX4 и ArduPilot.
- Docs содержат compatibility matrix.
- Существующие PX4 SITL тесты не сломаны.

### Automated Tests

#### Tests That Need No Refactoring

- `px4_profile_marks_orbit_supported`.
- `ardupilot_profile_marks_orbit_requires_stack_mapping`.
- `generic_profile_marks_orbit_via_fallback`.
- `unsupported_command_produces_compiler_error_not_panic`.
- `supported_with_caveats_produces_warning_in_artifact`.
- `detect_px4_from_heartbeat_autopilot_field`.
- `detect_ardupilot_from_heartbeat_autopilot_field`.
- `autopilot_stack_serde_roundtrip_snake_case`.
- `command_support_serde_roundtrip_all_variants`.

#### Tests That Need Light Refactoring

- Compatibility matrix docs check от profile data.
- CLI `--profile` flag passes profile to compiler.
- Existing SITL dry-run tests параметризованы по profile.

#### Tests That Need Heavy Refactoring

- SITL-backed profile conformance checks.
- Autopilot-version-specific profile registry.
- Parameter schema validation от реальных autopilot metadata.

---

## M83 — Primitive Real Mission Pack

### Goal

Реализовать минимальный набор real-command миссий, которые компилируются
end-to-end через M80-M82 в MAVLink Common планы. Три простые миссии
доказывают что вся архитектура работает вместе прежде чем добавлять
Urban-сложность.

```text
M83 is the first smoke test of the full pipeline:
IR -> compiler -> profile -> artifact -> validator
```

### Scope

1. Миссия 1: Takeoff-Hold-Land.
   ```text
   arm -> takeoff(3m) -> loiter_time(10s) -> land
   ```

2. Миссия 2: Takeoff-Orbit-Land.
   ```text
   arm -> takeoff(3m) -> orbit(center=current, radius=2m, turns=3, ccw) -> land
   ```

3. Миссия 3: Takeoff-Waypoint-Square-Land.
   ```text
   arm -> takeoff(3m) -> follow_route(4 waypoints, 5m square) -> land
   ```

4. Каждая миссия определяет:
   - `MissionCommandSeq` с id и vehicle_id;
   - compiled `MavlinkCommonPlan` для трёх profiles (generic, px4, ardupilot);
   - expected ACKs для каждой фазы;
   - timeout policy (сколько секунд ждать каждую команду);
   - abort policy (RTL при любом timeout);
   - preflight safety requirements (altitude bounds, geo_origin present);
   - dry-run artifact output.

5. Scenario DSL fixtures:
   - `scenarios/primitive.hover.json` — обновить чтобы содержал
     `MissionCommandSeq` вместо только `primitive_mission`;
   - `scenarios/primitive.orbit.json` — аналогично;
   - `scenarios/primitive.takeoff-land.json` — аналогично.

6. Если primitive не portable — миссия явно говорит об этом:
   - orbit compilation на ArduPilot profile → warning + stack-specific note
     в artifact;
   - landing completion может отличаться по profile → зафиксировано в
     `warnings`.

### Non-Goals

- Нет real flight.
- Нет claim что orbit работает идентично на PX4 и ArduPilot.
- Нет внешней зависимости от connected vehicle.
- Нет PX4/SITL как обязательного шага для тестов.

### Done Criteria

- Три primitive миссии компилируются в детерминированные MAVLink планы.
- PX4/ArduPilot profiles классифицируют каждую миссию корректно.
- Artifact validator принимает все три mission artifacts.
- `cargo test` зелёный без PX4/SITL.
- Docs объясняют что именно можно проверить без железа.

### Automated Tests

#### Tests That Need No Refactoring

- `hover_mission_command_seq_validates`: IR валидация проходит.
- `hover_mission_compiles_to_loiter_time_item`.
- `orbit_mission_compiles_to_loiter_turns_on_px4_profile`.
- `orbit_mission_produces_waypoint_fallback_on_generic_profile`.
- `square_route_mission_produces_four_waypoint_items`.
- `all_primitive_missions_have_abort_policy`.
- `all_primitive_missions_have_timeout_policy`.
- `primitive_mission_artifacts_validate`.

#### Tests That Need Light Refactoring

- Fixture-backed dry-run artifacts для всех трёх primitive missions.
- Artifact validator checks expected ACK и telemetry sections.
- Replay summary включает command lifecycle события.

#### Tests That Need Heavy Refactoring

- Simulated ACK/telemetry state machine.
- Backend executor integration tests.
- SITL execution harness для primitive missions.

---

## M84 — Urban Real Mission Pack

### Goal

Сделать Urban основным прикладным полигоном проекта. Urban маршруты
компилируются в command IR и MAVLink планы, а не остаются только
simulation-local behavior.

Дополнительно: Urban граф получает поддержку реальных GPS-координат
(из B.24 M80), и добавляется multi-agent route deconfliction (из B.24 M81).

### Scope

1. Geo-referenced Urban граф:
   ```rust
   pub struct UrbanNode {
       pub id: UrbanNodeId,
       pub pose: Pose,                  // local coords, backward compat
       pub geo: Option<UrbanGeoPoint>,  // GPS coords, if geo-referenced
   }

   pub struct UrbanGeoPoint {
       pub lat_deg: f64,
       pub lon_deg: f64,
       pub alt_m: f64,
   }
   ```
   - Если `geo` есть у всех узлов — route export использует GPS напрямую.
   - Если `geo` отсутствует — прежнее поведение (local + geo_origin offset).
   - Если частично заполнено — validation error.
   - `coordinate_mode: "geo_referenced" | "local_with_origin"` в export artifact.
   - GeoJSON utility: `parse_urban_map_geojson(input: &str)` → базовый import
     для Point/LineString features.

2. Multi-agent route deconfliction:
   ```rust
   pub struct UrbanSegmentLock {
       pub edge_id: UrbanEdgeId,
       pub held_by: AgentId,
       pub acquired_at_tick: u64,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum UrbanDeconflictPolicy {
       FirstCome,
       Priority,
       RoundRobin,
   }
   ```
   - Patrol runner резервирует следующий сегмент перед движением.
   - При конфликте применяется `UrbanState.deconflict_policy`.
   - Replay events: `UrbanSegmentLockAcquired`, `UrbanSegmentLockReleased`,
     `UrbanSegmentConflict`, `UrbanDeconflictWait`.
   - Метрики: `urban_segment_conflict_count`, `urban_deconflict_wait_ticks`.

3. Urban mission types (command IR output):

   **urban-block-patrol:**
   ```text
   takeoff -> follow_route(perimeter segments) -> [optional loiter_time at POI]
           -> land / return_to_launch
   ```

   **urban-search-until-target:**
   ```text
   takeoff -> follow_route(search segments) -> [mocked detection event]
           -> loiter_time(hold) или return_to_launch при detection
   ```

   **urban-blocked-route-response:**
   ```text
   takeoff -> follow_route -> detect blocked segment
           -> policy: wait / follow_route(replan) / abort
   ```

   **urban-multi-agent-patrol:**
   ```text
   N agents, split route segments with ownership
   -> each agent: takeoff -> follow_route(owned_segments) -> land
   -> segment deconfliction between agents
   -> on agent failure: reallocation of unfinished segments
   ```

4. MAVLink output для Urban missions:
   - каждая Urban mission type компилируется через M81 compiler;
   - Urban waypoints → `MISSION_ITEM_INT` с `MAV_CMD_NAV_WAYPOINT`;
   - hold points → `MAV_CMD_NAV_LOITER_TIME`;
   - профиль добавляет warnings если coordinate frame не подтверждён;
   - export artifact содержит route metadata + command IR summary.

5. Temporal constraints:
   - `UrbanTemporalConstraint { edge_id, available_from_tick, available_until_tick }` —
     сегмент доступен только в определённое окно тиков;
   - integration с `UrbanTemporaryObstacle` scheduler;
   - planner учитывает temporal constraints при route planning.

### Non-Goals

- Нет real lidar.
- Нет real CV/bus detector.
- Нет certified collision avoidance.
- Нет full GIS/navmesh engine.
- Нет claim что buildings/roads физически точны если fixture не говорит обратное.

### Done Criteria

- Geo-referenced `UrbanNode` экспортирует корректные WGS84 waypoints.
- Multi-agent patrol не занимает один сегмент одновременно.
- Urban-block-patrol компилируется в command IR и MAVLink план.
- Urban-search-until-target реагирует на mocked detection event.
- Blocked-route response производит updated command plan.
- Temporal constraints влияют на route planner.
- `scenarios/urban.geo-referenced.json` загружается и экспортируется.

### Automated Tests

#### Tests That Need No Refactoring

- `geo_node_export_uses_node_geo_directly`.
- `mixed_geo_nodes_fail_validation`.
- `segment_lock_exclusive_no_simultaneous_hold`.
- `first_come_policy_respects_arrival_order`.
- `urban_patrol_command_seq_validates`.
- `urban_patrol_compiles_to_waypoint_items`.
- `urban_search_stops_on_mock_detection_event`.
- `blocked_route_response_produces_replacement_route`.
- `temporal_constraint_prevents_early_route_use`.

#### Tests That Need Light Refactoring

- Multi-agent urban scenario builder.
- Segment lock assertion helper.
- GeoJSON fixture helper.
- Urban command IR export assertion helper.

#### Tests That Need Heavy Refactoring

- Multi-agent temporal deconfliction stress tests.
- Richer map import от реального OSM фрагмента.
- Multi-altitude Urban airspace model.

---

## M85 — MAVLink Extensions: Geofence + FC Parameters

### Goal

Добавить две hardware-adjacent MAVLink операции которые нужны до любого
реального эксперимента: загрузку geofence на борт FC и чтение/запись
параметров FC.

Сейчас geofence проверяется только software-side в preflight. В production
он должен быть загружен прямо на борт чтобы FC enforced его аппаратно.
Параметры FC (скорость, высота RTL, failsafe) должны быть верифицированы
или выставлены программно перед миссией.

### Scope

1. Geofence upload типы:
   ```rust
   pub enum FenceItem {
       CircleInclusion { center: CommandPosition, radius_m: f32 },
       CircleExclusion { center: CommandPosition, radius_m: f32 },
       PolygonInclusion { vertices: Vec<CommandPosition> },
       PolygonExclusion { vertices: Vec<CommandPosition> },
   }

   pub struct FenceUploadReport {
       pub items_uploaded: usize,
       pub ack: String,
       pub fence_enabled: bool,
   }
   ```

2. `MavlinkTransport::upload_geofence(items: &[FenceItem])`:
   - компилирует fence items в `MISSION_ITEM_INT` с `MAV_CMD_NAV_FENCE_*`;
   - использует тот же MISSION_COUNT/REQUEST/ACK handshake;
   - после upload: `MAV_CMD_DO_FENCE_ENABLE`;
   - возвращает `FenceUploadReport`.

3. FC parameter types:
   ```rust
   pub enum FcParamValue {
       Int32(i32),
       Float(f32),
   }

   pub struct FcParam {
       pub id: String,
       pub value: FcParamValue,
       pub index: u16,
   }

   pub mod known_params {
       /// PX4: horizontal mission speed, m/s. Range: [0, 20].
       pub const MPC_XY_CRUISE: &str = "MPC_XY_CRUISE";
       /// ArduCopter: waypoint speed, cm/s. Range: [0, 2000].
       pub const WPNAV_SPEED: &str = "WPNAV_SPEED";
       /// ArduPilot: RTL altitude, cm AGL. Range: [200, 8000].
       pub const RTL_ALT: &str = "RTL_ALT";
       /// PX4: takeoff minimum altitude, m. Range: [0, 50].
       pub const MIS_TAKEOFF_ALT: &str = "MIS_TAKEOFF_ALT";
       /// PX4: mission loiter radius, m.
       pub const NAV_LOITER_RAD: &str = "NAV_LOITER_RAD";
       /// ArduCopter: geofence action (0=none,1=RTL,2=hover).
       pub const FENCE_ACTION: &str = "FENCE_ACTION";
   }
   ```

4. `MavlinkTransport` param API:
   - `read_param(id: &str) -> Result<FcParam, ParamError>`;
   - `write_param(id: &str, value: FcParamValue) -> Result<FcParam, ParamError>`;
   - `read_all_params() -> Result<Vec<FcParam>, ParamError>`;
   - timeout и retry по тому же паттерну что mission upload.

5. Pre-mission param requirements в `SafetyConfig`:
   ```rust
   pub struct ParamRequirement {
       pub param_id: String,
       pub min: Option<f32>,
       pub max: Option<f32>,
       pub expected_value: Option<FcParamValue>,
   }
   ```
   - Preflight читает params с борта в execute mode, пропускает в dry-run.
   - Если FC недоступен в dry-run → skip с warning.

6. Dry-run artifact extensions:
   - `geofence_items: Option<Vec<FenceItemSummary>>`;
   - `params_snapshot: Option<Vec<FcParamSummary>>`.

### Non-Goals

- Нет geofence read-back verification.
- Нет certified geofence enforcement claim.
- Нет runtime param change во время выполнения миссии.
- Нет полного parameter backup/restore.

### Done Criteria

- `upload_geofence` компилируется под `mavlink-transport`.
- Mock тесты проверяют правильные MAVLink команды для каждого fence type.
- `read_param` и `write_param` компилируются и проходят mock тесты.
- `known_params` содержит минимум 6 documented параметров для PX4 и ArduPilot.
- Dry-run artifact содержит fence summary и param snapshot placeholder.
- Preflight rule `geofence.waypoint_outside` остаётся — upload дополняет,
  не заменяет software-side check.

### Automated Tests

#### Tests That Need No Refactoring

- `fence_upload_circle_sends_nav_fence_circle_inclusion`.
- `fence_upload_polygon_sends_vertex_items_in_order`.
- `fence_enable_command_sent_after_items_accepted`.
- `fence_upload_failure_returns_structured_error`.
- `read_param_sends_request_and_parses_value`.
- `write_param_sends_param_set_and_awaits_ack`.
- `param_requirement_passes_within_bounds`.
- `param_requirement_fails_outside_bounds`.
- `dry_run_artifact_contains_fence_summary`.

#### Tests That Need Light Refactoring

- Mock transport fence capture helper.
- Mock conn param fixture builder.
- Artifact fence and param assertion helpers.

#### Tests That Need Heavy Refactoring

- Local PX4/SIH manual test: upload fence, verify accepted.
- ArduPilot FENCE_POINT legacy path.
- Read_all_params на большом наборе (100+ params).

---

## M86 — Swarm Command Plane

### Goal

Перейти от команд одному дрону к координированным multi-drone миссиям
через единый GCS.

Swarm command plane описывает как supervisor производит N per-agent command
plans из одного swarm mission intent, как управляется ownership, как
обрабатываются failures, и как выполняются синхронные swarm-операции.

### Scope

1. Command fanout:
   ```rust
   pub struct SwarmMission {
       pub id: String,
       pub agents: Vec<AgentMissionPlan>,
       pub global_abort_policy: AbortCmd,
       pub topology: SwarmTopologyRef,
   }

   pub struct AgentMissionPlan {
       pub agent_id: AgentId,
       pub command_seq: MissionCommandSeq,
       pub owned_tasks: Vec<TaskId>,
       pub owned_segments: Vec<UrbanEdgeId>,
       pub replacement_priority: u8,
   }
   ```
   - `SwarmMission` компилируется через M81 compiler в N per-agent
     `MavlinkCommonPlan`.
   - Каждый агент получает свой artifact.

2. Swarm supervisor states:
   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum SwarmSupervisorState {
       Planned,
       Dispatched,
       Active,
       Degraded,
       Replacing,
       Aborting,
       Completed,
       Failed,
   }
   ```

3. Synchronized swarm commands:
   ```rust
   pub enum SwarmCommand {
       ArmAll { timeout_per_drone: Duration },
       TakeoffAll { altitude_m: f32, jitter_window_ms: u64 },
       AbortAll { reason: String },
       StartAll { jitter_window_ms: u64 },
   }

   pub struct SwarmCommandReport {
       pub command: String,
       pub per_agent: Vec<(AgentId, CommandOutcome)>,
       pub overall: SwarmCommandOutcome,
       pub latency_ms: u64,
   }
   ```
   - `TakeoffAll` отправляет takeoff с небольшим jitter между агентами.
   - `AbortAll` fire-and-forget: RTL всем без ожидания ack.
   - `SwarmCommandReport` входит в общий run report.

4. Ownership и replacement:
   - Duplicate ownership → SwarmMission validation error.
   - Failed agent → release unfinished `TaskId` и `UrbanEdgeId`.
   - Release events → reallocation через existing M73 path.
   - Replacement `MissionCommandSeq` компилируется через M81.
   - `SwarmCommandReport` per-agent ACK tracking.

5. Formation waypoints:
   ```rust
   pub struct FormationWaypoint {
       pub lead_position: CommandPosition,
       /// value: `(agent_id, offset_position)` relative to lead.
       pub follower_offsets: Vec<(AgentId, CommandPosition)>,
   }
   ```
   - `expand_formation_waypoints(formation, lead_route)` →
     `Vec<(AgentId, Vec<CommandPosition>)>`.

### Non-Goals

- Нет drone-to-drone прямых команд без GCS посредника.
- Нет distributed consensus без надёжной сети.
- Нет certified synchronization.
- Нет real-time formation feedback без telemetry loop.

### Done Criteria

- `SwarmMission` производит N per-agent `MavlinkCommonPlan`.
- Duplicate ownership → validation error.
- `ArmAll` тест: N mock connections → N arm commands отправлены.
- `TakeoffAll` тест: jitter между командами корректен.
- `AbortAll` тест: RTL отправлен всем без ожидания ack.
- `SwarmCommandReport` сериализуется с per-agent статусами.
- Formation waypoints тест: ведомый получает lead + offset.

### Automated Tests

#### Tests That Need No Refactoring

- `swarm_mission_produces_per_agent_plans`.
- `duplicate_ownership_fails_validation`.
- `arm_all_sends_arm_to_each_agent`.
- `takeoff_all_jitter_within_window`.
- `abort_all_sends_rtl_without_waiting_ack`.
- `swarm_report_partial_on_one_timeout`.
- `formation_waypoints_offset_applied_correctly`.
- `failed_agent_releases_owned_tasks_and_segments`.

#### Tests That Need Light Refactoring

- Multi-agent mock connection fixture builder.
- Swarm command assertion helper.
- Formation plan assertion helper.

#### Tests That Need Heavy Refactoring

- Local PX4/SIH 2-agent sync takeoff test.
- Abort-all stress: 8 agents, verify all receive RTL.
- CBBA/gossip integration through swarm command events.

---

## M87 — Swarm Topologies, Mothership + Transport Abstraction

### Goal

Сделать паттерны координации роя явными в коде, не hand-waving.

Topology влияет на supervisor policy, command routing и failure handling.
Mothership/carrier — специализированный topology паттерн для развёртывания
под-дронов. DroneLink abstraction создаёт транспортную основу для future
P2P без привязки к конкретному radio.

### Scope

1. Swarm topology types:
   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "kind")]
   pub enum SwarmTopology {
       CentralizedGcs,
       P2PLogical,
       Mothership(MothershipTopology),
       RelayMesh(RelayMeshTopology),
   }

   pub struct MothershipTopology {
       pub carrier_agent_id: AgentId,
       pub staging_waypoint: CommandPosition,
       pub deploy_policy: DeployPolicy,
       pub collect_policy: CollectPolicy,
   }

   pub struct RelayMeshTopology {
       pub relay_agent_ids: Vec<AgentId>,
       pub link_availability: HashMap<AgentId, Vec<AgentId>>,
       pub min_relay_count: usize,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum DeployPolicy {
       OnCarrierArrival,
       ExplicitTick(u64),
   }
   ```

2. Mothership mission mechanics:
   - Carrier агент летит к `staging_waypoint`.
   - При `deploy_policy` event: sub-agent задачи переходят из `Pending` в
     `Unassigned` → аллокатор включает их.
   - Carrier ждёт или летит patrol пока sub-agents выполняют миссию.
   - При завершении: `collect_policy` инициирует возврат carrier.
   - Replay events: `CarrierArrived`, `SubMissionsDeployed`,
     `SubMissionsCollected`, `CarrierReturning`.
   - Abort policy: sub-agents не успели → `Abandoned`.

3. `DroneLink` transport abstraction:
   ```rust
   pub trait DroneLink: Send {
       type Error: std::error::Error + Send;
       fn send(&mut self, to: &AgentId, msg: RawMessage) -> Result<(), Self::Error>;
       fn recv(&mut self) -> Result<Option<(AgentId, RawMessage)>, Self::Error>;
       fn local_id(&self) -> &AgentId;
   }
   ```

   Implementations:
   - `SimulatedDroneLink` — wraps `InMemAgentTransport` (backward compat);
   - `UdpDroneLink` — UDP unicast/broadcast для SITL;
   - `SerialDroneLink` — placeholder, возвращает `Err(NotImplemented)`;
   - `NullDroneLink` — для тестов, отбрасывает все сообщения.

4. `DroneLinkConfig` для scenario DSL:
   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "kind")]
   pub enum DroneLinkConfig {
       Simulated,
       Udp { bind_addr: String, peers: BTreeMap<AgentId, String> },
       Serial { path: String, baud: u32 },
   }
   ```

5. Topology-aware artifacts:
   - `topology: SwarmTopology` в run report;
   - link assumptions записаны явно;
   - topology caveats в artifacts;
   - Mesh/relay документировано как coordination abstraction, не RF truth.

6. AgentNode адаптируется к DroneLink:
   - `AgentNode<T: DroneLink>` вместо `AgentNode<T: Transport>`;
   - существующие тесты используют `SimulatedDroneLink`.

### Non-Goals

- Нет radio protocol или antenna/RSSI model.
- Нет guarantee что relay placement работает в physical RF.
- Нет certified lost-link behavior.
- Нет multi-level carrier hierarchy (carrier of carriers).

### Done Criteria

- `SwarmTopology` сериализуется и влияет на supervisor policy.
- Mothership mission: sub-tasks активируются после CarrierArrived.
- Replay содержит deploy/collect события с правильными tick'ами.
- `DroneLink` trait определён, все четыре реализации компилируются.
- CBBA тесты проходят с `SimulatedDroneLink` без изменений алгоритма.
- `UdpDroneLink` loopback тест работает.
- `SerialDroneLink` компилируется и возвращает `NotImplemented`.
- Docs явно документируют что mesh — coordination abstraction, не RF.

### Automated Tests

#### Tests That Need No Refactoring

- `centralized_topology_dispatches_through_gcs`.
- `mothership_sub_tasks_pending_before_carrier_arrival`.
- `mothership_sub_tasks_activate_on_carrier_arrived_event`.
- `mothership_replay_contains_deploy_and_collect_events`.
- `relay_mesh_link_loss_changes_availability`.
- `simulated_drone_link_send_recv_roundtrip`.
- `null_drone_link_drops_all_messages`.
- `serial_drone_link_returns_not_implemented`.
- `drone_link_config_serde_roundtrip`.
- `cbba_converges_over_simulated_drone_link`.
- `docs_smoke_mesh_not_rf_implementation`.

#### Tests That Need Light Refactoring

- Topology fixture builders.
- Mothership scenario builder.
- DroneLink test harness.

#### Tests That Need Heavy Refactoring

- CBBA convergence over UdpDroneLink (два потока).
- Full carrier + 2 sub-drones end-to-end simulation.
- Topology-aware CBBA/gossip benchmark.

---

## M88 — SITL Dual-Stack + Hardware-Entry Evidence Pack

### Goal

Подготовить дисциплину evidence которая нужна перед любым реальным
hardware-экспериментом, и убедиться что оба SITL пути (PX4 и ArduPilot)
поддерживаются без деградации.

```text
Dry-run artifact + MAVLink plan + profile + preflight + ACK contract
  = complete evidence pack that can be reviewed before any vehicle run.
```

### Scope

1. Evidence pack schema:
   ```rust
   pub struct MissionEvidencePack {
       pub schema_version: String,
       pub mission_id: String,
       pub git_commit: String,
       pub created_at: DateTime<Utc>,
       pub command_ir_summary: CommandIrSummary,
       pub mavlink_plan_summary: MavlinkPlanSummary,
       pub capability_profile: AutopilotStack,
       pub preflight_report: SafetyValidationReport,
       pub expected_ack_contract: Vec<ExpectedAck>,
       pub artifact_validation: ArtifactValidationReport,
       pub replay_summary: Option<String>,
       pub run_command: String,
       pub caveats: Vec<String>,
       pub limitations: Vec<String>,
   }
   ```

2. Evidence pack generation:
   - `generate_evidence_pack(plan, compiled_plan, profile, preflight)`
     → `MissionEvidencePack`;
   - записывается в output dir как `mission_evidence_pack.v1.json`;
   - artifact validator обновляется для проверки evidence pack.

3. PX4 path — сохранение:
   - существующий PX4/SIH workflow (M48/M58/M59) не ломается;
   - primitive missions routing через M81 compiler;
   - hardware gates и runbooks остаются консервативными;
   - all existing SITL tests remain green.

4. ArduPilot path — scaffolding:
   - ArduPilot profile готов (M82);
   - dry-run compilation через M81 с ArduPilot profile;
   - `docs/ARDUPILOT_SITL.md` — runbook (команды, setup, известные отличия);
   - optional local SITL harness: `scripts/run_ardupilot_local.sh` если
     зависимости управляемы;
   - automated тесты не требуют установленного ArduPilot.

5. Hardware-entry checklist (дополнение к M79):
   - selected autopilot and version;
   - selected airframe and variant;
   - selected link type (serial/UDP/...);
   - selected coordinate frame/local origin policy;
   - selected altitude reference;
   - geofence and failsafe parameters verified on FC;
   - manual kill/abort procedure rehearsed;
   - first mission MUST be primitive takeoff-hold-land or dry-run only;
   - evidence pack validated for intended mission;
   - `docs/HARDWARE_READINESS.md` updated with M80-M87 capabilities.

6. Result interpretation document:
   - dry-run success = command plan is valid, не flight success;
   - SITL success = command upload accepted by simulator, не real airframe;
   - `UnsupportedPrimitive` = нужен fallback или другой profile;
   - profile caveat = поведение зависит от firmware версии;
   - ACK mismatch = live run может отличаться от dry-run.

### Non-Goals

- Нет real hardware flight в этом milestone.
- Нет production certification.
- Нет operator training claim.
- Нет assumption что SITL = real airframe.

### Done Criteria

- Evidence pack schema существует, сериализуется, валидируется.
- Primitive и Urban missions могут производить evidence packs.
- Artifact validator проверяет evidence pack структуру.
- PX4 dry-run пути не регрессируют.
- ArduPilot profile dry-run компилирует core primitive mission.
- `docs/ARDUPILOT_SITL.md` содержит runbook commands.
- `docs/HARDWARE_READINESS.md` обновлён с M80-M87 Verified Scope.
- Automated тесты зелёные без PX4/ArduPilot installation.

### Automated Tests

#### Tests That Need No Refactoring

- `evidence_pack_validates_for_primitive_mission`.
- `evidence_pack_validates_for_urban_perimeter_mission`.
- `missing_preflight_report_fails_evidence_validation`.
- `unsupported_primitive_in_plan_requires_caveat_in_evidence`.
- `px4_profile_dry_run_compiles_hover_mission`.
- `ardupilot_profile_dry_run_compiles_hover_mission`.
- `profile_differences_visible_in_evidence_pack`.
- `docs_smoke_ardupilot_sitl_runbook_commands`.
- `docs_smoke_hardware_entry_checklist_sections`.
- `existing_px4_dry_run_tests_still_pass`.

#### Tests That Need Light Refactoring

- Artifact validator subcommand для evidence packs.
- Report exporter для evidence pack summaries.
- Dual-stack dry-run comparison helper.

#### Tests That Need Heavy Refactoring

- Dual PX4/ArduPilot SITL evidence runs.
- End-to-end command upload state machine под mocked ACK/telemetry.
- Versioned evidence schema across all mission families.

---

## Ожидаемый уровень после M80–M88

После этого плана проект всё ещё не является:

- production drone system;
- certified safety stack;
- real perception system;
- hardware-proven swarm controller;
- real RF mesh networking stack;
- ready for uncontrolled field use.

Но он станет значительно серьёзнее как pre-hardware platform:

- mission intent независима от hardware (command IR);
- command plans детерминированы и machine-reviewable;
- PX4 и ArduPilot различия явны, не скрыты;
- Urban маршруты привязаны к GPS-координатам реального города;
- несколько дронов не конфликтуют на одном сегменте;
- geofence загружается на борт, не только проверяется simulation-side;
- параметры FC конфигурируются программно перед миссией;
- рой поднимается синхронно с единого GCS;
- mothership паттерн тестируется в симуляции;
- topology моделей явно описана, не хардкодена;
- алгоритмы готовы к замене транспорта на реальный radio-layer.

Когда железо появится, следующий этап начинается отдельным планом:

```text
primitive takeoff-hold-land dry-run на выбранном железе
  -> telemetry read-back verification
    -> geofence upload + verify
      -> parameter verification
        -> single-drone constrained flight
          -> multi-drone только после отдельного safety review
```

Ценность M80–M88 в том, что этот этап начинается с disciplined,
evidence-backed foundation вместо improvised scripts и unclear claims.

## Вещи которые не делать в этом плане

- Не реализовывать RF/radio P2P без конкретного radio-железа.
- Не строить full GIS движок или navmesh engine.
- Не добавлять real physics или sensor simulation.
- Не делать UI/visualizer как readiness requirement.
- Не делать long benchmark reruns без новых behavior или новых
  interpretation вопросов.
- Не обещать semver-stable API до стабилизации hardware/backend boundaries.
- Не переходить к multi-drone hardware до single-drone controlled
  hardware planning.
- Не добавлять PX4-only или ArduPilot-only hidden behavior в generic
  mission primitives.
