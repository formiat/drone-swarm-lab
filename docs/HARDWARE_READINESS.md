# Hardware Readiness Boundary

This project is not hardware-ready. It is a research prototype with portable
simulation checks and experimental PX4 SITL tooling. The current code does not
provide flight certification, a certified safety layer, hardware-specific
failsafe tuning, operator training, or a production flight workflow.

Use `sitl_agent` against real drones only as a deliberately planned hardware
experiment in a controlled environment. The CLI treats remote, TCP, and serial
connections as hardware candidates and requires `--allow-hardware-candidate` so
that this path is not enabled accidentally.

## Verified Scope

| Area | Status | What is verified |
|---|---|---|
| Mock SITL | Portable | Mission waypoints are sent through in-memory mock transport with no PX4, sockets, simulator, or hardware. |
| Dry-run SITL | Portable | Scenario loading, waypoint extraction, coordinate-frame reporting, and upload-plan formatting are deterministic. |
| Portable regression | Portable | `portable_sitl_regression_smoke`, `sitl_docs`, safety validation, mock replay, and multi-agent manifest checks run without external PX4. |
| Single-agent PX4 SITL | Experimental | Feature-gated mission upload, optional arm/takeoff/start, telemetry progress, run report, and replay log plumbing exist for local PX4 SITL. Live simulator verification remains manual/local. |
| Multi-agent SITL foundation | Experimental foundation | `multi_sitl.v1` config, per-agent task subsets, dry-run/mock manifest, MAVLink system/component mapping, and duplicate ownership rejection are covered without real multi-agent PX4 orchestration. |

## Not Verified On Hardware

- Real airframe-specific failsafe behavior.
- Real radio or telemetry link loss behavior.
- Real GNSS, estimator, compass, barometer, or sensor failure behavior.
- Real battery discharge, voltage sag, return margin, or payload impact.
- Real obstacle avoidance, collision avoidance, or detect-and-avoid behavior.
- Pilot handoff, operator workload, crew coordination, or emergency procedure reliability.
- Certified geofence enforcement, flight termination, or remote ID compliance.
- Hardware-in-the-loop CI.
- Multi-agent real PX4 flight orchestration.
- Any production safety guarantee.

## Connection Classes

| Class | Examples | Meaning |
|---|---|---|
| `mock` | `sitl_agent --mock` | Portable in-memory transport. No hardware path. |
| `dry-run` | `sitl_agent --dry-run` | Prints the mission upload plan. No hardware path. |
| `local_px4_sitl_udp` | `udp:127.0.0.1:14550`, `udp:localhost:14550`, `udp:[::1]:14550` | Local PX4 SITL candidate. Still experimental, but not treated as hardware by the CLI guard. |
| `hardware_candidate` | `serial:/dev/ttyUSB0:57600`, `tcp:*`, `udp:*` with non-loopback host | May target real hardware or a remote endpoint. Requires `--allow-hardware-candidate`. |

The classifier is a guardrail, not a safety guarantee. A loopback endpoint can
still be forwarded to hardware by external tools, and a non-loopback endpoint
can be a lab SITL VM. The operator is responsible for knowing what is connected.

## Safety Assumptions

- The scenario coordinate frame and altitude contract have been reviewed before upload.
- Pre-upload safety validation is enabled and configured for the intended local test volume.
- PX4 parameters, arming checks, failsafe settings, RTL behavior, and mode transitions are reviewed outside this repository.
- A human operator can take control immediately.
- The environment is controlled, low-risk, and legally appropriate.
- No autonomous flight is performed outside a controlled test.

## Operator Checklist

The operator checklist below is the minimum boundary before any hardware
experiment. All items below must be true:

- Physical kill switch or flight termination path is available and tested.
- Manual pilot override is available and rehearsed.
- Geofence or equivalent containment is configured outside this repository.
- Test area is low-risk, controlled, legally allowed, and clear of people, animals, traffic, and fragile property.
- Propeller/bench safety is handled before any powered test.
- PX4 parameters, arming checks, failsafe actions, RTL altitude, and battery failsafes are reviewed.
- Mission waypoints, local coordinate conversion, altitude, and expected path are reviewed in `--dry-run`.
- Safety config geofence, mission radius, waypoint jump, and altitude limits are reviewed.
- Logs are enabled and storage is available.
- Emergency RTL, hold, manual mode, and disarm procedure are rehearsed.
- A second observer is present for flight tests when required by local rules.

This checklist is not flight certification. Passing it does not make the project
production-ready or safe for real-world autonomous drone operations.
