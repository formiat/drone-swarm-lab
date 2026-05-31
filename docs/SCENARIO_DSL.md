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
| `urban-patrol` | `run_config.urban_state` | Must include `UrbanMap`, route loop, Dijkstra planner, valid node/edge refs, and waypoint placeholder tasks; M65 runner follows the planned route in order |
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
- `planner` — currently must be `"dijkstra"`.

The fixture still uses `TaskKind::Waypoint` placeholder tasks for compatibility,
but Urban Patrol completion is now route-based rather than task-assignment
based. Completion means the selected scout traverses every planned route
segment in order before timeout with zero Urban judge violations. Failure means
timeout, a static/execution judge violation, or an invalid start contract. The
selected alive agent must start within `0.01m` of the validated start node pose;
M65 v0 does not infer or silently teleport from an arbitrary `agent.pose`. M65
v0 has no replanning, so `urban_replan_count = 0`.

It does not implement lidar/raycast, bus detection, dynamic obstacles,
multi-agent route conflicts, arbitrary polygons, PX4/SITL export, hardware
readiness, or a visual UI.

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

The repository includes 20 pre-built scenario files in `scenarios/`:

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

## Export / Import

```rust
use swarm_sim::{export_suite, load_scenario_suite};

let suite = load_scenario_suite("scenarios/coverage.ideal.json")?;
let json = export_suite(&suite)?;
std::fs::write("exported.json", json)?;
```
