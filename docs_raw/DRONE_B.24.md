# DRONE_B.24 — Следующая фаза: Urban, MAVLink и рой

Дата фиксации: 2026-06-03

Источник: обсуждение post-M79, анализ текущего кодовой базы.

Этот документ фиксирует план следующей фазы развития проекта после завершения
M70–M79. M70–M79 подняли проект до `hardware-integration candidate`. Теперь цель
другая:

```text
Сделать проект способным к управлению реальным роем дронов в реальной
городской среде с использованием стандартного MAVLink-стека.
```

Акцент на:

1. **Urban** — реалистичная городская среда с реальными GPS-координатами,
   деконфликтом маршрутов нескольких дронов и временно-зависимыми
   ограничениями.
2. **MAVLink Common dialect** — реальный код для реального железа, работающий
   на PX4 и ArduPilot без fork per-autopilot.
3. **Рой** — координация нескольких дронов через единый GCS, паттерн
   mothership/carrier, транспортная абстракция для P2P gossip.

## Архитектурная граница (без изменений)

PX4/ArduPilot owns:

- stabilization, attitude/rate control, motor physics;
- low-level waypoint following;
- flight failsafes и параметры безопасности борта;
- RF link management.

This project owns:

- mission-level planning and route export;
- task allocation and reallocation;
- MAVLink mission/param/fence upload protocol;
- preflight validation and artifact evidence;
- swarm coordination via GCS (не drone-to-drone RF);
- replay, metrics, benchmark evidence.

## Target State After M80–M87

После этого плана проект должен уметь:

- строить Urban граф из реальных GPS-координат (OSM-совместимый формат);
- координировать несколько дронов в городском пространстве без коллизий на
  сегментах;
- загружать geofence прямо на борт через MAVLink;
- читать и писать параметры FC программно перед миссией;
- запускать arm/takeoff/execute на PX4 и ArduPilot через один код с диалектной
  адаптацией;
- синхронно поднимать рой из N дронов с единого GCS;
- выполнять миссию mothership: долететь до зоны, развернуть под-дроны,
  собрать их;
- запускать CBBA и gossip через любой транспортный слой (InMem / UDP / Serial).

## Non-Goals

- Нет drone-to-drone RF протокола без конкретного radio-железа.
- Нет certified obstacle avoidance.
- Нет real lidar/SLAM/CV.
- Нет real-time distributed consensus без latency-bounded network.
- Нет производственной сертификации.
- Нет semver-стабильного публичного API до стабилизации boundaries.

## Milestone Chain

```text
M80 Urban Geo-Referenced Graph
  -> M81 Urban Multi-Agent Route Deconfliction
    -> M82 MAVLink Geofence Upload
      -> M83 FC Parameter Management
        -> M84 ArduPilot Compatibility Layer
          -> M85 Synchronized GCS-Swarm Commands
            -> M86 Mothership / Carrier Mission Pattern
              -> M87 DroneLink Transport Abstraction
```

Почему такой порядок:

1. Сначала делаем Urban граф geo-referenced — это фундамент для любого
   реального городского применения.
2. Деконфликт маршрутов требует реального графа и нескольких агентов.
3. Geofence upload на борт — следующий MAVLink-шаг после mission upload.
4. Управление параметрами FC дополняет geofence и нужно для pre-mission
   конфигурации.
5. ArduPilot-совместимость расширяет hardware target без нового mission кода.
6. Синхронный рой строится поверх стабильного MAVLink-стека.
7. Mothership требует синхронного запуска и зависимых задач.
8. DroneLink-абстракция — последний шаг: делает алгоритмы (CBBA, gossip)
   независимыми от конкретного транспорта.

---

## M80 — Urban Geo-Referenced Graph

### Goal

Перейти от абстрактных локальных координат Urban-графа к реальным
GPS-координатам в узлах. Это делает Urban-маршруты применимыми к конкретным
реальным местам без ручного выбора geo_origin.

```text
GPS-координаты в UrbanNode -> export -> WGS84 waypoints без offset-арифметики
```

### Scope

1. Расширение `UrbanNode`:
   ```rust
   pub struct UrbanNode {
       pub id: UrbanNodeId,
       pub pose: Pose,                   // local coords (backward compat)
       pub geo: Option<UrbanGeoPoint>,   // GPS coords if geo-referenced
   }

   pub struct UrbanGeoPoint {
       pub lat_deg: f64,
       pub lon_deg: f64,
       pub alt_m: f64,
   }
   ```
   - Если `geo` присутствует, route export использует его напрямую.
   - Если отсутствует, поведение прежнее: local pose + geo_origin offset.
   - Валидация: если хотя бы один узел имеет `geo`, все должны иметь (error).

2. Обновление route export:
   - `export_route_loop_to_waypoints` проверяет наличие `geo` на узлах.
   - Geo-referenced граф производит waypoints без `geo_origin` сдвига.
   - Non-geo граф работает как прежде.
   - Export artifact записывает `coordinate_mode: "geo_referenced"` или
     `"local_with_origin"`.

3. GeoJSON import utility:
   - `parse_urban_map_geojson(input: &str) -> Result<UrbanMap, UrbanImportError>`:
     читает LineString features как рёбра, Point features как узлы;
   - сохраняет `geo` в каждом `UrbanNode`;
   - вычисляет `pose` из `geo` через метрическое приближение для backward compat
     с simulation layer.
   - Это utility function, не обязательный production path.

4. Scenario fixture реального города:
   - `scenarios/urban.geo-referenced.json` — небольшой фрагмент реального
     квартала (5–10 узлов) с реальными lat/lon из OSM;
   - полностью portable, не зависит от внешних данных во время теста;
   - демонстрирует что export производит корректные WGS84 waypoints.

5. Документация:
   - разделить понятия "geo-referenced Urban graph" и "local simulation
     graph";
   - явно указать что GeoJSON import — utility, не validated production path;
   - указать ограничение: проект не является GIS-движком.

### Non-Goals

- Нет full OSM parser.
- Нет polygon/navmesh/geometry engine.
- Нет real obstacle avoidance поверх geo-referenced графа.
- Нет elevation model или terrain-following.
- Нет реального геодезического projection (только метрическое приближение
  малых углов).

### Done Criteria

- `UrbanNode` с `geo: Some(...)` производит корректные WGS84 waypoints в
  export без geo_origin.
- Non-geo граф (existing) работает без изменений.
- GeoJSON utility парсит simple fixture без panic.
- `scenarios/urban.geo-referenced.json` загружается, валидируется, экспортирует
  корректный waypoint план в dry-run.
- `coordinate_mode` записан в export artifact.

### Automated Tests

#### Tests That Need No Refactoring

- `geo_referenced_node_export_uses_node_geo_directly`: узел с `geo` →
  waypoint lat/lon совпадают с узловыми, без geo_origin сдвига.
- `mixed_geo_nodes_fail_validation`: карта с частично заполненным `geo` →
  validation error.
- `local_node_export_unchanged`: узел без `geo` → behavior идентичен текущему.
- `geojson_import_roundtrip`: parse fixture → UrbanMap → export → geo
  совпадает с исходными координатами.
- `geo_referenced_export_artifact_records_coordinate_mode`: artifact
  содержит `"coordinate_mode": "geo_referenced"`.

#### Tests That Need Light Refactoring

- Shared geo-node fixture builder (lat/lon → UrbanNode + Pose).
- Export assertion helper для проверки WGS84 точности.
- GeoJSON fixture helper с несколькими вариантами топологии.

#### Tests That Need Heavy Refactoring

- Property tests: round-trip geo → pose → geo для малых смещений.
- Integration test: geo-referenced dry-run через полный sitl_agent pipeline.
- Реальный OSM фрагмент как regression-stable fixture.

---

## M81 — Urban Multi-Agent Route Deconfliction

### Goal

Несколько дронов летят по одному Urban-графу без одновременного занятия
одного сегмента. Это mission-level deconfliction, не physical collision
avoidance.

```text
два агента -> общий сегмент -> один ждёт -> replay объясняет решение
```

### Scope

1. Segment ownership registry:
   ```rust
   pub struct UrbanSegmentLock {
       pub edge_id: UrbanEdgeId,
       pub held_by: AgentId,
       pub acquired_at_tick: u64,
   }
   ```
   - В начале каждого тика patrol runner резервирует следующий сегмент.
   - Если сегмент уже занят другим агентом — применяется policy.
   - По завершении сегмента lock снимается.

2. Right-of-way policies:
   - `FirstCome` — первый захватил, второй ждёт;
   - `Priority` — агент с большим `priority` field проходит первым;
   - `RoundRobin` — чередование при повторных конфликтах.
   - Policy задаётся в `UrbanState.deconflict_policy`.

3. Replay events:
   - `UrbanSegmentLockAcquired { agent_id, edge_id, tick }`;
   - `UrbanSegmentLockReleased { agent_id, edge_id, tick }`;
   - `UrbanSegmentConflict { agents: Vec<AgentId>, edge_id, tick, winner }`;
   - `UrbanDeconflictWait { agent_id, edge_id, tick }`.

4. Metrics:
   - `urban_segment_conflict_count: u64`;
   - `urban_deconflict_wait_ticks: u64`;
   - `urban_segment_utilization: f64` — доля тиков когда сегмент был занят.

5. Scenario fixtures:
   - `scenarios/urban.multi-agent.deconflict.json` — два агента на
     перекрывающихся маршрутах;
   - три профиля: FirstCome, Priority, RoundRobin.

### Non-Goals

- Нет physical collision avoidance.
- Нет multi-height 3D deconfliction.
- Нет real-time RF coordination между дронами.
- Нет yield policy основанной на внешнем трафике или людях.

### Done Criteria

- Два агента на пересекающемся маршруте не занимают один сегмент одновременно.
- Replay содержит lock/conflict/wait события.
- Все три policy проходят детерминированные тесты.
- Метрики conflict_count и wait_ticks ненулевые на конфликтном сценарии.
- Одноагентный сценарий работает без изменений.

### Automated Tests

#### Tests That Need No Refactoring

- `segment_lock_exclusive`: два агента запрашивают один сегмент →
  только один получает lock.
- `first_come_policy_respects_arrival_order`: агент прибывший первым
  проходит первым.
- `priority_policy_prefers_higher_priority_agent`: агент с большим priority
  не ждёт.
- `lock_released_after_segment_complete`: после прохождения сегмента lock
  снимается, метрика utilization обновляется.
- `replay_contains_conflict_event`: конфликтный сценарий → event лог
  содержит `UrbanSegmentConflict`.
- `single_agent_no_conflicts`: один агент работает без изменений.

#### Tests That Need Light Refactoring

- Multi-agent urban scenario builder.
- Segment lock assertion helper.
- Deconfliction replay event fixture.

#### Tests That Need Heavy Refactoring

- Property tests: N агентов на random топологии, no simultaneous lock invariant.
- Stress test: 8 агентов на малом графе, все policy варианты.
- Regression: single-agent throughput не деградирует при включённом deconfliction.

---

## M82 — MAVLink Geofence Upload

### Goal

Загружать geofence прямо на борт FC через MAVLink, а не только проверять его
на стороне проекта в preflight. Это первый шаг к полноценному pre-flight
hardware contract.

```text
SafetyConfig.geofence -> MAVLink FENCE messages -> FC enforces fence in hardware
```

### Scope

1. Типы geofence для upload:
   ```rust
   pub enum FenceUploadItem {
       CircularInclusion { center: Waypoint, radius_m: f32 },
       PolygonInclusion { vertices: Vec<Waypoint> },
       PolygonExclusion { vertices: Vec<Waypoint> },
   }
   ```

2. MAVLink-сообщения (Common dialect):
   - `FENCE_POINT` (legacy circular fence, для ArduPilot backward compat);
   - `MISSION_ITEM_INT` с `MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION`,
     `MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION`,
     `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION`,
     `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION`;
   - `MAV_CMD_DO_FENCE_ENABLE` для активации после upload.

3. `upload_geofence` функция в `MavlinkTransport`:
   - принимает `&[FenceUploadItem]`;
   - использует тот же MISSION_COUNT/REQUEST/ACK handshake что и mission upload
     (fence items — это тоже mission items в MAVLink v2);
   - возвращает `FenceUploadReport` с count, ack.

4. Dry-run artifact:
   - `sitl_dry_run_artifact.v1.json` расширяется полем
     `geofence_items: Option<Vec<FenceItemSummary>>`;
   - `FenceItemSummary` содержит shape type, vertex count, radius.

5. Pre-mission validation:
   - `SafetyConfig.geofence` → `FenceUploadItem::PolygonInclusion`;
   - Если geofence задан и mode = execute, проект загружает fence перед mission.
   - Если upload fails → mission abort, exit code 2.

### Non-Goals

- Нет geofence download/read-back verification.
- Нет certified geofence enforcement claim.
- Нет runtime geofence breach handling на стороне проекта (FC сам обрабатывает).
- Нет complex geofence shapes (только convex polygon и circle).

### Done Criteria

- `upload_geofence` компилируется под `mavlink-transport` feature.
- Dry-run artifact содержит fence summary.
- Mock upload test проверяет правильные MISSION_ITEM_INT команды для каждого
  типа fence.
- `MAV_CMD_DO_FENCE_ENABLE` отправляется после успешного fence upload.
- Preflight rule `geofence.waypoint_outside` остаётся — fence upload
  дополняет, не заменяет software-side check.

### Automated Tests

#### Tests That Need No Refactoring

- `fence_upload_circular_sends_correct_mavlink_command`: circle inclusion →
  `MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION` с правильными param1/x/y/z.
- `fence_upload_polygon_sends_vertex_items`: N-vertex polygon →
  N items с `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION`.
- `fence_upload_enable_command_sent_after_items`: после fence ACK →
  `MAV_CMD_DO_FENCE_ENABLE` отправляется.
- `dry_run_artifact_contains_fence_summary`: dry-run с geofence config →
  artifact содержит `geofence_items`.
- `fence_upload_failure_aborts_mission`: rejected fence ACK → exit code 2.

#### Tests That Need Light Refactoring

- Shared fence item MAVLink assertion helper.
- Mock transport fence capture helper.
- Artifact fence assertion helper.

#### Tests That Need Heavy Refactoring

- Integration: full dry-run → fence upload → mission upload sequence.
- Local PX4/SIH manual test: загрузить fence, проверить что FC принял.
- ArduPilot FENCE_POINT legacy path tests.

---

## M83 — FC Parameter Management

### Goal

Читать и писать параметры FC программно через MAVLink перед миссией. Это
позволяет автоматически настраивать скорость, высоту RTL и failsafe под
конкретную миссию без ручного доступа к QGC.

```text
pre-mission config -> PARAM_REQUEST_LIST / PARAM_SET -> FC -> param ack
```

### Scope

1. Типы параметров:
   ```rust
   pub enum FcParamValue {
       Int32(i32),
       Float(f32),
   }

   pub struct FcParam {
       pub id: String,          // param name, max 16 chars (MAVLink limit)
       pub value: FcParamValue,
       pub index: u16,
   }
   ```

2. API в `MavlinkTransport`:
   - `read_param(id: &str) -> Result<FcParam, ParamError>` —
     `PARAM_REQUEST_READ` + ожидание `PARAM_VALUE`;
   - `write_param(id: &str, value: FcParamValue) -> Result<FcParam, ParamError>` —
     `PARAM_SET` + ожидание `PARAM_VALUE` ack;
   - `read_all_params() -> Result<Vec<FcParam>, ParamError>` —
     `PARAM_REQUEST_LIST` + сбор всех `PARAM_VALUE`;
   - timeout и retry следуют тому же паттерну что mission upload.

3. Known-param registry:
   ```rust
   pub mod known_params {
       pub const MPC_XY_CRUISE: &str = "MPC_XY_CRUISE";   // PX4 horiz speed
       pub const WPNAV_SPEED:   &str = "WPNAV_SPEED";     // ArduCopter speed
       pub const RTL_ALT:       &str = "RTL_ALT";         // ArduPilot RTL alt
       pub const MIS_TAKEOFF_ALT: &str = "MIS_TAKEOFF_ALT"; // PX4 takeoff alt
       // ...
   }
   ```
   - Только строки-константы, не генерирует код.
   - Каждая константа сопровождается doc-комментарием: dialect, единицы, диапазон.

4. Pre-mission param validation:
   - Новая секция в `SafetyConfig`: `param_requirements: Vec<ParamRequirement>`;
   - `ParamRequirement { param_id, min: Option<f32>, max: Option<f32> }`;
   - Preflight читает params с борта (только в execute mode), проверяет bounds;
   - Если FC недоступен в dry-run → skip с warning.

5. Param snapshot в artifact:
   - `params_snapshot: Option<Vec<FcParamSummary>>` в run report;
   - фиксирует значения key params на момент запуска миссии.

### Non-Goals

- Нет full FC configuration management system.
- Нет param backup/restore.
- Нет parameter migration между версиями прошивки.
- Нет runtime param change во время выполнения миссии.

### Done Criteria

- `read_param` и `write_param` компилируются под `mavlink-transport`.
- Mock тесты проверяют правильные MAVLink сообщения.
- `known_params` содержит минимум 5 documented параметров для PX4 и ArduPilot.
- Dry-run с `param_requirements` пропускает param read без паники.
- Artifact param snapshot сериализуется без потерь.

### Automated Tests

#### Tests That Need No Refactoring

- `read_param_sends_request_and_parses_value`: mock conn возвращает
  `PARAM_VALUE` → `FcParam` корректно populated.
- `write_param_sends_param_set_and_awaits_ack`: `PARAM_SET` отправлен с
  правильным id/value, ack получен.
- `read_param_timeout_returns_error`: нет ответа → `ParamError::Timeout`.
- `param_requirement_passes_within_bounds`: param value в bounds → no violation.
- `param_requirement_fails_outside_bounds`: param value за bounds → violation.
- `param_snapshot_roundtrip_json`: сериализация/десериализация без потерь.

#### Tests That Need Light Refactoring

- Mock conn param fixture builder.
- Param assertion helper.
- Safety config param requirement helper.

#### Tests That Need Heavy Refactoring

- Read_all_params на большом наборе (100+ params).
- Local PX4/SIH manual test: read WPNAV_SPEED / MPC_XY_CRUISE.
- ArduPilot vs PX4 param name equivalence table tests.

---

## M84 — ArduPilot Compatibility Layer

### Goal

Arm/takeoff/execute sequence работает на ArduPilot через тот же code path что
и PX4. Один код, один CI, два autopilot.

ArduPilot и PX4 используют один MAVLink Common dialect, но по-разному
оркестрируют режимы и команды.

```text
AutopilotDialect::ArduPilot -> set GUIDED mode -> arm -> takeoff -> set AUTO mode
AutopilotDialect::Px4       -> arm -> MAV_CMD_NAV_TAKEOFF -> MISSION_START
```

### Scope

1. Тип диалекта:
   ```rust
   #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum AutopilotDialect {
       Px4,
       ArduPilot,
   }

   impl Default for AutopilotDialect {
       fn default() -> Self { Self::Px4 }
   }
   ```

2. Адаптация lifecycle команд:
   - **PX4** (текущее): arm → `MAV_CMD_NAV_TAKEOFF` → `MAV_CMD_MISSION_START`.
   - **ArduPilot**: `MAV_CMD_DO_SET_MODE (GUIDED)` → arm →
     `MAV_CMD_NAV_TAKEOFF` → `MAV_CMD_DO_SET_MODE (AUTO)`.
   - `dialect` поле в `MissionLifecycleOptions`.
   - Общая логика максимально shared; различия изолированы в функции
     `send_pre_mission_commands(dialect, conn, options)`.

3. Heartbeat system_id detection:
   - `AutopilotDialect::detect_from_heartbeat(heartbeat: &HEARTBEAT_DATA)` —
     определяет диалект по `autopilot` полю:
     `MAV_AUTOPILOT_ARDUPILOTMEGA` → ArduPilot,
     `MAV_AUTOPILOT_PX4` → Px4.
   - Возможность auto-detect вместо явного указания.

4. CLI и сценарии:
   - `--dialect px4|ardupilot|auto` флаг в `sitl_agent`;
   - `autopilot_dialect: Option<AutopilotDialect>` в `MissionLifecycleOptions`;
   - Default → Px4 для backward compat.

5. Support matrix обновление:
   - новая колонка `dialect` в support matrix;
   - PX4: как было;
   - ArduPilot: `experimental` до первого live-теста.

### Non-Goals

- Нет поддержки других autopilot (Betaflight, Cleanflight и т.д.).
- Нет runtime switching диалекта в середине миссии.
- Нет ArduPilot-специфичных mission items (только Common dialect).
- Нет hardware verification claim для ArduPilot.

### Done Criteria

- `AutopilotDialect` сериализуется/десериализуется в snake_case.
- Lifecycle tests для обоих диалектов: правильные команды в правильном порядке.
- `detect_from_heartbeat` возвращает правильный диалект для обоих autopilot.
- `--dialect` флаг работает в CLI.
- Support matrix содержит ArduPilot строки с `experimental` статусом.
- Существующие PX4 тесты не сломаны.

### Automated Tests

#### Tests That Need No Refactoring

- `px4_lifecycle_sends_expected_commands`: arm → takeoff → start в правильном порядке.
- `ardupilot_lifecycle_sets_guided_before_arm`: `DO_SET_MODE GUIDED` отправляется
  до arm.
- `ardupilot_lifecycle_sets_auto_after_takeoff`: `DO_SET_MODE AUTO` отправляется
  после takeoff ack.
- `detect_px4_from_heartbeat`: `MAV_AUTOPILOT_PX4` → `AutopilotDialect::Px4`.
- `detect_ardupilot_from_heartbeat`: `MAV_AUTOPILOT_ARDUPILOTMEGA` →
  `AutopilotDialect::ArduPilot`.
- `dialect_roundtrip_json`: сериализация `"ardupilot"` / `"px4"` без потерь.

#### Tests That Need Light Refactoring

- Shared lifecycle command assertion helper с dialect parameter.
- Mock conn dialect sequence fixture.

#### Tests That Need Heavy Refactoring

- Local ArduPilot SITL manual test (ArduCopter SITL).
- Dry-run equivalence: PX4 и ArduPilot produce same waypoint plan.
- Regression: все существующие PX4 SITL тесты проходят с dialect=px4.

---

## M85 — Synchronized GCS-Swarm Commands

### Goal

Единый GCS координирует синхронные операции роя: совместный взлёт, команды
всему рою одновременно, аварийная остановка всех.

```text
GCS -> arm all -> takeoff all (within window) -> execute -> abort all if needed
```

Это не P2P между дронами. Это centralized GCS coordination через отдельные
MAVLink соединения с каждым FC.

### Scope

1. Swarm operation types:
   ```rust
   pub enum SwarmCommand {
       ArmAll { timeout_per_drone: Duration },
       TakeoffAll { altitude_m: f32, sync_window_ms: u64 },
       AbortAll { reason: String },
       StartAll { sync_window_ms: u64 },
   }
   ```

2. `SwarmSupervisor` расширение:
   - `execute_swarm_command(&mut self, cmd: SwarmCommand) -> SwarmCommandReport`;
   - для `ArmAll`: arm каждый дрон параллельно (или в быстрой последовательности),
     собирает acks;
   - для `TakeoffAll`: send takeoff с небольшим jitter между дронами
     (`tick_offset_ms` per agent) чтобы избежать turbulence interference;
   - для `AbortAll`: RTL/disarm всем немедленно, не ждёт ack;
   - jitter и timeout задаются в конфиге.

3. `SwarmCommandReport`:
   - per-agent результат (acked / timeout / rejected);
   - общий статус (all_success / partial / failed);
   - latency от первого до последнего ack;
   - report входит в общий run report.

4. Formation waypoints (relative):
   ```rust
   pub struct FormationWaypoint {
       pub lead_waypoint: Waypoint,          // absolute position of lead drone
       pub offsets: Vec<(AgentId, Waypoint)>, // relative offsets for followers
   }
   ```
   - `build_formation_plan(lead_route, offsets)` → `Vec<(AgentId, SitlPlan)>`;
   - каждый ведомый получает lead waypoints + offset.

5. Abort-all reliability:
   - AbortAll отправляет `MAV_CMD_NAV_RETURN_TO_LAUNCH` каждому агенту;
   - не ждёт ack (fire-and-forget для максимальной скорости);
   - replay записывает sent/ack для каждого агента.

### Non-Goals

- Нет drone-to-drone прямых команд.
- Нет distributed consensus без надёжной сети.
- Нет formation flying с real-time position feedback без telemetry loop.
- Нет certified synchronization.

### Done Criteria

- `ArmAll` тест: N дронов через mock connections → все arm-команды отправлены.
- `TakeoffAll` тест: jitter между takeoff командами корректен.
- `AbortAll` тест: команды RTL отправлены всем без ожидания ack.
- `SwarmCommandReport` сериализуется с per-agent статусами.
- Formation waypoints тест: ведомый получает lead + offset.

### Automated Tests

#### Tests That Need No Refactoring

- `arm_all_sends_arm_to_each_agent`: N mock connections → N arm commands.
- `takeoff_all_respects_jitter_window`: timestamp разница между командами
  в заданном диапазоне.
- `abort_all_sends_rtl_without_waiting_ack`: RTL отправлен всем, функция
  возвращается немедленно.
- `swarm_report_partial_success_when_one_agent_fails`: один timeout →
  `partial` статус.
- `formation_waypoints_offset_applied`: ведомый waypoint = lead + offset.

#### Tests That Need Light Refactoring

- Multi-agent mock connection fixture builder.
- Swarm command assertion helper.
- Formation plan assertion helper.

#### Tests That Need Heavy Refactoring

- Local PX4/SIH test: 2-agent sync takeoff.
- Timing stability test: jitter bounds под load.
- Abort-all stress: 8 agents, verify all receive RTL.

---

## M86 — Mothership / Carrier Mission Pattern

### Goal

Один дрон-носитель летит в зону, разворачивает под-дроны (активирует их
миссии), ждёт завершения, собирает их. Это чисто mission-planning паттерн,
не физическая доставка.

```text
carrier flies to staging point
  -> deploy event fires -> sub-drone tasks activate
    -> sub-drones execute their missions
      -> collect event fires -> carrier returns
```

### Scope

1. `CarrierMission` тип в scenario DSL:
   ```rust
   pub struct CarrierMission {
       pub carrier_agent_id: AgentId,
       pub staging_waypoint: Waypoint,
       pub deploy_at_tick: Option<u64>,  // None = on arrival
       pub collect_at_tick: Option<u64>, // None = on sub-drone completion
       pub sub_missions: Vec<SubMissionRef>,
   }

   pub struct SubMissionRef {
       pub agent_id: AgentId,
       pub scenario_path: String,
       pub activate_after: ActivationTrigger,
   }

   pub enum ActivationTrigger {
       CarrierArrival,
       ExplicitTick(u64),
   }
   ```

2. Dependent task graph:
   - Sub-drone tasks имеют статус `Pending` до активации.
   - При deploy event: статус меняется на `Unassigned` → аллокатор включает их.
   - При collect event: не-завершённые sub-задачи получают `Abandoned`.

3. Replay events:
   - `CarrierArrived { agent_id, waypoint, tick }`;
   - `SubMissionsDeployed { carrier_id, sub_agent_ids, tick }`;
   - `SubMissionsCollected { carrier_id, tick, completed, abandoned }`;
   - `CarrierReturning { agent_id, tick }`.

4. Metrics:
   - `carrier_deployment_tick: Option<u64>`;
   - `carrier_collection_tick: Option<u64>`;
   - `sub_missions_completed: u64`;
   - `sub_missions_abandoned: u64`.

5. Scenario fixture:
   - `scenarios/carrier.urban-deploy.json` — 1 carrier + 2 sub-drones на
     urban маршрутах;
   - carrier летит к staging point, sub-дроны выполняют patching.

### Non-Goals

- Нет physical drone docking/attachment simulation.
- Нет real carrier deployment mechanism (battery swap, release mechanism).
- Нет multi-level carrier hierarchy (carrier of carriers).
- Нет real-time position-dependent deployment (только tick-based).

### Done Criteria

- Sub-drone задачи активируются после CarrierArrival события.
- Replay содержит deploy/collect события с правильными tick'ами.
- Carrier mission completes в детерминированном тесте.
- Метрики sub_missions_completed и abandoned корректны.
- Scenario fixture загружается и валидируется.

### Automated Tests

#### Tests That Need No Refactoring

- `sub_tasks_pending_before_carrier_arrival`: до arrival → задачи не
  аллоцируются.
- `sub_tasks_activate_on_carrier_arrival_event`: arrival event → статус Unassigned.
- `sub_tasks_abandoned_on_collect_if_incomplete`: collect event → незавершённые
  задачи Abandoned.
- `carrier_replay_contains_deploy_and_collect_events`: replay содержит оба события.
- `carrier_metrics_count_completed_sub_missions`: 2 sub-дрона завершают →
  `sub_missions_completed == 2`.

#### Tests That Need Light Refactoring

- Carrier mission scenario builder.
- Dependent task activation helper.
- Carrier replay event assertion helper.

#### Tests That Need Heavy Refactoring

- Simulation: полный carrier + 2 sub-дрона, end-to-end.
- Cancellation: carrier returns early → sub-дроны получают Abandoned.
- Urban + carrier: sub-дроны на geo-referenced маршрутах.

---

## M87 — DroneLink Transport Abstraction

### Goal

Алгоритмы рой-координации (CBBA, gossip) работают поверх любого транспорта:
InMem симуляции, UDP SITL, или Serial radio — без изменений алгоритмического кода.

```text
CBBA / gossip
    |
DroneLink trait
    |
SimulatedLink | UdpLink | SerialLink (placeholder)
```

### Scope

1. `DroneLink` trait:
   ```rust
   pub trait DroneLink: Send {
       type Error: std::error::Error + Send;

       fn send(&mut self, to: &AgentId, msg: RawMessage) -> Result<(), Self::Error>;
       fn recv(&mut self) -> Result<Option<(AgentId, RawMessage)>, Self::Error>;
       fn local_id(&self) -> &AgentId;
   }
   ```

2. Реализации:
   - `SimulatedDroneLink` — wraps существующий `InMemAgentTransport`
     (backward compat, без изменений simulation tests);
   - `UdpDroneLink` — UDP unicast/broadcast для SITL, использует
     `std::net::UdpSocket`;
   - `SerialDroneLink` — placeholder, компилируется без panic, возвращает
     `Err(NotImplemented)` на все операции;
   - `NullDroneLink` — для тестов, отбрасывает все сообщения.

3. Адаптация `AgentNode`:
   - `AgentNode<T: DroneLink>` вместо `AgentNode<T: Transport>`;
   - существующие тесты используют `SimulatedDroneLink` — изменения минимальны;
   - новые тесты используют `UdpDroneLink` для SITL.

4. Link configuration:
   ```rust
   pub enum DroneLinkConfig {
       Simulated,
       Udp { bind_addr: String, peers: HashMap<AgentId, String> },
       Serial { path: String, baud: u32 },
   }
   ```
   - Конфигурируется из сценария или CLI, не хардкодится.

5. Latency и loss model для `UdpDroneLink`:
   - `SimulatedLatency { min_ms, max_ms, loss_rate }` — инжектируется
     в UdpDroneLink для stress-тестов;
   - совместимо с существующим `packet_loss_rate` в RunConfig.

6. Документация:
   - явно указать что `SerialDroneLink` — placeholder для будущего radio-интерфейса;
   - для реального P2P нужен конкретный radio-layer (RFD900, LoRa и т.д.);
   - этот модуль создаёт архитектурную основу, не заменяет RF-проектирование.

### Non-Goals

- Нет RF-протокола или hardware radio в этом milestone.
- Нет автоматического discovery пиров.
- Нет mesh routing (это задача radio layer).
- Нет encryption или authentication.

### Done Criteria

- `DroneLink` trait определён и экспортирован.
- CBBA тесты проходят с `SimulatedDroneLink` без изменений алгоритма.
- `UdpDroneLink` двустороннее тест (два процесса / два сокета) работает.
- `SerialDroneLink` компилируется и возвращает `NotImplemented` без panic.
- `DroneLinkConfig` сериализуется/десериализуется.

### Automated Tests

#### Tests That Need No Refactoring

- `simulated_drone_link_sends_and_receives`: send + recv roundtrip в InMem.
- `null_drone_link_drops_all_messages`: recv всегда None.
- `serial_drone_link_returns_not_implemented`: send/recv → `Err(NotImplemented)`.
- `drone_link_config_roundtrip_json`: все варианты сериализуются без потерь.
- `cbba_converges_over_simulated_link`: существующий CBBA тест через новый
  DroneLink wrapper.

#### Tests That Need Light Refactoring

- Shared DroneLink test harness.
- CBBA fixture builder для DroneLink варианта.
- UdpDroneLink loopback test helper.

#### Tests That Need Heavy Refactoring

- CBBA convergence over UdpDroneLink (два потока в тесте).
- Latency injection: CBBA с 50ms simulated latency всё ещё сходится.
- Full swarm simulation via UdpDroneLink vs SimulatedDroneLink equivalence.

---

## Ожидаемый уровень после M80–M87

После этого плана проект всё ещё не является:

- production drone system;
- certified safety stack;
- real perception system с lidar/CV;
- hardware-proven swarm controller;
- production mesh networking stack.

Но он становится значительно ближе к управляемому реальному эксперименту:

- Urban маршруты привязаны к реальным GPS-координатам;
- несколько дронов в городе не конфликтуют на одном сегменте;
- geofence загружается на борт, не только проверяется simulation-side;
- FC параметры конфигурируются программно перед миссией;
- код работает на PX4 и ArduPilot без форка;
- рой поднимается синхронно с единого GCS;
- mothership паттерн тестируется в симуляции;
- алгоритмы готовы к замене транспорта на реальный radio-layer.

Следующий этап после появления железа начинается отдельным планом:

```text
single drone bench (no props)
  -> MAVLink connectivity + param read
    -> geofence upload verify
      -> mission upload + dry run
        -> controlled single-drone flight
          -> multi-drone only after separate safety review
```

## Вещи которые не надо делать в этом плане

- Не имплементировать RF/radio P2P без конкретного radio-железа.
- Не строить full GIS движок или navmesh engine.
- Не делать real physics или sensor simulation.
- Не добавлять UI/visualizer как readiness requirement.
- Не делать long benchmark reruns без новых behavior или новых interpretation
  вопросов.
- Не обещать semver-stable API до стабилизации hardware boundaries.
- Не переходить к multi-drone hardware до single-drone controlled experiment.
