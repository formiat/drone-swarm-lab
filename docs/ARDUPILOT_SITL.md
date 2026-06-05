# ArduPilot SITL Runbook

This document is the M89 optional ArduPilot SITL boundary. Automated tests do
not require ArduPilot, Gazebo, HIL, hardware, network services, or a running
simulator.

## What M89 Provides

M89 provides dry-run dual-stack evidence:

```bash
cargo run -p swarm-examples --bin sitl_dual_stack_evidence -- \
  --scenario scenarios/primitive.takeoff-hold-land.json \
  --agent-id agent-0 \
  --output-dir target/m89-dual-stack \
  --force

cargo run -p swarm-examples --bin artifact_validator -- \
  --output-dir target/m89-dual-stack \
  --mode dual-stack-evidence \
  --strict
```

The generated pack contains:

- `px4/sitl_dry_run_artifact.v1.json`;
- `ardupilot/sitl_dry_run_artifact.v1.json`;
- `sitl_dual_stack_evidence_pack.v1.json`;
- one shared `command_ir_hash`;
- profile-specific warnings;
- expected ACK and telemetry counts;
- explicit `abort_replacement` evidence;
- explicit `fc_safety_contract` evidence.

For primitive single-agent evidence, replacement is
`not_applicable_single_agent_primitive`. That means timeout abort policy and
terminal state are recorded, but live survivor replacement is not claimed.

## Optional Local ArduPilot SITL

ArduPilot SITL evidence is optional/manual. Dry-run dual-stack evidence does not prove ArduPilot command acceptance, mode behavior, failsafe behavior, or hardware readiness.

When an operator has a local ArduPilot SITL setup, use the local simulator's
documented launch command and connect only to loopback endpoints. The repository
does not install or start ArduPilot automatically.

Operator-provided example shape:

```bash
# Terminal 1: start local ArduPilot SITL using the operator's installed command.
# Example placeholder only; replace with the local ArduPilot environment command.
ARDUPILOT_SITL_CMD='operator-provided local SITL command'

# Terminal 2: generate portable dry-run evidence first.
cargo run -p swarm-examples --bin sitl_dual_stack_evidence -- \
  --scenario scenarios/primitive.takeoff-hold-land.json \
  --agent-id agent-0 \
  --output-dir target/m89-dual-stack \
  --force
```

Do not describe this as live ArduPilot SITL evidence unless the operator also
captures a local run artifact with command transcript, simulator version,
connection endpoint, and validator output.

## Stop Conditions

Stop and do not claim M89 live ArduPilot evidence when:

- the endpoint is not local loopback;
- the simulator command is unknown or not operator-provided;
- command ACK, mode transition, mission start, telemetry, abort, or failsafe
  behavior is not captured;
- `artifact_validator --mode dual-stack-evidence --strict` fails;
- PX4 and ArduPilot behavior is described as equivalent based only on dry-run
  artifacts;
- FC/safety contract evidence is described as certified flight safety.

## Boundary

M89 is a pre-hardware evidence discipline milestone with no hardware readiness claim. It keeps PX4 first-class
through existing PX4/SIH history and prevents the codebase from becoming
PX4-only by preserving an ArduPilot profile workflow. It is not production
readiness, hardware validation, HIL validation, or flight certification.
It also does not prove PX4/ArduPilot behavior equivalence.
