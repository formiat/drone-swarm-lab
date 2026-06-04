# Artifact Validation

M72 adds a machine-checkable evidence contract for local SITL artifacts. It is
intended for simulation, dry-run, and local PX4/SIH research packs before any
future hardware-candidate work. It does not certify hardware safety and does not
run PX4/SIH in CI.

## Artifact Pack Layout

A supervisor output directory is the child directory created by
`sitl_supervisor --output-dir <root> --run-id <id>`:

```text
<root>/<run-id>/
  manifest.json
  events.sitl-log.json
  run-report.json
  replay-summary.txt
  safety_validation_report.v1.json
  scenario.snapshot.json
  config.snapshot.json
  command.txt
```

`manifest.json` uses `multi_sitl_manifest.v1` and includes
`artifact_metadata` with the command line, git commit, build profile, run id,
snapshot paths, and command capture path. Old committed M58/M59 artifacts may
lack that metadata; validate them with `--allow-historical` or `--mode
historical`.

For dry-run validation, the output directory may contain:

```text
<output-dir>/
  sitl_dry_run_artifact.v1.json
```

Legacy/test packs may use `dry-run.json`. In `--mode dry-run`,
`artifact_validator` validates the dry-run artifact and its M81
`mavlink_common_plan` section instead of requiring `manifest.json`.

## Validator CLI

Validate a current supervisor pack:

```bash
cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir target/sitl/local-multi-agent-sih \
  --mode supervisor-run \
  --strict
```

Validate an old committed evidence pack without treating missing M72 metadata as
a hard failure:

```bash
cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir results/m59_px4_sih_failure_reallocation_2026-05-31/m59-px4-sih-failure-reallocation \
  --mode historical \
  --allow-historical
```

Use `--json` to print `artifact_validation_report.v1`.

Validate a dry-run M81 compiler artifact. This is the
`artifact_validator --mode dry-run` path:

```bash
cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir target/m81-dry-run \
  --mode dry-run \
  --strict
```

Exit codes:

| Code | Meaning |
|---|---|
| `0` | Artifact pack passed validation. |
| `2` | Artifact pack was readable, but one or more validation rules failed. |
| `3` | CLI usage error. |

## Rule IDs

| Rule ID | Meaning |
|---|---|
| `artifact.manifest_missing` | `manifest.json` is absent. |
| `artifact.manifest_schema_unsupported` | Manifest schema is not `multi_sitl_manifest.v1`. |
| `artifact.manifest_command_missing` | Manifest metadata has no captured command. |
| `artifact.git_commit_missing` | Manifest metadata has no usable git commit. |
| `artifact.build_profile_missing` | Manifest metadata has no build profile. |
| `artifact.run_id_mismatch` | Manifest, report, and event log run ids disagree. |
| `artifact.output_dir_mismatch` | Output directory basename does not match report run id. |
| `artifact.final_status_mismatch` | Report final status and event-log final status disagree. |
| `artifact.completed_task_missing_event` | Report completed-task count or task ids do not match completion events. |
| `artifact.replay_summary_count_mismatch` | Report/replay summary counters do not match the event log. |
| `artifact.replacement_seq_mismatch` | Completion event seq does not match the active mission item seq. |
| `artifact.safety_report_missing` | Supervisor pack has no `safety_validation_report.v1.json`. |
| `artifact.limitations_missing` | Connection execute report lacks limitations. |
| `artifact.overwrite_policy_missing` | Manifest metadata does not identify captured command/output policy metadata. |
| `artifact.degraded_record_missing` | Failed/reallocated current supervisor pack has no degraded records. |
| `artifact.degraded_event_missing` | Degraded record has no matching failure detected/classified replay events. |
| `artifact.degraded_final_status_mismatch` | Degraded record final status is inconsistent with the run report. |
| `artifact.degraded_recovery_task_mismatch` | Recovered tasks in the report are missing from recovery replay events. |
| `artifact.degraded_unsupported_path_unlabeled` | Current degraded record uses `unknown` without historical mode. |
| `artifact.mavlink_plan_missing` | Dry-run artifact or its `mavlink_common_plan` section is absent. |
| `artifact.mavlink_plan_schema_unsupported` | Dry-run or `MavlinkCommonPlan` schema is unsupported. |
| `artifact.mavlink_plan_command_missing` | The M81 plan has no commands/items, or mission item sequences are not contiguous. |
| `artifact.mavlink_plan_ack_missing` | Expected ACK coverage is incomplete for commands, mission upload, or mission start. |
| `artifact.mavlink_plan_unsupported_required` | Required unsupported features are present while `validation_result.passed` is still true. |
| `artifact.mavlink_plan_ir_hash_missing` | `command_ir_hash` is absent or empty. |
| `artifact.parse_failed` | A required artifact could not be read or parsed. |

## Local Harness

The harness scripts are manual-only helpers:

```bash
DRY_RUN=1 scripts/run_m58_local.sh
DRY_RUN=1 scripts/run_m59_local.sh
```

Live mode requires local PX4/SIH setup and explicit launch commands:

```bash
PX4_AGENT0_CMD='...' PX4_AGENT1_CMD='...' scripts/run_m58_local.sh
PX4_AGENT0_CMD='...' PX4_AGENT1_CMD='...' scripts/run_m59_local.sh
```

The scripts start only commands supplied by the operator, track only their own
PIDs, write deterministic local result directories, and call
`artifact_validator` after the supervisor run. Missing PX4/SIH configuration is
reported as an actionable local setup error.

## Manual Boundary

M72 validates artifact consistency. It does not provide automated PX4 CI,
Gazebo/HIL coverage, physical hardware readiness, production failover, runtime
obstacle avoidance, or flight certification. Live M58/M59 harness runs must stay
operator-controlled and outside default CI.

M73 adds degraded-supervisor validation for failed/reallocated packs. It checks
that `run-report.json.degraded` records have matching degraded replay events and
that recovered tasks are present in `supervisor_recovery_completed`. Historical
M58/M59 evidence can still be checked with `--allow-historical` when it lacks
M73 fields.
