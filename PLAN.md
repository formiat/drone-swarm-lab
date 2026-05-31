# Context

Планируем реализацию `M64 - Urban Foundations` по roadmap
`docs_raw/DRONE_C.21.md`. M63 уже закрыт отдельным коммитом, поэтому M64
должен начинать Urban-направление с честной минимальной основы, а не с полной
миссии "облети квартал", лидара, автобуса, dynamic obstacles, multi-agent
deconfliction или PX4/SITL export.

Цель M64: добавить reusable substrate для будущих Urban Patrol/Search этапов:

- road graph как основной навигационный primitive;
- deterministic route planner поверх графа;
- простые map constraints;
- initial judge API;
- `urban_patrol` scenario DSL fixture;
- metrics/replay schema placeholders, только если они реально подключены к
  runner/reports/replay и не создают ложных claims.

Важная архитектурная граница: проект остается mission-level simulation,
planning, coordination, replay and metrics layer. Он не должен становиться
low-level flight controller, physics simulator, SLAM, real lidar/object
detection stack или заменой PX4.

Протоколы оркестратора прочитаны:

- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`.

Notion/GitLab в пользовательском prompt не запрошены; `notion_policy` для
этого запуска `optional`, поэтому внешние чтения не выполняются.

# Investigation context

`INVESTIGATION.md` в workspace отсутствует.

Изученный локальный контекст:

- `docs_raw/DRONE_C.21.md` фиксирует линейную цепочку M63 -> M64 -> M65 -> M66
  и явно задает road-graph-first Urban plan.
- `README.md`, `docs/STATUS.md`, `docs/SCENARIO_DSL.md`,
  `docs/EXTENSION_GUIDE.md`, `docs/REPLAY.md` уже описывают extension path,
  schema policy и текущие ограничения.
- `crates/swarm-types/src/pose.rs` уже содержит `Pose` и `Aabb`; их нужно
  переиспользовать для Urban map и static obstacles.
- `crates/swarm-types/src/edge.rs` содержит `InspectionGraph`, но это
  inspection-specific модель. Для Urban лучше добавить отдельные
  `UrbanNodeId`/`UrbanEdgeId`/`UrbanMap`, чтобы не смешивать инфраструктурную
  inspection semantics с road graph semantics.
- `crates/swarm-sim/src/dsl.rs` валидирует `ScenarioSuite` и mission-specific
  constraints; сюда нужно добавить urban validation.
- `crates/swarm-sim/src/runner.rs` содержит `RunConfig` и mission-specific
  runtime state (`inspection_state`, `wildfire_state`, `grid_state`); Urban
  foundation можно подключить как additive optional state/config без bump
  schema version.
- `crates/swarm-scenarios` содержит deterministic scenario builders и profile
  enums; M64 должен добавить аналогичный `urban` модуль и публичный fixture.
- `crates/swarm-metrics/src/metrics.rs` и
  `crates/swarm-sim/src/report_export.rs` требуют синхронного обновления, если
  добавляются user-facing Urban metrics.
- `crates/swarm-replay/src/event_log.rs` можно расширять additive event
  variants без смены replay schema, если старые логи продолжают
  десериализоваться.

# Affected components

- `crates/swarm-types/src/urban.rs` - новые shared Urban-типы:
  `UrbanNodeId`, `UrbanEdgeId`, `UrbanObstacleId`, `UrbanNode`, `UrbanEdge`,
  `UrbanStaticObstacle`, `UrbanMap`, `UrbanRouteLoop`, `UrbanRouteSegment`,
  `UrbanPlannedRoute`, `UrbanViolation`.
- `crates/swarm-types/src/lib.rs` - экспорт Urban-типов.
- `crates/swarm-types/src/task.rs` - решить, нужен ли новый `TaskKind` уже в
  M64. Предпочтительный вариант: не добавлять новый kind без runtime semantics;
  использовать existing `Waypoint` для placeholder route tasks и явно
  документировать, что полноценная Urban mission semantics начинается в M65.
- `crates/swarm-sim/src/urban.rs` - deterministic planner + judge:
  Dijkstra/A* over `UrbanMap`, route-loop expansion, validation helpers,
  obstacle/blocked-edge checks.
- `crates/swarm-sim/src/runner.rs` - additive `RunConfig::urban_state` или
  `RunConfig::urban_config`, начальное заполнение Urban metrics, без изменения
  общего algorithm behavior для существующих missions.
- `crates/swarm-sim/src/dsl.rs` - validation для `mission == "urban-patrol"`:
  map exists, route loop valid, node/edge refs valid, blocked edges не
  используются, fixture has agents/tasks, actionable error messages.
- `crates/swarm-sim/src/lib.rs` - публичный экспорт `urban` API, если нужно для
  тестов и будущего M65.
- `crates/swarm-scenarios/src/urban.rs` - deterministic `urban_patrol`
  fixture/profile builder.
- `crates/swarm-scenarios/src/lib.rs` - экспорт builder/profile.
- `scenarios/urban.patrol.json` - portable DSL fixture.
- `crates/swarm-metrics/src/metrics.rs` - Urban metric skeleton:
  `route_length_m`, `route_planned`, `urban_violation_count`,
  `urban_route_completed` или имена с `urban_` prefix, если есть риск
  конфликтов с уже существующим `avg_route_length`.
- `crates/swarm-sim/src/report_export.rs` и `crates/swarm-sim/src/benchmark.rs`
  - JSON/CSV/Markdown export и aggregation для новых user-facing metrics, если
  эти поля добавляются в `RunMetrics`.
- `crates/swarm-replay/src/event_log.rs`, `crates/swarm-replay/src/replay.rs`,
  `docs/REPLAY.md` - optional additive replay placeholders/events:
  `UrbanRoutePlanned`, `UrbanJudgeViolation`, возможно
  `UrbanRouteSegmentEntered`. Добавлять только если runner их реально пишет или
  если placeholder schema нужен M65 и покрыт roundtrip tests.
- `docs/SCENARIO_DSL.md` - Urban DSL section and example.
- `docs/EXTENSION_GUIDE.md` - Urban extension path and road-graph-first
  boundary.
- `docs/STATUS.md` - M64 status, limitations, non-goals.
- `README.md` - feature matrix, milestone table, scenario list, quick command
  if a small smoke run is supported.
- `docs/BENCHMARK_RESULTS.md` / `docs/REGRESSION.md` - только короткая
  оговорка, что M64 не является benchmark refresh и не обновляет M62 evidence;
  полноценный Urban benchmark остается будущим M69.

# Implementation steps

1. Добавить Urban domain model в `crates/swarm-types/src/urban.rs`.
   - Newtype wrappers сделать по локальным правилам: private inner field,
     derives `AsRef`, `Deref`, `DerefMut`, `From`, `Into`; `Display` добавить,
     если id участвует в error/report strings.
   - `UrbanNode` содержит `id: UrbanNodeId` и `pose: Pose`.
   - `UrbanEdge` содержит `id`, `from`, `to`, `cost`, `length_m`,
     optional `corridor_width_m`, optional `blocked`.
   - `UrbanStaticObstacle` сначала только AABB: `id`, `bounds: Aabb`,
     optional `label`.
   - `UrbanMap` содержит nodes/edges/static_obstacles.
   - Добавить методы lookup/validation:
     unique ids, finite/non-negative cost/length/width, all edge endpoints
     exist, valid AABB bounds, route loop nodes exist.
   - Ошибки сделать typed, actionable, без `anyhow`.

2. Реализовать planner/judge в `crates/swarm-sim/src/urban.rs`.
   - Вынести algorithmic слой из `swarm-types`, чтобы shared types не
     становились simulation engine.
   - Начать с Dijkstra. A* не нужен, пока нет heuristic requirements.
   - Детерминизм обеспечить сортировкой adjacency по `(cost, edge_id, to_id)`
     и tie-breaking в priority queue через `f64::total_cmp`, hop count и id.
   - API:
     `plan_route(map, from, to) -> Result<UrbanPlannedRoute, UrbanRouteError>`;
     `expand_route_loop(map, loop_nodes) -> Result<UrbanPlannedRoute, ...>`;
     `judge_route(map, route) -> Vec<UrbanViolation>`.
   - Judge checks:
     route uses existing graph edges;
     blocked edge is violation;
     edge endpoint inside AABB obstacle is violation;
     if cheap local implementation is still small, add segment-vs-AABB
     intersection for edge crossing buildings/no-fly zones.

3. Подключить Urban config к Scenario DSL.
   - Добавить optional `urban_state`/`urban_config` в `RunConfig`.
   - Минимальная структура: `map: UrbanMap`, `route_loop: UrbanRouteLoop`,
     optional `start_node`, optional `planner: "dijkstra"` reserved for future.
   - `validate_mission_specific` в `crates/swarm-sim/src/dsl.rs` для
     `mission == "urban-patrol"` должен требовать `urban_state`, валидный
     graph, route loop length >= 2, all route nodes present, at least one agent,
     and at least one task/waypoint placeholder until M65 defines real progress
     semantics.
   - Ошибки должны указывать точный field path:
     `run_config.urban_state.map.edges[3].from`, `route_loop.nodes[1]`, etc.

4. Добавить deterministic fixture в `crates/swarm-scenarios/src/urban.rs`.
   - Профиль `UrbanProfile::PatrolSmallBlock`.
   - Map: 4-6 intersections, deterministic square/block graph,
     one optional blocked edge and one AABB building/no-fly zone that does not
     break the happy path.
   - Route loop: e.g. `n0 -> n1 -> n2 -> n3 -> n0`.
   - Agents: 1 scout with stable pose at start node.
   - Tasks: placeholder waypoint tasks on route nodes using `TaskKind::Waypoint`
     until M65 adds mission-specific completion semantics.
   - Run config: `max_ticks`, `enable_movement` policy selected explicitly.

5. Добавить `scenarios/urban.patrol.json`.
   - JSON должен быть generated/hand-written deterministic fixture, no local
     paths, no machine-specific data.
   - `schema_version` оставить `"0.1"`, если изменения additive and backward
     compatible.
   - Добавить catalog test coverage, чтобы файл грузился вместе с остальными
     scenarios.

6. Подключить metrics skeleton.
   - Добавить fields в `RunMetrics` с `#[serde(default)]`.
   - Если метрики попадают в aggregate/report, добавить соответствующие fields
     в `AggregateMetrics::from_runs`, `Display`, CSV/Markdown export and tests.
   - В runner для Urban config:
     if route planning succeeds: `route_planned = true`, `route_length_m` =
     суммарная длина/стоимость planned segments, `urban_violation_count` =
     judge violations, `urban_route_completed` пока derived only from initial
     route-progress placeholder or `false` unless M64 implements actual route
     progress. Не заявлять полноценное patrol completion до M65.
   - Если planning fails, metrics/report should expose actionable failure rather
     than panic.

7. Добавить replay placeholders только при реальном использовании.
   - Минимально: serde roundtrip for new event variants, если добавлены.
   - Не писать events из runner, если нет meaningful route event source.
   - Если runner пишет route planning result, добавить
     `UrbanRoutePlanned { route_id/edge_ids/length_m/tick }` and
     `UrbanJudgeViolation { violation_type/location/tick }`.
   - Обновить `docs/REPLAY.md` and replay summary tests.

8. Обновить docs/status.
   - `README.md`: добавить M64 в milestone table, feature matrix row для
     Urban Foundations, fixture path, non-goals.
   - `docs/STATUS.md`: отметить M64 как planned/implemented foundation после
     кода; явно сказать "без bus/lidar/dynamic obstacles/PX4 export".
   - `docs/SCENARIO_DSL.md`: описать `urban-patrol`, `urban_state`,
     `UrbanMap`, route loop, AABB obstacles, validation errors.
   - `docs/EXTENSION_GUIDE.md`: добавить Urban как пример mission-level
     extension path и подчеркнуть road graph first.
   - `docs/REPLAY.md`: обновить только если добавлены Urban replay events.
   - `docs/BENCHMARK_RESULTS.md`/`docs/REGRESSION.md`: если затрагиваются,
     отметить, что M64 не обновляет benchmark baseline и не требует 500/1000
     seed run.

9. Не делать в M64.
   - Не добавлять bus detector.
   - Не добавлять lidar/raycast.
   - Не добавлять dynamic obstacles.
   - Не делать multi-agent route conflicts/deconfliction.
   - Не делать PX4/SITL export.
   - Не вводить polygon dependency. Если obstacle check нужен, ограничиться
     локальным AABB/segment check.
   - Не менять behavior existing missions beyond additive fields/defaults and
     docs/tests.

10. Команды проверки для реализации M64.
    - Форматирование: `cargo fmt --all`.
    - Lint: `cargo clippy --all-targets -- -D warnings`.
    - Build/check:
      `/home/formi/.local/bin/runlim cargo check -p swarm-types -p swarm-sim -p swarm-scenarios -p swarm-examples`.
    - Targeted tests:
      `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-types urban`;
      `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban`;
      `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-scenarios urban`;
      `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim --test scenario_catalog`.
    - Report/export tests if metrics are exported:
      `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim report_export`.
    - Replay tests if replay events are added:
      `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay urban`.
    - Docs smoke tests if they exist or are added:
      `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`.
    - Optional smoke run only after `urban.patrol.json` can execute through the
      CLI without pretending benchmark evidence:
      `/home/formi/.local/bin/runlim cargo run -p swarm-examples --bin strategy_comparison -- --scenario-suite scenarios/urban.patrol.json --output-dir target/m64_urban_smoke`.
      If the CLI smoke is not meaningful because M64 only parses/plans, skip it
      and document the reason in the implementation summary.

# Testing strategy

## 1. Tests that need no refactoring

- `crates/swarm-types` unit tests:
  - valid `UrbanMap` passes validation;
  - duplicate node ids rejected;
  - duplicate edge ids rejected;
  - edge with missing `from`/`to` rejected with actionable field path;
  - negative/non-finite cost/length/corridor width rejected;
  - invalid AABB bounds rejected.
- `crates/swarm-sim` unit tests:
  - Dijkstra returns deterministic shortest route on a simple block;
  - deterministic tie-breaking picks the same edge sequence when two paths have
    equal cost;
  - route loop expands into expected ordered segments;
  - missing start/end node returns typed error;
  - blocked edge is not used when an alternative path exists;
  - no route available returns typed error, not panic;
  - judge reports blocked-edge violation for a manually invalid route;
  - judge reports AABB endpoint/intersection violation for a simple edge.
- DSL tests:
  - inline `urban-patrol` fixture parses and validates;
  - missing `urban_state` rejected for `mission == "urban-patrol"`;
  - route loop with unknown node rejected;
  - map edge with unknown endpoint rejected.
- Scenario catalog test:
  - `scenarios/urban.patrol.json` loads through existing scenario catalog.
- Metrics tests:
  - `RunMetrics` JSON with absent Urban fields deserializes using defaults;
  - Urban route length/planned/violation fields aggregate correctly if aggregate
    export is implemented.
- Replay tests, only if events are added:
  - Urban route/judge events serde roundtrip;
  - legacy replay logs without Urban events still deserialize.
- Docs smoke tests:
  - README/status mention M64 non-goals;
  - `docs/SCENARIO_DSL.md` documents `urban-patrol` and AABB-only scope;
  - docs do not claim lidar/bus/dynamic obstacle implementation.

## 2. Tests that need light refactoring

- Shared urban fixture builder for tests, to avoid duplicating the same block
  graph in `swarm-types`, `swarm-sim` and `swarm-scenarios`.
- Route assertion helper:
  compare edge id sequence, total length and endpoint node sequence with clear
  panic messages.
- Judge assertion helper:
  assert violation kind + edge/node/obstacle id without brittle string matching.
- Scenario catalog helper:
  existing `all_scenario_files_load` can stay, but adding a helper to load one
  named scenario would make Urban validation failures easier to diagnose.
- Docs/status assertion helper:
  reuse the style of existing docs smoke tests, but factor repeated phrase
  checks if M64 adds several docs assertions.

## 3. Tests that need heavy refactoring

- Random graph generation with guaranteed route existence and deterministic
  shortest-path oracle.
- Property tests for Dijkstra over arbitrary positive-weight graphs.
- Polygon geometry tests; intentionally out of M64.
- Multi-planner abstraction tests if A* or multiple route planners are added.
- Full route-progress simulation/e2e tests for Urban Patrol; this belongs to
  M65 unless M64 deliberately implements progress.
- Multi-agent route conflict/deconfliction tests; this belongs to later Urban
  multi-agent prep, not M64.

Автотест gaps для M64:

- Реальный lidar/raycast, bus detection, dynamic obstacle behavior,
  multi-agent deconfliction and PX4/SITL export не покрываются, потому что это
  явные non-goals M64.
- Publication benchmark/1000 seeds не запускается: M64 не обновляет benchmark
  evidence. Если нужен performance sanity, достаточно маленького deterministic
  CLI smoke run, но не benchmark claim.

# Risks and tradeoffs

- Риск смешать Urban road graph с существующим `InspectionGraph`.
  Митигировать отдельными Urban types и явными docs boundaries.
- Риск преждевременно добавить полноценный `TaskKind`/adapter без runtime
  semantics.
  Предпочтительно в M64 использовать `Waypoint` placeholders и оставить
  mission completion semantics для M65.
- Риск раздуть geometry.
  Ограничиться AABB и простым segment/AABB check; polygons оставить future work.
- Риск недетерминизма в planner tie-breaking.
  Проверять equal-cost graph тестом и сортировать adjacency/order explicitly.
- Риск сломать старые scenario JSON из-за новых required fields.
  Все новые поля должны быть optional/defaulted; urban validation включается
  только для `mission == "urban-patrol"`.
- Риск ложных user-facing claims.
  Docs должны говорить "foundation", "route planning/judge skeleton", not
  "Urban Patrol/Search complete".
- Риск расширить CSV/Markdown schemas без тестов.
  Любое новое user-facing metric field должно сопровождаться export/header
  tests.
- Риск увеличить время test suite из-за CLI smoke.
  CLI smoke оставить optional и маленьким; основной gate сделать unit/catalog
  tests.

# Что могло сломаться

- Поведение существующих missions:
  additive `RunConfig`/`RunMetrics` fields могут случайно изменить defaults или
  success computation. Проверять existing support matrix/regression targeted
  tests и JSON backward-compat tests.
- Scenario DSL compatibility:
  новые validation rules могут начать отклонять старые non-urban fixtures.
  Проверять `cargo test -p swarm-sim --test scenario_catalog` и inline negative
  tests, что urban-specific validation applies only to urban.
- Report/export contracts:
  новые metrics columns могут изменить CSV/Markdown order или JSON shape.
  Проверять `report_export` tests and benchmark pack tests when export changes.
- Replay compatibility:
  новые event variants могут сломать old log deserialization or summary output.
  Проверять replay roundtrip and legacy deserialization tests.
- Performance/resources:
  Dijkstra на маленьком graph должен быть дешевым; если planner API позволит
  большие graphs, нужен тест/guard на отсутствие pathological loops. В M64
  достаточно deterministic unit tests on small graphs.
- Интеграции docs/status:
  README/status могут снова заявить больше, чем реализовано. Проверять docs
  smoke tests with required limitation phrases.

# Open questions

- Нужен ли новый `TaskKind` для Urban уже в M64, или лучше оставить
  `Waypoint` placeholders до M65? Текущий предпочтительный ответ: не добавлять
  новый kind без completion semantics.
- Где именно хранить `UrbanMap`: `Scenario::urban_map` или
  `RunConfig::urban_state.map`? Текущий предпочтительный ответ:
  `RunConfig::urban_state` как additive mission config, потому что existing
  mission-specific state уже живет в `RunConfig`; если при реализации окажется,
  что map лучше как scenario environment, можно выбрать `Scenario::urban_map`,
  но обязательно с serde default and compatibility tests.
- Добавлять ли replay events в M64 или только types/metrics placeholders?
  Ответ зависит от runner integration: если route planning фактически
  выполняется during run, событие `UrbanRoutePlanned` полезно; если M64 только
  parse/plan/judge API, replay events лучше отложить до M65.
- Должен ли `urban.patrol.json` выполнять route progress в CLI уже в M64?
  Предпочтительно нет: M64 foundation может ограничиться load/validate/plan/
  judge. Полноценная patrol progress/completion belongs to M65.
