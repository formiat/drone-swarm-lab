# M59 PX4/SIH Failure Reallocation

Date: 2026-05-31

Scope: controlled local PX4/SIH only. This artifact demonstrates one manual
failure-injection path with active-survivor mission replacement. It does not
claim Gazebo, HIL, hardware, automated PX4 CI, production flight readiness, or
coverage of a broad failure matrix.

## Environment

- PX4 checkout: `/home/formi/Documents/RustProjects/PX4-Autopilot`
- PX4 commit: `a2be919`
- PX4 backend: `build/px4_sitl_sih/bin/px4`
- PX4 model: `sihsim_quadx`
- Supervisor mode: `sitl_supervisor --connection --execute --reupload-on-failure`
- Agents: 2 local PX4/SIH instances
- Endpoints:
  - agent-0: `udpin:0.0.0.0:14540`, system id 1
  - agent-1: `udpin:0.0.0.0:14541`, system id 2

## PX4 Startup

From `/home/formi/Documents/RustProjects/PX4-Autopilot/build/px4_sitl_sih`:

```bash
PX4_SIM_SPEED_FACTOR=1 PX4_SIM_MODEL=sihsim_quadx \
  bin/px4 -d -i 0 > /home/formi/Documents/RustProjects/drone/results/m59_px4_sih_failure_reallocation_2026-05-31/px4-agent-0.log 2>&1

PX4_SIM_SPEED_FACTOR=1 PX4_SIM_MODEL=sihsim_quadx \
  bin/px4 -d -i 1 > /home/formi/Documents/RustProjects/drone/results/m59_px4_sih_failure_reallocation_2026-05-31/px4-agent-1.log 2>&1
```

## Supervisor Command

```bash
cargo run -p swarm-examples --bin sitl_supervisor --features mavlink-transport -- \
  --connection --execute \
  --scenario results/m59_px4_sih_failure_reallocation_2026-05-31/sitl.multi-agent.failure.scenario.json \
  --config results/m59_px4_sih_failure_reallocation_2026-05-31/sitl.multi-agent.failure.onboard.config.json \
  --allow-hardware-candidate \
  --timeout 8 \
  --telemetry-timeout 5 \
  --no-progress-timeout 180 \
  --reupload-on-failure \
  --output-dir results/m59_px4_sih_failure_reallocation_2026-05-31 \
  --run-id m59-px4-sih-failure-reallocation \
  --force
```

Failure injection:

```text
2026-05-31T10:43:54-0300 killing PX4 instance 0
2026-05-31T10:44:11-0300 kill signal sent
```

Full stdout/stderr is in `supervisor-run.txt`.

## Result

- `final_status`: `completed_with_reallocation`
- agent-0 final status: `disconnected`, completed 0/2 initial tasks.
- agent-1 final status: `completed`, completed 4/4 after mission replacement.
- `agent_lost`: 1
- `task_released`: 2
- `task_reassigned`: 2
- `reallocation_completed`: 1
- `survivor_mission_updates`: 1
- `tasks_recovered`: 2
- `reallocation_latency_ticks`: 0

Primary artifacts:

- `m59-px4-sih-failure-reallocation/events.sitl-log.json`
- `m59-px4-sih-failure-reallocation/run-report.json`
- `m59-px4-sih-failure-reallocation/manifest.json`
- `m59-px4-sih-failure-reallocation/replay-summary.txt`
