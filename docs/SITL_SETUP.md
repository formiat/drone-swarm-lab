# SITL Setup Guide

## Mock Mode (no PX4 required)

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

## Real PX4 Mode (experimental)

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
