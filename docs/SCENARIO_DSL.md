# Scenario DSL

The Scenario DSL is a JSON-based format for describing reproducible simulation scenarios. All scenarios in `scenarios/` use this format.

## Schema Version

Current schema version: `0.1`

All scenario suites must include `"schema_version": "0.1"`. Legacy files without this field default to `"0.1"`.

For mission-extension work, follow
[`docs/EXTENSION_GUIDE.md`](EXTENSION_GUIDE.md). Additive mission fixtures can
usually stay on schema `0.1`; incompatible structure or validation changes need
an explicit schema policy update and compatibility tests.

## ScenarioSuite Format

```json
{
  "name": "My Suite",
  "schema_version": "0.1",
  "scenarios": [
    {
      "mission": "coverage",
      "profile": "ideal-no-failures",
      "scenario": {
        "name": "coverage_ideal",
        "seed": 0,
        "agents": [...],
        "tasks": [...]
      },
      "run_config": {
        "max_ticks": 50,
        "timeout_ticks": 5,
        "packet_loss_rate": 0.0,
        "latency_ticks": 0,
        "failures": [],
        "dynamic_tasks": [],
        "partition_events": [],
        "safety_config": null,
        "inspection_state": null,
        "grid_state": null,
        "enable_movement": false,
        "enable_cbba": false,
        "gossip_interval_ticks": 3,
        "latency_per_hop": 0,
        "tick_duration_ms": 100
      }
    }
  ]
}
```

## Scenario Geo Origin

M70 adds optional `scenario.geo_origin`:

```json
"geo_origin": {
  "lat_deg": 47.397742,
  "lon_deg": 8.545594,
  "alt_m": 0.0
}
```

This origin is the WGS84 home point used by SITL/PX4 waypoint conversion when a
local simulation route is exported to global mission coordinates. If omitted,
the SITL upload path keeps the existing PX4/SIH default origin. Validation
requires finite latitude, longitude, and altitude values; latitude must be in
`[-90, 90]` and longitude in `[-180, 180]`.

## Preflight Safety

M71 adds static preflight validation for scenario entries. The DSL validator
converts error-severity preflight violations into validation errors using their
stable rule ids. `run_config.safety_config` can now carry optional
`max_altitude_m`, `min_altitude_m`, `max_route_length_m`, and
`max_duration_ticks` fields in addition to geofence, no-fly zones, and
separation constraints.

See [`docs/PREFLIGHT_SAFETY.md`](PREFLIGHT_SAFETY.md) for the full rule table,
including `geofence.waypoint_outside`, `nofly.waypoint_inside`,
`altitude.above_max`, `ownership.duplicate_task_id`, `urban.blocked_edge`, and
`semantics.unsupported_strategy_pair`.

## Required Fields

### Suite level

- `name` — non-empty string
- `schema_version` — must be `"0.1"`
- `scenarios` — non-empty array of entries

### Entry level

- `mission` — non-empty string (e.g., `coverage`, `sar`, `inspection`)
- `profile` — non-empty string (e.g., `ideal`, `standard`)
- `scenario.name` — non-empty string
- `scenario.agents` — non-empty array
- `scenario.tasks` — non-empty array
- `run_config.max_ticks` — must be > 0

## Mission-Specific Constraints

| Mission | Required Fields | Validation Rule |
|---|---|---|
| `sar` | `run_config.grid_state` | Must have `grid_state` with non-empty grid; tasks must have `grid_cell` |
| `inspection` | `run_config.enable_movement` | Must be `true`; tasks must have `edge_id` |
| `cbba-stress` | `run_config.enable_cbba` | Must be `true`; `gossip_interval_ticks <= 5` |
| `sitl` | tasks with `pose` | At least one task must have a `pose` |
| `urban-patrol` | `run_config.urban_state` | Must include `UrbanMap`, route loop, valid Urban planner, valid node/edge refs, and waypoint placeholder tasks; M65/M68 runner follows the planned route in order |
| `urban-search` | `run_config.urban_state`, `run_config.urban_search_state` | Reuses the Urban road graph and start contract, then validates bus targets and deterministic mocked detector config; M66 runner stops on real bus detection or times out |
| `safety` | `run_config.safety_config` | Must have `safety_config` with geofence or no-fly zones |

## Urban Patrol

M64 added `urban-patrol` as a foundation mission fixture. M65 makes it an
executable one-agent patrol simulation. The DSL uses
`run_config.urban_state` with:

- `map.nodes[]` — road graph intersections with `id` and `pose`.
- `map.edges[]` — directed road/corridor segments with `id`, `from`, `to`,
  `cost`, `length_m`, optional `corridor_width_m`, and `blocked`.
- `map.static_obstacles[]` — AABB-only static obstacles such as buildings or
  no-fly rectangles.
- `route_loop.nodes[]` — ordered graph node ids expanded through deterministic
  Dijkstra shortest paths.
- `start_node` — optional but validated when present; in M65 it must exist in
  the map and match `route_loop.nodes[0]`.
- `planner` — optional planner selector. Supported values are `"dijkstra"`
  and `"corridor-aware"`. Missing values default to `"dijkstra"`.
- `temporary_obstacles[]` — (M74) optional list of time-gated edge blockages.
  Each entry requires `edge_id` and `appears_at_tick`; `disappears_at_tick`,
  `reason`, and `severity` are optional. An obstacle is active at tick `t` when
  `t >= appears_at_tick` and (`disappears_at_tick` is absent or `t < disappears_at_tick`).
- `blocked_route_policy` — (M74) optional policy applied when a blocked edge is
  detected ahead. Supported values: `"wait"` (default), `"replan"`, `"abort"`.

Example with a temporary obstacle and wait policy:

```json
"urban_state": {
  "map": { "nodes": [...], "edges": [...], "static_obstacles": [] },
  "route_loop": { "nodes": ["n0", "n1", "n2", "n0"] },
  "start_node": "n0",
  "planner": "dijkstra",
  "temporary_obstacles": [
    {
      "edge_id": "e-n1-n2",
      "appears_at_tick": 5,
      "disappears_at_tick": 15,
      "reason": "construction"
    }
  ],
  "blocked_route_policy": "wait"
}
```

The fixture still uses `TaskKind::Waypoint` placeholder tasks for compatibility,
but Urban Patrol completion is now route-based rather than task-assignment
based. Completion means the selected scout traverses every planned route
segment in order before timeout with zero Urban judge violations. Failure means
timeout, a static/execution judge violation, an invalid start contract, or an
unresolvable blockage with `blocked_route_policy: "abort"` or `"replan"` when
no alternate route exists. The selected alive agent must start within `0.01m` of
the validated start node pose.

Urban Patrol itself does not implement lidar/raycast, bus detection, dynamic
obstacles, multi-agent route deconfliction, arbitrary polygons, PX4 execution,
hardware readiness, or a visual UI.

M70 adds a deterministic Urban Route Export dry-run path for `urban-patrol`.
The authoritative source is `run_config.urban_state`: the planned route is
converted into ordered SITL-compatible waypoint items with stable route identity
fields (`edge_id`, `from_node_id`, `to_node_id`, `segment_index`,
`point_index_on_segment`), explicit altitude, route length, segment count,
waypoint count, and `geo_origin`. This is a local waypoint export artifact, not
proof of PX4 execution, hardware readiness, perception, or obstacle avoidance.

## Urban Search

M66 adds `urban-search` as a simulation-only search mission on top of the
Urban Patrol road graph. It uses the same `run_config.urban_state` map,
`route_loop`, `start_node`, and Urban planner constraints, plus
`run_config.urban_search_state`:

- `buses[]` — mocked bus targets with `id`, fallback static `pose`, optional
  `active_from_tick` / `active_until_tick` visibility windows, and optional
  M75 `route`.
- `buses[].route.stops[]` — scheduled moving-bus stops over Urban map nodes.
  Each stop has `node_id` and `arrival_tick`. Arrival ticks must be strictly
  increasing and every node id must exist in `run_config.urban_state.map`.
- `buses[].route.speed_m_per_tick` — finite positive route metadata for moving
  buses. Current sampling uses the scheduled stop ticks; the speed field is
  retained for future route-generation extensions and validation.
- `detector.detection_range_m` — distance threshold for observable buses.
- `detector.detection_probability` — probability in `[0, 1]` for turning an
  in-range observation into a real detection.
- `detector.false_positive_rate` — probability in `[0, 1]` for a false
  positive when no real bus is detected on that tick.
- `detector.seed` — deterministic detector RNG seed.

The selected scout follows the route repeatedly until the first real bus
detection or timeout. For a moving bus, the detector samples the bus pose at the
current tick from the declared route before applying range/probability checks.
`BusObserved`, `BusDetected`, `BusFalsePositive`, and `UrbanSearchCompleted`
replay events make the run inspectable and record the sampled bus pose for real
observations/detections. Search success means a real bus was detected with zero
Urban judge violations and no runtime unsupported reason. False positives are
counted but do not complete the mission.

## Urban Perimeter Patrol

M75 adds optional perimeter patrol semantics under `run_config.urban_state`:

- `perimeter_patrol.polygon[]` — at least three finite local poses. A duplicated
  closing pose is accepted and normalized.
- `perimeter_patrol.spacing_m` — finite positive waypoint spacing.

The helper `perimeter_waypoints(polygon, spacing_m)` produces a deterministic
closed waypoint list in input order. The standard `urban-patrol` profile list
includes `perimeter-square`, which uses the same square block graph as
`patrol-small-block` and reports:

- `perimeter_completion_rate`
- `perimeter_length_m`
- `time_to_complete_perimeter`
- `perimeter_violations`

Perimeter progress reuses existing Urban route replay events
(`UrbanRoutePlanned`, `UrbanSegmentEntered`, `UrbanSegmentCompleted`,
`UrbanPatrolCompleted`) rather than adding a separate replay event family.

This is a mocked detector, not lidar/raycast, computer vision, dynamic
traffic, physical obstacle avoidance, PX4/SITL export, hardware readiness, or
visualization.

## Urban Multi-Agent Analysis Fixture

M67 adds `scenarios/urban.multi-agent.json` as a deterministic two-agent Urban
analysis fixture. It uses the same `urban-patrol` mission and road-graph
contract, but includes two scout agents so replay analysis can measure
inter-agent separation and route conflicts from trace data.
When run through `strategy_comparison --scenario-suite ... --output-dir ... --replay-log ...`,
the fixture produces replay logs and `urban_analysis/` artifacts containing two
agent route traces plus manifest-level separation/conflict measurements.

This fixture is not a new control mode. It does not implement route
deconfliction, collision avoidance, dynamic obstacles, physical simulation,
PX4/SITL export, hardware readiness, or visualization. Its purpose is to keep
the Urban replay/analysis schema portable and testable.

## Urban Corridor Delta

M68 adds `scenarios/urban.corridor-delta.json` as a small algorithmic
before/after fixture. It keeps the same road graph and compares:

- `corridor-delta-dijkstra` with `planner: "dijkstra"`;
- `corridor-delta-corridor-aware` with `planner: "corridor-aware"`.

The corridor-aware planner remains road-graph based. It does not simulate
lidar/raycast or physical collision avoidance. It uses existing
`map.edges[].corridor_width_m` plus static AABB obstacle clearance to penalize
narrow or low-clearance road segments. The expected metric delta is lower
`urban_route_risk_score` for the corridor-aware profile, with a possible
increase in route length and completion time.

## Minimal Example

```json
{
  "name": "Minimal Coverage",
  "schema_version": "0.1",
  "scenarios": [
    {
      "mission": "coverage",
      "profile": "ideal",
      "scenario": {
        "name": "minimal",
        "seed": 0,
        "agents": [
          {"id": "agent-0", "role": "scout", "health": "alive", "pose": {"x": 0, "y": 0}}
        ],
        "tasks": [
          {"id": "task-0", "status": "unassigned", "priority": 1, "pose": {"x": 10, "y": 10}}
        ]
      },
      "run_config": {
        "max_ticks": 50
      }
    }
  ]
}
```

## Validation

```bash
# Validate a scenario suite
cargo run -p swarm-examples --bin strategy_comparison \
  -- --scenario-suite scenarios/coverage.ideal.json

# Validate via Rust API
use swarm_sim::{validate_scenario_suite, load_scenario_suite};

let suite = load_scenario_suite("scenarios/my.json")?;
let errors = validate_scenario_suite(&suite);
for err in &errors {
    eprintln!("[{}] {}", err.field, err.message);
}
```

## Available Scenarios

The repository includes pre-built scenario files in `scenarios/`, including:

- `coverage.ideal.json` — 5 agents, 3 tasks, ideal network
- `coverage.safety.json` — coverage with no-fly zone
- `sar.ideal.json` — SAR with belief-aware grid
- `sar.uncertain.json` — SAR with moderate sensor noise
- `sar.noisy.json` — SAR with high false-positive rate
- `inspection.linear.json` — linear infrastructure, 3 agents
- `inspection.perimeter.json` — perimeter inspection, battery constraints
- `inspection.random.json` — random graph inspection
- `emergency-mesh.ideal.json` — mesh network with relay
- `cbba_stress.json` — CBBA convergence under packet loss
- `sitl.waypoints.json` — SITL waypoints, 1 agent
- `urban.patrol.json` — M65 Urban Patrol road-graph simulation fixture
- `urban.search.json` — M66 Urban Search static-bus simulation fixture
- `urban.multi-agent.json` — M67 two-agent Urban replay-analysis fixture
- `urban.corridor-delta.json` — M68 Dijkstra vs corridor-aware planner delta
- standard generated Urban profiles also include M75 `search-moving-bus` and
  `perimeter-square`; these are builder-level fixtures and do not imply
  hardware or physics evidence.

## Export / Import

```rust
use swarm_sim::{export_suite, load_scenario_suite};

let suite = load_scenario_suite("scenarios/coverage.ideal.json")?;
let json = export_suite(&suite)?;
std::fs::write("exported.json", json)?;
```
