# M89 Dual-Stack Dry-Run Evidence

This artifact records portable PX4 and ArduPilot dry-run evidence for the
canonical primitive `takeoff-hold-land` mission.

## Commands

```bash
timeout 300 /home/formi/.local/bin/runlim cargo run -p swarm-examples --bin sitl_dual_stack_evidence -- \
  --scenario scenarios/primitive.takeoff-hold-land.json \
  --agent-id agent-0 \
  --output-dir results/m89_dual_stack_evidence \
  --force

timeout 300 /home/formi/.local/bin/runlim cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir results/m89_dual_stack_evidence \
  --mode dual-stack-evidence \
  --strict
```

Validator result:

```text
artifact validation passed: results/m89_dual_stack_evidence
```

## Files

- `sitl_dual_stack_evidence_pack.v1.json`
- `px4/sitl_dry_run_artifact.v1.json`
- `ardupilot/sitl_dry_run_artifact.v1.json`

## Boundary

This is dry-run evidence only. It does not start PX4 or ArduPilot, does not
connect to MAVLink transport, does not prove ArduPilot command acceptance, does
not prove PX4/ArduPilot behavior equivalence, does not validate live
replacement/failover, and does not claim hardware readiness or certified flight
safety.

For this primitive single-agent pack, `abort_replacement.replacement_policy` is
`not_applicable_single_agent_primitive`. The pack records timeout abort policy,
terminal state, profile caveats, and FC/safety contract summaries.
