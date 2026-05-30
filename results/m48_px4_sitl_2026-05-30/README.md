# M48 PX4 SITL Result - 2026-05-30

This directory captures the M48 single-agent golden path run against a live
local PX4 SIH/SITL instance.

## Environment

- Host OS: Ubuntu 25.10.
- PX4 source path: `/home/formi/Documents/RustProjects/PX4-Autopilot`
  outside this repository.
- PX4 commit: `a2be9197b57b2d065e4c47ca7fc5b7565ee07282`.
- PX4 setup command: `bash Tools/setup/ubuntu.sh --no-nuttx`.
- Setup note: PX4's Ubuntu setup script reported that Gazebo binaries are not
  available for Ubuntu 25.10, so this run used PX4 SIH instead of Gazebo.
- PX4 backend: `px4_sitl_sih`, `sihsim_quadx`, headless.
- PX4 startup command:

```bash
PX4_SIM_SPEED_FACTOR=1 make px4_sitl_sih sihsim_quadx
```

Observed PX4 startup summary:

```text
PX4_GIT_TAG: v0.0.0
PX4 config: px4_sitl_sih
INFO [init] SIH simulator
INFO [mavlink] mode: Normal, data rate: 4000000 B/s on udp port 18570 remote port 14550
INFO [px4] Startup script returned successfully
INFO [commander] Ready for takeoff!
```

## Dry Run

Command:

```bash
/home/formi/.local/bin/runlim cargo run -p swarm-examples --bin sitl_agent --features mavlink-transport -- \
  --dry-run \
  --scenario scenarios/sitl.px4-golden.json \
  --agent-id agent-0
```

Artifact: `dry-run.txt`.

Result: the scenario resolved to 3 local-simulation waypoints with explicit
altitudes: 5m, 6m, and 5m.

## Live Run

Command:

```bash
/home/formi/.local/bin/runlim cargo run -p swarm-examples --bin sitl_agent --features mavlink-transport -- \
  --connection udpin:0.0.0.0:14550 \
  --scenario scenarios/sitl.px4-golden.json \
  --agent-id agent-0 \
  --allow-hardware-candidate \
  --execute \
  --timeout 5 \
  --telemetry-timeout 10 \
  --no-progress-timeout 60 \
  --run-report results/m48_px4_sitl_2026-05-30/single-agent-report.json \
  --replay-log results/m48_px4_sitl_2026-05-30/single-agent.sitl-log.json
```

`--allow-hardware-candidate` was required because `udpin:0.0.0.0:14550` is a
wildcard listener. The PX4 log reported MAVLink as localhost-only, and this was
still a local SITL run, not a hardware experiment.

Artifact: `live-run.txt`.

Result:

```text
Real MAVLink mode: mission started; uploaded_count=3 armed=true took_off=true started=true post_start_heartbeat=true abort_result=None
Real MAVLink mode: mission complete; uploaded_count=3 completed=3 failed=0 total=3
```

## Run Report

Artifact: `single-agent-report.json`.

```json
{
  "schema_version": "sitl_run_report.v1",
  "scenario_path": "scenarios/sitl.px4-golden.json",
  "scenario_name": "sitl_px4_golden_0",
  "mission": "sitl",
  "profile": "px4-golden",
  "agent_id": "agent-0",
  "connection_string": "udpin:0.0.0.0:14550",
  "mode": "connection_execute",
  "mission_item_count": 3,
  "completed_count": 3,
  "failed_count": 0,
  "final_status": "completed",
  "error": null,
  "abort_result": null
}
```

## Replay Summary

Command:

```bash
cargo run -p swarm-examples --bin replay -- \
  --sitl-summary results/m48_px4_sitl_2026-05-30/single-agent.sitl-log.json
```

Artifact: `replay-summary.txt`.

```text
SITL run: sitl_px4_golden_0:agent-0:connection_execute
Scenario: sitl_px4_golden_0 | Agent: agent-0 | Mode: connection_execute
Events: 75
Upload: clear=1 count=1 requested=3 sent=3 ack_accepted=1 ack_rejected=0
Commands: sent=3 ack_accepted=3 ack_rejected=0
Telemetry: heartbeat=27 current_seq=13 waypoint_reached=15 task_completed=3
Failures: aborts=0 disconnected=0 failures=0 final_status=completed
Reallocation: agent_lost=0 task_released=0 task_reassigned=0 completed=0 tasks_recovered=0 latency_ticks=none
```

## Legacy Connection Attempt

Artifact: `legacy-udp-pre-fix-attempt.txt`.

The initial documented `udp:127.0.0.1:14550` form failed before the live run
because the underlying `mavlink` crate does not accept `udp:` directly:

```text
error: connection failed: mavlink connection error: Protocol unsupported
```

The code now normalizes legacy `udp:` and `tcp:` aliases before opening the
MAVLink transport, while docs use native `udpin:` / `udpout:` / `tcpout:` forms.

## Scope

This verifies the single-agent PX4 SIH/SITL golden path only. It does not verify
Gazebo, HIL, real aircraft, multi-agent PX4 orchestration, or production safety.
