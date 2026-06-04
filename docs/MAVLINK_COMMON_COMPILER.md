# MAVLink Common Compiler

**M81 - MAVLink Common Compiler**

M81 translates the hardware-agnostic `MissionCommandPlan` from
`swarm-mission-ir` into a transport-free `MavlinkCommonPlan`. The compiler
produces typed MAVLink Common command and mission-item intent that can be
validated and inspected before any upload layer is involved.

This is a compiler artifact, not a flight run:

- no hardware upload;
- no serial, UDP, TCP, or radio transport;
- no PX4-only mode sequence;
- no ArduPilot-only mode sequence;
- no `MISSION_ITEM_INT` byte/message serialization.

PX4/ArduPilot semantics are not identical even when both stacks expose MAVLink
Common commands. M81 still emits generic Common command intent; M82 adds a
separate capability/profile pass that annotates the plan with
`MavlinkCompatibilityReport` for `mavlink_common_generic`, `px4`, or
`ardupilot`. The profile pass does not silently rewrite mission semantics.

## Output Shape

`MavlinkCommonPlan` uses schema version `mavlink_common_plan.v1` and contains:

- `source_mission_id`;
- deterministic `command_ir_hash` using SHA-256 over canonical
  `MissionCommandPlan` JSON with a versioned digest domain;
- `command_prelude` for `COMMAND_LONG`-style commands that run before mission
  upload/start;
- ordered `mission_items` for upload-style navigation commands;
- optional `mission_start`;
- `command_postlude` for `COMMAND_LONG`-style commands that run after uploaded
  mission execution completes;
- `expected_acks` / expected ACKs;
- `telemetry_milestones`;
- `unsupported_features`;
- `validation_result`;
- optional M82 `compatibility` report with per-command classifications,
  `required_execution_mode`, `required_mode_transitions`, preconditions and
  caveats.

Dry-run artifacts from `sitl_agent --dry-run --dry-run-artifact <path>` include
this plan as optional `mavlink_common_plan` next to the existing
`command_ir_summary`.

## Execution Order

`MavlinkCommonPlan` is an ordered phase artifact. A future executor must apply
the phases in this order:

1. send `command_prelude`;
2. upload `mission_items` when present;
3. send `mission_start` when present;
4. monitor uploaded mission progress until mission item execution completes;
5. send `command_postlude`.

For direct command-only plans with no `mission_items`, lifecycle commands such
as `land` or `return_to_launch` can remain in `command_prelude` because no
upload/start phase exists. For mixed plans such as `takeoff -> go_to/hold ->
land`, post-route lifecycle commands are emitted in `command_postlude`, not in
`command_prelude`.

## Supported Common Commands

| IR command | M81 output |
|---|---|
| `arm` / `disarm` | `MAV_CMD_COMPONENT_ARM_DISARM` command prelude |
| `takeoff` | `MAV_CMD_NAV_TAKEOFF` command prelude |
| `land` | `MAV_CMD_NAV_LAND` command prelude for command-only plans, or command postlude after uploaded mission items |
| `return_to_launch` / `abort` | `MAV_CMD_NAV_RETURN_TO_LAUNCH` command prelude for command-only plans, or command postlude after uploaded mission items |
| `go_to` | `MAV_CMD_NAV_WAYPOINT` mission item |
| `follow_route` | ordered `MAV_CMD_NAV_WAYPOINT` mission items |
| `hold` | `MAV_CMD_NAV_LOITER_TIME` mission item when an anchor position exists |
| `loiter_time` | `MAV_CMD_NAV_LOITER_TIME` mission item when an anchor position exists |
| `orbit` | structured unsupported feature by default, or deterministic waypoint approximation when enabled |
| `pause` / `resume` | structured unsupported features in M81 |

`Hold` and `LoiterTime` have duration-only IR semantics. The compiler only maps
them to `MAV_CMD_NAV_LOITER_TIME` when it can resolve a position from the last
compiled waypoint or `MavlinkCommonPlanOptions.default_hold_position`. It does
not invent a fake coordinate.

## Dry-Run Validation

M83 adds three primitive real command missions that use the same dry-run
compiler boundary:

- `scenarios/primitive.takeoff-hold-land.json`;
- `scenarios/primitive.orbit.json`;
- `scenarios/primitive.square.json`.

They compile to `MavlinkCommonPlan` with command lifecycle ordering, expected
ACKs, telemetry milestones, timeout/abort policy in `command_ir_summary`, and
the embedded static `safety_report`. Orbit is represented through the configured
waypoint approximation unless a later profile-specific executor supplies a
native mapping. This is no real flight and no hardware upload.

`artifact_validator --mode dry-run` looks for
`sitl_dry_run_artifact.v1.json` or legacy `dry-run.json` and checks the M81
section plus the M82 compatibility report for current artifacts:

- `artifact.mavlink_plan_missing`;
- `artifact.mavlink_plan_schema_unsupported`;
- `artifact.mavlink_plan_command_missing`;
- `artifact.mavlink_plan_ack_missing`;
- `artifact.mavlink_plan_telemetry_missing`;
- `artifact.mavlink_plan_order_unsafe`;
- `artifact.dry_run_policy_missing`;
- `artifact.dry_run_safety_report_failed`;
- `artifact.mavlink_plan_unsupported_required`;
- `artifact.mavlink_plan_ir_hash_missing`;
- `artifact.mavlink_profile_missing`;
- `artifact.mavlink_profile_unknown`;
- `artifact.mavlink_profile_unsupported`;
- `artifact.mavlink_profile_hardware_blocking`;
- `artifact.mavlink_profile_result_mismatch`.

These checks validate artifact consistency only. They do not prove PX4, Gazebo,
HIL, hardware, or production readiness.

See [`docs/MAVLINK_CAPABILITY_PROFILES.md`](MAVLINK_CAPABILITY_PROFILES.md) for
the M82 compatibility classes, compatibility matrix, and profile caveats.

## Example

```bash
cargo run --bin sitl_agent -- \
  --dry-run \
  --scenario scenarios/urban.patrol.json \
  --agent-id agent-0 \
  --dry-run-artifact target/m81-dry-run/sitl_dry_run_artifact.v1.json \
  --mavlink-profile px4

cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir target/m81-dry-run \
  --mode dry-run \
  --strict
```

The first command writes a portable artifact. The second command validates the
dry-run artifact without starting PX4 or touching hardware.
