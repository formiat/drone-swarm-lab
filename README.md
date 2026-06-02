# Swarm Coordination Runtime

Swarm Coordination Runtime is a Rust workspace for mission-level coordination of autonomous drone fleets. The current code focuses on deterministic simulation, task ownership, heartbeat-based membership, failure detection, and measurable recovery behaviour rather than low-level flight control.

This is a **research prototype**, not a production flight-control system.

## Quick Start (Golden Path)

### 1. Clone and test

```bash
git clone <repo-url>
cd drone
cargo test --workspace
```

Expected: 250+ tests pass in ~15 seconds.

### 2. Run smoke benchmark

```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --smoke --mission coverage --output-dir results/coverage_smoke/
```

Expected: `results/coverage_smoke/` created with `results.json`, `results.csv`, `table.md`, `manifest.json`.

### 3. Run scenario suite

```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --scenario-suite scenarios/coverage.safety.json --output-dir results/safety_smoke/
```

### 4. Create benchmark pack with report

```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --quick --mission sar --output-dir results/sar_quick/ --report results/sar_quick/report.md
```

### 5. Inspect replay

```bash
# Generate replay log
cargo run -p swarm-examples --bin strategy_comparison -- \
  --smoke --mission coverage --replay-log results/replay/

# Inspect summary
cargo run --bin replay -- --log results/replay/*.json --summary

# ASCII snapshot at tick 50
cargo run --bin replay -- --log results/replay/*.json --tick 50

# Filtered event timeline
cargo run --bin replay -- --log results/replay/*.json --timeline --category urban
```

### 6. Run wildfire mapping benchmark

```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --smoke --mission wildfire --output-dir results/wildfire_smoke/
```

### 7. Inspect SITL waypoint plan

```bash
cargo run --bin sitl_agent -- \
  --dry-run --scenario scenarios/sitl.waypoints.json --agent-id agent-0
```

Expected: a portable mission upload plan printed without PX4, including
scenario name, task ids, waypoint sequence, local coordinates, and altitude
interpretation.

### 7a. Export an Urban route to a SITL waypoint plan

```bash
cargo run --bin sitl_agent -- \
  --dry-run \
  --scenario scenarios/urban.patrol.json \
  --agent-id agent-0 \
  --dry-run-artifact results/urban_route_export/dry-run.json
```

Expected: Urban Patrol route segments are converted into an ordered waypoint
mission with route length, segment count, waypoint count, altitude, coordinate
origin, and stable route identity fields. The optional JSON artifact uses the
`sitl_dry_run_artifact.v1` schema. This is the M70 Urban Route Export + Geo Origin
dry-run path. It is a local waypoint workflow only: no PX4 upload is
performed, no hardware readiness is claimed, and it does not add real
perception, lidar/raycast, certified obstacle avoidance, dynamic traffic
handling, or low-level flight control.

### 8. Run mock SITL

```bash
cargo run --bin sitl_agent -- \
  --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0
```

Expected: the same waypoint plan is sent through the in-memory mock transport
without external PX4, simulator processes, network sockets, or hardware.

### 9. Verify portable SITL checks

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_agent portable_sitl_regression_smoke

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_docs
```

Expected: reviewers can validate scenario loading, waypoint extraction, safety
validation, dry-run output, mock replay logging, and documentation anchors
without running PX4.

### 10. Verify dynamic reallocation checks

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-runtime reallocation

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples sitl_observability
```

Expected: heartbeat timeout returns unfinished tasks from a lost agent to the
pool, surviving agents recover assignable tasks, ownership stays unique, and
SITL event logs expose agent lost / task released / task reassigned events.
These checks are mock/fake/runtime-level. M59 adds a controlled local
`--connection --execute --reupload-on-failure` path that detects a failed live
agent while another local PX4/SIH agent is active, releases unfinished tasks
through runtime reallocation, and replaces the active survivor mission. This is
still local PX4/SIH evidence only, not Gazebo, HIL, hardware, or production
failover validation.

### 11. Inspect multi-agent SITL manifest

```bash
cargo run -p swarm-examples --bin sitl_supervisor -- \
  --dry-run \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.config.json

cargo run -p swarm-examples --bin sitl_supervisor -- \
  --mock \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.config.json \
  --fail-agent agent-0 \
  --fail-after-ticks 1 \
  --heartbeat-timeout-ticks 3 \
  --replay-log target/sitl/multi-supervisor.sitl-log.json

cargo run -p swarm-examples --bin sitl_agent -- \
  --dry-run \
  --scenario scenarios/sitl.multi-agent.json \
  --agent-id agent-0 \
  --multi-agent-config scenarios/sitl.multi-agent.config.json
```

Expected: a portable `multi_sitl_manifest.v1` JSON manifest with per-agent
MAVLink system/component ids, connection strings, lifecycle mode, start delays,
task subsets, waypoint subsets, standalone `sitl_agent` commands, and ownership
summary. Duplicate ownership is rejected before upload. This is a
dry-run/mock/config foundation. A separate two-instance PX4 SIH upload-only
check is captured in `results/m55_multi_agent_px4_sih_2026-05-30/`.

### 12. Run a multi-agent PX4/SIH execute supervisor

```bash
cargo run -p swarm-examples --bin sitl_supervisor --features mavlink-transport -- \
  --connection --execute \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.execute.config.json \
  --safety-config path/to/sitl-safety.json \
  --timeout 5 --telemetry-timeout 10 --no-progress-timeout 60 \
  --reupload-on-failure \
  --output-dir target/sitl \
  --run-id local-multi-agent-sih
```

Expected: `sitl_supervisor` validates every configured agent before upload,
rejects hardware-candidate endpoints unless `--allow-hardware-candidate` is
explicit, then starts local PX4/SIH endpoints, polls active agents stepwise for
telemetry progress, writes a common event log, and emits a structured
multi-agent run report. With `--reupload-on-failure`, failed-agent
reallocation is an explicit controlled local path: unfinished tasks are
released through runtime reallocation and uploaded as a replacement mission to
an active survivor. Real hardware is not claimed.
The common event log uses per-agent mission/task/failure events with `agent_id`,
so repeated waypoint sequence numbers remain attributable to the correct agent.

M60 hardens this supervisor workflow for repeatable local PX4/SIH research
runs. `--output-dir` creates a stable run directory containing
`manifest.json`, `events.sitl-log.json`, `run-report.json` for connection
execute mode, `replay-summary.txt`, `safety_validation_report.v1.json`,
`scenario.snapshot.json`, `config.snapshot.json`, and `command.txt`; `--run-id`
fixes the directory and log identity; `--force` is required to overwrite
existing artifacts. M72 adds `artifact_validator`, which checks this pack with
stable rule ids such as `artifact.final_status_mismatch` and
`artifact.replacement_seq_mismatch`:

```bash
cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir target/sitl/local-multi-agent-sih \
  --mode supervisor-run \
  --strict
```

The manual-only helpers `scripts/run_m58_local.sh` and
`scripts/run_m59_local.sh` can rerun local PX4/SIH evidence when PX4 is
installed; use `DRY_RUN=1` to inspect commands without launching PX4. The
supervisor now returns stable exit codes: `2` for CLI/config errors, `3` for
safety or hardware-candidate guard failures, `20` for unavailable
endpoint/feature gates, `21` for upload or command rejection, `22` for
heartbeat/telemetry/progress timeouts, `23` for abort failures, `30` for runtime
partial failure after start, and `40` for artifact write or overwrite-policy
failures.

### 13. Upload or execute a mission in PX4 SITL

```bash
cargo run -p swarm-examples --bin sitl_agent --features mavlink-transport -- \
  --connection udpin:127.0.0.1:14550 \
  --scenario scenarios/sitl.px4-golden.json \
  --agent-id agent-0 \
  --safety-config path/to/sitl-safety.json \
  --upload-only
```

This is an experimental waypoint path for PX4 SITL. Upload-only mode waits for a
MAVLink heartbeat, validates the mission before upload, optionally clears the
existing mission, sends mission count, responds to `MISSION_REQUEST_INT` or
legacy `MISSION_REQUEST`, sends `MISSION_ITEM_INT` waypoints, and requires an
accepted `MISSION_ACK`. If `--safety-config` is omitted, conservative SITL
defaults are used.

Remote, wildcard, TCP, and serial connections are treated as hardware
candidates and are guarded by `--allow-hardware-candidate`. This opt-in does
not make the project hardware-ready. Read
[`docs/HARDWARE_READINESS.md`](docs/HARDWARE_READINESS.md) before any hardware
experiment.

Execution is opt-in:

```bash
cargo run -p swarm-examples --bin sitl_agent --features mavlink-transport -- \
  --connection udpin:127.0.0.1:14550 \
  --scenario scenarios/sitl.px4-golden.json \
  --agent-id agent-0 \
  --execute --timeout 5 --telemetry-timeout 10 --no-progress-timeout 60 \
  --run-report target/sitl/single-agent-report.json \
  --replay-log target/sitl/single-agent.sitl-log.json
```

`--execute` uploads the mission, sends arm/takeoff/start commands, requires
command acknowledgements, checks for a fresh post-start heartbeat, then waits
for typed telemetry progress. It maps `MISSION_CURRENT` and
`MISSION_ITEM_REACHED` to SITL task ids, exits `0` only after every waypoint
task is completed, and attempts RTL abort on rejected, disconnected, or stalled
missions. With `--run-report`, it writes a structured JSON final report with
scenario, agent id, mission item count, completed/failed counts, final status,
and error details when available. With `--replay-log`, it writes an ordered SITL
event trace covering upload handshake events, lifecycle commands, telemetry
progress, aborts, and failures.

```bash
cargo run -p swarm-examples --bin replay -- \
  --sitl-summary target/sitl/single-agent.sitl-log.json
```

---

## Current Status

| Feature | Status | Since | Notes |
|---|---|---|---|
| Benchmark (smoke/quick/full) | ✅ Stable | M21/M62/M63/M69 | `--output-dir`, `--report`, `BenchmarkManifest`; M69 current-head 1000-seed release benchmark is in `results/all_1000_jobs14_m69_release/`; M62 500-seed baseline remains historical evidence for commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd` |
| Mission DSL | ✅ Stable | M19 | `schema_version: "0.1"`, validation API |
| Platform Extension Guide | ✅ Stable-ish | M61 | `docs/EXTENSION_GUIDE.md` documents mission, strategy, metrics, crate boundary, and schema-version extension paths without promising semver-stable public API |
| Safety Layer | ✅ Stable | M20 | `SafetyAllocator` wrapper, no-fly/geofence/separation |
| SAR v2 | ✅ Stable | M14 | `BeliefMap`, sensor noise, confirmation scans |
| CBBA Robustness | ✅ Stable | M15 | Convergence metrics, TSP ordering, retransmission |
| Infrastructure Inspection | ✅ Stable | M16 | Edge coverage, route efficiency |
| Mock SITL | ✅ Stable | M20 | `sitl_agent --mock`, no external deps |
| SITL Dry-Run | ✅ Stable | M43 | `sitl_agent --dry-run`, portable mission upload plan without PX4 |
| SITL Portable Regression | ✅ Stable | M50 | `portable_sitl_regression_smoke` and `sitl_docs` validate dry-run/mock/safety/docs without external PX4 |
| Dynamic Reallocation | ✅ Stable local SITL | M51/M59 | Heartbeat timeout releases unfinished tasks from lost agents, recovers assignable tasks on survivors, exposes runtime metrics and SITL reallocation events; `sitl_supervisor --mock` emits the failure/reallocation flow; M59 wires the same runtime path into controlled local PX4/SIH active-survivor mission replacement behind `--reupload-on-failure` |
| Multi-Agent SITL Foundation | ✅ Stable local SITL | M52/M58/M59 | `multi_sitl.v1` config, public fixtures, `sitl_supervisor` dry-run/mock orchestration, per-agent task subsets, MAVLink system/component mapping, duplicate ownership rejection, two-instance PX4 SIH upload-only evidence, local multi-agent PX4/SIH execute evidence, and controlled active-survivor mission replacement after a failed agent |
| PX4/SIH Supervisor Hardening | ✅ Stable local workflow | M60 | `sitl_supervisor --output-dir`, `--run-id`, `--force`, checked artifact overwrite policy, stable exit codes, `task_ownership` / `events_summary` / `final_status` / `limitations` in `sitl_multi_agent_run_report.v1`, and replay summary artifacts for repeatable local PX4/SIH runs |
| Artifact Validation | ✅ Stable local workflow | M72 | `artifact_validator` validates supervisor packs (`manifest.json`, event log, run report, replay summary, safety report, snapshots, command metadata) with stable rule ids; `scripts/run_m58_local.sh` and `scripts/run_m59_local.sh` are manual-only PX4/SIH harnesses with `DRY_RUN=1` |
| Hardware Readiness Boundary | ✅ Stable | M53 | `docs/HARDWARE_READINESS.md`, connection classes, and `--allow-hardware-candidate` guard remote/wildcard/serial hardware candidates; this documents the boundary, not hardware readiness |
| Supervisor Controller Boundary | ✅ Stable | M57 | `sitl_supervisor` mock orchestration is split into a testable internal supervisor module with `AgentController`, `MockAgentController`, fake-controller coverage, and assertable `SupervisorMetrics`; M58 adds the separate live PX4/SIH execute controller path |
| Replay / Debuggability | ✅ Stable | M23/M67 | `replay` CLI, ASCII visualization, deterministic timeline output, and optional `--agent` / `--category urban` filters |
| Mission Semantics | ✅ Stable | M33 | `TaskKind`, 6 concrete adapters, `AdapterRegistry`, adapter-driven completion/scoring in runner and allocator |
| Urban Foundations | ✅ Stable substrate | M64 | Road-graph-first Urban substrate: `UrbanMap`, deterministic Dijkstra route-loop planning, AABB static obstacle judge, `urban-patrol` DSL validation, `scenarios/urban.patrol.json`, and Urban metrics skeleton |
| Urban Patrol | ✅ Simulation v0 | M65 | One drone follows an ordered road-graph block loop, completes before timeout, records Urban replay events and exports patrol metrics; simulation-only, with no lidar, dynamic obstacles, multi-agent deconfliction, PX4/SITL export, hardware claim, or UI |
| Urban Search | ✅ Simulation v1 | M66 | One scout follows the Urban road graph and stops on a deterministic mocked bus detector hit; includes `urban-search` DSL validation, `scenarios/urban.search.json`, replay events, regression thresholds, and bus-detection/time/false-positive/distance metrics; still no lidar/raycast, dynamic obstacles, real perception, PX4/SITL export, hardware claim, or UI |
| Urban Replay / Analysis | ✅ Diagnostic tooling | M67 | Urban replay logs now support route-trace and judge-report artifacts under benchmark packs, timeline filtering, `UrbanViolation.obstacle_id`, a two-agent analysis fixture in `scenarios/urban.multi-agent.json`, and aggregate separation/conflict metrics; still diagnostic only, with no avoidance or physical simulation |
| Urban Corridor Planner | 🧪 Experimental algorithm delta | M68 | Adds `planner: "corridor-aware"` for Urban route loops, route-risk metrics, `scenarios/urban.corridor-delta.json`, and a small before/after evidence pack; this is mission-level planning, not physical avoidance or PX4 proof |
| Preflight Safety Contract | ✅ Static gate | M71 | `SafetyValidationReport`, rule-id based preflight violations, dry-run/supervisor preflight gates, `safety_validation_report.v1.json`, `docs/PREFLIGHT_SAFETY.md`, and stable exit code convention 2/3/4/5; not certified flight safety |
| Planner Quality | ✅ Stable | M34 | `RoutePlanner` trait, 2-opt, battery-aware feasibility v2 (ordered-subset feasibility, battery model v2 integration, meaningful runner metrics) |
| Dynamic Mission Correctness | ✅ Stable | M35/M63 | Mission-specific success semantics (SAR=targets-found, inspection=coverage-threshold, wildfire=mapped-ratio threshold plus failure/unassigned guards), SAR unsupported reasons (cbba=delayed-reconvergence, centralized=static-pre-plan), support matrix tests |
| Regression Harness v2 | ✅ Stable | M36 | Calibrated thresholds, portability fixes, wildfire/realism suites, failure delta output, and repeated release determinism sweep for `jobs=1/4/14` |
| Realism Scenario Pack | ✅ Stable | M37 | Realism profiles (light/medium/heavy), scenario JSONs, battery model metadata, baseline vs realism comparison |
| Wildfire v2 | ⚠️ Partial | M38/M63 | Spatial spread, wind influence, zone expansion, high-priority metrics, replay integration, scenario JSONs; flood remains future work and is not implemented as a separate mission |
| Decision / Audit Report | ✅ Stable | M39b | Status audit, README honesty update, benchmark docs marked historical |
| Regression Repair | ✅ Stable | M39a | Unified regression entrypoints, wildfire/realism repair, runtime ordering fixes, SAR scan completion fix, and repeated default regression sweep |
| Wildfire Mapping | ✅ Stable | M30/M63 | `TaskKind::MappingZone`, `WildfireState`, hazard zones, dynamic threat; flood is future work and is not implemented as a separate mission |
| Simulation Realism | ✅ Stable | M31 | Battery model v2, altitude sensor penalty, wind drift, pose noise, comms jitter, time-gated no-fly zones, `--realism` preset |
| Reporting & Metrics | ✅ Stable | M32 | Per-row mission/scenario in exports, mission-scoped profiles, merged `all` benchmark id, wildfire/planner metrics, realism metadata in manifest |
| Real PX4 | 🧪 Experimental | M49/M58/M59 | Feature-gated single-agent PX4 SITL report/replay plumbing, local multi-agent PX4/SIH execute supervisor plumbing, pre-upload safety validation, arm/takeoff/start, telemetry-to-task progress mapping, controlled `--reupload-on-failure` active-survivor mission replacement, structured final reports, compact SITL event summaries, public `scenarios/sitl.px4-golden.json`, `scenarios/sitl.multi-agent.execute.config.json`, and captured single-agent/upload-only/execute/failure SIH evidence; still not hardware-ready |

**Test coverage:** 380+ tests, 10 crates, 22 JSON scenarios.

> **Project Status:** For an honest audit of what is fully complete vs partially complete, see [`docs/STATUS.md`](docs/STATUS.md).

> **Extension Status:** [`docs/EXTENSION_GUIDE.md`](docs/EXTENSION_GUIDE.md) documents the current stable-ish in-repository extension points for missions, allocation strategies, metrics, crate boundaries, and schema versioning. It is not semver-stable public API and does not change the hardware/non-goals boundary.

---

## Regression Testing

The benchmark platform includes a regression harness (`RegressionSuite`, `ThresholdChecker`, `RegressionRunner`) that runs critical scenarios and checks their health against configurable thresholds.

```bash
# Run all default regression suites
cargo run -p swarm-examples --bin regression_runner -- --jobs 4

# Run regression via strategy_comparison CLI
cargo run -p swarm-examples --bin strategy_comparison -- --regression

# Compare against committed baseline
cargo run -p swarm-examples --bin regression_runner -- --compare-baseline results/baseline.json --jobs 4
```

Exit code is `0` if all suites pass, `1` if any threshold is violated. Failure output includes metric name, actual value, threshold bound, and delta.

Current status note: the default regression entrypoints passed the repeated
release sweep at `jobs=1/4/14`. The captured sweep is in
`results/m56_regression_determinism_2026-05-30/`. The latest committed full
simulation benchmark refresh is the M69 1000-seed release run in
`results/all_1000_jobs14_m69_release/`. The older M62 500-seed release baseline
in `results/all_500_jobs14_m62_release/` remains historical validation evidence
for commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd`. Use these simulation
runs as benchmark evidence, not as PX4/SITL or hardware evidence.

### Portable SITL Checks (M50)

SITL has a separate portable regression path for the CLI boundary. These checks
are intentionally not part of the benchmark regression runner: they validate
the dry-run/mock waypoint workflow, not simulation performance metrics.

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_agent portable_sitl_regression_smoke

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_docs
```

These checks require no external PX4, no simulator, no network endpoint, and no
real hardware. Live PX4 SITL verification remains a manual/local workflow in
[`docs/SITL_SETUP.md`](docs/SITL_SETUP.md).

### Dynamic Reallocation Checks (M51)

M51 adds the minimal runtime contract needed before multi-agent SITL: a lost
agent releases unfinished tasks, assignable tasks are recovered by surviving
agents, and the SITL event log can show agent lost / task released / task
reassigned / reallocation completed events.

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-runtime reallocation

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-runtime failure

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples sitl_observability
```

These checks are deterministic and use in-memory/mock/fake runtime paths. They
do not start PX4 and do not claim live multi-agent PX4 failure handling; real
multi-agent PX4 execute orchestration remains later work beyond the M52
foundation.

### Multi-Agent SITL Foundation Checks (M52)

M52 adds the config and manifest foundation for multi-agent SITL. It maps each
`agent_id` to MAVLink system/component ids, connection string, lifecycle mode,
start delay, and explicit task subset. It also validates duplicate ownership
before any upload path.

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples sitl_multi_agent

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_agent multi_agent

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_docs
```

These checks remain portable. They exercise JSON config parsing, task subset
splitting, `sitl_supervisor` dry-run/mock manifests, mock supervisor
reallocation, and pre-upload duplicate ownership rejection without starting PX4.

### Supervisor Controller Boundary Checks (M57)

M57 keeps the external `sitl_supervisor --dry-run` / `--mock` CLI stable while
moving the mock supervisor state machine into a reusable internal module. The
new boundary introduces an internal `AgentController` trait, `MockAgentController`
and assertable `SupervisorMetrics`. The shared supervisor loop is exercised
through a test-only fake controller as well as the mock controller, so future
controller implementations can reuse the same lifecycle/progress path. M57 does
not add a live PX4 supervisor mode.

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples sitl_supervisor

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor
```

These checks cover mock controller upload/poll behavior, metrics formatting,
mock failure/reallocation metrics, CLI negative argument handling, and the
existing subprocess supervisor contract.

### Live Multi-Agent PX4/SIH Execute And Reallocation Checks (M58/M59)

M58 adds an experimental `sitl_supervisor --connection --execute` path for local
PX4/SIH endpoints. It reuses the same multi-agent manifest, requires
`lifecycle: "execute"` for every live-supervised agent, applies per-agent
pre-upload safety validation before any feature-gated MAVLink work, rejects
hardware-candidate connection strings unless explicitly allowed, drives each
agent through the PX4 upload/execute/telemetry progress path, and writes a
common SITL event log with explicit per-agent mission/task/failure attribution
plus a `sitl_multi_agent_run_report.v1` JSON report. `--safety-config` is
accepted only in `--connection --execute`; dry-run/mock supervisor modes reject
it instead of silently ignoring it.

M59 adds explicit `--reupload-on-failure` handling. The supervisor starts local
agents, polls them stepwise, detects a failed active agent, marks it lost,
releases its unfinished tasks through the runtime reallocation path, records
`agent_lost`, `task_released`, `task_reassigned`, `reallocation_completed`,
`survivor_mission_update_started`, and `survivor_mission_update_completed`,
then aborts/replaces the mission state for an active survivor. The report
includes a `reallocation` section with released/reassigned/recovered tasks,
latency ticks, survivor mission update count, and tasks completed after
reallocation.

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples sitl_connection

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples sitl_supervisor

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor
```

These automated checks are still portable: fake controllers cover report/event
aggregation and active-survivor edge cases, CLI tests cover validation order,
and no external PX4 endpoint is required. Real local PX4/SIH execute and
failure-injection runs remain manual workflows because they need running PX4
instances and operator-controlled endpoints. Captured local artifacts are in
`results/m58_multi_agent_px4_sih_execute_2026-05-31/` and
`results/m59_px4_sih_failure_reallocation_2026-05-31/`.

### Default Suites (M36)

| Suite | Mission | Profile | Strategy | Mode | Key Thresholds |
|---|---|---|---|---|---|
| `sar_ideal_greedy` | sar | ideal | greedy | smoke | task_completion_rate ≥ 0.80, targets_found ≥ 2, belief_entropy_final ≤ 0.75 |
| `sar_standard_greedy` | sar | standard | greedy | smoke | task_completion_rate ≥ 0.70, belief_entropy_final ≤ 0.6 |
| `inspection_linear_all` | inspection | linear | all | smoke | edge_coverage_rate ≥ 0.85, success_rate ≥ 0.90 |
| `inspection_perimeter_all` | inspection | perimeter | all | smoke | edge_coverage_rate ≥ 0.25 (floor) |
| `inspection_perimeter_experimental` | inspection | perimeter | greedy | smoke | edge_coverage_rate ≥ 0.30 |
| `cbba_coverage_ideal_no_failures` | coverage | ideal-no-failures | cbba | quick | success_rate ≥ 0.90, convergence_ticks_p95 ≤ 15 |
| `cbba_coverage_light_loss_no_failures` | coverage | light-loss-no-failures | cbba | quick | success_rate ≥ 0.80, convergence_ticks_p95 ≤ 20 |
| `safety_coverage` | coverage | ideal-no-failures | greedy | smoke | safety_violations ≤ 0 |
| `emergency_mesh_ideal` | emergency-mesh | ideal | greedy | smoke | network_availability ≥ 0.001 |
| `wildfire_small_static_greedy` | wildfire | small-static | greedy | smoke | task_completion_rate ≥ 0.80 |
| `wildfire_medium_dynamic_greedy` | wildfire | medium-dynamic | greedy | smoke | task_completion_rate ≥ 0.60 |
| `realism_coverage_smoke` | coverage | ideal-no-failures | greedy | smoke | success_rate ≥ 0.75 (realism preset) |

**Modes:** `smoke` = 1 seed; `quick` = 10 seeds. **SAR and wildfire** use `task_completion_rate` (not `success_rate`) because M35 changed success semantics to mission-specific definitions. For wildfire, `success=true` requires `mapped_zone_count / total_zone_count >= wildfire_success_threshold` (default `0.8`), all expected failures detected, and `max_task_unassigned_ticks <= max_unassigned_ticks`; task completion can still be `1.000` when this stricter mapped-ratio predicate is false.

### Threshold Policy

- **No `>= 0.0` thresholds.** Every threshold must be calibrated to catch real regressions.
- **Smoke thresholds** are set against seed 0 results; allow variance headroom (~20–30% below observed).
- **Quick thresholds** are tighter (10-seed average is more stable).
- When adding a new suite: run smoke first, observe metrics, set threshold ~20% below the passing value.

## Baseline Management

Baselines are committed JSON artifacts (`results/baseline.json`) that store reference metric values per suite. They enable delta comparison across runs.

```bash
# Generate a fresh baseline after code changes
cargo run -p swarm-examples --bin regression_runner -- --update-baseline results/baseline.json --jobs 4
```

Baseline format:

```json
{
  "version": "1.0",
  "created_at": "2026-05-28T...",
  "commit": "abc123",
  "results": {
    "suite_name": { "success_rate": 0.85, "avg_edge_coverage_rate": 0.98, ... }
  }
}
```

**Update process:**
1. Complete the milestone (all tests pass, code is stable).
2. Run `--update-baseline results/baseline.json`.
3. Commit `results/baseline.json` referencing the code commit hash.
4. The `commit` field in the JSON is set automatically by `regression_runner`.

## Stress Testing

Parametric sweeps over variables such as packet loss, agent count, or grid size are supported via the stress harness. Coverage CBBA suites already exercise `ideal-no-failures` and `light-loss-no-failures` profiles.

---

## Known Limitations

1. **Simulation/SITL only:** No real hardware workflow; PX4 integration is limited to experimental local SITL waypoint upload, opt-in single-agent lifecycle/progress tracking, local multi-agent PX4/SIH execute supervisor plumbing, controlled active-survivor mission replacement after failed-agent reallocation, and captured SIH evidence with static pre-upload safety checks.
2. **Hardware boundary:** Remote UDP, wildcard UDP, TCP, and serial connection strings are hardware candidates and require `--allow-hardware-candidate`; this is only an explicit opt-in guard, not flight certification or proof of hardware readiness. See [`docs/HARDWARE_READINESS.md`](docs/HARDWARE_READINESS.md).
3. **Multi-agent SITL remains experimental:** M52/M58/M59 support config-driven per-agent task subsets, dry-run/mock manifests, mock supervisor reallocation, standalone command generation, duplicate ownership checks, local PX4 SIH upload-only verification, a local live execute supervisor path, and controlled active-survivor mission replacement after a failed agent. It does not provide robust distributed coordination, automated PX4 CI, Gazebo/HIL/hardware validation, or hardware safety guarantees.
4. **SITL coordinate frame:** `sitl_agent` dry-run/mock mode treats `Pose { x, y, z }` as local simulation coordinates; `x/y` are not WGS84 latitude/longitude, and `z` is local altitude.
5. **3D pose:** Scenarios support `z` coordinate and altitude-aware sensors, but most missions operate primarily in XY plane.
6. **Deterministic RNG:** Scenarios use seeded RNG; real-world noise is modeled optionally via `--realism` preset.
7. **Battery model v2:** Hover/climb/cruise drain rates are configurable but not calibrated against real flight data.
8. **Regression smoke variance:** Smoke suites use 1 seed; high-variance missions (SAR, emergency-mesh, wildfire) have conservative thresholds. Promote to `Quick` (10 seeds) for tighter coverage.

See [Strategy Support Matrix](#strategy-support-matrix) for per-strategy known limitations.

---

## Strategy Support Matrix

| Mission | Strategy | Status | Notes |
|---------|----------|--------|-------|
| coverage | all | stable | All strategies produce success_rate > 0.9 on ideal/standard profiles |
| sar | greedy, auction, connectivity-aware | stable | — |
| sar | cbba | unsupported | CBBA re-convergence delay after `release_task()` exceeds `max_unassigned_ticks`; explicit `unsupported_reason: delayed_reconvergence` (M35) |
| sar | centralized | unsupported | Static pre-planning incompatible with SAR dynamic task release; agents revisit stale cell assignments |
| inspection (linear/random) | all | stable | — |
| inspection (perimeter) | greedy, auction, connectivity-aware | experimental | Battery/time constraint limits coverage; success rate ~0–0.4 |
| inspection (perimeter) | centralized | experimental | Static plan; moderate coverage |
| inspection (perimeter) | cbba | experimental | Allocation gap (`max_bundle_size`); bundle-slot fix (M26) improves coverage |
| urban-patrol | greedy + dijkstra corridor-delta baseline | stable | M68 baseline profile uses shortest route over the same Urban road graph |
| urban-patrol | greedy + corridor-aware corridor-delta | experimental | M68 lowers route-risk on the synthetic corridor fixture, with longer route/time tradeoff; broader benchmark refresh remains M69 |
| urban-search | greedy | stable | Static mocked-bus search fixture with deterministic detector |

**Status meanings:**
- **stable** — success_rate > 0 across standard seeds; suitable for benchmarking.
- **experimental** — works but constrained by battery/time or algorithmic limits; use with awareness.
- **unsupported** — consistently 0% success due to a known root cause; tracked for future milestones.

---

## Non-Goals

- **Not a production flight-control system.** This is a research prototype for coordination algorithms.
- **Not a certified safety layer.** Safety constraints are checked but not formally verified.
- **Not ready for real-world swarm flights.** Simulation-only with experimental PX4 SITL mission upload, opt-in lifecycle commands, and static pre-upload validation.
- **Not a MAVLink ground control station.** PX4 integration is experimental and minimal.

---

## Workspace Layout

| Crate | Purpose |
| --- | --- |
| `swarm-types` | Shared IDs, agent/task/message types, pose, velocity, mission semantics (`TaskKind`, `MissionAdapter`). |
| `swarm-comms` | Transport trait, in-memory network, UDP transport, MAVLink transport (optional). |
| `swarm-runtime` | Membership, failure detection, task registry, coordinator, `AgentNode`. |
| `swarm-alloc` | Greedy, auction, connectivity-aware, centralized, CBBA allocation strategies. |
| `swarm-sim` | Deterministic clock, scenario model, generic scenario runner, DSL loader, JSON/CSV export, Urban replay analysis artifacts. |
| `swarm-scenarios` | Scenario builders: Coverage, Emergency Mesh, SAR, Infrastructure Inspection, Wildfire Mapping, Urban Patrol, Urban Search; flood remains future work. |
| `swarm-metrics` | Per-run and aggregate metrics, including Urban route planning, patrol completion, mocked bus-search detection/time/false-positive/distance fields, and diagnostic separation/conflict fields. |
| `swarm-replay` | Event log, replay engine, summary/timeline CLI, ASCII visualization. |
| `swarm-safety` | Safety layer: geofence, no-fly zones, separation constraints. |
| `swarm-examples` | Runnable binaries: `strategy_comparison`, `regression_runner`, `sitl_agent`, `replay`. |

The intended extension surfaces are documented in
[`docs/EXTENSION_GUIDE.md`](docs/EXTENSION_GUIDE.md). Use `swarm-types`
(`TaskKind`, `MissionAdapter`, `RunState`), `swarm-alloc`
(`Allocator`, `Strategy`, `StrategyRegistry`), `swarm-metrics`
(`RunMetrics`, `AggregateMetrics`), and the documented scenario/replay/report
schemas for in-repository extensions. These are stable-ish research extension
points, not a published semver-stable SDK.

---

## Milestones Overview

| Milestone | Status | Key Deliverable |
|---|---|---|
| M1 | ✅ | Foundational coordination: heartbeat, failure detection, greedy reallocation |
| M2 | ✅ | Auction allocation, capability matching, task expiration |
| M3 | ✅ | Pluggable transport, multiprocess runtime |
| M4 | ✅ | Partial connectivity, gossip-based convergence |
| M5 | ✅ | Emergency Mesh Network, connectivity-aware allocation |
| M6 | ✅ | Strategy Comparison Platform, centralized oracle |
| M7 | ✅ | Replay infrastructure, JSON/CSV export |
| M8 | ✅ | Kinematic model, battery drain, movement |
| M9 | ✅ | SAR v1: grid search, hidden targets |
| M10 | ✅ | CBBA: distributed consensus-based bundle algorithm |
| M11 | ✅ | Hardening: mission-aware export, proptest |
| M12 | ✅ | Mission DSL: JSON scenario suites |
| M13 | ✅ | Safety Layer: geofence, no-fly, separation |
| M14 | ✅ | SAR v2: BeliefMap, Bayes updating, sensor noise |
| M15 | ✅ | CBBA Robustness: TSP ordering, retransmission, convergence metrics |
| M16 | ✅ | Infrastructure Inspection: edge coverage, route efficiency |
| M17 | ✅ | SITL / MAVLink scaffold |
| M18 | ✅ | Scenario Catalog Hardening: validation, smoke tests |
| M19 | ✅ | DSL Schema / Validation: `schema_version`, typed errors |
| M20 | ✅ | SITL Path Consolidation: mock vs real PX4 |
| M21 | ✅ | Reproducible Benchmark Pack: smoke/quick/full, manifest |
| M22 | ✅ | Benchmark Report / Analysis: `docs/BENCHMARK_RESULTS.md` |
| M23 | ✅ | Replay / Debuggability: `replay` CLI, ASCII viz |
| M24 | ✅ | Release Candidate / Golden Path: README, docs, non-goals |
| M27 | ✅ | Mission Semantics Layer: `TaskKind`, `MissionAdapter`, `RunState` |
| M28 | ✅ | Planner Quality Upgrade: `RoutePlanner`, 2-opt, battery-aware feasibility |
| M29 | ✅ | Stress & Regression Harness: `RegressionSuite`, baseline artifacts, threshold checking |
| M30 | ✅ | New Mission Prototype: Wildfire Mapping with `TaskKind::MappingZone`, hazard zones, dynamic threat; flood remains future work |
| M31 | ✅ | Simulation Realism Foundation: battery model v2, altitude sensor penalty, wind drift, pose noise, comms jitter, time-gated no-fly zones |
| M32 | ✅ | Benchmark Identity Hardening: per-row mission/scenario in exports, mission-scoped profiles, merged `all` benchmark id, realism metadata in manifest |
| M33 | ✅ | Mission Semantics Integration: 6 concrete adapters, `AdapterRegistry`, adapter-driven completion/scoring in runner and allocator, DSL kind validation |
| M34 | ✅ | Planner Correctness v2: `RoutePlanner` trait, 2-opt, battery-aware feasibility (ordered-subset), meaningful route metrics |
| M35 | ✅ | Dynamic Mission Correctness: mission-specific success semantics, SAR unsupported reasons, support matrix tests |
| M36 | ✅ | Regression Harness v2: calibrated thresholds, wildfire/realism suites, portable tests (tempdir), failure delta output, refreshed baseline |
| M57 | ✅ | Supervisor Controller Boundary: `sitl_supervisor` mock state machine extracted behind internal controller boundary, fake-controller transitions covered, metrics made assertable, CLI compatibility tests expanded |
| M58 | ✅ | Live Multi-Agent PX4/SIH Execute Orchestration: `sitl_supervisor --connection --execute`, per-agent safety/hardware gates, local PX4/SIH agent execution, common event log, structured multi-agent run report, portable fake-controller/CLI coverage, and captured two-agent PX4/SIH execute artifact |
| M59 | ✅ | Live PX4/SIH Failure & Reallocation: explicit `--reupload-on-failure`, stepwise active-agent polling, runtime release/reassignment events, active-survivor mission replacement, report reallocation metrics, portable fake live-controller coverage, and captured controlled PX4/SIH failure artifact |
| M60 | ✅ | PX4/SIH Supervisor Hardening: repeatable local `sitl_supervisor` run layout with `--output-dir` / `--run-id`, explicit `--force` overwrite policy, stable exit codes, report summary fields, replay summaries, and docs/tests for troubleshooting; not hardware readiness |
| M61 | ✅ | Platform / API Stabilization: `docs/EXTENSION_GUIDE.md`, crate-boundary notes, schema-version policy, docs sync, and test-only mission/strategy/runner extension fixtures; not semver-stable public API |
| M62 | ✅ | Benchmark / Baseline Refresh: 500-seed release `--mission all --jobs 14` validation baseline captured for commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd` in `results/all_500_jobs14_m62_release/`; after M63 this is historical evidence unless rerun on current HEAD; not a publication-grade 1000-seed run |
| M63 | ✅ | Evidence Cleanup / Status Honesty: no benchmark rerun; M62 pack marked as historical evidence for `81260ca7afa114a5d9add7b832f6c5d7875b88cd`, flood moved to future work, wildfire success predicate documented/tested, and M58/M59 replay artifacts covered by sanity tests |
| M64 | ✅ | Urban Foundations: road graph model, deterministic Dijkstra route planning, route-loop expansion, AABB static obstacle judge, `urban-patrol` DSL fixture, and Urban metrics skeleton |
| M65 | ✅ | Urban Patrol v0: one scout follows the ordered `urban-patrol` road-graph loop, completes when all planned segments are traversed without judge violations before timeout, emits Urban replay events, and exports patrol completion/time/distance/efficiency metrics; simulation-only |
| M66 | ✅ | Urban Search v1: one scout follows the Urban road-graph loop, evaluates a deterministic mocked bus detector, stops on bus detection, emits bus observation/detection/false-positive/search-completion replay events, exports search metrics, validates `urban-search` DSL, and adds a smoke regression gate; simulation-only |
| M67 | ✅ | Urban Replay / Analysis: replay timeline filters, additive `UrbanViolation.obstacle_id`, route-trace and judge-report artifacts in benchmark packs, two-agent analysis fixture, and diagnostic Urban separation/conflict metrics; no benchmark rerun and no avoidance/control behavior change |
| M68 | ✅ | Algorithm Depth: `corridor-aware` Urban planner uses corridor width and obstacle-clearance risk to choose a lower-risk route on `scenarios/urban.corridor-delta.json`; docs/support matrix/report exports include the route-risk delta; CBBA/SAR weak rows remain explicitly unsupported/analysis-only |
| M69 | ✅ | Benchmark Refresh / Research Evidence: release `--seeds 1000 --mission all --jobs 14` benchmark captured for code commit `5d1d3cd17cacba7482c1d9b93eb5acc107af8f71` in `results/all_1000_jobs14_m69_release/`; regression runner passed at `--jobs 14`; current `--mission all` covers coverage/emergency-mesh/SAR/inspection/wildfire, while Urban remains separate M68 scenario-suite evidence |
| M70 | ✅ | Urban Route Export + Geo Origin: `urban-patrol` route loops export to ordered SITL dry-run waypoint plans and optional `sitl_dry_run_artifact.v1` JSON with route metadata, stable route identity fields, `geo_origin`, and effective default SITL origin; not PX4 execution evidence |
| M71 | ✅ | Preflight Safety And Invariant Contract: static `SafetyValidationReport` preflight gate with rule ids, severity, affected id, reason, geofence/no-fly/altitude/route/ownership/Urban/semantic checks, dry-run artifact integration, supervisor output-dir safety report, and stable exit code convention 2/3/4/5; not certified flight safety |
| M72 | ✅ | Artifact Validator + SITL Harness: `artifact_validator`, `artifact_validation_report.v1`, M72 manifest metadata, scenario/config/command snapshots, stable artifact rule ids, portable validator tests, and manual-only M58/M59 local PX4/SIH harness scripts; not automated PX4 CI or hardware readiness |

---

## Docs

| Document | Description |
|---|---|
| [`docs/BENCHMARK_RESULTS.md`](docs/BENCHMARK_RESULTS.md) | Real benchmark numbers and analysis |
| [`docs/EXTENSION_GUIDE.md`](docs/EXTENSION_GUIDE.md) | Mission, strategy, metrics, crate-boundary, and schema-version extension guide |
| [`docs/SCENARIO_DSL.md`](docs/SCENARIO_DSL.md) | Scenario suite format and validation |
| [`docs/REPLAY.md`](docs/REPLAY.md) | Replay event log schema and CLI usage |
| [`docs/SITL_SETUP.md`](docs/SITL_SETUP.md) | Mock, dry-run, and experimental PX4 SITL setup |
| [`docs/ARTIFACT_VALIDATION.md`](docs/ARTIFACT_VALIDATION.md) | M72 artifact validator contract, rule ids, and manual local SITL harness |

---

## Build

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

## Realism Profiles (M37)

The simulation supports three realism profiles that model environmental noise, wind drift, communication jitter, and battery drain at different intensities.

### Profile Parameters

| Profile | Pose Noise (m) | Wind (m/tick) | Comms Jitter (ticks) | Hover Drain | Climb Drain | Cruise Drain | Reserve |
|---|---|---|---|---|---|---|---|
| **Light** | 0.2 | (0.05, 0.05, 0.0) | 1 | 0.005/tick | 0.03/m | 0.01/m | 10% |
| **Medium** (default) | 0.5 | (0.1, 0.1, 0.0) | 1 | 0.01/tick | 0.05/m | 0.02/m | 15% |
| **Heavy** | 1.0 | (0.2, 0.2, 0.0) | 2 | 0.02/tick | 0.08/m | 0.03/m | 20% |

### CLI Usage

```bash
# Default medium profile
cargo run -p swarm-examples --bin strategy_comparison -- --realism --smoke

# Explicit profile selection
cargo run -p swarm-examples --bin strategy_comparison -- --realism --realism-profile light --smoke
cargo run -p swarm-examples --bin strategy_comparison -- --realism --realism-profile heavy --quick
```

### Realism Scenario Files

Pre-configured scenario suites with realism presets are available in `scenarios/`:

| File | Mission | Profile |
|---|---|---|
| `scenarios/coverage.realism.json` | Coverage | medium |
| `scenarios/sar.realism.json` | SAR | medium |
| `scenarios/inspection.realism.json` | Inspection | medium |
| `scenarios/wildfire.realism.json` | Wildfire | medium |

### Urban Patrol, Search, And Analysis Scenarios

M65 makes `scenarios/urban.patrol.json` an executable deterministic
simulation fixture. One scout follows the ordered road-graph loop
`n0 -> n1 -> n2 -> n3 -> n0` and succeeds when every planned segment is
traversed before timeout with zero Urban judge violations. Replay logs include
`UrbanRoutePlanned`, `UrbanSegmentEntered`, `UrbanSegmentCompleted`, and
`UrbanPatrolCompleted`; reports include patrol completion, time, distance,
route efficiency, violation count, and `urban_replan_count = 0`.

`run_config.urban_state.start_node` is an enforced start contract in M65: when
present, it must exist in the map and match `route_loop.nodes[0]`. The selected
alive agent must start within `0.01m` of that node pose, so an inconsistent
`agent.pose` fails validation/runtime instead of producing a false successful
patrol.

M66 adds `scenarios/urban.search.json` and the explicit `urban-search` mission
type. It reuses the same road graph and start contract, then evaluates
`run_config.urban_search_state`: bus targets, detector range, detection
probability, false-positive rate, and deterministic detector seed. The v1
runner stops on the first real bus detection and reports `bus_detected`,
`time_to_detect_bus`, `false_positive_count`, `distance_before_detection`, and
`search_success_without_violation`.

M67 adds diagnostic analysis around those replay logs. When a benchmark pack is
generated with `--replay-log`, Urban runs also write
`urban_analysis/*.route-trace.json`, `*.route-trace.csv`,
`*.judge-report.json`, `*.judge-report.csv`, and an
`urban_analysis/manifest.json`. The route trace records planned edges, entered
and completed route segments, pose samples, and segment status. The judge
report records Urban violation type, tick, segment, pose, reason, and optional
`obstacle_id`. Aggregate reports also include diagnostic
`avg_urban_min_agent_separation_m`,
`avg_urban_separation_violation_count`, and
`avg_urban_route_conflict_count` fields when Urban replay evidence is present.

`scenarios/urban.multi-agent.json` is an M67 two-agent analysis fixture. It is
meant for replay/analysis and separation-conflict checks only; it does not add
multi-agent Urban control, route deconfliction, or avoidance. Use
`replay --timeline --category urban` to inspect Urban events and
`replay --timeline --agent agent-0` to inspect one agent's event stream.
Running this fixture through `--scenario-suite ... --output-dir ... --replay-log ...`
produces replay logs plus `urban_analysis/` artifacts with two-agent route
traces, minimum separation, and route-conflict counts.

M68 adds `scenarios/urban.corridor-delta.json`, a small before/after algorithm
fixture over one road graph. The `corridor-delta-dijkstra` profile takes the
shorter narrow shortcut. The `corridor-delta-corridor-aware` profile uses
`planner: "corridor-aware"` and accepts a longer route with lower
`urban_route_risk_score`, computed from corridor width and static-obstacle
clearance. This is a planner-risk proxy for strategy comparison; it is not
lidar, physical collision probability, dynamic obstacle avoidance, or PX4/SITL
evidence.

This remains simulation-only: it does not implement lidar/raycast, dynamic
obstacles, multi-agent route deconfliction, real perception, PX4/SITL export,
hardware readiness, or a visual UI.

Load a realism scenario directly:

```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --scenario-suite scenarios/coverage.realism.json --output-dir results/coverage_realism/
```

### Expected Impact on Metrics

Realism typically degrades mission metrics relative to ideal conditions:

- **Coverage**: -5% to -15% due to pose noise and wind drift
- **SAR**: +10-30% detection time due to sensor noise and wind
- **Inspection**: -10% to -20% edge coverage due to battery drain and positioning errors
- **Wildfire**: -5% to -10% mapped ratio due to comms jitter and drift
- **Battery**: -10% to -25% margin due to hover/climb/cruise drain

Battery model metadata (`hover_drain_per_tick`, `climb_drain_per_meter`, `cruise_drain_per_meter`, `reserve_fraction`) is now included in `BenchmarkManifest` for reproducibility.

## Wildfire v2 (M38)

Wildfire is now a first-class mission with rich dynamic behavior and metrics.
Flood is future work and is not implemented as a separate mission in M63.

### Profiles

| Profile | Agents | Zones | Max Ticks | Dynamic Threat | Update Interval |
|---|---|---|---|---|---|
| `small-static` | 2 | 2 | 200 | No | 999 |
| `medium-dynamic` | 4 | 4 | 400 | Yes | 50 |
| `large-static` | 6 | 6 | 300 | No | 999 |
| `high-threat-dynamic` | 4 | 4 | 500 | Yes | 25 (fast escalation) |

### Dynamic Behavior

When `enable_dynamic_threat: true`:

- **Base escalation**: threat +0.1, priority +1 per update interval
- **Spatial spread**: zones with threat > 0.8 spread +0.05 to adjacent zones (when `enable_spatial_spread: true`)
- **Wind influence**: wind vector accelerates threat growth for high-threat zones
- **Significant jump**: threat increase > 0.2 in one update boosts priority by +2 instead of +1

### Metrics

- `hazard_zones_mapped` — total zones mapped
- `high_priority_zones_mapped` — zones with priority >= 5
- `time_to_map_first_high_risk` — tick when first high-priority zone was mapped
- `threat_level_over_time` — vector of average threat per tick
- `zone_observations` — total agent observations of zones
- `priority_updates` — count of task priority changes
- `final_avg_threat_level` — average threat at simulation end

### Scenario Files

- `scenarios/wildfire.small-static.json`
- `scenarios/wildfire.medium-dynamic.json`
- `scenarios/wildfire.realism.json`

## License

[MIT](LICENSE)
