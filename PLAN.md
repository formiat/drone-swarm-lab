# Context

Планируем реализацию `M66 - Urban Search v1` по линейному roadmap
`docs_raw/DRONE_C.21.md`.

Текущая база уже прошла M64/M65:

- `UrbanMap`, `UrbanNode`, `UrbanEdge`, `UrbanRouteLoop`,
  `UrbanPlannedRoute`, `UrbanViolation` находятся в
  `crates/swarm-types/src/urban.rs`;
- route planning, route-loop expansion, static AABB judge,
  `route_start_node(...)` и `pose_along_segment(...)` находятся в
  `crates/swarm-sim/src/urban.rs`;
- `RunConfig::urban_state` и dedicated `run_urban_patrol(...)` находятся в
  `crates/swarm-sim/src/runner.rs`;
- `scenarios/urban.patrol.json` и `crates/swarm-scenarios/src/urban.rs`
  дают deterministic one-agent patrol fixture;
- replay уже содержит M65 Urban events:
  `UrbanRoutePlanned`, `UrbanSegmentEntered`, `UrbanSegmentCompleted`,
  `UrbanViolation`, `UrbanPatrolCompleted`;
- metrics/report export уже содержат M65 route/patrol fields.

M66 должен добавить вторую практическую Urban mission:

> Drone patrols the block until it detects a bus through a mocked detector.

Архитектурная граница остается прежней:

- проект моделирует mission-level simulation, planning, replay and metrics;
- M66 не реализует real CV, camera simulation, lidar, realistic bus physics,
  SLAM, PX4/SITL export, hardware readiness или visual UI;
- bus detector должен быть явно mocked и deterministic under seed control.

Обязательные протоколы прочитаны для планирования:

- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`.

No Notion task or GitLab MR was requested. `notion_policy` is `optional`, so no
external Notion/GitLab reads are required. Remote SSH/HTTP access is not needed
and must not be used for this milestone.

# Investigation context

`INVESTIGATION.md` отсутствует.

`PLAN.md` отсутствовал в workspace before this planning round, so this file is
created as a fresh M66 plan.

Local context inspected:

- `docs_raw/DRONE_C.21.md` defines M66 as Urban Search v1 after M65 Urban
  Patrol v0.
- `README.md` and `docs/STATUS.md` currently mark M65 complete and explicitly
  say bus detection remains future work.
- `docs/SCENARIO_DSL.md` currently documents only `urban-patrol` and
  `run_config.urban_state`.
- `docs/REPLAY.md` currently documents M65 Urban replay events only.
- `crates/swarm-types/src/urban.rs` has no bus or detector types yet.
- `crates/swarm-sim/src/runner.rs` currently routes any `urban_state` run to
  Urban Patrol. M66 must avoid accidentally treating Urban Search as Patrol.
- `crates/swarm-replay/src/event_log.rs`,
  `crates/swarm-replay/src/replay.rs`, and
  `crates/swarm-examples/src/bin/replay.rs` need additive M66 events and
  summary counters.
- `crates/swarm-metrics/src/metrics.rs` and
  `crates/swarm-sim/src/report_export.rs` need additive M66 metrics/export
  fields with serde defaults.
- `crates/swarm-examples/src/bin/strategy_comparison.rs` and
  `crates/swarm-examples/src/regression_lib.rs` currently support explicit
  `urban-patrol`, but not `urban-search`.

No code/test/build command was required for planning. During implementation,
all quick checks must use `timeout 300`; every `cargo test` and `cargo run`
must use `/home/formi/.local/bin/runlim`, and every `cargo test` must set
`PROPTEST_DISABLE_FAILURE_PERSISTENCE=1`. Long benchmarks, 500/1000-seed
runs, PX4/SITL runs and hardware checks must be documented as future evidence
work, not hidden inside M66 implementation.

# Affected components

- `crates/swarm-types/src/urban.rs`
  - Add `UrbanBusId` newtype.
  - Add `UrbanBus` with:
    - `id`;
    - static `pose` for M66 v1;
    - optional `active_from_tick`;
    - optional `active_until_tick`.
  - Add `UrbanDetectorConfig` with:
    - `detection_range_m`;
    - `detection_probability`;
    - `false_positive_rate`;
    - deterministic `seed`.
  - Add `UrbanSearchState` with:
    - `buses: Vec<UrbanBus>`;
    - `detector: UrbanDetectorConfig`.
  - Add validation helpers for bus ids, finite poses, active windows,
    probabilities in `[0, 1]`, finite non-negative range, and duplicate ids.
  - Keep all additions serde-compatible and additive.

- `crates/swarm-types/src/lib.rs`
  - Export new M66 Urban Search types.

- `crates/swarm-sim/src/runner.rs`
  - Add `RunConfig::urban_search_state: Option<UrbanSearchState>`.
  - Route `urban_search_state.is_some()` to a dedicated Urban Search runner
    before the existing Urban Patrol branch.
  - Keep `urban-patrol` behavior unchanged when `urban_search_state` is absent.
  - Refactor M65 route-following enough to share route progress logic between
    Patrol and Search, rather than duplicating the full tick loop.

- `crates/swarm-sim/src/urban.rs`
  - Add deterministic mock detector helpers:
    - active bus filtering by tick;
    - distance/range check against current agent pose;
    - deterministic Bernoulli draw for detection probability;
    - deterministic Bernoulli draw for false positive rate;
    - no field-of-view and no line-of-sight in M66.
  - Prefer a small local helper struct such as `UrbanDetectionOutcome` over
    wiring detector logic directly into `runner.rs`.
  - Reuse `Pose::distance_to(...)` and existing route progress helpers.

- `crates/swarm-sim/src/dsl.rs`
  - Add mission-specific validation for `mission == "urban-search"`.
  - Require both `run_config.urban_state` and `run_config.urban_search_state`.
  - Reuse M65 map/route/start-node validation.
  - Validate detector config and bus list.
  - Keep `urban-patrol` validation separate and unchanged.

- `crates/swarm-replay/src/event_log.rs`
  - Add additive M66 events:
    - `BusObserved`;
    - `BusDetected`;
    - `BusFalsePositive`;
    - `UrbanSearchCompleted`.
  - Suggested event fields:
    - `agent_id`;
    - `tick`;
    - `bus_id: Option<UrbanBusId>` where applicable;
    - `pose`;
    - `distance_m` for real observations/detections;
    - `detector_seed`;
    - `distance_travelled_m` for completion;
    - `detected: bool` and `reason` for `UrbanSearchCompleted`.
  - Use stable snake_case serde names.

- `crates/swarm-replay/src/replay.rs`
  - Extend `ReplaySummary` with:
    - `bus_observations`;
    - `bus_detections`;
    - `bus_false_positives`;
    - `urban_search_completions`;
    - `urban_search_time_to_detection_ticks: Vec<u64>`;
    - `urban_search_no_detection_count`.
  - Ensure replay ignores unknown-to-state M66 events but summarizes them.

- `crates/swarm-examples/src/bin/replay.rs`
  - Print M66 Urban Search counters under `--summary`.

- `crates/swarm-metrics/src/metrics.rs`
  - Add defaulted per-run fields:
    - `bus_detected: bool`;
    - `time_to_detect_bus: Option<u64>`;
    - `false_positive_count: u64`;
    - `distance_before_detection: f64`;
    - `search_success_without_violation: bool`.
  - Add aggregate fields:
    - `bus_detection_rate`;
    - `avg_time_to_detect_bus`;
    - `avg_false_positive_count`;
    - `avg_distance_before_detection`;
    - `search_success_without_violation_rate`.

- `crates/swarm-sim/src/report_export.rs`
  - Add M66 fields to JSON export rows.
  - Add M66 fields to CSV headers/rows.
  - Add M66 fields to Markdown comparison and focused `urban-search` report.
  - Update comparison/equality helper tests for new fields.

- `crates/swarm-sim/src/benchmark.rs`
  - If common Markdown table includes Urban columns, add compact M66 columns or
    route M66 metrics only through focused report to avoid over-wide tables.
  - Preserve existing report identity and benchmark manifest behavior.

- `crates/swarm-scenarios/src/urban.rs`
  - Add Urban Search profiles, for example:
    - `search-static-bus`;
    - `search-out-of-range`;
    - `search-false-positive`.
  - Add `build_urban_search_scenario(...)`.
  - Reuse the M65 small-block map and route fixture where possible.

- `crates/swarm-scenarios/src/lib.rs`
  - Export new Urban Search builder/profile types.

- `scenarios/urban.search.json`
  - Add a portable deterministic happy-path fixture:
    - one scout starts at `start_node`;
    - route is the small block loop;
    - one static bus is inside detection range on the route;
    - detector probability is deterministic and should detect before timeout.
  - Add negative file fixture only if scenario catalog tests need it; otherwise
    use inline negative fixtures in unit tests.

- `crates/swarm-examples/src/bin/strategy_comparison.rs`
  - Add explicit `--mission urban-search`.
  - Keep `urban-search` out of `--mission all` unless docs and benchmark scope
    intentionally change in the same milestone. Recommended: keep it explicit
    until M69 benchmark refresh.

- `crates/swarm-examples/src/regression_lib.rs`
  - Add `urban-search` as an explicit regression mission builder.

- `crates/swarm-sim/tests/scenario_catalog.rs`
  - Add catalog test for `scenarios/urban.search.json`.

- `crates/swarm-examples/tests/replay_cli.rs`
  - Add replay CLI summary fixture for M66 events.

- `crates/swarm-examples/tests/sitl_docs.rs`
  - Update docs smoke tests for M66 wording and limitations.

- Documentation:
  - `README.md`;
  - `docs/STATUS.md`;
  - `docs/SCENARIO_DSL.md`;
  - `docs/EXTENSION_GUIDE.md`;
  - `docs/REPLAY.md`;
  - `docs/REGRESSION.md`;
  - `docs/BENCHMARK_RESULTS.md`.

# Implementation steps

1. Add M66 data model.
   Files:
   - `crates/swarm-types/src/urban.rs`;
   - `crates/swarm-types/src/lib.rs`.

   Implement additive bus/search types:

   - `UrbanBusId(String)` with the same newtype conventions as existing
     `UrbanNodeId`/`UrbanEdgeId`.
   - `UrbanBus { id, pose, active_from_tick, active_until_tick }`.
   - `UrbanDetectorConfig { detection_range_m, detection_probability,
     false_positive_rate, seed }`.
   - `UrbanSearchState { buses, detector }`.

   M66 should implement static bus `pose` first. Graph-node/edge-position bus
   placement can be a later extension unless it stays tiny and does not
   complicate validation.

2. Add validation for buses and detector config.
   Files:
   - `crates/swarm-types/src/urban.rs`;
   - `crates/swarm-sim/src/dsl.rs`.

   Rules:

   - bus ids are unique;
   - bus pose coordinates are finite;
   - `active_from_tick <= active_until_tick` when both exist;
   - detection range is finite and `>= 0`;
   - probabilities are finite and in `[0, 1]`;
   - `urban-search` requires at least one bus for normal scenarios, except
     explicit negative fixtures may use an out-of-range bus rather than no bus.

3. Extend `RunConfig` and mission dispatch.
   File:
   - `crates/swarm-sim/src/runner.rs`.

   Add:

   - `pub urban_search_state: Option<UrbanSearchState>`;
   - dispatch order:
     `urban_search_state.is_some()` -> `run_urban_search(...)`,
     otherwise existing `urban_state.is_some()` -> `run_urban_patrol(...)`.

   This prevents `urban-search` scenarios from accidentally running through the
   M65 Patrol completion path.

4. Refactor route-following into a reusable helper.
   Files:
   - `crates/swarm-sim/src/runner.rs`;
   - optionally `crates/swarm-sim/src/urban.rs`.

   Current M65 route progress lives inside `run_urban_patrol(...)`. M66 should
   extract just enough shared state to avoid copying the tick loop:

   - selected agent and validated start node;
   - planned route;
   - current segment index;
   - distance on segment;
   - total distance travelled;
   - segment entered/completed event emission;
   - pose at current route progress.

   The helper should support two policies:

   - Patrol policy: complete after one loop.
   - Search policy: keep patrolling the same loop until detection, violation or
     timeout. When the last segment completes and no bus was detected, reset to
     segment 0 and continue counting total distance.

5. Implement deterministic mock detector.
   File:
   - `crates/swarm-sim/src/urban.rs`.

   Suggested behavior:

   - Every tick after route pose is updated, evaluate active buses.
   - A bus is observable when `agent_pose.distance_to(bus.pose) <=
     detection_range_m`.
   - Emit `BusObserved` for every active bus in range before the probability
     draw.
   - Emit `BusDetected` and stop the mission when an in-range bus passes the
     deterministic detection probability draw.
   - Emit `BusFalsePositive` when no real detection happened and the false
     positive draw succeeds.
   - False positives do not count as mission success in M66; they increment
     metrics and replay summary only.
   - Use a deterministic RNG derived from detector seed, scenario seed and
     tick/order. Prefer a single helper so unit tests can lock expected
     outcomes without depending on incidental call order.

   Do not implement field-of-view, line-of-sight, occlusion, camera images,
   lidar/raycast, or physical bus movement in M66.

6. Implement Urban Search runner.
   File:
   - `crates/swarm-sim/src/runner.rs`.

   Success predicate:

   ```text
   bus_detected
   && urban_violation_count == 0
   && time_to_detect_bus <= max_ticks
   && no invalid start contract
   ```

   Timeout predicate:

   ```text
   max_ticks reached without BusDetected
   ```

   Metrics:

   - `success = search_success_without_violation`;
   - `bus_detected`;
   - `time_to_detect_bus`;
   - `false_positive_count`;
   - `distance_before_detection`;
   - M64/M65 route fields still populated:
     `urban_route_planned`, `urban_route_length_m`,
     `urban_violation_count`, `urban_distance_travelled_m`.

   Replay:

   - reuse M65 route events;
   - add bus/detector events;
   - emit `UrbanSearchCompleted` on both detected and timeout terminal paths
     with `detected`/`reason` fields, so replay summary can count no-detection
     outcomes deterministically.

7. Extend replay schema and summaries.
   Files:
   - `crates/swarm-replay/src/event_log.rs`;
   - `crates/swarm-replay/src/replay.rs`;
   - `crates/swarm-examples/src/bin/replay.rs`;
   - `crates/swarm-examples/tests/replay_cli.rs`.

   Add event serde roundtrip tests and summary tests. Existing old logs should
   remain readable; M66 should be additive.

8. Extend metrics and reports.
   Files:
   - `crates/swarm-metrics/src/metrics.rs`;
   - `crates/swarm-sim/src/report_export.rs`;
   - `crates/swarm-sim/src/benchmark.rs` if common Markdown table changes.

   All new fields must use `#[serde(default)]` where applicable to preserve old
   metrics/report deserialization. Update JSON/CSV/Markdown tests and compare
   helpers.

9. Add scenario builders and scenario file.
   Files:
   - `crates/swarm-scenarios/src/urban.rs`;
   - `crates/swarm-scenarios/src/lib.rs`;
   - `scenarios/urban.search.json`;
   - `crates/swarm-sim/tests/scenario_catalog.rs`.

   Start with three deterministic fixtures:

   - happy path: static bus near the route, `detection_probability = 1.0`,
     `false_positive_rate = 0.0`;
   - timeout/no-detection: bus outside range, `detection_probability = 1.0`,
     `false_positive_rate = 0.0`;
   - false-positive control: bus outside range,
     `detection_probability = 0.0`, deterministic seed and
     `false_positive_rate = 1.0` or a locked seeded probability case.

10. Wire CLI/regression mission support.
    Files:
    - `crates/swarm-examples/src/bin/strategy_comparison.rs`;
    - `crates/swarm-examples/src/regression_lib.rs`;
    - `docs/REGRESSION.md`.

    Add explicit `--mission urban-search`. Keep it out of `--mission all` until
    M69 unless implementation intentionally refreshes benchmark scope and docs.

11. Update docs and status.
    Files:
    - `README.md`;
    - `docs/STATUS.md`;
    - `docs/SCENARIO_DSL.md`;
    - `docs/EXTENSION_GUIDE.md`;
    - `docs/REPLAY.md`;
    - `docs/REGRESSION.md`;
    - `docs/BENCHMARK_RESULTS.md`.

    Required wording:

    - detector is mocked;
    - no real object recognition claim;
    - no camera simulation;
    - no lidar/raycast;
    - no line-of-sight realism unless actually implemented;
    - no realistic bus physics;
    - no PX4/SITL or hardware readiness claim;
    - `urban-search` is explicit smoke/regression mission, not a refreshed
      publication benchmark.

12. Keep M66 evidence bounded.
    Files:
    - `docs/BENCHMARK_RESULTS.md`;
    - `docs/REGRESSION.md`;
    - possibly result docs only if a quick smoke artifact is intentionally
      produced.

    M66 should run quick deterministic tests/smoke only. Do not run 500/1000
    seed benchmarks. If implementation needs a long run, document it as future
    M69 evidence work.

# Testing strategy

## Tests that need no refactoring

These should be implemented together with the main functional changes:

- `crates/swarm-types/src/urban.rs`
  - bus entity serde roundtrip;
  - duplicate bus id validation;
  - invalid bus pose validation;
  - invalid active window validation;
  - detector probability/range validation.

- `crates/swarm-sim/src/urban.rs`
  - detector in-range success with `detection_probability = 1.0`;
  - detector out-of-range no-detection with `detection_probability = 1.0`;
  - detector `detection_probability = 0.0` never detects real bus;
  - controlled false positive with deterministic seed/rate;
  - inactive bus is ignored before `active_from_tick` and after
    `active_until_tick`.

- `crates/swarm-sim/src/dsl.rs`
  - `urban-search` rejects missing `urban_state`;
  - `urban-search` rejects missing `urban_search_state`;
  - `urban-search` rejects invalid detector config;
  - `urban-search` rejects invalid bus config;
  - `urban-search` accepts valid small-block fixture.

- `crates/swarm-sim/src/runner.rs`
  - happy-path Urban Search detects bus and stops before timeout;
  - out-of-range bus times out with `bus_detected = false`;
  - false-positive path increments `false_positive_count` but does not set
    `bus_detected` or `success`;
  - judge violation prevents search success;
  - invalid start contract still fails before detector success;
  - route loop repeats until detection/timeout when bus is not found on first
    loop.

- `crates/swarm-replay/src/event_log.rs`
  - bus/search event serde roundtrip.

- `crates/swarm-replay/src/replay.rs`
  - replay summary counts `BusObserved`, `BusDetected`,
    `BusFalsePositive`, `UrbanSearchCompleted`, detection ticks, and
    no-detection completions.

- `crates/swarm-metrics/src/metrics.rs`
  - aggregate search metrics from multiple runs:
    detection rate, average detection tick, false positive count, distance
    before detection, success-without-violation rate.

- `crates/swarm-sim/src/report_export.rs`
  - JSON/CSV/Markdown contain M66 fields;
  - compare report detects mismatched M66 fields.

- `crates/swarm-scenarios/src/urban.rs`
  - search happy-path fixture completes;
  - out-of-range fixture times out;
  - false-positive fixture is deterministic.

- `crates/swarm-sim/tests/scenario_catalog.rs`
  - `scenarios/urban.search.json` loads, validates and runs with expected
    deterministic search outcome.

- `crates/swarm-examples/tests/replay_cli.rs`
  - `replay --summary` prints bus/search counters.

- `crates/swarm-examples/tests/sitl_docs.rs`
  - docs contain M66 mocked detector boundaries and do not claim real CV/lidar.

Recommended quick verification commands for implementation:

```bash
timeout 300 cargo fmt --all
timeout 300 /home/formi/.local/bin/runlim cargo check -p swarm-types -p swarm-sim -p swarm-scenarios -p swarm-replay -p swarm-examples
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-types urban
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay urban
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-scenarios urban
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim report_export
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test replay_cli
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs
timeout 300 cargo clippy --all-targets -- -D warnings
```

Optional quick smoke, only if it stays well under the 5 minute limit:

```bash
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo run -p swarm-examples --bin strategy_comparison -- --smoke --mission urban-search --output-dir target/m66_urban_search_smoke
```

Do not run 500/1000-seed benchmarks in M66.

## Tests that need light refactoring

- Shared Urban route runtime fixture/helper for Patrol and Search tests, so
  route progress assertions do not duplicate map construction.
- `assert_urban_search_outcome(...)` helper for success/timeout/false-positive
  metrics.
- Deterministic detector RNG helper exposed only inside `swarm-sim` tests if
  needed to avoid brittle random-call-order assertions.
- Report assertion helper for grouped Urban metrics, because report columns are
  getting wider after M65/M66.
- Replay summary assertion helper for Urban event counters.

## Tests that need heavy refactoring

These are out of M66 unless implementation unexpectedly grows:

- Dynamic bus route/schedule property tests.
- Line-of-sight or occlusion geometry tests.
- Field-of-view tests.
- Multi-agent search partitioning tests.
- Large replay performance tests for long Urban Search traces.
- Statistical tests for detector probability distributions across many seeds.

# Risks and tradeoffs

- **Runner branching risk:** Current runner uses `urban_state.is_some()` to
  enter Patrol. M66 must add `urban_search_state` and dispatch it first;
  otherwise Search can accidentally complete as Patrol without detection.

- **Route loop semantics:** Search should keep patrolling until detection or
  timeout. This differs from M65 Patrol, which completes after one loop. Shared
  route-progress refactoring must keep those terminal policies separate.

- **Deterministic RNG contract:** If detector RNG depends on incidental event
  order, tests will be brittle. Use a clearly documented seed/tick/order helper.

- **False-positive semantics:** False positives should be observable in replay
  and metrics, but should not count as target detection success in M66. If this
  rule changes later, metrics names and docs must change.

- **Report width:** Adding M66 fields to common Markdown tables may make
  reports too wide. Focused `urban-search` report may be preferable, with JSON
  and CSV carrying all fields.

- **Backward compatibility:** Replay and metrics additions should be additive
  with serde defaults. Old logs/reports should remain readable.

- **Scenario DSL growth:** `RunConfig` now risks accumulating many optional
  mission states. M66 can accept this for momentum, but M67/M68 may need a
  cleaner mission-state enum if optional fields become confusing.

- **No realism overclaim:** The detector is mocked and distance-based. Docs must
  avoid implying real object recognition, line-of-sight, camera simulation or
  lidar.

- **Performance:** Search can loop until timeout and emit more replay events
  than Patrol. Keep default fixtures short and add tests for bounded event
  counts.

# Open questions

1. Should M66 support only `pose` buses, or also `node_id`/`edge_id +
   distance_m` placement immediately? Recommendation: implement `pose` first;
   add graph-relative placement later unless it stays very small.

2. Should `UrbanSearchCompleted` be emitted for timeout/no-detection, or only
   for successful detection? Recommendation: emit it for both, with
   `detected: bool` and `reason`, so replay summary can count no-detection
   terminal outcomes.

3. Should `urban-search` be included in `--mission all` immediately?
   Recommendation: no. Keep it explicit until M69 benchmark refresh decides the
   benchmark scope.

4. Should false positives ever stop the mission? Recommendation: no for M66;
   they are reported but do not satisfy the success predicate.

5. Should dynamic bus routes be included in M66? Recommendation: no unless the
   static bus implementation is already complete and the dynamic route remains
   trivial. Dynamic schedule/property tests belong to later work.

6. Should line-of-sight be approximated with AABB obstacles in M66?
   Recommendation: no. Distance-only mocked detector is enough for perception
   decision logic; line-of-sight is a separate geometry milestone.
