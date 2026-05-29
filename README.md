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
```

### 6. Run wildfire / flood mapping benchmark

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

### 8. Run mock SITL

```bash
cargo run --bin sitl_agent -- \
  --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0
```

### 9. Upload a mission to PX4 SITL

```bash
cargo run -p swarm-examples --bin sitl_agent --features mavlink-transport -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0 \
  --safety-config path/to/sitl-safety.json
```

This is an experimental waypoint upload path for PX4 SITL. It waits for a
MAVLink heartbeat, validates the mission before upload, optionally clears the
existing mission, sends mission count, responds to `MISSION_REQUEST_INT` or
legacy `MISSION_REQUEST`, sends `MISSION_ITEM_INT` waypoints, and requires an
accepted `MISSION_ACK`. If `--safety-config` is omitted, conservative SITL
defaults are used.

---

## Current Status

| Feature | Status | Since | Notes |
|---|---|---|---|
| Benchmark (smoke/quick/full) | ✅ Stable | M21 | `--output-dir`, `--report`, `BenchmarkManifest` |
| Mission DSL | ✅ Stable | M19 | `schema_version: "0.1"`, validation API |
| Safety Layer | ✅ Stable | M20 | `SafetyAllocator` wrapper, no-fly/geofence/separation |
| SAR v2 | ✅ Stable | M14 | `BeliefMap`, sensor noise, confirmation scans |
| CBBA Robustness | ✅ Stable | M15 | Convergence metrics, TSP ordering, retransmission |
| Infrastructure Inspection | ✅ Stable | M16 | Edge coverage, route efficiency |
| Mock SITL | ✅ Stable | M20 | `sitl_agent --mock`, no external deps |
| SITL Dry-Run | ✅ Stable | M43 | `sitl_agent --dry-run`, portable mission upload plan without PX4 |
| Replay / Debuggability | ✅ Stable | M23 | `replay` CLI, ASCII visualization |
| Mission Semantics | ✅ Stable | M33 | `TaskKind`, 6 concrete adapters, `AdapterRegistry`, adapter-driven completion/scoring in runner and allocator |
| Planner Quality | ✅ Stable | M34 | `RoutePlanner` trait, 2-opt, battery-aware feasibility v2 (ordered-subset feasibility, battery model v2 integration, meaningful runner metrics) |
| Dynamic Mission Correctness | ✅ Stable | M35 | Mission-specific success semantics (SAR=targets-found, inspection=coverage-threshold, wildfire=mapped-ratio), SAR unsupported reasons (cbba=delayed-reconvergence, centralized=static-pre-plan), support matrix tests |
| Regression Harness v2 | ✅ Stable | M36 | Calibrated thresholds, portability fixes, wildfire/realism suites, failure delta output; fully stable after M39a repair |
| Realism Scenario Pack | ✅ Stable | M37 | Realism profiles (light/medium/heavy), scenario JSONs, battery model metadata, baseline vs realism comparison |
| Wildfire v2 | ⚠️ Partial | M38 | Spatial spread, wind influence, zone expansion, high-priority metrics, replay integration, scenario JSONs; flood not implemented as separate mission |
| Decision / Audit Report | ✅ Stable | M39b | Status audit, README honesty update, benchmark docs marked historical |
| Regression Repair | ✅ Stable | M39a | Unified regression entrypoints, fixed wildfire/realism in `strategy_comparison --regression`, removed duplication between binaries |
| Wildfire / Flood Mapping | ✅ Stable | M30 | `TaskKind::MappingZone`, `WildfireState`, hazard zones, dynamic threat |
| Simulation Realism | ✅ Stable | M31 | Battery model v2, altitude sensor penalty, wind drift, pose noise, comms jitter, time-gated no-fly zones, `--realism` preset |
| Reporting & Metrics | ✅ Stable | M32 | Per-row mission/scenario in exports, mission-scoped profiles, merged `all` benchmark id, wildfire/planner metrics, realism metadata in manifest |
| Real PX4 | 🧪 Experimental | M45 | Feature-gated PX4 SITL mission upload with pre-upload safety validation; no arm/takeoff/execution supervision |

**Test coverage:** 360+ tests, 10 crates, 18 JSON scenarios.

> **Project Status:** For an honest audit of what is fully complete vs partially complete, see [`docs/STATUS.md`](docs/STATUS.md).

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

### Default Suites (M36)

| Suite | Mission | Profile | Strategy | Mode | Key Thresholds |
|---|---|---|---|---|---|
| `sar_ideal_greedy` | sar | ideal | greedy | smoke | task_completion_rate ≥ 0.80, belief_entropy_final ≤ 0.5 |
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

**Modes:** `smoke` = 1 seed; `quick` = 10 seeds. **SAR and wildfire** use `task_completion_rate` (not `success_rate`) because M35 changed success semantics to mission-specific definitions.

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

1. **Simulation only:** No real hardware workflow; PX4 integration is limited to experimental SITL waypoint upload with static pre-upload safety checks.
2. **Single-agent SITL:** Multi-agent SITL not yet supported.
3. **SITL coordinate frame:** `sitl_agent` dry-run/mock mode treats `Pose { x, y, z }` as local simulation coordinates; `x/y` are not WGS84 latitude/longitude, and `z` is local altitude.
4. **3D pose:** Scenarios support `z` coordinate and altitude-aware sensors, but most missions operate primarily in XY plane.
5. **Deterministic RNG:** Scenarios use seeded RNG; real-world noise is modeled optionally via `--realism` preset.
6. **Battery model v2:** Hover/climb/cruise drain rates are configurable but not calibrated against real flight data.
7. **Regression smoke variance:** Smoke suites use 1 seed; high-variance missions (SAR, emergency-mesh, wildfire) have conservative thresholds. Promote to `Quick` (10 seeds) for tighter coverage.

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

**Status meanings:**
- **stable** — success_rate > 0 across standard seeds; suitable for benchmarking.
- **experimental** — works but constrained by battery/time or algorithmic limits; use with awareness.
- **unsupported** — consistently 0% success due to a known root cause; tracked for future milestones.

---

## Non-Goals

- **Not a production flight-control system.** This is a research prototype for coordination algorithms.
- **Not a certified safety layer.** Safety constraints are checked but not formally verified.
- **Not ready for real-world swarm flights.** Simulation-only with experimental PX4 SITL mission upload and static pre-upload validation.
- **Not a MAVLink ground control station.** PX4 integration is experimental and minimal.

---

## Workspace Layout

| Crate | Purpose |
| --- | --- |
| `swarm-types` | Shared IDs, agent/task/message types, pose, velocity, mission semantics (`TaskKind`, `MissionAdapter`). |
| `swarm-comms` | Transport trait, in-memory network, UDP transport, MAVLink transport (optional). |
| `swarm-runtime` | Membership, failure detection, task registry, coordinator, `AgentNode`. |
| `swarm-alloc` | Greedy, auction, connectivity-aware, centralized, CBBA allocation strategies. |
| `swarm-sim` | Deterministic clock, scenario model, generic scenario runner, DSL loader, JSON/CSV export. |
| `swarm-scenarios` | Scenario builders: Coverage, Emergency Mesh, SAR, Infrastructure Inspection, Wildfire / Flood Mapping. |
| `swarm-metrics` | Per-run and aggregate metrics. |
| `swarm-replay` | Event log, replay engine, summary CLI, ASCII visualization. |
| `swarm-safety` | Safety layer: geofence, no-fly zones, separation constraints. |
| `swarm-examples` | Runnable binaries: `strategy_comparison`, `regression_runner`, `sitl_agent`, `replay`. |

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
| M30 | ✅ | New Mission Prototype: Wildfire / Flood Mapping with `TaskKind::MappingZone`, hazard zones, dynamic threat |
| M31 | ✅ | Simulation Realism Foundation: battery model v2, altitude sensor penalty, wind drift, pose noise, comms jitter, time-gated no-fly zones |
| M32 | ✅ | Benchmark Identity Hardening: per-row mission/scenario in exports, mission-scoped profiles, merged `all` benchmark id, realism metadata in manifest |
| M33 | ✅ | Mission Semantics Integration: 6 concrete adapters, `AdapterRegistry`, adapter-driven completion/scoring in runner and allocator, DSL kind validation |
| M34 | ✅ | Planner Correctness v2: `RoutePlanner` trait, 2-opt, battery-aware feasibility (ordered-subset), meaningful route metrics |
| M35 | ✅ | Dynamic Mission Correctness: mission-specific success semantics, SAR unsupported reasons, support matrix tests |
| M36 | ✅ | Regression Harness v2: calibrated thresholds, wildfire/realism suites, portable tests (tempdir), failure delta output, refreshed baseline |

---

## Docs

| Document | Description |
|---|---|
| [`docs/BENCHMARK_RESULTS.md`](docs/BENCHMARK_RESULTS.md) | Real benchmark numbers and analysis |
| [`docs/SCENARIO_DSL.md`](docs/SCENARIO_DSL.md) | Scenario suite format and validation |
| [`docs/REPLAY.md`](docs/REPLAY.md) | Replay event log schema and CLI usage |
| [`docs/SITL_SETUP.md`](docs/SITL_SETUP.md) | Mock, dry-run, and experimental PX4 SITL setup |

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

## Wildfire / Flood v2 (M38)

Wildfire is now a first-class mission with rich dynamic behavior and metrics.

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
