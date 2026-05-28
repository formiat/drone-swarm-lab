# Benchmark Results

> **Historical Note:** This report reflects a 500-seed benchmark run on commit
> `8fb5ab130968a17b35c76314b7c133fb4fe791af` (pre-M33 Mission Semantics
> Integration, pre-M34-M38). It remains useful as a historical validation of
> M32b reporting identity, but does not represent the current HEAD. For
> up-to-date project status, see [`STATUS.md`](STATUS.md).

This document records the post-M32b benchmark pack. It replaces the older
pre-M32 quick report whose mixed-mission rows had stale `mission`/`scenario`
identity fields.

## Current Run

- **Date:** 2026-05-27
- **Benchmark run id:** `2026-05-27T164016Z_all_500_custom`
- **Benchmark git commit:** `8fb5ab130968a17b35c76314b7c133fb4fe791af`
- **Build profile:** release
- **Mode:** custom 500 seeds, all built-in missions
- **Jobs:** 14 Rayon worker jobs on a 16-core machine
- **Scenario runs:** `500 seeds * 5 strategies * 36 profiles = 90000`
- **Aggregated rows:** 180
- **Runtime:** 14 min 50.67 sec
- **Peak RSS:** 76232 KB
- **Output pack:** `results/all_500_jobs14_m32b_release/`
- **Identity check:** 0 bad rows for per-row `mission`, `scenario`, `profile`, and run id

Command:

```bash
cargo build --release -p swarm-examples --bin strategy_comparison

/usr/bin/time -f 'elapsed=%E maxrss_kb=%M' \
  target/release/strategy_comparison \
    --seeds 500 \
    --mission all \
    --jobs 14 \
    --output-dir results/all_500_jobs14_m32b_release
```

Generated artifacts:

- `results/all_500_jobs14_m32b_release/manifest.json` - run metadata
- `results/all_500_jobs14_m32b_release/results.json` - machine-readable aggregate data
- `results/all_500_jobs14_m32b_release/results.csv` - tabular aggregate data
- `results/all_500_jobs14_m32b_release/table.md` - full markdown table

Previous validation packs are preserved in:

- `results/all_200_jobs14_m32b_release/`
- `results/all_50_jobs14_m32b_release/`
- `results/all_50_jobs14_m32b/`
- `results/all_20_jobs14_m32b/`

The 500-seed release run is still a validation/custom run, not a publishable
statistical run. The next publishable benchmark should use the same release
path with `--full` or `--seeds 1000`.

## Runtime History

| Run | Build | Seeds | Jobs | Runtime | Peak RSS | Output |
|---|---|---:|---:|---:|---:|---|
| 20-seed validation | debug | 20 | 14 | 4:23.12 | 42104 KB | `results/all_20_jobs14_m32b/` |
| 50-seed validation | debug | 50 | 14 | 9:28.44 | 41992 KB | `results/all_50_jobs14_m32b/` |
| 50-seed validation | release | 50 | 14 | 1:30.12 | 16480 KB | `results/all_50_jobs14_m32b_release/` |
| 200-seed validation | release | 200 | 14 | 6:16.21 | 37228 KB | `results/all_200_jobs14_m32b_release/` |
| 500-seed validation | release | 500 | 14 | 14:50.67 | 76232 KB | `results/all_500_jobs14_m32b_release/` |

Release mode is about 6.3x faster than the 50-seed debug run. The 500-seed
release run scales well from the 200-seed release run: 2.5x more seeds took
about 2.37x more wall-clock time.

The aggregate metrics are close but not byte-identical between debug and
release runs, so determinism should be audited before using these numbers as
publishable evidence.

## Mission-Level Summary

Values below are averaged across all profiles of each mission for a strategy.

| Mission | Strategy | Profiles | Success | Completion | Availability | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| coverage | auction | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | cbba | 24 | 0.750 | 1.000 | 0.992 | 0.0 |
| coverage | centralized | 24 | 1.000 | 1.000 | 0.991 | 0.0 |
| coverage | connectivity-aware | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | greedy | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| emergency-mesh | auction | 5 | 0.398 | 0.570 | 0.514 | 2.1 |
| emergency-mesh | cbba | 5 | 0.432 | 0.605 | 0.540 | 4.3 |
| emergency-mesh | centralized | 5 | 0.828 | 1.000 | 0.551 | 0.0 |
| emergency-mesh | connectivity-aware | 5 | 0.404 | 0.576 | 0.522 | 2.3 |
| emergency-mesh | greedy | 5 | 0.376 | 0.548 | 0.489 | 2.7 |
| inspection | auction | 3 | 0.733 | 0.733 | 0.288 | 11877.3 |
| inspection | cbba | 3 | 0.560 | 0.560 | 0.729 | 6534.3 |
| inspection | centralized | 3 | 0.683 | 0.683 | 0.344 | 6736.1 |
| inspection | connectivity-aware | 3 | 0.729 | 0.729 | 0.291 | 11792.9 |
| inspection | greedy | 3 | 0.775 | 0.775 | 0.628 | 12362.8 |
| sar | auction | 2 | 0.730 | 0.733 | 0.047 | 1095.8 |
| sar | cbba | 2 | 0.006 | 0.009 | 0.025 | 1113.7 |
| sar | centralized | 2 | 0.004 | 0.004 | 0.033 | 178.9 |
| sar | connectivity-aware | 2 | 0.715 | 0.717 | 0.038 | 1107.3 |
| sar | greedy | 2 | 0.706 | 0.739 | 0.039 | 4812.3 |
| wildfire | auction | 2 | 0.984 | 1.000 | 1.000 | 18.0 |
| wildfire | cbba | 2 | 0.621 | 1.000 | 1.000 | 735.4 |
| wildfire | centralized | 2 | 0.984 | 1.000 | 1.000 | 18.7 |
| wildfire | connectivity-aware | 2 | 0.984 | 1.000 | 1.000 | 18.8 |
| wildfire | greedy | 2 | 0.998 | 1.000 | 1.000 | 1.7 |

## Key Findings

1. **Coverage is mostly solved for the current profiles.** Auction,
   centralized, connectivity-aware, and greedy reached 1.000 average success
   across all 24 coverage profiles. CBBA averages 0.750 success with 1.000
   completion, so the remaining coverage issue is concentrated rather than
   broad.

2. **CBBA coverage rows still need investigation.** Six CBBA coverage profiles
   report `Success = 0.000` with `Completion = 1.000` and `Coverage = 0.000`.
   One more profile, `coverage/partition-prone-cascade-failure`, reports
   `Success = 0.996` and `Completion = 0.996` but still `Coverage = 0.000`.
   This looks suspicious enough to inspect the success predicate and metric
   extraction before treating it as an algorithm result.

3. **Emergency mesh currently favors centralized allocation.** Centralized
   reached 0.828 average success and 1.000 completion with zero conflicts.
   The distributed strategies are functional but lower: CBBA 0.432,
   connectivity-aware 0.404, auction 0.398, and greedy 0.376.

4. **SAR still exposes planner gaps.** Auction has the best average success
   at 0.730, connectivity-aware follows at 0.715, and greedy is close at
   0.706. Centralized and CBBA remain near zero. SAR remains the clearest
   mission for testing grid/belief task support.

5. **Inspection is profile-sensitive.** Greedy has the best average inspection
   success at 0.775, followed by auction at 0.733 and connectivity-aware at
   0.729. Linear and random are solved by most strategies, but perimeter
   remains difficult: greedy reaches 0.326, auction 0.200,
   connectivity-aware 0.188, centralized 0.048, and CBBA 0.034.

6. **Wildfire is strong except CBBA on dynamic fire spread.** Greedy reaches
   0.998 average success; auction, centralized, and connectivity-aware reach
   0.984. CBBA remains much lower at 0.621 because
   `wildfire/medium-dynamic` succeeds only 0.358 of the time.

## Coverage Notes

Coverage rows with `Success < 1.000` are isolated to CBBA:

| Strategy | Profile | Success | Completion | Coverage | Realloc | Availability |
|---|---|---:|---:|---:|---:|---:|
| cbba | coverage/high-latency-cascade-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/high-latency-multiple-failures | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-single-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-multiple-failures | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/partition-prone-cascade-failure | 0.996 | 0.996 | 0.000 | 0.000 | 0.992 |
| cbba | coverage/high-latency-single-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-cascade-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |

## SAR Detail

| Strategy | Profile | Success | Completion | PoD | Targets | BeliefEntropy | FalsePosRate |
|---|---|---:|---:|---:|---:|---:|---:|
| auction | sar/ideal | 0.686 | 0.688 | 0.124 | 0.2 | 0.365 | 0.543 |
| auction | sar/standard | 0.774 | 0.778 | 0.083 | 0.2 | 0.315 | 0.428 |
| greedy | sar/ideal | 0.668 | 0.726 | 0.113 | 0.2 | 0.359 | 0.538 |
| greedy | sar/standard | 0.744 | 0.752 | 0.068 | 0.2 | 0.309 | 0.418 |
| cbba | sar/ideal | 0.012 | 0.014 | 0.148 | 0.3 | 0.369 | 0.547 |
| cbba | sar/standard | 0.000 | 0.004 | 0.079 | 0.2 | 0.313 | 0.425 |
| connectivity-aware | sar/ideal | 0.642 | 0.644 | 0.149 | 0.3 | 0.363 | 0.531 |
| connectivity-aware | sar/standard | 0.788 | 0.790 | 0.079 | 0.2 | 0.314 | 0.427 |
| centralized | sar/ideal | 0.004 | 0.004 | 0.087 | 0.2 | 0.333 | 0.534 |
| centralized | sar/standard | 0.004 | 0.004 | 0.052 | 0.2 | 0.303 | 0.427 |

## Inspection Detail

| Strategy | Profile | Success | EdgeCoverage | MissedEdges | RouteEfficiency |
|---|---|---:|---:|---:|---:|
| auction | inspection/random | 1.000 | 1.000 | 0.0 | 0.363 |
| auction | inspection/perimeter | 0.200 | 0.764 | 9.4 | 0.151 |
| auction | inspection/linear | 1.000 | 1.000 | 0.0 | 0.304 |
| greedy | inspection/random | 1.000 | 1.000 | 0.0 | 0.212 |
| greedy | inspection/perimeter | 0.326 | 0.863 | 5.5 | 0.076 |
| greedy | inspection/linear | 1.000 | 1.000 | 0.0 | 0.157 |
| cbba | inspection/random | 0.732 | 0.957 | 1.0 | 0.217 |
| cbba | inspection/perimeter | 0.034 | 0.903 | 3.9 | 0.103 |
| cbba | inspection/linear | 0.914 | 0.953 | 0.5 | 0.178 |
| connectivity-aware | inspection/random | 1.000 | 1.000 | 0.0 | 0.370 |
| connectivity-aware | inspection/perimeter | 0.188 | 0.759 | 9.6 | 0.149 |
| connectivity-aware | inspection/linear | 1.000 | 1.000 | 0.0 | 0.302 |
| centralized | inspection/random | 1.000 | 1.000 | 0.0 | 0.669 |
| centralized | inspection/perimeter | 0.048 | 0.588 | 16.5 | 0.194 |
| centralized | inspection/linear | 1.000 | 1.000 | 0.0 | 0.550 |

## Emergency Mesh Detail

| Strategy | Profile | Success | Completion | Availability | Realloc | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| auction | emergency-mesh/packet-loss-10 | 0.378 | 0.610 | 0.614 | 0.000 | 2.0 |
| auction | emergency-mesh/low-loss | 0.358 | 0.604 | 0.608 | 0.000 | 1.2 |
| auction | emergency-mesh/single-failure | 0.560 | 0.560 | 0.259 | 0.000 | 1.3 |
| auction | emergency-mesh/ideal | 0.344 | 0.592 | 0.596 | 0.000 | 1.1 |
| auction | emergency-mesh/medium-loss | 0.352 | 0.486 | 0.492 | 0.000 | 5.1 |
| greedy | emergency-mesh/packet-loss-10 | 0.350 | 0.582 | 0.587 | 0.000 | 2.7 |
| greedy | emergency-mesh/low-loss | 0.352 | 0.598 | 0.602 | 0.000 | 1.7 |
| greedy | emergency-mesh/single-failure | 0.570 | 0.570 | 0.254 | 0.000 | 1.9 |
| greedy | emergency-mesh/ideal | 0.322 | 0.570 | 0.575 | 0.000 | 1.5 |
| greedy | emergency-mesh/medium-loss | 0.288 | 0.422 | 0.430 | 0.000 | 6.0 |
| cbba | emergency-mesh/packet-loss-10 | 0.382 | 0.614 | 0.621 | 0.000 | 4.6 |
| cbba | emergency-mesh/low-loss | 0.414 | 0.660 | 0.663 | 0.000 | 3.6 |
| cbba | emergency-mesh/single-failure | 0.598 | 0.598 | 0.254 | 0.000 | 3.6 |
| cbba | emergency-mesh/ideal | 0.408 | 0.656 | 0.659 | 0.000 | 3.7 |
| cbba | emergency-mesh/medium-loss | 0.360 | 0.496 | 0.503 | 0.000 | 6.2 |
| connectivity-aware | emergency-mesh/packet-loss-10 | 0.390 | 0.622 | 0.626 | 0.000 | 2.3 |
| connectivity-aware | emergency-mesh/low-loss | 0.376 | 0.622 | 0.626 | 0.000 | 1.4 |
| connectivity-aware | emergency-mesh/single-failure | 0.554 | 0.554 | 0.264 | 0.000 | 1.3 |
| connectivity-aware | emergency-mesh/ideal | 0.352 | 0.600 | 0.604 | 0.000 | 1.1 |
| connectivity-aware | emergency-mesh/medium-loss | 0.348 | 0.482 | 0.488 | 0.000 | 5.1 |
| centralized | emergency-mesh/packet-loss-10 | 0.768 | 1.000 | 0.583 | 0.000 | 0.0 |
| centralized | emergency-mesh/low-loss | 0.754 | 1.000 | 0.583 | 0.000 | 0.0 |
| centralized | emergency-mesh/single-failure | 1.000 | 1.000 | 0.553 | 0.000 | 0.0 |
| centralized | emergency-mesh/ideal | 0.752 | 1.000 | 0.583 | 0.000 | 0.0 |
| centralized | emergency-mesh/medium-loss | 0.866 | 1.000 | 0.453 | 0.000 | 0.0 |

## Wildfire Detail

| Strategy | Profile | Success | Completion | ZonesMapped | PriorityUpdates | FinalThreat |
|---|---|---:|---:|---:|---:|---:|
| auction | wildfire/small-static | 0.992 | 1.000 | 0.016 | 0.0 | 0.500 |
| auction | wildfire/medium-dynamic | 0.976 | 1.000 | 0.110 | 3.2 | 0.319 |
| greedy | wildfire/small-static | 0.998 | 1.000 | 0.008 | 0.0 | 0.500 |
| greedy | wildfire/medium-dynamic | 0.998 | 1.000 | 0.008 | 0.2 | 0.301 |
| cbba | wildfire/small-static | 0.884 | 1.000 | 0.224 | 0.0 | 0.500 |
| cbba | wildfire/medium-dynamic | 0.358 | 1.000 | 2.754 | 86.5 | 0.804 |
| connectivity-aware | wildfire/small-static | 0.992 | 1.000 | 0.018 | 0.0 | 0.500 |
| connectivity-aware | wildfire/medium-dynamic | 0.976 | 1.000 | 0.112 | 3.2 | 0.319 |
| centralized | wildfire/small-static | 0.992 | 1.000 | 0.018 | 0.0 | 0.500 |
| centralized | wildfire/medium-dynamic | 0.976 | 1.000 | 0.112 | 3.2 | 0.319 |

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
