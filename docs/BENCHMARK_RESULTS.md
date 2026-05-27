# Benchmark Results

This document records the current post-M32b benchmark pack. It replaces the
older pre-M32 quick report whose mixed-mission rows had stale
`mission`/`scenario` identity fields.

## Current Run

- **Date:** 2026-05-27
- **Benchmark run id:** `2026-05-27T154001Z_all_20_custom`
- **Git commit:** `62913c0b0f5d67acdd2ed2bc8e6de5086bcff498`
- **Mode:** custom 20 seeds, all built-in missions
- **Jobs:** 14 Rayon worker jobs on a 16-core machine
- **Scenario runs:** `20 seeds * 5 strategies * 36 profiles = 3600`
- **Aggregated rows:** 180
- **Runtime:** 4 min 23.12 sec
- **Peak RSS:** 42104 KB
- **Output pack:** `results/all_20_jobs14_m32b/`
- **Identity check:** 0 bad rows for per-row `mission`, `scenario`, `profile`, and run id

Command:

```bash
/usr/bin/time -f 'elapsed=%E maxrss_kb=%M' \
  cargo run -q -p swarm-examples --bin strategy_comparison -- \
    --seeds 20 \
    --mission all \
    --jobs 14 \
    --output-dir results/all_20_jobs14_m32b
```

Generated artifacts:

- `results/all_20_jobs14_m32b/manifest.json` - run metadata
- `results/all_20_jobs14_m32b/results.json` - machine-readable aggregate data
- `results/all_20_jobs14_m32b/results.csv` - tabular aggregate data
- `results/all_20_jobs14_m32b/table.md` - full markdown table

This is still a validation/custom run, not a publishable statistical run. The
next publishable benchmark should use the same path after the M32b fixes, but
with `--full` or `--seeds 1000`.

## Mission-Level Summary

Values below are averaged across all profiles of each mission for a strategy.

| Mission | Strategy | Profiles | Success | Completion | Availability | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| coverage | auction | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | cbba | 24 | 0.750 | 1.000 | 0.992 | 0.0 |
| coverage | centralized | 24 | 1.000 | 1.000 | 0.991 | 0.0 |
| coverage | connectivity-aware | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | greedy | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| emergency-mesh | auction | 5 | 0.360 | 0.580 | 0.515 | 1.8 |
| emergency-mesh | cbba | 5 | 0.420 | 0.640 | 0.572 | 4.2 |
| emergency-mesh | centralized | 5 | 0.780 | 1.000 | 0.586 | 0.0 |
| emergency-mesh | connectivity-aware | 5 | 0.430 | 0.650 | 0.574 | 2.0 |
| emergency-mesh | greedy | 5 | 0.380 | 0.600 | 0.534 | 2.0 |
| inspection | auction | 3 | 0.783 | 0.783 | 0.313 | 12182.5 |
| inspection | cbba | 3 | 0.617 | 0.617 | 0.743 | 6796.8 |
| inspection | centralized | 3 | 0.700 | 0.700 | 0.340 | 6823.7 |
| inspection | connectivity-aware | 3 | 0.800 | 0.800 | 0.299 | 12253.2 |
| inspection | greedy | 3 | 0.783 | 0.783 | 0.605 | 12468.8 |
| sar | auction | 2 | 0.700 | 0.700 | 0.028 | 1744.4 |
| sar | cbba | 2 | 0.000 | 0.025 | 0.001 | 1182.1 |
| sar | centralized | 2 | 0.000 | 0.000 | 0.050 | 163.8 |
| sar | connectivity-aware | 2 | 0.700 | 0.700 | 0.048 | 816.5 |
| sar | greedy | 2 | 0.675 | 0.675 | 0.083 | 4674.0 |
| wildfire | auction | 2 | 0.975 | 1.000 | 1.000 | 31.1 |
| wildfire | cbba | 2 | 0.575 | 1.000 | 1.000 | 803.7 |
| wildfire | centralized | 2 | 0.975 | 1.000 | 1.000 | 31.1 |
| wildfire | connectivity-aware | 2 | 0.975 | 1.000 | 1.000 | 31.1 |
| wildfire | greedy | 2 | 1.000 | 1.000 | 1.000 | 0.0 |

## Key Findings

1. **Coverage is mostly solved for the current profiles.** Auction,
   centralized, connectivity-aware, and greedy reached 1.000 average success
   across all 24 coverage profiles. CBBA completed all tasks, but reports
   0.000 success in six high-latency/heavy-loss failure profiles, so its
   coverage success predicate still deserves investigation.

2. **Emergency mesh currently favors centralized allocation.** Centralized
   reached 0.780 average success and 1.000 completion with zero conflicts.
   The distributed strategies are functional, but lower: connectivity-aware
   0.430, CBBA 0.420, greedy 0.380, auction 0.360.

3. **SAR still exposes strategy-specific gaps.** Auction and
   connectivity-aware average 0.700 success, greedy is close at 0.675, while
   centralized and CBBA remain at 0.000 success. This matches the existing
   concern that SAR grid/belief tasks are not handled equally well by all
   planners.

4. **Inspection is profile-sensitive.** Linear and random profiles are solved
   by most strategies, but perimeter is hard. Connectivity-aware has the best
   average success across inspection profiles (0.800); centralized has the
   best route efficiency on linear/random but weak perimeter success.

5. **Wildfire is mostly strong except CBBA on dynamic fire spread.** Greedy
   reaches 1.000 average success; auction, centralized, and
   connectivity-aware reach 0.975. CBBA drops to 0.575 average success because
   `wildfire/medium-dynamic` succeeds only 0.250 of the time.

## Coverage Notes

Coverage failures are isolated to CBBA rows:

| Strategy | Profile | Success | Completion | Coverage | Realloc | Availability |
|---|---|---:|---:|---:|---:|---:|
| cbba | coverage/high-latency-multiple-failures | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-multiple-failures | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-single-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/high-latency-cascade-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/heavy-loss-cascade-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |
| cbba | coverage/high-latency-single-failure | 0.000 | 1.000 | 0.000 | 0.000 | 1.000 |

The suspicious part is the combination `Completion = 1.000` with
`Coverage = 0.000`. Before treating this as an algorithmic result, inspect the
coverage success predicate and metric extraction for CBBA in these profiles.

## SAR Detail

| Strategy | Profile | Success | Completion | PoD | Targets | BeliefEntropy | FalsePosRate |
|---|---|---:|---:|---:|---:|---:|---:|
| auction | sar/standard | 0.700 | 0.700 | 0.117 | 0.3 | 0.318 | 0.480 |
| auction | sar/ideal | 0.700 | 0.700 | 0.150 | 0.3 | 0.381 | 0.603 |
| cbba | sar/standard | 0.000 | 0.050 | 0.050 | 0.1 | 0.316 | 0.446 |
| cbba | sar/ideal | 0.000 | 0.000 | 0.150 | 0.3 | 0.384 | 0.586 |
| centralized | sar/standard | 0.000 | 0.000 | 0.050 | 0.1 | 0.304 | 0.440 |
| centralized | sar/ideal | 0.000 | 0.000 | 0.075 | 0.1 | 0.334 | 0.543 |
| connectivity-aware | sar/standard | 0.600 | 0.600 | 0.133 | 0.4 | 0.315 | 0.421 |
| connectivity-aware | sar/ideal | 0.800 | 0.800 | 0.150 | 0.3 | 0.363 | 0.533 |
| greedy | sar/standard | 0.650 | 0.650 | 0.067 | 0.2 | 0.310 | 0.422 |
| greedy | sar/ideal | 0.700 | 0.700 | 0.150 | 0.3 | 0.377 | 0.595 |

## Inspection Detail

| Strategy | Profile | Success | EdgeCoverage | MissedEdges | RouteEfficiency |
|---|---|---:|---:|---:|---:|
| auction | inspection/linear | 1.000 | 1.000 | 0.0 | 0.292 |
| auction | inspection/random | 1.000 | 1.000 | 0.0 | 0.402 |
| auction | inspection/perimeter | 0.350 | 0.739 | 10.4 | 0.148 |
| cbba | inspection/linear | 0.950 | 0.930 | 0.7 | 0.186 |
| cbba | inspection/random | 0.850 | 0.955 | 1.1 | 0.223 |
| cbba | inspection/perimeter | 0.050 | 0.890 | 4.4 | 0.098 |
| centralized | inspection/linear | 1.000 | 1.000 | 0.0 | 0.529 |
| centralized | inspection/random | 1.000 | 1.000 | 0.0 | 0.681 |
| centralized | inspection/perimeter | 0.100 | 0.573 | 17.1 | 0.191 |
| connectivity-aware | inspection/linear | 1.000 | 1.000 | 0.0 | 0.302 |
| connectivity-aware | inspection/random | 1.000 | 1.000 | 0.0 | 0.420 |
| connectivity-aware | inspection/perimeter | 0.400 | 0.739 | 10.4 | 0.142 |
| greedy | inspection/linear | 1.000 | 1.000 | 0.0 | 0.155 |
| greedy | inspection/random | 1.000 | 1.000 | 0.0 | 0.209 |
| greedy | inspection/perimeter | 0.350 | 0.842 | 6.3 | 0.075 |

## Emergency Mesh Detail

| Strategy | Profile | Success | Completion | Availability | Realloc | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| auction | emergency-mesh/medium-loss | 0.350 | 0.550 | 0.555 | 0.000 | 4.0 |
| auction | emergency-mesh/single-failure | 0.600 | 0.600 | 0.252 | 0.000 | 1.1 |
| auction | emergency-mesh/ideal | 0.300 | 0.600 | 0.605 | 0.000 | 1.4 |
| auction | emergency-mesh/packet-loss-10 | 0.250 | 0.550 | 0.556 | 0.000 | 1.6 |
| auction | emergency-mesh/low-loss | 0.300 | 0.600 | 0.605 | 0.000 | 1.1 |
| cbba | emergency-mesh/medium-loss | 0.350 | 0.550 | 0.555 | 0.000 | 5.0 |
| cbba | emergency-mesh/single-failure | 0.600 | 0.600 | 0.252 | 0.000 | 2.8 |
| cbba | emergency-mesh/ideal | 0.450 | 0.750 | 0.751 | 0.000 | 4.7 |
| cbba | emergency-mesh/packet-loss-10 | 0.350 | 0.650 | 0.651 | 0.000 | 5.9 |
| cbba | emergency-mesh/low-loss | 0.350 | 0.650 | 0.652 | 0.000 | 3.0 |
| centralized | emergency-mesh/medium-loss | 0.800 | 1.000 | 0.554 | 0.000 | 0.0 |
| centralized | emergency-mesh/single-failure | 1.000 | 1.000 | 0.565 | 0.000 | 0.0 |
| centralized | emergency-mesh/ideal | 0.700 | 1.000 | 0.603 | 0.000 | 0.0 |
| centralized | emergency-mesh/packet-loss-10 | 0.700 | 1.000 | 0.603 | 0.000 | 0.0 |
| centralized | emergency-mesh/low-loss | 0.700 | 1.000 | 0.603 | 0.000 | 0.0 |
| connectivity-aware | emergency-mesh/medium-loss | 0.350 | 0.550 | 0.556 | 0.000 | 5.0 |
| connectivity-aware | emergency-mesh/single-failure | 0.650 | 0.650 | 0.254 | 0.000 | 1.2 |
| connectivity-aware | emergency-mesh/ideal | 0.250 | 0.550 | 0.556 | 0.000 | 1.1 |
| connectivity-aware | emergency-mesh/packet-loss-10 | 0.450 | 0.750 | 0.751 | 0.000 | 1.6 |
| connectivity-aware | emergency-mesh/low-loss | 0.450 | 0.750 | 0.751 | 0.000 | 0.9 |
| greedy | emergency-mesh/medium-loss | 0.300 | 0.500 | 0.508 | 0.000 | 4.5 |
| greedy | emergency-mesh/single-failure | 0.600 | 0.600 | 0.251 | 0.000 | 1.4 |
| greedy | emergency-mesh/ideal | 0.400 | 0.700 | 0.702 | 0.000 | 0.9 |
| greedy | emergency-mesh/packet-loss-10 | 0.200 | 0.500 | 0.506 | 0.000 | 2.5 |
| greedy | emergency-mesh/low-loss | 0.400 | 0.700 | 0.702 | 0.000 | 0.9 |

## Wildfire Detail

| Strategy | Profile | Success | Completion | ZonesMapped | PriorityUpdates | FinalThreat |
|---|---|---:|---:|---:|---:|---:|
| auction | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| auction | wildfire/medium-dynamic | 0.950 | 1.000 | 0.200 | 5.8 | 0.334 |
| cbba | wildfire/small-static | 0.900 | 1.000 | 0.250 | 0.0 | 0.500 |
| cbba | wildfire/medium-dynamic | 0.250 | 1.000 | 3.050 | 92.8 | 0.840 |
| centralized | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| centralized | wildfire/medium-dynamic | 0.950 | 1.000 | 0.200 | 5.8 | 0.334 |
| connectivity-aware | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| connectivity-aware | wildfire/medium-dynamic | 0.950 | 1.000 | 0.200 | 5.8 | 0.334 |
| greedy | wildfire/small-static | 1.000 | 1.000 | 0.000 | 0.0 | 0.500 |
| greedy | wildfire/medium-dynamic | 1.000 | 1.000 | 0.000 | 0.0 | 0.300 |

## Next Steps

1. Investigate the CBBA coverage rows where completion is 1.000 but coverage
   and success are 0.000.
2. Decide whether SAR failures for centralized/CBBA are expected planner
   limitations or missing task adapters for grid/belief tasks.
3. Run the same all-mission benchmark with `--seeds 100` as an intermediate
   confidence pass.
4. After the suspicious rows are understood, run the publishable
   `--seeds 1000 --mission all --jobs <N>` benchmark and update this document
   with the final pack.
