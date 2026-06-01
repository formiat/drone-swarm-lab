# BEFORE_HARDWARE_C.23 - итоговые майлстоуны до железа

Дата фиксации: 2026-06-01

Источник: синтез `BEFORE_HARDWARE_A.22`, `BEFORE_HARDWARE_B.22`,
`BEFORE_HARDWARE_C.22` и последующего обсуждения.

Этот документ заменяет предыдущую C-версию как рекомендуемый план. Лучший
базовый порядок взят из `C.22`: сначала mission/export/safety/evidence/fault
discipline, затем realism, scenario pressure, algorithm credibility, benchmark
evidence and runbooks. Из `A.22` взята компактная milestone-линейка и
pre-hardware boundary. Из `B.22/B.23` взяты конкретные полезные задачи:
`geo_origin`, SAR threshold, confidence intervals, CBBA diagnostics,
communication-aware scoring, wildfire priority reallocation, SAR belief
updates, moving bus and perimeter patrol.

## Контекст

Железа сейчас нет и в ближайшее время не будет. Поэтому цель не в том, чтобы
сделать "боевой дрон" без дрона. Цель:

```text
Поднять проект до hardware-integration candidate:
когда железо появится, интеграция начинается с контролируемой,
документированной и протестированной основы, а не с ad-hoc экспериментов.
```

Архитектурная граница остается прежней:

- PX4/autopilot owns: stabilization, attitude/rate control, motor physics,
  low-level waypoint execution, flight failsafes.
- This project owns: mission-level planning, route export, task allocation and
  reallocation, preflight validation, replay, metrics, benchmark evidence,
  SITL workflows and operator discipline.

Главный принцип:

```text
Не строим "виртуальный реальный дрон".
Строим production-grade mission/supervisor layer:
validation, artifacts, fault handling, replay, metrics and run discipline.
```

## Target State Before Hardware

Перед первым controlled hardware experiment проект должен иметь:

- Urban route export to waypoint mission;
- strict preflight safety gate before dry-run/SITL/upload;
- artifact validator for every serious run;
- deterministic fault-injection matrix;
- mission-level blocked-route decisions;
- useful mission realism without fake physics: moving bus, perimeter patrol,
  temporary blocked edges;
- seeded scenario generator for stress and degradation testing;
- algorithm credibility work with interpretable benchmark deltas;
- benchmark/degradation evidence with uncertainty;
- operational runbooks and explicit hardware go/no-go gates.

Это все еще не делает проект production-ready. Это делает его disciplined
mission/supervisor research platform, готовой к будущей осторожной интеграции с
железом.

## Non-Goals

До появления железа не делать как основной workstream:

- hardware-specific code paths beyond existing boundary guards;
- real HIL;
- real lidar/raycast/SLAM/CV;
- certified obstacle avoidance;
- regulatory or production safety claims;
- public semver API promise;
- UI/visualizer as main readiness work;
- arbitrary polygon/navmesh/geometry engine;
- long 1000-seed reruns without new behavior or interpretation layer.

## Milestone Chain

```text
M70 Urban Route Export + Preflight Safety Gate
  -> M71 Artifact Validator + Local SITL Harness
    -> M72 Fault Injection + Degraded-Mode Supervisor
      -> M73 Urban Blocked-Route Decision Logic
        -> M74 Mission Realism: Moving Bus + Perimeter Patrol
          -> M75 Synthetic Scenario Testbed
            -> M76 Algorithm Credibility + Differentiation
              -> M77 Benchmark + Degradation Evidence
                -> M78 Operational Runbooks + Hardware Entry Gate
```

Почему не начинать с benchmark credibility как отдельного M70: benchmark
становится максимально полезным после того, как есть новые behavior boundaries
и failure/urban logic, которые можно измерять. Но отдельные "credibility"
исправления из B-плана не теряются: они входят в M76/M77.

---

## M70 - Urban Route Export + Preflight Safety Gate

### Goal

Соединить существующий Urban simulation layer с PX4/SIH waypoint workflow и
сразу поставить preflight gate перед любым dry-run/SITL/upload.

```text
Urban planned route -> ordered waypoint mission -> safety report -> dry-run/SITL-compatible plan
```

Это не hardware execution. Это deterministic conversion and validation path,
который позже можно использовать в SITL и hardware-adjacent экспериментах.

### Scope

1. Route-to-waypoint conversion:
   - convert `UrbanPlannedRoute` segments into ordered waypoint items;
   - preserve node/edge/task/segment identity where practical;
   - keep deterministic ordering;
   - explicitly include altitude/default mission parameters;
   - define waypoint spacing for long Urban edges.

2. Configurable `geo_origin`:
   ```rust
   pub struct GeoOrigin {
       pub lat_deg: f64,
       pub lon_deg: f64,
       pub alt_m: f64,
   }
   pub struct Scenario {
       pub geo_origin: Option<GeoOrigin>,
       // ...
   }
   ```
   - If `scenario.geo_origin` exists, pass it to `MissionUploadOptions.home_origin`.
   - If absent, keep current SITL default behavior.
   - Add explicit current PX4/SIH default origin to SITL scenarios so the
     coordinate frame is visible in data, not hidden in code.

3. Preflight safety gate:
   - geofence bounds;
   - no-fly/static obstacle intersection;
   - known Urban graph edges only;
   - blocked edges rejected unless policy explicitly allows wait/replan later;
   - max altitude and min altitude where relevant;
   - max route length / estimated duration if already practical;
   - duplicate task or segment ownership;
   - missing waypoint/task ids;
   - invalid or non-finite coordinates.

4. `SafetyValidationReport` shape:
   ```rust
   pub struct SafetyValidationReport {
       pub passed: bool,
       pub violations: Vec<SafetyViolation>,
   }

   pub struct SafetyViolation {
       pub rule_id: String,
       pub severity: ViolationSeverity,
       pub affected_id: Option<String>,
       pub reason: String,
   }
   ```

5. Dry-run export artifact:
   - source scenario path;
   - planner/adapter name;
   - route length;
   - waypoint count;
   - start/end waypoint summary;
   - altitude and `geo_origin`;
   - safety result;
   - run id, command, config snapshot and git commit where practical.

6. CLI behavior:
   - unsafe mission exits before upload/dry-run success;
   - error message names failed rule ids;
   - output artifact records safety result if `--output-dir` was requested;
   - stable exit code convention starts here: validation=2, runtime=3,
     artifact=4, environment=5.

7. Documentation:
   - clarify that export means local waypoint workflow;
   - no hardware readiness claim;
   - no real obstacle avoidance claim;
   - no real perception claim.

### Non-Goals

- No hardware run.
- No Gazebo/HIL gate by default.
- No certified obstacle avoidance.
- No arbitrary polygon/navmesh requirement.
- No low-level flight control.

### Done Criteria

- Urban patrol route exports to deterministic waypoint list.
- Exported mission passes preflight or fails with structured rule ids.
- Dry-run artifact is readable and reproducible.
- `geo_origin` can override the default origin without PX4.
- SITL scenarios expose their origin in scenario data.
- Docs separate simulation route validity from SITL waypoint execution.

### Automated Tests

#### Tests That Need No Refactoring

- `urban_route_exports_ordered_waypoints`: simple Urban route -> expected
  waypoint order.
- `urban_route_export_stable_ids`: repeated export preserves task/segment ids.
- `urban_route_altitude_explicit`: altitude is preserved or defaulted
  deterministically.
- `geo_origin_roundtrip_json`: `geo_origin` serializes/deserializes without loss.
- `geo_origin_overrides_default_in_dry_run`: local offset converts from supplied
  origin, not hardcoded default.
- `safety_rejects_forbidden_obstacle_route`: exported route crossing static
  obstacle fails before dry-run success.
- `safety_rejects_duplicate_ownership`: duplicate task/segment ownership fails.
- `safety_rejects_non_finite_waypoint`: NaN/inf coordinate fails.

#### Tests That Need Light Refactoring

- Shared Urban route-to-waypoint fixture builder.
- Safety report assertion helper with rule-id matching.
- Export metadata assertion helper.
- CLI output helper for validation exit code and rule-id assertions.

#### Tests That Need Heavy Refactoring

- Manual/ignored local PX4/SIH upload test for exported Urban route.
- Route densification property tests.
- Cross-run export artifact comparison tool.
- Battery reserve estimator tests if route duration estimation becomes part of
  the gate.

---

## M71 - Artifact Validator + Local SITL Harness

### Goal

Make run artifacts trustworthy. Future hardware work should depend on
machine-checkable evidence, not console output or manual notes.

### Scope

1. Artifact validator inputs:
   - manifest;
   - run report;
   - event log;
   - replay summary;
   - safety validation report;
   - scenario snapshot where present;
   - benchmark/result table where relevant.

2. Validator checks:
   - manifest has command, git commit, build profile, run id and schema version;
   - run id and output directory are consistent;
   - event log final status matches run report final status;
   - completed tasks in report exist in event log;
   - replay summary counts match event log categories;
   - replacement mission completion events reference active replacement mission
     seq, not the original manifest seq;
   - agent ids and task ids are consistent across artifacts;
   - no accidental overwrite unless `--force` was used;
   - SITL/PX4 artifacts include a limitations section.

3. CLI/tooling:
   - `validate-artifact --dir <path>` or an equivalent library-backed command;
   - readable error list with rule ids;
   - exit code 0 for valid, non-zero for invalid;
   - portable tests using inline or committed tiny fixtures.

4. Local SITL harness scripts:
   - `scripts/run_m58_local.sh`:
     - start one or two PX4/SIH instances;
     - wait for MAVLink endpoints;
     - run `sitl_supervisor`;
     - collect logs;
     - cleanup via `trap`;
     - write artifacts to deterministic output directory.
   - `scripts/run_m59_local.sh`:
     - same baseline plus deterministic kill/failure trigger;
     - verify supervisor detects agent loss and creates replacement/recovery
       artifact.
   - Scripts are manual/local. They are not default CI.

5. Result discipline:
   - explicit `--run-id`;
   - explicit `--output-dir`;
   - explicit `--force` overwrite semantics;
   - command-line capture;
   - config snapshot.

6. Documentation:
   - define acceptable evidence for simulation, dry-run, local PX4/SIH and
     future hardware;
   - add "Local M58/M59 reproduction" section to SITL docs;
   - state local assumptions and expected failure messages.

### Non-Goals

- No remote artifact store.
- No CI-managed PX4 container by default.
- No hardware artifact claim.
- No broad PX4 version certification.

### Done Criteria

- A valid tiny artifact fixture passes validator.
- Deliberately inconsistent fixtures fail with clear rule ids.
- M58/M59-style reports are covered by validator logic or compatibility tests.
- A developer can rerun local M58/M59-like workflows from documented scripts.
- Missing PX4 produces actionable environment error.

### Automated Tests

#### Tests That Need No Refactoring

- `validator_valid_tiny_artifact_passes`.
- `validator_missing_manifest_field_fails`.
- `validator_final_status_mismatch_fails`.
- `validator_event_report_task_mismatch_fails`.
- `validator_replay_summary_count_mismatch_fails`.
- `validator_replacement_completion_seq_uses_active_mission`.
- `validator_requires_sitl_limitations_section`.

#### Tests That Need Light Refactoring

- Shared artifact fixture builder.
- Validator rule-id assertion helper.
- Event-log/report consistency helper.
- Harness dry-run mode that does not launch PX4.
- Portable process cleanup helper for script tests.

#### Tests That Need Heavy Refactoring

- Validator over full committed M58/M59 artifact directories.
- Multi-artifact pack validator for benchmark directories.
- Schema-version compatibility matrix.
- Ignored/manual two-PX4 harness integration test.

---

## M72 - Fault Injection + Degraded-Mode Supervisor

### Goal

Systematically exercise failure behavior before hardware exists.

Successful golden paths are not enough. The supervisor needs explicit behavior
for degraded conditions:

```text
detect -> classify -> decide -> recover/abort -> report
```

### Scope

1. Failure modes:
   - agent lost before upload;
   - mission upload rejected;
   - agent lost after upload before start;
   - heartbeat lost;
   - no-progress timeout;
   - partial completion then failure;
   - stale telemetry;
   - replacement mission rejected;
   - survivor fails after replacement;
   - bad waypoint/mission item;
   - duplicate ownership discovered mid-run;
   - unsupported strategy selected.

2. Supervisor decisions:
   - abort;
   - wait;
   - reassign unfinished tasks;
   - release tasks;
   - mark partial success;
   - mark total failure;
   - continue with survivor;
   - refuse unsafe replacement.

3. Report fields:
   - failure mode;
   - detected tick/time;
   - affected agent;
   - tasks completed before failure;
   - tasks recovered;
   - tasks abandoned;
   - replacement mission id;
   - recovery latency;
   - final status.

4. Replay events:
   - failure detected;
   - failure classified;
   - recovery started;
   - recovery completed;
   - recovery failed;
   - final degraded status.

5. Metrics:
   - failure detected count;
   - tasks released;
   - tasks reassigned;
   - recovery latency ticks;
   - survivor completion status;
   - unrecovered tasks.

6. Documentation:
   - failure matrix table;
   - supported / experimental / unsupported / not-tested status;
   - exact recovery semantics;
   - which failures are fake-controller only and which have local SITL evidence.

### Non-Goals

- No hardware failure testing.
- No physical failsafe validation.
- No real RF/link-loss modeling beyond deterministic profiles.
- No repeated-failure policy beyond bounded first implementation.

### Done Criteria

- Every supported failure mode has fake-controller coverage.
- Supervisor final status is deterministic and explainable.
- Recovered task ownership remains valid.
- Artifact validator verifies degraded-mode runs.
- At least one representative local SITL failure artifact remains valid if the
  local environment is available.

### Automated Tests

#### Tests That Need No Refactoring

- `fake_upload_rejection_is_reported`.
- `fake_no_progress_timeout_reassigns_tasks`.
- `fake_heartbeat_lost_releases_tasks`.
- `fake_partial_completion_then_disconnect_preserves_completed_tasks`.
- `fake_replacement_mission_rejected_marks_partial_or_failure`.
- `fake_survivor_completes_recovered_tasks`.
- `failure_metrics_aggregate_expected_counts`.
- `degraded_run_artifact_validates`.

#### Tests That Need Light Refactoring

- Reusable fake controller scenario builder.
- Failure-mode assertion helper.
- Shared final-status validation helper.
- Artifact validator integration with failure reports.

#### Tests That Need Heavy Refactoring

- Manual/ignored local PX4/SIH fault-injection harness.
- Repeated failure property tests.
- Long-running supervisor soak with synthetic failures.
- Stochastic communication failure sweeps.

---

## M73 - Urban Blocked-Route Decision Logic

### Goal

Add mission-level reactivity without pretending to implement real obstacle
avoidance.

```text
edge becomes blocked -> detector/policy notices -> wait or replan -> judge/report
```

This is the correct place for the user's "do not collide with buildings/other
objects" idea at the current project level: the simulation provides structured
blocked edges and a deterministic mock detector, while the project tests
mission-level decisions.

### Scope

1. Dynamic blocked route state:
   ```rust
   pub struct UrbanTemporaryObstacle {
       pub edge_id: UrbanEdgeId,
       pub appears_at_tick: u64,
       pub disappears_at_tick: Option<u64>,
       pub reason: String,
       pub severity: ObstacleSeverity,
   }

   pub enum ObstacleSeverity {
       Hard,
       Soft,
   }
   ```

2. Effective blocked set per tick:
   - static blocked edges from map;
   - active temporary obstacles;
   - optional policy distinction for hard/soft blocks.

3. Mock obstacle detector:
   - graph lookahead by N hops or distance;
   - deterministic result;
   - no real lidar/raycast;
   - no physical sensor stream;
   - outputs "blocked edge observed" events, not raw perception.

4. Policies:
   - `Wait`: hold until edge unblocks;
   - `Replan`: compute alternate route around blocked edge;
   - `Abort`: fail safely if no route exists;
   - `Yield`: later multi-agent policy where another agent has priority.

5. Judge behavior:
   - route through active hard blocked edge is violation;
   - wait/replan policy can avoid violation;
   - no-route scenario must fail explicitly, not hang.

6. Replay/report:
   - `UrbanEdgeBlocked`;
   - `UrbanEdgeUnblocked`;
   - `UrbanBlockedEdgeObserved`;
   - `UrbanPolicyDecision`;
   - `UrbanRouteReplanned`;
   - `UrbanWaitStarted`;
   - `UrbanWaitCompleted`;
   - `UrbanAbortReason`.

7. Metrics:
   - `urban_replan_count`;
   - `urban_wait_time_ticks`;
   - `urban_blocked_edge_count`;
   - `urban_replan_success_rate`;
   - `urban_unresolved_blockage_count`;
   - `urban_violation_count`.

### Non-Goals

- No certified obstacle avoidance.
- No real lidar/raycast/SLAM/CV.
- No physics.
- No arbitrary polygon geometry beyond small helpers required by Urban.

### Done Criteria

- One deterministic blocked-edge scenario recovers by wait.
- One deterministic blocked-edge scenario recovers by replan.
- One no-route scenario fails safely with explicit reason.
- Replay explains the decision.
- Metrics distinguish route following from wait/replan behavior.

### Automated Tests

#### Tests That Need No Refactoring

- `temporary_obstacle_active_within_window`.
- `temporary_obstacle_without_disappears_stays_active`.
- `runner_emits_edge_blocked_event`.
- `runner_emits_edge_unblocked_event`.
- `judge_rejects_agent_on_hard_blocked_edge`.
- `wait_policy_completes_after_unblock`.
- `replan_policy_finds_alternate_route`.
- `abort_policy_reports_no_route`.
- `replay_contains_policy_decision_event`.

#### Tests That Need Light Refactoring

- Blocked-edge scenario builder.
- Route policy assertion helper.
- Urban replay event fixture helper.
- Small graph helper with alternate path/no alternate path variants.

#### Tests That Need Heavy Refactoring

- Multi-agent yield policy tests.
- Dynamic obstacle schedule property tests.
- Larger generated-map stress tests.
- Cross-policy comparison benchmark fixture.

---

## M74 - Mission Realism: Moving Bus + Perimeter Patrol

### Goal

Add realistic mission primitives that remain compatible with the current project
boundary. This milestone is not about physics or visual simulation. It is about
making the headless mission layer closer to useful real tasks.

### Scope

1. Moving bus model:
   ```rust
   pub struct UrbanBusStop {
       pub node_id: UrbanNodeId,
       pub arrival_tick: u64,
   }

   pub struct UrbanBusRoute {
       pub stops: Vec<UrbanBusStop>,
       pub speed_m_per_tick: f64,
   }

   pub struct UrbanBus {
       pub route: Option<UrbanBusRoute>,
       // existing fields remain for static bus compatibility
   }
   ```

2. `UrbanBus::pose_at_tick(map, tick)`:
   - static bus: current pose while active;
   - moving bus: interpolate between route stops;
   - outside active window: `None`;
   - invalid route: scenario validation failure.

3. Detection:
   - `detect_buses` uses `pose_at_tick`;
   - detection probability remains explicit;
   - no real CV/perception claim;
   - replay records bus position/detection at tick where relevant.

4. Perimeter patrol:
   - scenario DSL accepts polygon, spacing, altitude;
   - builder creates deterministic waypoints along perimeter;
   - last waypoint closes route or completion predicate explicitly defines loop;
   - can export through M70 route-to-waypoint path;
   - metrics include perimeter length, completion, time to complete and
     violations.

5. Temporary obstacles compatibility:
   - moving bus and perimeter patrol can be combined with M73 blocked-edge logic;
   - blocked perimeter segment should trigger wait/replan/abort policy.

6. Documentation:
   - this is mission realism, not physical realism;
   - moving bus is simulated target schedule, not perception stack;
   - perimeter patrol is waypoint mission, not continuous flight planner.

### Non-Goals

- No geometry engine for arbitrary real maps.
- No lidar or camera model.
- No multi-height/3D building avoidance.
- No real traffic simulation.

### Done Criteria

- Static bus behavior remains backward compatible.
- Moving bus pose is deterministic and validated.
- Urban Search detects moving bus only when agent and bus positions intersect
  within detection conditions.
- Perimeter patrol completes on simple square fixture.
- Perimeter patrol can produce waypoint export/dry-run artifact.

### Automated Tests

#### Tests That Need No Refactoring

- `bus_pose_at_tick_static_returns_fixed_pose`.
- `bus_pose_at_tick_interpolates_between_stops`.
- `bus_pose_at_tick_none_outside_window`.
- `detect_buses_finds_moving_bus_when_in_range`.
- `detect_buses_misses_moving_bus_out_of_range`.
- `perimeter_waypoints_square_expected_count`.
- `perimeter_waypoints_deterministic`.
- `perimeter_patrol_completes_simple_square`.
- `perimeter_patrol_metrics_exported`.

#### Tests That Need Light Refactoring

- Shared perimeter waypoint builder.
- Urban moving-target fixture helper.
- Mission adapter fixture for perimeter patrol.
- Export helper for perimeter route dry-run.

#### Tests That Need Heavy Refactoring

- Property tests for convex polygons.
- Concave polygon handling decision and tests.
- Multi-agent perimeter segmentation.
- Perimeter with dynamic blocked segment and replan.

---

## M75 - Synthetic Scenario Testbed

### Goal

Avoid overfitting to a few hand-written scenarios. Build deterministic scenario
families for stress and degradation tests.

Without hardware, realistic pressure comes from reproducible variation: maps,
blocked edges, bus schedules, packet loss, failures, obstacle density and task
density.

### Scope

1. Scenario generator API:
   - deterministic seed;
   - explicit generator parameters;
   - stable scenario names;
   - manifest records generator settings and schema version.

2. Initial generators:
   - Urban grid/block maps;
   - blocked edge schedules;
   - bus route schedules;
   - perimeter patrol polygons;
   - packet-loss profiles;
   - latency profiles;
   - agent failure profiles;
   - wildfire threat patterns where already supported.

3. Scenario manifest:
   ```rust
   pub struct GeneratedScenarioManifest {
       pub generator_name: String,
       pub seed: u64,
       pub parameters: serde_json::Value,
       pub schema_version: String,
       pub git_commit: Option<String>,
   }
   ```

4. Scenario library categories:
   - `tiny`: unit tests, very small;
   - `small`: quick regression;
   - `medium`: explicit CI smoke if acceptable;
   - `stress`: not default CI;
   - `regression-stable`: pinned seed, versioned expectation;
   - `experimental`: useful but not stable.

5. Validation:
   - generated scenario is valid;
   - same seed produces same scenario;
   - invalid parameter combinations fail clearly;
   - generated scenario does not depend on machine-local paths.

6. Documentation:
   - how to regenerate;
   - which profiles are default regression;
   - which profiles are exploratory;
   - how scenario generator versions affect historical artifacts.

### Non-Goals

- No opaque random benchmark.
- No large generated suites in default CI.
- No claim that generated distributions match real-world distributions.
- No generated scenario without reproducible seed/manifest.

### Done Criteria

- At least one Urban generator is committed.
- Same seed produces identical scenario.
- Generated scenarios include manifest metadata.
- At least one generated Urban blocked-edge fixture feeds M73 tests.
- Generated scenarios can be used by benchmark/regression tooling.

### Automated Tests

#### Tests That Need No Refactoring

- `same_seed_same_scenario`.
- `different_seed_changes_expected_fields`.
- `invalid_generator_config_rejected`.
- `generated_urban_map_validates`.
- `generated_blocked_edge_schedule_validates`.
- `generated_manifest_records_seed_and_params`.

#### Tests That Need Light Refactoring

- Scenario generator trait/helper.
- Shared manifest metadata assertion helper.
- Small generated scenario fixture.
- Snapshot-like assertion for stable tiny generated scenarios.

#### Tests That Need Heavy Refactoring

- Property tests over generated maps.
- Cross-mission generated scenario framework.
- Large scenario stress runner.
- Cross-version generator reproducibility tests.

---

## M76 - Algorithm Credibility + Differentiation

### Goal

Make algorithm behavior interpretable and measurably different in the conditions
where differences should matter.

This milestone combines the useful "benchmark credibility" and "algorithm
differentiation" tasks from B.22/B.23. It should be done after M70-M75 have
enough behavior and artifacts to measure.

### Scope

1. SAR success threshold:
   - add `sar_success_threshold: f64` to run config;
   - keep default `1.0` for backward compatibility;
   - scenario profiles may set `0.8` where "probability of detection" is the
     intended success predicate;
   - docs clarify SAR success = PoD threshold, not necessarily all targets found.

2. Confidence intervals and variance:
   - add `stddev_success_rate`;
   - add `stderr_success_rate`;
   - add min/max where practical;
   - report N runs;
   - export to JSON/CSV/Markdown.

3. CBBA diagnostics:
   - add optional `conflicting_task_count` to CBBA replay event;
   - inspect heavy-loss profiles with replay timeline;
   - either fix reconvergence issue, for example gossip burst after agent
     failure, or document limitation with evidence.

4. Communication-aware allocation scoring:
   - add `comms_penalty_weight: f64` defaulting to 0.0;
   - greedy/auction scoring penalizes task distance beyond `agent.comms_range`;
   - manifest records whether penalty was active;
   - compare heavy-loss/partition-prone profiles with and without penalty.

5. Wildfire priority-triggered reallocation:
   - add `wildfire_priority_realloc_threshold`;
   - priority update above threshold queues forced reallocation;
   - coordinator releases lower-priority assignment and reconsiders tasks next
     tick;
   - report/replay records the forced decision.

6. SAR belief-entropy ordering:
   - add `dynamic_belief_updates: bool`;
   - after scan, update remaining uncertainty for task ordering;
   - default false for compatibility;
   - benchmark static vs dynamic belief behavior.

7. Targeted benchmark delta:
   - coverage heavy-loss with/without comms penalty;
   - wildfire dynamic with/without priority reallocation;
   - SAR with/without dynamic belief;
   - CBBA diagnostic profile before/after fix or limitation statement.

### Non-Goals

- No "complex algorithm always wins" claim.
- No long benchmark rerun before metrics and hypotheses are defined.
- No unsupported strategy/mission pair presented as success.
- No ML/perception work.

### Done Criteria

- SAR benchmark rows become interpretable under explicit threshold.
- Exports include uncertainty fields.
- CBBA weak rows are explained or fixed with replay evidence.
- At least one targeted benchmark shows an expected algorithm delta.
- `docs/BENCHMARK_RESULTS.md` explains where complex strategies help, where
  they do not, and why.

### Automated Tests

#### Tests That Need No Refactoring

- `sar_threshold_0_8_succeeds_with_partial_detection`.
- `sar_threshold_1_0_requires_all_found`.
- `aggregate_stderr_zero_for_uniform_runs`.
- `aggregate_stderr_matches_known_formula`.
- `report_export_contains_stderr`.
- `cbba_bundle_updated_has_conflict_count`.
- `comms_penalty_reduces_score_beyond_range`.
- `comms_penalty_zero_no_effect`.
- `wildfire_priority_reallocates_above_threshold`.
- `wildfire_priority_below_threshold_no_realloc`.
- `sar_dynamic_belief_changes_task_order`.
- `sar_static_belief_unchanged_when_disabled`.

#### Tests That Need Light Refactoring

- Benchmark profile builder with explicit comms/priority/belief settings.
- Shared scoring delta assertion helper.
- CBBA fixture with explicit agent failure mid-run.
- Report export fixture helper for statistical fields.

#### Tests That Need Heavy Refactoring

- Property tests over aggregate statistics.
- Multi-agent communication partition scenarios.
- Wildfire repeated-threshold sweep.
- Long-run reproducibility harness for targeted deltas.

---

## M77 - Benchmark + Degradation Evidence

### Goal

Turn "it passed a scenario" into "we know where it works, where it degrades and
where it is unsupported".

M77 should happen after at least one new behavior from M73-M76 exists, because
otherwise benchmark work is mostly presentation.

### Scope

1. Statistical layer:
   - mean;
   - stddev;
   - stderr;
   - confidence interval;
   - min/max;
   - failure rate;
   - N runs.

2. Degradation curves:
   - packet loss;
   - latency;
   - number of agents;
   - map size;
   - task density;
   - urban obstacle density;
   - blocked-edge frequency;
   - bus detection probability;
   - failure count.

3. Benchmark support matrix:
   - `supported`;
   - `experimental`;
   - `supported_with_caveats`;
   - `unsupported`;
   - `not_evaluated`;
   - `known_bug`.

4. Urban benchmark scope:
   - explicitly choose one:
     - include Urban in `--mission all`;
     - add explicit `--mission urban`;
     - keep Urban as scenario-suite evidence with documented reason.

5. Current vs historical artifacts:
   - classify artifacts by commit and schema version;
   - stale packs are not presented as current evidence;
   - benchmark README explains date, code commit, scenario generator version
     and limitations.

6. Interpretation:
   - SAR success semantics;
   - wildfire success vs completion;
   - emergency-mesh oracle/centralized caveats if relevant;
   - CBBA weak rows;
   - Urban wait/replan tradeoffs;
   - moving bus and perimeter patrol evidence boundaries.

### Non-Goals

- No publication paper unless explicitly chosen.
- No 1000-seed rerun by default if current evidence is enough.
- No unsupported pair success claims.
- No hardware evidence claim.

### Done Criteria

- At least one degradation sweep artifact exists.
- Reports include statistical uncertainty for key metrics.
- Support matrix is visible in benchmark/report docs.
- Urban benchmark scope is explicit.
- Current/historical distinction is documented or machine-checkable.

### Automated Tests

#### Tests That Need No Refactoring

- `confidence_interval_helper_matches_known_values`.
- `benchmark_export_includes_statistical_fields`.
- `support_matrix_marks_unsupported_pair`.
- `unsupported_pair_not_counted_as_success_claim`.
- `manifest_records_seed_range_and_profile`.
- `benchmark_readme_contains_scope_and_commit`.

#### Tests That Need Light Refactoring

- Benchmark pack validation helper.
- Multi-pack comparison helper.
- Degradation suite runner helper.
- Summary table consistency assertions.

#### Tests That Need Heavy Refactoring

- Statistical delta validation.
- Historical artifact database.
- Long-run reproducibility harness.
- Cross-version benchmark pack compatibility checks.

---

## M78 - Operational Runbooks + Hardware Entry Gate

### Goal

Prepare the human and procedural side of a future hardware experiment. Hardware
readiness is not only code. Operators need repeatable procedures, abort rules,
artifact expectations and conservative go/no-go gates.

### Scope

1. Runbooks:
   - simulation runbook;
   - Urban scenario runbook;
   - SITL dry-run/export runbook;
   - local PX4/SIH manual runbook;
   - failure recovery runbook;
   - artifact validation runbook;
   - future hardware candidate runbook.

2. Pre-run checklist:
   - mission file validated;
   - safety report passed;
   - artifact output dir unique;
   - `geo_origin` matches intended location;
   - geofence/no-fly assumptions recorded;
   - expected failure behavior recorded;
   - manual override assumption recorded;
   - operator has abort procedure.

3. Go/no-go gates:
   - no hardware if simulation fails;
   - no hardware if dry-run fails;
   - no hardware if artifact validator fails;
   - no hardware if mission has unclassified safety violations;
   - no hardware without external safety process separate from this project;
   - no multi-drone hardware without separate safety review after single-drone.

4. Post-run inspection:
   - validate artifacts;
   - inspect replay timeline;
   - compare run report and event log;
   - record known limitations;
   - decide whether rerun is allowed.

5. Error handling:
   - structured CLI errors;
   - stable exit codes: 0 ok, 2 validation, 3 runtime, 4 artifact, 5 environment;
   - actionable messages for missing PX4, bad scenario, unsafe mission and
     artifact mismatch.

6. API/platform boundary:
   - external-style mission example;
   - schema compatibility smoke;
   - report/replay schema policy;
   - no public semver promise unless explicitly chosen later.

7. Documentation:
   - update `docs/HARDWARE_READINESS.md`;
   - state what is ready for simulation;
   - state what is ready for local SITL;
   - state what is not ready for hardware;
   - include required boundary phrases:
     - "first hardware experiment is still not product readiness";
     - "multi-agent hardware requires separate safety review";
     - "no regulatory or certified safety claim".

### Non-Goals

- No real hardware checklist pretending to be complete without hardware.
- No legal/regulatory certification.
- No public product-readiness claim.
- No semver commitment unless a separate API branch is selected.

### Done Criteria

- A new developer can run simulation and local SITL dry-run from docs.
- Artifacts can be validated after a run.
- Error messages are actionable.
- Go/no-go gates are explicit.
- Hardware boundary remains conservative.

### Automated Tests

#### Tests That Need No Refactoring

- `docs_smoke_hardware_readiness_not_product`.
- `docs_smoke_multi_agent_requires_separate_review`.
- `docs_smoke_no_regulatory_or_certified_claim`.
- `docs_smoke_required_runbook_sections_exist`.
- `cli_error_missing_scenario_file_has_env_exit`.
- `cli_error_unsafe_mission_has_validation_exit`.
- `schema_compatibility_smoke_existing_fixtures`.

#### Tests That Need Light Refactoring

- Shared docs phrase assertion helper.
- Shared CLI error assertion helper.
- Runbook command fixture validation.
- External-style mission fixture.

#### Tests That Need Heavy Refactoring

- End-to-end scripted dry-run following the runbook.
- Artifact validator integration over runbook-generated output.
- Manual/ignored local PX4/SIH rehearsal.
- Versioned schema migration tests.

---

## Practical Priority

If only a few milestones can be done soon:

1. M70 - Urban Route Export + Preflight Safety Gate.
2. M71 - Artifact Validator + Local SITL Harness.
3. M72 - Fault Injection + Degraded-Mode Supervisor.
4. M73 - Urban Blocked-Route Decision Logic.
5. M75 - Synthetic Scenario Testbed.

M74 is useful and visible, but should not outrank safety/evidence/fault
discipline. M76 becomes most valuable after M70-M75 create better measurement
targets. M77 should not be only a cosmetic benchmark rewrite; it should measure
new behavior. M78 should be updated continuously, but finalized after M70-M72
exist.

## Expected Level After M70-M78

After this plan, without hardware, the project would be:

```text
hardware-integration candidate / hardware-ready research platform
```

It still would not be:

- a production drone system;
- a certified safety stack;
- a real perception system;
- a hardware-proven swarm controller;
- ready for uncontrolled field use.

When hardware appears, the next stage must be a separate plan:

```text
bench without propellers
  -> MAVLink connectivity verification
    -> mission upload only, no execute
      -> telemetry mapping
        -> abort/failsafe validation
          -> single-drone constrained flight
            -> multi-drone only after separate safety review
```

The value of these pre-hardware milestones is that the later stage starts from
a controlled, evidence-backed foundation instead of improvised scripts and
unclear claims.

## Things Not To Do In This Plan

- UI/visualizer as a readiness substitute.
- Hierarchical coordination for 8+ agents without benchmark evidence that it is
  needed.
- Polygon geometry/lidar raycast as a mainline dependency.
- Published API/semver before external users or explicit API branch.
- Logistics/delivery mission before precedence constraints in allocator are
  actually supported.
- 1000-seed rerun before new behavior or new interpretation questions exist.
- Hardware-specific implementation beyond conservative boundary guards.
- Real HIL/lidar/CV/SLAM before hardware exists.
