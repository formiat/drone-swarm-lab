# Extension Guide

This guide documents the current extension points for adding missions,
allocation strategies, metrics, and schema fields inside this workspace.

M61 makes the extension path explicit, but it does not make the project a
published SDK. These APIs are stable-ish for in-repository work and research
extensions, not semver-stable public API. Internal modules, SITL supervisor
state machines, MAVLink details, and test-only helpers may still change.

## Scope And Non-Goals

Use this guide when you want to add:

- a new mission type;
- a road-graph Urban mission extension built on the M64 foundation;
- a new allocator or benchmark strategy;
- a new metric and report field;
- a replay or report schema field tied to one of those changes.

Do not treat this as:

- a semver-stable public API contract;
- a crate publishing checklist;
- a production flight-control or hardware-readiness checklist;
- permission to add a real mission without regression and support-matrix work.

## Crate Boundaries

stable-ish extension points:

| Crate / surface | Use |
|---|---|
| `swarm-types` | Shared `TaskKind`, `Task`, `RunState`, `MissionAdapter`, `AdapterRegistry`, ids, poses, roles, and agent/task data. |
| `swarm-alloc` | `Allocator`, optional allocation extension methods, `Strategy`, and `StrategyRegistry`. |
| `swarm-metrics` | `RunMetrics` and `AggregateMetrics` fields used by reports, regression thresholds, and benchmark summaries. |
| Scenario DSL | JSON scenario suites with explicit `schema_version` documented in `docs/SCENARIO_DSL.md`. |
| Replay/report schemas | Simulation replay `0.2`, SITL event log `sitl_event_log.v1`, SITL reports `sitl_run_report.v1` / `sitl_multi_agent_run_report.v1`. |

Workspace-internal but usable with care:

| Crate / surface | Use |
|---|---|
| `swarm-scenarios` | Built-in scenario builders and fixtures. Keep new builders deterministic. |
| `swarm-sim` | `ScenarioRunner`, DSL loader/validator, benchmark/report export helpers, regression harness. |
| `swarm-replay` | Event log builder and replay summary helpers for simulation events. |

Internal or experimental:

| Surface | Reason |
|---|---|
| `swarm-examples` binaries | CLI behavior is user-facing, but most implementation modules are binary support code. |
| SITL supervisor internals | Controller state machines and fake controllers are not external extension points. |
| MAVLink transport internals | Experimental PX4/SIH plumbing, not a general ground-control API. |
| Test-only fixtures | They validate extension contracts but are not supported missions or strategies. |

## Add A Mission

1. Choose or add a `TaskKind` in `crates/swarm-types/src/task.rs`.
   Existing mission kinds include `CoverageCell`, `SarScan`,
   `SarConfirmationScan`, `InspectionEdge`, `MappingZone`,
   `RelayPlacement`, and `Waypoint`.
2. Implement `MissionAdapter` from `crates/swarm-types/src/mission.rs`.
   Define:
   - `task_kind`;
   - `route_cost`;
   - `is_completed`;
   - `score`.
3. Wire the adapter in `AdapterRegistry` in `crates/swarm-types/src/adapter.rs`.
   Avoid replacing existing adapter semantics unless the change is intentional
   and covered by tests.
4. Add or update a scenario builder in `crates/swarm-scenarios/src/...`.
   The builder should be deterministic for a given seed and should not depend
   on local filesystem state.
5. Add scenario JSON/DSL fixtures under `scenarios/...`.
   Scenario suites must include `"schema_version": "0.1"` and must pass the
   mission-specific validation in `crates/swarm-sim/src/dsl.rs`.
6. Define completion semantics.
   Prefer expressing the runtime state through `RunState` when the semantics
   are generic. Add mission-specific runtime state only when `RunState` cannot
   represent the completion condition.
7. Add metrics only when the mission needs mission-specific observability.
   Use the metric workflow below.
8. Add replay events only when existing events cannot explain the mission.
   Prefer generic task assignment/completion events when they are enough.
9. Add support-matrix and regression coverage.
   If a strategy/mission pair is intentionally unsupported, record an explicit
   unsupported reason instead of letting the run fail ambiguously.

### Urban Mission Path

M64 adds an Urban foundation, M65 adds the first Urban Patrol simulation, M66
adds Urban Search v1 with a deterministic mocked bus detector, M67 adds
diagnostic replay/analysis tooling, M75 adds scheduled moving bus targets plus
perimeter patrol semantics, and M76 adds deterministic generated Urban testbed
fixtures. Future Urban work should reuse this road-graph path instead of
starting with arbitrary polygons:

- shared types live in `crates/swarm-types/src/urban.rs`;
- deterministic Dijkstra planning, experimental M68 corridor-aware planning,
  and the initial judge live in
  `crates/swarm-sim/src/urban.rs`;
- `run_config.urban_state` carries the road graph, route loop, optional
  `start_node`, planner choice, optional temporary obstacles, and optional
  `perimeter_patrol` in Scenario DSL;
- supported Urban planner values are `"dijkstra"` and the experimental
  `"corridor-aware"` planner;
- M65 validates `start_node` against `route_loop.nodes[0]` and requires the
  selected alive agent pose to start within `0.01m` of that node;
- `run_config.urban_search_state` carries M66/M75 bus targets and mocked
  detector settings (`detection_range_m`, `detection_probability`,
  `false_positive_rate`, `seed`). Static buses use `pose` and optional active
  windows; moving buses add `route.stops[]` over Urban map node ids plus
  `speed_m_per_tick`;
- `scenarios/urban.patrol.json` is the portable fixture for catalog tests;
- `scenarios/urban.search.json` is the portable fixture for mocked bus-search
  catalog and regression tests;
- `scenarios/urban.multi-agent.json` is the portable two-agent analysis
  fixture for replay route traces, judge reports, and separation metrics; it is
  intended to run through scenario-suite mode with replay enabled;
- `scenarios/urban.corridor-delta.json` is the portable M68 before/after
  fixture for comparing Dijkstra against the experimental corridor-aware
  planner;
- standard Urban builders also expose M75 `search-moving-bus` and
  `perimeter-square` profiles for deterministic moving-target/perimeter
  simulation tests;
- generated Urban suites should use `SyntheticUrbanGenerator` from
  `crates/swarm-scenarios/src/generated.rs` when the fixture needs systematic
  variation by seed, grid size, blocked edges, bus mode, failures, or comms
  overlays. Generated suites must carry `generator_manifest` so provenance is
  visible in Scenario DSL artifacts;
- `scenarios/urban.generated.tiny.json` is the portable M76 checked-in
  generated fixture. Regenerate it through `generate_scenario_suite` rather
  than hand-editing generated values;
- metrics report route planning and patrol execution fields:
  `urban_route_length_m`, `urban_route_planned`,
  `urban_violation_count`, `urban_route_completed`,
  `urban_patrol_completed`, `urban_time_to_complete_loop`,
  `urban_distance_travelled_m`, `urban_route_efficiency`, and
  `urban_replan_count`;
- M66 search metrics add `bus_detected`, `time_to_detect_bus`,
  `false_positive_count`, `distance_before_detection`, and
  `search_success_without_violation`, plus aggregate report fields with the
  same semantics;
- M75 perimeter metrics add `perimeter_completion_rate`,
  `perimeter_length_m`, `time_to_complete_perimeter`, and
  `perimeter_violations`, plus aggregate/report export fields;
- M67 diagnostic metrics add `urban_min_agent_separation_m`,
  `urban_separation_violation_count`, and `urban_route_conflict_count`, plus
  aggregate report fields. These are measured from replay traces and are not
  route-deconfliction or avoidance guarantees;
- M68 route-risk metrics add `urban_route_risk_score` and
  `avg_urban_route_risk_score`. They are route-planning risk proxies based on
  corridor width and AABB obstacle clearance, not physical collision
  probabilities;
- M70 route export adapters should use `crates/swarm-sim/src/urban/route_export.rs`
  as the boundary from Urban planned routes to SITL waypoint plans. Preserve
  route identity fields (`edge_id`, `from_node_id`, `to_node_id`,
  `segment_index`, `point_index_on_segment`), explicit altitude, route length,
  segment count, waypoint count, and `geo_origin` metadata. Keep this as a
  dry-run/SITL-compatible export boundary; do not add hardware, perception, or
  obstacle-avoidance claims in route export adapters;
- replay logs expose `UrbanRoutePlanned`, `UrbanSegmentEntered`,
  `UrbanSegmentCompleted`, `UrbanViolation`, `UrbanPatrolCompleted`,
  `BusObserved`, `BusDetected`, `BusFalsePositive`, and
  `UrbanSearchCompleted`. M75 moving buses reuse the bus event pose field for
  the sampled pose at the event tick; perimeter patrol reuses route-progress
  events rather than adding a new replay event family.
- M67 benchmark packs with Urban replay logs can emit
  `urban_analysis/*.route-trace.json`, `*.route-trace.csv`,
  `*.judge-report.json`, `*.judge-report.csv`, and
  `urban_analysis/manifest.json`;
- the replay CLI supports `--timeline`, `--agent <agent-id>`, and
  `--category urban` for deterministic event inspection.

This is intentionally a mission-level substrate. The M66 detector is mocked and
distance/probability based. The M67 two-agent fixture is diagnostic only. The
M68 corridor-aware planner is a route scoring extension, not physical
avoidance. Do not add real lidar/raycast, dynamic obstacles, multi-agent route
deconfliction, PX4/SITL export, hardware claims, visual UI, or arbitrary polygon
dependencies as part of this path. M76 generated suites are deterministic input
generation only; they are not benchmark, PX4, hardware, or physics evidence.
Those belong to later milestones with their own tests and docs.

## Add A Strategy

1. Implement `Allocator` from `crates/swarm-alloc/src/allocator.rs`.
   The minimal required method is `allocate`.
2. Override optional allocator methods only when needed:
   - `allocate_with_connectivity` for network-aware behavior;
   - `allocate_with_adapter` or `allocate_with_registry` for mission-semantic
     scoring;
   - `allocation_metrics` for strategy-specific telemetry;
   - `is_distributed` for distributed strategies.
3. Implement `Strategy` from `crates/swarm-alloc/src/strategy.rs`.
   `name()` is part of CLI/report identity, so keep it stable and
   lowercase-kebab-case.
4. Register the strategy in `StrategyRegistry` if it should be part of the
   default benchmark matrix.
5. Update CLI/benchmark selection logic where the strategy is user-selectable.
6. Document support-matrix behavior. For unsupported mission/strategy pairs,
   prefer an explicit unsupported reason over silent low success rates.
7. Add benchmark/regression coverage that is small enough for CI and does not
   require local machine-specific files.

## Add A Metric

1. Add the per-run field to `RunMetrics` in
   `crates/swarm-metrics/src/metrics.rs`.
2. Use `#[serde(default)]` for additive fields so older JSON remains readable.
3. Add aggregate fields to `AggregateMetrics` when the metric must be shown in
   benchmark summaries, regression thresholds, or Markdown tables.
4. Populate the field in `crates/swarm-sim/src/runner.rs`,
   `crates/swarm-sim/src/benchmark.rs`, or SITL report code as appropriate.
5. Export the field through JSON/CSV/Markdown report helpers when it is
   user-facing.
6. Update docs and tests:
   - report/export header assertions;
   - regression threshold extraction if the metric can be gated;
   - README/status text if the metric is part of a milestone claim.

Do not add a real metric only to satisfy documentation. If the metric is not
used by reports, regression, or analysis, keep it out.

## Schema Version Policy

Scenario DSL:

- Current schema version: `0.1`.
- Scenario suites must include `"schema_version": "0.1"`.
- Legacy scenario files without the field default to `0.1`.
- Bump the scenario schema only for incompatible structure or validation
  changes. Add compatibility tests before making a bump.

Simulation replay:

- Current schema version: `0.2`.
- Event logs without `schema_version` default to `0.2`.
- Additive event variants or optional fields can stay on `0.2` if old logs
  still deserialize and summaries remain meaningful.

SITL schemas:

- SITL event log: `sitl_event_log.v1`.
- Single-agent SITL report: `sitl_run_report.v1`.
- Multi-agent SITL report: `sitl_multi_agent_run_report.v1`.
- Multi-agent SITL config: `multi_sitl.v1`.
- Multi-agent SITL manifest: `multi_sitl_manifest.v1`.

M73 degraded-supervisor fields are additive within these same schema versions:
`run-report.json.degraded`, optional per-agent `failure_mode` /
`tasks_abandoned`, and `supervisor_*` degraded replay events. Extensions should
preserve `#[serde(default)]` compatibility and must not turn fake-tested failure
modes into hardware claims without separate evidence.

Report/export schemas:

- JSON can add optional/defaulted fields when old files still deserialize.
- CSV/Markdown changes are user-visible; update docs and tests when columns
  are added, renamed, or removed.
- Do not reuse a field with changed semantics. Add a new field or document a
  versioned semantic break.

## Regression, Replay, And Report Checklist

Before calling an extension complete:

- a small scenario fixture exists, or the absence is explicitly justified;
- the mission/strategy pair is in the support matrix or has an unsupported
  reason;
- replay events explain any new runtime state transitions;
- report JSON/CSV/Markdown includes new user-facing metrics;
- docs mention the schema and support-matrix impact;
- automated tests cover happy path, negative path, and at least one edge case
  where practical.

## Minimal Test-Only Extension Path

M61 validates this guide with test-only fixtures instead of adding a real
mission:

- a custom `MissionAdapter` fixture exercises `TaskKind`, `RunState`, route
  cost, completion, and scoring without registering a production mission;
- a custom `Strategy` fixture confirms that `StrategyRegistry` accepts an
  extension strategy without mutating the default registry;
- a tiny in-memory `ScenarioRunner::run_with_log` path confirms that a
  kind-tagged mission fixture can run with replay logging enabled;
- docs smoke tests keep this guide linked from README/status and check the
  schema-version terms above.

These fixtures are contract tests, not supported user-facing missions or
strategies.
