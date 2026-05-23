# Scenario DSL

The Scenario DSL is a JSON-based format for describing reproducible simulation scenarios. All scenarios in `scenarios/` use this format.

## Schema Version

Current schema version: `0.1`

All scenario suites must include `"schema_version": "0.1"`. Legacy files without this field default to `"0.1"`.

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
| `safety` | `run_config.safety_config` | Must have `safety_config` with geofence or no-fly zones |

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

The repository includes 12 pre-built scenario files in `scenarios/`:

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

## Export / Import

```rust
use swarm_sim::{export_suite, load_scenario_suite};

let suite = load_scenario_suite("scenarios/coverage.ideal.json")?;
let json = export_suite(&suite)?;
std::fs::write("exported.json", json)?;
```
