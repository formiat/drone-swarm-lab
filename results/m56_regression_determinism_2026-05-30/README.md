# M56 Regression Determinism Sweep - 2026-05-30

This directory captures the regression determinism sweep after fixing the
runtime ordering issues found during the first pass.

## Fixes Verified

- Runtime gossip merge now applies received assignments/generations in stable id
  order instead of `HashMap` iteration order.
- Agent movement now chooses the nearest assigned task deterministically, with
  `task_id` as the tie-breaker.
- SAR scan tasks are marked completed after a cell scan, so regression
  `task_completion_rate` no longer depends on released scan tasks being
  reallocated by chance.
- The SAR ideal smoke threshold now checks completed scans, targets found, and
  a calibrated entropy ceiling for the non-revisit scan behavior.

The `pre_fix/` and `pre_final_fix/` subdirectories contain the intermediate
failed sweeps that exposed the issue. The top-level files are the final
post-fix sweep.

## Commands

Release binaries were built once:

```bash
cargo build --release -p swarm-examples --bin regression_runner --bin strategy_comparison
```

Final sweep:

```bash
target/release/regression_runner --jobs 1
target/release/regression_runner --jobs 4
target/release/regression_runner --jobs 14
target/release/strategy_comparison --regression --jobs 1
target/release/strategy_comparison --regression --jobs 4
target/release/strategy_comparison --regression --jobs 14
```

Each `regression_runner` job setting was repeated three times. Each
`strategy_comparison --regression` job setting was repeated twice.

## Result

Artifact: `sweep-status.tsv`.

```text
kind	jobs	run	exit_code
regression_runner	1	1	0
regression_runner	1	2	0
regression_runner	1	3	0
regression_runner	4	1	0
regression_runner	4	2	0
regression_runner	4	3	0
regression_runner	14	1	0
regression_runner	14	2	0
regression_runner	14	3	0
strategy_comparison	1	1	0
strategy_comparison	1	2	0
strategy_comparison	4	1	0
strategy_comparison	4	2	0
strategy_comparison	14	1	0
strategy_comparison	14	2	0
```

The default regression entrypoints are now stable across the tested job counts
and repeated runs.
