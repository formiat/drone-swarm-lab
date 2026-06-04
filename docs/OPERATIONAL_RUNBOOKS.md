# Operational Runbooks And Hardware Entry Gate

M79 defines the operational boundary before any future hardware experiment.
This is a procedural readiness layer, not a hardware readiness claim. The
project remains a research prototype.

Required boundary phrases:

- first hardware experiment is still not product readiness;
- multi-agent hardware requires separate safety review;
- no regulatory or certified safety claim.

Use these runbooks in order. Do not skip directly to a hardware-candidate
connection because a later step looks familiar.

## Scope And Non-Goals

This document covers:

- simulation runbook;
- Urban scenario runbook;
- SITL dry-run/export runbook;
- local PX4/SIH runbook;
- artifact validation runbook;
- future hardware candidate runbook;
- preflight checklist;
- go/no-go gates;
- post-run inspection.

This document does not provide legal review, regulatory certification,
airframe-specific failsafe tuning, pilot training, production flight workflow,
or a complete hardware checklist without real hardware.

## Required Preflight Checklist

Record every item before a run that may later be cited as evidence:

- mission file validated;
- safety report passed with no error-severity rule ids;
- artifact output directory is unique, or overwrite is explicit through
  `--force`;
- `geo_origin` and coordinate-frame assumptions recorded;
- geofence/no-fly assumptions recorded;
- expected failure behavior recorded;
- manual override assumption recorded;
- operator knows whether the connection is `mock`, `dry-run`,
  `local_px4_sitl_udp`, or `hardware_candidate`;
- known limitations recorded before execution.

## Go/No-Go Gates

These gates are explicit, not best effort:

- no hardware if simulation fails;
- no hardware if SITL dry-run/export fails;
- no hardware if preflight safety fails;
- no hardware if artifact validator fails;
- no hardware if mission has unclassified safety violations;
- no hardware without external safety process;
- no multi-drone hardware before separate single-drone review;
- no public product-readiness claim after a first hardware experiment;
- no regulatory or certified safety claim from this repository.

## Runbook 1: Simulation

Use simulation before any SITL or hardware-candidate path.

```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --mission urban \
  --smoke \
  --output-dir target/m79-simulation
```

Expected artifacts:

- benchmark pack in `target/m79-simulation/`;
- `manifest.json`;
- `results.json`;
- `results.csv`;
- `table.md`.

Stop/abort conditions:

- command exits non-zero;
- output directory cannot be written;
- report has unsupported rows that are not understood for the intended claim;
- simulation result contradicts the expected mission behavior.

## Runbook 2: Urban Scenario

Use this when the candidate mission is Urban-specific.

```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --scenario-suite scenarios/urban.corridor-delta.json \
  --output-dir target/m79-urban
```

Expected artifacts:

- route trace artifact when replay/export is enabled by the suite;
- judge report for Urban route violations when available;
- benchmark/report outputs that identify `urban-patrol` or `urban-search`.

Stop/abort conditions:

- Urban preflight reports `urban.blocked_edge` or another error-severity rule;
- route is not planned;
- route risk or violation count is inconsistent with the intended test;
- mocked bus/perimeter semantics are being interpreted as real perception.

## Runbook 3: SITL Dry-Run / Export

Use dry-run before any connection mode.

```bash
cargo run -p swarm-examples --bin sitl_agent -- \
  --dry-run \
  --scenario scenarios/urban.patrol.json \
  --agent-id agent-0 \
  --dry-run-artifact target/m79-dry-run/sitl_dry_run_artifact.v1.json
```

Expected artifacts:

- printed waypoint plan;
- `sitl_dry_run_artifact.v1.json`;
- embedded `SafetyValidationReport`;
- explicit `geo_origin`, coordinate frame, route length, waypoint count, and
  altitude assumptions.

Stop/abort conditions:

- dry-run exits non-zero;
- safety report has error-severity rule ids;
- waypoint count, altitude, coordinate frame, or route identity is unexpected;
- artifact path cannot be written.

## Runbook 3a: M84 Urban Geo Dry-Run Pack

Use these fixtures when the claim is about Urban WGS84 route export or mission
template metadata:

```bash
cargo run -p swarm-examples --bin sitl_agent -- \
  --dry-run \
  --scenario scenarios/urban.geo-block-loop.json \
  --agent-id agent-0 \
  --dry-run-artifact target/m84-geo-block/sitl_dry_run_artifact.v1.json

cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir target/m84-geo-block \
  --mode dry-run \
  --strict
```

Repeat the dry-run/validator pair for:

- `urban.geo-block-loop.json`;
- `urban.geo-search-bus.json`;
- `urban.geo-inspection-corridor.json`.

`scenarios/fixtures/urban_small_block.geojson` is the small GeoJSON importer
fixture. It is not a full OSM parser. The `urban.geo-search-bus.json` fixture
uses a deterministic mocked detector; it is not real perception. M84 is not certified collision avoidance, not PX4 execution evidence, not Gazebo/HIL, and
not hardware readiness.

Expected evidence:

- `coordinate_mode: wgs84_node_geo`;
- `waypoints[].geo` on every exported waypoint;
- MAVLink `mission_items[].lat_e7`, `lon_e7`, and `relative_alt_m` matching
  the exported waypoint geo metadata;
- `urban_mission_template`;
- `urban_blocked_route_policy`;
- `urban_mock_perception` for `urban-search`.

Stop/abort conditions:

- dry-run exits non-zero;
- `artifact_validator --mode dry-run --strict` fails;
- any Urban geo artifact is described as a full map parser, certified obstacle
  avoidance, hardware run, or real perception system.

## Runbook 4: Artifact Validation

Validate any local supervisor output pack before citing it.

```bash
cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir target/sitl/local-multi-agent-sih \
  --mode supervisor-run \
  --strict
```

Expected artifacts:

- `artifact_validation_report.v1.json` when output is requested by the
  validator workflow;
- pass/fail status with stable artifact rule ids.

Stop/abort conditions:

- `artifact.final_status_mismatch`;
- `artifact.replacement_seq_mismatch`;
- `artifact.safety_report_missing`;
- `artifact.limitations_missing`;
- any new artifact rule id that is not understood by the operator.

## Runbook 4a: M83 Primitive Command Dry-Run

Use this when the candidate is a primitive command mission rather than Urban or
task allocation:

```bash
cargo run -p swarm-examples --bin sitl_agent -- \
  --dry-run \
  --scenario scenarios/primitive.takeoff-hold-land.json \
  --agent-id agent-0 \
  --dry-run-artifact target/m83-primitive/sitl_dry_run_artifact.v1.json \
  --mavlink-profile px4

cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir target/m83-primitive \
  --mode dry-run \
  --strict
```

Repeat with `scenarios/primitive.orbit.json` and
`scenarios/primitive.square.json` when the claim covers all M83 primitives.
Expected evidence:

- command sequence in `command_ir_summary`;
- timeout/abort policy, `expected_terminal_state`, and
  `completion_tolerance`;
- `mavlink_common_plan.expected_acks`;
- `mavlink_common_plan.telemetry_milestones`;
- M82 `compatibility` report for the selected profile;
- `safety_report.passed=true`.

Stop/abort conditions:

- dry-run exits non-zero;
- `artifact_validator --mode dry-run --strict` fails;
- orbit profile caveats are being treated as PX4/ArduPilot equivalence;
- any output is described as real flight, hardware upload, or certified flight
  safety.

## Runbook 5: Local PX4/SIH

Local PX4/SIH remains manual and experimental. It is not automated PX4 CI,
Gazebo/HIL validation, hardware evidence, or production failover.

Inspect commands first:

```bash
DRY_RUN=1 scripts/run_m58_local.sh
DRY_RUN=1 scripts/run_m59_local.sh
```

Run only when local PX4/SIH commands are configured by the operator:

```bash
PX4_AGENT0_CMD='...' PX4_AGENT1_CMD='...' scripts/run_m58_local.sh
PX4_AGENT0_CMD='...' PX4_AGENT1_CMD='...' scripts/run_m59_local.sh
```

Expected artifacts:

- supervisor output directory;
- `manifest.json`;
- `events.sitl-log.json`;
- `run-report.json`;
- `replay-summary.txt`;
- `safety_validation_report.v1.json`;
- `scenario.snapshot.json`;
- `config.snapshot.json`;
- `command.txt`.

Stop/abort conditions:

- PX4/SIH endpoint is not local loopback;
- hardware-candidate guard is triggered unexpectedly;
- heartbeat, telemetry, progress, mission upload, command ack, or abort fails;
- `artifact_validator --strict` fails after the run.

## Runbook 6: Future Hardware Candidate

This path is intentionally conservative. The repository can prepare a candidate
procedure, but it cannot approve hardware operation by itself.

Minimum entry conditions:

- simulation runbook passed;
- Urban runbook passed if the mission is Urban;
- SITL dry-run/export passed;
- artifact validation passed for relevant local supervisor packs;
- external safety process completed outside this repository;
- pilot/operator roles assigned;
- manual override path rehearsed;
- physical kill switch or flight termination path available and tested;
- geofence/no-fly assumptions reviewed;
- local legal/regulatory constraints reviewed outside this repository;
- single-drone controlled review completed before any multi-drone hardware.

Example guarded command shape:

```bash
cargo run -p swarm-examples --bin sitl_agent --features mavlink-transport -- \
  --connection serial:/dev/ttyUSB0:57600 \
  --allow-hardware-candidate \
  --scenario scenarios/sitl.px4-golden.json \
  --agent-id agent-0 \
  --safety-config path/to/safety.json \
  --dry-run-artifact target/m79-hardware-candidate/preflight.json
```

This command shape is not a recommendation to fly. It documents the guardrail:
hardware-candidate endpoints require explicit acknowledgement and still need an
external safety process.

## Post-Run Inspection

After any run:

- validate artifacts;
- inspect replay timeline;
- compare run report and event log;
- compare safety report with the scenario snapshot;
- record known limitations;
- record whether rerun is allowed;
- keep historical artifacts tied to their `git_commit`;
- do not upgrade a successful first hardware experiment into a production
  readiness claim.

## API And Schema Boundary

Current schema stability is repository-local:

- report/replay schemas may gain additive fields;
- benchmark and SITL artifacts must keep schema versions visible;
- external-style mission examples should be treated as compatibility smoke, not
  a semver promise;
- no public semver commitment exists unless a future API branch explicitly
  chooses one.
