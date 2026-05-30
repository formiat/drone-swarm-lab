# Project Status

**Date:** 2026-05-30
**HEAD commit:** see `git rev-parse HEAD`
**Last audit:** M43-M53 Real SITL / PX4 follow-up

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
| M51 Dynamic Reallocation for Failed Agent | Supervisor mock complete | Runtime reallocation, metrics, SITL event schema, and `sitl_supervisor --mock` heartbeat-timeout/reallocation flow are implemented. Live PX4 failure/reallocation remains future work. |
| M52 Multi-Agent SITL Foundation | Foundation plus upload-only check complete | `multi_sitl.v1`, public `scenarios/sitl.multi-agent.json` / `scenarios/sitl.multi-agent.config.json`, dry-run/mock manifest, task subsets, duplicate ownership checks, mock supervisor orchestration, and a two-instance PX4 SIH upload-only check exist. |
| M53 Hardware Readiness Boundary | Complete | `docs/HARDWARE_READINESS.md`, connection classes, and `--allow-hardware-candidate` guard hardware-candidate endpoints. |

## Current Known Limitations

### SITL / PX4

- **M48 is live-verified on local PX4 SIH, not Gazebo or hardware.** The
  repository contains a 2026-05-30 PX4 SIH result with version/backend/command,
  report JSON, and replay summary. It does not prove Gazebo behavior, HIL, real
  aircraft, or production safety.
- **Multi-agent PX4 is upload-only verified, not flight orchestration.** M52
  proves explicit ownership, per-agent commands, mock supervisor orchestration,
  and a local two-instance PX4 SIH upload-only check. It does not launch,
  execute, and coordinate multiple real PX4 missions as a single autonomous
  flight workflow.
- **M51 reallocation is live in the mock supervisor, not in PX4.** Reallocation
  events are emitted by `sitl_supervisor --mock` after heartbeat timeout. They
  are not yet emitted by a live multi-agent PX4 supervisor failure flow.
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
| Real multi-agent PX4 | Partial | Upload-only local PX4 SIH check exists; live execute orchestration and PX4 failure/reallocation remain future work. |
| Large benchmark publication | Not ready | Default regression flake is fixed; benchmark baselines/results still need a fresh publication run. |
| Hardware experiment | Not product-ready | Requires external safety process; see `docs/HARDWARE_READINESS.md`. |

## Recommended Next Steps

1. Decide whether the next PX4 milestone should cover live multi-agent execute
   orchestration, live PX4 failure/reallocation, or HIL/hardware boundary work.
2. Refresh benchmark baselines/results before using them as publication claims.
3. Keep README, `docs/SITL_SETUP.md`, `docs/REPLAY.md`, and this file in sync
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
```

For M48 live verification, inspect `results/m48_px4_sitl_2026-05-30/`. For the
two-instance PX4 SIH upload-only check, inspect
`results/m55_multi_agent_px4_sih_2026-05-30/`. Do not extend either result to
Gazebo, HIL, real hardware, multi-agent execute orchestration, or live PX4
failure/reallocation without a new captured run.
