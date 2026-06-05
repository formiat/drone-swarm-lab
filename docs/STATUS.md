# Project Status

**Date:** 2026-06-05
**HEAD commit:** see `git rev-parse HEAD`
**Last audit:** M88 Logical Swarm Topologies

This document is the current status summary for the repository. It supersedes
the older M39b-only audit and should be read together with the README current
status table.

## Milestone Status

| Milestone | Status | Notes |
|---|---|---|
| M32 Reporting & Metrics Hardening | Complete | Mixed-mission report identity, JSON/CSV/Markdown exports, and merged `all` benchmark id are implemented. |
| M33 Mission Semantics Integration | Complete | `TaskKind`, concrete adapters, `AdapterRegistry`, adapter-driven completion/scoring in runner and allocator. |
| M34 Planner Correctness v2 | Complete | `RoutePlanner`, 2-opt, battery-aware ordered-subset feasibility, and planner metrics are present. |
| M35 Dynamic Mission Correctness | Partial | Mission-specific success semantics are implemented; SAR CBBA and SAR centralized remain explicitly unsupported. |
| M36 Regression Harness v2 | Complete | Regression infrastructure exists; repeated release sweeps passed for `regression_runner` and `strategy_comparison --regression` at `jobs=1/4/14`. |
| M37 Realism Scenario Pack | Complete | Light/medium/heavy profiles and scenario metadata exist; this is not a calibrated research study. |
| M38 Wildfire v2 | Partial | Wildfire dynamic threat work exists; flood remains future work and is not a separate mission. |
| M39a Regression Repair | Complete | Runtime ordering and SAR scan-completion fixes removed the reproduced default regression flake; sweep artifacts are in `results/m56_regression_determinism_2026-05-30/`. |
| M39b Decision / Audit Report | Complete | Historical audit and README honesty pass. |
| M43 SITL Contract & Dry-Run Foundation | Complete | `--mock`, `--dry-run`, `--connection`, typed errors, connection classes, and waypoint extraction are implemented. |
| M44 MAVLink Mission Upload Protocol | Complete with debt removed | Mission upload handshake is implemented; generic `MavlinkTransport::send()` is explicitly unsupported instead of sending a fake `RAW_RPM`. |
| M45 Pre-upload Safety Validation | Complete | Safety config, default rules, subset validation, and actionable violations are implemented. |
| M46 Flight Sequence | Complete | Upload-only/execute lifecycle, arm/takeoff/start/abort command handling, and bounded failures are implemented. |
| M47 Telemetry Loop & TaskStatus Mapping | Complete | `MISSION_CURRENT`, `MISSION_ITEM_REACHED`, completion/rejection/disconnect/no-progress mapping are implemented. |
| M48 Single-Agent PX4 SITL Golden Path | Complete for local PX4 SIH | Live single-agent PX4 SIH run completed on 2026-05-30 with `scenarios/sitl.px4-golden.json`; report/replay artifacts are in `results/m48_px4_sitl_2026-05-30/`. |
| M49 SITL Observability & Replay | Complete | SITL event log, replay summary, task id mapping, failure events, and reallocation schema events are implemented. |
| M50 Mock Regression & Docs Hardening | Complete | Portable dry-run/mock/docs checks exist and require no PX4. |
| M51 Dynamic Reallocation for Failed Agent | Supervisor mock complete | Runtime reallocation, metrics, SITL event schema, and `sitl_supervisor --mock` heartbeat-timeout/reallocation flow are implemented. M59 reuses this runtime path for controlled live-supervisor active-survivor mission replacement. |
| M52 Multi-Agent SITL Foundation | Foundation plus upload-only check complete | `multi_sitl.v1`, public `scenarios/sitl.multi-agent.json` / `scenarios/sitl.multi-agent.config.json`, dry-run/mock manifest, task subsets, duplicate ownership checks, mock supervisor orchestration, and a two-instance PX4 SIH upload-only check exist. |
| M53 Hardware Readiness Boundary | Complete | `docs/HARDWARE_READINESS.md`, connection classes, and `--allow-hardware-candidate` guard hardware-candidate endpoints. |
| M57 Supervisor Controller Boundary | Complete | `sitl_supervisor` mock orchestration is extracted into a testable internal supervisor module with `AgentController`, `MockAgentController`, fake-controller tests over the shared supervisor loop, returned `SupervisorMetrics`, and expanded CLI negative tests. M58 builds the live PX4/SIH path beside this boundary. |
| M58 Live Multi-Agent PX4/SIH Execute Orchestration | Complete as experimental local SITL workflow | `sitl_supervisor --connection --execute` with `scenarios/sitl.multi-agent.execute.config.json` validates all live agents, rejects non-execute lifecycles, applies per-agent safety and hardware-candidate gates before upload, runs local PX4/SIH controllers under one supervisor, and writes a common SITL event log plus `sitl_multi_agent_run_report.v1`. A two-agent execute artifact is captured in `results/m58_multi_agent_px4_sih_execute_2026-05-31/`. |
| M59 Live PX4/SIH Failure & Reallocation | Complete as controlled local SITL workflow | `--reupload-on-failure` detects a failed active live agent, emits runtime release/reassignment events, aborts/replaces an active survivor mission, writes report `reallocation` metrics and replay summary counters, and has fake-controller coverage. A controlled PX4/SIH failure artifact is captured in `results/m59_px4_sih_failure_reallocation_2026-05-31/`. |
| M60 PX4/SIH Supervisor Hardening | Complete for local workflow hardening | `sitl_supervisor` now supports `--output-dir`, `--run-id`, and `--force`, refuses artifact overwrites by default, returns stable exit codes, writes replay summaries for output-dir runs, and extends `sitl_multi_agent_run_report.v1` with `task_ownership`, `events_summary`, `final_status`, and `limitations`. This hardens repeatable local PX4/SIH research runs, not hardware readiness. |
| M61 Platform / API Stabilization | Complete as in-repository extension guidance | `docs/EXTENSION_GUIDE.md` documents mission, strategy, metrics, crate boundaries, schema-version policy, and test-only extension fixtures. It is a stable-ish in-repository guide, not a semver-stable published API or hardware-readiness claim. |
| M62 Benchmark / Baseline Refresh | Complete as historical 500-seed validation baseline | Release `strategy_comparison --seeds 500 --mission all --jobs 14` completed for commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd`. Artifacts are in `results/all_500_jobs14_m62_release/`; after M63 this is historical validation evidence unless rerun on current HEAD, not a publication-grade 1000-seed statistical run. |
| M63 Evidence Cleanup / Status Honesty | Complete without benchmark rerun | README/status/benchmark docs mark the M62 pack as historical evidence for `81260ca7afa114a5d9add7b832f6c5d7875b88cd`, flood is future work, wildfire success semantics are documented/tested, and committed M58/M59 SITL artifacts have targeted replay/event sanity tests. |
| M64 Urban Foundations | Complete as stable substrate | `UrbanMap`, directed road graph nodes/edges, deterministic Dijkstra route-loop planning, AABB static obstacle judge, `urban-patrol` DSL validation, `scenarios/urban.patrol.json`, and Urban metrics skeleton are implemented. |
| M65 Urban Patrol v0 | Complete as simulation-only mission | One scout follows the ordered `urban-patrol` road-graph loop and succeeds only after traversing every planned segment before timeout with zero Urban judge violations. The runner emits Urban replay events and reports patrol completion/time/distance/efficiency metrics. M65 itself has no bus detection; M66 adds mocked bus search separately. Lidar/raycast, dynamic obstacles, multi-agent route deconfliction, PX4/SITL export, hardware claims, visual UI, and publication benchmark evidence remain future work. |
| M66 Urban Search v1 | Complete as simulation-only mission | One scout follows the Urban road graph and evaluates a deterministic mocked bus detector. `urban-search` DSL validation, `scenarios/urban.search.json`, bus observation/detection/false-positive/search-completion replay events, bus detection/time/false-positive/distance metrics, focused reports, and a smoke regression gate are implemented. Lidar/raycast, dynamic obstacles, real perception, multi-agent deconfliction, PX4/SITL export, hardware claims, visual UI, and publication benchmark evidence remain future work. |
| M67 Urban Replay / Analysis | Complete as diagnostic tooling | Simulation replay now supports deterministic timeline output with `--agent` / `--category urban` filters, additive `UrbanViolation.obstacle_id`, route-trace and judge-report JSON/CSV artifacts for Urban benchmark packs, a two-agent analysis fixture in `scenarios/urban.multi-agent.json`, suite-mode replay artifact generation, and diagnostic Urban separation/conflict aggregate metrics populated from replay-enabled Urban traces. This adds observability only; it does not add avoidance, multi-agent Urban control, real perception, lidar/raycast, PX4/SITL export, hardware claims, or a benchmark rerun. |
| M68 Algorithm Depth On Urban + Existing Missions | Complete as small Urban planner delta | `planner: "corridor-aware"` is implemented for Urban route loops, `urban_route_risk_score` is exported, `scenarios/urban.corridor-delta.json` compares Dijkstra against the corridor-aware planner, and docs/support matrix explain the lower-risk/longer-route tradeoff. This is not a full benchmark refresh and does not change CBBA/SAR unsupported status. |
| M69 Benchmark Refresh / Research Evidence | Complete for built-in simulation suite | Release `strategy_comparison --seeds 1000 --mission all --jobs 14` completed for code commit `5d1d3cd17cacba7482c1d9b93eb5acc107af8f71`. Artifacts are in `results/all_1000_jobs14_m69_release/`; runtime was 28:55.25 with peak RSS 207684 KB. `regression_runner --jobs 14` passed. The current `--mission all` suite covers coverage, emergency-mesh, SAR, inspection, and wildfire; Urban scenario-suite evidence remains separate in `results/m68_urban_corridor_delta/`. |
| M70 Urban Route Export + Geo Origin | Complete as portable dry-run/SITL waypoint export boundary | `urban-patrol` routes can be exported through `sitl_agent --dry-run` into ordered waypoint missions with route length, segment count, waypoint count, stable route identity fields, explicit altitude, scenario `geo_origin`, effective default origin, and `sitl_dry_run_artifact.v1` JSON artifacts. This is local waypoint export only; it is not hardware readiness, real perception, lidar/raycast, obstacle avoidance, Gazebo/HIL, or PX4 execution evidence. |
| M71 Preflight Safety And Invariant Contract | Complete as static input gate | `SafetyValidationReport` preflight checks now emit rule-id based violations with severity, affected id, and reason. Dry-run and supervisor inputs pass a preflight gate before execution, dry-run artifacts include the safety report, `sitl_supervisor --output-dir` writes `safety_validation_report.v1.json`, and stable exit codes use 2/3/4/5 for validation/runtime/artifact/environment categories. This is not certified flight safety or hardware validation. |
| M72 Artifact Validator + SITL Harness | Complete as local artifact discipline | `artifact_validator` validates supervisor output packs with stable rule ids such as `artifact.final_status_mismatch`, `artifact.replacement_seq_mismatch`, and `artifact.safety_report_missing`; it checks manifest metadata, run id/final status/event summary/replay summary/task completion/replacement seq/safety/limitations consistency, and supports historical mode for old M58/M59 packs. `sitl_supervisor --output-dir` now captures `scenario.snapshot.json`, `config.snapshot.json`, and `command.txt`. `scripts/run_m58_local.sh` and `scripts/run_m59_local.sh` are manual-only PX4/SIH harness helpers with `DRY_RUN=1`; this is not automated PX4 CI or hardware readiness. |
| M73 Fault Injection And Degraded Supervisor | Complete as fake-tested pre-hardware boundary | The live supervisor now emits additive `degraded` report records, failure-mode counts, decision counts, abandoned-task/recovery-failure metrics, and degraded replay events. `artifact_validator` checks degraded report/event consistency for new packs while preserving historical mode for old M58/M59 evidence. See `docs/DEGRADED_SUPERVISOR.md`. This is not hardware failure validation, RF modeling, Gazebo/HIL coverage, or production failover. |
| M74 Urban Blocked-Route Decision Logic | Complete as deterministic simulation-only decision layer | `UrbanTemporaryObstacle` and `UrbanBlockedPolicy` (Wait/Replan/Abort) are implemented. The patrol runner checks an effective blocked-set each tick, runs a graph lookahead detector, and applies the selected policy. Replan finds an alternate route via `plan_route_excluding` and validates it through the M71 judge gate. Replay emits 8 new events (`UrbanEdgeBlocked`, `UrbanEdgeUnblocked`, `UrbanObstacleDetected`, `UrbanPolicyDecision`, `UrbanRouteReplanned`, `UrbanWaitStarted`, `UrbanWaitCompleted`, `UrbanNoRouteAvailable`). Four new metrics track wait time, blocked-edge count, replan success rate, and unresolved blockages. Preflight validates temporary obstacle declarations. Three new scenario profiles exercise Wait, Replan, and No-Alternative paths. This is mission-level reactivity only; no real sensors, no physics, no certified obstacle avoidance. |
| M75 Urban Mission Realism Follow-up | Complete as deterministic simulation-only mission semantics | `UrbanBusRoute` and `UrbanBusStop` add scheduled moving mocked bus targets over Urban map nodes. The detector samples `pose_at_tick` from the current map/tick and keeps static-bus behavior backward-compatible. `UrbanPerimeterPatrol` and `perimeter_waypoints` add deterministic closed perimeter waypoint generation, a `perimeter-square` profile, and perimeter metrics/export fields (`perimeter_completion_rate`, `perimeter_length_m`, `time_to_complete_perimeter`, `perimeter_violations`). This is mission-level simulation realism only; no lidar/raycast, physics engine, real perception, PX4/SITL execution evidence, hardware validation, or certified obstacle avoidance. |
| M76 Synthetic Scenario Testbed | Complete as deterministic generator infrastructure | Scenario DSL now has optional `generator_manifest` metadata with manifest-schema validation and legacy compatibility. `swarm-scenarios` exposes `SyntheticUrbanGenerator`, typed Urban generator configs, library presets, and deterministic generated suites with static obstacles, temporary blocked edges, optional mocked bus/failure/comms overlays. `generate_scenario_suite` can regenerate `scenarios/urban.generated.tiny.json`. This is testbed/regression infrastructure only; no benchmark refresh, PX4/SITL evidence, hardware validation, physics engine, real perception, or certified obstacle avoidance. |
| M77 Algorithm Differentiation | Complete as targeted algorithm/diagnostic implementation | `RunConfig` now has neutral-default `comms_penalty_weight`, `wildfire_priority_realloc_threshold`, and `dynamic_belief_updates`. Greedy/auction/connectivity-aware scoring can use communication range when opted in; wildfire dynamic priority threshold crossings emit a dedicated replay event and release tasks for deterministic reassignment; SAR can re-rank unfinished scan tasks by entropy; CBBA bundle replay events include `conflict_count`. `strategy_comparison --profiles ...` supports targeted deltas, and `results/m77_algorithm_delta/coverage/` contains a 1-seed release smoke artifact. This is not a 1000-seed publication benchmark, hardware/PX4 evidence, or a CBBA gossip-burst fix. |
| M78 Benchmark Evidence Layer | Complete as simulation reporting/evidence layer | `AggregateMetrics` and JSON/CSV/Markdown reports expose stderr/stddev/95% CI/min/max/failure-rate fields for key binary metrics. Report rows include machine-readable `support_status` / `support_reason`, `BenchmarkManifest` records `artifact_kind`, SAR can opt into `run_config.sar_success_threshold`, `--mission urban` is explicit while legacy `--mission all` remains M69-comparable, and `--degradation coverage-packet-loss` writes a bounded simulation degradation artifact in `results/m78_degradation_coverage_packet_loss_2026-06-03/`. This is not a new 1000-seed run, PX4/SITL evidence, hardware evidence, or publication-grade statistical study. |
| M79 Operational Runbooks And Hardware Entry Gate | Complete as documentation/tested operational boundary | `docs/OPERATIONAL_RUNBOOKS.md` defines simulation, Urban, SITL dry-run/export, artifact validation, local PX4/SIH, and future hardware-candidate runbooks with preflight checklist, go/no-go gates, post-run inspection, command examples, and conservative hardware boundary language. This is not a hardware run, not regulatory certification, not a semver promise, and not product readiness. |
| M80 Mission Command IR | Complete as hardware-agnostic IR foundation | New crate `swarm-mission-ir` defines 13 command primitives (`arm`, `disarm`, `takeoff`, `hold`, `land`, `return_to_launch`, `go_to`, `follow_route`, `loiter_time`, `orbit`, `pause`, `resume`, `abort`) with explicit semantics (coordinate frame, altitude reference, timeout policy, terminal state, completion tolerance) and typed validation via `MissionIrError`. `swarm-sim` exposes `urban_route_to_follow_route` to convert Urban planned routes to IR without MAVLink fields. `SitlDryRunArtifact` now includes an optional `command_ir_summary`. `docs/MISSION_COMMAND_IR.md` explains the IR boundary. No MAVLink serialisation, PX4/ArduPilot-specific behavior, or hardware execution. |
| M81 MAVLink Common Compiler | Complete as transport-free compiler artifact | `swarm-comms` now exposes `compile_mavlink_common_plan` and `MavlinkCommonPlan` (`mavlink_common_plan.v1`) for backend-neutral MAVLink Common command intent. The compiler maps supported IR commands to typed Common commands/items such as `MAV_CMD_NAV_TAKEOFF`, `MAV_CMD_NAV_WAYPOINT`, `MAV_CMD_NAV_LOITER_TIME`, `MAV_CMD_NAV_LAND`, and `MAV_CMD_NAV_RETURN_TO_LAUNCH`, separates `command_prelude`, mission upload/start, and `command_postlude` ordering, records structured unsupported features, emits expected ACKs/telemetry milestones, and computes stable SHA-256 `command_ir_hash`. `sitl_agent --dry-run --dry-run-artifact` embeds the plan, and `artifact_validator --mode dry-run` validates it, including unsafe ordering guards. This is no hardware upload, no message serialization; PX4/ArduPilot semantics are not identical. |
| M82 PX4 / ArduPilot Capability Profiles | Complete as conservative dry-run compatibility layer | `swarm-comms` now exposes `MavlinkCapabilityProfileId`, static profiles for `mavlink_common_generic`, `px4`, and `ardupilot`, `MavlinkCompatibilityReport`, per-command compatibility classes, explicit `required_execution_mode`, `required_mode_transitions`, preconditions, `mode_caveats`, and `compatibility_matrix_rows()` for the docs compatibility matrix. `compile_mavlink_common_plan` attaches the report without changing M81 command/item ordering, `sitl_agent --dry-run --dry-run-artifact --mavlink-profile px4|ardupilot` records selected profile compatibility, and `artifact_validator --mode dry-run` checks the compatibility section and hardware-blocking unknown/unsupported classifications. This is not exhaustive autopilot certification, not vendor SDK integration, not ArduPilot SITL evidence, and not hardware upload. See `docs/MAVLINK_CAPABILITY_PROFILES.md`. |
| M83 Primitive Real Mission Pack | Complete as portable command-mission artifact pack | Canonical `takeoff-hold-land`, `orbit`, and `waypoint-square` scenarios compile through `sitl_agent --dry-run --dry-run-artifact` into `MissionCommandPlan` summaries and `MavlinkCommonPlan` artifacts. The artifacts expose command sequence, expected ACKs, telemetry milestones, timeout/abort policy, terminal/tolerance policy, `safety_report.passed=true`, and PX4/ArduPilot compatibility annotations. This is no real flight, no connected vehicle, no PX4/ArduPilot equivalence claim, no Urban behavior, and not certified flight safety or hardware readiness. |
| M84 Urban Geo Route Export + Mission Templates | Complete as portable Urban dry-run/testbed boundary | `UrbanNode` can now carry WGS84 `geo`, Urban map validation rejects mixed geo/local node sets, route export records `coordinate_mode` (`wgs84_node_geo` for all-geo maps) and emits direct WGS84 waypoint metadata while preserving local densified export for local maps, `sitl_agent --dry-run --dry-run-artifact` records Urban mission template / blocked-policy / mocked detector metadata, `urban-search` can use the same route-export path, `artifact_validator --mode dry-run` checks Urban coordinate/perception metadata and WGS84 MAVLink mission item coordinates, and the canonical pack includes `scenarios/urban.geo-block-loop.json`, `scenarios/urban.geo-search-bus.json`, `scenarios/urban.geo-inspection-corridor.json`, and GeoJSON fixture `scenarios/fixtures/urban_small_block.geojson`. This is not real perception, no full OSM parser, not certified collision avoidance, no PX4 execution evidence, no Gazebo/HIL, and no hardware readiness. |
| M85 Urban Multi-Agent Deconfliction | Complete as simulation-only route ownership layer | `urban-patrol` can opt into mission-level segment ownership to avoid assigning the same road segment to multiple agents at the same time. Replay/analysis artifacts expose segment locks, conflicts, and separation diagnostics. This is not physical collision avoidance, not PX4 execution, not real perception, and not hardware readiness. |
| M86 MAVLink Safety / FC Contract | Complete as transport-free FC contract layer | `swarm-comms` now exposes FC geofence types, fence compilation into `MavlinkCommonPlan.geofence_prelude`, `fence_summary`, `FcParamRequirement` / `FcParamSnapshot` validation, known PX4/ArduPilot parameter metadata, and `FcContractValidationResult`. Capability profiles include per-kind fence rules; PX4 polygon fences are modeled with caveats, PX4 circle fences and all ArduPilot fences remain blocked until evidence exists. `swarm-sim::safety_config_to_fence_plan` bridges simulation AABB safety config into FC fence plans. This is dry-run contract evidence only: no live fence upload, no parameter read/write transport, no hardware readiness, and no certified safety claim. See `docs/FC_CONTRACT.md`. |
| M87 Swarm Command Plane | Complete as supervisor-integrated mission-level command-plane foundation | New crate `swarm-command-plane` defines `swarm_command_plane.v1`, command-plane roles, per-agent fanout into M80 `MissionCommandPlan` and M81/M82/M86 `MavlinkCommonPlan`, ownership records/handoffs, per-agent failure policies, synchronized GCS command windows, full manifest/report command-plane artifacts, SITL replay events, summary counters, and strict artifact-validator checks. This is mission-level coordination only: no RF mesh, no distributed consensus guarantee, no physical collision avoidance, no simultaneous hardware takeoff claim, and no PX4/ArduPilot equivalence guarantee. See `docs/SWARM_COMMAND_PLANE.md`. |
| M88 Logical Swarm Topologies | Complete as coordination-layer topology contract | `swarm-command-plane` now records logical topology config and deterministic command-route decisions for centralized GCS, P2P logical links, relay paths, mesh partitions, and mothership dependencies. `multi_sitl.v1` accepts optional topology configs, manifests include compact topology summaries and full route artifacts, SITL/generic replay logs emit topology configured/route selected/blocked/degraded/dependency events, and strict artifact validation checks topology consistency. This is not an RF mesh stack, not production radio routing, not physical mothership behavior, not distributed consensus, and not hardware readiness. See `docs/SWARM_TOPOLOGIES.md`. |

## Current Known Limitations

### SITL / PX4

- **M48 is live-verified on local PX4 SIH, not Gazebo or hardware.** The
  repository contains a 2026-05-30 PX4 SIH result with version/backend/command,
  report JSON, and replay summary. It does not prove Gazebo behavior, HIL, real
  aircraft, or production safety.
- **Multi-agent PX4/SIH execute orchestration exists as a local experimental
  workflow.** M52 proves explicit ownership, per-agent commands, mock
  supervisor orchestration, and a local two-instance PX4 SIH upload-only check.
  M58 adds `sitl_supervisor --connection --execute` for local endpoints with a
  common report/event log and a captured two-agent execute artifact. This still
  is not PX4 CI, Gazebo/HIL, real hardware, or production flight orchestration.
- **M59 reallocation is controlled local SITL, not production failover.**
  Reallocation events are emitted by `sitl_supervisor --mock`, by fake
  live-supervisor tests, and by a controlled local PX4/SIH failure-injection
  artifact. The live supervisor starts local agents, polls them stepwise,
  detects a failed active agent, releases unfinished tasks through runtime
  reallocation, and aborts/replaces an active survivor mission. This remains a
  narrow local SIH workflow, not automated PX4 CI, Gazebo/HIL, hardware, or
  onboard distributed autonomy.
- **M57 was an internal boundary; M58 is the first live supervisor plumbing.**
  The mock supervisor state machine remains testable behind an internal
  controller boundary, while M58 adds the separate live PX4/SIH execute path and
  M59 adds explicit mission replacement after failed-agent reallocation.
- **M60 hardens repeatability and failure diagnosis.** Local supervisor runs can
  use `--output-dir` and `--run-id` to produce a predictable run directory with
  manifest, event log, run report, and replay summary. Existing artifacts are
  protected unless `--force` is explicit, and stable exit codes separate
  CLI/config, safety, endpoint/feature, upload/command, timeout, runtime, and
  artifact failures.
- **Hardware is out of scope.** The project is not flight-certified and is not a
  production safety layer.

### Regression / Benchmarks

- **Default regression determinism sweep passed after fixes.** The sweep covered
  `regression_runner` and `strategy_comparison --regression` at `jobs=1/4/14`
  with repeated runs. Artifacts are in
  `results/m56_regression_determinism_2026-05-30/`.
- **M69 refreshed current-head simulation benchmark evidence.** The 1000-seed
  release benchmark for code commit `5d1d3cd17cacba7482c1d9b93eb5acc107af8f71`
  is in `results/all_1000_jobs14_m69_release/` and is summarized in
  `docs/BENCHMARK_RESULTS.md`.
- **M78 adds interpretation metadata, not a replacement long run.** New
  benchmark packs record `artifact_kind`, exports include confidence interval
  fields and support classification, and stale packs remain historical unless
  their manifest `git_commit` matches the checked-out HEAD.
- **M62 remains historical simulation evidence.** The 500-seed release baseline
  is preserved in `results/all_500_jobs14_m62_release/` for commit
  `81260ca7afa114a5d9add7b832f6c5d7875b88cd`.
- **The 1000-seed benchmark is still not an M48 substitute.** It evaluates
  simulation behavior only; live PX4 SITL requires local PX4/SIH runs.
- **Urban is not part of current `--mission all`.** M69 covers the built-in
  coverage/emergency-mesh/SAR/inspection/wildfire benchmark suite. Urban
  algorithm evidence remains the separate M68 scenario-suite artifact. M78 adds
  explicit `--mission urban` for future Urban benchmark evidence without
  changing the M69-compatible `--mission all` suite.

### Algorithmic

- **SAR CBBA**: unsupported due to delayed reconvergence after task release.
- **SAR Centralized**: unsupported because static pre-planning is incompatible
  with dynamic belief search.
- **Inspection perimeter**: constrained by battery/time and intentionally
  experimental for some strategies.
- **Flood mission**: future work; not implemented as a separate mission.
- **Wildfire success semantics**: `success=true` requires
  `mapped_zone_count / total_zone_count >= wildfire_success_threshold` (default
  `0.8`), all expected failures detected, and
  `max_task_unassigned_ticks <= max_unassigned_ticks`. This is intentionally
  stricter than task completion, so wildfire benchmark rows can show
  `Completion = 1.000` while `Success < 1.000`.
- **Urban Patrol v0 + M70 export**: M65 implements one-agent ordered road-graph
  patrol in simulation; this is the ordered road-graph patrol boundary.
  Completion means all planned segments are traversed
  before timeout with zero Urban judge violations. M70 can export that planned
  route to a deterministic dry-run/SITL-compatible waypoint list with
  `geo_origin` metadata and an optional JSON artifact. This is not lidar, real
  obstacle avoidance, dynamic traffic, multi-agent deconfliction, PX4 execution,
  hardware readiness, or real perception.
- **Urban Search v1**: M66 adds one-agent mocked bus search on top of the same
  road graph. The detector is deterministic and distance/probability based; it
  is not lidar/raycast, computer vision, dynamic traffic, multi-agent
  deconfliction, PX4/SITL export, hardware readiness, or real perception.
- **Urban Replay / Analysis**: M67 makes Urban runs easier to inspect. Replay
  timeline output can be filtered by agent or Urban event category, benchmark
  packs can include route-trace and judge-report JSON/CSV artifacts, and
  `scenarios/urban.multi-agent.json` provides a deterministic two-agent
  analysis fixture. Suite-mode runs with replay enabled can now produce those
  analysis artifacts for the fixture. The separation/conflict metrics are
  diagnostic measurements from replay traces, not a route-deconfliction or
  collision avoidance system.
- **Urban Multi-Agent Deconfliction**: M85 adds opt-in mission-level segment
  ownership for `urban-patrol` via
  `run_config.urban_state.deconfliction.enabled`. Agents reserve an Urban
  road-graph segment before entering it and release it after completion.
  Supported right-of-way policies are `first_come`, `priority`, and
  `round_robin`; `mission_critical_override` is deliberately unsupported until
  a concrete rule exists. Locked segments can trigger `wait`, `replan`, or
  `abort`, with dedicated replay events and `urban_deconflict_*` metrics. This
  is not lidar/raycast, not physical collision avoidance, not RF coordination,
  not PX4/SITL execution evidence, not hardware readiness, and not real
  perception. `artifact_validator --mode benchmark-pack` now checks M85
  `urban_analysis/*.segment-ownership.json` artifacts for overlapping holders
  on the same Urban edge when such artifacts are present.
- **Urban Corridor-Aware Planner**: M68 adds `planner: "corridor-aware"` as an
  experimental mission-level route planner. It penalizes narrow corridors and
  low static-obstacle clearance to reduce `urban_route_risk_score` on the
  deterministic corridor-delta fixture. It is not lidar, physical collision
  avoidance, dynamic traffic handling, PX4 execution, or hardware evidence.
- **Urban Mission Realism Follow-up**: M75 adds scheduled moving semantic bus
  targets and deterministic perimeter patrol metrics inside the simulation
  layer. The bus detector is still mocked and distance/probability based; the
  perimeter path is still a mission-level route pattern over the Urban graph.
  This is not real lidar/raycast, a physics engine, dynamic obstacle avoidance,
  PX4 execution evidence, or hardware validation.
- **Synthetic Scenario Testbed**: M76 adds deterministic generated Scenario DSL
  inputs and generator manifests. It helps produce reproducible Urban regression
  fixtures and extension inputs, but it is not a calibrated benchmark corpus,
  PX4/SITL validation, physical simulation, hardware validation, real
  perception, or obstacle-avoidance certification.
- **Algorithm Differentiation**: M77 adds opt-in algorithm knobs and replay
  diagnostics so targeted deltas are explainable. Defaults preserve previous
  behavior. The committed coverage artifact is a 1-seed release smoke run, not
  statistical evidence. CBBA heavy-loss behavior remains diagnostic-only unless
  later replay evidence justifies a bounded gossip-burst change.

### Platform / API

- **Extension points are documented, not published as an SDK.** M61 documents
  stable-ish in-repository paths for missions, strategies, metrics, and schema
  changes. It does not promise semver-stable public API, crate publication, or
  external plugin compatibility.
- **Test-only extension fixtures are not supported features.** The M61 adapter,
  strategy, and runner fixtures validate contracts but are not new real
  missions or benchmark strategies.

## Readiness

| Goal | Status | Blocker |
|---|---|---|
| Portable SITL verification | Ready | Run `sitl_agent`/`sitl_docs` targeted tests. |
| In-repository extension work | Ready with M61 boundaries | Use `docs/EXTENSION_GUIDE.md`; external semver-stable plugin/API work remains out of scope. |
| Urban algorithm work | Has M68/M76 inputs and M85 opt-in deconfliction; not in full benchmark yet | M68 provides a corridor-aware planner delta and route-risk metric. M76 adds deterministic synthetic Urban fixtures for regression/extension work. M85 adds simulation-only mission-level segment ownership, not physical avoidance. The current M69 `--mission all` benchmark does not include Urban scenario suites; dynamic obstacles, richer judging, physical collision avoidance, and hardware evidence remain future work. |
| Algorithm differentiation work | Ready for targeted follow-up | M77 exposes opt-in knobs and targeted profile filtering. Use controlled tests and small targeted runs first; do not extrapolate the 1-seed M77 smoke artifact to publication claims. |
| M48 live PX4 verification | Complete for local PX4 SIH | Captured in `results/m48_px4_sitl_2026-05-30/`; Gazebo/HIL/hardware remain out of scope. |
| Real multi-agent PX4/SIH | Experimental local workflow with M60 hardening | Upload-only, execute, and controlled failure/reallocation SIH evidence exists. `sitl_supervisor --connection --execute --reupload-on-failure --output-dir ... --run-id ...` can produce stable artifacts and exit codes for local runs; automated PX4 CI, Gazebo/HIL, hardware, broader failure modes, and production safety remain future work. |
| Artifact validation | Ready for local SITL packs | Use `artifact_validator --output-dir <pack> --mode supervisor-run --strict` for new supervisor output dirs. Historical M58/M59 packs can be checked with `--allow-historical`; live harness scripts remain manual-only. |
| Degraded supervisor evidence | Ready for fake-tested/pre-hardware packs | New supervisor packs with failures should include `degraded` records and matching `supervisor_failure_detected` / `supervisor_failure_classified` / recovery events. Use `docs/DEGRADED_SUPERVISOR.md` as the boundary document. |
| Large benchmark publication | Evidence captured, interpretation improved but still not publication-grade | M69 provides a 1000-seed release simulation pack for its recorded commit. M78 adds stats/support metadata and one degradation-sweep entrypoint, but publication claims still need broader degradation curves, paired comparisons, and must not be presented as PX4/SITL or hardware evidence. |
| Hardware experiment | Not product-ready | Requires external safety process; see `docs/HARDWARE_READINESS.md`. |

## Recommended Next Steps

1. Use `docs/EXTENSION_GUIDE.md` when adding the next in-repository mission,
   strategy, metric, or schema field. Keep the support matrix and regression
   coverage in the same change.
2. Use M60 `--output-dir` / `--run-id` for any new local PX4/SIH evidence so
   artifacts are repeatable and overwrite-safe.
3. Expand the local M59 workflow only if the project needs broader failure
   modes, repeated failure recovery, or automated PX4/SIH orchestration.
4. Inspect M69 SAR, wildfire, emergency-mesh, and CBBA benchmark interpretation
   gaps before making publication-level algorithm claims.
5. Use M68 corridor-delta only as small Urban algorithm evidence. Use M76
   generated suites for reproducible Urban regression inputs; add Urban to a
   future full benchmark entrypoint only if broader Urban claims are needed.
6. Use M70 `sitl_agent --dry-run --dry-run-artifact` before any optional manual
   Urban PX4/SIH upload experiment; do not treat the artifact as hardware or
   obstacle-avoidance evidence.
7. Keep M71 `docs/PREFLIGHT_SAFETY.md` in sync with rule ids and exit code
   semantics whenever preflight checks or CLI categories change.
8. Use M72 `artifact_validator` for new local supervisor evidence packs before
   citing them in README/status docs. Keep historical mode explicit for old
   committed evidence that lacks M72 metadata.
9. Keep README, `docs/BENCHMARK_RESULTS.md`, `docs/EXTENSION_GUIDE.md`,
   `docs/SITL_SETUP.md`, `docs/SCENARIO_DSL.md`, `docs/REPLAY.md`,
   `docs/ARTIFACT_VALIDATION.md`, and this file in sync when extension, Urban
   analysis, or SITL evidence changes state.

## How to Verify This Status

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-types adapter

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-alloc strategy

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim extension_fixture

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim urban

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-scenarios urban

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-scenarios generated

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim --test scenario_catalog

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-replay timeline

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim urban_analysis

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-comms --features mavlink-transport

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-runtime reallocation

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_supervisor

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_connection
```

For M48 live verification, inspect `results/m48_px4_sitl_2026-05-30/`. For the
two-instance PX4 SIH upload-only check, inspect
`results/m55_multi_agent_px4_sih_2026-05-30/`. For M58/M59, inspect
`results/m58_multi_agent_px4_sih_execute_2026-05-31/` and
`results/m59_px4_sih_failure_reallocation_2026-05-31/`. M60 adds local
supervisor artifact/exit-code/report hardening. M61 adds the extension guide
and test-only extension contract checks. M62 adds a 500-seed release simulation
benchmark baseline for commit `81260ca7afa114a5d9add7b832f6c5d7875b88cd`; M63
marks it historical because no current-HEAD rerun was performed. M64 adds Urban
foundation code and docs, M65 adds Urban Patrol v0 simulation semantics, M66
adds Urban Search v1 simulation semantics with a mocked bus detector, M67 adds
Urban replay/analysis diagnostics, M68 adds a small corridor-aware planner
delta, M74/M75 add Urban decision/mission realism semantics, and M76 adds
deterministic generated Urban testbed inputs; none of these milestones refreshes
the full benchmark evidence. Do not extend
any existing result to Gazebo, HIL, real hardware, automated PX4 CI,
semver-stable external API, or publication-level algorithm claims without new
code/evidence.
