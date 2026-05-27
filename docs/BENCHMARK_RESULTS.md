# Benchmark Results

This document records the current post-M32b benchmark pack. It replaces the
older pre-M32 quick report whose mixed-mission rows had stale
`mission`/`scenario` identity fields.

## Current Run

- **Date:** 2026-05-27
- **Benchmark run id:** `2026-05-27T161946Z_all_50_custom`
- **Benchmark git commit:** `f9a739bdb77dc65ce9d1dfe25645f2bb1e06022f`
- **Build profile:** release
- **Mode:** custom 50 seeds, all built-in missions
- **Jobs:** 14 Rayon worker jobs on a 16-core machine
- **Scenario runs:** `50 seeds * 5 strategies * 36 profiles = 9000`
- **Aggregated rows:** 180
- **Runtime:** 1 min 30.12 sec
- **Peak RSS:** 16480 KB
- **Output pack:** `results/all_50_jobs14_m32b_release/`
- **Identity check:** 0 bad rows for per-row `mission`, `scenario`, `profile`, and run id

Command:

```bash
cargo build --release -p swarm-examples --bin strategy_comparison

/usr/bin/time -f 'elapsed=%E maxrss_kb=%M' \
  target/release/strategy_comparison \
    --seeds 50 \
    --mission all \
    --jobs 14 \
    --output-dir results/all_50_jobs14_m32b_release
```

Generated artifacts:

- `results/all_50_jobs14_m32b_release/manifest.json` - run metadata
- `results/all_50_jobs14_m32b_release/results.json` - machine-readable aggregate data
- `results/all_50_jobs14_m32b_release/results.csv` - tabular aggregate data
- `results/all_50_jobs14_m32b_release/table.md` - full markdown table

The previous debug 50-seed validation pack is preserved in
`results/all_50_jobs14_m32b/`. The previous 20-seed validation pack is
preserved in `results/all_20_jobs14_m32b/`. The release 50-seed run is still a
validation/custom run, not a publishable statistical run. The next publishable
benchmark should use the same release path with `--full` or `--seeds 1000`.

## Release vs Debug Runtime

| Run | Build | Seeds | Jobs | Runtime | Peak RSS | Output |
|---|---|---:|---:|---:|---:|---|
| 20-seed validation | debug | 20 | 14 | 4:23.12 | 42104 KB | `results/all_20_jobs14_m32b/` |
| 50-seed validation | debug | 50 | 14 | 9:28.44 | 41992 KB | `results/all_50_jobs14_m32b/` |
| 50-seed validation | release | 50 | 14 | 1:30.12 | 16480 KB | `results/all_50_jobs14_m32b_release/` |

Release mode is about 6.3x faster than the 50-seed debug run. The aggregate
metrics are close but not byte-identical between debug and release runs, so
determinism should be audited before using these numbers as publishable
evidence.

## Mission-Level Summary

Values below are averaged across all profiles of each mission for a strategy.

| Mission | Strategy | Profiles | Success | Completion | Availability | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| coverage | auction | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | cbba | 24 | 0.749 | 0.999 | 0.992 | 0.0 |
| coverage | centralized | 24 | 1.000 | 1.000 | 0.991 | 0.0 |
| coverage | connectivity-aware | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | greedy | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| emergency-mesh | auction | 5 | 0.412 | 0.680 | 0.591 | 1.8 |
| emergency-mesh | cbba | 5 | 0.440 | 0.708 | 0.610 | 4.4 |
| emergency-mesh | centralized | 5 | 0.732 | 1.000 | 0.662 | 0.0 |
| emergency-mesh | connectivity-aware | 5 | 0.384 | 0.652 | 0.578 | 2.0 |
| emergency-mesh | greedy | 5 | 0.364 | 0.632 | 0.555 | 2.5 |
| inspection | auction | 3 | 0.700 | 0.700 | 0.302 | 11704.9 |
| inspection | cbba | 3 | 0.547 | 0.547 | 0.722 | 6411.8 |
| inspection | centralized | 3 | 0.687 | 0.687 | 0.376 | 6585.7 |
| inspection | connectivity-aware | 3 | 0.727 | 0.727 | 0.310 | 12159.3 |
| inspection | greedy | 3 | 0.820 | 0.820 | 0.601 | 12317.4 |
| sar | auction | 2 | 0.770 | 0.770 | 0.047 | 712.5 |
| sar | cbba | 2 | 0.010 | 0.010 | 0.010 | 959.8 |
| sar | centralized | 2 | 0.000 | 0.000 | 0.020 | 144.0 |
| sar | connectivity-aware | 2 | 0.760 | 0.760 | 0.066 | 966.0 |
| sar | greedy | 2 | 0.710 | 0.740 | 0.055 | 4728.6 |
| wildfire | auction | 2 | 0.980 | 1.000 | 1.000 | 26.5 |
| wildfire | cbba | 2 | 0.660 | 1.000 | 1.000 | 804.9 |
| wildfire | centralized | 2 | 0.980 | 1.000 | 1.000 | 26.5 |
| wildfire | connectivity-aware | 2 | 0.980 | 1.000 | 1.000 | 26.5 |
| wildfire | greedy | 2 | 1.000 | 1.000 | 1.000 | 0.0 |

## Key Findings

1. **Coverage is mostly solved for the current profiles.** Auction,
   centralized, connectivity-aware, and greedy reached 1.000 average success
   across all 24 coverage profiles. CBBA averages 0.749 success despite 0.999
   completion, so the remaining coverage issue is concentrated rather than
   broad.

2. **CBBA coverage rows still need investigation.** Six CBBA coverage profiles
   report `Success = 0.000` with `Completion = 1.000` and `Coverage = 0.000`.
   One more profile, `coverage/partition-prone-cascade-failure`, reports
   `Success = 0.980` and `Completion = 0.980` but still `Coverage = 0.000`.
   This looks suspicious enough to inspect the success predicate and metric
   extraction before treating it as an algorithm result.

3. **Emergency mesh currently favors centralized allocation.** Centralized
   reached 0.732 average success and 1.000 completion with zero conflicts.
   The distributed strategies are functional but lower: CBBA 0.440, auction
   0.412, connectivity-aware 0.384, and greedy 0.364.

4. **SAR still exposes planner gaps.** Auction has the best average success
   at 0.770. Connectivity-aware follows at 0.760, greedy at 0.710, while
   centralized remains at 0.000 and CBBA is nearly zero at 0.010. This keeps
   SAR as the clearest mission for testing grid/belief task support.

5. **Inspection is profile-sensitive.** Greedy has the best average inspection
   success at 0.820, followed by connectivity-aware at 0.727 and auction at
   0.700. Linear and random are solved by most strategies, but perimeter
   remains difficult: greedy reaches 0.460, connectivity-aware 0.180, auction
   0.100, centralized 0.060, and CBBA 0.020.

6. **Wildfire is mostly strong except CBBA on dynamic fire spread.** Greedy
   reaches 1.000 average success; auction, centralized, and
   connectivity-aware reach 0.980. CBBA improves over the debug 50-seed run but
   remains much lower at 0.660 because `wildfire/medium-dynamic` succeeds only
   0.360 of the time.

## Coverage Notes

Coverage rows with `Success < 1.000` are isolated to CBBA:

| Strategy | Profile | Success | Completion | Coverage | Realloc | Availability |
|---|---|---:|---:|---:|---:|---:|
| cbba | coverage/high-latency-cascade-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-cascade-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/high-latency-single-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/high-latency-multiple-failures | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-single-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-multiple-failures | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/partition-prone-cascade-failure | 0.980 | 0.980 | 0.000 | 0.000 | 0.991 |

## SAR Detail

| Strategy | Profile | Success | Completion | PoD | Targets | BeliefEntropy | FalsePosRate |
|---|---|---:|---:|---:|---:|---:|---:|
| cbba | sar/standard | 0.000 | 0.000 | 0.127 | 0.4 | 0.316 | 0.446 |
| cbba | sar/ideal | 0.020 | 0.020 | 0.210 | 0.4 | 0.390 | 0.627 |
| greedy | sar/standard | 0.640 | 0.660 | 0.047 | 0.1 | 0.309 | 0.442 |
| greedy | sar/ideal | 0.780 | 0.820 | 0.070 | 0.1 | 0.372 | 0.599 |
| auction | sar/standard | 0.800 | 0.800 | 0.100 | 0.3 | 0.316 | 0.460 |
| auction | sar/ideal | 0.740 | 0.740 | 0.160 | 0.3 | 0.377 | 0.582 |
| connectivity-aware | sar/standard | 0.760 | 0.760 | 0.133 | 0.4 | 0.320 | 0.469 |
| connectivity-aware | sar/ideal | 0.760 | 0.760 | 0.150 | 0.3 | 0.374 | 0.598 |
| centralized | sar/standard | 0.000 | 0.000 | 0.053 | 0.2 | 0.308 | 0.497 |
| centralized | sar/ideal | 0.000 | 0.000 | 0.090 | 0.2 | 0.343 | 0.615 |

## Inspection Detail

| Strategy | Profile | Success | EdgeCoverage | MissedEdges | RouteEfficiency |
|---|---|---:|---:|---:|---:|
| cbba | inspection/random | 0.720 | 0.958 | 0.9 | 0.228 |
| cbba | inspection/linear | 0.900 | 0.942 | 0.6 | 0.173 |
| cbba | inspection/perimeter | 0.020 | 0.889 | 4.4 | 0.102 |
| greedy | inspection/random | 1.000 | 1.000 | 0.0 | 0.215 |
| greedy | inspection/linear | 1.000 | 1.000 | 0.0 | 0.160 |
| greedy | inspection/perimeter | 0.460 | 0.864 | 5.4 | 0.077 |
| auction | inspection/random | 1.000 | 1.000 | 0.0 | 0.386 |
| auction | inspection/linear | 1.000 | 1.000 | 0.0 | 0.297 |
| auction | inspection/perimeter | 0.100 | 0.745 | 10.2 | 0.149 |
| connectivity-aware | inspection/random | 1.000 | 1.000 | 0.0 | 0.394 |
| connectivity-aware | inspection/linear | 1.000 | 1.000 | 0.0 | 0.315 |
| connectivity-aware | inspection/perimeter | 0.180 | 0.792 | 8.3 | 0.156 |
| centralized | inspection/random | 1.000 | 1.000 | 0.0 | 0.657 |
| centralized | inspection/linear | 1.000 | 1.000 | 0.0 | 0.550 |
| centralized | inspection/perimeter | 0.060 | 0.570 | 17.2 | 0.185 |

## Emergency Mesh Detail

| Strategy | Profile | Success | Completion | Availability | Realloc | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| cbba | emergency-mesh/packet-loss-10 | 0.320 | 0.700 | 0.702 | 0.000 | 3.6 |
| cbba | emergency-mesh/ideal | 0.400 | 0.780 | 0.781 | 0.000 | 4.6 |
| cbba | emergency-mesh/medium-loss | 0.300 | 0.500 | 0.507 | 0.000 | 6.1 |
| cbba | emergency-mesh/single-failure | 0.780 | 0.780 | 0.278 | 0.000 | 3.7 |
| cbba | emergency-mesh/low-loss | 0.400 | 0.780 | 0.780 | 0.000 | 3.9 |
| greedy | emergency-mesh/packet-loss-10 | 0.260 | 0.640 | 0.644 | 0.000 | 1.9 |
| greedy | emergency-mesh/ideal | 0.360 | 0.740 | 0.742 | 0.000 | 1.4 |
| greedy | emergency-mesh/medium-loss | 0.220 | 0.420 | 0.430 | 0.000 | 6.3 |
| greedy | emergency-mesh/single-failure | 0.700 | 0.700 | 0.296 | 0.000 | 1.5 |
| greedy | emergency-mesh/low-loss | 0.280 | 0.660 | 0.664 | 0.000 | 1.4 |
| auction | emergency-mesh/packet-loss-10 | 0.260 | 0.640 | 0.645 | 0.000 | 1.5 |
| auction | emergency-mesh/ideal | 0.300 | 0.680 | 0.684 | 0.000 | 0.9 |
| auction | emergency-mesh/medium-loss | 0.340 | 0.540 | 0.547 | 0.000 | 5.0 |
| auction | emergency-mesh/single-failure | 0.760 | 0.760 | 0.296 | 0.000 | 0.8 |
| auction | emergency-mesh/low-loss | 0.400 | 0.780 | 0.782 | 0.000 | 0.8 |
| connectivity-aware | emergency-mesh/packet-loss-10 | 0.280 | 0.660 | 0.664 | 0.000 | 1.7 |
| connectivity-aware | emergency-mesh/ideal | 0.340 | 0.720 | 0.723 | 0.000 | 1.0 |
| connectivity-aware | emergency-mesh/medium-loss | 0.280 | 0.480 | 0.488 | 0.000 | 4.9 |
| connectivity-aware | emergency-mesh/single-failure | 0.660 | 0.660 | 0.277 | 0.000 | 1.1 |
| connectivity-aware | emergency-mesh/low-loss | 0.360 | 0.740 | 0.742 | 0.000 | 1.1 |
| centralized | emergency-mesh/packet-loss-10 | 0.620 | 1.000 | 0.702 | 0.000 | 0.0 |
| centralized | emergency-mesh/ideal | 0.620 | 1.000 | 0.702 | 0.000 | 0.0 |
| centralized | emergency-mesh/medium-loss | 0.800 | 1.000 | 0.584 | 0.000 | 0.0 |
| centralized | emergency-mesh/single-failure | 1.000 | 1.000 | 0.623 | 0.000 | 0.0 |
| centralized | emergency-mesh/low-loss | 0.620 | 1.000 | 0.702 | 0.000 | 0.0 |

## Wildfire Detail

| Strategy | Profile | Success | Completion | ZonesMapped | PriorityUpdates | FinalThreat |
|---|---|---:|---:|---:|---:|---:|
| cbba | wildfire/small-static | 0.960 | 1.000 | 0.120 | 0.0 | 0.500 |
| cbba | wildfire/medium-dynamic | 0.360 | 1.000 | 2.860 | 88.2 | 0.813 |
| greedy | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| greedy | wildfire/medium-dynamic | 1.000 | 1.000 | 0.000 | 0.0 | 0.300 |
| auction | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| auction | wildfire/medium-dynamic | 0.960 | 1.000 | 0.160 | 4.6 | 0.327 |
| connectivity-aware | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| connectivity-aware | wildfire/medium-dynamic | 0.960 | 1.000 | 0.160 | 4.6 | 0.327 |
| centralized | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| centralized | wildfire/medium-dynamic | 0.960 | 1.000 | 0.160 | 4.6 | 0.327 |

## Next Steps

1. Audit determinism between debug and release runs, including strategy/profile
   ordering and aggregate metric differences.
2. Investigate the CBBA coverage rows where completion is high but coverage is
   0.000.
3. Decide whether SAR failures for centralized/CBBA are expected planner
   limitations or missing task adapters for grid/belief tasks.
4. After the suspicious rows are resolved or explicitly documented, run the
   publishable `--seeds 1000 --mission all --jobs <N>` release benchmark and
   update this document with the final pack.
