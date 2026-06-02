# Hardware Readiness Boundary

This project is not hardware-ready. It is a research prototype with portable
simulation checks and experimental PX4 SITL tooling. The current code does not
provide flight certification, a certified safety layer, hardware-specific
failsafe tuning, operator training, or a production flight workflow.

Use `sitl_agent` against real drones only as a deliberately planned hardware
experiment in a controlled environment. The CLI treats remote, wildcard, TCP,
and serial connections as hardware candidates and requires
`--allow-hardware-candidate` so that this path is not enabled accidentally.

## Verified Scope

| Area | Status | What is verified |
|---|---|---|
| Mock SITL | Portable | Mission waypoints are sent through in-memory mock transport with no PX4, sockets, simulator, or hardware. |
| Dry-run SITL | Portable | Scenario loading, waypoint extraction, coordinate-frame reporting, and upload-plan formatting are deterministic. |
| Urban route export dry-run | Portable | M70 converts `urban-patrol` planned road-graph routes into ordered SITL-compatible waypoint plans with explicit altitude, `geo_origin`, route stats, and `sitl_dry_run_artifact.v1` JSON evidence. This is local export evidence only, not PX4 execution or hardware readiness. |
| Preflight safety contract | Portable static gate | M71 requires mission inputs to pass `SafetyValidationReport` checks before dry-run, SITL upload, or hardware-candidate experiments. It catches geofence, no-fly, altitude, route length, ownership, Urban, and known semantic issues. This is not certified flight safety. |
| Portable regression | Portable | `portable_sitl_regression_smoke`, `sitl_docs`, safety validation, mock replay, and multi-agent manifest checks run without external PX4. |
| Single-agent PX4 SITL | Experimental | Feature-gated mission upload, optional arm/takeoff/start, telemetry progress, run report, replay log plumbing, and public `scenarios/sitl.px4-golden.json` exist for local PX4 SITL. Live simulator verification remains manual/local. |
| Multi-agent SITL foundation | Experimental foundation | `multi_sitl.v1` config, public `scenarios/sitl.multi-agent.json` / `scenarios/sitl.multi-agent.config.json`, per-agent task subsets, dry-run/mock manifest, mock supervisor reallocation, MAVLink system/component mapping, duplicate ownership rejection, and local two-instance PX4 SIH upload-only mission acceptance are covered. |
| Live multi-agent PX4/SIH execute/reallocation | Experimental local SITL workflow | M58 adds `sitl_supervisor --connection --execute`, `scenarios/sitl.multi-agent.execute.config.json`, per-agent safety/hardware gates, local endpoint execution, common event log, and structured multi-agent report. M59 adds explicit `--reupload-on-failure` active-survivor mission replacement after failed-agent runtime reallocation. Captured local PX4/SIH artifacts exist for two-agent execute and one controlled failure/reallocation path. This is not hardware readiness, automated PX4 CI, Gazebo/HIL validation, or production failover. |
| PX4/SIH supervisor hardening | Local research workflow hardening | M60 adds `sitl_supervisor --output-dir`, `--run-id`, `--force`, checked overwrite policy, stable exit codes, replay summaries, and report summary fields for repeatable local runs. This improves artifact discipline and diagnosis only; it is not hardware-ready and does not change the operator checklist. |
| Supervisor Controller Boundary | Portable internal boundary | M57 extracts mock `sitl_supervisor` orchestration behind an internal `AgentController` / `MockAgentController` boundary with a shared loop, fake-controller tests, and assertable `SupervisorMetrics`. This is a code-structure and testability milestone, not hardware readiness. |

## Not Verified On Hardware

- Real airframe-specific failsafe behavior.
- Real radio or telemetry link loss behavior.
- Real GNSS, estimator, compass, barometer, or sensor failure behavior.
- Real battery discharge, voltage sag, return margin, or payload impact.
- Real obstacle avoidance, collision avoidance, or detect-and-avoid behavior.
- Pilot handoff, operator workload, crew coordination, or emergency procedure reliability.
- Certified geofence enforcement, flight termination, or remote ID compliance.
- Hardware-in-the-loop CI.
- Multi-agent real PX4 flight orchestration on hardware.
- Any production safety guarantee.

## Connection Classes

| Class | Examples | Meaning |
|---|---|---|
| `mock` | `sitl_agent --mock` | Portable in-memory transport. No hardware path. |
| `dry-run` | `sitl_agent --dry-run` | Prints the mission upload plan. No hardware path. |
| `local_px4_sitl_udp` | `udpin:127.0.0.1:14550`, `udpin:localhost:14550`, `udpout:127.0.0.1:14550`; legacy `udp:*` loopback aliases are accepted | Local PX4 SITL candidate. Still experimental, but not treated as hardware by the CLI guard. |
| `hardware_candidate` | `serial:/dev/ttyUSB0:57600`, `tcpout:*`, `tcpin:*`, `udpout:*`, `udpin:0.0.0.0:*`, `udpin:*` with non-loopback host | May target real hardware, a wildcard listener, or a remote endpoint. Requires `--allow-hardware-candidate`. |

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
- M71 preflight passes with no error-severity rule ids; review `docs/PREFLIGHT_SAFETY.md` and any `safety_validation_report.v1.json` artifact before proceeding.
- For Urban missions, the M70 Urban Route Export artifact is reviewed before any optional manual upload; it does not prove perception, obstacle avoidance, dynamic traffic handling, or hardware safety.
- Safety config geofence, mission radius, waypoint jump, and altitude limits are reviewed.
- Logs are enabled and storage is available.
- Emergency RTL, hold, manual mode, and disarm procedure are rehearsed.
- A second observer is present for flight tests when required by local rules.

This checklist is not flight certification. Passing it does not make the project
production-ready or safe for real-world autonomous drone operations.
