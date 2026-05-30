# SITL Setup Guide

This guide covers the portable SITL paths and the experimental PX4 SITL path.
The project is still a research prototype: do not use this workflow on real
hardware.

## Mode Matrix

| Mode | Command flag | External deps | Purpose | Status |
|---|---|---|---|---|
| Mock | `--mock` | None | Send extracted waypoints to in-memory `MockMavlinkTransport` | Stable and CI-friendly |
| Dry-run | `--dry-run` | None | Print the mission upload plan without connecting to PX4 | Stable portable contract |
| PX4 SITL upload-only | `--connection <addr> [--upload-only]` | PX4 SITL + `mavlink-transport` feature | Upload waypoint mission to PX4 SITL without starting flight | Experimental |
| PX4 SITL execute | `--connection <addr> --execute` | PX4 SITL + `mavlink-transport` feature | Upload, arm/takeoff/start mission, map telemetry to task progress, write optional final report, and abort on bounded failures | Experimental single-agent golden path |

## CI / Manual Boundary

M50 makes the portable SITL path regression-safe without requiring PX4. M51 adds
mock/fake/runtime-level failure and dynamic reallocation checks for the future
multi-agent SITL path. The automated boundary is deliberately narrow: tests may
load scenarios, extract waypoints, validate static safety rules, run dry-run,
run mock mode, inspect mock replay logs, detect a lost agent by heartbeat
timeout, recover assignable tasks on surviving agents, and summarize
reallocation events. These checks use no external PX4, no simulator process, no
network endpoint, and no real hardware.

Recommended automated checks:

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_agent portable_sitl_regression_smoke

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_docs

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-runtime reallocation

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples sitl_observability
```

Manual/local PX4 checks are separate. They require a running PX4 SITL instance,
the `mavlink-transport` feature, and an operator-controlled endpoint such as
`udp:127.0.0.1:14550`. Manual PX4 verification may cover upload-only mode,
execute lifecycle, telemetry progress, timeout tuning, final reports, and SITL
replay logs.

M51 reallocation events are part of this portable boundary. They can appear in
SITL event logs as `agent_lost`, `task_released`, `task_reassigned`, and
`reallocation_completed`. They prove the runtime/mock contract, not live
multi-agent PX4 readiness.

Out of scope for automated CI in this repository:

- real PX4 CI orchestration;
- real multi-agent PX4 SITL orchestration;
- HIL;
- real aircraft;
- production autopilot certification;
- production safety guarantees.

## Quick Start: Dry-Run Mode

Dry-run is the recommended first check before any PX4 work. It loads the
scenario, extracts pose tasks, and prints the waypoint plan that the connection
mode uploads as a MAVLink mission.

```bash
cargo run --bin sitl_agent -- \
  --dry-run --scenario scenarios/sitl.waypoints.json --agent-id agent-0
```

Expected output includes:

```text
mode: dry-run
agent_id: agent-0
scenario_path: scenarios/sitl.waypoints.json
suite_name: SITL Waypoints
scenario_name: sitl_waypoints_0
mission: sitl
profile: waypoints
coordinate_frame: local_simulation
altitude_source: pose.z (serde default 0.0 when omitted)
waypoints:
  seq=0 task_id=wp-0 x=10.000 y=20.000 z=0.000
```

Dry-run does not create a MAVLink connection and does not upload anything to
PX4.

## Quick Start: Mock Mode

Mock mode sends the same extracted waypoints to an in-memory
`MockMavlinkTransport` and prints them to stderr. It requires no external
dependencies and works out of the box.

```bash
cargo run --bin sitl_agent -- \
  --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0
```

Expected output:

```text
SITL Agent: agent-0 | 3 waypoints | mock=true
WAYPOINT seq=0 x=10.0 y=20.0 z=0.0
WAYPOINT seq=1 x=50.0 y=30.0 z=0.0
...
Mock mode: 3 waypoints sent.
```

This remains the recommended path for CI and tests that should not depend on
PX4.

For the full portable smoke, run `portable_sitl_regression_smoke`. It verifies
scenario load, waypoint extraction, safety validation, dry-run output, mock
transport output, expected mission item count, and mock replay log summary.

## Experimental PX4 SITL Mode

Prerequisites:

1. PX4 SITL running, for example `make px4_sitl gazebo_iris`.
2. MAVLink connection address, for example `udp:127.0.0.1:14550`.
3. Build with the `mavlink-transport` feature.

```bash
cargo build --bin sitl_agent --features mavlink-transport
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0 \
  --safety-config path/to/sitl-safety.json \
  --upload-only
```

Without the feature, `--connection` returns a stable error with the required
build instruction. A syntactically bad connection string returns a stable
`bad connection string` error without attempting a network connection.
If `--safety-config` is omitted, conservative SITL defaults are used.

The connection path performs a minimal mission upload transaction:

1. Validate the mission against pre-upload safety rules.
2. Wait for MAVLink `HEARTBEAT`.
3. Send `MISSION_CLEAR_ALL` by default.
4. Send `MISSION_COUNT`.
5. Answer `MISSION_REQUEST_INT` with `MISSION_ITEM_INT`.
6. Fall back to legacy `MISSION_REQUEST` if the vehicle sends it.
7. Require final `MISSION_ACK` with `MAV_MISSION_ACCEPTED`.

This mode uploads the mission only. It does not arm the vehicle, take off,
switch modes, start the mission, track execution progress, or perform
runtime collision avoidance.

## Experimental PX4 Execute Lifecycle

M46 added explicit execution mode after successful upload, and M47 extends it
with a single-agent telemetry progress loop. It is opt-in: plain
`--connection` remains upload-only for safety and backwards compatibility.

```bash
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0 \
  --safety-config path/to/sitl-safety.json \
  --execute \
  --timeout 5 \
  --telemetry-timeout 10 \
  --no-progress-timeout 60 \
  --run-report target/sitl/single-agent-report.json
```

Useful bounded variants:

```bash
# Skip arm for controlled SITL experiments where the vehicle is already armed.
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0 \
  --execute --no-arm --timeout 5 --telemetry-timeout 10 --no-progress-timeout 60

# Start the lifecycle and then request RTL abort immediately after a short delay.
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0 \
  --execute --abort-after 10 --timeout 5
```

Execution lifecycle:

1. Upload mission using the same safety-gated mission protocol as upload-only.
2. Send `MAV_CMD_COMPONENT_ARM_DISARM` unless `--no-arm` is set.
3. Send `MAV_CMD_NAV_TAKEOFF` using the first waypoint altitude with a 2.5m
   floor.
4. Send `MAV_CMD_MISSION_START`.
5. Require a fresh post-start `HEARTBEAT` before considering the active
   lifecycle healthy.
6. Enter the telemetry progress loop and consume:
   `HEARTBEAT`, `MISSION_CURRENT`, `MISSION_ITEM_REACHED`, and runtime
   `MISSION_ACK` rejection/completion signals.
7. Map each mission item `seq` to the `task_id` from the generated `SitlPlan`.
8. Exit `0` only after all waypoint tasks reach `TaskStatus::Completed`.
9. If `--abort-after <seconds>` is set, send RTL abort after that delay and do
   not report the mission as completed.

Failure behavior:

- upload failure: no arm/takeoff/start command is sent;
- arm failure: exit non-zero with a clear command error;
- takeoff/start command failure: send RTL abort and report the abort result;
- post-start heartbeat timeout: send RTL abort and report the abort result;
- telemetry heartbeat timeout (`--telemetry-timeout`): mark unfinished tasks as
  failed, send RTL abort, and exit non-zero;
- no-progress timeout (`--no-progress-timeout`): mark unfinished tasks as
  failed, send RTL abort, and exit non-zero;
- runtime mission rejection: mark unfinished tasks as failed, send RTL abort,
  and exit non-zero;
- abort failure is reported, not hidden as success.

Progress output is intentionally compact and immediate:

```text
progress: current seq=1 task_id=wp-1 completed=1/3
progress: reached seq=1 task_id=wp-1 completed=2/3
Real MAVLink mode: mission complete; completed=3 failed=0 total=3
```

This is still an experimental single-agent SITL path. It does not merge
multi-agent telemetry, does not provide a UI, and is not a hardware failsafe
implementation.

## M48 Single-Agent Golden Path Report

M48 adds an optional structured final report for the single-agent execute path:

```bash
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0 \
  --execute \
  --timeout 5 \
  --telemetry-timeout 10 \
  --no-progress-timeout 60 \
  --run-report target/sitl/single-agent-report.json
```

The report is pretty JSON. It is written only for `--connection --execute` and
contains:

```json
{
  "schema_version": "sitl_run_report.v1",
  "scenario_path": "scenarios/sitl.waypoints.json",
  "scenario_name": "sitl_waypoints_0",
  "mission": "sitl",
  "profile": "waypoints",
  "agent_id": "agent-0",
  "connection_string": "udp:127.0.0.1:14550",
  "mode": "connection_execute",
  "mission_item_count": 3,
  "completed_count": 3,
  "failed_count": 0,
  "final_status": "completed",
  "error": null,
  "abort_result": null
}
```

Failure reports use the same schema and set `final_status`, `error`, and
`abort_result` when an abort was attempted. The report is a final summary only;
use the M49 replay log when the ordered protocol/progress trace is needed.

## M49 SITL Replay Log

M49 adds an optional ordered event log for mock, upload-only, and execute SITL
runs. It complements `--run-report`: the report answers "what was the final
state?", while the replay log answers "what happened before that state?".

```bash
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0 \
  --execute \
  --timeout 5 \
  --telemetry-timeout 10 \
  --no-progress-timeout 60 \
  --run-report target/sitl/single-agent-report.json \
  --replay-log target/sitl/single-agent.sitl-log.json
```

The event log is pretty JSON with schema version `sitl_event_log.v1`. It uses
deterministic `step` numbers instead of wall-clock timestamps and records
semantic SITL events, not every raw MAVLink packet. Event classes include:

- connection opened;
- heartbeat seen;
- mission clear/count/request/item/ack;
- arm/takeoff/start/abort command sent and acknowledged/rejected/timeout;
- telemetry current sequence changes;
- waypoint reached and task completed;
- abort requested, disconnected, failure, and run completed.

Upload-only mode also supports `--replay-log`:

```bash
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0 \
  --upload-only \
  --replay-log target/sitl/upload-only.sitl-log.json
```

Mock mode supports the same flag for portable local checks without PX4:

```bash
cargo run --bin sitl_agent -- \
  --mock \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0 \
  --replay-log target/sitl/mock.sitl-log.json
```

Dry-run rejects `--replay-log`, because dry-run prints a static upload plan and
does not execute a runtime behavior trace.

To inspect the compact summary:

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
```

## M48 Tested PX4 SITL Setup

Manual PX4 SITL verification is required before treating a local environment as
tested. This repository run did not start a live PX4 simulator, so the verified
setup fields below are intentionally explicit and should be filled with the
operator's actual local run:

- PX4 version/commit: pending local PX4 run.
- Simulator backend: pending local PX4 run, commonly Gazebo / PX4 SITL.
- Startup command: `make px4_sitl gazebo_iris` or the backend-specific
  equivalent used locally.
- `sitl_agent` connection string: commonly `udp:127.0.0.1:14550`.
- Expected MAVLink endpoint: PX4 should emit heartbeat and mission protocol
  responses on the configured UDP endpoint.
- Golden scenario: `scenarios/sitl.waypoints.json`.
- Expected result: all waypoint tasks completed and final report
  `final_status=completed`.

Do not copy these pending fields into a release note as a verified result. A
valid M48 manual result must include the actual PX4 version/backend, exact
startup command, exact connection string, command output summary, and generated
report JSON.

## Pre-Upload Safety Validation

`--connection` validates the scenario before creating a MAVLink transport or
uploading mission items. The default SITL safety config is intentionally static
and portable:

- geofence: `x=-1000..=1000`, `y=-1000..=1000`;
- altitude: `0..=120m`;
- max waypoint jump: `500m`;
- max mission radius from home: `1000m`;
- no no-fly zones by default;
- home is required and resolved from `--safety-config`, `scenario.base_station`,
  or the selected agent's initial pose.

Optional JSON config:

```json
{
  "geofence": { "min_x": -500.0, "max_x": 500.0, "min_y": -500.0, "max_y": 500.0 },
  "min_altitude_m": 5.0,
  "max_altitude_m": 80.0,
  "max_waypoint_jump_m": 150.0,
  "max_mission_radius_m": 400.0,
  "no_fly_zones": [
    { "id": "nfz-0", "bounds": { "min_x": 20.0, "max_x": 40.0, "min_y": 20.0, "max_y": 40.0 } }
  ],
  "home": { "x": 0.0, "y": 0.0, "z": 0.0 },
  "require_home": true
}
```

Validation errors are actionable and include a stable `rule_id`, task id or
waypoint sequence when available, actual value, and allowed range. Example:

```text
safety validation failed: rule_id=outside_geofence task_id=wp-1 seq=1 actual=point=(50.000,30.000) allowed=geofence=[x:0.000..=20.000, y:0.000..=25.000]
```

This is not hardware certification and not runtime collision avoidance. It is a
pre-upload guard for the experimental SITL connection path.

## Coordinate Frame Contract

For M45, `sitl_agent` uses a deliberately narrow coordinate contract:

- `Pose { x, y, z }` means local simulation coordinates.
- `x` and `y` are not WGS84 latitude/longitude.
- `z` is interpreted as altitude relative to the local origin.
- In PX4 SITL mode, `z` is sent unchanged as relative altitude; `home_origin.alt_m`
  is not subtracted for the relative-altitude MAVLink frame.
- If `z` is omitted in JSON, serde defaults it to `0.0`.
- `local_simulation` is the only supported frame in dry-run/mock mode.
- In PX4 SITL mode, local `x` is converted as east meters and local `y` as
  north meters from the configured home origin.
- The default home origin is the common PX4 SITL Zurich origin:
  `lat=47.397742`, `lon=8.545594`, `alt=0.0`.
- Uploaded items use `MISSION_ITEM_INT` with
  `MAV_FRAME_GLOBAL_RELATIVE_ALT_INT`.

M47 adds single-agent telemetry progress mapping after arm/takeoff/start. It
tracks `MISSION_CURRENT`, waypoint reached telemetry, task completion, runtime
mission rejection, disconnect timeout, and no-progress timeout. Multi-agent
SITL telemetry merge and hardware-specific failsafe tuning remain future work.

## Real Hardware Warning

Do not use the current `sitl_agent` against real drones. The repository does not
provide a certified safety layer, hardware readiness checks, preflight policy,
operator workflow, emergency handling, or a production flight workflow. Treat
all SITL functionality as simulation/development tooling.

## Troubleshooting

| Problem | Cause | Fix |
|---|---|---|
| `missing SITL mode` | No `--mock`, `--dry-run`, or `--connection` was provided | Choose exactly one mode |
| `conflicting SITL modes` | More than one mode was provided | Keep exactly one of `--mock`, `--dry-run`, `--connection <addr>` |
| `no pose tasks found` | Scenario has no tasks with `pose` | Use or adapt `scenarios/sitl.waypoints.json` |
| `feature missing` | `--connection` was used without `mavlink-transport` | Build/run with `--features mavlink-transport` |
| `bad connection string` | Connection address is not a supported form | Use `udp:<host>:<port>` for PX4 SITL |
| `safety config read failed` | `--safety-config` points to a missing/unreadable file | Fix the path or omit the option to use defaults |
| `safety config parse failed` | Safety config is not valid JSON | Fix JSON syntax |
| `safety validation failed` | Mission violates a pre-upload safety rule | Read the `rule_id`, `actual`, and `allowed` fields and adjust scenario/config |
| Mock works but PX4 fails | Portable waypoint extraction is valid, but the external PX4 endpoint, feature flag, mission protocol, or vehicle state is not ready | Re-run dry-run/mock first, then verify PX4 SITL startup, `mavlink-transport`, endpoint, heartbeat, arming state, and PX4 logs |
| Unexpected coordinate or altitude behavior | SITL uses the local simulation coordinate frame, not WGS84 input coordinates | Re-read Coordinate Frame Contract and verify `Pose { x, y, z }` values before upload |
| No PX4 connection | PX4 SITL is not running or address is wrong | Start PX4 SITL and verify the MAVLink endpoint |
| Mission upload timeout | PX4 did not send heartbeat, mission request, or final ack | Verify the endpoint, PX4 mode, and that no other GCS owns the mission protocol |
| Mission rejected | PX4 returned a non-accepted `MISSION_ACK` | Check waypoint coordinates, altitude, frame support, and PX4 logs |
| Command rejected | PX4 returned a non-accepted `COMMAND_ACK` for arm/takeoff/start/abort | Check PX4 mode, arming checks, safety state, and command parameters |
| Post-start heartbeat timeout | `--execute` started the mission but did not observe fresh heartbeat before `--timeout` | Verify PX4 is still connected and inspect PX4/SITL logs; the agent attempts RTL abort |
| Telemetry heartbeat timeout | `--execute` started progress tracking but did not observe heartbeat before `--telemetry-timeout` | Verify PX4 is still connected and inspect PX4/SITL logs; the agent attempts RTL abort |
| No mission progress timeout | Heartbeats may continue, but mission seq/completion did not advance before `--no-progress-timeout` | Increase timeout for long legs or inspect PX4 mission execution; the agent attempts RTL abort |
| Run report write failed | `--run-report` parent directory cannot be created or the JSON file cannot be written | Fix path permissions or choose a writable path |
| Replay log write failed | `--replay-log` parent directory cannot be created or the JSON file cannot be written | Fix path permissions or choose a writable path |
