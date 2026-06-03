# Context

Планируем M75 - Urban Mission Realism Follow-up из
`docs_raw/BEFORE_HARDWARE_A.23.md:701`. Цель M75 - добавить "useful realism"
для Urban missions без реальной физики, сенсоров и hardware dependencies:
moving semantic bus targets, perimeter patrol route patterns, deterministic
waypoint generation and metrics.

Текущая база уже имеет:

- M64-M65 Urban road-graph substrate и one-agent `urban-patrol`
  (`crates/swarm-types/src/urban.rs:112`, `crates/swarm-sim/src/runner/urban_patrol.rs:53`).
- M66 static mocked bus search (`crates/swarm-types/src/urban.rs:162`,
  `crates/swarm-sim/src/urban/detection.rs:20`,
  `crates/swarm-sim/src/runner/urban_search.rs:8`).
- M70 route export boundary for `urban-patrol` (`crates/swarm-sim/src/urban/route_export.rs`,
  `crates/swarm-examples/src/sitl_plan.rs:381`).
- M74 blocked-route decision layer with temporary obstacles and Wait/Replan/Abort
  (`crates/swarm-types/src/urban.rs:189`,
  `crates/swarm-sim/src/runner/urban_patrol.rs:283`).

Важно не смешивать M75 Urban perimeter patrol со старым `inspection` perimeter
benchmark (`scenarios/inspection.perimeter.json`, README perimeter rows). M75
должен остаться частью Urban mission layer and SITL-waypoint-compatible route
logic, а не переписыванием inspection mission.

Notion/GitLab context не использовался: prompt не содержит Notion task или
GitLab/MR target, `notion_policy=optional`; обязательные протоколы были
прочитаны.

# Investigation context

`INVESTIGATION.md` в workspace отсутствует.

Релевантные findings из локального исследования:

- `UrbanBus` сейчас статичен: `id`, `pose`, `active_from_tick`,
  `active_until_tick` (`crates/swarm-types/src/urban.rs:162`).
- `UrbanSearchState::validate` проверяет duplicate bus ids, finite static pose,
  active window and detector probability (`crates/swarm-types/src/urban.rs:400`).
- `detect_buses` сейчас не принимает map и считает distance до `bus.pose`
  (`crates/swarm-sim/src/urban/detection.rs:20`).
- `BusObserved` / `BusDetected` replay events уже хранят observed `pose`, поэтому
  для moving bus не требуется новый event type, если detector будет отдавать
  sampled pose (`crates/swarm-replay/src/event_log.rs:167`).
- `run_urban_search` вызывает `detect_buses` на tick 0 и на каждом movement tick
  (`crates/swarm-sim/src/runner/urban_search.rs:182`,
  `crates/swarm-sim/src/runner/urban_search.rs:300`).
- `urban_patrol_metrics` уже централизует Urban route metrics; M75 perimeter
  metrics нужно добавить туда и в `RunMetrics`
  (`crates/swarm-sim/src/runner/urban_metrics.rs:21`,
  `crates/swarm-metrics/src/metrics/run.rs:128`).
- `RunConfig::UrbanState` сейчас содержит `map`, `route_loop`, `start_node`,
  `planner`, `temporary_obstacles`, `blocked_route_policy`
  (`crates/swarm-sim/src/runner/types.rs:140`).
- Scenario builders for Urban profiles живут в
  `crates/swarm-scenarios/src/urban.rs:14`; static bus fixture is at
  `crates/swarm-scenarios/src/urban.rs:242`.
- Existing unit/integration tests for Urban runner and detector are in
  `crates/swarm-sim/src/urban/tests.rs:472`,
  `crates/swarm-sim/src/runner/tests.rs:574`,
  `crates/swarm-scenarios/src/urban.rs:570`,
  `crates/swarm-sim/src/dsl/tests.rs:548`.

# Affected components

- `crates/swarm-types/src/urban.rs`
  - Add `UrbanBusStop`, `UrbanBusRoute`, `UrbanBus::route`.
  - Add perimeter input type if implemented as typed field rather than only
    helper input, e.g. `UrbanPerimeterPatrol`.
  - Extend validation for moving bus routes and perimeter declarations.

- `crates/swarm-sim/src/urban/detection.rs`
  - Change detector to sample bus pose at tick from map-aware bus route.

- `crates/swarm-sim/src/urban/geometry.rs` or new
  `crates/swarm-sim/src/urban/perimeter.rs`
  - Add deterministic perimeter waypoint generation.

- `crates/swarm-sim/src/urban/mod.rs`
  - Re-export new bus/perimeter helpers.

- `crates/swarm-sim/src/runner/types.rs`
  - Add optional perimeter patrol configuration to `UrbanState`, or a small
    dedicated field such as `perimeter_patrol: Option<UrbanPerimeterPatrol>`.

- `crates/swarm-sim/src/runner/urban_patrol.rs`
  - Route source selection: current `route_loop` path remains default; perimeter
    route becomes optional mode that produces deterministic route/waypoints.

- `crates/swarm-sim/src/runner/urban_search.rs`
  - Pass `urban_state.map` into detector and preserve static bus behavior.

- `crates/swarm-sim/src/runner/urban_metrics.rs`,
  `crates/swarm-metrics/src/metrics/run.rs`,
  `crates/swarm-metrics/src/metrics/aggregate.rs`,
  `crates/swarm-sim/src/report_export/*`
  - Add and export perimeter metrics:
    `perimeter_completion_rate`, `perimeter_length_m`,
    `time_to_complete_perimeter`, `perimeter_violations`.

- `crates/swarm-replay/src/event_log.rs`,
  `crates/swarm-replay/src/replay/summary.rs`
  - Existing bus events can carry moving observed pose. Add perimeter-specific
    replay events only if route events cannot make perimeter progress clear.

- `crates/swarm-scenarios/src/urban.rs`
  - Add profiles/builders for moving bus and square perimeter patrol.

- `scenarios/urban.search.json` and new scenario fixtures
  - Keep static bus backward-compatible; add moving bus and square perimeter
    fixtures.

- Docs:
  - `README.md`
  - `docs/STATUS.md`
  - `docs/SCENARIO_DSL.md`
  - `docs/REPLAY.md`
  - `docs/BENCHMARK_RESULTS.md` if scenario-suite/regression rows are updated
  - `docs/EXTENSION_GUIDE.md`
  - `docs/SITL_SETUP.md` if M70 dry-run export compatibility is documented

# Implementation steps

1. Update Urban bus types and backward-compatible JSON schema.
   - Files:
     - `crates/swarm-types/src/urban.rs:162`
     - `crates/swarm-types/src/lib.rs:25`
   - Add:
     ```rust
     pub struct UrbanBusStop {
         pub node_id: UrbanNodeId,
         pub arrival_tick: u64,
     }

     pub struct UrbanBusRoute {
         pub stops: Vec<UrbanBusStop>,
         pub speed_m_per_tick: f64,
     }

     pub struct UrbanBus {
         pub id: UrbanBusId,
         pub pose: Pose,
         pub active_from_tick: Option<u64>,
         pub active_until_tick: Option<u64>,
         #[serde(default, skip_serializing_if = "Option::is_none")]
         pub route: Option<UrbanBusRoute>,
     }
     ```
   - Expected result: old static bus JSON still parses; new moving bus route can
     be serialized/deserialized.

2. Implement `UrbanBus::pose_at_tick`.
   - Files:
     - `crates/swarm-types/src/urban.rs:162`
     - optional helper in `crates/swarm-sim/src/urban/geometry.rs:1` if pose
       interpolation should stay outside `swarm-types`.
   - Contract:
     ```rust
     impl UrbanBus {
         pub fn pose_at_tick(&self, map: &UrbanMap, tick: u64) -> Option<Pose> {
             // static: use pose + active window
             // moving: find adjacent stops by arrival_tick and interpolate node poses
             // outside route window: None
         }
     }
     ```
   - Route semantics:
     - zero route: invalid in validation, `pose_at_tick` returns `None`;
     - one stop: pose exists only at/within that stop's route window;
     - repeated or non-monotonic `arrival_tick`: validation error;
     - stop node missing from map: validation error;
     - `speed_m_per_tick` must be finite and `> 0.0`, even if interpolation uses
       scheduled ticks first. Keep it as route metadata for future generator use.
   - Expected result: deterministic static and moving pose sampling.

3. Extend Urban search validation for moving bus routes.
   - Files:
     - `crates/swarm-types/src/urban.rs:400`
     - `crates/swarm-sim/src/dsl/urban_validate.rs:26`
     - `crates/swarm-sim/src/dsl/tests.rs:548`
   - Add a map-aware validation method, for example:
     ```rust
     impl UrbanSearchState {
         pub fn validate_with_map(&self, map: &UrbanMap) -> Vec<UrbanMapValidationError>;
     }
     ```
   - Keep existing `validate()` as static/backward-compatible shallow validation,
     but make DSL validation call `validate_with_map` when `urban_state.map` is
     available.
   - Expected result: scenario DSL rejects unknown bus stop node ids, invalid
     route timing, invalid speed, and still accepts old static buses.

4. Make detector sample moving bus pose at current tick.
   - Files:
     - `crates/swarm-sim/src/urban/detection.rs:20`
     - `crates/swarm-sim/src/runner/urban_search.rs:182`
     - `crates/swarm-sim/src/runner/urban_search.rs:300`
     - `crates/swarm-sim/src/runner/urban_events.rs:94`
   - Change detector signature to include the map:
     ```rust
     pub fn detect_buses(
         map: &UrbanMap,
         agent_pose: Pose,
         tick: u64,
         scenario_seed: u64,
         search_state: &UrbanSearchState,
     ) -> UrbanDetectionOutcome
     ```
   - Replace `bus.pose` with `bus.pose_at_tick(map, tick)?`.
   - Keep sorting by bus id so deterministic probability draw order remains
     stable after route sampling.
   - Expected result: `BusObserved` and `BusDetected` replay events record the
     actual sampled moving bus pose, while false positives remain pose of the
     agent/detector event and do not complete the mission.

5. Add moving-bus scenario profiles and fixtures.
   - Files:
     - `crates/swarm-scenarios/src/urban.rs:14`
     - `scenarios/urban.search.json`
     - possibly new `scenarios/urban.moving-bus.json`
     - `crates/swarm-sim/src/regression/suites.rs:231` if a smoke suite is added
   - Add `UrbanProfile::SearchMovingBus` and profile name
     `"search-moving-bus"`.
   - Build a route that intersects the scout only at a predictable tick, for
     example bus route `n0(t=0) -> n1(t=4) -> n2(t=8)`, detector probability 1.0,
     false positive 0.0.
   - Expected result: a portable fixture where timing/range determines detection,
     and static bus fixture remains unchanged.

6. Add deterministic perimeter waypoint generation.
   - Files:
     - new `crates/swarm-sim/src/urban/perimeter.rs` or extend
       `crates/swarm-sim/src/urban/geometry.rs:1`
     - `crates/swarm-sim/src/urban/mod.rs:1`
   - Add:
     ```rust
     pub fn perimeter_waypoints(polygon: &[Pose], spacing_m: f64) -> Result<Vec<Pose>, UrbanRouteError>
     ```
   - Semantics:
     - reject fewer than 3 points;
     - reject non-finite coordinates and `spacing_m <= 0.0`;
     - treat input as closed even if last point is not equal to first;
     - output is closed: first generated waypoint equals final waypoint, or
       final waypoint is an explicit return-to-start marker;
     - deterministic edge walk in input order;
     - no arbitrary polygon geometry beyond perimeter sampling; no point-in-
       polygon, navmesh, occlusion, or raycast.
   - Expected result: a reusable waypoint generator for square/convex perimeter
     fixtures.

7. Add optional perimeter config to Urban state.
   - Files:
     - `crates/swarm-sim/src/runner/types.rs:140`
     - `crates/swarm-types/src/urban.rs`
     - `crates/swarm-sim/src/dsl/tests.rs`
   - Suggested type:
     ```rust
     pub struct UrbanPerimeterPatrol {
         pub polygon: Vec<Pose>,
         pub spacing_m: f64,
     }

     pub struct UrbanState {
         pub route_loop: UrbanRouteLoop,
         pub perimeter_patrol: Option<UrbanPerimeterPatrol>,
         // existing fields...
     }
     ```
   - Preserve `route_loop` as required for current `urban-patrol`; perimeter
     mode can generate route-like waypoints/segments internally or use a fixture
     map whose route loop follows the generated perimeter nodes.
   - Expected result: JSON can declare perimeter intent without breaking
     existing Urban Patrol/Search scenarios.

8. Integrate perimeter patrol into Urban runner.
   - Files:
     - `crates/swarm-sim/src/runner/urban_patrol.rs:53`
     - `crates/swarm-sim/src/runner/urban_metrics.rs:21`
     - `crates/swarm-sim/src/runner/urban_helpers.rs`
   - Use one of two implementation paths, preferring the simpler one after
     coding:
     - Path A: perimeter fixture builder converts polygon to `UrbanMap` +
       `UrbanRouteLoop`, runner stays mostly unchanged.
     - Path B: runner detects `urban_state.perimeter_patrol` and builds a
       temporary planned route from perimeter waypoints.
   - Expected result: square perimeter patrol completes deterministically and
     reports perimeter-specific metrics. Path A is lower-risk and keeps M70
     export compatibility cleaner.

9. Add perimeter metrics to run and aggregate reports.
   - Files:
     - `crates/swarm-metrics/src/metrics/run.rs:128`
     - `crates/swarm-metrics/src/metrics/aggregate.rs`
     - `crates/swarm-metrics/src/metrics/display.rs`
     - `crates/swarm-metrics/src/metrics/tests.rs`
     - `crates/swarm-sim/src/report_export/csv.rs`
     - `crates/swarm-sim/src/report_export/json.rs`
     - `crates/swarm-sim/src/report_export/compare.rs`
     - `crates/swarm-sim/src/report_export/focused.rs`
     - `crates/swarm-sim/src/benchmark/markdown.rs`
   - Add fields with `#[serde(default)]`:
     - `perimeter_completion_rate: f64`
     - `perimeter_length_m: f64`
     - `time_to_complete_perimeter: Option<u64>`
     - `perimeter_violations: u64`
   - Expected result: benchmark/report exports include M75 metrics without
     breaking deserialization of historical reports.

10. Add replay support only where necessary.
    - Files:
      - `crates/swarm-replay/src/event_log.rs:167`
      - `crates/swarm-replay/src/replay/summary.rs:3`
      - `crates/swarm-replay/src/replay/render.rs`
      - `docs/REPLAY.md:32`
    - Moving bus can reuse `BusObserved` and `BusDetected` because both already
      contain observed `pose`.
    - For perimeter, first prefer existing `UrbanRoutePlanned`,
      `UrbanSegmentEntered`, `UrbanSegmentCompleted`, `UrbanPatrolCompleted`.
      Add `UrbanPerimeterPlanned` only if existing route events cannot expose
      perimeter length/completion unambiguously.
    - Expected result: replay summaries remain readable and moving bus pose is
      visible without event-schema churn.

11. Add scenario catalog fixtures.
    - Files:
      - `crates/swarm-scenarios/src/urban.rs:570`
      - `crates/swarm-sim/tests/scenario_catalog.rs`
      - new or updated `scenarios/urban.search.json`
      - new `scenarios/urban.perimeter-patrol.json` if separate fixture is
        clearer than adding another entry to `urban.patrol.json`
    - Add tests that the moving bus fixture validates and detects at expected
      tick, static bus fixture still detects, and square perimeter patrol
      completes.
    - Expected result: fixtures are portable, deterministic and load through the
      existing scenario catalog.

12. Keep optional M70 dry-run export compatibility.
    - Files:
      - `crates/swarm-examples/src/sitl_plan.rs:381`
      - `crates/swarm-examples/tests/sitl_agent.rs` or split test module under
        `crates/swarm-examples/tests/sitl_agent/*`
      - `docs/SITL_SETUP.md:155`
    - If perimeter is represented as generated `UrbanMap` + `UrbanRouteLoop`,
      existing M70 export should work without protocol changes; add a dry-run
      test only if no real PX4/SIH is needed.
    - Expected result: perimeter fixture can produce SITL-compatible dry-run
      waypoint plan, or docs explicitly mark export as postponed if code path is
      not stable.

13. Update docs and user-facing status.
    - Files:
      - `README.md:300`, `README.md:655`, `README.md:743`
      - `docs/STATUS.md:52`
      - `docs/SCENARIO_DSL.md:180`
      - `docs/REPLAY.md:37`
      - `docs/EXTENSION_GUIDE.md:93`
      - `docs/SITL_SETUP.md:155`
      - `docs/BENCHMARK_RESULTS.md` only if regression/smoke evidence is added
    - Required wording:
      - moving bus is a semantic target schedule, not physical traffic model;
      - detector remains mocked, no CV/lidar/raycast;
      - perimeter patrol is waypoint mission realism, not field readiness;
      - no pursuit/intercept and no hardware readiness claim.
    - Expected result: README and docs reflect M75 without overstating realism.

14. Run formatting, clippy and targeted automated tests.
    - Commands:
      - `cargo fmt --all`
      - `timeout 300 cargo clippy --workspace --all-targets -- -D warnings`
      - `timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-types urban`
      - `timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban`
      - `timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-scenarios urban`
      - `timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim --test scenario_catalog`
      - if docs tests changed:
        `timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`
    - No long benchmark, PX4/SIH, Gazebo, HIL, hardware, installs or 1000-seed
      runs are required for M75 implementation.

# Testing strategy

## 1. Tests that need no refactoring

Implement together with functional changes:

- `bus_pose_at_tick_static_returns_fixed_pose`
  - file: `crates/swarm-types/src/urban.rs`
  - static bus, no route, active window includes tick, returns original pose.
- `bus_pose_at_tick_static_returns_none_outside_window`
  - file: `crates/swarm-types/src/urban.rs`
  - preserves old active window semantics.
- `bus_pose_at_tick_interpolates_between_stops`
  - file: `crates/swarm-types/src/urban.rs`
  - two stops at known nodes, tick midpoint returns midpoint pose.
- `bus_pose_at_tick_returns_none_outside_route_window`
  - file: `crates/swarm-types/src/urban.rs`
  - moving route before first and after last scheduled stop.
- `urban_search_state_validation_rejects_unknown_bus_route_stop`
  - file: `crates/swarm-types/src/urban.rs` or `crates/swarm-sim/src/dsl/tests.rs`
  - map-aware validation rejects unknown `node_id`.
- `urban_search_state_validation_rejects_non_monotonic_bus_route`
  - file: `crates/swarm-types/src/urban.rs`
  - route stops must be strictly ordered by `arrival_tick`.
- `detect_buses_finds_moving_bus_when_in_range`
  - file: `crates/swarm-sim/src/urban/tests.rs`
  - sampled moving pose is in range and probability=1.
- `detect_buses_misses_moving_bus_out_of_range`
  - file: `crates/swarm-sim/src/urban/tests.rs`
  - sampled moving pose is out of range; no observation/detection.
- `detect_buses_records_sampled_moving_pose`
  - file: `crates/swarm-sim/src/urban/tests.rs`
  - observation pose equals interpolated pose, not static fallback pose.
- `urban_search_static_bus_fixture_detects_target`
  - existing file: `crates/swarm-scenarios/src/urban.rs:637`
  - keep and adapt for new `route: None` default.
- `urban_search_moving_bus_fixture_detects_at_expected_tick`
  - file: `crates/swarm-scenarios/src/urban.rs`
  - verifies fixture-level timing.
- `perimeter_waypoints_square_correct_count`
  - file: `crates/swarm-sim/src/urban/tests.rs`
  - square 20x20 with spacing 10 gives deterministic expected count.
- `perimeter_waypoints_is_deterministic`
  - file: `crates/swarm-sim/src/urban/tests.rs`
  - two calls equal.
- `perimeter_waypoints_closed_route`
  - file: `crates/swarm-sim/src/urban/tests.rs`
  - route returns to start.
- `perimeter_waypoints_rejects_invalid_spacing`
  - file: `crates/swarm-sim/src/urban/tests.rs`
  - negative/zero/non-finite spacing rejected.
- `perimeter_patrol_completes_on_square`
  - file: `crates/swarm-sim/src/runner/tests.rs` or
    `crates/swarm-scenarios/src/urban.rs`
  - metrics success, completion rate 1.0, no perimeter violations.
- `perimeter_patrol_metrics_exported`
  - file: `crates/swarm-metrics/src/metrics/tests.rs` and report export tests.
- Docs smoke tests in `crates/swarm-examples/tests/sitl_docs.rs`
  - required phrases: mocked detector, semantic target, no physics, no hardware
    readiness.

## 2. Tests that need light refactoring

- Bus route fixture builder
  - Extract helpers from `crates/swarm-sim/src/urban/tests.rs:10` and
    `crates/swarm-sim/src/runner/tests.rs:548` so detector/runner tests can
    build the same moving bus route without duplication.
- Moving-target detector fixture
  - Update helper currently taking only `bus_pose` to optionally set
    `UrbanBusRoute`.
- Shared perimeter builder for convex polygon
  - Add helper for square polygon and route-fixture conversion.
- Metrics assertion helper for perimeter completion
  - Avoid duplicating assertions for `perimeter_completion_rate`,
    `perimeter_length_m`, `time_to_complete_perimeter`, and
    `perimeter_violations`.
- Scenario catalog helper
  - Reuse existing `crates/swarm-sim/tests/scenario_catalog.rs` Urban assertions
    for both moving bus and perimeter fixtures.

## 3. Tests that need heavy refactoring

Do not implement as required M75 scope unless the implementation naturally
creates the infrastructure:

- Detection probability multi-seed stability tests
  - Needs a broader seeded statistical harness; current M75 deterministic tests
    should use probability 0/1.
- Line-of-sight building occlusion
  - Explicit non-goal for M75; requires geometry/raycast model.
- Property test: convex polygon waypoints lie on perimeter
  - Useful later, but needs robust numeric tolerances and polygon generators.
- Multi-agent perimeter partition tests
  - Requires Urban multi-agent route ownership/deconfliction beyond M75.
- Full dry-run export artifact diff for perimeter route
  - Useful after the perimeter representation stabilizes; can be added as an
    M70/M75 integration follow-up.

# Risks and tradeoffs

- API/schema risk: adding `UrbanBus::route` and perimeter config changes JSON
  shape. Use `#[serde(default, skip_serializing_if = "Option::is_none")]` and
  keep static `pose` required for backward compatibility.
- Semantic risk: `speed_m_per_tick` could conflict with scheduled
  `arrival_tick`. For M75, scheduled stops should be authoritative; speed is
  metadata/generator hint. Document this explicitly.
- Determinism risk: moving bus detection must keep stable bus ordering before
  deterministic RNG draws. Sort observations by bus id after pose sampling.
- Metrics compatibility risk: adding `RunMetrics` fields requires updating
  aggregate, CSV, JSON, focused report and tests, otherwise compile failures or
  missing output columns are likely.
- Scope creep risk: perimeter patrol can easily become arbitrary geometry,
  navmesh or physical path planning. Keep it to deterministic perimeter
  waypoint generation and route-compatible simulation.
- Replay compatibility risk: adding new replay event variants increases schema
  churn. Prefer existing Urban route and bus events unless a new event is needed
  for unambiguous perimeter summaries.
- SITL/export tradeoff: representing perimeter as generated Urban route nodes is
  simpler and preserves M70 export compatibility; direct runner-only perimeter
  waypoints may be faster but harder to export.
- Existing inspection perimeter confusion: docs must clearly say M75 perimeter
  is Urban waypoint mission realism, not the older `inspection` perimeter
  benchmark.

# Open questions

1. Should perimeter patrol be a new mission string/profile, for example
   `urban-perimeter`, or remain `urban-patrol` with optional
   `run_config.urban_state.perimeter_patrol`?
   - Preferred implementation: keep `urban-patrol` and add a profile/fixture
     first, to avoid a new mission dispatch path unless needed.

2. Should moving bus `speed_m_per_tick` participate in interpolation or remain
   metadata while `arrival_tick` defines the schedule?
   - Preferred implementation: `arrival_tick` defines deterministic schedule;
     `speed_m_per_tick` is validated and documented as route metadata/generator
     hint for future M76 scenarios.

3. Should perimeter waypoint generation output `Pose` only or route identity
   metadata too?
   - Preferred implementation: start with `Pose` helper plus fixture conversion
     to Urban nodes/edges so M70 route identity and export machinery remain
     usable.

4. Should M75 add a regression suite entry?
   - Preferred implementation: add focused unit/integration tests and scenario
     catalog tests first. Add a smoke regression only if it is fast and does not
     require benchmark reruns.
