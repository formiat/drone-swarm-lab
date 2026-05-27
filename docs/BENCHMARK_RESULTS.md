# Benchmark Results

This document records the current post-M32b benchmark pack. It replaces the
older pre-M32 quick report whose mixed-mission rows had stale
`mission`/`scenario` identity fields.

## Current Run

- **Date:** 2026-05-27
- **Benchmark run id:** `2026-05-27T155737Z_all_50_custom`
- **Git commit:** `e309ad1c282cff9e5a1072403f703c6849334375`
- **Mode:** custom 50 seeds, all built-in missions
- **Jobs:** 14 Rayon worker jobs on a 16-core machine
- **Scenario runs:** `50 seeds * 5 strategies * 36 profiles = 9000`
- **Aggregated rows:** 180
- **Runtime:** 9 min 28.44 sec
- **Peak RSS:** 41992 KB
- **Output pack:** `results/all_50_jobs14_m32b/`
- **Identity check:** 0 bad rows for per-row `mission`, `scenario`, `profile`, and run id

Command:

```bash
/usr/bin/time -f 'elapsed=%E maxrss_kb=%M' \
  cargo run -q -p swarm-examples --bin strategy_comparison -- \
    --seeds 50 \
    --mission all \
    --jobs 14 \
    --output-dir results/all_50_jobs14_m32b
```

Generated artifacts:

- `results/all_50_jobs14_m32b/manifest.json` - run metadata
- `results/all_50_jobs14_m32b/results.json` - machine-readable aggregate data
- `results/all_50_jobs14_m32b/results.csv` - tabular aggregate data
- `results/all_50_jobs14_m32b/table.md` - full markdown table

The previous 20-seed validation pack is preserved in
`results/all_20_jobs14_m32b/`. The 50-seed run is still a validation/custom
run, not a publishable statistical run. The next publishable benchmark should
use the same post-M32b path with `--full` or `--seeds 1000`.

## Mission-Level Summary

Values below are averaged across all profiles of each mission for a strategy.

| Mission | Strategy | Profiles | Success | Completion | Availability | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| coverage | auction | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | cbba | 24 | 0.749 | 0.999 | 0.992 | 0.0 |
| coverage | centralized | 24 | 1.000 | 1.000 | 0.991 | 0.0 |
| coverage | connectivity-aware | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | greedy | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| emergency-mesh | auction | 5 | 0.396 | 0.664 | 0.597 | 2.0 |
| emergency-mesh | cbba | 5 | 0.428 | 0.696 | 0.610 | 4.0 |
| emergency-mesh | centralized | 5 | 0.732 | 1.000 | 0.662 | 0.0 |
| emergency-mesh | connectivity-aware | 5 | 0.388 | 0.656 | 0.586 | 1.9 |
| emergency-mesh | greedy | 5 | 0.416 | 0.684 | 0.599 | 2.4 |
| inspection | auction | 3 | 0.693 | 0.693 | 0.306 | 11928.5 |
| inspection | cbba | 3 | 0.547 | 0.547 | 0.739 | 6511.5 |
| inspection | centralized | 3 | 0.687 | 0.687 | 0.342 | 6816.3 |
| inspection | connectivity-aware | 3 | 0.720 | 0.720 | 0.310 | 11826.1 |
| inspection | greedy | 3 | 0.787 | 0.787 | 0.674 | 11643.7 |
| sar | auction | 2 | 0.790 | 0.790 | 0.076 | 1595.4 |
| sar | cbba | 2 | 0.000 | 0.010 | 0.010 | 1191.0 |
| sar | centralized | 2 | 0.000 | 0.000 | 0.020 | 169.9 |
| sar | connectivity-aware | 2 | 0.680 | 0.680 | 0.078 | 1215.1 |
| sar | greedy | 2 | 0.710 | 0.740 | 0.075 | 4787.1 |
| wildfire | auction | 2 | 0.980 | 1.000 | 1.000 | 26.5 |
| wildfire | cbba | 2 | 0.630 | 1.000 | 1.000 | 763.4 |
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
   The distributed strategies are functional but lower: CBBA 0.428, greedy
   0.416, auction 0.396, and connectivity-aware 0.388.

4. **SAR still exposes planner gaps.** Auction has the best average success
   at 0.790. Greedy follows at 0.710, connectivity-aware at 0.680, while
   centralized and CBBA remain at 0.000 success. This keeps SAR as the clearest
   mission for testing grid/belief task support.

5. **Inspection is profile-sensitive.** Greedy has the best average inspection
   success at 0.787, followed by connectivity-aware at 0.720. Linear and
   random are solved by most strategies, but perimeter remains difficult:
   greedy reaches 0.360, connectivity-aware 0.160, auction 0.080, centralized
   0.060, and CBBA 0.000.

6. **Wildfire is mostly strong except CBBA on dynamic fire spread.** Greedy
   reaches 1.000 average success; auction, centralized, and
   connectivity-aware reach 0.980. CBBA improves over the 20-seed run but
   remains much lower at 0.630 because `wildfire/medium-dynamic` succeeds only
   0.360 of the time.

## Coverage Notes

Coverage rows with `Success < 1.000` are isolated to CBBA:

| Strategy | Profile | Success | Completion | Coverage | Realloc | Availability |
|---|---|---:|---:|---:|---:|---:|
| cbba | coverage/heavy-loss-multiple-failures | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/high-latency-single-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/high-latency-multiple-failures | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/high-latency-cascade-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-cascade-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/partition-prone-cascade-failure | 0.980 | 0.980 | 0.000 | 0.000 | 0.991 |
| cbba | coverage/heavy-loss-single-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |

## SAR Detail

| Strategy | Profile | Success | Completion | PoD | Targets | BeliefEntropy | FalsePosRate |
|---|---|---:|---:|---:|---:|---:|---:|
| greedy | sar/standard | 0.740 | 0.760 | 0.073 | 0.2 | 0.312 | 0.469 |
| greedy | sar/ideal | 0.680 | 0.720 | 0.130 | 0.3 | 0.366 | 0.587 |
| centralized | sar/standard | 0.000 | 0.000 | 0.053 | 0.2 | 0.305 | 0.454 |
| centralized | sar/ideal | 0.000 | 0.000 | 0.140 | 0.3 | 0.336 | 0.535 |
| cbba | sar/standard | 0.000 | 0.020 | 0.120 | 0.4 | 0.316 | 0.448 |
| cbba | sar/ideal | 0.000 | 0.000 | 0.180 | 0.4 | 0.387 | 0.587 |
| connectivity-aware | sar/standard | 0.840 | 0.840 | 0.060 | 0.2 | 0.318 | 0.476 |
| connectivity-aware | sar/ideal | 0.520 | 0.520 | 0.220 | 0.4 | 0.372 | 0.560 |
| auction | sar/standard | 0.800 | 0.800 | 0.073 | 0.2 | 0.317 | 0.453 |
| auction | sar/ideal | 0.780 | 0.780 | 0.160 | 0.3 | 0.380 | 0.621 |

## Inspection Detail

| Strategy | Profile | Success | EdgeCoverage | MissedEdges | RouteEfficiency |
|---|---|---:|---:|---:|---:|
| greedy | inspection/linear | 1.000 | 1.000 | 0.0 | 0.162 |
| greedy | inspection/perimeter | 0.360 | 0.861 | 5.6 | 0.077 |
| greedy | inspection/random | 1.000 | 1.000 | 0.0 | 0.230 |
| centralized | inspection/linear | 1.000 | 1.000 | 0.0 | 0.543 |
| centralized | inspection/perimeter | 0.060 | 0.603 | 15.9 | 0.194 |
| centralized | inspection/random | 1.000 | 1.000 | 0.0 | 0.676 |
| cbba | inspection/linear | 0.900 | 0.956 | 0.4 | 0.185 |
| cbba | inspection/perimeter | 0.000 | 0.883 | 4.7 | 0.107 |
| cbba | inspection/random | 0.740 | 0.955 | 1.0 | 0.217 |
| connectivity-aware | inspection/linear | 1.000 | 1.000 | 0.0 | 0.297 |
| connectivity-aware | inspection/perimeter | 0.160 | 0.762 | 9.5 | 0.151 |
| connectivity-aware | inspection/random | 1.000 | 1.000 | 0.0 | 0.387 |
| auction | inspection/linear | 1.000 | 1.000 | 0.0 | 0.321 |
| auction | inspection/perimeter | 0.080 | 0.763 | 9.5 | 0.151 |
| auction | inspection/random | 1.000 | 1.000 | 0.0 | 0.393 |

## Emergency Mesh Detail

| Strategy | Profile | Success | Completion | Availability | Realloc | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| greedy | emergency-mesh/packet-loss-10 | 0.280 | 0.660 | 0.665 | 0.000 | 2.2 |
| greedy | emergency-mesh/low-loss | 0.380 | 0.760 | 0.761 | 0.000 | 1.3 |
| greedy | emergency-mesh/ideal | 0.300 | 0.680 | 0.684 | 0.000 | 1.4 |
| greedy | emergency-mesh/medium-loss | 0.400 | 0.600 | 0.605 | 0.000 | 5.5 |
| greedy | emergency-mesh/single-failure | 0.720 | 0.720 | 0.277 | 0.000 | 1.7 |
| centralized | emergency-mesh/packet-loss-10 | 0.620 | 1.000 | 0.702 | 0.000 | 0.0 |
| centralized | emergency-mesh/low-loss | 0.620 | 1.000 | 0.702 | 0.000 | 0.0 |
| centralized | emergency-mesh/ideal | 0.620 | 1.000 | 0.702 | 0.000 | 0.0 |
| centralized | emergency-mesh/medium-loss | 0.800 | 1.000 | 0.584 | 0.000 | 0.0 |
| centralized | emergency-mesh/single-failure | 1.000 | 1.000 | 0.623 | 0.000 | 0.0 |
| cbba | emergency-mesh/packet-loss-10 | 0.320 | 0.700 | 0.720 | 0.000 | 4.0 |
| cbba | emergency-mesh/low-loss | 0.400 | 0.780 | 0.779 | 0.000 | 3.6 |
| cbba | emergency-mesh/ideal | 0.380 | 0.760 | 0.761 | 0.000 | 2.6 |
| cbba | emergency-mesh/medium-loss | 0.340 | 0.540 | 0.550 | 0.000 | 6.5 |
| cbba | emergency-mesh/single-failure | 0.700 | 0.700 | 0.238 | 0.000 | 3.4 |
| connectivity-aware | emergency-mesh/packet-loss-10 | 0.360 | 0.740 | 0.742 | 0.000 | 1.6 |
| connectivity-aware | emergency-mesh/low-loss | 0.340 | 0.720 | 0.722 | 0.000 | 1.2 |
| connectivity-aware | emergency-mesh/ideal | 0.280 | 0.660 | 0.664 | 0.000 | 0.5 |
| connectivity-aware | emergency-mesh/medium-loss | 0.300 | 0.500 | 0.507 | 0.000 | 5.1 |
| connectivity-aware | emergency-mesh/single-failure | 0.660 | 0.660 | 0.295 | 0.000 | 1.2 |
| auction | emergency-mesh/packet-loss-10 | 0.360 | 0.740 | 0.742 | 0.000 | 1.5 |
| auction | emergency-mesh/low-loss | 0.280 | 0.660 | 0.664 | 0.000 | 1.2 |
| auction | emergency-mesh/ideal | 0.300 | 0.680 | 0.683 | 0.000 | 0.8 |
| auction | emergency-mesh/medium-loss | 0.380 | 0.580 | 0.586 | 0.000 | 5.2 |
| auction | emergency-mesh/single-failure | 0.660 | 0.660 | 0.312 | 0.000 | 1.1 |

## Wildfire Detail

| Strategy | Profile | Success | Completion | ZonesMapped | PriorityUpdates | FinalThreat |
|---|---|---:|---:|---:|---:|---:|
| greedy | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| greedy | wildfire/medium-dynamic | 1.000 | 1.000 | 0.000 | 0.0 | 0.300 |
| centralized | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| centralized | wildfire/medium-dynamic | 0.960 | 1.000 | 0.160 | 4.6 | 0.327 |
| cbba | wildfire/small-static | 0.900 | 1.000 | 0.180 | 0.0 | 0.500 |
| cbba | wildfire/medium-dynamic | 0.360 | 1.000 | 2.840 | 85.8 | 0.800 |
| connectivity-aware | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| connectivity-aware | wildfire/medium-dynamic | 0.960 | 1.000 | 0.160 | 4.6 | 0.327 |
| auction | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| auction | wildfire/medium-dynamic | 0.960 | 1.000 | 0.160 | 4.6 | 0.327 |

## Next Steps

1. Investigate the CBBA coverage rows where completion is high but coverage is
   0.000.
2. Decide whether SAR failures for centralized/CBBA are expected planner
   limitations or missing task adapters for grid/belief tasks.
3. Run the same all-mission benchmark with `--seeds 100` as an intermediate
   confidence pass after the suspicious CBBA coverage rows are understood.
4. After the suspicious rows are resolved or explicitly documented, run the
   publishable `--seeds 1000 --mission all --jobs <N>` benchmark and update
   this document with the final pack.
