# M55 Multi-Agent PX4 SIH Upload-Only Result - 2026-05-30

This directory captures the optional/manual two-agent PX4 SIH check. It verifies
that two local PX4 SIH instances can each accept the correct per-agent mission
subset in upload-only mode.

## Environment

- PX4 source path: `/home/formi/Documents/RustProjects/PX4-Autopilot`.
- PX4 commit: `a2be919`.
- PX4 backend: `px4_sitl_sih`, model `sihsim_quadx`.
- Startup method: two direct PX4 SIH instances using separate working
  directories under `build/px4_sitl_sih/instance_0` and `instance_1`.
- PX4 instance ids: `-i 0` and `-i 1`.

Observed MAVLink ports:

```text
instance 0: Normal local 18570 -> remote 14550; Onboard local 14580 -> remote 14540
instance 1: Normal local 18571 -> remote 14550; Onboard local 14581 -> remote 14541
```

The shared normal/GCS listener `udpin:0.0.0.0:14550` is not suitable for
sequential per-agent uploads in this setup; the captured `agent-0-upload.txt`
attempt timed out waiting for `MISSION_REQUEST seq=0`. The successful uploads
use the per-instance onboard listener ports.

## Commands

Agent 0:

```bash
cargo run -p swarm-examples --bin sitl_agent --features mavlink-transport -- \
  --connection udpin:0.0.0.0:14540 \
  --scenario scenarios/sitl.multi-agent.json \
  --agent-id agent-0 \
  --multi-agent-config scenarios/sitl.multi-agent.config.json \
  --allow-hardware-candidate \
  --upload-only \
  --timeout 8 \
  --replay-log results/m55_multi_agent_px4_sih_2026-05-30/agent-0-onboard.sitl-log.json
```

Agent 1:

```bash
cargo run -p swarm-examples --bin sitl_agent --features mavlink-transport -- \
  --connection udpin:0.0.0.0:14541 \
  --scenario scenarios/sitl.multi-agent.json \
  --agent-id agent-1 \
  --multi-agent-config scenarios/sitl.multi-agent.config.json \
  --allow-hardware-candidate \
  --upload-only \
  --timeout 8 \
  --replay-log results/m55_multi_agent_px4_sih_2026-05-30/agent-1-onboard.sitl-log.json
```

## Result

Agent 0:

```text
Real MAVLink mode: mission accepted; lifecycle=upload-only uploaded_count=2 target_system=1 target_component=1 cleared_existing=true
```

Agent 1:

```text
Real MAVLink mode: mission accepted; lifecycle=upload-only uploaded_count=2 target_system=2 target_component=1 cleared_existing=true
```

Replay summaries:

```text
agent-0: ack_accepted=1 ack_rejected=0 final_status=upload_accepted
agent-1: ack_accepted=1 ack_rejected=0 final_status=upload_accepted
```

## Scope

This is a multi-agent PX4 SIH upload-only check. It does not arm, take off,
execute missions, coordinate simultaneous flight, or verify live PX4
failure/reallocation.
