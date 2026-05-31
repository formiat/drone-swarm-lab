# M62 200-Seed Release Benchmark

This directory contains the M62 simulation benchmark refresh for commit
`a32b1f4888719abb491f38ddc9dfbdb63d3957e2`.

## Run Metadata

- Date: 2026-05-31
- Benchmark run id: `2026-05-31T021752Z_all_200_custom`
- Build profile: release
- Command: `target/release/strategy_comparison --seeds 200 --mission all --jobs 14 --output-dir results/all_200_jobs14_m62_release`
- Jobs: 14
- Seed range: 0..200
- Strategies: auction, cbba, centralized, connectivity-aware, greedy
- Profiles: 38
- Aggregate rows: 190
- Runtime: 6 min 21.94 sec
- Peak RSS: 43096 KB

## Files

- `manifest.json` - run metadata and command line
- `results.json` - machine-readable aggregate metrics
- `results.csv` - tabular aggregate metrics
- `table.md` - full Markdown table

## Notes

This is a current-HEAD validation baseline, not a publication-grade 1000-seed
statistical run. It also does not replace PX4/SIH evidence: it only evaluates
simulation behavior.
