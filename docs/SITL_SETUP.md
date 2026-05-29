# SITL Setup Guide

This guide covers the portable SITL paths and the experimental PX4 SITL path.
The project is still a research prototype: do not use this workflow on real
hardware.

## Mode Matrix

| Mode | Command flag | External deps | Purpose | Status |
|---|---|---|---|---|
| Mock | `--mock` | None | Send extracted waypoints to in-memory `MockMavlinkTransport` | Stable and CI-friendly |
| Dry-run | `--dry-run` | None | Print the mission upload plan without connecting to PX4 | Stable portable contract |
| PX4 SITL | `--connection <addr>` | PX4 SITL + `mavlink-transport` feature | Upload waypoint mission to PX4 SITL | Experimental; no arm/takeoff/execution supervision |

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
  --agent-id agent-0
```

Without the feature, `--connection` returns a stable error with the required
build instruction. A syntactically bad connection string returns a stable
`bad connection string` error without attempting a network connection.

The connection path performs a minimal mission upload transaction:

1. Wait for MAVLink `HEARTBEAT`.
2. Send `MISSION_CLEAR_ALL` by default.
3. Send `MISSION_COUNT`.
4. Answer `MISSION_REQUEST_INT` with `MISSION_ITEM_INT`.
5. Fall back to legacy `MISSION_REQUEST` if the vehicle sends it.
6. Require final `MISSION_ACK` with `MAV_MISSION_ACCEPTED`.

This mode uploads the mission only. It does not arm the vehicle, take off,
switch modes, start the mission, track execution progress, or perform
operator/safety checks.

## Coordinate Frame Contract

For M44, `sitl_agent` uses a deliberately narrow coordinate contract:

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

Arm/takeoff, mission start, execution tracking, telemetry supervision, and
multi-agent SITL remain future milestones.

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
| No PX4 connection | PX4 SITL is not running or address is wrong | Start PX4 SITL and verify the MAVLink endpoint |
| Mission upload timeout | PX4 did not send heartbeat, mission request, or final ack | Verify the endpoint, PX4 mode, and that no other GCS owns the mission protocol |
| Mission rejected | PX4 returned a non-accepted `MISSION_ACK` | Check waypoint coordinates, altitude, frame support, and PX4 logs |
