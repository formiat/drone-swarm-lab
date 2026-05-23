# SITL Setup Guide

## Overview

This document describes how to set up and run PX4 Software-In-The-Loop (SITL) with the swarm coordination runtime.

## Prerequisites

- Linux (Ubuntu 22.04 recommended)
- ~10 GB free disk space
- Rust toolchain (1.94+)
- PX4-Autopilot
- Gazebo (comes with PX4 SITL make target)

## Installing PX4 and Gazebo

```bash
# Clone PX4
git clone https://github.com/PX4/PX4-Autopilot.git --recursive
cd PX4-Autopilot

# Install dependencies (Ubuntu)
bash ./Tools/setup/ubuntu.sh

# Build SITL with Gazebo
make px4_sitl gazebo_plane
```

> **Note:** The first build downloads and compiles Gazebo. This takes 20-60 minutes depending on your machine.

## Running SITL

### Terminal 1: PX4 SITL + Gazebo

```bash
cd PX4-Autopilot
make px4_sitl gazebo_plane
```

PX4 will start and wait for MAVLink connections on UDP port 14550 (default).

To set a custom home position:
```bash
PX4_HOME_LAT=47.397742 PX4_HOME_LON=8.545594 make px4_sitl gazebo_plane
```

### Terminal 2: MAVProxy (optional, for monitoring)

```bash
mavproxy.py --master udp:127.0.0.1:14550
```

### Terminal 3: sitl_agent

```bash
cd /path/to/swarm-coordination-runtime

# Mock mode (no PX4 needed, tests waypoint conversion)
cargo run --bin sitl_agent -- \
  --mock \
  --scenario scenarios/coverage.ideal.json \
  --agent-id agent-0

# Real mode (requires PX4 SITL running)
cargo run --bin sitl_agent -- \
  --connection udpout:127.0.0.1:14550 \
  --scenario scenarios/coverage.ideal.json \
  --agent-id agent-0
```

## Expected Output

### Mock mode

```
SITL Agent: agent-0 | 3 tasks with pose | mock=true
WAYPOINT seq=0 x=10.0 y=20.0 z=0.0
WAYPOINT seq=1 x=30.0 y=40.0 z=0.0
All waypoints sent. Completed.
```

### Real mode (with PX4 SITL)

```
SITL Agent: agent-0 | 3 tasks with pose | mock=false
WAYPOINT seq=0 x=10.0 y=20.0 z=0.0
WAYPOINT seq=1 x=30.0 y=40.0 z=0.0
All waypoints sent. Completed.
```

In MAVProxy you should see the mission items being received by the autopilot.

## Coordinate System

The swarm simulation uses 2D Cartesian coordinates (x, y). PX4 SITL uses WGS84 (lat, lon).

Current conversion: `lat = home_lat + x * 1e-7`, `lon = home_lon + y * 1e-7`.

1 unit in simulation ≈ 0.1 meters at the equator.

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `mavlink::connect` returns connection error | Check PX4 is running and the connection string is correct (`udpout:127.0.0.1:14550` for client mode) |
| Gazebo not found | Run `make px4_sitl gazebo_plane` from PX4-Autopilot directory |
| Port 14550 in use | Kill existing PX4 process: `killall px4` |
| `sitl_agent` can't find scenario | Use absolute path or run from workspace root |
