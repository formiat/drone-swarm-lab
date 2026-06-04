# Benchmark Results

This document records the current M69 simulation benchmark refresh, the older
historical M62 baseline, and the M78 benchmark evidence layer that makes future
packs easier to interpret.

The latest committed full release benchmark pack is
`results/all_1000_jobs14_m69_release/`, generated from benchmark code commit
`5d1d3cd17cacba7482c1d9b93eb5acc107af8f71`. The older
`results/all_500_jobs14_m62_release/` pack remains historical validation
evidence for commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd`.

M78 does not replace the M69 1000-seed pack and does not rerun it by default.
It adds report metadata: `success_stddev`, `success_stderr`, `success_ci95_low`,
`success_ci95_high`, `success_min`, `success_max`, `failure_rate`,
task-completion CI fields, and `support_status` / `support_reason`. New
`BenchmarkManifest` files also record `artifact_kind`; an artifact is current
evidence only for the `git_commit` recorded in its manifest. If the checked-out
HEAD differs, treat the pack as historical evidence until rerun.

M64 adds Urban foundation code and documentation, M65 adds Urban Patrol v0
simulation semantics, M66 adds Urban Search v1 with a deterministic mocked bus
detector, replay events, search metrics, and a smoke regression gate, and M67
adds Urban replay/analysis diagnostics. These milestones do not refresh
benchmark evidence. `scenarios/urban.patrol.json`, `scenarios/urban.search.json`,
and `scenarios/urban.multi-agent.json` are deterministic Urban fixtures for
smoke/regression/analysis checks, not benchmark baselines or publication runs.
M67 route-trace, judge-report, and timeline tooling should be treated as
diagnostic evidence, not as a new algorithmic benchmark result.

M85 adds `scenarios/urban.multi-agent-deconflict.json` and opt-in Urban
segment ownership metrics (`urban_deconflict_*`, `urban_segment_utilization`,
and `urban_avg_delay_per_agent_ticks`). This is current-head simulation
functionality for mission-level road-graph deconfliction. It is not part of
the historical M69/M62 benchmark packs and must not be described as physical
collision avoidance, PX4/SITL evidence, hardware readiness, lidar/raycast, RF
coordination, or real perception.

M68 adds one small algorithmic delta for Urban route planning:
`planner: "corridor-aware"` uses corridor width and static-obstacle clearance
to lower `urban_route_risk_score` on `scenarios/urban.corridor-delta.json`.
That fixture is not a full benchmark refresh. Treat
`results/m68_urban_corridor_delta/` as current-head before/after algorithm
evidence only.

M69 has now captured a 1000-seed release run for the built-in `--mission all`
benchmark suite. Current `--mission all` covers coverage, emergency-mesh, SAR,
inspection, and wildfire. Urban scenario-suite fixtures are not part of this
entrypoint yet, so Urban evidence remains the separate M68 artifact. M78 adds
explicit `--mission urban` for future Urban benchmark evidence without changing
the M69-compatible `--mission all` suite.

For live PX4/SIH evidence, see `docs/STATUS.md` and the `results/m48_*`,
`results/m55_*`, `results/m58_*`, and `results/m59_*` artifacts. Simulation
benchmark results must not be used as a substitute for PX4/SIH or hardware
validation.

M78 adds `--degradation coverage-packet-loss` as the first bounded degradation
sweep preset. Its output is simulation degradation evidence with
`artifact_kind: "degradation"`; the current artifact is
`results/m78_degradation_coverage_packet_loss_2026-06-03/`. It is not a
publication benchmark, PX4/SITL evidence, Gazebo/HIL evidence, hardware
evidence, or production safety evidence.
It is not hardware evidence.

M72 `artifact_validator` validates local SITL supervisor packs, including event
log/report/replay summary/safety consistency and replacement seq semantics. It
does not currently validate full benchmark result directories such as
`results/all_1000_jobs14_m69_release/`; benchmark-pack validation remains future
work. See `docs/ARTIFACT_VALIDATION.md`.

## M69 Current 1000-Seed Run

- **Date:** 2026-05-31
- **Benchmark run id:** `2026-05-31T193356Z_all_1000_full`
- **Benchmark code commit:** `5d1d3cd17cacba7482c1d9b93eb5acc107af8f71`
- **Build profile:** release
- **Mode:** custom 1000 seeds, built-in `--mission all` simulation suite
- **Jobs:** 14 Rayon worker jobs
- **Scenario runs:** `1000 seeds * 5 strategies * 38 profiles = 190000`
- **Aggregated rows:** 190
- **Runtime:** 28 min 55.25 sec
- **Peak RSS:** 207684 KB
- **Output pack:** `results/all_1000_jobs14_m69_release/`
- **Regression gate:** `target/release/regression_runner --jobs 14` passed

Command:

```bash
cargo build --release -p swarm-examples --bin strategy_comparison --bin regression_runner

/usr/bin/time -f 'RUN_TIME=%E\nPEAK_RSS_KB=%M' \
  /home/formi/.local/bin/runlim \
  target/release/strategy_comparison \
    --seeds 1000 \
    --mission all \
    --jobs 14 \
    --output-dir results/all_1000_jobs14_m69_release
```

Generated artifacts:

- `results/all_1000_jobs14_m69_release/manifest.json` - run metadata
- `results/all_1000_jobs14_m69_release/results.json` - machine-readable aggregate data
- `results/all_1000_jobs14_m69_release/results.csv` - tabular aggregate data
- `results/all_1000_jobs14_m69_release/table.md` - full Markdown table
- `results/all_1000_jobs14_m69_release/run.log` - captured stdout and runtime/RSS lines

Mission-level summary:

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
- This pack is simulation evidence only. It does not replace PX4/SIH evidence
  and does not include Urban scenario-suite fixtures.

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

The M62 pack predates M67, so it does not contain `urban_analysis/` route-trace
or judge-report artifacts and does not include the newer diagnostic Urban
separation/conflict metrics. Generate a new pack with `--replay-log` on current
HEAD if those diagnostics are needed for Urban work.

M75 adds moving mocked bus targets and Urban perimeter metrics in code, but it
does not refresh the benchmark evidence. Treat M75 validation as targeted
simulation/unit coverage unless a later milestone captures a dedicated Urban
scenario-suite pack.

M76 adds deterministic synthetic Urban scenario-suite generation and the tiny
checked-in fixture `scenarios/urban.generated.tiny.json`. This is reproducible
testbed infrastructure, not a benchmark refresh. The fixture carries
`generator_manifest` metadata so seed/category/parameter provenance is visible,
but no M69/M62 benchmark conclusions should be extended to generated Urban
suites without a dedicated run.

## M77 Targeted Algorithm Delta

M77 is an algorithm-differentiation milestone. It intentionally does not rerun
the full 1000-seed publication-style benchmark. The committed artifact is a
small release smoke run that validates the new targeted profile plumbing and
communication-aware allocation knobs.

Captured artifact:

- Output: `results/m77_algorithm_delta/coverage/`
- Command:
  `timeout 300 /home/formi/.local/bin/runlim cargo run --release -p swarm-examples --bin strategy_comparison -- --mission coverage --profiles m77-comms-heavy-loss,m77-comms-partition-prone --seeds 1 --jobs 1 --output-dir results/m77_algorithm_delta/coverage --run-id-prefix m77-coverage-smoke`
- Profiles:
  - `m77-comms-heavy-loss`
  - `m77-comms-partition-prone`
- New setting: `RunConfig.comms_penalty_weight = 50.0`
- Seeds: `1`
- Jobs: `1`

Interpretation:

- This is smoke evidence that targeted M77 profiles execute in release mode and
  that result manifests record the filtered profile set.
- It is not statistical evidence and must not be compared with M69 or M62 as a
  benchmark refresh.
- Controlled unit tests are the primary evidence that
  `comms_penalty_weight = 0.0` preserves old allocation behavior and non-zero
  weight changes constructed assignment cases.
- M77 also adds wildfire priority reallocation and SAR entropy-ordering tests,
  plus CBBA `conflict_count` replay diagnostics. It does not add a CBBA
  gossip-burst fix.

## M68 Urban Corridor Delta

M68 compares two `urban-patrol` profiles over the same deterministic road
graph:

- `corridor-delta-dijkstra`: baseline shortest-path planner;
- `corridor-delta-corridor-aware`: experimental planner that adds route-risk
  penalty from `corridor_width_m` and AABB obstacle clearance.

Captured artifact:

- Output: `results/m68_urban_corridor_delta/`
- Commit: `87e51a9331b65278f0f1fe5503958ca2ab35a998`
- Command: `cargo run -p swarm-examples --bin strategy_comparison -- --scenario-suite scenarios/urban.corridor-delta.json --output-dir results/m68_urban_corridor_delta --replay-log results/m68_urban_corridor_delta/replay --jobs 4`
- Dijkstra result: `avg_urban_route_length_m=40.000`,
  `avg_urban_route_risk_score=190.000`,
  `avg_urban_time_to_complete_loop=10.000`
- Corridor-aware result: `avg_urban_route_length_m=80.000`,
  `avg_urban_route_risk_score=70.000`,
  `avg_urban_time_to_complete_loop=20.000`

Interpretation:

- Dijkstra chooses the shorter narrow shortcut;
- corridor-aware chooses the longer safer detour;
- `avg_urban_route_risk_score` is lower for corridor-aware;
- `avg_urban_route_length_m` and `avg_urban_time_to_complete_loop` are higher
  for corridor-aware because the planner trades route length for lower static
  route risk.

This delta is intentionally small and scenario-local. It does not prove
general Urban superiority, M85 segment ownership behavior, lidar/CV behavior,
physical collision avoidance, PX4/SITL behavior, or hardware readiness.
Unsupported pairs such as SAR+CBBA and SAR+centralized remain unsupported with
their existing reasons; M68 does not implement failure-triggered gossip burst.

## M70 Urban Route Export

M70 is not a benchmark refresh. It adds a portable dry-run/SITL waypoint export
boundary for `urban-patrol`: planned Urban routes can be converted into ordered
waypoint plans with explicit altitude, `geo_origin`, route stats, stable route
identity fields, and `sitl_dry_run_artifact.v1` JSON artifacts.

Do not present M70 artifacts as `--mission all` evidence, PX4 execution
evidence, Gazebo/HIL evidence, hardware readiness, real perception, lidar, or
obstacle-avoidance validation.

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
| M69 refresh | release | 1000 | 14 | 28:55.25 | 207684 KB | `results/all_1000_jobs14_m69_release/` |

The M69 1000-seed run is the current release benchmark pack. The M62 rows are
preserved for historical runtime comparison only.

## Historical M62 Mission-Level Summary

Values below are from the historical M62 500-seed pack and are averaged across
all profiles of each mission for a strategy. For current M69 summary numbers,
use the M69 section above and the full `results/all_1000_jobs14_m69_release/`
artifact.

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

4. **SAR success remains effectively zero under the current strict predicate.**
   Auction, connectivity-aware, centralized, and greedy still reach high task
   completion for SAR, but success is only 0.000-0.002 per row. CBBA is much
   lower on completion. M78 distinguishes `probability_of_detection` and
   `targets_found` quality metrics from binary success. Legacy scenarios keep
   strict success (`all targets found`), while future SAR scenarios can opt into
   `run_config.sar_success_threshold` to define threshold success by
   found-target ratio. Treat SAR as an explicitly weak/open benchmark area
   unless the selected predicate is documented with the artifact.

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

1. Treat `results/all_1000_jobs14_m69_release/` as current release simulation
   benchmark evidence for the built-in benchmark suite at its recorded
   `git_commit`; after code changes, treat it as historical until rerun.
2. Treat `results/all_500_jobs14_m62_release/` as historical evidence for
   commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd`.
3. Do not present M69 as PX4/SITL, hardware, or Urban scenario-suite evidence.
4. Inspect SAR, wildfire, emergency-mesh, and CBBA weak rows before making
   publication-level algorithm claims. Use `support_status` and
   `support_reason` to avoid treating unsupported, known-bug, or caveated rows
   as success claims.
5. If broader Urban claims are needed, use explicit `--mission urban` or
   scenario-suite evidence instead of assuming `--mission all` already covers
   Urban.
6. Keep this document aligned with `README.md`, `docs/STATUS.md`, and the
   committed M69/M62 result artifacts.
