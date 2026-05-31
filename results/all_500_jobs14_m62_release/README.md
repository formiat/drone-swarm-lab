# M62 500-Seed Release Benchmark

This directory contains the M62 simulation benchmark refresh for commit
`81260ca7afa114a5d9add7b832f6c5d7875b88cd`.

M63 did not rerun this benchmark. Treat this directory as historical validation
evidence for that commit, not as current-HEAD evidence, unless a future
benchmark refresh explicitly regenerates it.

## Run Metadata

- Date: 2026-05-31
- Benchmark run id: `2026-05-31T023230Z_all_500_custom`
- Build profile: release
- Command: `target/release/strategy_comparison --seeds 500 --mission all --jobs 14 --output-dir results/all_500_jobs14_m62_release`
- Jobs: 14
- Seed range: 0..500
- Strategies: auction, cbba, centralized, connectivity-aware, greedy
- Profiles: 38
- Aggregate rows: 190
- Scenario runs: 95000
- Runtime: 16 min 7.34 sec
- Peak RSS: 92264 KB

## Files

- `manifest.json` - run metadata and command line
- `results.json` - machine-readable aggregate metrics
- `results.csv` - tabular aggregate metrics
- `table.md` - full Markdown table

## Notes

This is a historical 500-seed validation baseline for commit
`81260ca7afa114a5d9add7b832f6c5d7875b88cd`, not a publication-grade 1000-seed
statistical run. It also does not replace PX4/SIH evidence: it only evaluates
simulation behavior.
