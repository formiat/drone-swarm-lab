# M97 Hardware-Entry Evidence Pack: Primitive

This artifact captures the M97 machine-checkable hardware-entry boundary for
`scenarios/primitive.takeoff-hold-land.json` and `agent-0`.

Generated from commit:

```text
ce729455c098a4546f369f9f71c820f998697e77
```

Generation command:

```bash
cargo run -p swarm-examples --bin sitl_agent -- \
  --hardware-entry-pack \
  --scenario scenarios/primitive.takeoff-hold-land.json \
  --agent-id agent-0 \
  --output-dir results/m97_hardware_entry_primitive_2026-06-10
```

Validation command:

```bash
cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir results/m97_hardware_entry_primitive_2026-06-10 \
  --mode hardware-entry-pack \
  --strict
```

Validation result: passed.

Pack summary:

- `pack_id`: `m97-primitive_takeoff_hold_land_3m_10s-agent-0`
- mission family: `primitive:takeoff-hold-land`
- readiness status: `execute_validated_locally`
- first allowed mission type: `primitive_takeoff_hold_land`
- single-drone gate passed: `true`
- multi-drone review required: `false`

Boundary:

- no hardware flight was performed;
- no certification, regulatory, or operator-training claim is made;
- local executor evidence must not be treated as proof that the mission is safe
  on real hardware.
