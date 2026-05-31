# Project Status

**Date:** 2026-05-31
**HEAD commit:** see `git rev-parse HEAD`
**Last audit:** M67 Urban Replay / Analysis

This document is the current status summary for the repository. It supersedes
the older M39b-only audit and should be read together with the README current
status table.

## Milestone Status

| Milestone | Status | Notes |
|---|---|---|
| M32 Reporting & Metrics Hardening | Complete | Mixed-mission report identity, JSON/CSV/Markdown exports, and merged `all` benchmark id are implemented. |
| M33 Mission Semantics Integration | Complete | `TaskKind`, concrete adapters, `AdapterRegistry`, adapter-driven completion/scoring in runner and allocator. |
| M34 Planner Correctness v2 | Complete | `RoutePlanner`, 2-opt, battery-aware ordered-subset feasibility, and planner metrics are present. |
| M35 Dynamic Mission Correctness | Partial | Mission-specific success semantics are implemented; SAR CBBA and SAR centralized remain explicitly unsupported. |
| M36 Regression Harness v2 | Complete | Regression infrastructure exists; repeated release sweeps passed for `regression_runner` and `strategy_comparison --regression` at `jobs=1/4/14`. |
| M37 Realism Scenario Pack | Complete | Light/medium/heavy profiles and scenario metadata exist; this is not a calibrated research study. |
| M38 Wildfire v2 | Partial | Wildfire dynamic threat work exists; flood remains future work and is not a separate mission. |
| M39a Regression Repair | Complete | Runtime ordering and SAR scan-completion fixes removed the reproduced default regression flake; sweep artifacts are in `results/m56_regression_determinism_2026-05-30/`. |
| M39b Decision / Audit Report | Complete | Historical audit and README honesty pass. |
| M43 SITL Contract & Dry-Run Foundation | Complete | `--mock`, `--dry-run`, `--connection`, typed errors, connection classes, and waypoint extraction are implemented. |
| M44 MAVLink Mission Upload Protocol | Complete with debt removed | Mission upload handshake is implemented; generic `MavlinkTransport::send()` is explicitly unsupported instead of sending a fake `RAW_RPM`. |
| M45 Pre-upload Safety Validation | Complete | Safety config, default rules, subset validation, and actionable violations are implemented. |
| M46 Flight Sequence | Complete | Upload-only/execute lifecycle, arm/takeoff/start/abort command handling, and bounded failures are implemented. |
| M47 Telemetry Loop & TaskStatus Mapping | Complete | `MISSION_CURRENT`, `MISSION_ITEM_REACHED`, completion/rejection/disconnect/no-progress mapping are implemented. |
| M48 Single-Agent PX4 SITL Golden Path | Complete for local PX4 SIH | Live single-agent PX4 SIH run completed on 2026-05-30 with `scenarios/sitl.px4-golden.json`; report/replay artifacts are in `results/m48_px4_sitl_2026-05-30/`. |
| M49 SITL Observability & Replay | Complete | SITL event log, replay summary, task id mapping, failure events, and reallocation schema events are implemented. |
| M50 Mock Regression & Docs Hardening | Complete | Portable dry-run/mock/docs checks exist and require no PX4. |
| M51 Dynamic Reallocation for Failed Agent | Supervisor mock complete | Runtime reallocation, metrics, SITL event schema, and `sitl_supervisor --mock` heartbeat-timeout/reallocation flow are implemented. M59 reuses this runtime path for controlled live-supervisor active-survivor mission replacement. |
| M52 Multi-Agent SITL Foundation | Foundation plus upload-only check complete | `multi_sitl.v1`, public `scenarios/sitl.multi-agent.json` / `scenarios/sitl.multi-agent.config.json`, dry-run/mock manifest, task subsets, duplicate ownership checks, mock supervisor orchestration, and a two-instance PX4 SIH upload-only check exist. |
| M53 Hardware Readiness Boundary | Complete | `docs/HARDWARE_READINESS.md`, connection classes, and `--allow-hardware-candidate` guard hardware-candidate endpoints. |
| M57 Supervisor Controller Boundary | Complete | `sitl_supervisor` mock orchestration is extracted into a testable internal supervisor module with `AgentController`, `MockAgentController`, fake-controller tests over the shared supervisor loop, returned `SupervisorMetrics`, and expanded CLI negative tests. M58 builds the live PX4/SIH path beside this boundary. |
| M58 Live Multi-Agent PX4/SIH Execute Orchestration | Complete as experimental local SITL workflow | `sitl_supervisor --connection --execute` with `scenarios/sitl.multi-agent.execute.config.json` validates all live agents, rejects non-execute lifecycles, applies per-agent safety and hardware-candidate gates before upload, runs local PX4/SIH controllers under one supervisor, and writes a common SITL event log plus `sitl_multi_agent_run_report.v1`. A two-agent execute artifact is captured in `results/m58_multi_agent_px4_sih_execute_2026-05-31/`. |
| M59 Live PX4/SIH Failure & Reallocation | Complete as controlled local SITL workflow | `--reupload-on-failure` detects a failed active live agent, emits runtime release/reassignment events, aborts/replaces an active survivor mission, writes report `reallocation` metrics and replay summary counters, and has fake-controller coverage. A controlled PX4/SIH failure artifact is captured in `results/m59_px4_sih_failure_reallocation_2026-05-31/`. |
| M60 PX4/SIH Supervisor Hardening | Complete for local workflow hardening | `sitl_supervisor` now supports `--output-dir`, `--run-id`, and `--force`, refuses artifact overwrites by default, returns stable exit codes, writes replay summaries for output-dir runs, and extends `sitl_multi_agent_run_report.v1` with `task_ownership`, `events_summary`, `final_status`, and `limitations`. This hardens repeatable local PX4/SIH research runs, not hardware readiness. |
| M61 Platform / API Stabilization | Complete as in-repository extension guidance | `docs/EXTENSION_GUIDE.md` documents mission, strategy, metrics, crate boundaries, schema-version policy, and test-only extension fixtures. It is a stable-ish in-repository guide, not a semver-stable published API or hardware-readiness claim. |
| M62 Benchmark / Baseline Refresh | Complete as historical 500-seed validation baseline | Release `strategy_comparison --seeds 500 --mission all --jobs 14` completed for commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd`. Artifacts are in `results/all_500_jobs14_m62_release/`; after M63 this is historical validation evidence unless rerun on current HEAD, not a publication-grade 1000-seed statistical run. |
| M63 Evidence Cleanup / Status Honesty | Complete without benchmark rerun | README/status/benchmark docs mark the M62 pack as historical evidence for `81260ca7afa114a5d9add7b832f6c5d7875b88cd`, flood is future work, wildfire success semantics are documented/tested, and committed M58/M59 SITL artifacts have targeted replay/event sanity tests. |
| M64 Urban Foundations | Complete as stable substrate | `UrbanMap`, directed road graph nodes/edges, deterministic Dijkstra route-loop planning, AABB static obstacle judge, `urban-patrol` DSL validation, `scenarios/urban.patrol.json`, and Urban metrics skeleton are implemented. |
| M65 Urban Patrol v0 | Complete as simulation-only mission | One scout follows the ordered `urban-patrol` road-graph loop and succeeds only after traversing every planned segment before timeout with zero Urban judge violations. The runner emits Urban replay events and reports patrol completion/time/distance/efficiency metrics. M65 itself has no bus detection; M66 adds mocked bus search separately. Lidar/raycast, dynamic obstacles, multi-agent route deconfliction, PX4/SITL export, hardware claims, visual UI, and publication benchmark evidence remain future work. |
| M66 Urban Search v1 | Complete as simulation-only mission | One scout follows the Urban road graph and evaluates a deterministic mocked bus detector. `urban-search` DSL validation, `scenarios/urban.search.json`, bus observation/detection/false-positive/search-completion replay events, bus detection/time/false-positive/distance metrics, focused reports, and a smoke regression gate are implemented. Lidar/raycast, dynamic obstacles, real perception, multi-agent deconfliction, PX4/SITL export, hardware claims, visual UI, and publication benchmark evidence remain future work. |
| M67 Urban Replay / Analysis | Complete as diagnostic tooling | Simulation replay now supports deterministic timeline output with `--agent` / `--category urban` filters, additive `UrbanViolation.obstacle_id`, route-trace and judge-report JSON/CSV artifacts for Urban benchmark packs, a two-agent analysis fixture in `scenarios/urban.multi-agent.json`, and diagnostic Urban separation/conflict aggregate metrics. This adds observability only; it does not add avoidance, multi-agent Urban control, real perception, lidar/raycast, PX4/SITL export, hardware claims, or a benchmark rerun. |

## Current Known Limitations

### SITL / PX4

- **M48 is live-verified on local PX4 SIH, not Gazebo or hardware.** The
  repository contains a 2026-05-30 PX4 SIH result with version/backend/command,
  report JSON, and replay summary. It does not prove Gazebo behavior, HIL, real
  aircraft, or production safety.
- **Multi-agent PX4/SIH execute orchestration exists as a local experimental
  workflow.** M52 proves explicit ownership, per-agent commands, mock
  supervisor orchestration, and a local two-instance PX4 SIH upload-only check.
  M58 adds `sitl_supervisor --connection --execute` for local endpoints with a
  common report/event log and a captured two-agent execute artifact. This still
  is not PX4 CI, Gazebo/HIL, real hardware, or production flight orchestration.
- **M59 reallocation is controlled local SITL, not production failover.**
  Reallocation events are emitted by `sitl_supervisor --mock`, by fake
  live-supervisor tests, and by a controlled local PX4/SIH failure-injection
  artifact. The live supervisor starts local agents, polls them stepwise,
  detects a failed active agent, releases unfinished tasks through runtime
  reallocation, and aborts/replaces an active survivor mission. This remains a
  narrow local SIH workflow, not automated PX4 CI, Gazebo/HIL, hardware, or
  onboard distributed autonomy.
- **M57 was an internal boundary; M58 is the first live supervisor plumbing.**
  The mock supervisor state machine remains testable behind an internal
  controller boundary, while M58 adds the separate live PX4/SIH execute path and
  M59 adds explicit mission replacement after failed-agent reallocation.
- **M60 hardens repeatability and failure diagnosis.** Local supervisor runs can
  use `--output-dir` and `--run-id` to produce a predictable run directory with
  manifest, event log, run report, and replay summary. Existing artifacts are
  protected unless `--force` is explicit, and stable exit codes separate
  CLI/config, safety, endpoint/feature, upload/command, timeout, runtime, and
  artifact failures.
- **Hardware is out of scope.** The project is not flight-certified and is not a
  production safety layer.

### Regression / Benchmarks

- **Default regression determinism sweep passed after fixes.** The sweep covered
  `regression_runner` and `strategy_comparison --regression` at `jobs=1/4/14`
  with repeated runs. Artifacts are in
  `results/m56_regression_determinism_2026-05-30/`.
- **M62 refreshed simulation benchmark evidence for commit
  `81260ca7afa114a5d9add7b832f6c5d7875b88cd`.** The 500-seed release baseline
  is in `results/all_500_jobs14_m62_release/` and is summarized in
  `docs/BENCHMARK_RESULTS.md`. M63 did not rerun the benchmark, so the pack is
  historical evidence rather than current-HEAD evidence.
- **1000-seed benchmark is still not an M48 substitute.** It can evaluate
  simulation behavior, but live PX4 SITL requires the M48 manual run.
- **Publication-level benchmark remains separate.** M62 is a historical
  validation baseline; a future publication claim should use a fresh 1000-seed
  run after SAR, wildfire, and CBBA interpretation questions are resolved.

### Algorithmic

- **SAR CBBA**: unsupported due to delayed reconvergence after task release.
- **SAR Centralized**: unsupported because static pre-planning is incompatible
  with dynamic belief search.
- **Inspection perimeter**: constrained by battery/time and intentionally
  experimental for some strategies.
- **Flood mission**: future work; not implemented as a separate mission.
- **Wildfire success semantics**: `success=true` requires
  `mapped_zone_count / total_zone_count >= wildfire_success_threshold` (default
  `0.8`), all expected failures detected, and
  `max_task_unassigned_ticks <= max_unassigned_ticks`. This is intentionally
  stricter than task completion, so wildfire benchmark rows can show
  `Completion = 1.000` while `Success < 1.000`.
- **Urban Patrol v0**: M65 implements one-agent ordered road-graph patrol in
  simulation. Completion means all planned segments are traversed before
  timeout with zero Urban judge violations. This is not lidar, real obstacle
  avoidance, dynamic traffic, multi-agent deconfliction, PX4/SITL export, or
  hardware readiness.
- **Urban Search v1**: M66 adds one-agent mocked bus search on top of the same
  road graph. The detector is deterministic and distance/probability based; it
  is not lidar/raycast, computer vision, dynamic traffic, multi-agent
  deconfliction, PX4/SITL export, hardware readiness, or real perception.
- **Urban Replay / Analysis**: M67 makes Urban runs easier to inspect. Replay
  timeline output can be filtered by agent or Urban event category, benchmark
  packs can include route-trace and judge-report JSON/CSV artifacts, and
  `scenarios/urban.multi-agent.json` provides a deterministic two-agent
  analysis fixture. The separation/conflict metrics are diagnostic
  measurements from replay traces, not a route-deconfliction or collision
  avoidance system.

### Platform / API

- **Extension points are documented, not published as an SDK.** M61 documents
  stable-ish in-repository paths for missions, strategies, metrics, and schema
  changes. It does not promise semver-stable public API, crate publication, or
  external plugin compatibility.
- **Test-only extension fixtures are not supported features.** The M61 adapter,
  strategy, and runner fixtures validate contracts but are not new real
  missions or benchmark strategies.

## Readiness

| Goal | Status | Blocker |
|---|---|---|
| Portable SITL verification | Ready | Run `sitl_agent`/`sitl_docs` targeted tests. |
| In-repository extension work | Ready with M61 boundaries | Use `docs/EXTENSION_GUIDE.md`; external semver-stable plugin/API work remains out of scope. |
| Urban analysis work | Ready for M68+ follow-up | M67 provides timeline filters, route trace and judge report artifacts, obstacle ids, a two-agent analysis fixture, and diagnostic separation/conflict metrics. Dynamic obstacles, richer judging, route deconfliction, and avoidance remain future work. |
| M48 live PX4 verification | Complete for local PX4 SIH | Captured in `results/m48_px4_sitl_2026-05-30/`; Gazebo/HIL/hardware remain out of scope. |
| Real multi-agent PX4/SIH | Experimental local workflow with M60 hardening | Upload-only, execute, and controlled failure/reallocation SIH evidence exists. `sitl_supervisor --connection --execute --reupload-on-failure --output-dir ... --run-id ...` can produce stable artifacts and exit codes for local runs; automated PX4 CI, Gazebo/HIL, hardware, broader failure modes, and production safety remain future work. |
| Large benchmark publication | Not ready | M62 gives a historical 500-seed validation baseline for commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd`; current-head publication-level evidence still needs a fresh run and interpretation of SAR/wildfire/CBBA rows. |
| Hardware experiment | Not product-ready | Requires external safety process; see `docs/HARDWARE_READINESS.md`. |

## Recommended Next Steps

1. Use `docs/EXTENSION_GUIDE.md` when adding the next in-repository mission,
   strategy, metric, or schema field. Keep the support matrix and regression
   coverage in the same change.
2. Use M60 `--output-dir` / `--run-id` for any new local PX4/SIH evidence so
   artifacts are repeatable and overwrite-safe.
3. Expand the local M59 workflow only if the project needs broader failure
   modes, repeated failure recovery, or automated PX4/SIH orchestration.
4. Inspect M62 SAR, wildfire, and CBBA benchmark interpretation gaps before
   making publication-level algorithm claims.
5. Rerun the benchmark only when refreshing current-head evidence; use 1000
   seeds only after those interpretation gaps are resolved or explicitly marked
   unsupported.
6. Keep README, `docs/BENCHMARK_RESULTS.md`, `docs/EXTENSION_GUIDE.md`,
   `docs/SITL_SETUP.md`, `docs/SCENARIO_DSL.md`, `docs/REPLAY.md`, and this
   file in sync when extension, Urban analysis, or SITL evidence changes state.

## How to Verify This Status

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-types adapter

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-alloc strategy

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim extension_fixture

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim urban

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-scenarios urban

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim --test scenario_catalog

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-replay timeline

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim urban_analysis

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-comms --features mavlink-transport

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-runtime reallocation

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_supervisor

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_connection
```

For M48 live verification, inspect `results/m48_px4_sitl_2026-05-30/`. For the
two-instance PX4 SIH upload-only check, inspect
`results/m55_multi_agent_px4_sih_2026-05-30/`. For M58/M59, inspect
`results/m58_multi_agent_px4_sih_execute_2026-05-31/` and
`results/m59_px4_sih_failure_reallocation_2026-05-31/`. M60 adds local
supervisor artifact/exit-code/report hardening. M61 adds the extension guide
and test-only extension contract checks. M62 adds a 500-seed release simulation
benchmark baseline for commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd`; M63
marks it historical because no current-HEAD rerun was performed. M64 adds Urban
foundation code and docs, M65 adds Urban Patrol v0 simulation semantics, M66
adds Urban Search v1 simulation semantics with a mocked bus detector, and M67
adds Urban replay/analysis diagnostics; none of these milestones refreshes the
benchmark evidence. Do not extend
any existing result to Gazebo, HIL, real hardware, automated PX4 CI,
semver-stable external API, or publication-level algorithm claims without new
code/evidence.
