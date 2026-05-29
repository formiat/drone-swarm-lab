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
  --agent-id agent-0 \
  --safety-config path/to/sitl-safety.json
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
| `safety config read failed` | `--safety-config` points to a missing/unreadable file | Fix the path or omit the option to use defaults |
| `safety config parse failed` | Safety config is not valid JSON | Fix JSON syntax |
| `safety validation failed` | Mission violates a pre-upload safety rule | Read the `rule_id`, `actual`, and `allowed` fields and adjust scenario/config |
| No PX4 connection | PX4 SITL is not running or address is wrong | Start PX4 SITL and verify the MAVLink endpoint |
| Mission upload timeout | PX4 did not send heartbeat, mission request, or final ack | Verify the endpoint, PX4 mode, and that no other GCS owns the mission protocol |
| Mission rejected | PX4 returned a non-accepted `MISSION_ACK` | Check waypoint coordinates, altitude, frame support, and PX4 logs |
