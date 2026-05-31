# M69 1000-Seed Release Benchmark

This directory contains the M69 current-head simulation benchmark refresh for
benchmark code commit `5d1d3cd17cacba7482c1d9b93eb5acc107af8f71`.

## Run Metadata

- Date: 2026-05-31
- Benchmark run id: `2026-05-31T193356Z_all_1000_full`
- Build profile: release
- Command: `target/release/strategy_comparison --seeds 1000 --mission all --jobs 14 --output-dir results/all_1000_jobs14_m69_release`
- Jobs: 14
- Seed range: 0..1000
- Strategies: auction, cbba, centralized, connectivity-aware, greedy
- Built-in benchmark profiles: 38
- Aggregate rows: 190
- Scenario runs: `1000 seeds * 5 strategies * 38 profiles = 190000`
- Runtime: 28 min 55.25 sec
- Peak RSS: 207684 KB

## Files

- `manifest.json` - run metadata and command line
- `results.json` - machine-readable aggregate metrics
- `results.csv` - tabular aggregate metrics
- `table.md` - full Markdown table
- `run.log` - captured benchmark stdout plus runtime and peak RSS

## Scope

This is a current-head 1000-seed release simulation benchmark for the built-in
`--mission all` benchmark suite. The manifest records these missions:

- coverage
- emergency-mesh
- sar
- inspection
- wildfire

Urban scenario-suite fixtures are not part of this `--mission all` entrypoint
yet. The current Urban algorithm evidence remains the separate M68 pack in
`results/m68_urban_corridor_delta/`.

This benchmark is not PX4/SITL evidence, hardware evidence, or a substitute for
the local PX4/SIH artifacts under `results/m48_*`, `results/m58_*`, and
`results/m59_*`.

## Mission-Level Summary

Mission averages across all strategies/profiles:

| Mission | Rows | AvgSuccess | AvgCompletion | PerfectRows | WeakRows |
|---|---:|---:|---:|---:|---:|
| coverage | 120 | 0.950 | 1.000 | 113 | 7 |
| emergency-mesh | 25 | 0.499 | 0.659 | 1 | 24 |
| inspection | 15 | 1.000 | 0.907 | 15 | 0 |
| sar | 10 | 0.001 | 0.803 | 0 | 10 |
| wildfire | 20 | 0.219 | 1.000 | 0 | 20 |

Interpretation:

- Coverage is stable for auction, centralized, connectivity-aware, and greedy;
  CBBA still has known weak rows under heavy/high-latency failure cases.
- Centralized remains an oracle-like upper bound for emergency-mesh completion.
- Inspection success is stable, but completion/conflict behavior still varies
  by strategy and profile.
- SAR success remains intentionally low despite high completion in several
  rows, because SAR success depends on target detection semantics, not only
  task completion.
- Wildfire completion remains high while success remains lower because success
  uses mapped-ratio plus failure/unassigned guards.

## Verification

Additional command run after the benchmark:

```bash
/home/formi/.local/bin/runlim target/release/regression_runner --jobs 14
```

Result: `overall_pass: true`.
