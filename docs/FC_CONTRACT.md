# FC Contract

**M86 - MAVLink Safety / FC Contract**

M86 adds a transport-free flight-controller contract layer on top of the M81
MAVLink Common compiler and the M82 capability profiles. It produces dry-run
artifact evidence for two safety-adjacent concerns:

- FC geofence intent: `MavlinkFencePlan` compiles polygon/circle fence items
  into `MavlinkCommonPlan.geofence_prelude`.
- FC parameter requirements: `FcParamRequirement` and `FcParamSnapshot` validate
  known PX4/ArduPilot parameter expectations when a snapshot is available.

This is not hardware upload, not certified geofencing, and not proof that PX4 or
ArduPilot will accept every emitted item. It is a structured contract artifact
that makes the intended FC boundary explicit before any future live integration.

## Artifact Fields

`MavlinkCommonPlan` can now include:

| Field | Meaning |
|---|---|
| `geofence_prelude` | MAVLink Common fence mission items that must be uploaded before the mission body. |
| `fence_summary` | Item counts, shape summary, aggregate profile classification, and caveats. |
| `fc_contract_result` | Validation result for fence support and optional parameter snapshot checks. |

`fc_contract_result.blocks_mission_start=true` makes
`validation_result.passed=false`. This prevents a dry-run artifact from looking
hardware-ready when the selected profile classifies a fence kind as
`unknown_until_sitl_or_hardware`, `unsupported`, or
`requires_stack_specific_mapping`, or when supplied parameter values violate the
declared requirements.

## Fence Items

M86 models these MAVLink Common fence commands:

| Fence kind | Command |
|---|---|
| `circle_inclusion` | `MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION` |
| `circle_exclusion` | `MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION` |
| `polygon_inclusion` | `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION` |
| `polygon_exclusion` | `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION` |
| `enable_fence=true` | `MAV_CMD_DO_FENCE_ENABLE` in `command_prelude` |

Polygon fences emit one mission item per vertex. Each vertex item stores the
polygon vertex count in `params[0]`, uses `MAV_FRAME_GLOBAL`, and carries WGS84
`lat_e7` / `lon_e7` coordinates. The compiler rejects polygons with fewer than
three vertices or more than the MAVLink Common polygon vertex limit.

## Profiles

The generic profile is syntax-level only. PX4 currently allows polygon fence
items with explicit caveats and treats circle fence items as unknown until SITL
or hardware evidence exists. ArduPilot fence item acceptance remains unknown
until ArduPilot SITL or hardware evidence is captured.

The profile rule is intentionally conservative: dry-run artifacts can describe
intent, but hardware-facing success requires profile support that does not block
mission start.

## Parameters

`swarm-comms` exposes:

- `FcParamId`
- `FcParamValue`
- `FcParamRange`
- `FcParamRequirement`
- `FcParamSnapshot`
- `FcParamReadPlan`
- `FcParamWritePlan`
- `FC_KNOWN_PARAMS_PX4`
- `FC_KNOWN_PARAMS_ARDUPILOT`

Known parameter metadata is a starting catalog, not a complete vendor database.
PX4 entries include `GF_ACTION`, `GF_MAX_HOR_DIST`, `COM_ARM_WO_GPS`, and
`EKF2_AID_MASK`. ArduPilot entries include `FENCE_ACTION`, `FENCE_ALT_MAX`, and
`FENCE_RADIUS`.

When no snapshot is provided, parameter requirements are recorded but not
treated as violations. Once a snapshot is available, missing or out-of-range
required parameters block the FC contract.

## SafetyConfig Bridge

`swarm-sim::safety_config_to_fence_plan` converts the simulation-side
`SafetyConfig` AABB model into an FC fence plan:

- `SafetyConfig.geofence` becomes a polygon inclusion item.
- each `no_fly_zones[]` entry becomes a polygon exclusion item.
- local AABB corners are converted to MAVLink global-int coordinates using a
  supplied `MavlinkCoordinateOrigin`.

The bridge preserves crate boundaries: `swarm-comms` remains independent of
`swarm-safety`, and the conversion lives in `swarm-sim`.

## Non-Goals

- No MAVLink message transport for fence upload.
- No live FC parameter read/write implementation.
- No PX4/ArduPilot equivalence claim.
- No hardware readiness or certified geofence behavior.
- No dynamic obstacle avoidance or runtime sensor validation.
