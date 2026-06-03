# Hardware Readiness Boundary

This project is not hardware-ready. It is a research prototype with portable
simulation checks and experimental PX4 SITL tooling. The current code does not
provide flight certification, a certified safety layer, hardware-specific
failsafe tuning, operator training, or a production flight workflow.
Required M79 boundary phrases:

- first hardware experiment is still not product readiness;
- multi-agent hardware requires separate safety review;
- no regulatory or certified safety claim.

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
| Artifact validation | Portable evidence gate | M72 adds `artifact_validator`, `artifact_validation_report.v1`, scenario/config/command snapshots, manifest metadata, and stable artifact rule ids for local SITL packs. It improves evidence discipline before future hardware-candidate work, but it is not automated PX4 CI, Gazebo/HIL validation, hardware readiness, or flight certification. |
| Degraded supervisor | Portable fake-tested boundary | M73 adds structured failure modes, supervisor decisions, degraded report fields, replay events, and artifact-validator checks for failed/reallocated supervisor packs. This is pre-hardware evidence discipline, not hardware failsafe validation, RF modeling, Gazebo/HIL coverage, or production failover. |
| Urban blocked-route decision | Portable simulation layer | M74 adds `UrbanTemporaryObstacle`, `UrbanBlockedPolicy` (Wait/Replan/Abort), effective blocked-set computation, 8 replay events, and 4 metrics (wait time, replan count, replan success rate, unresolved blockages). Three scenario profiles exercise each policy path. This is deterministic mission-level reactivity only, not real sensors, physics, certified obstacle avoidance, or hardware validation. |
| Urban mission realism | Portable simulation layer | M75 adds `UrbanBusRoute`/`UrbanBusStop` for scheduled moving mocked bus targets, `pose_at_tick` interpolation, backward-compatible static bus behavior, `perimeter_waypoints` for closed perimeter patrol, and perimeter metrics. This is deterministic simulation semantics only, not real lidar/raycast, physics, dynamic obstacle avoidance, PX4/SITL execution evidence, or hardware validation. |
| Synthetic scenario testbed | Portable generator infrastructure | M76 adds `SyntheticUrbanGenerator`, typed generator configs, library presets, `generator_manifest` in scenario DSL, and deterministic generated scenario suites reproducible from seed and parameters. This is regression/testbed infrastructure only, not benchmark evidence, PX4/SITL validation, hardware validation, real perception, or physics simulation. |
| Algorithm differentiation | Portable simulation layer | M77 adds opt-in `comms_penalty_weight`, `wildfire_priority_realloc_threshold`, `dynamic_belief_updates`, and CBBA `conflict_count` in replay events. Defaults preserve previous behavior. A committed 1-seed smoke artifact shows CBBA failure under heavy communication loss. This is targeted algorithm diagnostics only, not a publication-grade benchmark, PX4/SITL evidence, hardware evidence, or a CBBA gossip-burst fix. |
| Benchmark evidence layer | Portable reporting/evidence layer | M78 adds stddev/stderr/95% CI/min/max/failure-rate fields to `AggregateMetrics` reports, machine-readable `support_status`/`support_reason`, `BenchmarkManifest.artifact_kind`, `sar_success_threshold`, `--mission urban` entrypoint, and a bounded `coverage-packet-loss` degradation sweep artifact. This is simulation reporting only, not a new 1000-seed run, PX4/SITL evidence, hardware evidence, or a publication-grade statistical study. |
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
- Real hardware degraded-supervisor fault injection.
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
- M72 artifact validation passes for any local supervisor output pack that will be cited as evidence; review `docs/ARTIFACT_VALIDATION.md` and keep historical artifacts clearly marked when they lack M72 metadata.
- For Urban missions, the M70 Urban Route Export artifact is reviewed before any optional manual upload; it does not prove perception, obstacle avoidance, dynamic traffic handling, or hardware safety.
- Safety config geofence, mission radius, waypoint jump, and altitude limits are reviewed.
- Logs are enabled and storage is available.
- Emergency RTL, hold, manual mode, and disarm procedure are rehearsed.
- A second observer is present for flight tests when required by local rules.

This checklist is not flight certification. Passing it does not make the project
production-ready or safe for real-world autonomous drone operations.

## Operational Runbooks

M79 adds the canonical operational runbook layer in
[`docs/OPERATIONAL_RUNBOOKS.md`](OPERATIONAL_RUNBOOKS.md). Use it before any
hardware-candidate experiment. It defines simulation, Urban, SITL dry-run,
artifact validation, local PX4/SIH, and future hardware-candidate procedures,
plus explicit go/no-go gates:

- no hardware if simulation fails;
- no hardware if SITL dry-run/export fails;
- no hardware if preflight safety fails;
- no hardware if artifact validator fails;
- no hardware without external safety process;
- no multi-drone hardware before separate single-drone review.
