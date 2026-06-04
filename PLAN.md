# PLAN.md - M84 Urban Geo-Referenced Mission Pack

## Context

Нужно запланировать реализацию `M84 - Urban Geo-Referenced Mission Pack` по
`docs_raw/DRONE_A.25.md`: Urban становится основным реалистичным mission
setting, а Urban-маршруты должны выходить не только в локальные waypoint export,
но и в реальные command IR / MAVLink Common dry-run artifacts.

Текущий HEAD уже закрывает фундамент:

- M80: `swarm-mission-ir` содержит `Position::Geo` и `Position::Local`
  ([position.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-mission-ir/src/position.rs:30)),
  `MissionCommand::FollowRoute`
  ([command.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-mission-ir/src/command.rs:37))
  и `MissionCommandPlan`
  ([plan.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-mission-ir/src/plan.rs:35)).
- M81/M82: `compile_mavlink_common_plan` уже умеет компилировать
  `Position::Geo` напрямую в `MISSION_ITEM_INT`-shape
  ([mavlink_common_plan.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-comms/src/mavlink_common_plan.rs:692))
  и локальные координаты через `home_origin`
  ([mavlink_coords.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-comms/src/mavlink_coords.rs:64)).
- M83: primitive dry-run artifacts уже включают `command_ir_summary`,
  `mavlink_common_plan`, ACKs/telemetry/policy/safety evidence.
- M70/M75: Urban уже имеет road graph, perimeter, mocked bus detector,
  temporary obstacles and blocked-route policy:
  `UrbanMap`, `UrbanNode`, `UrbanEdge`
  ([urban.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-types/src/urban.rs:96)),
  `UrbanSearchState`
  ([urban.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-types/src/urban.rs:248)),
  `UrbanBlockedPolicy`
  ([urban.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-types/src/urban.rs:294)).

Текущая проблема M84: Urban export пока строит только local simulation waypoints.
`UrbanRouteWaypoint` хранит `Pose`, а `urban_route_to_follow_route` всегда
создаёт `Position::Local`
([route_export.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-sim/src/urban/route_export.rs:190)).
`SitlDryRunArtifact` пишет `coordinate_frame`, `geo_origin`,
`effective_geo_origin`, `start_global/end_global`, но не пишет `coordinate_mode`
([sitl_plan.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_plan.rs:224)).

Короткие baseline-проверки на этапе планирования:

- `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban_route_to_follow_route` - PASS.
- `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples urban_patrol_plan_uses_route_export` - PASS.
- `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim geo_origin` - PASS.

## Investigation context

`INVESTIGATION.md` отсутствует, поэтому отдельного investigation artifact нет.
План построен по локальному коду, `docs_raw/DRONE_A.25.md`, README/docs и
коротким baseline-тестам выше. Notion/GitLab в пользовательском prompt не
упоминались, `notion_policy=optional`, поэтому внешние задачи/MR не читались.

## Affected components

- `crates/swarm-types/src/urban.rs`: добавить node-level WGS84 тип и поле у
  `UrbanNode`, расширить `UrbanMap::validate`.
- `crates/swarm-sim/src/urban/route_export.rs`: добавить coordinate mode,
  WGS84-aware route waypoint/export metadata и IR conversion.
- `crates/swarm-sim/src/urban/mod.rs`: экспортировать новые geo/GeoJSON helpers.
- Новый модуль `crates/swarm-sim/src/urban/geojson_import.rs`: маленький
  GeoJSON utility/testbed.
- `crates/swarm-sim/src/dsl/urban_validate.rs` и
  `crates/swarm-sim/src/dsl/tests.rs`: валидировать mixed geo/non-geo maps,
  новые mission templates and fixtures.
- `crates/swarm-sim/src/runner/types.rs`: добавить mission/template metadata в
  `UrbanState` без ломки старых сценариев.
- `crates/swarm-sim/src/runner/urban_patrol.rs`,
  `crates/swarm-sim/src/runner/urban_search.rs`,
  `crates/swarm-sim/src/runner/urban_events.rs`: сделать явными route decision /
  mocked perception события для M84 artifacts.
- `crates/swarm-examples/src/sitl_plan.rs`: записывать `coordinate_mode`, строить
  Urban command IR с WGS84 waypoint positions, сохранять route/mission metadata
  в dry-run artifact.
- `crates/swarm-examples/src/artifact_validator.rs`: добавить проверки
  coordinate mode / route metadata / mocked perception evidence для Urban
  dry-run artifacts.
- `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs`,
  `crates/swarm-examples/tests/artifact_validator.rs`,
  `crates/swarm-examples/tests/sitl_docs.rs`: dry-run artifact and docs smoke
  tests.
- `scenarios/`: добавить стабильные fixtures для geo Urban perimeter/block/search
  and GeoJSON import.
- `README.md`, `docs/STATUS.md`, `docs/SCENARIO_DSL.md`,
  `docs/SITL_SETUP.md`, `docs/MISSION_COMMAND_IR.md`,
  `docs/MAVLINK_COMMON_COMPILER.md`, `docs/ARTIFACT_VALIDATION.md`,
  `docs/REPLAY.md`, `docs/HARDWARE_READINESS.md`,
  `docs/EXTENSION_GUIDE.md`, `docs/OPERATIONAL_RUNBOOKS.md`: синхронизировать
  пользовательские claims, команды запуска, ограничения и validation rules.

## Implementation steps

1. Расширить Urban node model в
   [crates/swarm-types/src/urban.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-types/src/urban.rs:96).

   Материализуемый результат:

   - добавить `UrbanGeoPoint { lat_deg, lon_deg, alt_m }`;
   - добавить `#[serde(default, skip_serializing_if = "Option::is_none")] pub geo: Option<UrbanGeoPoint>` в `UrbanNode`;
   - сохранить backward compatibility: старые JSON без `geo` продолжают
     парситься как local-only maps;
   - расширить `UrbanMap::validate`
     ([urban.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-types/src/urban.rs:328)):
     все nodes либо имеют `geo`, либо все не имеют; mixed maps возвращают
     `UrbanMapValidationError` с полем `nodes[].geo`;
   - валидировать latitude/longitude/altitude как finite и WGS84 bounds.

   Контракт:

   ```rust
   #[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
   pub struct UrbanGeoPoint {
       pub lat_deg: f64,
       pub lon_deg: f64,
       pub alt_m: f64,
   }

   #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
   pub struct UrbanNode {
       pub id: UrbanNodeId,
       pub pose: Pose,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub geo: Option<UrbanGeoPoint>,
   }
   ```

2. Добавить coordinate mode и WGS84-aware Urban route export в
   [crates/swarm-sim/src/urban/route_export.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-sim/src/urban/route_export.rs:29).

   Материализуемый результат:

   - добавить `UrbanCoordinateMode::{LocalWithOrigin, Wgs84NodeGeo}`;
   - добавить `coordinate_mode` в `UrbanRouteExportMetadata`;
   - добавить `geo: Option<UrbanGeoPoint>` в `UrbanRouteWaypoint`;
   - для geo maps waypoint на destination node должен не интерполировать local
     segment points, а использовать node geo directly для route-node export;
   - для local maps сохранить текущую densification/spacing логику и task ids;
   - `urban_route_to_follow_route`
     ([route_export.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-sim/src/urban/route_export.rs:198))
     должен создавать `Position::Geo` при `coordinate_mode=Wgs84NodeGeo`, иначе
     `Position::Local`.

   Псевдокод:

   ```rust
   let mode = UrbanCoordinateMode::from_map(map)?;
   match mode {
       Wgs84NodeGeo => {
           for segment in route.segments {
               let node = map.node(&segment.to)?;
               push_waypoint(position = Position::Geo(node.geo.unwrap().into()));
           }
       }
       LocalWithOrigin => current_local_densified_export(...),
   }
   ```

3. Добавить стабильный GeoJSON utility/testbed в новый файл
   `crates/swarm-sim/src/urban/geojson_import.rs` и экспортировать его из
   [crates/swarm-sim/src/urban/mod.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-sim/src/urban/mod.rs:1).

   Материализуемый результат:

   - парсить только `FeatureCollection`;
   - поддержать `Point` features как nodes:
     `properties.id` обязателен, координаты `[lon, lat]` или `[lon, lat, alt]`;
   - поддержать `LineString` features как directed edges:
     `properties.id`, `properties.from`, `properties.to`, optional
     `cost`, `length_m`, `corridor_width_m`, `blocked`;
   - вычислять local `pose` от первого node geo или explicit import origin через
     существующую metres-per-degree аппроксимацию, чтобы simulation/judge
     продолжали работать;
   - не добавлять полноценный `geojson` crate без необходимости: для
     ограниченного stable fixture достаточно `serde_json::Value`, который уже
     есть в `swarm-sim`;
   - возвращать typed error enum, не `anyhow`.

   Контракт:

   ```rust
   pub struct UrbanGeoJsonImportOptions {
       pub default_altitude_m: f64,
       pub bidirectional_edges: bool,
   }

   pub fn import_urban_map_from_geojson_str(
       input: &str,
       options: &UrbanGeoJsonImportOptions,
   ) -> Result<UrbanMap, UrbanGeoJsonImportError>;
   ```

4. Расширить Urban mission/template metadata в
   [crates/swarm-sim/src/runner/types.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-sim/src/runner/types.rs:140).

   Материализуемый результат:

   - добавить `UrbanMissionTemplate` enum с `perimeter_patrol`, `block_loop`,
     `search_until_target`, `inspection_corridor_candidate`;
   - добавить в `UrbanState` optional `mission_template`,
     `blocked_route_policy`, static assumptions / one-altitude-band metadata
     оставляя старые сценарии валидными;
   - `urban-patrol` и `urban-search` должны использовать эти metadata как
     artifact labels, а не менять текущую simulation core поведение радикально.

   Псевдокод:

   ```rust
   #[serde(rename_all = "snake_case")]
   pub enum UrbanMissionTemplate {
       PerimeterPatrol,
       BlockLoop,
       SearchUntilTarget,
       InspectionCorridorCandidate,
   }
   ```

5. Обновить DSL validation для M84 в
   [crates/swarm-sim/src/dsl/urban_validate.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-sim/src/dsl/urban_validate.rs:48)
   и [crates/swarm-sim/src/dsl/validate.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-sim/src/dsl/validate.rs:264).

   Материализуемый результат:

   - mixed geo/non-geo Urban maps fail validation;
   - geo maps не требуют `scenario.geo_origin` для WGS84 output, но local maps
     сохраняют текущий `geo_origin` / default behavior;
   - `urban-search` требует explicit mocked detector metadata and bus targets;
   - blocked edges/temporary obstacles должны быть совместимы с
     `blocked_route_policy` and route loop;
   - validation errors должны быть stable enough для тестов.

6. Создать canonical M84 scenarios and GeoJSON fixture в `scenarios/`.

   Материализуемый результат:

   - `scenarios/urban.geo-perimeter.json`: geo nodes + perimeter patrol route;
   - `scenarios/urban.geo-block-loop.json`: "облети квартал" as ordered road
     graph loop over WGS84 nodes;
   - `scenarios/urban.geo-search-bus.json`: "облетай квартал пока не встретишь
     автобус" with mocked detector and explicit target/event metadata;
   - `scenarios/urban.geo-inspection-corridor.json`: candidate corridor fixture,
     clearly documented as candidate/template, not full inspection product;
   - `scenarios/fixtures/urban_small_block.geojson`: small portable fixture
     parsed by the new utility.

7. Update `sitl_plan` Urban dry-run path in
   [crates/swarm-examples/src/sitl_plan.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_plan.rs:399).

   Материализуемый результат:

   - `SitlCoordinateFrame` or new artifact field must distinguish local
     simulation frame from `wgs84_node_geo`;
   - добавить `coordinate_mode` в `SitlPlan` and `SitlDryRunArtifact`
     ([sitl_plan.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_plan.rs:224));
   - `format_dry_run_plan`
     ([sitl_plan.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_plan.rs:731))
     должен печатать `coordinate_mode`;
   - `dry_run_artifact_with_mavlink_profile`
     ([sitl_plan.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_plan.rs:816))
     должен сохранять `coordinate_mode`, route/template metadata, perception
     metadata для `urban-search`;
   - `build_command_ir_plan`
     ([sitl_plan.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_plan.rs:882))
     должен строить Urban `FollowRoute` с `Position::Geo` для geo maps и
     `Position::Local` для local maps.

   Важная граница: `start_global/end_global` для geo maps должны быть взяты из
   node geo directly, а не пересчитаны через `geo_origin`.

8. Расширить artifact validator в
   [crates/swarm-examples/src/artifact_validator.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/artifact_validator.rs:318).

   Материализуемый результат:

   - добавить rule ids:
     `artifact.urban_coordinate_mode_missing`,
     `artifact.urban_geo_route_metadata_missing`,
     `artifact.urban_mock_perception_missing`;
   - для current strict dry-run Urban artifacts требовать `coordinate_mode`;
   - для geo Urban artifacts проверять, что mission items совпадают с
     route WGS84 nodes within integer scaling tolerance;
   - для `urban-search` требовать explicit mocked detector/perception metadata в
     artifact, но не требовать реального detector/CV.

9. Обновить replay/event evidence для mission-level reactivity.

   Материализуемый результат:

   - в `crates/swarm-sim/src/runner/urban_events.rs` зафиксировать M84 события:
     `UrbanRouteDecision`, `UrbanMockTargetObserved`, `UrbanBlockedPolicyApplied`
     или расширить существующие события без дублирования;
   - `urban_search.rs` должен явно писать mocked detector seed/range/probability
     в event/artifact path;
   - `urban_patrol.rs` должен писать выбранный template и blocked policy
     decision path;
   - `docs/REPLAY.md` должен перечислять эти события как simulation evidence,
     not real safety/perception.

10. Обновить docs and user-facing status.

    Материализуемый результат:

    - `README.md`: добавить M84 строку в текущую milestone table;
    - `docs/STATUS.md`: `Last audit: M84 Urban Geo-Referenced Mission Pack`,
      честные ограничения;
    - `docs/SCENARIO_DSL.md`: node `geo`, `coordinate_mode`,
      GeoJSON utility, M84 templates;
    - `docs/SITL_SETUP.md`: команды dry-run для новых M84 scenarios и
      artifact validator;
    - `docs/MISSION_COMMAND_IR.md`: Urban WGS84 `FollowRoute` semantics;
    - `docs/MAVLINK_COMMON_COMPILER.md`: local-with-origin vs direct-WGS84
      compilation;
    - `docs/ARTIFACT_VALIDATION.md`: новые rule ids and validation examples;
    - `docs/REPLAY.md`: mocked perception/blocked policy events;
    - `docs/HARDWARE_READINESS.md`: explicitly not hardware-ready, no obstacle
      avoidance/collision certification;
    - `docs/EXTENSION_GUIDE.md` and `docs/OPERATIONAL_RUNBOOKS.md`: how to add
      future Urban geo mission templates.

11. Добавить docs smoke tests in
    [crates/swarm-examples/tests/sitl_docs.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/tests/sitl_docs.rs:699).

    Материализуемый результат:

    - новый тест `m84_docs_describe_urban_geo_pack_boundaries`;
    - проверять фразы: `coordinate_mode`, `WGS84`, `GeoJSON`, `mocked detector`,
      `not certified collision avoidance`, `no full OSM parser`.

12. Обновить dry-run CLI tests in
    [crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs:267).

    Материализуемый результат:

    - `urban_geo_perimeter_dry_run_uses_wgs84_node_coordinates`;
    - `urban_local_with_origin_dry_run_remains_unchanged`;
    - `urban_geo_search_artifact_records_mock_perception_metadata`;
    - `urban_geo_block_loop_mavlink_plan_contains_route_metadata`;
    - all generated artifacts pass `artifact_validator --mode dry-run --strict`
      equivalent helper.

13. Запустить обязательные проверки реализации.

    Материализуемый результат:

    - `cargo fmt --all`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-types urban`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim geojson`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples urban_geo`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent urban`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test artifact_validator urban`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs m84`;
    - `timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings`
      или `make clippy`, если появится repo-specific target.

## Testing strategy

### 1. Tests that need no refactoring

Запланировать и реализовать вместе с основными изменениями:

- `swarm-types` unit tests:
  - geo node serde roundtrip;
  - valid all-geo map passes;
  - all-local map remains valid;
  - mixed geo/non-geo nodes fail with `nodes[].geo`;
  - invalid lat/lon/alt fail.
- `swarm-sim::urban` unit tests:
  - geo-referenced node export uses destination node geo directly;
  - local node export remains byte/shape-compatible with current local output;
  - `urban_route_to_follow_route` emits `Position::Geo` for geo map;
  - perimeter/block loop route order is deterministic;
  - GeoJSON import preserves coordinates within tolerance;
  - GeoJSON import computes local poses from geo for simulation compatibility.
- `swarm-sim::dsl` tests:
  - new M84 scenarios validate;
  - mixed geo map in scenario DSL fails validation;
  - unknown/unsupported GeoJSON geometry fails typed utility error.
- `swarm-examples::sitl_plan` tests:
  - geo Urban dry-run artifact has `coordinate_mode=wgs84_node_geo`;
  - local Urban dry-run artifact keeps current local-with-origin semantics;
  - geo Urban `mavlink_common_plan.mission_items[*].lat_e7/lon_e7` match node
    WGS84 coordinates;
  - `urban-search` artifact records mocked detector metadata;
  - blocked-route policy metadata is present and stable.
- Docs smoke:
  - README/STATUS/SITL/DSL/compiler/artifact validation docs contain M84
    boundaries and non-goals.

### 2. Tests that need light refactoring

- Вынести shared Urban fixture builders из текущих локальных тестов
  (`make_urban_entry` in
  [dsl/tests.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-sim/src/dsl/tests.rs:76),
  `urban_suite` in
  [sitl_plan.rs](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_plan.rs:1201))
  в тестовые helper functions внутри соответствующих modules.
- Добавить common helper для dry-run artifact validation в
  `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs`, чтобы
  новые M84 сценарии не копировали M83 loop.
- Расширить artifact validator fixtures in
  `crates/swarm-examples/tests/artifact_validator.rs`, чтобы можно было
  мутировать `coordinate_mode` and Urban metadata without duplicating large JSON.
- Добавить replay/artifact validator checks для perception/route-decision events,
  если существующий event summary helper не покрывает новые поля.

### 3. Tests that need heavy refactoring

Не блокируют M84, но должны быть явно отмечены как future work:

- property tests для geo/local roundtrip over broad coordinate ranges;
- generalized map import pipeline for real OSM fragments;
- richer map constraint model beyond current road graph;
- multi-altitude Urban airspace model;
- geometry/navmesh collision engine and polygon no-fly validation;
- full SITL/PX4/ArduPilot execution of geo Urban missions.

## Risks and tradeoffs

- **Schema compatibility:** добавление optional `UrbanNode.geo` безопасно для
  старых fixtures, но новый required `coordinate_mode` в current strict artifacts
  может ломать старые dry-run artifacts. Проверять через historical mode или
  добавить validator downgrade только для historical artifacts.
- **Coordinate semantics:** direct WGS84 nodes and local-with-origin maps нельзя
  смешивать. Mixed map должен fail early, иначе MAVLink items могут silently
  получить часть координат из node geo, часть через origin.
- **MAVLink altitude semantics:** M81 сейчас пишет
  `MAV_FRAME_GLOBAL_RELATIVE_ALT_INT`; `UrbanGeoPoint.alt_m` нужно трактовать как
  relative altitude under current `AltitudeReference`, не как AMSL, пока не
  появится отдельный altitude datum.
- **GeoJSON utility scope:** это testbed, не GIS engine. Поддержка только Point /
  LineString должна быть явно documented, иначе появится ложное ожидание OSM /
  polygon/navmesh support.
- **Urban search semantics:** mocked detector remains simulation evidence, not
  real CV/lidar. Artifact metadata должна предотвращать claims о реальном
  обнаружении автобуса.
- **Performance/resources:** M84 не требует long benchmark. Все тесты должны
  укладываться в 300s timeout; 1000-seed/500-seed runs не планируются.
- **Docs claims:** README/STATUS/HARDWARE_READINESS должны прямо говорить:
  no real lidar, no certified collision avoidance, no full OSM parser, no
  hardware-ready claim.

## Open questions

- Нужно ли добавлять explicit `geo_origin` для GeoJSON import fixtures или
  достаточно использовать первый Point как local origin? Для M84 предлагается
  использовать первый Point как default, а explicit origin оставить optional.
- Нужно ли делать `urban-search` dry-run artifact runnable через
  `sitl_agent --dry-run` как route export без simulation execution, или отдельно
  писать simulation run artifact? Для M84 предлагается dry-run artifact +
  existing simulation tests: dry-run доказывает command/MAVLink shape, simulation
  tests доказывают mocked detector outcome.
- Нужно ли включать `inspection_corridor_candidate` в benchmark/regression gate?
  Для M84 предлагается только scenario validation + dry-run artifact test; long
  benchmark не нужен.
