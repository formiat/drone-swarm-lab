# DRONE_C.21 - Линейный roadmap после M57-M62

Дата фиксации: 2026-05-31

Основа: сравнение `DRONE_A.20.md`, `DRONE_B.20.md`,
`DRONE_C.20.md`, текущего локального кода, README/docs и committed result
artifacts.

## Назначение документа

Этот документ заменяет формат "набор вариантов" на линейную цепочку
milestone. Выбор следующего основного направления уже сделан:

```text
evidence cleanup
  -> Urban foundations
  -> Urban Patrol
  -> Urban Search
  -> replay/analysis and multi-agent prep
  -> algorithm depth
  -> benchmark refresh
  -> SITL/platform decision
```

Идея: развивать проект не как замену PX4 и не как физический симулятор, а как
mission-level simulation, planning, coordination, replay and metrics layer.

## Что взято из A/B/C.20

### Из DRONE_A.20

Берем как базовую архитектурную рамку:

- проект не должен реализовывать low-level flight control;
- PX4 остается execution layer for real SITL waypoint workflows;
- этот workspace развивает mission-level navigation, map constraints,
  decision logic, replay, metrics and coordination;
- Urban Navigation / Mission Realism становится главным следующим направлением;
- replay/debuggability нужно делать рядом с новой миссией, а не отдельным
  украшением;
- Platform/API packaging пока рано стабилизировать.

### Из DRONE_B.20

Берем как backlog после появления новой миссии:

- communication-aware scoring;
- mission-specific planners;
- CBBA convergence analysis;
- benchmark interpretation issues;
- local PX4/SIH harness and artifact validation as later supporting work.

Не переносим как открытый долг:

- M59 replacement replay seq bug уже исправлен в последующих коммитах.

### Из DRONE_C.20

Берем как детализацию реализации:

- road-graph-first Urban plan;
- AABB/static obstacle first, polygons later;
- mock perception framing;
- independent judge;
- route trace/replay discipline;
- split Urban Patrol, Urban Search and later multi-agent/deconfliction stages.

## Архитектурная граница

Проект не должен делать:

- motor physics;
- attitude/rate control;
- real lidar processing;
- real object detection;
- SLAM;
- certified collision avoidance;
- hardware readiness claims.

Проект должен делать:

- map-aware mission constraints;
- deterministic route planning over a simplified environment;
- independent simulation judge;
- mock perception events;
- mission-level stop/replan/report decisions;
- multi-agent task allocation and deconfliction at the mission layer;
- replay, reports, metrics and benchmark evidence.

Короткая формула:

```text
Do not build a new PX4.
Build map-aware mission planning, a deterministic judge, mock perception,
replay, metrics, and coordination logic.
```

## Correction: M62 Benchmark Evidence

Текущий 500-seed benchmark artifact:

- artifact: `results/all_500_jobs14_m62_release/`;
- manifest commit: `81260ca7afa114a5d9add7b832f6c5d7875b88cd`;
- observed pre-document HEAD: `f9ed1c399589631e3079f0d31dc01bc999f75892`;
- simulation-affecting code changed after the artifact, including
  `crates/swarm-sim/src/runner.rs`.

Поэтому до нового прогона этот artifact нужно считать historical validation
evidence, not current-HEAD evidence.

## Линейный план

```text
M63 Evidence Cleanup / Status Honesty
  -> M64 Urban Foundations
    -> M65 Urban Patrol v0
      -> M66 Urban Search v1
        -> M67 Urban Replay / Analysis + Multi-Agent Prep
          -> M68 Algorithm Depth On Urban + Existing Missions
            -> M69 Benchmark Refresh / Research Evidence
              -> M70 SITL Export And Platform Boundary Decision
```

M70 является decision milestone: если после M69 нужен live/SITL artifact для
Urban routes, он идет в PX4/SIH direction; если важнее external reuse, он идет
в Platform/API direction. До M70 эти направления не должны отвлекать основной
путь.

---

## M63 - Evidence Cleanup / Status Honesty

### Цель

Создать честную стартовую точку перед новой большой миссией.

M63 короткий, но обязательный. Он закрывает stale claims и старый
wildfire/flood debt, чтобы Urban work не начинался поверх противоречивого
status.

### Что сделать

1. Benchmark status:
   - либо rerun 500-seed release benchmark на текущем HEAD;
   - либо явно маркировать `results/all_500_jobs14_m62_release/` как historical
     evidence for `81260ca...`.

2. Docs/status sync:
   - README;
   - `docs/STATUS.md`;
   - `docs/BENCHMARK_RESULTS.md`;
   - result README/manifest notes if needed.

3. Flood wording cleanup:
   - убрать active flood claims from user-facing docs/comments;
   - оставить flood as future work;
   - не реализовывать flood в M63.

4. Wildfire success semantics:
   - задокументировать exact success predicate;
   - добавить тесты для small-static и medium-dynamic;
   - объяснить success vs task completion mismatch in benchmark docs.

5. M58/M59 artifact sanity:
   - replay summaries parse;
   - expected event categories present;
   - M59 replacement seq semantics stay correct.

### Не делать

- Не начинать Urban implementation before status cleanup.
- Не делать minimal flood mission.
- Не делать 1000-seed publication benchmark.
- Не менять algorithm behavior beyond tiny test/docs fixes.

### Done criteria

- User-facing docs do not claim current benchmark evidence for stale artifacts.
- Flood is clearly future work unless implemented later.
- Wildfire success/completion semantics are documented and tested.
- M58/M59 replay artifacts remain readable and semantically consistent.
- Targeted regression/SITL tests pass.

### Tests

#### Tests that need no refactoring

- Docs smoke tests for required limitation phrases.
- Benchmark manifest identity test.
- Wildfire success semantics tests.
- Replay summary tests for M58/M59 categories.
- Existing SITL supervisor/replay tests.

#### Tests that need light refactoring

- Benchmark-pack validation helper.
- Shared docs/status assertion helper.
- Small wildfire fixture with explicit mapped-ratio expectations.

#### Tests that need heavy refactoring

- Structured status manifest instead of duplicated Markdown claims.
- Historical/current benchmark classifier.

---

## M64 - Urban Foundations

### Цель

Добавить минимальную основу для Urban missions без полноценной геометрии,
лидара, автобуса или multi-agent deconfliction.

M64 должен создать reusable substrate:

- road graph;
- route planner;
- simple map constraints;
- initial judge API;
- scenario DSL fixtures;
- metrics/replay schema placeholders where needed.

### Почему road graph first

Начинать с arbitrary polygons опасно: geometry quickly becomes the project.
Road graph gives practical navigation semantics with much lower risk:

- intersections as nodes;
- road/corridor segments as edges;
- deterministic shortest path;
- route loop;
- AABB buildings/no-fly zones as first static obstacles.

Polygon geometry can be added later after route/judge/replay are stable.

### Что сделать

1. Urban map model:
   - `UrbanMap`;
   - graph nodes with `Pose`;
   - graph edges with id, from, to, cost/length;
   - optional edge metadata such as corridor width;
   - static obstacles as AABB first.

2. Scenario DSL:
   - add an `urban_patrol` fixture or profile;
   - include map, agents, route loop and run config;
   - keep it deterministic and portable.

3. Route planner:
   - Dijkstra or A*;
   - deterministic tie-breaking;
   - route from node to node;
   - route loop expansion into planned segments.

4. Initial judge API:
   - route uses existing graph edges;
   - planned route does not use blocked edges;
   - planned route does not enter AABB obstacles if edge/point check is cheap;
   - report violation type and location.

5. Metrics skeleton:
   - route_length_m;
   - route_planned;
   - urban_violation_count;
   - urban_route_completed initially false/true based on route progress.

### Не делать

- No bus detector.
- No lidar/raycast.
- No dynamic obstacles.
- No multi-agent route conflicts.
- No PX4/SITL export.
- No arbitrary polygon dependency unless a tiny local implementation is enough.

### Done criteria

- Urban fixture loads through scenario catalog.
- Route planner returns a deterministic route on a simple block.
- Invalid map/route inputs are rejected with actionable errors.
- Judge can identify at least one simple invalid route.
- New types are documented enough for M65 implementation.

### Tests

#### Tests that need no refactoring

- Urban DSL parse/validation from inline fixture.
- Road graph node/edge validation.
- Deterministic shortest path unit test.
- Invalid edge/node references are rejected.
- Simple AABB obstacle violation test if implemented in M64.

#### Tests that need light refactoring

- Shared urban fixture builder.
- Route assertion helper.
- Judge assertion helper.
- Scenario catalog helper for urban fixtures.

#### Tests that need heavy refactoring

- Random graph generation with guaranteed route existence.
- Polygon geometry tests.
- Route planner abstraction if multiple planners are supported.

---

## M65 - Urban Patrol v0

### Цель

Реализовать первую полноценную Urban mission:

> One drone patrols a city-block loop and completes it without judge violations.

M65 превращает foundations из M64 в user-visible simulation capability.

### Что сделать

1. Mission semantics:
   - patrol route is an ordered loop over graph nodes or generated waypoints;
   - completion means all required segments/nodes are visited in order or under
     clearly documented rules;
   - failure means timeout or judge violation.

2. Runner integration:
   - agent moves along planned route, not simply direct-to-nearest task if that
     would ignore the road graph;
   - route progress updates each tick;
   - mission can finish before max_ticks.

3. Judge integration:
   - route validity checked before run;
   - execution checked during run where possible;
   - violations recorded with tick, agent, segment/point and reason.

4. Replay/events:
   - `UrbanRoutePlanned`;
   - `UrbanSegmentEntered`;
   - `UrbanSegmentCompleted`;
   - `UrbanViolation`;
   - `UrbanPatrolCompleted`.

5. Metrics:
   - `urban_patrol_completed`;
   - `urban_violation_count`;
   - `route_length_m`;
   - `route_efficiency`;
   - `time_to_complete_loop`;
   - `distance_travelled_m`;
   - `replan_count = 0` in v0.

6. Docs:
   - README/status mention Urban Patrol as simulation-only;
   - scenario docs describe map/route fields;
   - limitations say no lidar, no real obstacle avoidance, no hardware claim.

### Не делать

- No buses.
- No dynamic obstacle avoidance.
- No multi-agent deconfliction.
- No PX4 claim.
- No visual UI.

### Done criteria

- `urban_patrol` small scenario runs and completes.
- An invalid route scenario fails with judge violation.
- Replay summary explains route completion and violations.
- JSON/CSV/Markdown exports include new user-facing metrics if exposed in
  benchmark/report tables.
- Regression smoke is portable and deterministic.

### Tests

#### Tests that need no refactoring

- Urban Patrol success fixture.
- Urban Patrol timeout fixture.
- Judge violation fixture.
- Replay event serialization roundtrip.
- Regression smoke for small Urban Patrol.
- Report/export header tests if metrics are exported.

#### Tests that need light refactoring

- Mission outcome assertion helper:
  - completed;
  - timeout;
  - violation.
- Route progress fixture.
- Replay summary fixture.

#### Tests that need heavy refactoring

- Property tests for random route loops.
- Random map tests where every generated route must remain valid.
- Long-run determinism sweep for urban scenarios.

---

## M66 - Urban Search v1

### Цель

Добавить вторую практическую Urban mission:

> Drone patrols the block until it detects a bus through a mocked detector.

M66 introduces perception-driven mission decision logic without real CV,
camera simulation or physics.

### Что сделать

1. Bus entity:
   - id;
   - pose or graph node/edge position;
   - active_from_tick / active_until_tick optional;
   - static bus first, dynamic route later if still small.

2. Mock detector:
   - detection range;
   - detection probability;
   - false positive rate;
   - deterministic seed;
   - optional field-of-view later.

3. Mission policy:
   - patrol until bus detection;
   - stop/report on confirmed detection;
   - timeout if not detected;
   - no judge violation allowed for success.

4. Replay/events:
   - `BusObserved`;
   - `BusDetected`;
   - `BusFalsePositive`;
   - `UrbanSearchCompleted`;
   - detector/no-detection summary counters.

5. Metrics:
   - `bus_detected`;
   - `time_to_detect_bus`;
   - `false_positive_count`;
   - `distance_before_detection`;
   - `search_success_without_violation`.

6. Docs:
   - explicitly say detector is mocked;
   - no real object recognition claim;
   - no line-of-sight realism unless implemented.

### Не делать

- No image/CV implementation.
- No lidar implementation.
- No realistic bus physics.
- No multi-agent search coordination unless trivial.

### Done criteria

- Deterministic search fixture detects a bus.
- Fixture with out-of-range bus does not detect before timeout.
- False-positive behavior is testable with seed control.
- Success predicate is clear: detected target, no judge violation, within
  timeout.
- Replay summary reports detection events.

### Tests

#### Tests that need no refactoring

- Bus entity parse/validation.
- Detector in-range success test with deterministic probability.
- Detector out-of-range no-detection test.
- False-positive controlled-seed test.
- Urban Search success/timeout fixtures.
- Replay event roundtrip for bus events.

#### Tests that need light refactoring

- Mock detector fixture helper.
- Search outcome assertion helper.
- Deterministic RNG helper for detector tests.

#### Tests that need heavy refactoring

- Dynamic bus schedule property tests.
- Line-of-sight geometry tests.
- Multi-agent search partitioning tests.

---

## M67 - Urban Replay / Analysis + Multi-Agent Prep

### Цель

Сделать Urban runs inspectable from text artifacts and prepare the ground for
multi-agent Urban scenarios.

M67 is not a visualizer milestone. It is a debugging and analysis milestone.

### Что сделать

1. Route trace export:
   - planned route;
   - executed route;
   - per-segment status;
   - per-agent pose trace if cheap enough.

2. Judge report:
   - violation type;
   - point/segment;
   - obstacle id;
   - tick;
   - agent id.

3. Replay timeline:
   - `replay --timeline`;
   - `replay --agent <id>`;
   - optional `replay --category urban`.

4. Multi-agent prep:
   - two-agent urban fixture;
   - separation metrics;
   - route conflict representation;
   - no advanced avoidance policy yet.

5. Reports:
   - route trace path in run report if artifact is written;
   - urban event counts in summary.

### Не делать

- No GUI.
- No Bevy/egui viewer.
- No full traffic simulator.
- No complex avoidance policy.

### Done criteria

- Urban Patrol/Search failures can be diagnosed from replay/report files.
- Timeline output is deterministic and readable.
- Two-agent fixture can at least measure route/separation conflicts.
- Replay schema remains backward-compatible.

### Tests

#### Tests that need no refactoring

- Timeline output fixture.
- Route trace JSON/CSV header test.
- Judge report serialization test.
- Two-agent separation measurement fixture.
- Replay compatibility test for old logs.

#### Tests that need light refactoring

- Shared replay timeline formatter.
- Compact route trace fixture builder.
- Urban summary assertion helper.

#### Tests that need heavy refactoring

- Versioned replay schema migration tests.
- Large replay performance tests.
- Cross-run replay diff tooling.
- Multi-agent deconfliction property tests.

---

## M68 - Algorithm Depth On Urban + Existing Missions

### Цель

После Urban Patrol/Search получить более содержательную среду для algorithmic
improvements and strategy comparison.

M68 должен дать хотя бы одно измеримое улучшение, а не просто новый параметр.

### Что сделать

1. Choose one primary algorithm improvement:
   - urban corridor-aware planner;
   - communication-aware scoring;
   - wildfire priority-triggered reallocation;
   - SAR uncertainty-aware planner.

2. Add benchmark delta:
   - before/after or enabled/disabled comparison;
   - at least one metric where the change should matter;
   - clear interpretation.

3. Support matrix update:
   - stable;
   - experimental;
   - unsupported with reason.

4. CBBA analysis:
   - analyze weak/unsupported pairs with replay;
   - decide inherent vs bug vs parameter issue;
   - try failure-triggered gossip burst only if evidence supports it.

5. Scaling prep:
   - add 8-agent or 16-agent profile only if useful for the chosen algorithm;
   - defer hierarchical coordination until measured need exists.

### Не делать

- Do not add many algorithm knobs without benchmark interpretation.
- Do not start hierarchical coordination before scaling evidence.
- Do not try to fix every weak benchmark row in one milestone.

### Done criteria

- One strategy/planner improvement has measurable benefit.
- Benchmark delta is committed or documented.
- Support matrix is updated.
- Unsupported pairs are not presented as failures without explanation.

### Tests

#### Tests that need no refactoring

- Unit tests for chosen scoring/planner change.
- Regression smoke for affected mission.
- Support matrix tests for explicit unsupported pairs.
- Benchmark delta smoke if cheap.

#### Tests that need light refactoring

- Shared scoring helper.
- Strategy comparison fixture.
- Replay diagnostic helper for convergence events.

#### Tests that need heavy refactoring

- CBBA convergence property tests under arbitrary message loss.
- Multi-agent scaling benchmark harness.
- Hierarchical coordination integration tests.

---

## M69 - Benchmark Refresh / Research Evidence

### Цель

Обновить benchmark evidence after Urban and Algorithm work, with honest
current/historical artifact handling.

M69 should not run before M65/M66 unless the project explicitly wants only a
cleanup benchmark.

### Что сделать

1. Current-head validation:
   - 500-seed release benchmark after Urban/Algorithm changes;
   - manifest commit must match intended code state.

2. Supported-pair matrix:
   - stable;
   - experimental;
   - unsupported with reason.

3. Benchmark interpretation:
   - where greedy is enough;
   - where connectivity-aware or mission-specific planner wins;
   - where centralized is an oracle;
   - where CBBA is unsupported or weak.

4. Optional publication-like run:
   - 1000-seed only if needed;
   - confidence intervals;
   - degradation curves.

5. Degradation suites:
   - packet loss;
   - latency;
   - agent count;
   - urban obstacle density;
   - bus detection probability;
   - failure count.

### Не делать

- Do not mix current and historical evidence.
- Do not include unsupported pairs as success claims.
- Do not use benchmark as substitute for PX4/SITL evidence.

### Done criteria

- Benchmark artifact identity is explicit.
- Docs distinguish current, historical and unsupported evidence.
- Tables include interpretation, not only numbers.
- Regression runner remains green.

### Tests

#### Tests that need no refactoring

- Existing benchmark export tests.
- Manifest/report identity tests.
- Regression runner suite.
- Baseline comparison tests.

#### Tests that need light refactoring

- Confidence interval helper tests.
- Benchmark pack validation helper.
- Summary table consistency tests.

#### Tests that need heavy refactoring

- Statistical delta report validation.
- Multi-pack comparison tooling.
- Long-run reproducibility harness.

---

## M70 - SITL Export And Platform Boundary Decision

### Цель

После Urban and benchmark refresh принять следующий осознанный decision:

1. углублять local PX4/SIH evidence for Urban-generated routes; or
2. стабилизировать platform/API boundary for external mission/strategy work; or
3. continue algorithm depth if Urban benchmark shows clear gaps.

M70 is a decision milestone, not a promise to do all three.

### Option A - Urban route export to SITL/PX4

Scope:

- convert Urban route to waypoint mission;
- validate with existing safety gate;
- run local PX4/SIH upload or execute if practical;
- capture artifact using output-dir/run-id discipline.

Non-goals:

- no hardware;
- no Gazebo gate;
- no real obstacle avoidance claim.

### Option B - PX4/SITL hardening

Scope:

- local integration harness;
- artifact validator;
- broader failure modes;
- replay timeline for SITL events.

### Option C - Platform/API packaging

Scope:

- external-style mission example;
- schema compatibility tests;
- crate boundary review;
- no public semver promise unless explicitly chosen.

### Done criteria

- A concrete next roadmap is selected based on M63-M69 evidence.
- If SITL is chosen, artifact reproducibility improves.
- If platform is chosen, extension path has at least one real mission behind it.
- If algorithm depth continues, benchmark gaps justify it.

### Tests

#### Tests that need no refactoring

- Urban route to waypoint conversion unit test if Option A is chosen.
- Artifact validator fixture if Option B is chosen.
- Extension example compile/test if Option C is chosen.

#### Tests that need light refactoring

- Shared route-to-waypoint conversion helper.
- SITL artifact consistency helper.
- External-style extension fixture.

#### Tests that need heavy refactoring

- Manual/ignored PX4/SIH integration test harness.
- Schema compatibility test suite.
- Multi-crate public API compatibility checks.

---

## Alternatives Deferred

These are not part of the M63-M70 mainline.

### Logistics / Delivery

Useful later if the goal becomes task dependencies, pickup/dropoff, capacity
and deadlines.

Do not choose before Urban because it is less connected to movement in a
physical environment and can become a scheduling/VRP project.

### Multi-target Pursuit

Useful later if the goal becomes dynamic target tracking.

Do not choose before Urban because moving-target semantics are harder to debug
without route trace and replay improvements.

### Full polygon geometry and lidar/raycast

Useful later after graph-based Urban Patrol/Search works.

Do not start here because it risks turning the next milestone into a geometry
or sensor-simulation project.

### Minimal flood mission

Useful only if Disaster Mapping becomes the main product/research direction.

For M63, prefer cleanup and future-work wording.

## Final Recommendation

Execute the milestone chain in order:

```text
M63 Evidence Cleanup / Status Honesty
M64 Urban Foundations
M65 Urban Patrol v0
M66 Urban Search v1
M67 Urban Replay / Analysis + Multi-Agent Prep
M68 Algorithm Depth On Urban + Existing Missions
M69 Benchmark Refresh / Research Evidence
M70 SITL Export And Platform Boundary Decision
```

The next implementation plan should be M63 first, then M64/M65. Urban Patrol
is the main new capability, but it should not start before the short evidence
cleanup because current benchmark/status claims are not fully aligned with the
current code state.
