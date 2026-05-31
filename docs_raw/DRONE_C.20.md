# DRONE_C.20 - Detailed Development Vectors

Дата фиксации: 2026-05-31

## Контекст

Этот документ фиксирует возможные направления развития проекта после
закрытия основной линии M57-M62 из `DRONE_A.19.md` и обсуждения идей из
`DRONE_B.19.md` / `DRONE_B.20.md`.

Цель документа не в том, чтобы объявить один обязательный roadmap, а в том,
чтобы разложить варианты по смыслу, пользе, готовности текущего кода,
рискам и ожидаемым проверкам.

Текущая сильная сторона проекта:

- deterministic simulation foundation;
- task ownership, failure detection, dynamic reallocation;
- benchmark/regression/report/replay infrastructure;
- mission semantics через `TaskKind`, `MissionAdapter`, `AdapterRegistry`;
- safety layer: geofence, no-fly zones, separation checks;
- 2D movement model with battery drain and movement metrics;
- local PX4/SIH single-agent and multi-agent evidence;
- controlled local PX4/SIH failure/reallocation workflow;
- extension guide for in-repository missions, strategies, metrics, and schemas.

Текущая слабая сторона проекта:

- simulation missions still remain fairly abstract;
- movement is direct-to-task, not route planning through constrained space;
- safety checks are mostly validator/judge logic, not avoidance or replanning;
- no polygon/navmesh/road-graph map model;
- no simulated perception layer for obstacles or object detection;
- benchmark artifacts must be kept aligned with the current HEAD to remain
  current evidence;
- wildfire/flood wording and wildfire success semantics still need cleanup if
  Disaster Mapping remains a claim.

## Принцип выбора

Следующий вектор стоит выбирать не по тому, что "можно реализовать", а по
тому, какой новый уровень достоверности он добавляет.

Основные вопросы:

1. Нужна ли сейчас новая пользовательская capability или более честная
   evidence base?
2. Хотим ли мы развивать mission-level coordination, navigation decision
   logic, PX4 workflow, или исследовательскую аналитику?
3. Должен ли следующий milestone быть коротким стабилизирующим шагом или
   новой содержательной миссией?
4. Какие утверждения о проекте мы сможем честно сделать после завершения?

## Вектор 0 - Evidence / Cleanup

### Суть

Короткий стабилизирующий pass перед любой крупной новой разработкой.

Это не самый интересный вектор, но он повышает доверие к текущему состоянию:
docs, benchmark artifacts, regression claims и status tables должны совпадать
с фактическим кодом.

### Что уже есть

- `docs/STATUS.md`, `docs/BENCHMARK_RESULTS.md`, README status table.
- 500-seed benchmark pack in `results/all_500_jobs14_m62_release/`.
- Regression runner and benchmark report exports.
- Captured M58/M59 PX4/SIH artifacts.

### Что надо сделать

1. Перепроверить, что benchmark artifact действительно соответствует текущему
   HEAD. Если после benchmark менялся simulation-affecting код, rerun benchmark
   or downgrade the artifact to historical evidence.

2. Синхронизировать README, `docs/STATUS.md`, `docs/BENCHMARK_RESULTS.md` и
   `results/.../manifest.json` statements.

3. Закрыть wildfire/flood wording debt:
   - если flood не реализуем сейчас, убрать обещание flood из user-facing
     claims и оставить его как future work;
   - если оставляем "wildfire / flood mapping" wording, нужен отдельный flood
     scope and implementation plan.

4. Зафиксировать wildfire success semantics:
   - либо success действительно равен documented completion threshold;
   - либо docs/report clearly say that success is stricter than completion.

5. Проверить, что recent SITL/M59 artifacts replay cleanly and contain the
   intended event semantics.

### Польза

- Уменьшает риск ложных claims.
- Дает чистую точку перед новой большой миссией.
- Упрощает review последующих изменений.

### Риски

- Не добавляет новой capability.
- Может выглядеть как "техническая уборка", хотя фактически является
  evidence hardening.

### Done criteria

- Benchmark/status docs do not claim current evidence for stale artifacts.
- README, `docs/STATUS.md`, `docs/BENCHMARK_RESULTS.md` agree on limitations.
- Flood scope is explicit: implemented, or out-of-scope/future work.
- Wildfire success/completion semantics are documented and tested.
- Existing regression and targeted SITL tests pass.

### Автоматические тесты

#### Без рефакторинга

- Benchmark pack manifest validation: git commit, command line, seed count,
  output files.
- Docs smoke test for required status phrases and limitation statements.
- Replay summary tests for M58/M59 event categories.
- Wildfire success/completion unit or integration test for existing profiles.

#### Легкий рефакторинг

- Helper that validates a committed benchmark pack against current repository
  expectations without assuming a machine-specific path.
- Shared docs assertion helper for README/status/benchmark consistency.
- Small wildfire fixture with explicit expected success threshold.

#### Тяжелый рефакторинг

- Reproducible benchmark-pack verifier that can compare a historical artifact
  against current code identity and mark it historical/current.
- Cross-document status schema instead of free-form Markdown status tables.

## Вектор 1 - Urban Navigation / Search

### Суть

Добавить новую simulation mission family, приближенную к реальным задачам, но
без настоящей физики, CV, Gazebo, hardware, or PX4 dependence.

Идея: "облети квартал", затем "облетай квартал пока не встретишь автобус".

Это не должно быть попыткой написать свой PX4 или physics engine. Это mission
and navigation decision layer:

- карта известна;
- здания или препятствия заданы как forbidden geometry;
- дороги/коридоры заданы как allowed traversal space;
- дрон летит на одной высоте;
- route planner строит безопасный маршрут;
- physical judge независимо проверяет нарушения;
- lidar/object detector are mocked perception blocks;
- replay/metrics explain decisions and failures.

### Почему это вписывается в проект

Текущий проект в основном отвечает на вопрос "какому агенту какую задачу
назначить". Urban Navigation добавляет следующий слой: "как агент проходит
задачу в ограниченной среде и как принимает decision-level решения".

Это не конфликтует с PX4/SITL направлением:

- PX4 остается execution layer for waypoints.
- Urban simulation остается deterministic research layer.
- Слой проекта отвечает за route choice, task semantics, replan policy,
  coordination and metrics.

### Что уже есть

- `Pose`, `Task`, `TaskKind`, `MissionAdapter`, `RunState`.
- `enable_movement` and gradual movement toward assigned tasks.
- Safety layer: geofence, AABB no-fly zones, separation.
- Replay events for safety violations, SAR scans/detections, task completion.
- SAR sensor model concepts: detection probability, false positive rate,
  detection range.
- DSL and scenario catalog validation.

### Чего нет

- Road graph, navmesh, polygon map, or allowed-corridor model.
- Polygon no-fly zones; current safety is AABB-centric.
- Route planner through constrained space.
- Dynamic obstacle model.
- Lidar/raycast simulation.
- Object detector event model for buses or other targets.
- Mission semantics for patrol loop completion and search-until-detected.

### Milestone U1 - Urban Patrol v0

**Goal:** one drone patrols a city-block loop without violating map constraints.

Scope:

1. Add `urban_patrol` scenario type or profile.
2. Represent the map as a road graph first, not arbitrary polygons:
   - nodes: intersections / route points;
   - edges: allowed road corridors;
   - buildings: AABB no-fly zones;
   - patrol route: ordered node loop or polygon converted to graph targets.
3. Add `TaskKind::UrbanWaypoint` or reuse `Waypoint` with an urban mission
   adapter only if no new semantics are needed.
4. Add route planner:
   - Dijkstra/A* over road graph;
   - deterministic tie-breaking;
   - no local obstacle avoidance yet.
5. Add independent judge:
   - entered building/no-fly zone;
   - left allowed road corridor;
   - separation breach if multiple agents are present;
   - route completed or incomplete.
6. Add replay events:
   - `UrbanRoutePlanned`;
   - `UrbanSegmentEntered`;
   - `UrbanViolation`;
   - `UrbanPatrolCompleted`.
7. Add metrics:
   - `patrol_completion_rate`;
   - `urban_violations`;
   - `route_length`;
   - `route_efficiency`;
   - `replans`;
   - `time_to_complete_loop`.

Non-goals:

- no lidar;
- no buses;
- no dynamic obstacles;
- no continuous physics;
- no PX4 requirement;
- no visualization requirement.

### Milestone U2 - Urban Search v1

**Goal:** drone patrols until it detects a bus.

Scope:

1. Add dynamic or static bus entity:
   - id;
   - pose or route over the road graph;
   - appearance/disappearance tick;
   - optional speed.
2. Add `BusDetector` mock:
   - range;
   - detection probability;
   - false positive rate;
   - line-of-sight can be added later, not required in v1.
3. Add mission policy:
   - continue patrol until bus detected;
   - stop/report after detection;
   - optional intercept waypoint after detection.
4. Add replay events:
   - `BusObserved`;
   - `BusDetectionFalsePositive`;
   - `UrbanSearchCompleted`.
5. Add metrics:
   - `bus_detection_rate`;
   - `time_to_detect_bus`;
   - `false_positive_count`;
   - `distance_before_detection`;
   - `search_success_without_violation`.

Non-goals:

- no real CV;
- no image simulation;
- no physical bus model beyond deterministic movement on the map.

### Milestone U3 - Dynamic Avoidance v2

**Goal:** introduce local replan decisions when unexpected obstacles or other
agents block a planned path.

Scope:

1. Add temporary obstacles on graph edges or road segments.
2. Add lidar-like range detector as a mocked geometry query.
3. Add policy:
   - stop;
   - wait;
   - replan around blocked edge;
   - yield to another drone.
4. Add multi-agent scenario with separation and route conflicts.
5. Add metrics:
   - avoided_collisions;
   - unresolved_blockages;
   - wait_time;
   - replan_success_rate;
   - near_miss_count.

Non-goals:

- no aerodynamic model;
- no motor/control simulation;
- no guarantee of hardware obstacle avoidance.

### Польза

- Makes simulation less abstract and closer to real mission decisions.
- Tests route planning, constraints, perception events, and judge metrics.
- Creates a practical M63 candidate stronger than purely abstract Logistics or
  Pursuit.
- Provides a base for later algorithm depth and benchmark work.

### Риски

- Scope creep into physics/visualization/lidar realism.
- Geometry complexity can grow quickly if polygons are introduced too early.
- If implemented as direct waypoint expansion only, it may not add enough new
  behavior beyond current SITL waypoints.

### Recommendation

Start with road graph and AABB buildings. Do not start with arbitrary polygons
or raycast lidar. Add those only after route-graph patrol and judge metrics are
stable.

### Автоматические тесты

#### Без рефакторинга

- Road graph parse/validation from inline scenario fixture.
- A* or Dijkstra returns deterministic loop route.
- Urban patrol completes on a simple square block.
- Judge reports no violation for a valid route.
- Judge reports no-fly violation for a route through a building.
- Replay roundtrip for urban events.

#### Легкий рефакторинг

- Route-planning helper shared by runner and tests.
- Scenario builder for small urban block fixtures.
- Metrics assertion helper for patrol completion and violations.
- Sensor fixture for bus detection probability with deterministic seed.

#### Тяжелый рефакторинг

- Polygon geometry support and point-in-polygon tests.
- Segment-vs-obstacle intersection checks.
- Dynamic edge blocking with replan policy.
- Multi-agent route conflict property tests.
- Lidar/raycast simulation tests.

## Вектор 2 - New Mission / Platform Validation

### Суть

Добавить новую mission family specifically to validate `docs/EXTENSION_GUIDE.md`
on a real supported mission, not only on test-only fixtures.

Urban Navigation is one candidate in this vector. Other candidates are
Logistics/Delivery and Multi-target Pursuit.

### Option 2A - Logistics / Delivery

Value:

- tests task dependencies;
- tests capacity constraints;
- adds precedence semantics absent from all current missions.

Possible scope:

- `TaskKind::Pickup`;
- `TaskKind::Dropoff`;
- `requires_pickup`;
- `cargo_capacity`;
- `delivered_items`;
- deadline/time-window optional in v2.

Metrics:

- `delivery_rate`;
- `late_deliveries`;
- `capacity_violations`;
- `precedence_violations`;
- `unserved_deliveries`;
- `route_cost`.

Risks:

- Requires changes to task registry and completion semantics.
- Can become a scheduling/VRP project if not scoped tightly.

Best use:

- Choose this if the goal is to stress dependencies and stateful tasks.

### Option 2B - Multi-target Pursuit

Value:

- tests moving targets;
- tests reactive task allocation;
- stresses dynamic replanning.

Possible scope:

- target trajectories;
- capture radius;
- dynamic target appearance;
- capture/escort modes later.

Metrics:

- `capture_rate`;
- `time_to_intercept`;
- `targets_lost`;
- `pursuit_distance`;
- `interception_efficiency`.

Risks:

- Moving target state touches runner internals.
- Completion semantics can become ambiguous if target and agent move in the
  same tick.

Best use:

- Choose this if the goal is algorithmic reactivity.

### Option 2C - Urban Patrol/Search

Value:

- connects mission semantics with constrained movement and physical judge.
- moves project closer to real-world task structure without hardware.

Best use:

- Choose this if the goal is practical realism and navigation decision logic.

### Done criteria

- New mission has at least two scenarios: small and medium.
- Support matrix says which strategies are stable, experimental, unsupported.
- Replay explains mission-specific transitions.
- Metrics are exported to JSON/CSV/Markdown if they are user-facing.
- Regression smoke exists and is portable.
- The mission is documented in README/status and scenario docs.

### Автоматические тесты

#### Без рефакторинга

- DSL parse/validation for new scenario type.
- Task generation test.
- Completion semantics test.
- Replay serialization roundtrip.
- One benchmark smoke for the small scenario.

#### Легкий рефакторинг

- Mission-specific fixture builder.
- Shared support-matrix assertion helper.
- Outcome assertion helper for mission metrics.

#### Тяжелый рефакторинг

- Property tests for dynamic target/task behavior.
- Multi-seed stability tests.
- Comparative strategy tests across several mission profiles.

## Вектор 3 - Algorithm Depth

### Суть

Improve the intelligence and measurable differentiation of allocation and
coordination strategies.

This vector is less about adding a new mission and more about making existing
strategies meaningfully different under pressure.

### Workstream 3A - Communication-aware scoring

Current gap:

- `comms_range` exists on agents.
- Connectivity-aware allocator exists.
- Most scoring still behaves as if communication cost does not matter.

Potential work:

1. Introduce message budget or communication penalty into allocation scoring.
2. Penalize assignments that move agents outside reliable communication range.
3. Compare greedy/auction/connectivity-aware under loss and partition profiles.
4. Report tradeoff: success vs message count vs availability.

Useful metrics:

- messages_attempted;
- messages_dropped;
- network_availability;
- disconnected_agents_max;
- task_completion_rate;
- success_rate;
- conflicts.

### Workstream 3B - Mission-specific planner modes

Current gap:

- Scoring differs by coefficients, but planners are not deeply mission-aware.

Potential work:

- SAR: prioritize high-information cells and dynamic belief entropy.
- Wildfire: priority-triggered reallocation after threat updates.
- Inspection: route optimization for non-centralized strategies.
- Urban: route-graph planner and replan policy.

### Workstream 3C - CBBA convergence and support matrix

Current gap:

- Some mission/strategy pairs are unsupported or weak.
- Some failures may be inherent to the strategy, not bugs.

Potential work:

1. Separate "unsupported by design" from "bug/regression".
2. Add replay-driven diagnostics for delayed reconvergence.
3. Experiment with gossip interval and failure-triggered gossip burst.
4. Re-benchmark CBBA after targeted changes.

### Workstream 3D - Scale beyond small swarms

Potential work:

- 8-agent and 16-agent scenario profiles.
- Message-count scaling curves.
- Hierarchical coordination only if benchmark shows need.

### Польза

- Produces stronger research story.
- Makes benchmark comparisons more meaningful.
- Reduces chance that all strategies look similar under current scenarios.

### Риски

- Without more realistic missions, algorithm improvements may optimize
  abstractions rather than useful behavior.
- CBBA fixes can be time-consuming and still not improve practical missions.

### Автоматические тесты

#### Без рефакторинга

- Unit tests for communication penalty scoring.
- Regression smoke comparing message counts in a tiny controlled profile.
- Support-matrix tests for explicitly unsupported pairs.
- Failure-triggered gossip behavior test if policy is added locally.

#### Легкий рефакторинг

- Shared scoring helper with deterministic inputs.
- Scenario fixtures for packet-loss and partition profiles.
- Replay diagnostic helper for convergence events.

#### Тяжелый рефакторинг

- Property tests for CBBA convergence under arbitrary message loss.
- Multi-agent scaling benchmark harness.
- Hierarchical coordination integration tests.

## Вектор 4 - PX4 / SITL Hardening

### Суть

Deepen the local PX4/SIH workflow after M58/M59/M60.

This is not hardware readiness. It is stronger local evidence that the
supervisor can orchestrate live PX4/SIH instances under more conditions.

### What exists

- single-agent PX4/SIH execute artifact;
- multi-agent PX4/SIH execute artifact;
- controlled local failure/reallocation artifact;
- output directory/run-id/force behavior;
- structured reports and replay summaries.

### Possible work

1. Broader failure matrix:
   - fail before upload;
   - fail after upload before start;
   - fail during mission;
   - fail after completing one task;
   - survivor failure after replacement.

2. Repeated failure recovery:
   - not just one failed agent;
   - bounded number of replacements;
   - final status distinguishes partial success from total failure.

3. PX4 orchestration helper:
   - optional local script to launch two SIH instances;
   - capture logs consistently;
   - not default CI.

4. Telemetry robustness:
   - no-progress timeout tuning;
   - heartbeat disconnect classification;
   - mission-current/reached event correlation.

5. Artifact validator:
   - validate run-report/event-log/manifest/replay-summary consistency.

### Польза

- Strengthens the non-toy claim of the project.
- Helps avoid one-off manual evidence.
- Improves confidence in supervisor behavior.

### Риски

- Slow and brittle.
- Local PX4 tooling can be machine-dependent.
- Does not itself improve simulation realism or algorithms.

### Автоматические тесты

#### Без рефакторинга

- Fake live controller failures for each failure timing.
- Report schema tests for partial success/failure.
- Event-log summary tests for repeated reallocation.
- Artifact consistency tests over committed small fixtures.

#### Легкий рефакторинг

- Shared fake live-controller scenario builder.
- Artifact validator library function.
- Timeout classification helper with deterministic inputs.

#### Тяжелый рефакторинг

- Ignored/manual PX4/SIH integration tests.
- Local PX4 launch harness with log capture.
- Multi-attempt supervisor runner for repeated failure cases.

## Вектор 5 - Benchmark / Research Report

### Суть

Turn simulation and SITL evidence into a stronger research artifact.

This should happen after either Algorithm Depth or Urban/New Mission work;
otherwise the benchmark mostly measures the current abstract scenario set.

### Possible work

1. 1000-seed benchmark runs for supported mission/strategy pairs.
2. Confidence intervals for major metrics.
3. Degradation curves:
   - packet loss;
   - latency;
   - number of agents;
   - map size;
   - task density;
   - failure count.
4. Strategy comparison report:
   - where CBBA wins;
   - where greedy is enough;
   - where centralized is unrealistic but useful as an oracle.
5. Explicit exclusion of unsupported pairs from success claims.

### Польза

- Strong analytical value.
- Useful for publication-like claims.
- Forces honest support matrix and metric interpretation.

### Риски

- Expensive runs can hide weak scenario semantics.
- Without confidence tooling, large runs may still be hard to interpret.

### Автоматические тесты

#### Без рефакторинга

- Existing benchmark export tests.
- Manifest/report identity tests.
- Baseline comparison tests.
- Regression runner default suite.

#### Легкий рефакторинг

- Confidence interval helper tests.
- Benchmark pack validation helper.
- Summary table consistency tests.

#### Тяжелый рефакторинг

- Statistical delta report validation.
- Multi-pack comparison tooling.
- Long-run reproducibility harness.

## Вектор 6 - Replay / Analysis

### Суть

Improve observability without building a visual UI.

This vector is useful as support infrastructure for Urban, Algorithm Depth,
PX4/SITL, and Benchmark work.

### Possible work

1. Route traces in replay:
   - per-agent pose by tick;
   - task/route assignment changes;
   - safety/judge events.

2. Textual map diagnostics:
   - route segments;
   - violation locations;
   - blocked edges;
   - detection events.

3. Timeline summaries:
   - when tasks were assigned;
   - when reallocated;
   - when a detector fired;
   - when planner replanned.

4. CSV export for analysis:
   - per-tick agent positions;
   - per-event category counts;
   - per-run decision metrics.

### Польза

- Makes bugs and behavior easier to inspect.
- Helps review new missions.
- Avoids building GUI/visualization too early.

### Риски

- Can become tooling-only work if not tied to a mission or algorithm change.
- Replay schema changes require compatibility discipline.

### Автоматические тесты

#### Без рефакторинга

- Replay event roundtrip tests.
- Summary output tests for new event categories.
- CSV header/row tests for per-tick traces.

#### Легкий рефакторинг

- Shared event-summary formatter.
- Compact route-trace fixture.
- Compatibility fixture for old replay logs.

#### Тяжелый рефакторинг

- Versioned replay schema migration tests.
- Large replay performance tests.
- Cross-run replay diff tooling.

## Recommended Path

### Recommended immediate sequence

1. **Evidence / Cleanup pass.**
   Keep this short. The goal is not to spend weeks on docs, but to avoid
   carrying stale benchmark/status claims into a new large milestone.

2. **Urban Patrol v0.**
   Add a road-graph based urban mission with buildings as no-fly zones and an
   independent judge. This is the highest-value next capability because it
   moves simulation toward realistic task structure while staying deterministic
   and portable.

3. **Urban Search v1.**
   Add the bus detector as a mocked perception block. This introduces
   decision-making based on sensor events without requiring real CV or physics.

4. **Replay / Analysis support.**
   Add route traces and decision summaries as needed for Urban Patrol/Search.
   Do not build a visualizer yet.

5. **Algorithm Depth or Benchmark.**
   After Urban mission exists, algorithm improvements and larger benchmark
   runs become more meaningful.

### Why not start with PX4 hardening?

PX4 hardening is useful, but it improves live workflow reliability rather than
the intelligence of simulated mission behavior. If the goal is to get closer to
real tasks, Urban Navigation gives more leverage.

### Why not start with pure Algorithm Depth?

Algorithm Depth is valuable, but current missions can be too abstract to make
algorithm differences convincing. A more realistic mission substrate gives
better pressure for algorithm work.

### Why not start with full lidar/polygon physics?

That would create too much complexity too early. The project needs route and
decision semantics first:

- road graph;
- constrained movement;
- judge;
- replay;
- metrics.

Only after that should polygon geometry, lidar raycast, and dynamic obstacle
avoidance be added.

## Decision Matrix

| Vector | User-visible value | Research value | Code risk | Runtime cost | Best timing |
|---|---:|---:|---:|---:|---|
| Evidence / Cleanup | Medium | High | Low | Medium | Now |
| Urban Navigation / Search | High | High | Medium | Low-Medium | Next major |
| New Mission generic | Medium | Medium | Medium | Low | If Urban is deferred |
| Algorithm Depth | Medium | High | Medium-High | Medium | After/alongside Urban |
| PX4 / SITL Hardening | Medium | Medium | High | High | When live workflow is priority |
| Benchmark / Research Report | Medium | High | Low-Medium | High | After stronger missions/algorithms |
| Replay / Analysis | Medium | Medium | Low-Medium | Low | Supporting track |

## Suggested Milestone Split

### M63 candidate - Urban Patrol v0

Outcome:

- `urban_patrol` scenario loads and runs.
- One-drone patrol completes a road-graph loop.
- Buildings/no-fly zones are judged independently.
- Replay and metrics explain the run.

### M64 candidate - Urban Search v1

Outcome:

- Bus entity and detector mock exist.
- Search stops on detection.
- Success semantics: detected target, no judge violation.
- Regression smoke covers deterministic detection.

### M65 candidate - Urban Multi-Agent / Avoidance v2

Outcome:

- Two or more drones share the same urban map.
- Separation and route conflicts are measured.
- Replan/wait/yield policy exists for blocked routes.

### M66 candidate - Research Benchmark Refresh

Outcome:

- Benchmark includes Urban Patrol/Search.
- Strategy comparison becomes more meaningful.
- Docs/report distinguish simulation, SITL, and unsupported claims.

## Final Recommendation

The most useful path is:

```text
short cleanup -> Urban Patrol -> Urban Search -> replay/analysis -> algorithm depth -> benchmark
```

This keeps the project aligned with its current architecture:

- PX4 remains the execution layer for real SITL experiments.
- The Rust workspace remains the mission, coordination, simulation, replay,
  and metrics layer.
- Perception and physical reality are mocked in controlled, testable blocks.
- The project gains a more realistic mission without pretending to solve
  hardware safety or full autonomous flight.
