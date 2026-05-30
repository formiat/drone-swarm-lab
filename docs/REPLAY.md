# Replay / Debuggability

The replay system allows inspecting simulation runs without reading raw JSON. It consists of an event log format, a replay CLI, and ASCII visualization.

## Event Log Schema

**Schema version:** `0.2`

Each simulation run can optionally produce an `EventLog` — a JSON file containing a sequence of timestamped events.

### Event Types

| Event | Description | Fields |
|---|---|---|
| `TickStart` | New simulation tick | `tick` |
| `AgentFailed` | Agent detected as failed | `agent_id`, `tick` |
| `TaskAssigned` | Task assigned to agent | `task_id`, `agent_id`, `tick` |
| `TaskStarted` | Task transitioned to InProgress | `task_id`, `agent_id`, `tick` |
| `TaskCompleted` | Task completed | `task_id`, `agent_id`, `tick` |
| `TaskExpired` | Task expired (deadline reached) | `task_id`, `tick` |
| `MessageSent` | Message sent | `from`, `to`, `tick`, `payload_len` |
| `MessageDropped` | Message dropped | `from`, `to`, `tick`, `reason` |
| `PartitionAdded` | Network partition created | `agent_a`, `agent_b`, `tick` |
| `PartitionRemoved` | Network partition healed | `agent_a`, `agent_b`, `tick` |
| `PoseUpdated` | Agent moved to new pose | `agent_id`, `pose`, `tick` |
| `SarScan` | SAR cell scanned | `agent_id`, `cell`, `tick`, `detected` |
| `SarDetection` | Target detected in SAR | `agent_id`, `target_pose`, `tick` |
| `EdgeVisited` | Inspection edge visited | `edge_id`, `agent_id`, `tick` |
| `SafetyViolation` | Safety constraint violated | `agent_id`, `violation_type`, `tick` |
| `CbbaConverged` | CBBA reached consensus | `tick` |
| `CbbaBundleUpdated` | CBBA bundle changed | `agent_id`, `bundle_size`, `tick` |

### Backward Compatibility

Event logs without `schema_version` default to `"0.2"` and are fully backward compatible with the v0.1 format (which only had the first 8 event types).

## SITL Event Log Schema

SITL/PX4 runs use a separate compact event log because MAVLink protocol events
do not fit the tick-oriented simulation replay schema.

**Schema version:** `sitl_event_log.v1`

Top-level fields:

| Field | Description |
|---|---|
| `schema_version` | Always `sitl_event_log.v1` for the M49 schema |
| `run_id` | Deterministic run id derived from scenario, agent, and mode |
| `scenario_path` | Source scenario path |
| `scenario_name` | Scenario name from the suite |
| `mission` | Scenario suite mission |
| `profile` | Scenario suite profile |
| `agent_id` | SITL agent id |
| `connection_string` | MAVLink connection string, or `null` for mock mode |
| `mode` | `mock`, `connection_upload_only`, or `connection_execute` |
| `events` | Ordered event list with deterministic `step` numbers |

SITL event types are serialized in `snake_case`:

| Event | Description | Key fields |
|---|---|---|
| `multi_agent_run_started` | Common supervisor run started | `step`, `agent_count`, `scenario` |
| `multi_agent_agent_started` | One supervised agent run started | `step`, `agent_id`, `connection_string`, `system_id`, `component_id` |
| `multi_agent_agent_finished` | One supervised agent run finished | `step`, `agent_id`, `final_status`, `completed_task_count` |
| `multi_agent_mission_count_sent` | One supervised agent mission count sent | `step`, `agent_id`, `count` |
| `multi_agent_mission_item_sent` | One supervised agent mission item sent | `step`, `agent_id`, `seq`, `task_id` |
| `multi_agent_current_seq_changed` | One supervised agent telemetry current mission sequence changed | `step`, `agent_id`, `seq`, `task_id` |
| `multi_agent_waypoint_reached` | One supervised agent telemetry waypoint reached | `step`, `agent_id`, `seq`, `task_id` |
| `multi_agent_task_completed` | One supervised agent SITL task marked completed | `step`, `agent_id`, `seq`, `task_id` |
| `multi_agent_failure` | One supervised agent terminal or bounded failure | `step`, `agent_id`, `status`, `error` |
| `multi_agent_run_finished` | Common supervisor run finished | `step`, `overall_status` |
| `connection_opened` | Runtime connection/context opened | `step`, `mode`, `connection_string` |
| `heartbeat_seen` | MAVLink heartbeat or telemetry heartbeat observed | `step` |
| `mission_clear_sent` | Existing mission clear command sent | `step` |
| `mission_count_sent` | Mission item count sent | `step`, `count` |
| `mission_item_requested` | Vehicle requested a mission item | `step`, `seq` |
| `mission_item_sent` | Mission item sent | `step`, `seq`, `task_id` |
| `mission_ack_received` | Final mission upload ack received | `step`, `result`, `accepted` |
| `command_sent` | Lifecycle command sent | `step`, `command` |
| `command_ack_received` | Lifecycle command ack received or timed out | `step`, `command`, `result`, `accepted` |
| `current_seq_changed` | Telemetry current mission sequence changed | `step`, `seq`, `task_id` |
| `waypoint_reached` | Telemetry waypoint reached | `step`, `seq`, `task_id` |
| `task_completed` | SITL task marked completed | `step`, `seq`, `task_id` |
| `abort_requested` | RTL abort was requested or attempted | `step`, `result` |
| `disconnected` | Telemetry connection/progress became disconnected | `step`, `reason` |
| `failure` | Terminal or bounded failure | `step`, `status`, `error` |
| `agent_lost` | Runtime/mock reallocation detected a lost agent | `step`, `agent_id` |
| `task_released` | Task was released from a lost agent | `step`, `task_id`, `previous_agent_id` |
| `task_reassigned` | Released task was reassigned to a survivor | `step`, `task_id`, `from_agent_id`, `to_agent_id`, `latency_ticks` |
| `reallocation_completed` | Reallocation summary for a failed agent | `step`, `failed_agent_id`, `reassignment_count`, `tasks_recovered`, `latency_ticks` |
| `run_completed` | Successful terminal status | `step`, `status` |

Reallocation events are schema/API/runtime covered and are produced by the mock
multi-agent supervisor flow. M58 adds a live multi-agent PX4 supervisor path
that writes common run-start/run-finished events, per-agent mission/task/failure
events with explicit `agent_id`, and aggregate per-agent report artifacts. The
per-agent event variants keep repeated waypoint `seq` values unambiguous across
agents. The live supervisor does not yet inject failures or emit a combined
real-PX4 reallocation log.

M57 keeps these replay semantics stable while moving mock supervisor execution
behind an internal supervisor/controller boundary. `MockAgentController` still
produces the same mock reallocation events, the shared supervisor loop is also
covered by a test-only fake controller, and `SupervisorMetrics` can now be
asserted directly in tests instead of only being parsed from stderr.

## Generating Replay Logs

Replay logs are generated by passing `--replay-log <dir>` to `strategy_comparison`:

```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --quick --mission coverage --replay-log results/replay/
```

Each run produces a `.replay.json` file in the specified directory.

SITL replay logs are generated by passing `--replay-log <file>` to
`sitl_agent`:

```bash
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udpin:127.0.0.1:14550 \
  --scenario scenarios/sitl.px4-golden.json \
  --agent-id agent-0 \
  --execute \
  --run-report target/sitl/single-agent-report.json \
  --replay-log target/sitl/single-agent.sitl-log.json
```

Mock mode can generate a portable SITL log without PX4:

```bash
cargo run --bin sitl_agent -- \
  --mock \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0 \
  --replay-log target/sitl/mock.sitl-log.json
```

The multi-agent supervisor can generate a common mock/fake run log with
heartbeat-timeout reallocation:

```bash
cargo run --bin sitl_supervisor -- \
  --mock \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.config.json \
  --fail-agent agent-0 \
  --fail-after-ticks 1 \
  --heartbeat-timeout-ticks 3 \
  --replay-log target/sitl/multi-supervisor.sitl-log.json
```

The experimental live multi-agent PX4/SIH execute supervisor uses the same SITL
event log schema for a common supervisor log:

```bash
cargo run --bin sitl_supervisor --features mavlink-transport -- \
  --connection --execute \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.execute.config.json \
  --safety-config path/to/sitl-safety.json \
  --run-report target/sitl/multi-agent-report.json \
  --replay-log target/sitl/multi-agent.sitl-log.json
```

The live log contains common `multi_agent_run_started`, per-agent
`multi_agent_agent_started` / `multi_agent_agent_finished`, per-agent
`multi_agent_mission_item_sent` / `multi_agent_task_completed` /
`multi_agent_failure` events, and `multi_agent_run_finished`. Mission/progress
events include `agent_id`, so a common log can reconstruct mappings such as
`(agent_id, seq) -> task_id` even when each agent starts waypoint numbering at
`seq=0`. The detailed per-agent final state is in the
`sitl_multi_agent_run_report.v1` report. Live PX4 failure/reallocation events
remain future work.

The captured M48 PX4 SIH replay is stored at
`results/m48_px4_sitl_2026-05-30/single-agent.sitl-log.json` with a compact
summary in `results/m48_px4_sitl_2026-05-30/replay-summary.txt`.
The captured mock supervisor reallocation replay is stored at
`results/m54_multi_agent_supervisor_2026-05-30/multi-supervisor.sitl-log.json`.

## Replay CLI

The `replay` binary provides three modes:

### Summary mode

Prints aggregate statistics from the event log:

```bash
cargo run --bin replay -- --log results/replay/run_0.replay.json --summary
```

Output includes:
- Total ticks, assignments, completions, failures
- Messages sent/dropped
- SAR scans/detections
- Edges visited
- Safety violations
- CBBA convergence ticks

### Tick snapshot mode

Renders an ASCII grid showing agent positions at a specific tick:

```bash
cargo run --bin replay -- --log results/replay/run_0.replay.json --tick 50
```

Legend:
- `A` — active agent
- `X` — failed agent
- `*` — mixed (active + failed in same cell)
- `.` — empty cell
- `2-9` — multiple agents in same cell

### Follow mode

Renders ASCII grid for every tick (useful for animation):

```bash
cargo run --bin replay -- --log results/replay/run_0.replay.json --follow
```

### SITL summary mode

Prints a compact summary from a SITL event log:

```bash
cargo run --bin replay -- --sitl-summary target/sitl/single-agent.sitl-log.json
```

Example output:

```text
SITL run: sitl_waypoints_0:agent-0:connection_execute
Scenario: sitl_waypoints_0 | Agent: agent-0 | Mode: connection_execute
Events: 18
Upload: clear=1 count=1 requested=3 sent=3 ack_accepted=1 ack_rejected=0
Commands: sent=3 ack_accepted=3 ack_rejected=0
Telemetry: heartbeat=2 current_seq=2 waypoint_reached=3 task_completed=3
Failures: aborts=0 disconnected=0 failures=0 final_status=completed
Reallocation: agent_lost=0 task_released=0 task_reassigned=0 completed=0 tasks_recovered=0 latency_ticks=none
Multi-agent: started=0 finished=0 agents_started=0 agents_finished=0 agent_count=none
Multi-agent events: mission_count=0 mission_items=0 current_seq=0 waypoint_reached=0 task_completed=0 failures=0
```

`--sitl-summary` is mutually exclusive with `--log`, `--summary`, `--tick`,
and `--follow`.

## Replay API

```rust
use swarm_replay::{read_from_file, summarize, snapshot_at_tick, render_ascii_grid};

let log = read_from_file(path)?;
let summary = summarize(&log);
println!("Ticks: {}, Assignments: {}", summary.total_ticks, summary.assignments);

let snap = snapshot_at_tick(&log, 50);
let grid = render_ascii_grid(&snap, (0.0, 100.0, 0.0, 100.0), 20);
println!("{}", grid);
```
