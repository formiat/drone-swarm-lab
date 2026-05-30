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
| M36 Regression Harness v2 | Partial | Regression infrastructure exists, and bounded `jobs=1` diagnostics pass on the current tree. A repeated jobs/seed-count determinism sweep is still pending before using it as a release gate. |
| M37 Realism Scenario Pack | Complete | Light/medium/heavy profiles and scenario metadata exist; this is not a calibrated research study. |
| M38 Wildfire v2 | Partial | Wildfire dynamic threat work exists; flood is still not a separate mission. |
| M39a Regression Repair | Partial | Earlier entrypoint repair exists; current bounded `jobs=1` entrypoint checks pass, but broader determinism work remains pending. |
| M39b Decision / Audit Report | Complete | Historical audit and README honesty pass. |
| M43 SITL Contract & Dry-Run Foundation | Complete | `--mock`, `--dry-run`, `--connection`, typed errors, connection classes, and waypoint extraction are implemented. |
| M44 MAVLink Mission Upload Protocol | Complete with debt removed | Mission upload handshake is implemented; generic `MavlinkTransport::send()` is explicitly unsupported instead of sending a fake `RAW_RPM`. |
| M45 Pre-upload Safety Validation | Complete | Safety config, default rules, subset validation, and actionable violations are implemented. |
| M46 Flight Sequence | Complete | Upload-only/execute lifecycle, arm/takeoff/start/abort command handling, and bounded failures are implemented. |
| M47 Telemetry Loop & TaskStatus Mapping | Complete | `MISSION_CURRENT`, `MISSION_ITEM_REACHED`, completion/rejection/disconnect/no-progress mapping are implemented. |
| M48 Single-Agent PX4 SITL Golden Path | Code complete, live verification pending | Report/replay/golden fake path exists. Public `scenarios/sitl.px4-golden.json` has explicit altitudes for the manual live PX4 run. |
| M49 SITL Observability & Replay | Complete | SITL event log, replay summary, task id mapping, failure events, and reallocation schema events are implemented. |
| M50 Mock Regression & Docs Hardening | Complete | Portable dry-run/mock/docs checks exist and require no PX4. |
| M51 Dynamic Reallocation for Failed Agent | Runtime/mock boundary complete | Runtime reallocation and metrics exist; SITL log has reallocation event schema. Live multi-agent PX4 supervisor reallocation flow is not wired. |
| M52 Multi-Agent SITL Foundation | Foundation complete | `multi_sitl.v1`, public `scenarios/sitl.multi-agent.json` / `scenarios/sitl.multi-agent.config.json`, dry-run/mock manifest, task subsets, and duplicate ownership checks exist. |
| M53 Hardware Readiness Boundary | Complete | `docs/HARDWARE_READINESS.md`, connection classes, and `--allow-hardware-candidate` guard hardware-candidate endpoints. |

## Current Known Limitations

### SITL / PX4

- **Live M48 PX4 result is pending.** The code path is present, but the
  repository does not yet contain a verified PX4 version/backend/command/report.
- **Multi-agent PX4 is a foundation, not orchestration.** M52 proves explicit
  ownership and per-agent commands; it does not launch and coordinate multiple
  real PX4 instances automatically.
- **M51 reallocation events are schema/API/runtime covered.** They are not yet
  emitted by a live multi-agent PX4 supervisor failure flow.
- **Hardware is out of scope.** The project is not flight-certified and is not a
  production safety layer.

### Regression / Benchmarks

- **Prior default regression flake was not reproduced in bounded checks.**
  `regression_runner --jobs 1` and `strategy_comparison --regression --jobs 1`
  passed on the current tree. A repeated jobs/seed-count determinism sweep is
  still required before using default regression status as a release gate.
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
| M48 live PX4 verification | Ready for manual attempt | Requires local PX4 SITL environment and result capture. |
| Real multi-agent PX4 | Not ready | Needs supervisor orchestration and live failure/reallocation design. |
| Large benchmark publication | Not ready | Regression flake and baseline freshness must be resolved first. |
| Hardware experiment | Not product-ready | Requires external safety process; see `docs/HARDWARE_READINESS.md`. |

## Recommended Next Steps

1. Run the M48 live PX4 SITL check with `scenarios/sitl.px4-golden.json`, then
   record PX4 version/backend/command/report/replay summary.
2. Run a repeated regression determinism sweep across jobs/seed-count variants
   before relying on benchmark gates or long seed runs.
3. Decide whether M51 should stay at "runtime/mock/schema covered" or become a
   separate live multi-agent supervisor reallocation milestone.
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
```

For M48 live verification, use the command in `docs/SITL_SETUP.md` and record the
actual PX4 setup details. Do not mark M48 live-verified from mock/fake tests.
