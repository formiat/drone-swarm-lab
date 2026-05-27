# Benchmark Results

This document records the current post-M32b benchmark pack. It replaces the
older pre-M32 quick report whose mixed-mission rows had stale
`mission`/`scenario` identity fields.

## Current Run

- **Date:** 2026-05-27
- **Benchmark run id:** `2026-05-27T162606Z_all_200_custom`
- **Benchmark git commit:** `261c6fa41393bcf2ca342c487d2dd7478b71e15c`
- **Build profile:** release
- **Mode:** custom 200 seeds, all built-in missions
- **Jobs:** 14 Rayon worker jobs on a 16-core machine
- **Scenario runs:** `200 seeds * 5 strategies * 36 profiles = 36000`
- **Aggregated rows:** 180
- **Runtime:** 6 min 16.21 sec
- **Peak RSS:** 37228 KB
- **Output pack:** `results/all_200_jobs14_m32b_release/`
- **Identity check:** 0 bad rows for per-row `mission`, `scenario`, `profile`, and run id

Command:

```bash
cargo build --release -p swarm-examples --bin strategy_comparison

/usr/bin/time -f 'elapsed=%E maxrss_kb=%M' \
  target/release/strategy_comparison \
    --seeds 200 \
    --mission all \
    --jobs 14 \
    --output-dir results/all_200_jobs14_m32b_release
```

Generated artifacts:

- `results/all_200_jobs14_m32b_release/manifest.json` - run metadata
- `results/all_200_jobs14_m32b_release/results.json` - machine-readable aggregate data
- `results/all_200_jobs14_m32b_release/results.csv` - tabular aggregate data
- `results/all_200_jobs14_m32b_release/table.md` - full markdown table

Previous validation packs are preserved in:

- `results/all_50_jobs14_m32b_release/`
- `results/all_50_jobs14_m32b/`
- `results/all_20_jobs14_m32b/`

The 200-seed release run is still a validation/custom run, not a publishable
statistical run. The next publishable benchmark should use the same release
path with `--full` or `--seeds 1000`.

## Runtime History

| Run | Build | Seeds | Jobs | Runtime | Peak RSS | Output |
|---|---|---:|---:|---:|---:|---|
| 20-seed validation | debug | 20 | 14 | 4:23.12 | 42104 KB | `results/all_20_jobs14_m32b/` |
| 50-seed validation | debug | 50 | 14 | 9:28.44 | 41992 KB | `results/all_50_jobs14_m32b/` |
| 50-seed validation | release | 50 | 14 | 1:30.12 | 16480 KB | `results/all_50_jobs14_m32b_release/` |
| 200-seed validation | release | 200 | 14 | 6:16.21 | 37228 KB | `results/all_200_jobs14_m32b_release/` |

Release mode is about 6.3x faster than the 50-seed debug run. The 200-seed
release run scales close to linearly from the 50-seed release run: 4x more
seeds took about 4.17x more wall-clock time.

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
| emergency-mesh | auction | 5 | 0.385 | 0.589 | 0.538 | 2.2 |
| emergency-mesh | cbba | 5 | 0.421 | 0.626 | 0.553 | 4.1 |
| emergency-mesh | centralized | 5 | 0.796 | 1.000 | 0.585 | 0.0 |
| emergency-mesh | connectivity-aware | 5 | 0.400 | 0.604 | 0.536 | 2.3 |
| emergency-mesh | greedy | 5 | 0.394 | 0.598 | 0.540 | 2.7 |
| inspection | auction | 3 | 0.745 | 0.745 | 0.289 | 11943.4 |
| inspection | cbba | 3 | 0.558 | 0.558 | 0.721 | 6415.9 |
| inspection | centralized | 3 | 0.688 | 0.688 | 0.336 | 6831.0 |
| inspection | connectivity-aware | 3 | 0.730 | 0.730 | 0.290 | 11929.1 |
| inspection | greedy | 3 | 0.807 | 0.807 | 0.627 | 12318.4 |
| sar | auction | 2 | 0.703 | 0.705 | 0.043 | 1030.3 |
| sar | cbba | 2 | 0.003 | 0.005 | 0.025 | 985.5 |
| sar | centralized | 2 | 0.005 | 0.005 | 0.028 | 179.1 |
| sar | connectivity-aware | 2 | 0.688 | 0.698 | 0.024 | 1151.5 |
| sar | greedy | 2 | 0.705 | 0.735 | 0.043 | 4780.9 |
| wildfire | auction | 2 | 0.985 | 1.000 | 1.000 | 17.3 |
| wildfire | cbba | 2 | 0.620 | 1.000 | 1.000 | 764.6 |
| wildfire | centralized | 2 | 0.985 | 1.000 | 1.000 | 17.2 |
| wildfire | connectivity-aware | 2 | 0.985 | 1.000 | 1.000 | 17.3 |
| wildfire | greedy | 2 | 0.995 | 1.000 | 1.000 | 3.7 |

## Key Findings

1. **Coverage is mostly solved for the current profiles.** Auction,
   centralized, connectivity-aware, and greedy reached 1.000 average success
   across all 24 coverage profiles. CBBA averages 0.750 success with 1.000
   completion, so the remaining coverage issue is concentrated rather than
   broad.

2. **CBBA coverage rows still need investigation.** Six CBBA coverage profiles
   report `Success = 0.000` with `Completion = 1.000` and `Coverage = 0.000`.
   One more profile, `coverage/partition-prone-cascade-failure`, reports
   `Success = 0.990` and `Completion = 0.990` but still `Coverage = 0.000`.
   This looks suspicious enough to inspect the success predicate and metric
   extraction before treating it as an algorithm result.

3. **Emergency mesh currently favors centralized allocation.** Centralized
   reached 0.796 average success and 1.000 completion with zero conflicts.
   The distributed strategies are functional but lower: CBBA 0.421,
   connectivity-aware 0.400, greedy 0.394, and auction 0.385.

4. **SAR still exposes planner gaps.** Greedy and auction are effectively tied
   around 0.705/0.703 average success, with connectivity-aware close at 0.688.
   Centralized and CBBA remain near zero. SAR remains the clearest mission for
   testing grid/belief task support.

5. **Inspection is profile-sensitive.** Greedy has the best average inspection
   success at 0.807, followed by auction at 0.745 and connectivity-aware at
   0.730. Linear and random are solved by most strategies, but perimeter
   remains difficult: greedy reaches 0.420, auction 0.235,
   connectivity-aware 0.190, centralized 0.065, and CBBA 0.025.

6. **Wildfire is strong except CBBA on dynamic fire spread.** Greedy reaches
   0.995 average success; auction, centralized, and connectivity-aware reach
   0.985. CBBA remains much lower at 0.620 because
   `wildfire/medium-dynamic` succeeds only 0.350 of the time.

## Coverage Notes

Coverage rows with `Success < 1.000` are isolated to CBBA:

| Strategy | Profile | Success | Completion | Coverage | Realloc | Availability |
|---|---|---:|---:|---:|---:|---:|
| cbba | coverage/high-latency-multiple-failures | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/partition-prone-cascade-failure | 0.990 | 0.990 | 0.000 | 0.000 | 0.992 |
| cbba | coverage/heavy-loss-cascade-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/high-latency-single-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-single-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/high-latency-cascade-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-multiple-failures | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |

## SAR Detail

| Strategy | Profile | Success | Completion | PoD | Targets | BeliefEntropy | FalsePosRate |
|---|---|---:|---:|---:|---:|---:|---:|
| centralized | sar/ideal | 0.000 | 0.000 | 0.090 | 0.2 | 0.337 | 0.562 |
| centralized | sar/standard | 0.010 | 0.010 | 0.063 | 0.2 | 0.305 | 0.448 |
| auction | sar/ideal | 0.675 | 0.680 | 0.163 | 0.3 | 0.370 | 0.549 |
| auction | sar/standard | 0.730 | 0.730 | 0.100 | 0.3 | 0.317 | 0.450 |
| connectivity-aware | sar/ideal | 0.640 | 0.650 | 0.135 | 0.3 | 0.370 | 0.571 |
| connectivity-aware | sar/standard | 0.735 | 0.745 | 0.100 | 0.3 | 0.315 | 0.433 |
| greedy | sar/ideal | 0.685 | 0.735 | 0.083 | 0.2 | 0.362 | 0.573 |
| greedy | sar/standard | 0.725 | 0.735 | 0.060 | 0.2 | 0.311 | 0.448 |
| cbba | sar/ideal | 0.005 | 0.010 | 0.142 | 0.3 | 0.371 | 0.570 |
| cbba | sar/standard | 0.000 | 0.000 | 0.087 | 0.3 | 0.315 | 0.442 |

## Inspection Detail

| Strategy | Profile | Success | EdgeCoverage | MissedEdges | RouteEfficiency |
|---|---|---:|---:|---:|---:|
| centralized | inspection/perimeter | 0.065 | 0.597 | 16.1 | 0.194 |
| centralized | inspection/random | 1.000 | 1.000 | 0.0 | 0.688 |
| centralized | inspection/linear | 1.000 | 1.000 | 0.0 | 0.556 |
| auction | inspection/perimeter | 0.235 | 0.769 | 9.2 | 0.150 |
| auction | inspection/random | 1.000 | 1.000 | 0.0 | 0.375 |
| auction | inspection/linear | 1.000 | 1.000 | 0.0 | 0.300 |
| connectivity-aware | inspection/perimeter | 0.190 | 0.762 | 9.5 | 0.150 |
| connectivity-aware | inspection/random | 1.000 | 1.000 | 0.0 | 0.378 |
| connectivity-aware | inspection/linear | 1.000 | 1.000 | 0.0 | 0.304 |
| greedy | inspection/perimeter | 0.420 | 0.865 | 5.4 | 0.076 |
| greedy | inspection/random | 1.000 | 1.000 | 0.0 | 0.222 |
| greedy | inspection/linear | 1.000 | 1.000 | 0.0 | 0.154 |
| cbba | inspection/perimeter | 0.025 | 0.908 | 3.7 | 0.101 |
| cbba | inspection/random | 0.750 | 0.954 | 1.1 | 0.223 |
| cbba | inspection/linear | 0.900 | 0.949 | 0.5 | 0.176 |

## Emergency Mesh Detail

| Strategy | Profile | Success | Completion | Availability | Realloc | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| centralized | emergency-mesh/packet-loss-10 | 0.725 | 1.000 | 0.618 | 0.000 | 0.0 |
| centralized | emergency-mesh/single-failure | 1.000 | 1.000 | 0.571 | 0.000 | 0.0 |
| centralized | emergency-mesh/medium-loss | 0.825 | 1.000 | 0.500 | 0.000 | 0.0 |
| centralized | emergency-mesh/low-loss | 0.715 | 1.000 | 0.618 | 0.000 | 0.0 |
| centralized | emergency-mesh/ideal | 0.715 | 1.000 | 0.618 | 0.000 | 0.0 |
| auction | emergency-mesh/packet-loss-10 | 0.370 | 0.645 | 0.648 | 0.000 | 1.8 |
| auction | emergency-mesh/single-failure | 0.555 | 0.555 | 0.281 | 0.000 | 1.4 |
| auction | emergency-mesh/medium-loss | 0.320 | 0.495 | 0.501 | 0.000 | 5.2 |
| auction | emergency-mesh/low-loss | 0.315 | 0.600 | 0.604 | 0.000 | 1.4 |
| auction | emergency-mesh/ideal | 0.365 | 0.650 | 0.653 | 0.000 | 1.3 |
| connectivity-aware | emergency-mesh/packet-loss-10 | 0.340 | 0.615 | 0.619 | 0.000 | 2.2 |
| connectivity-aware | emergency-mesh/single-failure | 0.620 | 0.620 | 0.264 | 0.000 | 1.4 |
| connectivity-aware | emergency-mesh/medium-loss | 0.310 | 0.485 | 0.492 | 0.000 | 4.8 |
| connectivity-aware | emergency-mesh/low-loss | 0.370 | 0.655 | 0.658 | 0.000 | 1.5 |
| connectivity-aware | emergency-mesh/ideal | 0.360 | 0.645 | 0.649 | 0.000 | 1.4 |
| greedy | emergency-mesh/packet-loss-10 | 0.395 | 0.670 | 0.673 | 0.000 | 2.6 |
| greedy | emergency-mesh/single-failure | 0.570 | 0.570 | 0.262 | 0.000 | 1.7 |
| greedy | emergency-mesh/medium-loss | 0.330 | 0.505 | 0.511 | 0.000 | 5.5 |
| greedy | emergency-mesh/low-loss | 0.350 | 0.635 | 0.639 | 0.000 | 1.8 |
| greedy | emergency-mesh/ideal | 0.325 | 0.610 | 0.614 | 0.000 | 1.7 |
| cbba | emergency-mesh/packet-loss-10 | 0.380 | 0.655 | 0.658 | 0.000 | 4.3 |
| cbba | emergency-mesh/single-failure | 0.640 | 0.640 | 0.260 | 0.000 | 3.8 |
| cbba | emergency-mesh/medium-loss | 0.335 | 0.515 | 0.523 | 0.000 | 6.0 |
| cbba | emergency-mesh/low-loss | 0.385 | 0.670 | 0.673 | 0.000 | 3.2 |
| cbba | emergency-mesh/ideal | 0.365 | 0.650 | 0.652 | 0.000 | 3.1 |

## Wildfire Detail

| Strategy | Profile | Success | Completion | ZonesMapped | PriorityUpdates | FinalThreat |
|---|---|---:|---:|---:|---:|---:|
| centralized | wildfire/small-static | 0.995 | 1.000 | 0.015 | 0.0 | 0.500 |
| centralized | wildfire/medium-dynamic | 0.975 | 1.000 | 0.100 | 2.9 | 0.317 |
| auction | wildfire/small-static | 0.995 | 1.000 | 0.015 | 0.0 | 0.500 |
| auction | wildfire/medium-dynamic | 0.975 | 1.000 | 0.100 | 2.9 | 0.317 |
| connectivity-aware | wildfire/small-static | 0.995 | 1.000 | 0.015 | 0.0 | 0.500 |
| connectivity-aware | wildfire/medium-dynamic | 0.975 | 1.000 | 0.100 | 2.9 | 0.317 |
| greedy | wildfire/small-static | 0.995 | 1.000 | 0.015 | 0.0 | 0.500 |
| greedy | wildfire/medium-dynamic | 0.995 | 1.000 | 0.020 | 0.6 | 0.303 |
| cbba | wildfire/small-static | 0.890 | 1.000 | 0.195 | 0.0 | 0.500 |
| cbba | wildfire/medium-dynamic | 0.350 | 1.000 | 2.840 | 88.2 | 0.813 |

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
