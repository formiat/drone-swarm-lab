# MAVLink Capability Profiles

**M82 - PX4 / ArduPilot Capability Profiles**

M86 extends these profiles with FC geofence and parameter contract metadata. See
[`docs/FC_CONTRACT.md`](FC_CONTRACT.md) for the fence/parameter artifact layer.

M82 keeps the M81 MAVLink Common compiler as a generic, transport-free compiler
and adds a conservative profile pass on top of the compiled
`MavlinkCommonPlan`. The pass writes a `MavlinkCompatibilityReport` into
current dry-run artifacts.

The key question is not only "does MAVLink Common define this command?" The
profile pass records:

- selected profile: `mavlink_common_generic`, `px4`, or `ardupilot`;
- command support classification;
- coordinate frame support;
- `required_execution_mode`;
- `required_mode_transitions`;
- preconditions and `mode_caveats`;
- mission-start semantics;
- takeoff/landing caveats;
- loiter/orbit fallback caveats;
- geofence and parameter support status.

This is no exhaustive autopilot certification, not vendor SDK integration, and
not hardware readiness. It is an artifact honesty layer:
dry-run artifacts can show where a plan is syntactically Common, where PX4 has
local SIH evidence, and where ArduPilot remains
`unknown_until_sitl_or_hardware`.

## Compatibility Classes

| Class | Meaning |
|---|---|
| `supported` | Supported by the selected profile without profile-specific caveats. |
| `supported_with_caveats` | Supported enough to describe, but caveats must remain visible in artifacts and docs. |
| `requires_stack_specific_mapping` | MAVLink Common syntax is not enough; a stack-specific mapping is required before claiming support. |
| `supported_via_fallback` | The compiler used an explicit fallback, such as waypoint approximation for orbit intent. |
| `unsupported` | The selected profile does not support this command/frame/behavior. |
| `unknown_until_sitl_or_hardware` | The repository does not yet have SITL or hardware evidence for this profile behavior. |

`unknown_until_sitl_or_hardware` can be valid for dry-run artifacts when it is
visible and `hardware_facing_allowed` is false. It must not be silently treated
as hardware-facing success.

## Artifact Shape

`MavlinkCommonPlan` now includes:

- `backend_profile`: string label matching the selected profile id;
- `compatibility.profile`;
- `compatibility.overall_classification`;
- `compatibility.hardware_facing_allowed`;
- `compatibility.command_results[]`;
- `compatibility.aggregate_mode_requirements[]`;
- `compatibility.caveats[]`.
- optional M86 fields: `geofence_prelude`, `fence_summary`, and
  `fc_contract_result`.

Each `command_results[]` row records command/frame classification plus
`required_execution_mode`, `required_mode_transitions`, `preconditions`, and
`mode_caveats` when the selected profile has mode assumptions for that command.
Dry-run artifact validation also checks that every row still matches the
compiled plan element identity (`command_id`, `seq`, `command`, `phase`,
`frame`), so stale compatibility reports cannot be accepted only because their
row count still matches.

## Compatibility Matrix

The compatibility matrix below is checked against `compatibility_matrix_rows()` in
`swarm-comms`; the stable row key must stay synchronized with profile data.

| Row key | Profile | Dimension | Classification | Summary |
|---|---|---|---|---|
| `mavlink_common_generic:command_support` | `mavlink_common_generic` | command_support | `supported_with_caveats` | Common commands are syntax-level only. |
| `mavlink_common_generic:frame_support` | `mavlink_common_generic` | frame_support | `supported_with_caveats` | Uses `MAV_FRAME_GLOBAL_RELATIVE_ALT_INT`. |
| `mavlink_common_generic:mode_transitions` | `mavlink_common_generic` | mode_transitions | `unknown_until_sitl_or_hardware` | No concrete autopilot mode proof. |
| `mavlink_common_generic:mission_start` | `mavlink_common_generic` | mission_start | `supported_with_caveats` | `MAV_CMD_MISSION_START` syntax only. |
| `mavlink_common_generic:loiter_orbit` | `mavlink_common_generic` | loiter_orbit | `supported_via_fallback` | Loiter time and waypoint orbit fallback only. |
| `mavlink_common_generic:geofence` | `mavlink_common_generic` | geofence | `supported` | Common fence mission item syntax is emitted; autopilot acceptance is not implied. |
| `mavlink_common_generic:parameters` | `mavlink_common_generic` | parameters | `unknown_until_sitl_or_hardware` | No autopilot metadata validation. |
| `px4:command_support` | `px4` | command_support | `supported_with_caveats` | Core primitive commands have local PX4/SIH evidence. |
| `px4:frame_support` | `px4` | frame_support | `supported_with_caveats` | Uses `MAV_FRAME_GLOBAL_RELATIVE_ALT_INT` in local SIH evidence. |
| `px4:mode_transitions` | `px4` | mode_transitions | `supported_with_caveats` | Heartbeat, arm, takeoff, upload and mission-start assumptions are explicit. |
| `px4:mission_start` | `px4` | mission_start | `supported_with_caveats` | `MAV_CMD_MISSION_START` is the current PX4 path. |
| `px4:loiter_orbit` | `px4` | loiter_orbit | `supported_via_fallback` | Orbit is waypoint approximation; direct orbit is not claimed. |
| `px4:geofence` | `px4` | geofence | `supported_with_caveats` | Polygon fence items are modeled with PX4 caveats; circle fence remains unknown. |
| `px4:parameters` | `px4` | parameters | `supported_with_caveats` | Only emitted primitive parameters are covered. |
| `ardupilot:command_support` | `ardupilot` | command_support | `unknown_until_sitl_or_hardware` | Common syntax is known; acceptance is not evidenced. |
| `ardupilot:frame_support` | `ardupilot` | frame_support | `unknown_until_sitl_or_hardware` | Frame syntax is known; ArduPilot acceptance needs SITL evidence. |
| `ardupilot:mode_transitions` | `ardupilot` | mode_transitions | `unknown_until_sitl_or_hardware` | Mode mapping requires ArduPilot SITL evidence. |
| `ardupilot:mission_start` | `ardupilot` | mission_start | `unknown_until_sitl_or_hardware` | Mission start semantics are not claimed yet. |
| `ardupilot:loiter_orbit` | `ardupilot` | loiter_orbit | `unknown_until_sitl_or_hardware` | Loiter/orbit acceptance is not evidenced. |
| `ardupilot:geofence` | `ardupilot` | geofence | `unknown_until_sitl_or_hardware` | ArduPilot fence acceptance needs ArduPilot SITL evidence. |
| `ardupilot:parameters` | `ardupilot` | parameters | `unknown_until_sitl_or_hardware` | No ArduPilot parameter metadata validation. |

## M86 FC Contract Additions

Profiles now include per-kind fence rules for circle inclusion/exclusion and
polygon inclusion/exclusion. `compile_mavlink_common_plan` uses those rules when
`MavlinkCommonPlanOptions.fence_plan` is provided:

- unsupported or unknown fence kinds fail compilation before the plan can be
  treated as valid;
- a successful fence plan emits `geofence_prelude` and optional
  `MAV_CMD_DO_FENCE_ENABLE`;
- `fc_contract_result` records whether fence and optional parameter checks block
  mission start.

This remains a dry-run contract. The live PX4/SIH path does not yet upload M86
fence items or read/write FC parameters.

## CLI

Dry-run artifacts can select the profile:

```bash
cargo run --bin sitl_agent -- \
  --dry-run \
  --scenario scenarios/urban.patrol.json \
  --agent-id agent-0 \
  --dry-run-artifact target/m82-dry-run/sitl_dry_run_artifact.v1.json \
  --mavlink-profile px4
```

Supported values are:

- `mavlink_common_generic`;
- `px4`;
- `ardupilot`.

M82 does not wire profile selection into live MAVLink upload. The live PX4/SIH
path remains a separate experimental workflow.

## M83 Primitive Mission Profiles

M83 primitive dry-run artifacts can be generated with any supported
`--mavlink-profile` value:

```bash
cargo run --bin sitl_agent -- \
  --dry-run \
  --scenario scenarios/primitive.square.json \
  --agent-id agent-0 \
  --dry-run-artifact target/m83-square/sitl_dry_run_artifact.v1.json \
  --mavlink-profile px4
```

The profile report classifies the compiled command sequence for
`takeoff-hold-land`, `orbit`, and `waypoint-square`. Orbit portability remains
explicitly caveated: M83 may use waypoint approximation, and native orbit
behavior is profile-specific or unknown without SITL/hardware evidence.
Landing completion can also be stack-specific. M83 validates artifacts only; it
does not claim PX4 and ArduPilot behave identically, does not connect to a
vehicle, and is not exhaustive autopilot certification.
