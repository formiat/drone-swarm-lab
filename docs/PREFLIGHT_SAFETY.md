# Preflight Safety

M71 adds a static preflight gate for mission inputs before dry-run, SITL upload,
or hardware-candidate experiments. It is a correctness and safety-input gate,
not certified flight safety.

## Preflight Rules

| Rule ID | Severity | Description |
|---|---|---|
| `geofence.waypoint_outside` | error | A task waypoint is outside `SafetyConfig.geofence`. |
| `nofly.waypoint_inside` | error | A task waypoint is inside an active no-fly AABB. |
| `altitude.above_max` | error | A task waypoint is above `SafetyConfig.max_altitude_m`. |
| `altitude.below_min` | warning | A task waypoint is below `SafetyConfig.min_altitude_m`. |
| `route.length_exceeds_max` | error | An Urban route exceeds `SafetyConfig.max_route_length_m`. |
| `route.duration_exceeds_max` | warning | `run_config.max_ticks * tick_duration_ms` exceeds `SafetyConfig.max_duration_ticks` seconds. |
| `pose.invalid_coordinate` | error | A task waypoint has non-finite coordinates. |
| `id.missing_task_id` | error | A task id is empty. |
| `ownership.duplicate_task_id` | error | Scenario tasks contain duplicate task ids. |
| `ownership.task_assigned_and_unassigned` | error | A task is `Unassigned` while `assigned_to` is set. |
| `urban.unknown_edge` | error | A planned Urban route references an unknown edge. |
| `urban.blocked_edge` | error | A planned Urban route uses a blocked edge. |
| `urban.aabb_intersection` | error | A planned Urban waypoint intersects a static obstacle AABB. |
| `urban.waypoint_outside_assumptions` | warning | A planned Urban waypoint is outside nominal map bounds. |
| `urban.invalid_temporary_obstacle` | error | A `temporary_obstacles` entry references an unknown edge or has `appears_at_tick >= disappears_at_tick`. |
| `semantics.unsupported_strategy_pair` | warning | A known weak mission/strategy pair is requested. |

## Reports

Preflight returns `SafetyValidationReport`:

- `passed=true` only when there are no `error` violations.
- Each violation carries `rule_id`, `severity`, optional `affected_id`, and
  `reason`.
- `sitl_agent --dry-run --dry-run-artifact` includes the report in
  `sitl_dry_run_artifact.v1`.
- M83 primitive real command artifacts (`takeoff-hold-land`, `orbit`, and
  `waypoint-square`) are expected to carry `safety_report.passed=true` before
  their `MissionCommandPlan` / `MavlinkCommonPlan` evidence is accepted.
- `sitl_supervisor --output-dir` writes `safety_validation_report.v1.json`.
- M72 `artifact_validator --mode supervisor-run` requires
  `safety_validation_report.v1.json` in supervisor packs and reports
  `artifact.safety_report_missing` when it is absent. See
  [`docs/ARTIFACT_VALIDATION.md`](ARTIFACT_VALIDATION.md).

## M86 FC Contract Bridge

M71 preflight is a simulation/input gate. M86 adds a separate FC-facing contract
layer for dry-run artifacts:

- `swarm-sim::safety_config_to_fence_plan` converts `SafetyConfig.geofence`
  AABB bounds into a MAVLink polygon inclusion fence.
- Each `SafetyConfig.no_fly_zones[]` AABB converts into a MAVLink polygon
  exclusion fence.
- `MavlinkCommonPlanOptions.fence_plan` compiles those items into
  `MavlinkCommonPlan.geofence_prelude`.
- `fc_contract_result` records whether the selected MAVLink capability profile
  and optional FC parameter snapshot allow the mission to start.

Passing M71 preflight does not mean an FC geofence was uploaded. Passing M86 FC
contract validation means only that the dry-run artifact is internally
consistent for the selected profile and parameter snapshot. Neither layer is
certified flight safety or hardware validation.

## Exit Code Convention

| Code | Category |
|---|---|
| `2` | validation / preflight |
| `3` | runtime / supervisor |
| `4` | artifact / report |
| `5` | environment / feature / hardware-candidate boundary |

## What Is Not Checked

- Runtime obstacle avoidance.
- Real sensor data.
- Hardware failsafe behaviour.
- Released task history after runtime reallocation.

M73 uses this same static gate for replacement missions. If a survivor
replacement route is rejected by preflight, the supervisor records
`unsafe_replacement_route` with decision `refuse_unsafe_replacement`; this is
evidence of refused recovery, not certified flight safety.

M83 uses this same static preflight gate for primitive command missions. Passing
M83 preflight means only that the scenario inputs and generated dry-run artifact
passed the repository's static validation. It does not certify flight safety,
does not validate dynamic obstacle avoidance, and does not replace PX4 or
ArduPilot failsafes.
- Full support matrix as machine-readable policy.
- Regulatory compliance.

## Not Certified Flight Safety

This project is not a certified flight-safety system. M71 rejects obviously
unsafe or inconsistent mission inputs, but it does not replace PX4 failsafes,
operator procedures, regulatory review, physical kill switches, or hardware
validation.

## Non-Goals

- No real hardware readiness claim.
- No real sensor/perception validation.
- No runtime obstacle avoidance.
- No publication benchmark refresh.
