# M77 Targeted Algorithm Delta - Coverage

This artifact is a small targeted M77 smoke run, not a publication benchmark.

Command:

```bash
timeout 300 /home/formi/.local/bin/runlim cargo run --release -p swarm-examples --bin strategy_comparison -- --mission coverage --profiles m77-comms-heavy-loss,m77-comms-partition-prone --seeds 1 --jobs 1 --output-dir results/m77_algorithm_delta/coverage --run-id-prefix m77-coverage-smoke
```

Scope:

- Mission: `coverage`
- Profiles: `m77-comms-heavy-loss`, `m77-comms-partition-prone`
- Seeds: `1`
- Jobs: `1`
- Build profile: `release`
- New M77 knob: `RunConfig.comms_penalty_weight = 50.0` for both profiles

Interpretation:

- This run verifies that the new profile filter and communication-aware allocator plumbing execute successfully in release mode.
- It is intentionally too small for statistical claims.
- The committed controlled allocator tests are the primary evidence that `comms_penalty_weight = 0.0` preserves old behavior and non-zero weight changes assignment in constructed cases.
- CBBA still has a known heavy-loss/failure limitation; M77 adds conflict-count replay diagnostics but does not claim a gossip-burst fix.
