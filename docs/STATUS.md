# Project Status

**Date:** 2026-05-28
**HEAD commit:** see `git rev-parse HEAD`
**Last audit:** M39b

This document provides an honest audit of the project state after M32-M39a. It complements the README Current Status table with nuance that does not fit in a single-line summary.

---

## Milestone Status

| Milestone | Status | Notes |
|---|---|---|
| M32 Reporting & Metrics Hardening | ✅ Complete | Mixed-mission report identity fixed; JSON/CSV/Markdown exports correct; `--mission all` produces valid benchmark packs |
| M33 Mission Semantics Integration | ✅ Complete | 6 concrete adapters, `AdapterRegistry`, adapter-driven completion/scoring in runner and allocator; DSL kind validation |
| M34 Planner Correctness v2 | ✅ Complete | `RoutePlanner` trait, 2-opt, battery-aware feasibility; unit tests present; impact is real but narrow (primarily CBBA) |
| M35 Dynamic Mission Correctness | ⚠️ Partial | Mission-specific success semantics implemented (SAR=targets-found, inspection=coverage, wildfire=mapped-ratio); SAR CBBA/centralized remain unsupported with explicit reasons; not all algorithmic gaps fixed |
| M36 Regression Harness v2 | ✅ Complete | Suites, thresholds, baseline committed; **became fully stable after M39a repair**; both CLI entrypoints now consistent |
| M37 Realism Scenario Pack | ✅ Complete | Light/medium/heavy profiles, 4 scenario JSONs, battery model metadata in manifest; **not yet a calibrated research study** |
| M38 Wildfire v2 | ⚠️ Partial | Wildfire profiles, dynamic threat, spatial spread, metrics, replay integration complete; **Flood is not implemented as a separate mission** |
| M39a Regression Repair | ✅ Complete | Unified entrypoints, fixed wildfire/realism in `strategy_comparison --regression`, eliminated duplication, fixed flaky tests |
| M39b Decision / Audit Report | ✅ Complete | This document; README updated; benchmark docs marked historical |

---

## Known Limitations

### Algorithmic

- **SAR CBBA**: Near-zero success due to delayed re-convergence after `release_task()`; documented as `unsupported_reason: delayed_reconvergence`
- **SAR Centralized**: Static pre-plan incompatible with dynamic belief search; documented as `unsupported_reason: static_pre_plan`
- **Inspection Perimeter**: Battery/time constraints limit coverage; success rate ~0-0.4 for some strategies
- **Emergency Mesh**: Centralized much stronger than distributed strategies
- **Wildfire CBBA**: Weaker on dynamic fire spread scenarios

### Infrastructure

- **Benchmark docs**: `docs/BENCHMARK_RESULTS.md` reflects commit `8fb5ab1` (pre-M33); not refreshed for current HEAD
- **Determinism**: `jobs=1` vs `jobs=4` mostly stable, but some suites show variance that should be investigated before a 1000-seed run
- **Realism**: Profiles exist but impact is not quantified across missions

### Not Implemented

- **Flood mission**: M38 is named "Wildfire / Flood v2" in historical docs, but flood has no separate scenario, adapter, or profile
- **Multi-agent SITL**: Only single-agent mock SITL exists
- **Visualization product**: Replay CLI only; no web/dashboard

---

## Readiness for 1000-Seed Run

| Criterion | Status | Blocker |
|---|---|---|
| Green workspace tests | ✅ | — |
| Stable regression | ✅ | — |
| Deterministic benchmark output | ⚠️ | Needs verification `jobs=1` vs `jobs=4` vs `jobs=14` |
| Fresh baseline on HEAD | ❌ | Baseline reflects pre-M33 state |
| Known gaps classified | ✅ | Documented in this file and support matrix |

**Verdict:** A 1000-seed run is technically possible now, but its interpretability would be limited without:
1. A fresh baseline committed;
2. Confirmation that `jobs` count does not affect aggregate metrics;
3. Updated benchmark docs.

Recommended: complete M40 (Benchmark Determinism) first.

---

## Next Steps

Per `DRONE_A.15.linear.md`:

1. **M40 — Benchmark Determinism**: Verify `jobs=1` vs `jobs=4` vs `jobs=14`; eliminate nondeterministic sources; commit fresh baseline
2. **M41 — Algorithmic Gap Triage**: Classify every known large failure; fix high-confidence bugs
3. **M42 — Regression Harness v3**: Separate smoke/quick/benchmark/experimental suites; action-oriented failure output
4. **M43 — Realism Calibration**: Quantify ideal vs light/medium/heavy impact
5. **M44 — Flood Decision**: Rename M38 or implement minimal flood variant
6. **M45 — Big Direction Decision**: Choose next major track (research benchmark / visualization / public API / SITL)

---

## How to Verify This Status

```bash
# Green tests
cargo test --workspace

# Stable regression (both entrypoints)
cargo run -p swarm-examples --bin regression_runner -- --jobs 4
cargo run -p swarm-examples --bin strategy_comparison -- --regression --jobs 4

# Clippy clean
cargo clippy --all-targets -- -D warnings
```
