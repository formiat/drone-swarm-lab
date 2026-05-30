# SITL Setup Guide

This guide covers the portable SITL paths and the experimental PX4 SITL path.
The project is still a research prototype: do not use this workflow on real
hardware.

## Mode Matrix

| Mode | Command flag | External deps | Purpose | Status |
|---|---|---|---|---|
| Mock | `--mock` | None | Send extracted waypoints to in-memory `MockMavlinkTransport` | Stable and CI-friendly |
| Dry-run | `--dry-run` | None | Print the mission upload plan without connecting to PX4 | Stable portable contract |
| Multi-agent dry-run/mock | `sitl_supervisor --dry-run/--mock` or `sitl_agent --multi-agent-config` | None | Split explicit waypoint subsets, run mock supervisor orchestration, and produce a manifest/replay log | Stable M52 foundation |
| PX4 SITL upload-only | `--connection <addr> [--upload-only]` | PX4 SITL + `mavlink-transport` feature | Upload waypoint mission to PX4 SITL without starting flight | Experimental |
| PX4 SITL execute | `--connection <addr> --execute` | PX4 SITL + `mavlink-transport` feature | Upload, arm/takeoff/start mission, map telemetry to task progress, write optional final report, and abort on bounded failures | Experimental single-agent golden path |

## CI / Manual Boundary

M50 makes the portable SITL path regression-safe without requiring PX4. M51 adds
mock/fake/runtime-level failure and dynamic reallocation checks for the future
multi-agent SITL path. M52 adds a portable multi-agent foundation: config parse,
explicit task split, dry-run/mock manifest, mock supervisor orchestration,
generated standalone commands, and duplicate ownership rejection before upload.
The automated boundary is
deliberately narrow: tests may load scenarios, extract waypoints, validate
static safety rules, run dry-run, run mock mode, inspect mock replay logs,
detect a lost agent by heartbeat timeout, recover assignable tasks on surviving
agents, summarize reallocation events, and validate multi-agent manifests.
These checks use no external PX4, no simulator process, no network endpoint,
and no real hardware.

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

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples sitl_multi_agent

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_agent multi_agent
```

Manual/local PX4 checks are separate. They require a running PX4 SITL instance,
the `mavlink-transport` feature, and an operator-controlled endpoint such as
`udpin:127.0.0.1:14550`. Manual PX4 verification may cover upload-only mode,
execute lifecycle, telemetry progress, timeout tuning, final reports, and SITL
replay logs. A two-instance PX4 SIH upload-only check is captured in
`results/m55_multi_agent_px4_sih_2026-05-30/`.

M51 reallocation events are part of this portable boundary. `sitl_supervisor
--mock` can emit `agent_lost`, `task_released`, `task_reassigned`, and
`reallocation_completed` after a deterministic heartbeat timeout. This proves
the runtime/mock supervisor contract, not live multi-agent PX4 failure handling.

M52 multi-agent manifests are also part of this portable boundary. They prove
that ownership and per-agent waypoint subsets are explicit and deterministic;
the optional PX4 SIH check proves upload-only mission acceptance for two local
instances. It does not prove autonomous multi-agent PX4 execute orchestration.

Out of scope for automated CI in this repository:

- real PX4 CI orchestration;
- real multi-agent PX4 SITL execute/failure orchestration;
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

## Multi-Agent SITL Foundation

M52 adds a config-driven multi-agent SITL foundation. It is intentionally
limited to explicit ownership and portable dry-run/mock behavior:

- `agent_id` -> MAVLink `system_id`;
- `agent_id` -> MAVLink `component_id`;
- `agent_id` -> connection string;
- `agent_id` -> explicit assigned task subset;
- per-agent start delay;
- per-agent `upload_only` / `execute` lifecycle mode.

Example `multi_sitl.v1` config:

```json
{
  "schema_version": "multi_sitl.v1",
  "agents": [
    {
      "agent_id": "agent-0",
      "system_id": 1,
      "component_id": 1,
      "connection_string": "udpin:127.0.0.1:14550",
      "start_delay_ms": 0,
      "lifecycle": "upload_only",
      "task_ids": ["wp-0", "wp-1"]
    },
    {
      "agent_id": "agent-1",
      "system_id": 2,
      "component_id": 1,
      "connection_string": "udpin:127.0.0.1:14560",
      "start_delay_ms": 250,
      "lifecycle": "execute",
      "task_ids": ["wp-2"]
    }
  ]
}
```

Use `sitl_supervisor` to inspect the full dry-run manifest:

```bash
cargo run -p swarm-examples --bin sitl_supervisor -- \
  --dry-run \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.config.json
```

The output is a `multi_sitl_manifest.v1` JSON document with scenario metadata,
per-agent task subsets, waypoint subsets, ownership summary, and generated
standalone commands for the several-process workflow.

Mock supervisor mode exercises the same split without PX4:

```bash
cargo run -p swarm-examples --bin sitl_supervisor -- \
  --mock \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.config.json \
  --manifest target/sitl/multi-agent-manifest.json
```

Mock supervisor mode also supports deterministic failure injection and a common
SITL replay log:

```bash
cargo run -p swarm-examples --bin sitl_supervisor -- \
  --mock \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.config.json \
  --fail-agent agent-0 \
  --fail-after-ticks 1 \
  --heartbeat-timeout-ticks 3 \
  --max-ticks 12 \
  --replay-log target/sitl/multi-supervisor.sitl-log.json
```

Expected summary:

```text
SUPERVISOR_METRICS agents=2 heartbeats=6 completed_tasks=4 lost_agents=1 reassignment_count=1 tasks_recovered=wp-1 reallocation_latency_ticks=0 final_status=completed
```

The several-process workflow uses the same config from individual `sitl_agent`
invocations:

```bash
cargo run -p swarm-examples --bin sitl_agent --features mavlink-transport -- \
  --scenario scenarios/sitl.multi-agent.json \
  --agent-id agent-0 \
  --multi-agent-config scenarios/sitl.multi-agent.config.json \
  --connection udpin:127.0.0.1:14550 \
  --upload-only
```

If `--connection` or `--upload-only`/`--execute` is omitted, `sitl_agent` can
read the connection and lifecycle from the matching config entry. CLI
connection/lifecycle flags override config for that one process; the task subset
always comes from config.

Duplicate ownership is a hard pre-upload error: the same task id cannot appear
under two agents. Unassigned pose tasks are allowed for partial experiments and
are reported in the manifest ownership summary.

### M57 Supervisor Controller Boundary

M57 is an internal refactor of the portable supervisor path. The external
`sitl_supervisor --dry-run` and `sitl_supervisor --mock` commands remain the
same, but mock orchestration now runs through a testable supervisor/controller
boundary:

- `AgentController` describes one agent lifecycle/progress/abort boundary;
- `MockAgentController` preserves the current no-PX4 mock workflow;
- `SupervisorMetrics` is returned from the supervisor module and still printed
  as the existing `SUPERVISOR_METRICS` line;
- CLI negative cases for missing/invalid supervisor arguments are covered by
  subprocess tests.

This does not add a live PX4 supervisor mode. Real multi-agent PX4/SIH execute
orchestration remains the next milestone after this boundary.

Out of scope for M52:

- robust distributed coordination;
- automatic task allocation;
- automated real multi-agent PX4 CI orchestration;
- live multi-agent PX4 failure/reallocation;
- real hardware usage;
- swarm safety certification.

## Experimental PX4 SITL Mode

Prerequisites:

1. PX4 SITL running, for example `make px4_sitl gz_x500` for Gazebo or
   `make px4_sitl_sih sihsim_quadx` for headless SIH.
2. MAVLink connection address, for example `udpin:127.0.0.1:14550`.
3. Build with the `mavlink-transport` feature.

```bash
cargo build --bin sitl_agent --features mavlink-transport
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udpin:127.0.0.1:14550 \
  --scenario scenarios/sitl.px4-golden.json \
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
  --connection udpin:127.0.0.1:14550 \
  --scenario scenarios/sitl.px4-golden.json \
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
  --connection udpin:127.0.0.1:14550 \
  --scenario scenarios/sitl.px4-golden.json \
  --agent-id agent-0 \
  --execute --no-arm --timeout 5 --telemetry-timeout 10 --no-progress-timeout 60

# Start the lifecycle and then request RTL abort immediately after a short delay.
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udpin:127.0.0.1:14550 \
  --scenario scenarios/sitl.px4-golden.json \
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
  --connection udpin:127.0.0.1:14550 \
  --scenario scenarios/sitl.px4-golden.json \
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
  "scenario_path": "scenarios/sitl.px4-golden.json",
  "scenario_name": "sitl_px4_golden_0",
  "mission": "sitl",
  "profile": "px4-golden",
  "agent_id": "agent-0",
  "connection_string": "udpin:127.0.0.1:14550",
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
  --connection udpin:127.0.0.1:14550 \
  --scenario scenarios/sitl.px4-golden.json \
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
  --connection udpin:127.0.0.1:14550 \
  --scenario scenarios/sitl.px4-golden.json \
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

The current repository contains one captured manual M48 verification:

- Date: 2026-05-30.
- Host OS: Ubuntu 25.10.
- PX4 source path: `/home/formi/Documents/RustProjects/PX4-Autopilot`
  outside this repository.
- PX4 commit: `a2be9197b57b2d065e4c47ca7fc5b7565ee07282`.
- Setup command: `bash Tools/setup/ubuntu.sh --no-nuttx`.
- Simulator backend: PX4 SIH, `sihsim_quadx`, headless.
- Startup command: `PX4_SIM_SPEED_FACTOR=1 make px4_sitl_sih sihsim_quadx`.
- PX4 MAVLink endpoint observed in startup log: normal mode on UDP port
  `18570`, remote port `14550`.
- `sitl_agent` connection string: `udpin:0.0.0.0:14550`.
- Hardware-boundary flag: `--allow-hardware-candidate`, required because
  `0.0.0.0` is a wildcard listener.
- Golden scenario: `scenarios/sitl.px4-golden.json`.
- Captured result directory: `results/m48_px4_sitl_2026-05-30/`.
- Result: all 3 waypoint tasks completed, final report
  `final_status=completed`.

This is a real local PX4 SIH/SITL verification of the single-agent golden path.
It is not Gazebo validation, HIL, real hardware validation, or a production
safety claim.

## M55 Multi-Agent PX4 SIH Upload-Only Check

The repository also contains a captured two-instance PX4 SIH upload-only check:

- Date: 2026-05-30.
- PX4 source path: `/home/formi/Documents/RustProjects/PX4-Autopilot`.
- PX4 commit: `a2be919`.
- Simulator backend: PX4 SIH, `sihsim_quadx`, headless.
- PX4 instances: `-i 0` with `MAV_SYS_ID=1`, `-i 1` with `MAV_SYS_ID=2`.
- Successful upload endpoints:
  - agent 0: `udpin:0.0.0.0:14540`, target system/component `1/1`;
  - agent 1: `udpin:0.0.0.0:14541`, target system/component `2/1`.
- Captured result directory: `results/m55_multi_agent_px4_sih_2026-05-30/`.
- Result: both agents accepted 2 mission items in upload-only mode.

The shared normal/GCS listener `udpin:0.0.0.0:14550` timed out in this
multi-instance setup; use per-instance onboard listener ports for this local
SIH workflow.

This check does not arm, take off, execute, coordinate simultaneous flight, or
verify live PX4 failure/reallocation.

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

Do not use the current `sitl_agent` against real drones as if it were a
production flight workflow. The repository does not provide a certified safety
layer, hardware-specific failsafe tuning, preflight policy, operator workflow,
emergency handling, or flight certification. Treat all SITL functionality as
simulation/development tooling.

The canonical boundary document is
[`docs/HARDWARE_READINESS.md`](HARDWARE_READINESS.md). Read it before any
hardware experiment.

### Connection Classes

`sitl_agent` separates connection classes so real hardware paths are not enabled
accidentally:

- `mock`: `--mock`, in-memory transport, no PX4 and no hardware path.
- `dry-run`: `--dry-run`, prints the mission upload plan and does not open a
  transport.
- `local_px4_sitl_udp`: loopback UDP connections such as
  `udpin:127.0.0.1:14550`, `udpin:localhost:14550`, or
  `udpout:127.0.0.1:14550`. Legacy `udp:*` loopback aliases are also accepted
  and normalized before calling the MAVLink crate.
- `hardware_candidate`: `serial:*`, `tcpin:*`, `tcpout:*`,
  `udpin:0.0.0.0:*`, `udpin:*` with a non-loopback host, `udpout:*` with a
  non-loopback host, or legacy `udp:*`/`tcp:*` with a non-loopback host. These
  may target real hardware, a wildcard listener, or a remote endpoint and require
  `--allow-hardware-candidate`.

Example guarded failure:

```text
hardware candidate connection 'udp:192.168.1.10:14550' classified as hardware_candidate; this path may target real hardware or a remote endpoint and requires --allow-hardware-candidate. Read docs/HARDWARE_READINESS.md before any hardware experiment
```

With `--allow-hardware-candidate`, the CLI prints an explicit warning before it
continues. The flag is an opt-in acknowledgement, not a safety guarantee.

## Troubleshooting

| Problem | Cause | Fix |
|---|---|---|
| `missing SITL mode` | No `--mock`, `--dry-run`, or `--connection` was provided | Choose exactly one mode |
| `conflicting SITL modes` | More than one mode was provided | Keep exactly one of `--mock`, `--dry-run`, `--connection <addr>` |
| `no pose tasks found` | Scenario has no tasks with `pose` | Use or adapt `scenarios/sitl.waypoints.json` |
| `feature missing` | `--connection` was used without `mavlink-transport` | Build/run with `--features mavlink-transport` |
| `bad connection string` | Connection address is not a supported form | Use `udpin:<host>:<port>` for PX4 SITL; legacy `udp:<host>:<port>` aliases are accepted but normalized before opening MAVLink |
| `hardware candidate connection` | Connection may target real hardware, a wildcard listener, or a remote endpoint | Use local loopback PX4 SITL UDP, or read `docs/HARDWARE_READINESS.md` and pass `--allow-hardware-candidate` only for a controlled hardware experiment |
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
