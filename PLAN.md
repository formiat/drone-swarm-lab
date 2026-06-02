# Context

Нужно запланировать реализацию M70 из
`docs_raw/BEFORE_HARDWARE_A.23.md`: соединить существующий Urban simulation
layer с текущим PX4/SIH waypoint workflow через deterministic export path:

```text
Urban planned route -> ordered waypoint mission -> dry-run/SITL-compatible plan
```

Это не hardware run, не Gazebo/HIL gate и не low-level flight control. Цель
M70 - сделать перенос Urban route в waypoint mission явным, воспроизводимым и
проверяемым без PX4:

- `urban-patrol` route должен экспортироваться в упорядоченный список waypoint
  items;
- route/segment/task identity должны сохраняться настолько, насколько это
  возможно без изменения смысла Urban route;
- altitude и coordinate origin должны стать явными в сценарии, плане и
  artifact;
- `geo_origin` должен переопределять WGS84 origin для dry-run/SITL waypoint
  conversion;
- README и все связанные docs должны честно отделять simulation route validity
  от SITL/PX4 waypoint execution.

Исходный prompt явно требует запланировать актуализацию README и всех
сопутствующих md, а также явно фиксировать необходимые тесты, билды, прогоны и
ручные/optional проверки.

# Investigation context

`INVESTIGATION.md` в workspace root отсутствует. Существующий `PLAN.md` также
отсутствовал, поэтому этот файл создаётся как новый план.

Что проверено перед планированием:

- `docs_raw/BEFORE_HARDWARE_A.23.md`: M70 является первым milestone
  pre-hardware цепочки M70-M79.
- `crates/swarm-types/src/urban.rs`: Urban route уже представлен типами
  `UrbanPlannedRoute`, `UrbanRouteSegment`, `UrbanMap`, `UrbanNode`,
  `UrbanEdge`, `UrbanRouteLoop`.
- `crates/swarm-sim/src/urban/planner.rs`: route loop уже детерминированно
  расширяется через `expand_route_loop_with_planner_name`.
- `crates/swarm-sim/src/scenario.rs`: `Scenario` пока не содержит
  `geo_origin`; есть `base_station`, но это local `Pose`, не WGS84 origin.
- `crates/swarm-sim/src/dsl/types.rs` / `validate.rs`: Scenario DSL
  сериализуется через `ScenarioSuiteEntry`; mission-specific validation уже
  различает `sitl`, `urban-patrol`, `urban-search`.
- `crates/swarm-examples/src/sitl_plan.rs`: current SITL plan строится только
  из `scenario.tasks[*].pose`; Urban route из `run_config.urban_state` сейчас
  не используется как источник waypoint export.
- `crates/swarm-comms/src/mavlink/types.rs`: `MissionUploadOptions` уже имеет
  `home_origin: MissionHomeOrigin` с hardcoded PX4/SIH default
  `47.397742, 8.545594, 0.0`.
- `crates/swarm-comms/src/mavlink/mission_items.rs`: conversion local
  x/y/z -> `MISSION_ITEM_INT` уже использует `options.home_origin`, но helper
  functions находятся под `mavlink-transport` feature.
- `scenarios/urban.patrol.json`: Urban fixture уже содержит road graph,
  route_loop и placeholder waypoint tasks, но не содержит `geo_origin`.
- `scenarios/sitl.waypoints.json` и `scenarios/sitl.px4-golden.json`: SITL
  fixtures не содержат явный WGS84 origin, хотя default origin скрыт в
  MAVLink upload options.
- `docs/SCENARIO_DSL.md`, `docs/SITL_SETUP.md`, `docs/STATUS.md`,
  `docs/HARDWARE_READINESS.md`, `docs/EXTENSION_GUIDE.md`, `docs/REPLAY.md`
  и `README.md`: текущие docs честно говорят, что Urban не имеет PX4/SITL
  export. M70 должен обновить эти утверждения без hardware-readiness claims.

Ключевой вывод: M70 лучше делать как отдельный typed export/adapter слой, а не
как тихое изменение старого `sitl_plan` extractor. Старый extractor должен
сохранить backwards-compatible behavior для обычных `sitl` scenarios с pose
tasks.

# Affected components

- `crates/swarm-sim/src/scenario.rs`
  - добавить `GeoOrigin`;
  - добавить `Scenario::geo_origin: Option<GeoOrigin>`;
  - сохранить serde backward compatibility через `#[serde(default)]`.
- `crates/swarm-sim/src/dsl/types.rs`, `load.rs`, `validate.rs`, `tests.rs`
  - обеспечить JSON roundtrip и validation для `geo_origin`;
  - reject non-finite / out-of-range lat/lon/alt values.
- `crates/swarm-types/src/urban.rs`
  - при необходимости добавить export-specific identity fields или helper
    accessors не ломая существующий `UrbanPlannedRoute`.
- `crates/swarm-sim/src/urban/route_export.rs` или новый соседний module в
  `crates/swarm-sim/src/urban/`
  - route-to-waypoint conversion без зависимости от `swarm-examples`;
  - densification long Urban edges;
  - metadata: route length, segment count, waypoint count, planner, altitude,
    geo origin.
- `crates/swarm-sim/src/urban/mod.rs`
  - экспортировать новый route export API для `swarm-examples` и tests.
- `crates/swarm-examples/src/sitl_plan.rs`
  - добавить Urban-aware plan builder path;
  - расширить `SitlWaypointItem` identity fields;
  - добавить origin/altitude/route metadata в `SitlPlan`;
  - сохранить старый pose-task path.
- `crates/swarm-examples/src/sitl_agent_runtime/connection.rs`
  - передавать `plan.geo_origin` в `MissionUploadOptions.home_origin`, если
    используется `mavlink-transport`;
  - при отсутствии `geo_origin` оставить текущий default behavior.
- `crates/swarm-examples/src/sitl_agent_runtime/mock.rs`,
  `crates/swarm-examples/src/sitl_agent_runtime/reports.rs`,
  `crates/swarm-examples/src/sitl_observability/*`
  - при необходимости добавить origin/export metadata в mock output, reports
    или replay log только если это нужно для readable artifact.
- `crates/swarm-examples/src/sitl_supervisor/*` и `crates/swarm-examples/src/sitl_multi_agent.rs`
  - убедиться, что multi-agent task subset / manifest не ломается от новых
    waypoint identity fields.
- `crates/swarm-comms/src/mavlink/types.rs` и
  `crates/swarm-comms/src/mavlink/mission_items.rs`
  - по возможности вынести local->global conversion helper из feature-gated
    зоны в тестируемый API или добавить feature-gated tests; не менять MAVLink
    protocol semantics.
- `scenarios/*.json`
  - добавить явный `geo_origin` в SITL scenarios;
  - добавить `geo_origin` и altitude/export settings в Urban M70 fixture или
    обновить `scenarios/urban.patrol.json`, если совместимость позволяет.
- `docs/SCENARIO_DSL.md`, `docs/SITL_SETUP.md`, `docs/STATUS.md`,
  `docs/HARDWARE_READINESS.md`, `docs/EXTENSION_GUIDE.md`,
  `docs/REPLAY.md`, `docs/BENCHMARK_RESULTS.md`, `README.md`
  - актуализировать M70 статус, usage, limitations and verification commands.
- `crates/swarm-examples/tests/sitl_agent/*`, `crates/swarm-sim/src/dsl/tests.rs`,
  `crates/swarm-sim/src/urban/tests.rs`, возможно
  `crates/swarm-comms/src/mavlink/tests_mission_upload.rs`
  - добавить self-contained portable tests.

# Implementation steps

1. В `crates/swarm-sim/src/scenario.rs` добавить тип `GeoOrigin`:
   `lat_deg: f64`, `lon_deg: f64`, `alt_m: f64`; добавить
   `#[serde(default, skip_serializing_if = "Option::is_none")] pub geo_origin:
   Option<GeoOrigin>` в `Scenario`; обновить `Scenario::empty` и все
   in-repo scenario builders/tests, которые создают `Scenario` напрямую.
   Результат: старые JSON без поля продолжают десериализоваться, новые сценарии
   могут явно задавать WGS84 origin.

2. В `crates/swarm-sim/src/dsl/validate.rs` добавить validation для
   `scenario.geo_origin`: finite values, `lat_deg` в `[-90, 90]`, `lon_deg` в
   `[-180, 180]`, finite `alt_m`; добавить ошибки с точным field path
   `scenario.geo_origin.*`. Результат: некорректный origin отклоняется до
   export/dry-run/upload.

3. В `crates/swarm-sim/src/dsl/tests.rs` добавить JSON tests:
   `geo_origin_roundtrip_json`, `geo_origin_rejects_bad_lat_lon`,
   `scenario_without_geo_origin_remains_valid`. Результат: serde backward
   compatibility и validation contract покрыты unit tests.

4. Добавить новый Urban export module, предпочтительно
   `crates/swarm-sim/src/urban/route_export.rs`, и подключить его через
   `crates/swarm-sim/src/urban/mod.rs`. Ввести typed structs:
   `UrbanRouteExportOptions`, `UrbanRouteWaypoint`, `UrbanRouteExport`,
   `UrbanRouteExportMetadata`. Поля результата должны включать:
   `seq`, local `x/y/z`, `task_id` where available, `edge_id`,
   `from_node_id`, `to_node_id`, `segment_index`, `point_index_on_segment`,
   `route_length_m`, `segment_count`, `waypoint_count`, `altitude_m`,
   `spacing_m`, `planner`. Результат: route-to-waypoint conversion становится
   отдельным тестируемым API.

5. В route export module реализовать deterministic conversion:
   - строить route через уже существующий
     `expand_route_loop_with_planner_name(&urban_state.map, &urban_state.route_loop, &urban_state.planner)`;
   - строить waypoints по route segments в стабильном порядке;
   - для каждого segment включать конечную node pose;
   - для long edges добавлять промежуточные точки с deterministic spacing
     `max_spacing_m` из options;
   - не дублировать start point, если он уже является текущей позицией агента;
   - для route loop completion допускать final waypoint на start node, если
     loop явно возвращается в start.
   Результат: Urban route превращается в SITL-compatible ordered waypoint list.

6. Определить altitude contract для M70: добавить default altitude option,
   например `UrbanRouteExportOptions::default_altitude_m`, и использовать его,
   если route node/task не задаёт `z`; для `Pose.z` использовать существующий
   serde default `0.0` только как input fact, но в export metadata всегда
   писать explicit altitude source. Результат: altitude больше не скрыт в
   implicit defaults.

7. В `crates/swarm-examples/src/sitl_plan.rs` расширить `SitlWaypointItem`
   optional identity fields:
   `source: SitlWaypointSource`, `edge_id`, `from_node_id`, `to_node_id`,
   `segment_index`, `point_index_on_segment`; добавить в `SitlPlan`
   `geo_origin`, `route_length_m`, `segment_count`, `waypoint_count`,
   `planner_or_adapter`, `export_kind`. Сохранить старый `sitl` path из
   pose tasks как `export_kind = "pose_tasks"`. Результат: один plan shape
   поддерживает старые SITL tasks и новый Urban route export.

8. В `build_sitl_plan_with_task_filter` добавить branch для
   `mission == "urban-patrol"` и `run_config.urban_state.is_some()`:
   использовать новый Urban route export API вместо placeholder task order.
   Для multi-agent `task_ids` не пытаться делить Urban route в M70; если
   `task_ids` задан для Urban route export, вернуть typed error или оставить
   только legacy pose-task behavior по явно задокументированному правилу.
   Результат: single-agent Urban Patrol dry-run показывает route-derived
   waypoints, а multi-agent semantics не ломаются молча.

9. В `format_dry_run_plan` вывести новые поля: `export_kind`,
   `planner_or_adapter`, `geo_origin`, `route_length_m`, `segment_count`,
   `waypoint_count`, `altitude_source`, plus waypoint identity fields для
   Urban route. Результат: dry-run readable artifact содержит всю информацию
   из M70 Scope.

10. В `crates/swarm-examples/src/sitl_agent_runtime/connection.rs` при сборке
    `MissionUploadOptions` установить `home_origin` из `plan.geo_origin`, если
    он есть; если нет, оставить `MissionUploadOptions::default()`. Добавить
    conversion helper between `swarm_sim::GeoOrigin` and
    `swarm_comms::MissionHomeOrigin` под `mavlink-transport` feature.
    Результат: connection/upload path использует scenario origin без изменения
    MAVLink protocol.

11. Если local->global conversion helpers в `swarm-comms` остаются private и
    feature-gated, добавить minimal public/test-only conversion surface:
    `local_waypoint_to_global_coordinate_summary` или equivalent helper под
    `mavlink-transport`, чтобы тест `geo_origin_overrides_default_in_dry_run`
    мог доказать изменение lat/lon без PX4. Результат: origin behavior
    проверяется automated test, а не только чтением кода.

12. Добавить dry-run export artifact support. Минимальный вариант: новый CLI
    flag для `sitl_agent`, например `--dry-run-artifact <path>`, который
    доступен только с `--dry-run`, пишет JSON с schema version, source scenario
    path, suite/scenario/mission/profile, agent id, export kind, adapter name,
    route length, segment/waypoint count, start/end waypoint summary, altitude,
    `geo_origin`, command args, run id if available, and git commit if
    available. Если отдельный flag окажется лишним, использовать существующий
    output-dir pattern нельзя без изменения CLI; тогда добавить explicit
    `--dry-run-artifact` как материализуемый output. Результат: M70 dry-run
    artifact readable and reproducible.

13. Обновить scenario fixtures:
    - `scenarios/sitl.waypoints.json`;
    - `scenarios/sitl.px4-golden.json`;
    - `scenarios/sitl.multi-agent.json`;
    - `scenarios/urban.patrol.json`;
    - при необходимости `scenarios/urban.corridor-delta.json`.
    Добавить явный current PX4/SIH default origin
    `{ "lat_deg": 47.397742, "lon_deg": 8.545594, "alt_m": 0.0 }` в SITL
    scenarios. Для Urban M70 fixture добавить origin и explicit altitude in
    export options или task/node pose fields, в зависимости от выбранного
    schema placement. Результат: coordinate frame видим в data, а не скрыт в
    code.

14. Обновить generated scenario builders in
    `crates/swarm-scenarios/src/urban.rs` и другие builders, которые напрямую
    создают `Scenario`, чтобы компиляция прошла и JSON fixtures оставались
    согласованными с generated scenarios. Результат: in-code fixtures не
    расходятся с repo JSON.

15. Обновить README и docs:
    - `README.md`: добавить quick-start command для Urban route dry-run export
      и artifact path; явно сказать no hardware/perception/avoidance claim.
    - `docs/SCENARIO_DSL.md`: документировать `scenario.geo_origin`, Urban
      route export fields, altitude/default semantics.
    - `docs/SITL_SETUP.md`: добавить section "Urban Route Export Dry-Run",
      expected output/artifact, origin semantics, optional/manual PX4 upload
      boundary.
    - `docs/STATUS.md`: M70 status после реализации; update recommended next
      steps.
    - `docs/HARDWARE_READINESS.md`: отметить, что M70 улучшает dry-run/SITL
      waypoint review, но не hardware readiness.
    - `docs/EXTENSION_GUIDE.md`: добавить extension guidance для route export
      adapters.
    - `docs/REPLAY.md`: если M70 не добавляет replay events, явно сказать что
      export artifact separate from replay; если добавит event/log fields,
      документировать schema compatibility.
    - `docs/BENCHMARK_RESULTS.md`: отметить, что M70 не является benchmark
      refresh.
    Результат: user-facing docs синхронизированы с новым behavior.

16. Добавить/обновить automated tests из раздела Testing strategy ниже. Все
    tests должны быть self-contained: inline fixtures или repo fixtures,
    никаких `$HOME`, внешнего PX4, sockets, hardware, sibling repos или
    machine-specific paths.

17. Запустить обязательные проверки после implementation:
    ```bash
    cargo fmt --all
    cargo check -p swarm-sim
    cargo check -p swarm-examples
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim geo_origin
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban_route_export
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples urban_route
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent urban
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check
    git diff --check
    find . -name '*.proptest-regressions' -print
    ```
    Если `mavlink-transport` helper/API меняется, добавить:
    ```bash
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms --features mavlink-transport mission
    cargo clippy -p swarm-comms --features mavlink-transport --all-targets -- -D warnings
    ```
    Результат: implementation проверен на affected crate/module scope.

18. Optional/manual verification не является done criterion, но если локальный
    PX4/SIH уже запущен и оператор явно хочет проверить upload-only, выполнить
    только manual artifact run:
    ```bash
    /home/formi/.local/bin/runlim cargo run -p swarm-examples --bin sitl_agent --features mavlink-transport -- \
      --connection udpin:127.0.0.1:14550 \
      --scenario scenarios/urban.patrol.json \
      --agent-id agent-0 \
      --safety-config path/to/sitl-safety.json \
      --upload-only
    ```
    Результат: optional local PX4/SIH artifact. Если не выполняется, docs and
    outbox должны явно сказать "manual upload-only not run".

# Testing strategy

## 1. Tests that need no refactoring

Эти tests нужно реализовать вместе с основными функциональными изменениями:

- `geo_origin_roundtrip_json` in `crates/swarm-sim/src/dsl/tests.rs`:
  serialize/deserialize `ScenarioSuiteEntry` with `scenario.geo_origin` and
  compare exact values.
- `geo_origin_absent_uses_sitl_default` in `crates/swarm-examples/src/sitl_plan.rs`
  or `crates/swarm-examples/tests/sitl_agent/...`: scenario without origin
  builds plan with `geo_origin == None` / default upload origin path preserved.
- `geo_origin_rejects_invalid_lat_lon` in `crates/swarm-sim/src/dsl/tests.rs`:
  invalid latitude/longitude produce field-specific validation errors.
- `urban_route_exports_ordered_waypoints` in
  `crates/swarm-sim/src/urban/tests.rs`: simple route loop exports expected
  ordered coordinates and seq values.
- `urban_route_export_stable_ids`: repeated export returns same `edge_id`,
  `from_node_id`, `to_node_id`, `segment_index`, `point_index_on_segment`.
- `urban_route_altitude_explicit`: export uses explicit/default altitude and
  records altitude source deterministically.
- `urban_route_export_densifies_long_edges`: one long edge with spacing
  produces deterministic intermediate waypoints and final endpoint.
- `urban_route_export_rejects_bad_spacing`: zero/negative/non-finite spacing
  returns typed error.
- `urban_patrol_dry_run_uses_route_export`: `sitl_agent --dry-run` on
  `scenarios/urban.patrol.json` prints route-derived metadata and waypoint
  identity, not just old placeholder task order.
- `geo_origin_overrides_default_in_dry_run`: fixture with non-default origin
  changes global coordinate summary while local x/y route geometry stays the
  same.
- `dry_run_artifact_contains_export_metadata`: `--dry-run-artifact` writes JSON
  with source scenario path, adapter name, route length, waypoint count,
  start/end summary, altitude and origin.
- `dry_run_artifact_rejects_non_dry_run`: CLI rejects artifact flag outside
  `--dry-run`.
- `sitl_docs_mentions_urban_export_limitations`: docs smoke test verifies no
  hardware/perception/avoidance claims were introduced.

## 2. Tests that need light refactoring

- Shared Urban route fixture helper in `crates/swarm-sim/src/urban/tests.rs`
  or a small test helper module to avoid duplicating node/edge setup.
- Shared SITL plan assertion helper in
  `crates/swarm-examples/tests/sitl_agent/supervisor_tests.rs` for asserting
  origin/export metadata in CLI output/artifacts.
- Export metadata assertion helper for JSON artifact shape to avoid brittle
  string-only CLI tests.
- A small conversion helper test in `swarm-comms` if local->global conversion
  remains feature-gated and cannot be asserted from `swarm-examples` without
  `mavlink-transport`.

## 3. Tests that need heavy refactoring

- Manual/ignored local PX4/SIH upload test for exported Urban route. This
  requires external PX4 process and must not become default CI.
- Cross-run export artifact comparison tool for proving artifact reproducibility
  across commits/runs. Useful later, not required for M70.
- Larger route densification property tests over generated maps. Useful after
  route export semantics stabilize; not required for first implementation.
- Multi-agent Urban route splitting/deconfliction tests. M70 is single-agent
  route export; multi-agent Urban control belongs to a later milestone.

# Risks and tradeoffs

- Behavior/API: adding `Scenario::geo_origin` changes the public Rust struct.
  All direct struct initializers must be updated. Serde compatibility is kept
  with `#[serde(default)]`; compile errors are expected in builders/tests until
  updated.
- Behavior/API: `SitlWaypointItem` identity fields must be additive or optional.
  Existing tests that compare old dry-run text may need updates but old
  pose-task extraction semantics must remain unchanged.
- Data/schema: adding `geo_origin` to JSON fixtures is additive. Old scenario
  files must still load; docs must state the default behavior when absent.
- Integration: passing `geo_origin` into `MissionUploadOptions.home_origin`
  changes uploaded global lat/lon for scenarios that set it. This is intended,
  but must be proven by tests without PX4.
- Integration: Urban route export should not silently use placeholder waypoint
  tasks as the authoritative route. The authoritative source for Urban is
  `run_config.urban_state`.
- Integration: multi-agent SITL subset logic currently filters task ids.
  Route-derived Urban waypoints do not naturally map to per-agent task subsets
  yet; M70 should reject or clearly keep this unsupported instead of inventing
  partial multi-agent Urban semantics.
- Performance/resources: densification can explode waypoint count for long
  edges if spacing is too small. Add validation/minimum spacing and report
  waypoint count.
- Documentation risk: M70 can be misread as "Urban can fly on hardware".
  README/docs must explicitly say it is dry-run/SITL-compatible waypoint
  export only: no real perception, no certified obstacle avoidance, no
  hardware readiness.

## Что могло сломаться и как проверять

- Старые `sitl_agent --dry-run` outputs: проверить `cargo test -p
  swarm-examples --test sitl_agent` and docs smoke tests.
- Scenario JSON compatibility: проверить `cargo test -p swarm-sim dsl`.
- MAVLink coordinate conversion: проверить `swarm-comms` feature-gated mission
  tests and a non-PX4 dry-run origin override test.
- Urban route semantics: проверить route export unit tests against existing
  planner tests in `crates/swarm-sim/src/urban/tests.rs`.
- Multi-agent SITL manifests: проверить `cargo test -p swarm-examples
  sitl_multi_agent` and `cargo test -p swarm-examples --test sitl_agent
  multi_agent`.
- Docs/status drift: проверить `cargo test -p swarm-examples --test sitl_docs`
  and manually inspect README, `docs/STATUS.md`, `docs/SITL_SETUP.md`,
  `docs/SCENARIO_DSL.md`, `docs/HARDWARE_READINESS.md`.

# Open questions

- Где именно хранить export options для altitude/spacing: в
  `run_config.urban_state`, в новом `run_config.urban_export`, или в
  `Scenario.geo_origin` plus hardcoded default export options? Для M70 можно
  начать с conservative defaults in code plus explicit docs, но если пользователь
  хочет scenario-level spacing/altitude config, нужно добавить отдельный schema
  field.
- Нужно ли M70 создавать отдельный binary/command для route export, или
  достаточно `sitl_agent --dry-run --dry-run-artifact`? Рекомендация: сначала
  расширить существующий `sitl_agent`, потому он уже является SITL waypoint
  boundary.
- Нужно ли включать Urban route export в `strategy_comparison` artifacts? Для
  M70 нет: это SITL/export boundary, не benchmark refresh.
- Должен ли `geo_origin.alt_m` участвовать в MAVLink relative altitude
  calculation или только документировать home altitude? Текущий MAVLink path
  использует `MISSION_ITEM_INT.z` as relative altitude; M70 должен сохранить
  это и явно документировать `alt_m` as origin metadata unless a future
  absolute-altitude mode is introduced.
