# Mission Command IR

**M80 — Mission Command Intermediate Representation**

## IR is mission intent, not hardware execution

The Mission Command IR is a hardware-agnostic representation of drone mission
actions. It sits between mission planning and hardware-specific execution:

```text
MissionIntent -> MissionCommand IR -> MAVLink Common Compiler (M81) -> backend/profile execution layer
```

A `MissionCommandPlan` encodes **what** a mission should do — the sequence of
commands, their parameters, coordinate frame, altitude reference, and timeout
policy. It does **not** encode:

- MAVLink message bytes or field layouts.
- PX4- or ArduPilot-specific mode transitions.
- Hardware-level motor control or stabilisation.
- Any network transport or serial link.

M81 implements the first backend compiler: `compile_mavlink_common_plan`
translates the IR into a transport-free `MavlinkCommonPlan`. Later layers can
apply PX4/ArduPilot capability profiles and transport/upload behavior without
moving MAVLink fields back into mission logic.

## Command primitives

All 13 hardware-agnostic mission command primitives:

| Command | Parameters | Semantics |
|---|---|---|
| `arm` | — | Arm vehicle motors |
| `disarm` | — | Disarm vehicle motors |
| `takeoff` | `altitude_m` | Ascend to altitude and hover |
| `hold` | `duration_secs` | Hold current position for duration |
| `land` | — | Land at current horizontal position |
| `return_to_launch` | — | Return to home and land |
| `go_to` | `position` | Fly to a specific position |
| `follow_route` | `route_id`, `waypoints` | Follow an ordered named route |
| `loiter_time` | `duration_secs` | Loiter at current position for duration |
| `orbit` | `center`, `radius_m`, `turns`, `direction` | Perform circular orbit |
| `pause` | — | Pause mission execution (vehicle holds) |
| `resume` | — | Resume after `pause` |
| `abort` | — | Abort mission immediately |

## Explicit semantics

Each `MissionCommandPlan` carries:

- **`coordinate_frame`**: `wgs84` | `local_ned` | `local_enu` — applies to all
  positions in the plan.
- **`altitude_reference`**: `amsl` | `agl` | `relative_home` | `ellipsoid`.
- **`timeout_policy`**: per-command and completion timeouts in seconds, and an
  action on timeout (`abort` | `return_to_launch` | `hold`).
- **`expected_terminal_state`**: what the vehicle state should be when the
  plan completes (`landed` | `hovering` | `at_waypoint` | `orbit_complete` |
  `route_complete` | `aborted`).
- **`completion_tolerance`**: acceptable position and altitude error in metres.
- **`mission_id`** and per-command **`command_id`** for logging and deconfliction.
- Optional **`source_task_id`**, **`source_route_id`**, **`source_agent_id`**
  per command entry for provenance tracking.

## Validation rules

`swarm_mission_ir::validate(&plan)` enforces:

| Rule | Condition | Error |
|---|---|---|
| Unique command ids | No two `CommandId` values are equal | `DuplicateCommandId` |
| Positive takeoff altitude | `altitude_m > 0` | `InvalidTakeoffAltitude` |
| Positive hold/loiter duration | `duration_secs > 0` | `InvalidDuration` |
| Non-empty route | `follow_route` waypoints list is non-empty | `EmptyRoute` |
| Finite coordinates | All `x`, `y`, `z` / `lat`, `lon`, `alt` are finite | `NonFiniteCoordinate` |
| Positive orbit radius | `radius_m > 0` | `InvalidOrbitRadius` |
| Positive orbit turns | `turns > 0` | `InvalidOrbitTurns` |
| Frame/position consistency | `wgs84` frame requires `geo` positions; `local_*` frames require `local` positions | `AmbiguousCoordinateFrame` |

## Urban route as `follow_route`

An Urban road-graph route can be represented as a single `follow_route` command
without any MAVLink fields. The `swarm_sim::urban_route_to_follow_route`
utility converts an `UrbanPlannedRoute` into a `MissionCommand::FollowRoute`:

```rust
use swarm_sim::urban_route_to_follow_route;
use swarm_mission_ir::RouteId;

let route_id = RouteId::from("urban-patrol-loop".to_owned());
let cmd = urban_route_to_follow_route(&map, &planned_route, route_id, 5.0);
```

Each segment's destination node becomes a `MissionWaypoint` with a local
position (`x_m`, `y_m`, `z_m = altitude_m`). The function returns `None` when
the route has no segments or when no node poses can be resolved.

## What this IR is NOT

- **Not a MAVLink plan.** No message serialisation, no MISSION_ITEM_INT, no
  MAVLink command ids.
- **Not PX4- or ArduPilot-specific.** No mode transitions, no autopilot
  parameter references, no vendor SDK calls.
- **Not hardware-ready.** This is a pre-compilation IR. Hardware execution
  requires a backend compiler (M81+) and a transport layer.
- **Not a certified safety layer.** Validation rules catch structural errors
  only; they do not substitute for hardware preflight or FC safety systems.

## Compiler path

- **M81 MAVLink Common Compiler**: implemented. It translates
  `MissionCommandPlan` into `MavlinkCommonPlan` with typed
  `MAV_CMD_NAV_TAKEOFF`, `MAV_CMD_NAV_WAYPOINT`,
  `MAV_CMD_NAV_LOITER_TIME`, `MAV_CMD_NAV_LAND`, expected ACKs, telemetry
  milestones, deterministic SHA-256 `command_ir_hash`, and structured
  unsupported features. It is no hardware upload; PX4/ArduPilot semantics are not identical. See
  [`docs/MAVLINK_COMMON_COMPILER.md`](MAVLINK_COMMON_COMPILER.md).
  Validate dry-run compiler artifacts with `artifact_validator --mode dry-run`.
- **M82 PX4 / ArduPilot Capability Profiles**: annotates or rejects commands
  based on autopilot-stack compatibility.
- **M83 Primitive Real Mission Pack**: three concrete missions that compile to
  MAVLink plans.

## Artifact schema

The crate `swarm-mission-ir` uses schema version `"mission_command_ir.v1"` for
`MissionCommandPlan` artifacts. Dry-run artifacts produced by `sitl_agent
--dry-run` include an optional `command_ir_summary` field with a compact
summary of the IR derived from the waypoint list and an optional
`mavlink_common_plan` field using schema version `"mavlink_common_plan.v1"`.
