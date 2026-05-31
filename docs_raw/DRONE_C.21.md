# DRONE_C.21 - Итоговый набор вариантов развития проекта

Дата фиксации: 2026-05-31

Основа: сравнение `DRONE_A.20.md`, `DRONE_B.20.md`,
`DRONE_C.20.md`, текущего локального кода, README/docs и committed result
artifacts.

## Назначение документа

Этот документ фиксирует итоговый набор вариантов развития проекта после
обсуждения трех конкурирующих планов A/B/C.20.

Он не заменяет технический план конкретной задачи. Его роль:

- выбрать канонический набор векторов;
- объяснить, какие части A/B/C.20 стоит оставить;
- убрать противоречия между "mission realism", PX4/SITL, algorithm depth и
  benchmark evidence;
- предложить разумный порядок следующих milestone;
- задать тестовые ожидания для каждого направления.

## Executive Summary

Ни один из A/B/C.20 не стоит брать целиком.

Лучший итоговый план:

```text
short evidence cleanup
  -> Urban Patrol v0
  -> Urban Search v1
  -> Replay / Analysis support
  -> Algorithm Depth on the stronger mission substrate
  -> Benchmark / Research Refresh
```

PX4/SITL hardening остается поддерживающим направлением, но не главным
следующим вектором. Platform/API packaging откладывается до появления хотя бы
одной реальной новой миссии через extension path.

Главная рекомендация: **Urban Navigation / Mission Realism** как следующий
основной вектор, но в строго ограниченном первом scope:

- road graph, not arbitrary polygons first;
- AABB buildings/no-fly zones first;
- deterministic route planner;
- independent judge;
- replay/metrics;
- no lidar/raycast in v0;
- no real physics;
- no PX4 dependency in v0.

## Comparison Of A/B/C.20

### DRONE_A.20

Сильные стороны:

- лучшая архитектурная рамка;
- правильно разделяет low-level flight control and mission-level logic;
- хорошо объясняет, почему Urban Navigation не конфликтует с PX4;
- делает Urban Navigation главным практическим вектором;
- содержит понятную тестовую стратегию;
- не пытается преждевременно стабилизировать API/platform.

Слабые стороны:

- местами слишком быстро переходит к polygon geometry;
- не так подробно раскрывает algorithm/benchmark backlog, как B.20;
- утверждения про current benchmark evidence требуют осторожности: текущий
  benchmark pack указывает на старый commit.

Что оставить:

- layered architecture principle;
- Urban Navigation as primary next vector;
- "do not build a PX4 replacement";
- replay/debuggability as supporting track;
- Platform/API packaging as later work.

### DRONE_B.20

Сильные стороны:

- лучший список algorithm-depth work items:
  communication-aware scoring, mission-specific planners, CBBA convergence,
  hierarchical coordination;
- хорошо выделяет benchmark interpretation issues;
- хорошо фиксирует PX4/SITL follow-up items: local harness, broader failure
  modes, replay timeline;
- Perimeter Patrol описан как low/medium complexity mission candidate.

Слабые стороны:

- слишком осторожно отрезает motion/navigation layer от проекта;
- слишком сильно смещает фокус в algorithm/PX4 backlog;
- replay seq fix уже закрыт последующими коммитами, поэтому не должен
  оставаться активным пунктом;
- current benchmark status needs correction before being used as evidence.

Что оставить:

- algorithm-depth backlog;
- benchmark interpretation backlog;
- SITL hardening as optional/supporting track;
- lightweight Perimeter Patrol idea, but broaden it into Urban Patrol/Search.

### DRONE_C.20

Сильные стороны:

- самая подробная staged decomposition;
- road-graph-first approach снижает риск geometry scope creep;
- хорошо разделяет Urban Patrol, Urban Search, Dynamic Avoidance;
- содержит детальные test categories per vector;
- явно признает M62/evidence cleanup as pre-step.

Слабые стороны:

- слишком длинный как основной roadmap;
- частично дублирует A.20;
- New Mission generic и Urban Navigation можно объединить, чтобы не держать
  два конкурирующих направления.

Что оставить:

- road-graph-first Urban plan;
- mock perception framing;
- decision matrix;
- detailed testing discipline;
- recommendation to postpone full lidar/polygon physics.

## Corrections To Carry Forward

### Benchmark current-HEAD claim

Текущий 500-seed benchmark artifact:

- artifact: `results/all_500_jobs14_m62_release/`;
- manifest commit: `81260ca7afa114a5d9add7b832f6c5d7875b88cd`;
- observed pre-document HEAD: `f9ed1c399589631e3079f0d31dc01bc999f75892`;
- simulation-affecting code changed after artifact, including
  `crates/swarm-sim/src/runner.rs`.

Therefore the artifact must be treated as historical validation evidence until
it is rerun for current HEAD or docs explicitly downgrade the claim.

### Replay seq debt

The M59 replay replacement-seq issue listed in `DRONE_B.20.md` is no longer an
open vector item. It was fixed by the later `Fix M59 replay replacement seqs`
commit. Future plans can still include artifact validation around seq semantics,
but not the original bug as an open task.

### Motion planning boundary

The project should not implement low-level flight control, motor physics,
attitude control, SLAM, real lidar processing, or certified obstacle avoidance.

The project can implement:

- map-aware mission constraints;
- route planning over a simplified map;
- independent simulation judge;
- mock perception events;
- mission-level replan/stop/report decisions;
- multi-agent deconfliction at the mission layer.

This is not a contradiction. PX4 can remain the waypoint execution layer while
the Rust workspace owns mission planning, simulation semantics, replay, metrics
and coordination.

## Final Vector Set

The final set has six active vectors plus one deferred vector.

```text
V0 Evidence / Cleanup
V1 Urban Navigation / Search
V2 Replay / Analysis
V3 Algorithm Depth
V4 PX4 / SITL Hardening
V5 Benchmark / Research Evidence
V6 Platform / API Packaging (deferred)
```

Generic New Mission is not a separate top-level vector in the final plan.
Urban Navigation/Search is the chosen New Mission candidate. Logistics and
Pursuit remain alternatives if Urban is deferred or completed.

## V0 - Evidence / Cleanup

### Purpose

Create a clean baseline before starting a new large feature.

This vector is intentionally short. It should not consume the project, but it
prevents stale claims from contaminating the next milestone.

### Current state

What exists:

- M58/M59 artifacts;
- benchmark pack in `results/all_500_jobs14_m62_release/`;
- README, `docs/STATUS.md`, `docs/BENCHMARK_RESULTS.md`;
- regression and benchmark tooling.

Known issues:

- benchmark pack is not current HEAD evidence;
- wildfire/flood wording remains historically confusing;
- wildfire success semantics are not cleanly explained against completion;
- some docs claim "current HEAD" too strongly.

### Work items

1. Decide benchmark treatment:
   - rerun the 500-seed release benchmark on current HEAD; or
   - mark existing pack as historical evidence for commit `81260ca...`.

2. Sync status docs:
   - README;
   - `docs/STATUS.md`;
   - `docs/BENCHMARK_RESULTS.md`;
   - result README/manifest notes if necessary.

3. Close flood wording:
   - preferred: remove "flood" from active capability claims;
   - leave flood as future work;
   - do not implement flood unless Disaster Mapping becomes the main vector.

4. Harden wildfire success semantics:
   - document exact success predicate;
   - add test for small-static and medium-dynamic;
   - explain success vs task completion in benchmark docs.

5. Validate M58/M59 artifacts:
   - replay summaries parse;
   - event categories are present;
   - replacement mission seq semantics remain correct.

### Done criteria

- No user-facing doc claims current benchmark evidence unless manifest commit
  matches the intended code state.
- Flood is either implemented or explicitly future work.
- Wildfire success/completion mismatch is documented and tested.
- Existing targeted test set passes.

### Tests

#### No refactoring

- Docs smoke tests for required limitation phrases.
- Benchmark manifest identity test for committed artifact metadata.
- Wildfire success semantics test.
- Replay summary tests for M58/M59 event categories.

#### Light refactoring

- Benchmark-pack validation helper.
- Shared docs/status assertion helper.
- Small wildfire fixture with explicit mapped-ratio expectations.

#### Heavy refactoring

- Structured status manifest instead of duplicated free-form Markdown claims.
- Historical/current benchmark classifier.

## V1 - Urban Navigation / Search

### Purpose

Add a practical simulation mission that is closer to real drone tasks without
requiring real hardware, Gazebo, visualization, real CV, or physics simulation.

This is the main recommended next vector.

### Core idea

Stage 1:

> "Облети квартал."

Stage 2:

> "Облетай квартал пока не встретишь автобус."

Interpretation:

- fixed flight altitude;
- local 2D coordinate frame;
- buildings are static obstacles/no-fly zones;
- allowed travel space is represented first as road graph/corridors;
- drone movement remains kinematic and deterministic;
- judge independently checks violations;
- perception blocks are mocked.

### Why this belongs in the project

It develops mission-level navigation and decision logic, not low-level flight
control.

PX4 answers:

- how to execute waypoints;
- low-level flight stabilization;
- real vehicle control.

This project answers:

- which route to choose;
- whether the route is valid;
- how to represent the mission;
- what to do after a detection event;
- how to score success/failure;
- how several agents avoid mission-level conflicts;
- how to replay and benchmark behavior.

### What already exists

- `Pose`, `Task`, `TaskKind`, `MissionAdapter`, `RunState`;
- scenario DSL and scenario catalog tests;
- movement with `enable_movement`;
- safety config with geofence, AABB no-fly zones, separation;
- replay events and summaries;
- SAR-like sensor concepts;
- metrics/report/export infrastructure.

### What is missing

- road graph / map model;
- route planner through allowed space;
- independent judge for route validity;
- urban-specific mission events;
- bus/dynamic object model;
- mock detector interface;
- route trace export;
- multi-agent route deconfliction.

### Milestone V1.1 - Urban Patrol v0

Goal: one drone completes a city-block patrol route on a road graph with no
judge violations.

Scope:

1. Introduce `urban_patrol` scenario profile.
2. Add minimal map representation:
   - nodes;
   - edges;
   - edge length;
   - allowed route loop;
   - optional AABB buildings/no-fly zones.
3. Add deterministic route planner:
   - Dijkstra or A*;
   - deterministic tie-breaking;
   - route is a sequence of graph nodes or generated waypoints.
4. Add judge:
   - route uses only allowed edges;
   - no point enters building/no-fly AABB;
   - loop completed;
   - timeout if incomplete.
5. Add metrics:
   - `urban_patrol_completed`;
   - `urban_violation_count`;
   - `route_length_m`;
   - `route_efficiency`;
   - `time_to_complete_loop`;
   - `replan_count` initially zero.
6. Add replay/report events:
   - route planned;
   - segment entered/completed;
   - violation;
   - patrol completed.

Non-goals:

- no arbitrary polygon geometry;
- no lidar;
- no bus;
- no dynamic obstacles;
- no PX4 execution;
- no multi-agent deconfliction.

### Milestone V1.2 - Urban Patrol v1 Geometry

Goal: expand from graph-only constraints to simple geometric constraints.

Scope:

1. Keep road graph as primary route topology.
2. Add corridor width or AABB corridor bands.
3. Detect leaving allowed corridor.
4. Detect segment intersection with AABB building/no-fly zone.
5. Export route trace.

Non-goals:

- no arbitrary polygon library unless needed;
- no continuous collision physics.

### Milestone V1.3 - Urban Search v1

Goal: drone patrols until it detects a bus through a mocked detector.

Scope:

1. Add bus entity:
   - static bus first, dynamic route later;
   - id, pose, active tick range.
2. Add `BusDetector` mock:
   - range;
   - detection probability;
   - false positive rate;
   - deterministic seed.
3. Add mission policy:
   - continue patrol until detection;
   - stop/report on detection;
   - timeout if not detected.
4. Add metrics:
   - `bus_detected`;
   - `time_to_detect_bus`;
   - `false_positive_count`;
   - `distance_before_detection`;
   - `search_success_without_violation`.
5. Add replay events:
   - bus observed;
   - bus detected;
   - false positive;
   - urban search completed.

Non-goals:

- no image processing;
- no camera model;
- no real object detection;
- no line-of-sight in first version unless cheap.

### Milestone V1.4 - Urban Multi-Agent / Avoidance v2

Goal: multiple drones share the urban map and resolve mission-level route
conflicts.

Scope:

1. Two or more drones on the same graph.
2. Separation measured by judge.
3. Route conflicts detected on edges/nodes.
4. Simple policies:
   - wait;
   - yield;
   - replan around blocked edge.
5. Metrics:
   - separation violations;
   - near misses;
   - wait time;
   - replan success rate;
   - unresolved blockages.

Non-goals:

- no certified collision avoidance;
- no full multi-agent traffic simulator;
- no onboard distributed autonomy claim.

### Risks

- Geometry scope creep.
- Mock lidar/object detector could be mistaken for real perception.
- If replay/report support is weak, debugging urban scenarios becomes painful.
- If route graph is too simple, the mission may look like waypoint expansion.

### Recommendation

Start with graph-first Urban Patrol. Add geometry and detector layers only
after route/judge/replay are stable.

## V2 - Replay / Analysis

### Purpose

Make behavior inspectable without building a visual 2D/3D viewer.

Urban missions need this immediately: without route traces and judge timelines,
failures will be hard to understand.

### Work items

1. Route trace:
   - planned route;
   - executed route;
   - segment status;
   - distance per segment.

2. Judge report:
   - violation type;
   - point/segment;
   - obstacle id;
   - tick;
   - agent id.

3. Timeline output:
   - task assigned;
   - route planned;
   - segment entered;
   - detector event;
   - violation;
   - replan;
   - completion.

4. CLI support:
   - `replay --timeline`;
   - `replay --agent <id>`;
   - optional `replay --category urban`.

5. CSV/JSON analysis export:
   - per-tick pose trace;
   - event counts;
   - route metrics.

### Done criteria

- Urban Patrol/Search runs can be debugged from text artifacts.
- Replay schema remains backward-compatible.
- New event categories have summary tests.

### Tests

#### No refactoring

- Event serialization roundtrip.
- Replay summary contains urban event counters.
- Timeline output is deterministic for a fixture.
- CSV route trace headers are stable.

#### Light refactoring

- Shared event-summary formatter.
- Compact route trace fixture.
- Backward-compatibility fixture for old replay logs.

#### Heavy refactoring

- Versioned replay schema migration tests.
- Large replay performance tests.
- Cross-run replay diff tooling.

## V3 - Algorithm Depth

### Purpose

Improve strategies after the project has a stronger mission substrate.

Algorithm work is valuable, but it becomes much more meaningful when tested
against Urban Patrol/Search or another realistic mission, not only point/zone
scenarios.

### Workstream V3.1 - Communication-aware scoring

Current gap:

- `comms_range` exists;
- connectivity-aware allocator exists;
- most scoring still ignores communication cost.

Work:

1. Add `message_budget` or `comms_penalty_weight`.
2. Penalize assignments outside reliable communication range.
3. Benchmark under packet-loss and partition profiles.
4. Compare greedy/auction/connectivity-aware/CBBA on success vs message cost.

Metrics:

- messages_attempted;
- messages_dropped;
- network_availability;
- disconnected_agents_max;
- task_completion_rate;
- success_rate.

### Workstream V3.2 - Mission-specific planners

Work:

- SAR uncertainty-aware planner;
- wildfire priority-triggered reallocation;
- inspection route optimization for non-centralized strategies;
- urban corridor-aware planner.

This should not be a generic abstraction first. Implement one measurable
mission-specific improvement, benchmark it, then generalize if needed.

### Workstream V3.3 - CBBA convergence and support matrix

Work:

1. Analyze unsupported/weak pairs with replay.
2. Decide whether each gap is inherent, a parameter issue, or a bug.
3. Try failure-triggered gossip burst or tuned gossip interval.
4. Update support matrix honestly.

### Workstream V3.4 - Scaling

Work:

- add 8-agent and 16-agent profiles;
- measure message count, conflicts, completion time;
- consider hierarchical coordination only after measuring need.

### Done criteria

- At least one strategy improvement shows measurable benefit on a meaningful
  scenario.
- Support matrix distinguishes unsupported-by-design from current bugs.
- Benchmark docs explain tradeoffs instead of only ranking success rates.

## V4 - PX4 / SITL Hardening

### Purpose

Keep improving local PX4/SIH evidence without making it the main next vector.

This vector is useful when a new mission needs export to SITL waypoints, or
when supervisor reliability becomes a bottleneck.

### Work items

1. Local integration harness:
   - launch two PX4 SIH instances;
   - wait for ports;
   - run supervisor;
   - capture logs;
   - clean up processes;
   - write repeatable result directory.

2. Artifact validator:
   - manifest/report/event-log/replay-summary consistency;
   - event seq/task id consistency;
   - final status consistency.

3. Broader failure modes:
   - fail before upload;
   - fail after upload;
   - fail during progress;
   - no-progress timeout;
   - partial completion then failure;
   - repeated failure if scope requires it.

4. Replay timeline for SITL:
   - agent-filtered timeline;
   - event categories;
   - elapsed time ordering.

### Non-goals

- no hardware readiness claim;
- no Gazebo as default gate;
- no PX4 CI unless explicitly chosen later;
- no production failover claim.

### Done criteria

- Local manual runs are reproducible enough for development use.
- Artifacts are easier to validate.
- Failure modes are covered by fake-controller tests even if PX4 runs remain
  manual.

## V5 - Benchmark / Research Evidence

### Purpose

Build stronger analytical claims after the mission/algorithm substrate improves.

The benchmark vector should not be the next big step before Urban or Algorithm
Depth. Otherwise it mainly measures the current abstract scenarios.

### Work items

1. Current-head 500-seed validation after major changes.
2. Supported-pair matrix:
   - stable;
   - experimental;
   - unsupported with reason.
3. 1000-seed publication-like run only when claims need it.
4. Confidence intervals:
   - mean;
   - stderr;
   - min/max;
   - failure rate.
5. Degradation curves:
   - packet loss;
   - latency;
   - agent count;
   - urban obstacle density;
   - bus detection probability;
   - failure count.
6. Strategy comparison interpretation:
   - where greedy is enough;
   - where connectivity-aware wins;
   - where centralized is only an oracle;
   - where CBBA is unsupported or weak.

### Done criteria

- Benchmark artifact commit identity is explicit.
- Current/historical evidence is not mixed.
- Tables include interpretation, not only numbers.
- Unsupported pairs are excluded from success claims or clearly marked.

## V6 - Platform / API Packaging

### Status

Deferred.

### Purpose

Eventually convert stable-ish extension guidance into a cleaner external API or
plugin boundary.

### Why deferred

The project already has `docs/EXTENSION_GUIDE.md`. Stabilizing public APIs now
would be premature because:

- no real supported new mission has exercised the full extension path yet;
- Urban Navigation may reveal missing extension surfaces;
- public API work gives little research value by itself.

### Revisit when

- Urban Patrol/Search or another real mission is complete;
- at least one external-style strategy/mission example exists;
- schema compatibility tests are already in place.

## Alternatives Kept On The Shelf

### Logistics / Delivery

Good future mission if the goal becomes task dependencies, pickup/dropoff,
capacity and deadlines.

Keep because:

- it tests a different axis than Urban;
- it stresses task registry and completion semantics.

Do not choose first because:

- it is less connected to physical navigation;
- it can become a scheduling/VRP project.

### Multi-target Pursuit

Good future mission if the goal becomes dynamic target tracking.

Keep because:

- it stresses reactive planning;
- it differs from static waypoint/zone tasks.

Do not choose first because:

- without map/perception it can become a toy chase model;
- moving target semantics are harder to debug without replay improvements.

### Minimal Perimeter Patrol

Could be a smaller version of Urban Patrol:

- polygon or route loop;
- generated waypoints;
- no road graph;
- no judge beyond safety config.

Use only if a quick demo is needed. As main M63, prefer graph-first Urban
Patrol because it creates more reusable navigation infrastructure.

## Recommended Milestone Sequence

### M63 - Evidence Cleanup And Urban Foundations

This can be split into two commits/plans if needed.

Outcome:

- benchmark/status docs are honest;
- flood/wildfire wording cleaned;
- wildfire success semantics tested;
- Urban map/road graph design finalized;
- initial fixtures planned.

### M64 - Urban Patrol v0

Outcome:

- `urban_patrol` scenario loads;
- one-drone route loop completes;
- road graph planner works;
- judge reports route validity;
- replay/report include core urban events and metrics;
- regression smoke exists.

### M65 - Urban Search v1

Outcome:

- bus entity exists;
- mock bus detector exists;
- mission stops on detection;
- success semantics include detection and no judge violation;
- deterministic search test exists.

### M66 - Urban Replay / Analysis And Multi-Agent Prep

Outcome:

- route traces;
- judge timeline;
- replay timeline filtering;
- multi-agent route conflict fixture;
- separation metrics ready for next step.

### M67 - Algorithm Depth On Urban + Existing Missions

Outcome:

- one mission-specific planner improvement;
- one communication-aware scoring improvement or explicit defer;
- benchmark delta showing measurable impact.

### M68 - Benchmark Refresh

Outcome:

- current-head benchmark after Urban/Algorithm changes;
- support matrix updated;
- benchmark docs distinguish current, historical and unsupported evidence.

## Test Strategy

### Tests that need no refactoring

- Scenario DSL load/validation tests for new urban fixtures.
- Unit tests for route graph parsing.
- Unit tests for deterministic shortest path.
- Unit tests for AABB judge violations.
- Replay event serialization roundtrip.
- Regression smoke for a small Urban Patrol scenario.
- Wildfire success semantics test.
- Benchmark manifest identity test.
- Existing SITL supervisor and replay tests remain green.

### Tests that need light refactoring

- Shared urban fixture builder.
- Route assertion helper:
  - route uses allowed edges;
  - route avoids forbidden zones;
  - route completes loop.
- Judge assertion helper:
  - no violation;
  - building violation;
  - corridor exit;
  - separation breach.
- Mock detector fixture:
  - bus visible;
  - bus out of range;
  - false positive controlled by seed.
- Replay timeline fixture.
- Benchmark pack validation helper.

### Tests that need heavy refactoring

- Polygon geometry property tests.
- Random map generation with route existence guarantees.
- Dynamic obstacle and replan property tests.
- Multi-agent deconfliction property tests.
- CBBA convergence under arbitrary message loss.
- Long-run benchmark reproducibility harness.
- Versioned replay schema migration tests.

## Final Recommendation

Adopt this final plan:

```text
V0 cleanup/evidence
V1 Urban Navigation/Search as main new capability
V2 replay/analysis as support
V3 algorithm depth after Urban gives better pressure
V4 PX4/SITL hardening only as needed
V5 benchmark/research after stronger scenarios
V6 platform packaging later
```

The next major implementation should be **Urban Patrol v0** after a short
evidence cleanup. It gives the project the most useful new capability:
realistic mission-level movement and decision semantics without crossing into
hardware, Gazebo, real CV, or low-level flight control.

Short formula:

```text
Do not build a new PX4.
Build map-aware mission planning, a deterministic judge, mock perception,
replay, metrics, and coordination logic.
```
