# Context

Планируем реализацию `M65 - Urban Patrol v0` по линейному roadmap
`docs_raw/DRONE_C.21.md`.

M63 уже закрыл status/evidence cleanup, M64 уже добавил Urban foundation:

- `UrbanMap`, `UrbanNode`, `UrbanEdge`, `UrbanRouteLoop`,
  `UrbanPlannedRoute`, `UrbanViolation` in `crates/swarm-types/src/urban.rs`;
- deterministic Dijkstra route planning, route-loop expansion and static
  AABB/blocked-edge judge in `crates/swarm-sim/src/urban.rs`;
- `run_config.urban_state` in `crates/swarm-sim/src/runner.rs`;
- `scenarios/urban.patrol.json`;
- Urban metric skeleton in `RunMetrics`, `AggregateMetrics`,
  JSON/CSV/Markdown reports;
- docs explicitly say that M64 is only foundation and that real Urban Patrol
  progress/completion belongs to M65.

M65 should turn that foundation into the first user-visible Urban simulation
capability:

> One drone patrols a city-block loop and completes it without judge
> violations.

Architectural boundary remains the same:

- this project is mission-level simulation, planning, replay and metrics;
- it is not a low-level flight controller, physics engine, SLAM stack,
  lidar/object-recognition stack or PX4 replacement;
- M65 must not add buses, dynamic obstacle avoidance, multi-agent
  deconfliction, PX4/SITL claims or visual UI.

Protocols read for this plan:

- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`.

No Notion task or GitLab MR was requested in the prompt; `notion_policy` is
`optional`, so no external Notion/GitLab reads are required.

# Investigation context

`INVESTIGATION.md` is absent in this workspace.

Local context inspected:

- `docs_raw/DRONE_C.21.md` describes M65 as the first full Urban Patrol
  milestone after M64 foundation.
- `PLAN.md` was absent at HEAD before this planning round
  (`5472dee removed plan`), so this file is created as a fresh M65 plan.
- `crates/swarm-types/src/urban.rs` already contains the shared Urban map,
  route and violation types.
- `crates/swarm-sim/src/urban.rs` already contains deterministic route planning
  and static judge helpers.
- `crates/swarm-sim/src/runner.rs` currently computes only foundation metrics:
  `urban_route_planned`, `urban_route_length_m`, `urban_violation_count`, and
  hardcodes `urban_route_completed = false`.
- Existing generic movement/task allocation can move agents direct-to-task; M65
  must avoid using that for Urban Patrol, because direct waypoint motion can
  ignore the road graph.
- `crates/swarm-replay/src/event_log.rs` currently has no Urban replay event
  variants.
- `crates/swarm-replay/src/replay.rs` and
  `crates/swarm-examples/src/bin/replay.rs` summarize common/SAR/inspection/
  wildfire events, but not Urban route progress.
- `crates/swarm-scenarios/src/urban.rs` and `scenarios/urban.patrol.json`
  already define a small block route loop with one agent, a blocked diagonal,
  and an AABB building.
- `README.md`, `docs/STATUS.md`, `docs/SCENARIO_DSL.md`,
  `docs/EXTENSION_GUIDE.md`, `docs/REPLAY.md`,
  `docs/BENCHMARK_RESULTS.md`, and `docs/REGRESSION.md` currently describe
  M64 as foundation and identify M65 as the next progress/completion boundary.

No code/test/build command was needed for planning. If implementation needs
quick checks, every command below must use a hard timeout of at most five
minutes; any long benchmark or determinism sweep must be documented as future
evidence work and not hidden inside M65 implementation.

# Affected components

- `crates/swarm-types/src/urban.rs`
  - Add execution/progress-friendly Urban types if needed:
    `UrbanPatrolStatus`, `UrbanPatrolViolationReason`, possibly an
    execution-level violation struct with tick/agent/pose/segment context.
  - Keep existing map/planned-route types stable and additive.

- `crates/swarm-types/src/lib.rs`
  - Export any new Urban execution/status types.

- `crates/swarm-sim/src/urban.rs`
  - Add route-following helpers:
    interpolation along an `UrbanRouteSegment`;
    segment endpoint lookup;
    distance-per-tick calculation;
    execution judge helper for current pose/segment.
  - Keep Dijkstra/judge deterministic.

- `crates/swarm-sim/src/runner.rs`
  - Replace M64 `urban_route_completed = false` placeholder with actual Urban
    Patrol runtime state.
  - Ensure Urban Patrol agent follows the planned road-graph route, not generic
    direct-to-nearest-task movement.
  - Emit Urban replay events when `run_with_log` is used.
  - End the mission early when patrol completes or fails by violation/timeout.

- `crates/swarm-metrics/src/metrics.rs`
  - Add M65 user-facing metrics with `#[serde(default)]`:
    `urban_patrol_completed`,
    `urban_time_to_complete_loop`,
    `urban_distance_travelled_m`,
    `urban_route_efficiency`,
    `urban_replan_count`.
  - Keep M64 fields and set `urban_route_completed` consistently with
    `urban_patrol_completed` for backward compatibility.
  - Add aggregate fields:
    `urban_patrol_completed_rate`,
    `avg_urban_time_to_complete_loop`,
    `avg_urban_distance_travelled_m`,
    `avg_urban_route_efficiency`,
    `avg_urban_replan_count`.

- `crates/swarm-sim/src/report_export.rs`
  - Add M65 aggregate fields to JSON/CSV export rows.
  - Add M65 columns to Markdown `ComparisonReport` table and focused
    `urban-patrol` report.
  - Update compare-report equality checks.

- `crates/swarm-sim/src/benchmark.rs`
  - Update `ComparisonReport::fmt` if M65 metrics are displayed in the common
    Markdown table.
  - Preserve existing column order assumptions where tests already parse
    `Completion`.

- `crates/swarm-replay/src/event_log.rs`
  - Add additive event variants:
    `UrbanRoutePlanned`,
    `UrbanSegmentEntered`,
    `UrbanSegmentCompleted`,
    `UrbanViolation`,
    `UrbanPatrolCompleted`.
  - Use stable serde names and avoid bumping schema unless compatibility tests
    show it is necessary.

- `crates/swarm-replay/src/replay.rs`
  - Extend `ReplaySummary` with Urban counters:
    route planned count, segment entered/completed count, violation count,
    patrol completed count, optional completion tick.
  - Count new events in `summarize`.
  - Include Urban event handling in snapshot/replay where relevant.

- `crates/swarm-examples/src/bin/replay.rs`
  - Print Urban summary lines under `--summary`, so route completion and
    violations are visible from CLI output.

- `crates/swarm-scenarios/src/urban.rs`
  - Update the existing `PatrolSmallBlock` fixture so it completes under the
    M65 runner.
  - Add controlled invalid/timeout fixture helpers if useful for tests:
    blocked required segment, obstacle-crossing route, too-low max_ticks.

- `scenarios/urban.patrol.json`
  - Update description from M64 foundation to M65 executable patrol if the file
    is still the primary happy-path fixture.
  - Keep the JSON portable and deterministic.
  - Add a second invalid JSON fixture only if scenario catalog tests need a
    file-level negative case; prefer inline fixtures for negative unit tests.

- `crates/swarm-sim/src/dsl.rs`
  - Tighten `urban-patrol` validation if M65 requires executable patrol
    semantics:
    route loop must be plannable, at least one alive agent, agent start node or
    start pose must match route start under documented tolerance, planner must
    be supported.
  - Keep validation scoped to `mission == "urban-patrol"` only.

- `crates/swarm-examples/src/regression_lib.rs`
  - Ensure `urban-patrol` regression smoke uses M65 completion semantics and
    remains deterministic.

- `crates/swarm-examples/src/bin/strategy_comparison.rs`
  - Ensure explicit `--mission urban-patrol` runs the M65 executable fixture.
  - Keep `urban-patrol` out of `--mission all` unless the implementation
    deliberately updates benchmark scope and docs.

- `crates/swarm-sim/tests/scenario_catalog.rs`
  - Update catalog tests for executable Urban Patrol expectations.

- `crates/swarm-examples/tests/*`
  - Add/adjust replay CLI, docs smoke and regression smoke tests where the
    current test crate layout already has matching coverage.

- Documentation:
  - `README.md`;
  - `docs/STATUS.md`;
  - `docs/SCENARIO_DSL.md`;
  - `docs/EXTENSION_GUIDE.md`;
  - `docs/REPLAY.md`;
  - `docs/REGRESSION.md`;
  - `docs/BENCHMARK_RESULTS.md`.

# Implementation steps

1. Define exact Urban Patrol semantics in code comments/tests, not only docs.
   Files:
   - `crates/swarm-sim/src/urban.rs`;
   - `crates/swarm-sim/src/runner.rs`;
   - `docs/SCENARIO_DSL.md`.

   Rules to implement:

   - planned route is `expand_route_loop(map, route_loop)`;
   - the route is ordered by segment index;
   - patrol is completed when the single selected patrol agent traverses every
     planned segment in order;
   - starting pose is the first route node pose, or must be snapped/validated
     against `urban_state.start_node` under a small documented tolerance;
   - failure by judge violation beats completion;
   - failure by timeout happens when `max_ticks` expires before completion;
   - M65 v0 has no replanning, so `urban_replan_count = 0`.

2. Add a dedicated Urban Patrol runtime/progress model.
   Files:
   - `crates/swarm-sim/src/urban.rs`;
   - possibly `crates/swarm-types/src/urban.rs` if shared status structs are
     needed.

   Suggested local structs/functions:

   - `UrbanPatrolProgress`:
     current segment index, distance on segment, total distance, entered set,
     completed segments, completion tick, violation state.
   - `UrbanPatrolStep`:
     events generated during a tick, new pose, segment transitions.
   - `step_urban_patrol(map, route, agent_id, pose, speed_m_per_tick, tick, progress)`.

   Keep this deterministic and independent of generic task assignment logic.

3. Integrate Urban Patrol into `ScenarioRunner`.
   File:
   - `crates/swarm-sim/src/runner.rs`.

   Implementation direction:

   - initialize planned route once before the tick loop;
   - run `judge_route` before execution and fail early if the planned route is
     statically invalid;
   - select exactly one alive patrol agent for M65 v0; reject/mark unsupported
     if no agent exists;
   - disable or bypass generic direct-to-task movement for Urban Patrol so the
     road graph is authoritative;
   - update the selected agent pose along the current segment each tick;
   - update `total_distance_travelled` and Urban-specific distance metrics from
     route-following, not direct waypoint movement;
   - break the loop early on `UrbanPatrolCompleted` or `UrbanViolation`;
   - compute `success` from:
     `urban_patrol_completed && urban_violation_count == 0 && !timeout`.

   Prefer a helper such as `run_urban_patrol_runtime(...)` inside runner or
   `swarm-sim/src/urban.rs` if that keeps the generic loop readable. Some
   refactoring of `RunMetrics` construction is acceptable.

4. Add execution judge integration.
   Files:
   - `crates/swarm-sim/src/urban.rs`;
   - `crates/swarm-sim/src/runner.rs`.

   Checks:

   - static route judge before tick loop;
   - during execution, current segment must match planned segment;
   - current pose must stay on/within the active segment corridor under a small
     tolerance, or v0 should document that execution follows the planned
     segment exactly and therefore dynamic execution judge only checks segment
     transitions;
   - any manually invalid route fixture must emit an `UrbanViolation` event and
     produce `success=false`.

5. Add Urban replay events and replay summary.
   Files:
   - `crates/swarm-replay/src/event_log.rs`;
   - `crates/swarm-replay/src/replay.rs`;
   - `crates/swarm-examples/src/bin/replay.rs`;
   - `docs/REPLAY.md`.

   Events:

   - `UrbanRoutePlanned { agent_id, tick, edge_ids, route_length_m }`;
   - `UrbanSegmentEntered { agent_id, tick, segment_index, edge_id, from, to }`;
   - `UrbanSegmentCompleted { agent_id, tick, segment_index, edge_id }`;
   - `UrbanViolation { agent_id, tick, segment_index, edge_id, pose, reason }`;
   - `UrbanPatrolCompleted { agent_id, tick, route_length_m, distance_travelled_m }`.

   The summary should report at least:

   - urban routes planned;
   - urban segments entered/completed;
   - urban violations;
   - urban patrol completions;
   - urban completion tick.

6. Extend metrics and reports.
   Files:
   - `crates/swarm-metrics/src/metrics.rs`;
   - `crates/swarm-sim/src/report_export.rs`;
   - `crates/swarm-sim/src/benchmark.rs`.

   Add per-run and aggregate fields listed in `Affected components`.
   `urban_route_efficiency` should be:

   ```text
   planned_route_length_m / urban_distance_travelled_m
   ```

   when distance is positive, otherwise `0.0`. For the happy-path fixture this
   should be near `1.0`.

   Export all new user-facing metrics through JSON, CSV and Markdown if they
   are added to `AggregateMetrics`.

7. Update scenario builders and fixtures.
   Files:
   - `crates/swarm-scenarios/src/urban.rs`;
   - `scenarios/urban.patrol.json`;
   - possibly `crates/swarm-sim/tests/scenario_catalog.rs`.

   Happy path:

   - one scout starts at `n0`;
   - route loop `n0 -> n1 -> n2 -> n3 -> n0`;
   - max_ticks enough for speed 2 m/s and 80 m route with 1s ticks;
   - patrol completes before `max_ticks`;
   - no judge violations.

   Negative cases can be inline test fixtures unless file-level coverage is
   needed:

   - timeout: max_ticks too low;
   - violation: required route segment blocked or intersects AABB.

8. Update CLI/regression integration.
   Files:
   - `crates/swarm-examples/src/regression_lib.rs`;
   - `crates/swarm-examples/src/bin/strategy_comparison.rs`;
   - `docs/REGRESSION.md`.

   Requirements:

   - explicit `--mission urban-patrol` should now be a meaningful simulation
     smoke, not only parse/plan validation;
   - keep it deterministic and portable;
   - do not silently add Urban Patrol to large `--mission all` release baselines
     unless docs and benchmark interpretation are updated in the same change.

9. Update user-facing documentation.
   Files:
   - `README.md`;
   - `docs/STATUS.md`;
   - `docs/SCENARIO_DSL.md`;
   - `docs/EXTENSION_GUIDE.md`;
   - `docs/REPLAY.md`;
   - `docs/REGRESSION.md`;
   - `docs/BENCHMARK_RESULTS.md`.

   Required wording:

   - M65 Urban Patrol v0 is simulation-only;
   - no lidar, no bus detector, no dynamic obstacle avoidance, no multi-agent
     route deconfliction, no PX4/SITL/hardware claim, no UI;
   - patrol completion predicate is ordered loop traversal without judge
     violations before timeout;
   - M62 benchmark evidence remains historical unless a fresh benchmark is
     explicitly run;
   - any regression smoke is not publication-grade evidence.

10. Keep long-running evidence out of M65 implementation.
    Files:
    - `docs/REGRESSION.md`;
    - `docs/BENCHMARK_RESULTS.md`;
    - outbox/implementation summary.

    Do not run 500/1000 seed benchmarks for M65 unless separately requested.
    If a long determinism sweep is desired, document it as future M69-style
    evidence work.

11. Verification commands for M65 implementation.

    Every command must have a hard five-minute timeout. Suggested commands:

    ```bash
    timeout 300 cargo fmt --all
    timeout 300 cargo clippy --all-targets -- -D warnings
    timeout 300 /home/formi/.local/bin/runlim cargo check -p swarm-types -p swarm-sim -p swarm-scenarios -p swarm-replay -p swarm-examples

    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban
    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-types urban
    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-scenarios urban
    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay urban
    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim report_export
    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim --test scenario_catalog
    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test replay_cli
    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs
    ```

    Optional CLI smoke, only if it completes quickly:

    ```bash
    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim \
      cargo run -p swarm-examples --bin strategy_comparison -- \
      --smoke --mission urban-patrol --output-dir target/m65_urban_patrol_smoke
    ```

    If the optional CLI smoke would exceed the time budget, skip it and record
    the skip explicitly.

# Testing strategy

## 1. Tests that need no refactoring

- `crates/swarm-sim` unit tests:
  - Urban Patrol happy path completes the small block route;
  - completion tick is less than `max_ticks`;
  - route progress enters/completes segments in order;
  - agent pose follows segment endpoints, not direct task shortcuts;
  - timeout fixture fails with `success=false`, `urban_patrol_completed=false`;
  - violation fixture fails with `urban_violation_count > 0`;
  - static planned-route violation emits an execution failure before claiming
    completion;
  - `urban_replan_count == 0` in v0.

- `crates/swarm-types` / `crates/swarm-sim` tests:
  - any new Urban status/reason types serde roundtrip;
  - route interpolation clamps exactly at segment end;
  - zero-length route or from==to handling is explicit and deterministic.

- `crates/swarm-replay` tests:
  - every new Urban event serde roundtrips;
  - legacy logs without Urban events still deserialize/summarize;
  - `summarize` counts `UrbanRoutePlanned`,
    `UrbanSegmentEntered`, `UrbanSegmentCompleted`, `UrbanViolation`,
    and `UrbanPatrolCompleted`.

- `crates/swarm-examples` replay CLI tests:
  - `replay --summary` includes Urban route planned/completed/violation counts
    for an inline or temp replay log;
  - old summary output still includes existing common counts.

- Scenario/catalog tests:
  - `scenarios/urban.patrol.json` loads and validates;
  - builder `UrbanProfile::PatrolSmallBlock` produces a run that completes;
  - invalid inline fixture is rejected or fails with judge violation depending
    on whether the invalidity is static DSL invalidity or runtime violation.

- Metrics/report tests:
  - `RunMetrics` absent M65 fields deserialize with defaults;
  - new M65 fields aggregate correctly;
  - JSON/CSV/Markdown export contains new headers and values;
  - focused `urban-patrol` report includes M65 metrics;
  - compare-report checks include every new aggregate field.

- Docs smoke tests:
  - README/status mention M65 Urban Patrol and simulation-only boundary;
  - docs mention no lidar, no buses, no dynamic obstacles, no PX4/hardware
    claim;
  - `docs/REPLAY.md` lists Urban events and summary counters;
  - `docs/SCENARIO_DSL.md` documents patrol completion semantics.

- Regression smoke:
  - explicit `urban-patrol` smoke run or direct regression-lib test is
    deterministic and portable;
  - no long seed sweep is required for M65.

## 2. Tests that need light refactoring

- Mission outcome assertion helper:
  - `assert_urban_completed(metrics)`;
  - `assert_urban_timeout(metrics)`;
  - `assert_urban_violation(metrics)`.

- Shared Urban fixture builder:
  - one happy-path block fixture;
  - one blocked-edge violation fixture;
  - one low-max-ticks timeout fixture.

- Route progress assertion helper:
  - compare segment indices and edge ids;
  - assert monotonic segment order;
  - assert final pose equals route start/end within tolerance.

- Replay summary fixture helper:
  - construct a small Urban event log without depending on filesystem or
    external result artifacts.

- Report header assertion helper:
  - reuse CSV/Markdown header checks for M64 and M65 Urban metric columns.

## 3. Tests that need heavy refactoring

- Property tests for random route loops with a guaranteed valid route.
- Random map tests with generated blocked edges/obstacles and an oracle for
  valid route existence.
- Long-run determinism sweep across jobs/seeds for Urban Patrol.
- Full route-trace diff tooling across replay logs.
- Multi-agent route conflict/deconfliction tests.
- Dynamic obstacle/bus detector tests, which belong to M66+.

Autotest gaps for M65:

- Real lidar/raycast, bus detection, dynamic obstacles, multi-agent
  deconfliction, PX4/SITL export and hardware behavior are not covered because
  they are explicit non-goals.
- Publication benchmark or 500/1000 seed evidence is not part of M65. If needed
  later, plan it as M69-style benchmark evidence.
- Visual validation is not needed because M65 has no visual UI.

# Risks and tradeoffs

- Risk: generic task movement shortcuts across the city block.
  Mitigation: Urban Patrol must bypass/disable generic direct-to-task movement
  and update pose only through ordered planned route segments.

- Risk: runner becomes harder to maintain if Urban logic is embedded directly
  into the large tick loop.
  Mitigation: keep route stepping and judge execution helpers in
  `swarm-sim/src/urban.rs`; runner should orchestrate state and metrics.

- Risk: metric naming confusion with existing generic route metrics.
  Mitigation: use `urban_` prefix for new M65 fields and keep M64 fields as
  backward-compatible aliases where needed.

- Risk: replay schema grows but summaries remain unhelpful.
  Mitigation: add both event variants and summary/CLI output in the same
  implementation, with tests.

- Risk: negative invalid-route fixture is rejected by DSL before runtime, so it
  does not test runtime judge failure.
  Mitigation: use two negative categories:
  static DSL-invalid fixtures for validation tests and runtime-invalid planned
  route/judge fixtures for execution failure tests.

- Risk: early completion changes existing success semantics for any scenario
  that happens to include `urban_state`.
  Mitigation: scope behavior to `urban_state.is_some()` and keep non-urban
  tests unchanged.

- Risk: adding Urban Patrol to `--mission all` changes historical benchmark
  interpretation.
  Mitigation: keep Urban explicit-only unless docs and benchmark results are
  intentionally refreshed.

- Risk: route-following tick math can introduce off-by-one completion ticks.
  Mitigation: test exact expected completion tick for the 80m, 2m/s, 1s tick
  happy path or document the chosen convention if completion happens at the
  next tick boundary.

# Что могло сломаться

- Existing mission behavior:
  - non-urban missions could regress if runner success computation or movement
    is changed globally.
  - Check with existing targeted scenario/runner tests and by keeping Urban
    branch guarded by `urban_state`.

- API/serialization contracts:
  - new `RunMetrics`, `AggregateMetrics`, replay event variants and JSON/CSV/
    Markdown columns are additive but may affect strict downstream parsers.
  - Check with serde default tests, export header tests and compare-report
    tests.

- Replay compatibility:
  - new Urban events must not break old logs.
  - Check with legacy replay deserialization/summary tests.

- Scenario DSL compatibility:
  - tightened `urban-patrol` validation must not reject non-urban fixtures.
  - Check with scenario catalog tests and inline validation tests.

- Performance/resources:
  - route stepping and Dijkstra over the small block should be cheap; bigger
    graphs may need planner caching or a guard.
  - Check with small deterministic tests; leave large sweeps to later
    milestone.

- Documentation/status accuracy:
  - docs may overclaim "Urban navigation" as real avoidance/hardware readiness.
  - Check docs smoke tests for non-goal phrases and simulation-only wording.

# Open questions

- Should `urban_route_completed` remain as the canonical field, or should M65
  add explicit `urban_patrol_completed` and keep `urban_route_completed` as a
  compatibility alias? Preferred answer: add `urban_patrol_completed` and set
  both fields consistently for now.

- Should the M65 runner path be a dedicated Urban Patrol branch or integrated
  into the generic tick loop? Preferred answer: dedicated route-following
  helper with a small guarded runner integration, to avoid generic movement
  shortcuts.

- What exact tick convention should completion use when a segment end is reached
  exactly on a tick boundary? Preferred answer: segment completion and patrol
  completion occur on that same tick.

- Should `urban-patrol` be included in `--mission all`? Preferred answer: no
  for M65, unless benchmark documentation is deliberately refreshed.

- Should invalid route scenario be a committed JSON fixture? Preferred answer:
  keep negative fixtures inline unless a file-level regression gate needs a
  portable invalid scenario.

- Should M65 write route trace files, or only replay events/metrics? Preferred
  answer: replay events and metrics only; detailed route trace export belongs
  to M67 unless it is trivial.
