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

### 6. Run mock SITL

```bash
cargo run --bin sitl_agent -- \
  --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0
```

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
| Replay / Debuggability | ✅ Stable | M23 | `replay` CLI, ASCII visualization |
| Mission Semantics | ✅ Stable | M27 | `TaskKind`, `MissionAdapter`, `allocate_with_adapter` |
| Planner Quality | ✅ Stable | M28 | `RoutePlanner` trait, 2-opt, battery-aware feasibility |
| Real PX4 | 🧪 Experimental | M20 | Feature-gated, requires PX4 SITL setup |

**Test coverage:** 250+ tests, 10 crates, 12 JSON scenarios.

---

## Known Limitations

1. **Simulation only:** No real hardware integration beyond experimental PX4 SITL scaffold.
2. **Single-agent SITL:** Multi-agent SITL not yet supported.
3. **2D world:** All scenarios operate in 2D (x, y) with fixed altitude.
4. **Deterministic RNG:** Scenarios use seeded RNG; real-world noise is not modeled.
5. **Simplified kinematics:** Battery drain is proportional to distance, not accounting for hover/climb.

See [Strategy Support Matrix](#strategy-support-matrix) for per-strategy known limitations.

---

## Strategy Support Matrix

| Mission | Strategy | Status | Notes |
|---------|----------|--------|-------|
| coverage | all | stable | All strategies produce success_rate > 0.9 on ideal/standard profiles |
| sar | greedy, auction, connectivity-aware | stable | — |
| sar | cbba | unsupported | CBBA re-convergence delay after `release_task()` exceeds `max_unassigned_ticks`; fix scoped to M27 |
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
- **Not ready for real-world swarm flights.** Simulation-only with experimental SITL scaffold.
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
| `swarm-scenarios` | Scenario builders: Coverage, Emergency Mesh, SAR, Infrastructure Inspection. |
| `swarm-metrics` | Per-run and aggregate metrics. |
| `swarm-replay` | Event log, replay engine, summary CLI, ASCII visualization. |
| `swarm-safety` | Safety layer: geofence, no-fly zones, separation constraints. |
| `swarm-examples` | Runnable binaries: `strategy_comparison`, `sitl_agent`, `replay`. |

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

---

## Docs

| Document | Description |
|---|---|
| [`docs/BENCHMARK_RESULTS.md`](docs/BENCHMARK_RESULTS.md) | Real benchmark numbers and analysis |
| [`docs/SCENARIO_DSL.md`](docs/SCENARIO_DSL.md) | Scenario suite format and validation |
| [`docs/REPLAY.md`](docs/REPLAY.md) | Replay event log schema and CLI usage |
| [`docs/SITL_SETUP.md`](docs/SITL_SETUP.md) | Mock and real PX4 SITL setup |

---

## Build

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

## License

[MIT](LICENSE)
