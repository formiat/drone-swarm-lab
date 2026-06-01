# BEFORE_HARDWARE_A.23 - итоговые майлстоуны до появления железа

Дата фиксации: 2026-06-01

Источник: сравнение `docs_raw/BEFORE_HARDWARE_A.22.md`,
`docs_raw/BEFORE_HARDWARE_B.22.md`, `docs_raw/BEFORE_HARDWARE_C.22.md`,
а также уже существующих `BEFORE_HARDWARE_B.23.md` и
`BEFORE_HARDWARE_C.23.md`.

Этот документ фиксирует итоговый pre-hardware план в нумерации M70-M79. Он не
пытается сделать проект production-ready без железа. Цель другая:

```text
Поднять проект до hardware-integration candidate:
когда железо появится, интеграция начинается с контролируемой,
документированной и протестированной основы, а не с ad-hoc экспериментов.
```

## Короткое сравнение A/B/C

**Лучший backbone: C.22/C.23.** Он правильно расставляет приоритеты для
pre-hardware стадии: execution boundary, safety contract, artifact evidence,
fault injection, затем Urban realism, scenario pressure, algorithms, benchmark
and runbooks.

**Что полезно из A.22:** широкая инфраструктурная рамка: invariant gate,
failure matrix, scenario generation, benchmark evidence layer, operational/API
discipline.

**Что полезно из B.22/B.23:** конкретные технические задачи:

- `geo_origin` в scenario DSL;
- SAR success threshold;
- confidence intervals / stderr;
- CBBA convergence diagnostics;
- communication-aware scoring;
- wildfire priority-triggered reallocation;
- SAR belief/entropy ordering;
- moving bus;
- temporary obstacles;
- perimeter patrol;
- local M58/M59 harness scripts.

**Итоговое решение:** взять C как порядок работ, но оставить нумерацию и
декомпозицию M70-M79 из обсуждения. Benchmark credibility не ставим самым первым
отдельным milestone: сначала надо укрепить execution/safety/evidence boundary,
после чего benchmark становится более содержательным. Но SAR threshold, CI and
CBBA diagnostics не теряются — они входят в M77/M78.

## Архитектурная граница

Проект не должен превращаться в PX4, physics engine или perception stack.

PX4/autopilot owns:

- stabilization;
- attitude/rate control;
- motor physics;
- low-level waypoint following;
- flight failsafes.

This project owns:

- mission-level planning;
- route export;
- task allocation/reallocation;
- preflight validation;
- simulation-level invariants;
- replay and metrics;
- benchmark evidence;
- local SITL workflow;
- operator run discipline.

## Target State Before Hardware

Перед первым controlled hardware experiment проект должен иметь:

- Urban route export to SITL/PX4 waypoint mission;
- configurable coordinate origin instead of hidden hardcoded origin;
- strict preflight safety/invariant gate;
- artifact validator for serious simulation/SITL runs;
- deterministic fault-injection matrix;
- mission-level blocked-route decision logic;
- useful mission realism without fake physics: moving bus, perimeter patrol,
  temporary blocked edges;
- seeded scenario generator for stress/degradation testing;
- algorithm credibility work with interpretable benchmark deltas;
- benchmark/degradation evidence with uncertainty;
- operational runbooks and explicit hardware go/no-go gates.

Это всё ещё не делает проект production-ready. Это делает его disciplined
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
M70 Urban Route Export + Geo Origin
  -> M71 Preflight Safety And Invariant Contract
    -> M72 Artifact Validator + SITL Harness
      -> M73 Fault Injection And Degraded Supervisor
        -> M74 Urban Blocked-Route Decision Logic
          -> M75 Urban Mission Realism Follow-up
            -> M76 Synthetic Scenario Testbed
              -> M77 Algorithm Differentiation
                -> M78 Benchmark Evidence Layer
                  -> M79 Operational Runbooks And Hardware Entry Gate
```

Почему такой порядок:

1. Сначала связываем Urban routes с waypoint/SITL boundary.
2. Сразу ставим safety/invariant gate перед любым execution/export.
3. Потом делаем artifacts machine-checkable.
4. Потом систематически ломаем supervisor через fault injection.
5. Потом добавляем более реалистичное Urban decision behavior.
6. Потом создаём генераторы стресс-сценариев.
7. Потом углубляем алгоритмы и измеряем delta.
8. Потом обновляем benchmark evidence.
9. В конце фиксируем operational runbooks and hardware entry gates.

---

## M70 - Urban Route Export + Geo Origin

### Goal

Соединить существующий Urban simulation layer с PX4/SIH waypoint workflow.

```text
Urban planned route -> ordered waypoint mission -> dry-run/SITL-compatible plan
```

Это не hardware execution. Это deterministic conversion and validation path,
который позже можно использовать в SITL и hardware-adjacent экспериментах.

### Scope

1. Route-to-waypoint conversion:
   - convert `UrbanPlannedRoute` segments into ordered waypoint items;
   - preserve node/edge/task/segment identity where practical;
   - keep deterministic ordering;
   - explicitly include altitude/default mission parameters;
   - define waypoint spacing for long Urban edges;
   - record route length, segment count and waypoint count.

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
   - Add dry-run fixture proving that a non-default origin changes global
     lat/lon without changing local route geometry.

3. Dry-run export artifact:
   - source scenario path;
   - planner/adapter name;
   - route length;
   - waypoint count;
   - start/end waypoint summary;
   - altitude and `geo_origin`;
   - run id, command, config snapshot and git commit where practical.

4. Integration with existing SITL planning:
   - exported mission must fit current SITL waypoint plan shape;
   - no new MAVLink protocol work unless the existing waypoint path cannot
     represent the route;
   - upload-only local PX4/SIH artifact is optional and manual.

5. Documentation:
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
- Export preserves stable segment/task identity where practical.
- Exported route has explicit altitude and coordinate origin.
- `geo_origin` can override the default origin without PX4.
- SITL scenarios expose their origin in scenario data.
- Dry-run artifact is readable and reproducible.
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
- `geo_origin_absent_uses_sitl_default`: old behavior remains default.

#### Tests That Need Light Refactoring

- Shared route-to-waypoint helper fixture.
- Export metadata assertion helper.
- SITL plan assertion helper.

#### Tests That Need Heavy Refactoring

- Manual/ignored local PX4/SIH upload test for exported Urban route.
- Cross-run export artifact comparison tool.
- Larger route densification property tests.

---

## M71 - Preflight Safety And Invariant Contract

### Goal

Make unsafe mission inputs fail before dry-run, SITL upload or future hardware
bench attempts.

This is more important than adding new mission features. It creates a strict
contract for what the project is willing to execute/export.

### Scope

1. Mission-level safety checks:
   - geofence bounds;
   - no-fly zone/static obstacle intersection;
   - max altitude;
   - min altitude if relevant;
   - max route length;
   - estimated mission duration if practical;
   - invalid or non-finite coordinates;
   - missing waypoint/task/segment ids.

2. Ownership invariants:
   - no duplicate task ownership;
   - no duplicate route segment ownership where exclusive ownership is required;
   - released tasks must be either reassigned, completed or explicitly abandoned;
   - replacement mission ownership must not duplicate old ownership.

3. Urban-specific safety checks:
   - route uses known graph edges;
   - route avoids blocked edges unless policy explicitly allows wait/replan
     later;
   - route avoids static AABB obstacles;
   - route planner and export metadata agree;
   - exported waypoint route stays inside declared Urban assumptions.

4. Mission semantics invariants:
   - completion requires the documented predicate;
   - unsupported strategy/mission pairs cannot silently claim supported success;
   - SAR/wildfire success semantics remain documented separately from task
     completion;
   - no route/export success if preflight failed.

5. `SafetyValidationReport` shape:
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

6. CLI behavior:
   - unsafe mission exits non-zero;
   - error message names failed rule ids;
   - output artifact records safety result if `--output-dir` was requested;
   - stable exit code convention starts here:
     - validation error: `2`;
     - runtime/supervisor error: `3`;
     - artifact/report error: `4`;
     - environment error: `5`.

7. Documentation:
   - list each preflight rule;
   - list what is not checked;
   - clearly state this is not certified flight safety.

### Non-Goals

- No certified safety case.
- No real obstacle avoidance.
- No runtime hardware failsafe implementation.
- No regulatory claim.

### Done Criteria

- Safety failures are structured and assertable in tests.
- Exported Urban routes use the same preflight contract.
- Unsafe dry-run fixtures fail deterministically.
- Existing valid scenarios remain green.
- Docs list each preflight rule and limitation.
- Project language remains conservative: simulation/preflight gate, not real
  flight guarantee.

### Automated Tests

#### Tests That Need No Refactoring

- Geofence violation fixture.
- No-fly/AABB violation fixture.
- Duplicate ownership rejection fixture.
- Non-finite coordinate rejection fixture.
- Unsupported pair cannot be marked as supported.
- Completion-without-predicate rejection.
- Unsafe exported Urban route fails before dry-run success.

#### Tests That Need Light Refactoring

- Shared `SafetyValidationReport` assertion helper.
- Small fixture builder for valid/invalid route plans.
- CLI output helper for rule-id assertions.
- Shared invariant assertion helper.

#### Tests That Need Heavy Refactoring

- Property tests for generated waypoints vs geofence/no-fly rules.
- Cross-mission preflight compatibility suite.
- Battery reserve estimator tests with mission-duration model.
- Versioned safety report compatibility tests.

---

## M72 - Artifact Validator + SITL Harness

### Goal

Make serious runs machine-checkable and repeatable.

Future hardware work will depend on artifacts more than informal console output.
M72 defines an evidence contract for simulation, dry-run and local PX4/SIH runs.

### Scope

1. Artifact validator inputs:
   - manifest;
   - run report;
   - event log;
   - replay summary;
   - scenario snapshot if present;
   - benchmark/result table where relevant.

2. Artifact validator checks:
   - manifest command/git commit/build profile present;
   - run id and output dir consistent;
   - event log final status matches run report final status;
   - completed tasks in report exist in event log;
   - mission replacement completion seq uses active mission seq;
   - replay summary counts are consistent with event log;
   - safety report exists where required;
   - no accidental overwrite unless `--force` was used;
   - limitations section exists for SITL/PX4 artifacts.

3. CLI/tooling:
   - small validator command or library function;
   - readable error list;
   - stable rule ids;
   - exit code `0` for valid artifact, non-zero for invalid artifact;
   - portable tests using inline/temp fixtures.

4. Local harness scripts:
   - `scripts/run_m58_local.sh` or equivalent:
     - starts two PX4/SIH instances if configured;
     - waits for MAVLink endpoints;
     - runs supervisor;
     - collects logs;
     - stops/cleans processes;
     - writes artifacts to deterministic output dir.
   - `scripts/run_m59_local.sh` or equivalent:
     - same baseline flow;
     - injects controlled first-agent loss;
     - validates reallocation artifact.

5. Clear manual-only boundary:
   - scripts are not default CI;
   - docs state local assumptions;
   - missing PX4 produces clear failure message;
   - artifact validator can run without PX4 on committed fixtures.

6. Result discipline:
   - output directory naming;
   - `--run-id`;
   - `--force` semantics;
   - command line capture;
   - config snapshot.

### Non-Goals

- No CI-managed PX4 container unless explicitly planned later.
- No hardware.
- No broad PX4 version certification.
- No remote artifact store.

### Done Criteria

- Valid tiny artifact fixture passes validator.
- Deliberately inconsistent fixture fails with clear rule ids.
- M58/M59-style reports are covered by validator logic or compatibility tests.
- Local SITL harness exists and is documented as manual-only.
- A developer can rerun M58-like/M59-like workflows from docs when PX4/SIH is
  installed.
- Failed local setup produces actionable error messages.

### Automated Tests

#### Tests That Need No Refactoring

- Valid tiny artifact fixture passes.
- Missing manifest field fails.
- Final-status mismatch fails.
- Event-log/report task mismatch fails.
- Replay summary count mismatch fails.
- Replacement mission seq mismatch fails.

#### Tests That Need Light Refactoring

- Shared artifact fixture builder.
- Validator rule-id assertion helper.
- Event-log/report consistency helper.
- Harness dry-run mode that does not launch PX4.

#### Tests That Need Heavy Refactoring

- Validator over full committed M58/M59 artifacts.
- Multi-artifact pack validator for benchmark directories.
- Schema-version compatibility matrix.
- Ignored/manual two-PX4 harness integration test.

---

## M73 - Fault Injection And Degraded Supervisor

### Goal

Exercise failure behavior before hardware exists.

Real systems fail. M73 makes supervisor behavior explicit under degraded
conditions:

```text
detect -> classify -> decide -> recover/abort -> report
```

### Scope

1. Failure modes:
   - agent lost before upload;
   - upload rejected;
   - agent lost after upload before mission start;
   - no-progress timeout;
   - heartbeat lost;
   - stale telemetry;
   - partial completion then failure;
   - replacement mission rejected;
   - survivor fails after replacement;
   - unsafe replacement route;
   - bad waypoint/mission item.

2. Supervisor decisions:
   - abort;
   - wait;
   - reassign unfinished tasks;
   - release tasks to pool;
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
   - replacement uploaded;
   - recovery completed/failed;
   - final status.

5. Mock/fake tests:
   - deterministic controller failures;
   - deterministic timeout;
   - deterministic partial completion;
   - deterministic mission rejection.

6. Local SITL/manual checks where practical:
   - one or two representative failure paths;
   - no default CI dependency;
   - artifacts validated by M72 validator.

7. Documentation:
   - failure matrix table;
   - supported / experimental / not-tested status;
   - exact recovery semantics.

### Non-Goals

- No hardware failure testing.
- No exhaustive physical failsafe validation.
- No real RF/link-loss modeling beyond deterministic simulation profiles.
- No repeated-failure policy beyond a bounded first implementation.

### Done Criteria

- Each supported failure mode has a fake-controller test.
- Supervisor final status is deterministic and explainable.
- Recovered task ownership is valid.
- Artifact validator can verify degraded-mode runs.
- Failure matrix exists in docs.
- Known unsupported paths are explicitly labeled.

### Automated Tests

#### Tests That Need No Refactoring

- Fake controller upload rejection.
- Fake controller no-progress timeout.
- Fake controller heartbeat/lost-agent recovery.
- Fake controller partial completion then disconnect.
- Replacement mission rejected.
- Survivor completes recovered tasks.
- Failure metrics aggregation test.

#### Tests That Need Light Refactoring

- Fake controller scenario builder.
- Failure-mode assertion helper.
- Shared final-status validation helper.
- Artifact validator integration with failure reports.

#### Tests That Need Heavy Refactoring

- Manual/ignored local PX4/SIH fault-injection harness.
- Repeated failure property tests.
- Long-running supervisor soak test with synthetic failures.
- Stochastic communication failure sweeps.

---

## M74 - Urban Blocked-Route Decision Logic

### Goal

Add mission-level reactivity without pretending to implement real obstacle
avoidance.

```text
edge becomes blocked -> detector/policy notices -> wait or replan -> judge/report
```

### Scope

1. Dynamic blocked route state:
   ```rust
   pub struct UrbanTemporaryObstacle {
       pub edge_id: UrbanEdgeId,
       pub appears_at_tick: u64,
       pub disappears_at_tick: Option<u64>,
       pub reason: Option<String>,
       pub severity: Option<String>,
   }
   ```

2. Effective blocked set:
   - static blocked edges from map;
   - temporary blocked edges active at current tick;
   - no hidden mutation of original map;
   - deterministic active/inactive transition.

3. Mock obstacle detector:
   - graph lookahead;
   - deterministic result;
   - no real lidar/raycast;
   - optional detection range in graph distance or meters.

4. Policies:
   - wait until unblocked;
   - replan around blocked edge;
   - abort if no route exists;
   - yield if another agent has priority, but only if a simple deterministic
     rule is ready;
   - refuse unsafe replacement route via M71 gate.

5. Replay events:
   - `UrbanEdgeBlocked`;
   - `UrbanEdgeUnblocked`;
   - `UrbanObstacleDetected`;
   - `UrbanPolicyDecision`;
   - `UrbanRouteReplanned`;
   - `UrbanWaitStarted`;
   - `UrbanWaitCompleted`;
   - `UrbanNoRouteAvailable`.

6. Metrics:
   - `urban_replan_count`;
   - `wait_time_ticks`;
   - `blocked_edge_count`;
   - `replan_success_rate`;
   - `unresolved_blockage_count`;
   - `near_miss_count` only if a precise definition exists.

7. Judge/report:
   - blocked edge violation if agent tries to traverse active blocked edge;
   - wait/replan explanation in route trace;
   - no-route failure is explicit, not silent timeout.

### Non-Goals

- No certified obstacle avoidance.
- No real sensor stream.
- No physics.
- No arbitrary polygon geometry unless needed for a tiny helper.
- No multi-agent yield complexity unless the single-agent blocked-route policy
  is stable.

### Done Criteria

- One deterministic blocked-edge scenario recovers by wait or replan.
- One deterministic no-route scenario fails safely.
- Replay explains the decision.
- Metrics distinguish route following from replan/wait behavior.
- Blocked-edge policy uses safety gate before accepting replacement route.

### Automated Tests

#### Tests That Need No Refactoring

- `temporary_obstacle_is_active_within_window`.
- `temporary_obstacle_no_disappears_stays_forever`.
- Blocked edge before arrival triggers selected policy.
- Wait policy completes after edge unblocks.
- Replan policy chooses alternate route.
- No alternate route fails with explicit reason.
- Replay contains blocked-edge and policy-decision events.

#### Tests That Need Light Refactoring

- Blocked-edge scenario builder.
- Route policy assertion helper.
- Urban replay event fixture helper.
- Effective blocked-set helper.

#### Tests That Need Heavy Refactoring

- Multi-agent yield policy tests.
- Dynamic obstacle schedule property tests.
- Larger generated-map stress tests.

---

## M75 - Urban Mission Realism Follow-up

### Goal

Make Urban missions closer to practical mission logic without adding real
physics, real sensors or hardware dependencies.

M75 is the "useful realism" layer: moving semantic targets, perimeter patrol,
and route patterns that map cleanly to SITL waypoints.

### Scope

1. Dynamic bus route:
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
       // existing static pose remains for backward compatibility
   }
   ```

2. `UrbanBus::pose_at_tick(map, tick)`:
   - without route: return static pose inside active window;
   - with route: interpolate between scheduled stops;
   - return `None` outside active route window;
   - deterministic and portable.

3. Moving bus detection:
   - detector samples bus pose at tick;
   - replay records observed bus pose;
   - false positive semantics remain separate from real detection;
   - existing static bus scenario remains valid.

4. Perimeter Patrol:
   - polygon/perimeter input;
   - waypoint spacing rule;
   - deterministic `perimeter_waypoints(polygon, spacing)`;
   - closed route;
   - optional export compatibility with M70;
   - metrics:
     - `perimeter_completion_rate`;
     - `perimeter_length_m`;
     - `time_to_complete_perimeter`;
     - `perimeter_violations`.

5. Scenario fixtures:
   - moving bus small fixture;
   - static bus backward-compat fixture;
   - square perimeter patrol fixture;
   - optional dry-run export fixture if M70 is stable.

6. Documentation:
   - moving bus is a semantic target, not a physical vehicle model;
   - detector is mocked;
   - perimeter patrol is waypoint mission realism, not field readiness.

### Non-Goals

- No real CV.
- No image simulation.
- No physical bus model beyond graph pose over time.
- No pursuit/intercept.
- No real lidar/raycast.
- No arbitrary polygon geometry beyond perimeter waypoint generation.

### Done Criteria

- `UrbanBus::pose_at_tick` returns interpolated pose for moving bus.
- Static bus behavior remains backward-compatible.
- Urban Search detects moving bus only when timing/range matches.
- Perimeter patrol generates deterministic waypoints.
- Perimeter patrol completes on simple square fixture.
- All new scenarios are portable and deterministic.

### Automated Tests

#### Tests That Need No Refactoring

- `bus_pose_at_tick_static_returns_fixed_pose`.
- `bus_pose_at_tick_interpolates_between_stops`.
- `bus_pose_at_tick_returns_none_outside_window`.
- `detect_buses_finds_moving_bus_when_in_range`.
- `detect_buses_misses_moving_bus_out_of_range`.
- `perimeter_waypoints_square_correct_count`.
- `perimeter_waypoints_is_deterministic`.
- `perimeter_waypoints_closed_route`.
- `perimeter_patrol_completes_on_square`.

#### Tests That Need Light Refactoring

- Bus route fixture builder.
- Moving-target detector fixture.
- Shared perimeter builder for convex polygon.
- Metrics assertion helper for perimeter completion.

#### Tests That Need Heavy Refactoring

- Detection probability multi-seed stability tests.
- Line-of-sight building occlusion if later added.
- Property test: convex polygon waypoints lie on perimeter.
- Multi-agent perimeter partition tests.

---

## M76 - Synthetic Scenario Testbed

### Goal

Avoid overfitting to a few hand-written scenarios. Build deterministic scenario
families for stress and degradation tests.

This is support infrastructure, not a new mission family.

### Scope

1. Scenario generator API:
   - deterministic seed;
   - explicit generator parameters;
   - stable scenario names;
   - manifest records generator settings;
   - scenario schema version.

2. Seeded Urban generator:
   - grid/block road graph;
   - corridor widths;
   - static obstacle density;
   - blocked edge schedule;
   - bus placement or route;
   - optional perimeter shape.

3. Failure generator:
   - agent failure tick;
   - failure type;
   - partial completion amount;
   - replacement acceptance/rejection.

4. Communication generator:
   - packet loss;
   - latency;
   - partitions;
   - agent count.

5. Scenario library categories:
   - tiny;
   - small;
   - medium;
   - stress;
   - regression-stable;
   - experimental.

6. Test usage:
   - small deterministic generated fixtures in unit tests;
   - no dependency on local absolute paths;
   - generated data kept small in CI;
   - large generated suites remain explicit/manual.

7. Documentation:
   - how to regenerate;
   - which profiles are default regression;
   - which profiles are exploratory;
   - no claim that generated scenarios match real-world distributions.

### Non-Goals

- No large random test in default CI.
- No opaque random failures.
- No generated scenario without reproducible seed/manifest.
- No claim that generated scenarios are real-world statistically representative.

### Done Criteria

- A seeded generator creates the same scenario on repeated runs.
- Different seed changes at least one expected field.
- Generated scenario passes DSL validation.
- At least one generated Urban blocked-edge fixture feeds M74 tests.
- Generator parameters are recorded in manifest.

### Automated Tests

#### Tests That Need No Refactoring

- Same seed yields identical scenario.
- Different seed changes at least one expected field.
- Generated Urban map validates.
- Generated blocked-edge schedule validates.
- Invalid generator config rejected.

#### Tests That Need Light Refactoring

- Scenario generator trait/helper.
- Manifest assertion helper.
- Small generated-fixture snapshot test.
- DSL validation helper for generated scenarios.

#### Tests That Need Heavy Refactoring

- Property tests over many generated maps.
- Cross-mission generated scenario framework.
- Long-run generated degradation suite.
- Cross-version generator reproducibility tests.

---

## M77 - Algorithm Differentiation

### Goal

Make strategies measurably different in the conditions where they should differ.

Current benchmark evidence is useful, but many strategies still look too similar
in broad aggregate results. M77 adds targeted algorithm changes and diagnostic
hooks so benchmark deltas become explainable.

### Scope

1. Communication-aware allocation scoring:
   - add `comms_penalty_weight` or equivalent config;
   - use `AllocationAgent.comms_range` in greedy/auction where appropriate;
   - preserve old behavior when weight is `0.0`;
   - compare coverage heavy-loss and partition-prone profiles.

2. Wildfire priority-triggered reallocation:
   - add `wildfire_priority_realloc_threshold`;
   - when priority update crosses threshold, enqueue force-reallocation;
   - release/reassign high-priority task deterministically;
   - replay/report distinguishes priority update from ordinary assignment.

3. SAR belief/entropy ordering:
   - add `dynamic_belief_updates: bool`;
   - update posterior after scan events;
   - rank unfinished tasks by remaining uncertainty when enabled;
   - keep static behavior when disabled.

4. CBBA convergence diagnostics:
   - include conflict count in relevant CBBA replay events;
   - run focused heavy-loss diagnostics;
   - test hypothesis that agent failure needs gossip burst;
   - add gossip burst only if replay evidence supports it;
   - otherwise document limitation in support matrix.

5. Benchmark delta:
   - run targeted small/medium comparisons, not a full 1000-seed run;
   - document where behavior changed and why;
   - update support matrix status for affected pairs.

### Non-Goals

- No hierarchical coordination unless scale evidence later justifies it.
- No algorithm rewrite before targeted evidence.
- No large publication run inside M77.
- No unsupported pair success claims.

### Done Criteria

- `comms_penalty_weight` changes allocation in controlled tests.
- Wildfire priority update can trigger deterministic reallocation.
- SAR dynamic belief mode changes ordering in controlled tests.
- CBBA convergence gap is diagnosed or clearly documented.
- At least one targeted benchmark delta is committed with interpretation.
- Existing stable behavior remains default when new flags are off.

### Automated Tests

#### Tests That Need No Refactoring

- `comms_penalty_zero_no_effect`.
- `comms_penalty_reduces_score_beyond_range`.
- `comms_penalty_infinite_range_no_effect`.
- `wildfire_priority_trigger_reallocates_agent`.
- `wildfire_priority_below_threshold_no_realloc`.
- `sar_dynamic_belief_updates_change_task_order`.
- `sar_static_belief_unchanged_with_flag_false`.
- `cbba_bundle_updated_has_conflict_count`.

#### Tests That Need Light Refactoring

- Scoring comparison helper.
- Wildfire priority update fixture.
- SAR belief-grid fixture.
- CBBA convergence replay fixture.
- Targeted benchmark delta helper.

#### Tests That Need Heavy Refactoring

- Property tests for CBBA bundle consistency under message loss.
- Multi-seed benchmark delta suite.
- Scale experiments with 8/16 agents.
- Hierarchical coordination integration tests if later chosen.

---

## M78 - Benchmark Evidence Layer

### Goal

Turn "we ran many seeds" into interpretable evidence.

M69 already produced a useful 1000-seed artifact. M78 improves the reporting and
interpretation layer instead of blindly rerunning long benchmarks.

### Scope

1. Statistical summary:
   - mean;
   - stddev;
   - stderr;
   - confidence interval;
   - min/max;
   - failure rate.

2. SAR benchmark credibility:
   - add `sar_success_threshold` if not already done in M77;
   - distinguish probability-of-detection success from "all targets found";
   - document threshold in scenarios and benchmark results.

3. Degradation curves:
   - packet loss;
   - latency;
   - agent count;
   - route length;
   - obstacle density;
   - blocked-edge frequency;
   - bus detection probability;
   - failure count.

4. Benchmark support matrix:
   - supported;
   - experimental;
   - unsupported;
   - known bug;
   - not evaluated;
   - supported with caveats where needed.

5. Current vs historical artifacts:
   - classify artifact by code commit and schema version;
   - docs do not present stale packs as current evidence;
   - benchmark result README explains scope.

6. Urban benchmark decision:
   - add Urban to `--mission all`; or
   - create explicit `--mission urban`; or
   - keep Urban as scenario-suite evidence with documented reason.

7. Report interpretation:
   - SAR success semantics;
   - wildfire success vs completion;
   - emergency-mesh oracle/centralized caveats;
   - CBBA weak rows;
   - Urban route-risk/replan tradeoffs.

### Non-Goals

- No publication paper unless explicitly chosen.
- No 1000-seed rerun by default if existing evidence is enough.
- No long run before interpretation questions are defined.
- No unsupported pair as success claim.
- No hardware evidence claim.

### Done Criteria

- Reports include statistical fields for key metrics.
- At least one degradation sweep exists as artifact.
- Unsupported rows are clearly marked.
- Urban benchmark scope is explicit.
- Current/historical distinction is machine-checkable or documented in artifact
  metadata.
- Docs distinguish simulation, SITL and future hardware evidence.

### Automated Tests

#### Tests That Need No Refactoring

- Confidence interval helper test.
- Aggregate stderr formula test.
- Report export includes statistical fields.
- Unsupported pair remains excluded or marked.
- Manifest records seed range and generator profile.
- SAR threshold test for partial detection if implemented here.

#### Tests That Need Light Refactoring

- Benchmark pack validator helper.
- Degradation suite runner helper.
- Summary table consistency assertions.
- Multi-pack comparison helper.

#### Tests That Need Heavy Refactoring

- Statistical delta report validation.
- Historical artifact database.
- Long-run reproducibility harness.
- Full generated degradation suite.

---

## M79 - Operational Runbooks And Hardware Entry Gate

### Goal

Prepare the human and procedural side of a future hardware experiment.

Hardware readiness is not just code. Before hardware exists, the project can
define exact go/no-go criteria, command sequences, artifact expectations and
failure handling procedure.

### Scope

1. Runbooks:
   - simulation runbook;
   - Urban scenario runbook;
   - SITL dry-run/export runbook;
   - local PX4/SIH runbook;
   - artifact validation runbook;
   - future hardware candidate runbook.

2. Preflight checklist:
   - mission file validated;
   - safety report passed;
   - artifact output dir unique;
   - `geo_origin` and frame assumptions recorded;
   - geofence/no-fly assumptions recorded;
   - expected failure behavior recorded;
   - manual override assumption recorded.

3. Go/no-go gates:
   - no hardware if simulation fails;
   - no hardware if export/dry-run fails;
   - no hardware if preflight safety fails;
   - no hardware if artifact validator fails;
   - no hardware if mission has unclassified safety violations;
   - no hardware without external safety process;
   - no multi-drone hardware before separate single-drone review.

4. Post-run inspection:
   - validate artifacts;
   - inspect replay timeline;
   - compare run report and event log;
   - record known limitations;
   - decide whether rerun is allowed.

5. API/platform boundary:
   - external-style mission example;
   - schema compatibility smoke;
   - report/replay schema policy;
   - no public semver promise unless explicitly chosen.

6. Documentation updates:
   - `docs/HARDWARE_READINESS.md`;
   - `docs/SITL_SETUP.md`;
   - `docs/STATUS.md`;
   - README status wording if necessary;
   - "first hardware experiment is still not product readiness";
   - single-drone controlled test is separate from multi-agent hardware.

### Non-Goals

- No real hardware checklist pretending to be complete without hardware.
- No legal/regulatory certification.
- No public product-readiness claim.
- No semver commitment unless API branch is selected.
- No UI/visualizer as readiness requirement.

### Done Criteria

- Runbooks exist and reference actual commands.
- Go/no-go gates are explicit.
- Hardware boundary remains conservative.
- Artifact validation procedure is part of runbooks.
- Command examples match real binaries/options.
- Future hardware entry has an explicit preflight path.
- Docs say project is hardware-integration candidate, not production product.

### Automated Tests

#### Tests That Need No Refactoring

- Docs smoke test for required runbook sections.
- Docs smoke test for "not hardware-ready" boundary language.
- Command examples reference existing binaries/options.
- Schema compatibility smoke for existing fixtures.
- CLI error tests for missing/invalid arguments.

#### Tests That Need Light Refactoring

- Shared docs assertion helper for safety boundary language.
- Runbook command fixture validation.
- Shared CLI assertion helper.
- External-style extension fixture.

#### Tests That Need Heavy Refactoring

- End-to-end scripted dry-run following the runbook.
- Artifact validator integration over runbook-generated output.
- Manual/ignored local PX4/SIH rehearsal.
- Public API compatibility checks if API branch is selected.

---

## Expected Level After M70-M79

After this plan, without hardware, the project still would not be:

- a production drone system;
- a certified safety stack;
- a real perception system;
- a hardware-proven swarm controller;
- ready for uncontrolled field use.

But it would be much closer to a controlled hardware experiment:

- Urban routes exportable to waypoint workflows;
- mission inputs validated before execution;
- coordinate frame explicit via `geo_origin`;
- failure behavior tested in fake/local modes;
- artifacts machine-checkable;
- scenario stress tests reproducible;
- algorithm deltas explainable;
- benchmark/degradation evidence interpretable;
- runbooks define exactly when not to proceed.

Correct target level:

```text
hardware-ready research platform / hardware-integration candidate
```

When hardware appears, the next stage should be a separate plan:

```text
bench without propellers
  -> MAVLink connectivity
    -> mission upload only
      -> telemetry mapping
        -> abort/failsafe validation
          -> single-drone constrained flight
            -> multi-drone only after separate safety review
```

The value of M70-M79 is that this later stage begins from a controlled,
evidence-backed foundation instead of improvised scripts and unclear claims.

## Things Not To Do In This Plan

- Do not present simulation/SITL evidence as hardware evidence.
- Do not add real perception/CV/lidar.
- Do not start arbitrary polygon/navmesh work as a main thread.
- Do not make UI/visualization the readiness milestone.
- Do not do repeated 1000-seed runs before new behavior or new interpretation
  justifies them.
- Do not add public semver commitments before API boundaries stabilize.
- Do not move into multi-drone hardware planning before single-drone controlled
  hardware planning exists.
