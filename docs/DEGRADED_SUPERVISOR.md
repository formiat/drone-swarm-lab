# Degraded Supervisor

M73 adds a pre-hardware degraded-supervisor contract for the local SITL
supervisor:

```text
detect -> classify -> decide -> recover/abort -> report
```

This is a structured reporting and fake-tested recovery boundary. It does not
claim hardware failure testing, real RF/link-loss modeling, Gazebo/HIL coverage,
or production failover.

## Failure Matrix

| Failure mode | Detection source | Decision | Recovery attempt | Final status | Automated coverage | Manual/SITL status |
|---|---|---|---|---|---|---|
| `agent_lost_before_upload` | fake pre-upload loss or live connection/open failure fallback | `mark_total_failure` | none | `failed` / `partial_failed` at run level | fake controller test | live/manual not claimed |
| `upload_rejected` | fake upload rejection or live MAVLink upload rejection fallback | `mark_total_failure` | none | `failed` / `partial_failed` at run level | fake controller test | optional local PX4/SITL check only |
| `agent_lost_after_upload_before_mission_start` | fake post-upload/pre-start loss or live heartbeat-before-start fallback | `mark_total_failure` | none | `failed` / `partial_failed` at run level | fake controller test | optional local PX4/SITL check only |
| `no_progress_timeout` | fake no-progress timeout or live no-progress timeout string fallback | `wait`, then `abort` / terminal mark | none unless future policy enables recovery | `failed` / `partial_failed` | fake controller test | no RF modeling claim |
| `heartbeat_lost` | fake active-controller heartbeat loss | `continue_with_survivor`, plus `release_tasks_to_pool` and `reassign_unfinished_tasks` counters | one bounded survivor replacement | `completed_with_reallocation` or degraded terminal status | fake controller test | local PX4/SIH remains manual |
| `stale_telemetry` | fake stale telemetry / no task progress | `wait`, then terminal mark or recovery path | bounded by configured path | `failed` / `partial_failed` | fake controller test | no RF modeling claim |
| `partial_completion_then_failure` | fake strict subset completion followed by failure | `mark_partial_success` or survivor continuation | one bounded survivor replacement when available | `partial_failed` or `completed_with_reallocation` | fake controller test | local PX4/SIH remains manual |
| `replacement_mission_rejected` | fake survivor replacement rejection or live replacement upload/execute failure fallback | `abort` / `mark_partial_success` | recovery fails once | `failed_recovery` record; run may be `partial_failed` | fake controller test | local PX4/SIH remains manual |
| `survivor_failed_after_replacement` | fake survivor accepts replacement then fails | `mark_partial_success` | no recursive recovery in M73 | `partial_failed` / `failed_recovery` | fake controller test | repeated recovery is future work |
| `unsafe_replacement_route` | M71 safety gate rejects replacement task subset | `refuse_unsafe_replacement` | no upload | `failed_recovery` record | fake/report contract test | not certified safety |
| `bad_waypoint_or_mission_item` | fake/report planning failure or invalid replacement waypoint/item | `mark_total_failure` / `abort` | none | `failed` / `failed_recovery` | fake controller test | live/PX4 version not tested |

## Supported In Fake Tests

The supported M73 scope is deterministic fake-controller coverage. Tests assert:

- `report.degraded.records`;
- `report.degraded.failure_mode_counts`;
- `report.degraded.decision_counts`;
- `report.degraded.tasks_abandoned`;
- `report.degraded.recovery_failed_count`;
- degraded replay event counters;
- M72 `artifact_validator` consistency rules for degraded packs.

## Experimental Local SITL

Local PX4/SIH supervisor runs may emit the same additive report and replay
schema, but M73 does not make them automated evidence by itself. Any local
artifact cited as evidence must be validated with:

```bash
artifact_validator --output-dir <pack> --mode supervisor-run --strict
```

Historical M58/M59 packs can still be checked with `--allow-historical` when
they lack M73 degraded fields.

## Not Tested / Non-Goals

- No hardware failure testing.
- No real RF/link-loss modeling.
- No Gazebo/HIL coverage.
- No repeated-failure recursive recovery.
- No production safety or flight certification claim.
- No broad PX4 failsafe matrix.

## Recovery Semantics

M73 performs one bounded recovery attempt. If a failed agent has unfinished
tasks and an active survivor exists behind `--reupload-on-failure`, the
supervisor can release unfinished tasks, compute a replacement mission, validate
the replacement task subset through the M71 safety gate, and upload the
replacement to the survivor. If replacement is rejected or unsafe, recovery is
reported as failed; the supervisor does not recursively search for another
survivor in M73.

## Report Fields

`sitl_multi_agent_run_report.v1` remains additive and backward-compatible.
`run-report.json.degraded` contains:

- `records`;
- `failure_mode_counts`;
- `decision_counts`;
- `tasks_abandoned`;
- `recovery_failed_count`.

Per-agent report entries also include optional `failure_mode` and
`tasks_abandoned`.

## Replay Events

M73 adds these SITL event types:

- `supervisor_failure_detected`;
- `supervisor_failure_classified`;
- `supervisor_recovery_started`;
- `supervisor_replacement_uploaded`;
- `supervisor_recovery_completed`;
- `supervisor_recovery_failed`;
- `supervisor_final_status`.

The replay summary includes counters for each event class.

## Manual Checks

Manual/local PX4/SIH fault-injection checks remain optional. If captured, store
the full supervisor output directory and validate it with M72/M73 artifact
validation before citing it as evidence.
