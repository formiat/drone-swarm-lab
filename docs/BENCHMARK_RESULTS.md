# Benchmark Results

This document records the M62 simulation benchmark refresh for commit
`81260ca7afa114a5d9add7b832f6c5d7875b88cd`. M63 did not rerun the benchmark, so
`results/all_500_jobs14_m62_release/` is now historical validation evidence,
not current-HEAD evidence. It supersedes the older M32b benchmark report as the
latest committed full benchmark pack, but it is still a 500-seed validation
baseline rather than a publication-grade 1000-seed statistical run.

M64 adds Urban foundation code and documentation but does not refresh benchmark
evidence. `scenarios/urban.patrol.json` is a deterministic foundation fixture,
not a benchmark baseline or publication run.

For live PX4/SIH evidence, see `docs/STATUS.md` and the `results/m48_*`,
`results/m55_*`, `results/m58_*`, and `results/m59_*` artifacts. Simulation
benchmark results must not be used as a substitute for PX4/SIH or hardware
validation.

## Historical M62 Run

- **Date:** 2026-05-31
- **Benchmark run id:** `2026-05-31T023230Z_all_500_custom`
- **Benchmark git commit:** `81260ca7afa114a5d9add7b832f6c5d7875b88cd`
- **Evidence status after M63:** historical evidence for the commit above;
  rerun before using it as current-HEAD evidence
- **Build profile:** release
- **Mode:** custom 500 seeds, all built-in simulation missions
- **Jobs:** 14 Rayon worker jobs
- **Scenario runs:** `500 seeds * 5 strategies * 38 profiles = 95000`
- **Aggregated rows:** 190
- **Runtime:** 16 min 7.34 sec
- **Peak RSS:** 92264 KB
- **Output pack:** `results/all_500_jobs14_m62_release/`
- **Identity check:** 0 bad rows for per-row `benchmark_run_id`, `mission`,
  `scenario`, and `profile`

Command:

```bash
cargo build --release -p swarm-examples --bin strategy_comparison

/usr/bin/time -f 'elapsed=%E maxrss_kb=%M' \
  target/release/strategy_comparison \
    --seeds 500 \
    --mission all \
    --jobs 14 \
    --output-dir results/all_500_jobs14_m62_release
```

Generated artifacts:

- `results/all_500_jobs14_m62_release/manifest.json` - run metadata
- `results/all_500_jobs14_m62_release/results.json` - machine-readable aggregate data
- `results/all_500_jobs14_m62_release/results.csv` - tabular aggregate data
- `results/all_500_jobs14_m62_release/table.md` - full Markdown table

Historical validation packs are preserved in:

- `results/all_200_jobs14_m62_release/`
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
| M62 refresh | release | 500 | 14 | 16:07.34 | 92264 KB | `results/all_500_jobs14_m62_release/` |

The M62 500-seed release runtime scales close to the seed-count increase from
the M62 200-seed run while covering the codebase at commit
`81260ca7afa114a5d9add7b832f6c5d7875b88cd` and 38 profiles.

## Mission-Level Summary

Values below are averaged across all profiles of each mission for a strategy.

| Mission | Strategy | Profiles | Success | Completion | Availability | Conflicts |
|---|---|---:|---:|---:|---:|---:|
| coverage | auction | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | cbba | 24 | 0.750 | 1.000 | 0.992 | 0.0 |
| coverage | centralized | 24 | 1.000 | 1.000 | 0.991 | 0.0 |
| coverage | connectivity-aware | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| coverage | greedy | 24 | 1.000 | 1.000 | 0.992 | 0.0 |
| emergency-mesh | auction | 5 | 0.388 | 0.560 | 0.505 | 2.2 |
| emergency-mesh | cbba | 5 | 0.427 | 0.600 | 0.534 | 4.2 |
| emergency-mesh | centralized | 5 | 0.828 | 1.000 | 0.551 | 0.0 |
| emergency-mesh | connectivity-aware | 5 | 0.405 | 0.577 | 0.519 | 2.3 |
| emergency-mesh | greedy | 5 | 0.418 | 0.590 | 0.529 | 2.3 |
| inspection | auction | 3 | 1.000 | 1.000 | 0.389 | 19568.4 |
| inspection | cbba | 3 | 1.000 | 0.527 | 0.610 | 6625.5 |
| inspection | centralized | 3 | 1.000 | 1.000 | 0.389 | 10315.0 |
| inspection | connectivity-aware | 3 | 1.000 | 1.000 | 0.389 | 19568.4 |
| inspection | greedy | 3 | 1.000 | 1.000 | 0.999 | 10347.4 |
| sar | auction | 2 | 0.001 | 0.994 | 0.041 | 50290.0 |
| sar | cbba | 2 | 0.000 | 0.023 | 0.027 | 8184.1 |
| sar | centralized | 2 | 0.001 | 1.000 | 0.022 | 20040.1 |
| sar | connectivity-aware | 2 | 0.001 | 0.994 | 0.041 | 50290.0 |
| sar | greedy | 2 | 0.000 | 0.999 | 0.040 | 49980.8 |
| wildfire | auction | 4 | 0.247 | 1.000 | 1.000 | 1013.9 |
| wildfire | cbba | 4 | 0.127 | 1.000 | 1.000 | 1978.8 |
| wildfire | centralized | 4 | 0.247 | 1.000 | 1.000 | 1013.9 |
| wildfire | connectivity-aware | 4 | 0.247 | 1.000 | 1.000 | 1013.9 |
| wildfire | greedy | 4 | 0.247 | 1.000 | 1.000 | 1013.9 |

## Key Findings

1. **Coverage remains solved for non-CBBA strategies.** Auction, centralized,
   connectivity-aware, and greedy all reach 1.000 average success across the 24
   coverage profiles.

2. **CBBA coverage still has concentrated failure rows.** Six high-loss /
   high-latency failure profiles report `Success = 0.000` while completion and
   coverage progress remain 1.000. `coverage/partition-prone-cascade-failure`
   is near solved at 0.996 success. This should be treated as a support-matrix
   or success-semantics issue to inspect before using CBBA coverage as a strong
   claim.

3. **Emergency mesh still favors centralized planning.** Centralized reaches
   0.828 average success and 1.000 completion with zero conflicts. The other
   strategies cluster around 0.39-0.43 success and 0.56-0.60 completion.

4. **SAR success remains effectively zero under the current success predicate.**
   Auction, connectivity-aware, centralized, and greedy still reach high task
   completion for SAR, but success is only 0.000-0.002 per row. CBBA is much
   lower on completion. Treat SAR as an explicitly weak/open benchmark area
   until the mission success predicate and target-found expectations are
   reviewed.

5. **Inspection is now success-stable, but metrics still distinguish quality.**
   Every inspection row reports success 1.000. CBBA has lower task completion
   on perimeter/random, and route efficiency differs strongly by strategy.

6. **Wildfire success is low despite full task completion.** Non-CBBA
   strategies average 0.247 success, CBBA averages 0.127, and all strategies
   report 1.000 completion. This is expected under the M35/M63 wildfire success
   predicate: `success=true` requires
   `mapped_zone_count / total_zone_count >= wildfire_success_threshold` (default
   `0.8`), all expected failures detected, and
   `max_task_unassigned_ticks <= max_unassigned_ticks`. Task completion only
   says tasks were assigned/completed; it does not guarantee enough distinct
   hazard zones were mapped to satisfy the stricter mission success predicate.

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
| coverage/partition-prone-cascade-failure | 0.996 | 0.996 | 1.000 | 0.000 | 0.992 |

### SAR Detail

| Strategy | Profile | Success | Completion | PoD | Targets | BeliefEntropy | FalsePosRate |
|---|---|---:|---:|---:|---:|---:|---:|
| auction | sar/ideal | 0.002 | 1.000 | 0.570 | 1.140 | 0.627 | 0.554 |
| auction | sar/standard | 0.000 | 0.988 | 0.435 | 1.306 | 0.434 | 0.429 |
| cbba | sar/ideal | 0.000 | 0.036 | 0.555 | 1.110 | 0.609 | 0.538 |
| cbba | sar/standard | 0.000 | 0.010 | 0.425 | 1.274 | 0.429 | 0.426 |
| centralized | sar/ideal | 0.002 | 1.000 | 0.565 | 1.130 | 0.614 | 0.539 |
| centralized | sar/standard | 0.000 | 1.000 | 0.441 | 1.322 | 0.431 | 0.423 |
| connectivity-aware | sar/ideal | 0.002 | 1.000 | 0.570 | 1.140 | 0.627 | 0.554 |
| connectivity-aware | sar/standard | 0.000 | 0.988 | 0.435 | 1.306 | 0.434 | 0.429 |
| greedy | sar/ideal | 0.000 | 1.000 | 0.574 | 1.148 | 0.628 | 0.555 |
| greedy | sar/standard | 0.000 | 0.998 | 0.447 | 1.342 | 0.435 | 0.430 |

### Wildfire Detail

| Strategy | Profile | Success | Completion | ZonesMapped | PriorityUpdates | FinalThreat |
|---|---|---:|---:|---:|---:|---:|
| auction | wildfire/high-threat-dynamic | 0.230 | 1.000 | 3.988 | 104.000 | 1.000 |
| auction | wildfire/large-static | 0.284 | 1.000 | 5.744 | 0.000 | 0.467 |
| auction | wildfire/medium-dynamic | 0.230 | 1.000 | 3.970 | 116.000 | 1.000 |
| auction | wildfire/small-static | 0.246 | 1.000 | 1.498 | 0.000 | 0.500 |
| cbba | wildfire/high-threat-dynamic | 0.094 | 1.000 | 3.910 | 104.000 | 1.000 |
| cbba | wildfire/large-static | 0.128 | 1.000 | 5.556 | 0.000 | 0.467 |
| cbba | wildfire/medium-dynamic | 0.104 | 1.000 | 3.878 | 116.000 | 1.000 |
| cbba | wildfire/small-static | 0.182 | 1.000 | 1.446 | 0.000 | 0.500 |
| greedy | wildfire/high-threat-dynamic | 0.230 | 1.000 | 3.988 | 104.000 | 1.000 |
| greedy | wildfire/large-static | 0.284 | 1.000 | 5.744 | 0.000 | 0.467 |
| greedy | wildfire/medium-dynamic | 0.230 | 1.000 | 3.970 | 116.000 | 1.000 |
| greedy | wildfire/small-static | 0.246 | 1.000 | 1.498 | 0.000 | 0.500 |

For wildfire rows, `Completion = 1.000` means the task-completion floor passed.
It is not equivalent to mission success. Mission success is computed from the
mapped-zone ratio and the failure/unassigned guards described above, which is
why `Success` remains below `Completion` in the M62 pack.

## Next Steps

1. Treat `results/all_500_jobs14_m62_release/` as historical evidence for
   commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd` until a current-HEAD rerun
   is performed.
2. Inspect the SAR success predicate before turning this baseline into a
   publication claim; wildfire success semantics are now documented as a
   mapped-ratio predicate with failure/unassigned guards.
3. Decide whether CBBA coverage under high-loss/high-latency failures should be
   explicitly unsupported or fixed.
4. If publication-level evidence is needed, run
   `--seeds 1000 --mission all --jobs 14` after the above interpretation work.
5. Keep this document aligned with `README.md`, `docs/STATUS.md`, and the
   committed `results/all_500_jobs14_m62_release/` artifact.
