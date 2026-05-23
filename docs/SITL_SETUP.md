# SITL Setup Guide

This guide covers both the mock SITL path (no external dependencies) and the real PX4 SITL path (experimental).

## Quick Start (Mock Mode)

The mock mode sends waypoints to an in-memory `MockMavlinkTransport` and prints them to stderr. It requires no external dependencies and works out of the box.

```bash
cargo run --bin sitl_agent -- \
  --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0
```

Expected output:
```
SITL Agent: agent-0 | 3 tasks with pose | mock=true
WAYPOINT seq=0 x=10.0 y=20.0 z=0.0
WAYPOINT seq=1 x=30.0 y=40.0 z=0.0
...
Mock mode: 3 waypoints sent.
```

This is the recommended path for testing and CI.

## Real PX4 Mode (Experimental)

Prerequisites:
1. PX4 SITL running: `make px4_sitl gazebo_iris`
2. MAVLink connection on UDP: `udp:127.0.0.1:14550`

Build with feature:
```bash
cargo build --bin sitl_agent --features mavlink-transport
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0
```

## Known Limitations

- Real PX4 path is experimental and requires manual SITL setup.
- Only waypoint tasks with `pose` are converted to MAVLink commands.
- Multi-agent SITL not yet supported (single agent only).
- `--connection` without `mavlink-transport` feature produces a build instruction error.

## Troubleshooting

| Problem | Cause | Fix |
|---|---|---|
| `0 pose-tasks` warning | Scenario has no tasks with `pose` | Use `scenarios/sitl.waypoints.json` |
| `mavlink-transport feature required` | Built without feature | `cargo build --features mavlink-transport` |
| No PX4 connection | PX4 SITL not running | Start PX4 SITL first |
