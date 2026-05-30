# Project Status

**Date:** 2026-05-30
**HEAD commit:** see `git rev-parse HEAD`
**Last audit:** M59 Live PX4/SIH Failure & Reallocation partial foundation

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
| M38 Wildfire v2 | Partial | Wildfire dynamic threat work exists; flood is still not a separate mission. |
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
| M51 Dynamic Reallocation for Failed Agent | Supervisor mock complete | Runtime reallocation, metrics, SITL event schema, and `sitl_supervisor --mock` heartbeat-timeout/reallocation flow are implemented. M59 reuses this runtime path for a controlled live-supervisor foundation mission replacement. |
| M52 Multi-Agent SITL Foundation | Foundation plus upload-only check complete | `multi_sitl.v1`, public `scenarios/sitl.multi-agent.json` / `scenarios/sitl.multi-agent.config.json`, dry-run/mock manifest, task subsets, duplicate ownership checks, mock supervisor orchestration, and a two-instance PX4 SIH upload-only check exist. |
| M53 Hardware Readiness Boundary | Complete | `docs/HARDWARE_READINESS.md`, connection classes, and `--allow-hardware-candidate` guard hardware-candidate endpoints. |
| M57 Supervisor Controller Boundary | Complete | `sitl_supervisor` mock orchestration is extracted into a testable internal supervisor module with `AgentController`, `MockAgentController`, fake-controller tests over the shared supervisor loop, returned `SupervisorMetrics`, and expanded CLI negative tests. M58 builds the live PX4/SIH path beside this boundary. |
| M58 Live Multi-Agent PX4/SIH Execute Orchestration | Complete as experimental local SITL plumbing | `sitl_supervisor --connection --execute` with `scenarios/sitl.multi-agent.execute.config.json` validates all live agents, rejects non-execute lifecycles, applies per-agent safety and hardware-candidate gates before upload, runs sequential local PX4/SIH controllers, and writes a common SITL event log plus `sitl_multi_agent_run_report.v1`. |
| M59 Live PX4/SIH Failure & Reallocation | Partial foundation | `--reupload-on-failure` turns a terminal failed live-agent run into runtime release/reassignment events, pending-survivor mission replacement, report `reallocation` metrics, and replay summary counters. The full stepwise live loop, active-survivor abort/clear/upload/execute replacement, and real PX4/SIH failure-injection artifact remain follow-up work. |

## Current Known Limitations

### SITL / PX4

- **M48 is live-verified on local PX4 SIH, not Gazebo or hardware.** The
  repository contains a 2026-05-30 PX4 SIH result with version/backend/command,
  report JSON, and replay summary. It does not prove Gazebo behavior, HIL, real
  aircraft, or production safety.
- **Multi-agent PX4/SIH execute orchestration exists as local experimental
  plumbing.** M52 proves explicit ownership, per-agent commands, mock
  supervisor orchestration, and a local two-instance PX4 SIH upload-only check.
  M58 adds `sitl_supervisor --connection --execute` for local endpoints with a
  common report/event log. This still is not PX4 CI, Gazebo/HIL, real hardware,
  or production flight orchestration.
- **M59 reallocation is a controlled foundation, not the full live loop.**
  Reallocation events are emitted by `sitl_supervisor --mock` and by the fake
  live-supervisor M59 path after a terminal failed agent run. The current live
  supervisor is still one-shot/sequential: it can update a pending survivor
  before that survivor starts, but it cannot poll multiple active agents,
  detect loss during active execution, or abort/clear/upload/execute a mission
  replacement for an already-running survivor. A captured real PX4/SIH
  failure-injection artifact is also still not present.
- **M57 was an internal boundary; M58 is the first live supervisor plumbing.**
  The mock supervisor state machine remains testable behind an internal
  controller boundary, while M58 adds the separate live PX4/SIH execute path and
  M59 adds explicit mission replacement after failed-agent reallocation.
- **Hardware is out of scope.** The project is not flight-certified and is not a
  production safety layer.

### Regression / Benchmarks

- **Default regression determinism sweep passed after fixes.** The sweep covered
  `regression_runner` and `strategy_comparison --regression` at `jobs=1/4/14`
  with repeated runs. Artifacts are in
  `results/m56_regression_determinism_2026-05-30/`.
- **1000-seed benchmark is not an M48 substitute.** It can evaluate simulation
  behavior, but live PX4 SITL requires the M48 manual run.
- **Historical benchmark docs may be stale.** Treat `docs/BENCHMARK_RESULTS.md`
  as historical unless refreshed for the current HEAD.

### Algorithmic

- **SAR CBBA**: unsupported due to delayed reconvergence after task release.
- **SAR Centralized**: unsupported because static pre-planning is incompatible
  with dynamic belief search.
- **Inspection perimeter**: constrained by battery/time and intentionally
  experimental for some strategies.
- **Flood mission**: not implemented as a separate mission.

## Readiness

| Goal | Status | Blocker |
|---|---|---|
| Portable SITL verification | Ready | Run `sitl_agent`/`sitl_docs` targeted tests. |
| M48 live PX4 verification | Complete for local PX4 SIH | Captured in `results/m48_px4_sitl_2026-05-30/`; Gazebo/HIL/hardware remain out of scope. |
| Real multi-agent PX4/SIH | Experimental local workflow with M59 gap | Upload-only SIH evidence exists and `sitl_supervisor --connection --execute --reupload-on-failure` can orchestrate controlled one-shot execute/reallocation foundation attempts; stepwise live loss detection, active-survivor mission replacement, PX4 CI, Gazebo/HIL, hardware, and a captured real failure-injection artifact remain future work. |
| Large benchmark publication | Not ready | Default regression flake is fixed; benchmark baselines/results still need a fresh publication run. |
| Hardware experiment | Not product-ready | Requires external safety process; see `docs/HARDWARE_READINESS.md`. |

## Recommended Next Steps

1. Implement the stepwise M59 follow-up if the project needs true live
   reallocation during active multi-agent execution.
2. Capture a real local PX4/SIH M59 failure-injection artifact only after the
   stepwise/live boundary is implemented or explicitly out of scope.
3. Refresh benchmark baselines/results before using them as publication claims.
4. Keep README, `docs/SITL_SETUP.md`, `docs/REPLAY.md`, and this file in sync
   when M48 live verification changes state.

## How to Verify This Status

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs

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
`results/m55_multi_agent_px4_sih_2026-05-30/`. M58 adds live execute supervisor
code and portable fake/CLI coverage; M59 adds a partial failed-agent mission
replacement foundation and portable fake coverage. Do not extend any existing
result to Gazebo, HIL, real hardware, real PX4/SIH failure/reallocation, or
stepwise active-survivor replacement without new code/evidence.
