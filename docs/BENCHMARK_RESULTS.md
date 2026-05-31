# Benchmark Results

This document records the current M62 simulation benchmark refresh for the
repository HEAD. It supersedes the older M32b benchmark report as the default
benchmark reference, but it is still a 200-seed validation baseline rather than
a publication-grade 1000-seed statistical run.

For live PX4/SIH evidence, see `docs/STATUS.md` and the `results/m48_*`,
`results/m55_*`, and SITL supervisor artifacts. Simulation benchmark results
must not be used as a substitute for PX4/SIH or hardware validation.

## Current Run

- **Date:** 2026-05-31
- **Benchmark run id:** `2026-05-31T021752Z_all_200_custom`
- **Benchmark git commit:** `a32b1f4888719abb491f38ddc9dfbdb63d3957e2`
- **Build profile:** release
- **Mode:** custom 200 seeds, all built-in simulation missions
- **Jobs:** 14 Rayon worker jobs
- **Scenario runs:** `200 seeds * 5 strategies * 38 profiles = 38000`
- **Aggregated rows:** 190
- **Runtime:** 6 min 21.94 sec
- **Peak RSS:** 43096 KB
- **Output pack:** `results/all_200_jobs14_m62_release/`
- **Identity check:** 0 bad rows for per-row `benchmark_run_id`, `mission`,
  `scenario`, and `profile`

Command:

```bash
cargo build --release -p swarm-examples --bin strategy_comparison

/usr/bin/time -f 'elapsed=%E maxrss_kb=%M' \
  target/release/strategy_comparison \
    --seeds 200 \
    --mission all \
    --jobs 14 \
    --output-dir results/all_200_jobs14_m62_release
```

Generated artifacts:

- `results/all_200_jobs14_m62_release/manifest.json` - run metadata
- `results/all_200_jobs14_m62_release/results.json` - machine-readable aggregate data
- `results/all_200_jobs14_m62_release/results.csv` - tabular aggregate data
- `results/all_200_jobs14_m62_release/table.md` - full Markdown table

Historical validation packs are preserved in:

- `results/all_500_jobs14_m32b_release/`
- `results/all_200_jobs14_m32b_release/`
- `results/all_50_jobs14_m32b_release/`
- `results/all_50_jobs14_m32b/`
- `results/all_20_jobs14_m32b/`

## Runtime History

| Run | Build | Seeds | Jobs | Runtime | Peak RSS | Output |
|---|---|---:|---:|---:|---:|---|
| M32b validation | debug | 20 | 14 | 4:23.12 | 42104 KB | `results/all_20_jobs14_m32b/` |
| M32b validation | debug | 50 | 14 | 9:28.44 | 41992 KB | `results/all_50_jobs14_m32b/` |
| M32b validation | release | 50 | 14 | 1:30.12 | 16480 KB | `results/all_50_jobs14_m32b_release/` |
| M32b validation | release | 200 | 14 | 6:16.21 | 37228 KB | `results/all_200_jobs14_m32b_release/` |
| M32b validation | release | 500 | 14 | 14:50.67 | 76232 KB | `results/all_500_jobs14_m32b_release/` |
| M62 refresh | release | 200 | 14 | 6:21.94 | 43096 KB | `results/all_200_jobs14_m62_release/` |

The M62 200-seed release runtime is close to the older M32b 200-seed release
runtime, while covering the current post-M61 codebase and 38 profiles.

## Mission-Level Summary

Values below are averaged across all profiles of each mission for a strategy.

| Mission | Strategy | Profiles | Success | Completion | Availability | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| coverage | auction | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | cbba | 24 | 0.750 | 1.000 | 0.992 | 0.0 |
| coverage | centralized | 24 | 1.000 | 1.000 | 0.991 | 0.0 |
| coverage | connectivity-aware | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | greedy | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| emergency-mesh | auction | 5 | 0.401 | 0.605 | 0.534 | 2.1 |
| emergency-mesh | cbba | 5 | 0.431 | 0.636 | 0.557 | 4.1 |
| emergency-mesh | centralized | 5 | 0.796 | 1.000 | 0.585 | 0.0 |
| emergency-mesh | connectivity-aware | 5 | 0.403 | 0.607 | 0.542 | 2.2 |
| emergency-mesh | greedy | 5 | 0.427 | 0.631 | 0.566 | 2.2 |
| inspection | auction | 3 | 1.000 | 1.000 | 0.393 | 19653.7 |
| inspection | cbba | 3 | 1.000 | 0.547 | 0.603 | 6550.0 |
| inspection | centralized | 3 | 1.000 | 1.000 | 0.393 | 10365.0 |
| inspection | connectivity-aware | 3 | 1.000 | 1.000 | 0.393 | 19653.7 |
| inspection | greedy | 3 | 1.000 | 1.000 | 0.999 | 10401.6 |
| sar | auction | 2 | 0.000 | 0.992 | 0.040 | 50251.3 |
| sar | cbba | 2 | 0.000 | 0.033 | 0.015 | 7860.8 |
| sar | centralized | 2 | 0.000 | 1.000 | 0.027 | 19640.5 |
| sar | connectivity-aware | 2 | 0.000 | 0.992 | 0.040 | 50251.3 |
| sar | greedy | 2 | 0.000 | 0.998 | 0.036 | 49907.4 |
| wildfire | auction | 4 | 0.251 | 1.000 | 1.000 | 1023.3 |
| wildfire | cbba | 4 | 0.111 | 1.000 | 1.000 | 2007.8 |
| wildfire | centralized | 4 | 0.251 | 1.000 | 1.000 | 1023.3 |
| wildfire | connectivity-aware | 4 | 0.251 | 1.000 | 1.000 | 1023.3 |
| wildfire | greedy | 4 | 0.251 | 1.000 | 1.000 | 1023.3 |

## Key Findings

1. **Coverage remains solved for non-CBBA strategies.** Auction, centralized,
   connectivity-aware, and greedy all reach 1.000 average success across the 24
   coverage profiles.

2. **CBBA coverage still has concentrated failure rows.** Six high-loss /
   high-latency failure profiles report `Success = 0.000` while completion and
   coverage progress remain 1.000. `coverage/partition-prone-cascade-failure`
   is near solved at 0.990 success. This should be treated as a support-matrix
   or success-semantics issue to inspect before using CBBA coverage as a strong
   claim.

3. **Emergency mesh still favors centralized planning.** Centralized reaches
   0.796 average success and 1.000 completion with zero conflicts. The other
   strategies cluster around 0.40-0.43 success and 0.60-0.64 completion.

4. **SAR success is zero under the current success predicate.** Auction,
   connectivity-aware, centralized, and greedy still reach high task completion
   for SAR, but all SAR rows have `Success = 0.000`. CBBA is much lower on
   completion. Treat SAR as an explicitly weak/open benchmark area until the
   mission success predicate and target-found expectations are reviewed.

5. **Inspection is now success-stable, but metrics still distinguish quality.**
   Every inspection row reports success 1.000. CBBA has lower task completion
   on perimeter/random, and route efficiency differs strongly by strategy.

6. **Wildfire success is low despite full task completion.** Non-CBBA
   strategies average 0.251 success, CBBA averages 0.111, and all strategies
   report 1.000 completion. This suggests the current wildfire success
   threshold is stricter than task assignment/completion and should be reported
   as such.

## Notable Rows

### CBBA Coverage Rows With Success Below 1.000

| Profile | Success | Completion | Coverage | Realloc | Availability |
|---|---:|---:|---:|---:|---:|
| coverage/heavy-loss-cascade-failure | 0.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| coverage/heavy-loss-multiple-failures | 0.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| coverage/heavy-loss-single-failure | 0.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| coverage/high-latency-cascade-failure | 0.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| coverage/high-latency-multiple-failures | 0.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| coverage/high-latency-single-failure | 0.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| coverage/partition-prone-cascade-failure | 0.990 | 0.990 | 0.999 | 0.000 | 0.992 |

### SAR Detail

| Strategy | Profile | Success | Completion | PoD | Targets | BeliefEntropy | FalsePosRate |
|---|---|---:|---:|---:|---:|---:|---:|
| auction | sar/ideal | 0.000 | 1.000 | 0.555 | 1.110 | 0.643 | 0.575 |
| auction | sar/standard | 0.000 | 0.985 | 0.467 | 1.400 | 0.445 | 0.448 |
| cbba | sar/ideal | 0.000 | 0.055 | 0.540 | 1.080 | 0.626 | 0.561 |
| cbba | sar/standard | 0.000 | 0.010 | 0.445 | 1.335 | 0.439 | 0.445 |
| centralized | sar/ideal | 0.000 | 1.000 | 0.560 | 1.120 | 0.634 | 0.564 |
| centralized | sar/standard | 0.000 | 1.000 | 0.452 | 1.355 | 0.441 | 0.440 |
| connectivity-aware | sar/ideal | 0.000 | 1.000 | 0.555 | 1.110 | 0.643 | 0.575 |
| connectivity-aware | sar/standard | 0.000 | 0.985 | 0.467 | 1.400 | 0.445 | 0.448 |
| greedy | sar/ideal | 0.000 | 1.000 | 0.562 | 1.125 | 0.644 | 0.576 |
| greedy | sar/standard | 0.000 | 0.995 | 0.482 | 1.445 | 0.446 | 0.449 |

### Wildfire Detail

| Strategy | Profile | Success | Completion | ZonesMapped | PriorityUpdates | FinalThreat |
|---|---|---:|---:|---:|---:|---:|
| auction | wildfire/high-threat-dynamic | 0.240 | 1.000 | 3.995 | 104.000 | 1.000 |
| auction | wildfire/large-static | 0.305 | 1.000 | 5.765 | 0.000 | 0.467 |
| auction | wildfire/medium-dynamic | 0.240 | 1.000 | 3.975 | 116.000 | 1.000 |
| auction | wildfire/small-static | 0.220 | 1.000 | 1.510 | 0.000 | 0.500 |
| cbba | wildfire/high-threat-dynamic | 0.065 | 1.000 | 3.950 | 104.000 | 1.000 |
| cbba | wildfire/large-static | 0.125 | 1.000 | 5.550 | 0.000 | 0.467 |
| cbba | wildfire/medium-dynamic | 0.115 | 1.000 | 3.920 | 116.000 | 1.000 |
| cbba | wildfire/small-static | 0.140 | 1.000 | 1.480 | 0.000 | 0.500 |
| greedy | wildfire/high-threat-dynamic | 0.240 | 1.000 | 3.995 | 104.000 | 1.000 |
| greedy | wildfire/large-static | 0.305 | 1.000 | 5.765 | 0.000 | 0.467 |
| greedy | wildfire/medium-dynamic | 0.240 | 1.000 | 3.975 | 116.000 | 1.000 |
| greedy | wildfire/small-static | 0.220 | 1.000 | 1.510 | 0.000 | 0.500 |

## Next Steps

1. Inspect the SAR and wildfire success predicates before turning this baseline
   into a publication claim.
2. Decide whether CBBA coverage under high-loss/high-latency failures should be
   explicitly unsupported or fixed.
3. If publication-level evidence is needed, run
   `--seeds 1000 --mission all --jobs 14` after the above interpretation work.
4. Keep this document aligned with `README.md`, `docs/STATUS.md`, and the
   committed `results/all_200_jobs14_m62_release/` artifact.
