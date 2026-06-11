# M97 Hardware-Entry Evidence Pack: Urban Single-Drone

This artifact captures the M97 machine-checkable hardware-entry boundary for
`scenarios/urban.geo-block-loop.json` and `agent-0`.

Generated from commit:

```text
ce729455c098a4546f369f9f71c820f998697e77
```

Generation command:

```bash
cargo run -p swarm-examples --bin sitl_agent -- \
  --hardware-entry-pack \
  --scenario scenarios/urban.geo-block-loop.json \
  --agent-id agent-0 \
  --output-dir results/m97_hardware_entry_urban_single_2026-06-10
```

Validation command:

```bash
cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir results/m97_hardware_entry_urban_single_2026-06-10 \
  --mode hardware-entry-pack \
  --strict
```

Validation result: passed.

Pack summary:

- `pack_id`: `m97-urban_geo_block_loop-agent-0`
- mission family: `urban:single-drone`
- readiness status: `dry_run_only`
- first allowed mission type: `urban_single_drone`
- single-drone gate passed: `true`
- multi-drone review required: `false`

Boundary:

- no hardware flight was performed;
- this is Urban single-drone export/local-executor evidence, not real
  perception, collision avoidance, PX4/SIH execution, or hardware approval;
- multi-drone Urban hardware remains blocked until a separate review gate.
