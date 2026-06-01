# BEFORE_HARDWARE_C.22 - pre-hardware milestones

Дата фиксации: 2026-05-31

Контекст: железа сейчас нет и в ближайшее время не будет. Цель этого плана -
делать работу, которая реально приближает проект к будущему controlled hardware
experiment, но не требует hardware/HIL сейчас и не заявляет hardware readiness.

Главный принцип:

```text
Не строим "виртуальный реальный дрон".
Строим production-grade mission/supervisor layer:
validation, artifacts, fault handling, replay, metrics and run discipline.
```

Проект не должен писать свой PX4, real perception, SLAM, lidar/CV или
certified obstacle avoidance. До появления железа наиболее ценная работа -
сделать так, чтобы будущий запуск на железе был не экспериментом "на авось", а
контролируемым workflow с проверяемыми mission inputs, clear safety boundary and
machine-checkable artifacts.

---

## Target State Before Hardware

Перед первым реальным hardware experiment проект должен иметь:

- Urban route export to waypoint mission;
- strict preflight safety contract;
- artifact validator for every serious run;
- deterministic fault-injection matrix;
- mission-level blocked-route decision logic;
- seeded scenario generator for stress and degradation testing;
- benchmark/degradation evidence with interpretation;
- operational runbooks and explicit go/no-go gates.

Это не делает проект production-ready. Это делает его готовым к первому
осторожному controlled hardware experiment later.

---

## Non-Goals

До появления железа не делать:

- hardware-specific code paths beyond existing boundary guards;
- real HIL;
- real lidar/raycast/SLAM/CV;
- certified obstacle avoidance;
- production safety claims;
- public semver API promise;
- UI/visualizer as the main workstream;
- long 1000-seed reruns without new behavior or new interpretation layer.

---

## Milestone Order

```text
BH1 Urban Route Export To PX4/SITL Waypoints
  -> BH2 Preflight Safety Contract Hardening
    -> BH3 Artifact Validator And Evidence Contract
      -> BH4 Fault Injection And Degraded-Mode Supervisor
        -> BH5 Urban Blocked-Route Decision Logic
          -> BH6 Synthetic Scenario Testbed
            -> BH7 Benchmark And Degradation Evidence
              -> BH8 Operational Runbooks And Hardware Entry Gate
```

BH1-BH4 are the highest priority because they build the launch discipline that
future hardware work will need. BH5-BH7 deepen simulation realism and evidence.
BH8 packages the resulting workflow into a practical go/no-go process.

---

## BH1 - Urban Route Export To PX4/SITL Waypoints

### Goal

Connect the existing Urban simulation layer to the existing PX4/SIH/SITL
waypoint workflow.

Urban currently plans and judges routes inside simulation. BH1 adds an export
boundary:

```text
Urban planned route -> ordered waypoint mission -> dry-run/SITL-compatible plan
```

This is not hardware execution. It is a deterministic conversion and validation
path that can later be used for SITL or hardware-adjacent experiments.

### Scope

1. Route conversion:
   - convert `UrbanPlannedRoute` segments to ordered waypoint items;
   - preserve stable task/segment ids;
   - preserve altitude assumptions explicitly;
   - choose waypoint spacing rule if an Urban segment is longer than a single
     waypoint hop.

2. Export metadata:
   - source scenario path;
   - planner name;
   - route length;
   - waypoint count;
   - altitude;
   - safety validation result;
   - git commit and command-line identity where practical.

3. Dry-run integration:
   - produce a `sitl_supervisor`/`sitl_agent` compatible waypoint plan or config;
   - run existing dry-run path without PX4;
   - write output to an explicit `--output-dir`.

4. Scope docs:
   - local SITL/PX4-compatible export only;
   - no hardware;
   - no real obstacle avoidance;
   - no real perception.

### Non-Goals

- no hardware run;
- no Gazebo/HIL;
- no new PX4 protocol work unless existing waypoint path cannot represent the
  route;
- no arbitrary polygon/navmesh requirement.

### Done Criteria

- Urban patrol route exports to a deterministic waypoint list.
- Exported waypoints pass existing dry-run validation.
- Export artifact includes route/source metadata.
- Docs explain the boundary clearly.

### Automated Tests

#### Tests that need no refactoring

- Unit test: simple square Urban route exports to ordered waypoint items.
- Unit test: waypoint task ids/segment ids are stable across repeated export.
- Unit test: altitude is preserved or defaulted deterministically.
- Dry-run smoke using a committed small Urban route fixture.

#### Tests that need light refactoring

- Shared route-to-waypoint helper fixture.
- Export metadata assertion helper.
- Safety-validation wrapper around exported waypoints.

#### Tests that need heavy refactoring

- Manual/ignored local PX4/SIH upload test for exported Urban route.
- Cross-run export artifact comparison tool.
- Larger route densification property tests.

---

## BH2 - Preflight Safety Contract Hardening

### Goal

Make unsafe mission inputs fail before execution. This is more important for
future hardware than adding new mission features.

The project already has safety validation pieces. BH2 turns them into a clearer
preflight contract for exported and simulated missions.

### Scope

1. Mission-level safety checks:
   - geofence bounds;
   - no-fly zone intersection;
   - max altitude;
   - min altitude if relevant;
   - max route length;
   - max estimated mission duration;
   - minimum battery reserve estimate;
   - duplicate task ownership;
   - missing waypoint/task ids;
   - invalid or non-finite coordinates.

2. Urban-specific safety checks:
   - route uses known graph edges;
   - route avoids blocked edges;
   - route avoids static AABB obstacles;
   - exported waypoint route stays inside the declared Urban assumptions;
   - route planner and export metadata agree.

3. Safety result schema:
   - `passed`;
   - list of violations;
   - severity;
   - rule id;
   - affected task/segment/waypoint id;
   - human-readable reason.

4. CLI behavior:
   - unsafe mission exits non-zero;
   - error message names failed rule ids;
   - output artifact records safety result if `--output-dir` was requested.

### Non-Goals

- no certified safety case;
- no real obstacle avoidance;
- no runtime hardware failsafe implementation;
- no regulatory claim.

### Done Criteria

- Safety failures are structured and assertable in tests.
- Exported Urban routes use the same safety contract.
- Docs list each preflight rule and its limitation.
- Unsafe dry-run fixtures fail deterministically.

### Automated Tests

#### Tests that need no refactoring

- Geofence violation fixture.
- No-fly/AABB violation fixture.
- Duplicate ownership rejection fixture.
- Non-finite coordinate rejection fixture.
- Unsafe exported Urban route fails before dry-run success.

#### Tests that need light refactoring

- Shared `SafetyValidationReport` assertion helper.
- Small fixture builder for valid/invalid route plans.
- CLI output helper for rule-id assertions.

#### Tests that need heavy refactoring

- Property tests for generated waypoints vs geofence/no-fly rules.
- Cross-mission preflight compatibility suite.
- Battery reserve estimator tests with mission-duration model.

---

## BH3 - Artifact Validator And Evidence Contract

### Goal

Make run artifacts trustworthy. Future hardware work will depend on artifacts
more than on informal console output.

BH3 adds a machine-checkable evidence contract for simulation, SITL dry-run and
local PX4/SIH artifacts.

### Scope

1. Validator inputs:
   - manifest;
   - run report;
   - event log;
   - replay summary;
   - benchmark/result table where relevant;
   - scenario snapshot if present.

2. Validator checks:
   - manifest command/git commit/build profile present;
   - run id and output dir consistent;
   - event log final status matches run report final status;
   - completed tasks in report exist in event log;
   - mission replacement completion seq uses active mission seq;
   - replay summary counts are consistent with event log;
   - no accidental overwrite unless `--force` was used;
   - limitations section exists for SITL/PX4 artifacts.

3. CLI/tooling shape:
   - small validator command or library function;
   - readable error list;
   - exit code `0` for valid, non-zero for invalid;
   - portable tests using committed tiny fixtures or inline temp fixtures.

4. Documentation:
   - define what counts as acceptable evidence;
   - distinguish simulation, dry-run, local PX4/SIH and hardware evidence.

### Non-Goals

- no remote artifact store;
- no CI dependency on local PX4;
- no hardware artifact claim.

### Done Criteria

- A committed small artifact fixture validates.
- A deliberately inconsistent fixture fails with clear rule ids.
- M58/M59-style reports are covered by validator logic or compatibility tests.
- Docs describe the evidence contract.

### Automated Tests

#### Tests that need no refactoring

- Valid tiny artifact fixture passes.
- Missing manifest field fails.
- Final-status mismatch fails.
- Event-log/report task mismatch fails.
- Replay summary count mismatch fails.

#### Tests that need light refactoring

- Shared artifact fixture builder.
- Validator rule-id assertion helper.
- Event-log/report consistency helper.

#### Tests that need heavy refactoring

- Validator over full committed M58/M59 artifacts.
- Multi-artifact pack validator for benchmark directories.
- Schema-version compatibility matrix.

---

## BH4 - Fault Injection And Degraded-Mode Supervisor

### Goal

Exercise failure behavior before hardware exists.

Real systems fail. BH4 makes the supervisor behavior explicit under degraded
conditions:

```text
detect -> classify -> decide -> recover/abort -> report
```

### Scope

1. Failure modes:
   - agent lost before upload;
   - upload rejected;
   - agent lost after upload before start;
   - no-progress timeout;
   - heartbeat lost;
   - partial completion then failure;
   - replacement mission rejected;
   - survivor fails after replacement;
   - stale telemetry;
   - bad waypoint/mission item.

2. Supervisor decisions:
   - abort;
   - wait;
   - reassign unfinished tasks;
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
   - final status.

4. Replay events:
   - failure detected;
   - failure classified;
   - recovery started;
   - recovery completed/failed;
   - final status.

### Non-Goals

- no local PX4 process automation required for default tests;
- no hardware failure experiments;
- no repeated-failure policy beyond a bounded first implementation.

### Done Criteria

- Each supported failure mode has a fake-controller test.
- Supervisor final status is deterministic and explainable.
- Recovered task ownership is valid.
- Artifact validator can verify degraded-mode runs.

### Automated Tests

#### Tests that need no refactoring

- Fake controller upload rejection.
- Fake controller no-progress timeout.
- Fake controller partial completion then disconnect.
- Replacement mission rejected.
- Survivor completes recovered tasks.

#### Tests that need light refactoring

- Fake controller scenario builder.
- Failure-mode assertion helper.
- Shared final-status validation helper.

#### Tests that need heavy refactoring

- Manual/ignored local PX4/SIH fault-injection harness.
- Repeated failure property tests.
- Long-running supervisor soak test with synthetic failures.

---

## BH5 - Urban Blocked-Route Decision Logic

### Goal

Add mission-level reactivity without pretending to implement real obstacle
avoidance.

BH5 extends Urban from static route following to deterministic route decisions:

```text
edge becomes blocked -> detector/policy notices -> wait or replan -> judge/report
```

### Scope

1. Dynamic blocked route state:
   - blocked edge id;
   - active from tick;
   - active until tick;
   - reason;
   - optional severity.

2. Mock obstacle detector:
   - graph lookahead;
   - deterministic result;
   - no real lidar/raycast;
   - optional detection range in graph distance or meters.

3. Policies:
   - wait until unblocked;
   - replan around blocked edge;
   - abort if no route exists;
   - yield if another agent has priority.

4. Metrics:
   - `urban_replan_count`;
   - `wait_time_ticks`;
   - `blocked_edge_count`;
   - `replan_success_rate`;
   - `unresolved_blockage_count`;
   - `near_miss_count` only if a precise definition exists.

5. Replay/report:
   - blocked edge observed;
   - policy decision;
   - route replanned;
   - wait started/completed;
   - abort reason.

### Non-Goals

- no certified obstacle avoidance;
- no real sensor stream;
- no physics;
- no arbitrary polygon geometry unless needed for a tiny helper.

### Done Criteria

- One deterministic blocked-edge scenario recovers by wait or replan.
- One deterministic no-route scenario fails safely.
- Replay explains the decision.
- Metrics distinguish route following from replan/wait behavior.

### Automated Tests

#### Tests that need no refactoring

- Blocked edge before arrival triggers selected policy.
- Wait policy completes after edge unblocks.
- Replan policy chooses alternate route.
- No alternate route fails with explicit reason.
- Replay contains blocked-edge and policy-decision events.

#### Tests that need light refactoring

- Blocked-edge scenario builder.
- Route policy assertion helper.
- Urban replay event fixture helper.

#### Tests that need heavy refactoring

- Multi-agent yield policy tests.
- Dynamic obstacle schedule property tests.
- Larger generated-map stress tests.

---

## BH6 - Synthetic Scenario Testbed

### Goal

Avoid overfitting to a few hand-written scenarios. Build deterministic scenario
families for stress and degradation tests.

This is support infrastructure, not a new mission family.

### Scope

1. Seeded Urban generator:
   - grid/block road graph;
   - corridor widths;
   - static obstacle density;
   - blocked edge schedule;
   - bus placement or route.

2. Failure generator:
   - agent failure tick;
   - failure type;
   - partial completion amount;
   - replacement acceptance/rejection.

3. Communication generator:
   - packet loss;
   - latency;
   - partitions;
   - agent count.

4. Manifest:
   - generator name;
   - seed;
   - parameters;
   - scenario schema version;
   - git commit if generated during a run.

5. Test usage:
   - small deterministic generated fixtures in unit tests;
   - no dependency on local absolute paths;
   - generated data kept small in CI.

### Non-Goals

- no large random test in default CI;
- no opaque random failures;
- no generated scenario without reproducible seed/manifest.

### Done Criteria

- A seeded generator creates the same scenario on repeated runs.
- Generated scenario passes DSL validation.
- At least one generated Urban blocked-edge fixture feeds BH5 tests.
- Generator parameters are recorded in manifest.

### Automated Tests

#### Tests that need no refactoring

- Same seed yields identical scenario.
- Different seed changes at least one expected field.
- Generated Urban map validates.
- Generated blocked-edge schedule validates.

#### Tests that need light refactoring

- Scenario generator trait/helper.
- Manifest assertion helper.
- Small generated-fixture snapshot test.

#### Tests that need heavy refactoring

- Property tests over many generated maps.
- Cross-mission generated scenario framework.
- Long-run generated degradation suite.

---

## BH7 - Benchmark And Degradation Evidence

### Goal

Turn "it passed a scenario" into "we know where it works, where it degrades and
where it is unsupported".

BH7 should happen after at least one new behavior from BH1-BH6 exists.

### Scope

1. Statistical layer:
   - mean;
   - stddev;
   - stderr;
   - confidence interval;
   - min/max;
   - failure rate.

2. Degradation curves:
   - packet loss;
   - latency;
   - agent count;
   - route length;
   - obstacle density;
   - blocked-edge frequency;
   - bus detection probability;
   - failure count.

3. Benchmark support matrix:
   - supported;
   - experimental;
   - unsupported;
   - supported with caveats.

4. Urban benchmark decision:
   - add Urban to `--mission all`; or
   - create explicit `--mission urban`; or
   - keep Urban as scenario-suite evidence with a documented reason.

5. Interpretation:
   - SAR success semantics;
   - wildfire success vs completion;
   - emergency-mesh oracle/centralized caveats;
   - CBBA weak rows;
   - Urban route-risk/replan tradeoffs.

### Non-Goals

- no long run before interpretation questions are defined;
- no unsupported pair as success claim;
- no hardware evidence claim.

### Done Criteria

- At least one degradation sweep exists as artifact.
- Reports include statistical fields for key metrics.
- Unsupported rows are clearly marked.
- Docs distinguish simulation, SITL and future hardware evidence.

### Automated Tests

#### Tests that need no refactoring

- Confidence interval helper test.
- Report export includes statistical fields.
- Unsupported pair remains excluded or marked.
- Manifest records seed range and generator profile.

#### Tests that need light refactoring

- Benchmark pack validator helper.
- Degradation suite runner helper.
- Summary table consistency assertions.

#### Tests that need heavy refactoring

- Statistical delta report validation.
- Multi-pack comparison tooling.
- Long-run reproducibility harness.

---

## BH8 - Operational Runbooks And Hardware Entry Gate

### Goal

Prepare the human and procedural side of a future hardware experiment.

Hardware readiness is not just code. Before hardware exists, the project can
still define the exact go/no-go criteria and artifact expectations.

### Scope

1. Runbooks:
   - simulation runbook;
   - SITL dry-run runbook;
   - local PX4/SIH runbook;
   - future hardware candidate runbook.

2. Preflight checklist:
   - mission file validated;
   - safety report passed;
   - artifact output dir unique;
   - manual override assumption recorded;
   - geofence/no-fly assumptions recorded;
   - expected failure behavior recorded.

3. Go/no-go gates:
   - no hardware if simulation fails;
   - no hardware if dry-run fails;
   - no hardware if artifact validator fails;
   - no hardware if mission has unclassified safety violations;
   - no hardware without external safety process.

4. Post-run inspection:
   - validate artifacts;
   - inspect replay timeline;
   - compare run report and event log;
   - record known limitations;
   - decide whether rerun is allowed.

5. Documentation:
   - update `docs/HARDWARE_READINESS.md`;
   - add "first hardware experiment is still not product readiness";
   - explicitly separate single-drone controlled test from multi-agent hardware.

### Non-Goals

- no real hardware checklist pretending to be complete without hardware;
- no legal/regulatory certification;
- no public product-readiness claim.

### Done Criteria

- Runbooks exist and reference actual commands.
- Go/no-go gates are explicit.
- Hardware boundary remains conservative.
- The first future hardware experiment has a concrete preflight path.

### Automated Tests

#### Tests that need no refactoring

- Docs smoke test for required runbook sections.
- Docs smoke test for "not hardware-ready" boundary language.
- Command examples reference existing binaries/options.

#### Tests that need light refactoring

- Shared docs assertion helper for safety boundary language.
- Runbook command fixture validation.

#### Tests that need heavy refactoring

- End-to-end scripted dry-run following the runbook.
- Artifact validator integration over runbook-generated output.
- Manual/ignored local PX4/SIH rehearsal.

---

## Practical Priority

If only a few milestones can be done soon:

1. BH1 - Urban Route Export To PX4/SITL Waypoints.
2. BH3 - Artifact Validator And Evidence Contract.
3. BH4 - Fault Injection And Degraded-Mode Supervisor.
4. BH5 - Urban Blocked-Route Decision Logic.
5. BH6 - Synthetic Scenario Testbed.

BH2 should be done alongside BH1/BH3 because safety validation is part of every
future execution boundary. BH7 becomes valuable after BH5/BH6 add new behavior.
BH8 should be updated continuously, but finalized after BH1-BH4 exist.

---

## Expected Level After This Plan

After BH1-BH8, without hardware, the project would still not be a production
drone system.

But it would be much closer to a controlled hardware experiment:

- mission inputs validated before execution;
- Urban routes exportable to waypoint workflows;
- failure behavior tested in fake/local modes;
- artifacts machine-checkable;
- scenario stress tests reproducible;
- benchmark/degradation evidence interpretable;
- runbooks define exactly when not to proceed.

That is the right pre-hardware level: not "боевой продукт", but a disciplined
mission/supervisor research platform that can later meet hardware carefully.
