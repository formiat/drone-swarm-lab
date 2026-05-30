# M54 Multi-Agent Supervisor Reallocation Result - 2026-05-30

This directory captures the mock/fake multi-agent supervisor run that closes the
runtime supervisor gap after M51/M52.

## Command

```bash
cargo run -p swarm-examples --bin sitl_supervisor -- \
  --mock \
  --scenario scenarios/sitl.multi-agent.json \
  --config scenarios/sitl.multi-agent.config.json \
  --fail-agent agent-0 \
  --fail-after-ticks 1 \
  --heartbeat-timeout-ticks 3 \
  --max-ticks 12 \
  --replay-log results/m54_multi_agent_supervisor_2026-05-30/multi-supervisor.sitl-log.json \
  --manifest results/m54_multi_agent_supervisor_2026-05-30/manifest.json
```

## Result

Artifact: `mock-run.txt`.

```text
SUPERVISOR_METRICS agents=2 heartbeats=6 completed_tasks=4 lost_agents=1 reassignment_count=1 tasks_recovered=wp-1 reallocation_latency_ticks=0 final_status=completed
```

Replay summary:

```text
SITL run: sitl-supervisor-sitl_multi_agent_0
Scenario: sitl_multi_agent_0 | Agent: supervisor | Mode: mock
Events: 26
Upload: clear=0 count=2 requested=0 sent=4 ack_accepted=0 ack_rejected=0
Commands: sent=0 ack_accepted=0 ack_rejected=0
Telemetry: heartbeat=6 current_seq=0 waypoint_reached=4 task_completed=4
Failures: aborts=0 disconnected=0 failures=0 final_status=completed
Reallocation: agent_lost=1 task_released=1 task_reassigned=1 completed=1 tasks_recovered=1 latency_ticks=0
```

## Scope

This verifies one-process mock/fake supervisor orchestration:

- two agents with explicit task ownership;
- deterministic heartbeat loss for `agent-0`;
- runtime timeout detection;
- release of unfinished failed-agent tasks;
- reallocation to the surviving agent;
- common SITL event log and replay summary.

It does not execute a live multi-agent PX4 failure/reallocation flow.
