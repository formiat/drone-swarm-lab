# M58 Multi-Agent PX4/SIH Execute

Date: 2026-05-31

Scope: controlled local PX4/SIH only. This artifact does not claim Gazebo,
HIL, hardware, automated PX4 CI, or production flight readiness.

## Environment

- PX4 checkout: `/home/formi/Documents/RustProjects/PX4-Autopilot`
- PX4 commit: `a2be919`
- PX4 backend: `build/px4_sitl_sih/bin/px4`
- PX4 model: `sihsim_quadx`
- Supervisor mode: `sitl_supervisor --connection --execute`
- Agents: 2 local PX4/SIH instances
- Endpoints:
  - agent-0: `udpin:0.0.0.0:14540`, system id 1
  - agent-1: `udpin:0.0.0.0:14541`, system id 2

## PX4 Startup

From `/home/formi/Documents/RustProjects/PX4-Autopilot/build/px4_sitl_sih`:

```bash
PX4_SIM_SPEED_FACTOR=1 PX4_SIM_MODEL=sihsim_quadx \
  bin/px4 -d -i 0 > /home/formi/Documents/RustProjects/drone/results/m58_multi_agent_px4_sih_execute_2026-05-31/px4-agent-0.log 2>&1

PX4_SIM_SPEED_FACTOR=1 PX4_SIM_MODEL=sihsim_quadx \
  bin/px4 -d -i 1 > /home/formi/Documents/RustProjects/drone/results/m58_multi_agent_px4_sih_execute_2026-05-31/px4-agent-1.log 2>&1
```

## Supervisor Command

```bash
cargo run -p swarm-examples --bin sitl_supervisor --features mavlink-transport -- \
  --connection --execute \
  --scenario scenarios/sitl.multi-agent.json \
  --config results/m58_multi_agent_px4_sih_execute_2026-05-31/sitl.multi-agent.execute.onboard.config.json \
  --allow-hardware-candidate \
  --timeout 8 \
  --telemetry-timeout 20 \
  --no-progress-timeout 120 \
  --output-dir results/m58_multi_agent_px4_sih_execute_2026-05-31 \
  --run-id m58-multi-agent-px4-sih-execute \
  --force
```

Full stdout/stderr is in `supervisor-run.txt`.

## Result

- `final_status`: `completed`
- `agent_lost`: 0
- `task_reassigned`: 0
- `agents_started`: 2
- `agents_finished`: 2
- `task_completed`: 4
- agent-0 completed 2/2 assigned tasks.
- agent-1 completed 2/2 assigned tasks.

Primary artifacts:

- `m58-multi-agent-px4-sih-execute/events.sitl-log.json`
- `m58-multi-agent-px4-sih-execute/run-report.json`
- `m58-multi-agent-px4-sih-execute/manifest.json`
- `m58-multi-agent-px4-sih-execute/replay-summary.txt`
