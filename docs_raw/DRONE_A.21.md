# DRONE_A.21 - Линейный milestone-план после сравнения A/B/C

Дата: 2026-05-31

Основа: сравнение `docs_raw/DRONE_A.20.md`, `docs_raw/DRONE_B.20.md` и
`docs_raw/DRONE_C.20.md`.

Этот документ заменяет предыдущую формулировку "набор вариантов" на линейный
план. На текущем этапе выбор фактически сделан: основной путь развития -
Urban Navigation / Search как mission-level realism без реального hardware.

Отложенные темы вроде Logistics, Pursuit, deeper PX4/SITL hardening и API
packaging остаются возможными будущими ответвлениями, но не являются
равноправным выбором прямо сейчас.

---

## Краткий вывод

Дальше проект разумно вести как последовательную цепочку:

```text
M63 Evidence Cleanup
-> M64 Urban Patrol v0
-> M65 Urban Search v1
-> M66 Replay / Analysis for Urban
-> M67 Urban Multi-Agent / Dynamic Avoidance
-> M68 Algorithm Depth on Urban + Existing Missions
-> M69 Benchmark / Research Evidence Refresh
-> M70 Next Branch Decision
```

Главный принцип:

```text
Не пишем свой PX4.
Пишем mission-level карту, route planning, mock perception, judge и decision logic.
```

То есть проект не должен уходить в low-level flight control, motor physics,
SLAM, real lidar, real CV или hardware safety. Он должен развивать слой выше
автопилота: mission semantics, route planning, simulation judge, allocation,
reallocation, replay, metrics and benchmark evidence.

---

## Архитектурная граница

| Слой | Ответственный слой |
|---|---|
| Stabilization, attitude/rate control, motor physics | PX4 / autopilot |
| Waypoint following | PX4 / autopilot |
| Real sensors, lidar, SLAM, object detection | external perception stack |
| Known mission map and constraints | this project |
| Mission-level route planning | this project |
| Mission-level decision logic | this project |
| Mock perception interfaces | this project |
| Independent simulation judge | this project |
| Multi-agent allocation/reallocation | this project |
| Replay/report/benchmark evidence | this project |

Если задача звучит как "облети квартал", проект отвечает на вопросы:

- как представить квартал как deterministic map;
- какие route segments разрешены;
- какие zones/buildings запрещены;
- как построить mission-level маршрут;
- как проверить маршрут независимым judge;
- как симулировать perception event, например обнаружение автобуса;
- как записать replay/report/metrics;
- как распределить route/tasks между агентами;
- как восстановиться после потери агента.

Проект не отвечает на вопросы:

- как стабилизировать дрон;
- как управлять моторами;
- как делать certified obstacle avoidance;
- как обрабатывать real lidar/camera stream;
- как гарантировать safety реального железа.

---

## Текущее состояние перед M63

Что уже есть:

- deterministic simulation foundation;
- scenario/mission DSL;
- `TaskKind`, `MissionAdapter`, `AdapterRegistry`;
- allocator/planner layer;
- runtime task ownership and reallocation;
- replay/event log/report export;
- regression/benchmark infrastructure;
- M61 extension guide;
- local PX4/SIH single-agent evidence;
- local PX4/SIH multi-agent execute evidence;
- controlled PX4/SIH failure/reallocation evidence;
- 500-seed release benchmark baseline.

Что еще слабое:

- simulation missions remain fairly abstract;
- movement is mostly direct-to-task, not route-through-constrained-space;
- safety checks exist, but there is no urban route judge;
- no road graph/navmesh/polygon map model;
- no first-class mock perception layer for obstacles/objects;
- no semantic dynamic object like bus;
- benchmark baseline does not prove future Urban claims;
- wildfire/flood wording and wildfire success semantics still need cleanup if
  disaster mapping remains a user-facing claim.

---

## Что было взято из A/B/C

Из `DRONE_A.20.md` взято:

- архитектурное разделение PX4/autopilot vs mission-level project layer;
- Urban Navigation / Mission Realism как основной следующий путь;
- идея "облети квартал" и "облетай квартал пока не встретишь автобус";
- запрет на преждевременный full lidar/SLAM/physics scope.

Из `DRONE_B.20.md` взято:

- concrete algorithm gaps:
  - communication-aware allocation;
  - mission-specific planners;
  - CBBA convergence;
  - 8/16-agent scaling;
- benchmark interpretation gaps:
  - SAR success;
  - wildfire success/completion;
  - CBBA coverage failures;
- discipline вокруг support matrix and confidence reporting.

Из `DRONE_C.20.md` взято:

- линейная структура;
- short Evidence / Cleanup перед большой разработкой;
- road graph first вместо arbitrary polygons first;
- milestone split:
  - Urban Patrol v0;
  - Urban Search v1;
  - Urban Multi-Agent / Avoidance v2;
  - Benchmark Refresh;
- тестовые категории по required refactoring level.

---

## M63 - Evidence Cleanup

### Цель

Коротко выровнять claims, status and benchmark evidence перед новой большой
разработкой.

Это не publication polish. Это технический sanity pass, чтобы дальнейшие
Urban/Algorithm/Benchmark изменения не строились поверх устаревших или мутных
утверждений.

### Работы

1. Синхронизировать README, `docs/STATUS.md`, `docs/BENCHMARK_RESULTS.md`:
   - M57-M62 закрыты в текущем scope;
   - M58/M59 имеют local PX4/SIH artifacts;
   - hardware/HIL не обещаны;
   - 500-seed baseline является current evidence только если simulation-affecting
     код после него не менялся.

2. Проверить benchmark artifact status:
   - если benchmark соответствует текущему HEAD, оставить как current baseline;
   - если после benchmark были simulation-affecting changes, пометить artifact
     as historical или выполнить refresh позже в M69.

3. Закрыть flood wording:
   - если flood сейчас не реализуем, убрать его из user-facing promises;
   - явно оставить flood как future work;
   - не начинать minimal flood mission внутри M63.

4. Зафиксировать wildfire success/completion semantics:
   - что такое completion;
   - что такое success;
   - почему success может быть ниже completion;
   - какие thresholds используются.

5. Проверить replay/report artifacts M58/M59:
   - replacement mission events;
   - completion seq for replacement tasks;
   - event log vs replay summary vs run report consistency.

### Кодовые изменения

Ожидаются небольшие:

- docs smoke tests;
- wildfire success/completion unit/integration test;
- possibly small replay/artifact validation helper;
- docs/status wording updates.

Большого runtime/refactor изменения быть не должно.

### Прогоны

Минимально:

- targeted tests touched by docs/replay/wildfire checks;
- existing relevant replay/report tests;
- no 500/1000-seed run unless M63 reveals stale baseline.

### Done criteria

- README/status/benchmark docs не противоречат друг другу;
- flood scope explicit: future/out-of-scope, unless separately planned;
- wildfire success/completion semantics tested or documented next to metrics;
- M58/M59 replay/report artifacts pass smoke validation;
- known limitations remain visible and honest.

### Автоматические тесты

#### Тесты без рефакторинга

- Docs smoke test for required status/limitation phrases.
- Wildfire success/completion deterministic test.
- Replay summary test for M58/M59 event categories.
- Benchmark manifest/report identity smoke.

#### Тесты с легким рефакторингом

- Shared helper for benchmark pack metadata validation.
- Shared docs assertion helper for README/status/benchmark consistency.
- Small wildfire fixture with explicit expected success threshold.

#### Тесты с тяжелым рефакторингом

- Versioned artifact validator for current vs historical benchmark packs.
- Cross-document status schema instead of free-form Markdown tables.

---

## M64 - Urban Patrol v0

### Цель

Добавить первую новую прикладную mission family: один агент облетает квартал по
заданной карте, не нарушая route/map constraints.

Это первый содержательный шаг к mission realism. Он не требует hardware,
Gazebo, real lidar, real CV or visualization.

### Scope

1. Добавить `urban_patrol` scenario/profile.

2. Добавить минимальную urban map model:
   - local XY frame;
   - road graph nodes;
   - road graph edges;
   - optional edge/corridor width;
   - AABB buildings/no-fly zones;
   - patrol loop as ordered graph route.

3. Добавить task generation:
   - patrol loop -> tasks/route targets;
   - deterministic ordering;
   - stable task ids;
   - predictable mapping into replay/report.

4. Добавить route planner:
   - Dijkstra or A* over road graph;
   - deterministic tie-breaking;
   - no dynamic obstacles yet;
   - no local obstacle avoidance yet.

5. Добавить independent judge:
   - route left allowed graph/corridor;
   - route entered building/no-fly zone;
   - route incomplete;
   - optional separation check if existing safety layer is easy to reuse.

6. Добавить replay events:
   - `UrbanRoutePlanned`;
   - `UrbanSegmentEntered`;
   - `UrbanViolation`;
   - `UrbanPatrolCompleted`.

7. Добавить metrics:
   - `patrol_completion_rate`;
   - `urban_violation_count`;
   - `route_length`;
   - `route_efficiency`;
   - `time_to_complete_loop`;
   - `replan_count` as schema-ready zero in v0.

### Non-goals

- no bus;
- no lidar;
- no dynamic obstacles;
- no polygons/navmesh unless unavoidable;
- no PX4 requirement;
- no visualization requirement;
- no certified safety claim.

### Кодовые изменения

Ожидаются substantial code changes:

- types/config for urban map and patrol profile;
- scenario parser/builder changes;
- route planner module;
- judge module or extension of existing safety layer;
- mission adapter or task kind additions;
- replay/report schema additions;
- regression scenario.

### Прогоны

Минимально:

- targeted urban unit/integration tests;
- cargo check/test for affected crates;
- one small regression smoke.

Long benchmark не нужен.

### Done criteria

- small urban patrol scenario loads and runs deterministically;
- valid square/loop route completes;
- invalid route through building fails by judge;
- replay/report explain route and violation/completion;
- metrics exported to JSON/CSV/Markdown where applicable;
- docs/status mention simulation-only urban patrol.

### Автоматические тесты

#### Тесты без рефакторинга

- Road graph DSL parse/validation from inline fixture.
- Deterministic route planner on square block.
- Urban patrol completes valid loop.
- Judge reports no violation for valid route.
- Judge reports building/no-fly violation for invalid route.
- Replay serialization roundtrip for urban events.
- Regression smoke for `urban_patrol_small`.

#### Тесты с легким рефакторингом

- Urban map builder fixtures.
- Shared route assertion helper.
- Shared judge assertion helper.
- Metrics assertion helper for urban outcomes.

#### Тесты с тяжелым рефакторингом

- Polygon geometry property tests.
- Segment-vs-polygon intersection tests.
- Large map route planning property tests.
- Multi-agent route conflict property tests.

---

## M65 - Urban Search v1

### Цель

Расширить Urban Patrol до задачи "облетай квартал пока не встретишь автобус".

Это первый step к mock perception and decision logic:

- perception mocked explicitly;
- bus is semantic object;
- mission success depends on detection event and judge constraints;
- no real CV or visual simulation.

### Scope

1. Добавить `Bus` entity:
   - id;
   - static pose or route graph position in v1;
   - optional deterministic route schedule;
   - appearance tick;
   - stable serialization.

2. Добавить `BusDetector` mock:
   - detection range;
   - deterministic seed;
   - detection probability;
   - false positive rate;
   - no image/lidar/CV dependency.

3. Добавить mission policy:
   - patrol loop continues until bus detected;
   - detection completes mission;
   - optional stop/report action;
   - no intercept required in v1.

4. Добавить replay events:
   - `BusObserved`;
   - `BusDetectionFalsePositive`;
   - `UrbanSearchCompleted`;
   - `UrbanSearchTimedOut`.

5. Добавить metrics:
   - `bus_detection_rate`;
   - `time_to_detect_bus`;
   - `false_positive_count`;
   - `distance_before_detection`;
   - `search_success_without_violation`.

### Non-goals

- no real object detection;
- no camera simulation;
- no physical bus model;
- no pursuit/intercept;
- no dynamic obstacle avoidance yet.

### Кодовые изменения

Ожидаются medium-size changes:

- semantic object config;
- mock detector trait or local detector abstraction;
- urban search mission adapter/policy;
- replay/report metrics;
- deterministic tests.

### Прогоны

Минимально:

- targeted urban search tests;
- regression smoke with bus present;
- regression smoke with bus absent;
- no 500-seed benchmark yet.

### Done criteria

- bus present -> deterministic detection under controlled seed;
- bus absent -> no false detection under zero false-positive profile;
- false-positive profile records false-positive events;
- success requires bus detection and no judge violation;
- replay/report explain detection timeline.

### Автоматические тесты

#### Тесты без рефакторинга

- Bus entity DSL parse/validation.
- Bus detector deterministic detection test.
- No-detection test with bus outside range.
- False-positive disabled profile test.
- Urban search completion semantics test.
- Replay roundtrip for bus events.

#### Тесты с легким рефакторингом

- Deterministic detector fixture.
- Shared semantic object scenario builder.
- Mission outcome helper for detected/not detected/timed out.

#### Тесты с тяжелым рефакторингом

- Dynamic bus schedule property tests.
- Detection probability multi-seed stability tests.
- Line-of-sight/raycast tests if later added.

---

## M66 - Replay / Analysis for Urban

### Цель

Сделать urban behavior inspectable without GUI.

После M64/M65 появятся route planning, judge violations and detector events.
Без хорошего replay/report их будет сложно анализировать. M66 превращает эти
события в readable artifacts.

### Scope

1. Route trace:
   - planned route per agent;
   - executed route;
   - segment status;
   - completion/failure reason.

2. Judge summary:
   - violation type;
   - obstacle/building id;
   - segment id;
   - tick/elapsed time;
   - location.

3. Timeline mode:
   - chronological event list;
   - agent-prefixed events;
   - task/reassignment events;
   - detector events;
   - judge events.

4. CSV/JSON analysis export:
   - per-event category counts;
   - per-agent positions if movement trace exists;
   - per-run decision metrics.

5. Replay filters:
   - by agent;
   - by event category;
   - by task id;
   - by violation type.

### Non-goals

- no visual UI;
- no 2D/3D viewer;
- no heavy schema migration unless required;
- no large performance optimization before real bottleneck.

### Кодовые изменения

Ожидаются focused tooling/reporting changes:

- replay CLI/report changes;
- event summary formatter;
- route trace export structures;
- tests for output stability.

### Прогоны

Минимально:

- replay tests;
- route trace fixture tests;
- urban smoke scenario to produce artifact;
- no long benchmark.

### Done criteria

- developer can inspect why urban run succeeded/failed from text artifacts;
- route and violation information is present in replay/report;
- detector events are visible in timeline;
- CSV/JSON exports are stable enough for benchmark post-processing.

### Автоматические тесты

#### Тесты без рефакторинга

- Replay event roundtrip tests.
- Timeline formatting tests for small fixture.
- CSV header/row tests for new exports.
- Summary output tests for urban event categories.

#### Тесты с легким рефакторингом

- Shared event-summary formatter.
- Compact route trace fixture.
- Compatibility fixture for old replay logs.

#### Тесты с тяжелым рефакторингом

- Versioned replay schema migration tests.
- Large replay performance tests.
- Cross-run replay diff tooling.

---

## M67 - Urban Multi-Agent / Dynamic Avoidance

### Цель

Расширить Urban Patrol/Search до нескольких агентов and route-level decision
logic in constrained map.

Это все еще не certified collision avoidance. Это mission-level coordination:
route partition, blocked edges, wait/replan/yield policy, judge and metrics.

### Scope

1. Multi-agent patrol partition:
   - split loop by route segments;
   - avoid duplicate ownership;
   - stable task ids;
   - clear assignment/reassignment events.

2. Temporary obstacles:
   - blocked edge;
   - blocked node;
   - appearance tick;
   - disappearance tick.

3. Mock obstacle detector:
   - graph/range query;
   - detects blocked edge ahead;
   - deterministic output.

4. Policies:
   - wait;
   - replan around blocked edge;
   - yield to another drone;
   - abort segment and reassign if blocked too long.

5. Multi-agent judge:
   - separation breach;
   - route conflict;
   - duplicate segment ownership;
   - unresolved blockage.

6. Metrics:
   - `avoided_collision_count`;
   - `near_miss_count`;
   - `wait_time_ticks`;
   - `replan_success_rate`;
   - `blocked_edge_count`;
   - `duplicate_ownership_count`.

### Non-goals

- no real-time obstacle avoidance;
- no real lidar;
- no hardware safety guarantee;
- no Gazebo dependency;
- no full physics.

### Кодовые изменения

Ожидаются substantial changes:

- multi-agent route ownership model;
- blocked-edge state;
- policy/replan logic;
- judge extensions;
- reallocation integration with urban tasks;
- replay/report events and metrics.

### Прогоны

Минимально:

- targeted multi-agent urban tests;
- regression smoke with two agents;
- blocked-edge deterministic scenario;
- no 500-seed benchmark unless M67 is stable and cheap.

### Done criteria

- two-agent urban patrol completes on simple map;
- blocked edge triggers wait/replan/yield according to policy;
- duplicate ownership is prevented or detected;
- separation conflicts are visible in metrics/replay;
- failure/reallocation works with urban tasks at simulation level.

### Автоматические тесты

#### Тесты без рефакторинга

- Two-agent route partition test.
- Duplicate ownership prevention/detection test.
- Blocked edge replan test.
- Wait policy test.
- Separation conflict judge test.
- Urban reallocation smoke test.

#### Тесты с легким рефакторингом

- Shared multi-agent urban fixture builder.
- Route partition assertion helper.
- Blocked-edge scenario builder.
- Reallocation outcome helper.

#### Тесты с тяжелым рефакторингом

- Multi-agent route conflict property tests.
- Dynamic obstacle schedule property tests.
- Multi-failure urban reallocation tests.
- Larger urban map stress tests.

---

## M68 - Algorithm Depth on Urban + Existing Missions

### Цель

После появления более реалистичной urban mission substrate углубить алгоритмы и
сделать различия между стратегиями измеримыми.

M68 не стоит делать раньше M64/M65, потому что на старых абстрактных сценариях
алгоритмы могут выглядеть слишком похожими.

### Scope

1. Communication-aware allocation:
   - use `comms_range` in scoring where appropriate;
   - optional `comms_penalty_weight`;
   - message budget or communication penalty;
   - compare under packet loss and partition-prone profiles.

2. Urban-aware planning:
   - route cost instead of straight-line distance;
   - blocked edge awareness;
   - replan cost;
   - separation/deconfliction cost.

3. Mission-specific planners:
   - SAR: uncertainty/belief-entropy-aware prioritization;
   - Wildfire: priority-triggered reallocation after threat updates;
   - Inspection: route optimization for non-centralized strategies;
   - Urban: graph-route planner and replan policy.

4. CBBA convergence and support matrix:
   - separate unsupported-by-design from regression;
   - replay-driven diagnostics for delayed reconvergence;
   - gossip interval experiments;
   - failure-triggered gossip burst if justified.

5. Scale experiments:
   - 8-agent profiles;
   - 16-agent profiles if 8-agent results show meaningful pressure;
   - message count scaling;
   - allocation latency;
   - task conflicts.

### Non-goals

- no hierarchical coordinator unless scale experiments justify it;
- no algorithm rewrite before evidence;
- no benchmark-heavy publication claims inside M68.

### Кодовые изменения

Ожидаются medium-to-large changes:

- scoring functions;
- allocator parameters/config;
- mission-specific planner hooks;
- support matrix updates;
- replay diagnostics;
- targeted benchmark profiles.

### Прогоны

Минимально:

- targeted allocator/planner tests;
- small comparative smoke profiles;
- no 1000-seed run;
- optional 50/100 seed exploratory runs if results are needed for direction.

### Done criteria

- communication cost affects allocation in controlled tests;
- urban route cost is used where straight-line distance is misleading;
- at least one mission-specific planner improvement has measurable behavior;
- support matrix clearly marks unsupported/experimental pairs;
- CBBA known gaps are either improved or documented with evidence.

### Автоматические тесты

#### Тесты без рефакторинга

- Unit tests for communication penalty scoring.
- Tiny controlled profile comparing message counts.
- Support-matrix tests for explicitly unsupported pairs.
- Urban route-cost scoring test.
- Failure-triggered gossip behavior test if policy is local.

#### Тесты с легким рефакторингом

- Shared scoring helper with deterministic inputs.
- Scenario fixtures for loss/partition profiles.
- Replay diagnostic helper for convergence events.
- Mission-specific planner fixture helpers.

#### Тесты с тяжелым рефакторингом

- Property tests for CBBA convergence under message loss.
- Multi-agent scaling benchmark harness.
- Hierarchical coordination integration tests if hierarchy is added.

---

## M69 - Benchmark / Research Evidence Refresh

### Цель

После Urban and Algorithm Depth обновить доказательную базу.

До M69 long-run benchmark не должен быть главным milestone: большой прогон
имеет смысл только после появления нового behavior, который стоит измерять.

### Scope

1. Interpret current benchmark gaps:
   - SAR success near zero;
   - wildfire success vs completion;
   - CBBA coverage failures;
   - centralized as oracle vs realistic distributed strategies.

2. Define supported mission/strategy pairs:
   - supported;
   - experimental;
   - unsupported;
   - supported with caveats.

3. Add statistical reporting:
   - mean;
   - stddev;
   - stderr;
   - confidence interval;
   - min/max;
   - failure rate.

4. Add degradation curves:
   - packet loss;
   - latency;
   - number of agents;
   - map size;
   - task density;
   - obstacle density for urban;
   - bus appearance/detection profile.

5. Refresh benchmark artifacts:
   - 500 seeds for validation baseline;
   - 1000 seeds only for publication-like claims.

### Non-goals

- no 1000-seed run by default;
- no publication polish unless explicitly chosen;
- no claims for unsupported pairs.

### Кодовые изменения

Ожидаются moderate tooling/reporting changes:

- confidence interval helpers;
- benchmark pack validation;
- support matrix integration;
- report table updates;
- maybe profile additions.

### Прогоны

Recommended:

- small/medium targeted validation runs first;
- 500-seed release run after behavior stabilizes;
- 1000-seed only if preparing external/publication-like claims.

### Done criteria

- benchmark includes Urban Patrol/Search if those are stable;
- reports distinguish current vs historical evidence;
- confidence/statistical summary exists for key metrics;
- unsupported pairs are excluded from success claims or clearly marked;
- benchmark results are reproducible enough for future comparison.

### Автоматические тесты

#### Тесты без рефакторинга

- Existing benchmark export tests.
- Manifest/report identity tests.
- Baseline comparison smoke.
- Regression runner default suite.

#### Тесты с легким рефакторингом

- Confidence interval helper tests.
- Benchmark pack validation helper.
- Summary table consistency tests.
- Support matrix report tests.

#### Тесты с тяжелым рефакторингом

- Statistical delta report validation.
- Multi-pack comparison tooling.
- Long-run reproducibility harness.

---

## M70 - Next Branch Decision

### Цель

После M63-M69 снова выбрать следующий крупный путь.

На текущем этапе это не нужно решать заранее. M63-M69 дают линейный ствол. M70
существует как deliberate decision point после того, как Urban/Algorithm/Benchmark
дадут новую фактическую картину.

### Возможные решения на M70

1. Logistics / Delivery:
   - pickup/dropoff;
   - precedence constraints;
   - cargo capacity;
   - deadlines/time windows later.

2. Multi-target Pursuit:
   - moving targets;
   - intercept/escort;
   - dynamic target appearance;
   - capture radius.

3. PX4/SITL deeper hardening:
   - broader failure matrix;
   - repeated failures;
   - local launch harness;
   - artifact validator.

4. Platform/API packaging:
   - stricter extension boundary;
   - external-ish mission examples;
   - schema compatibility tests;
   - semver/publishing preparation.

5. Publication/readiness pass:
   - README polish;
   - public examples;
   - stronger benchmark narrative;
   - packaging/release checklist.

### Кодовые изменения

M70 itself is decision/planning. Code changes begin only after choosing the next
post-Urban path.

### Прогоны

No mandatory run. Use M69 artifacts as input.

### Done criteria

- M63-M69 status reviewed against actual code and artifacts;
- next post-Urban path selected explicitly;
- new plan written with code/test/run scope;
- hardware remains out-of-scope unless explicitly revisited.

### Автоматические тесты

#### Тесты без рефакторинга

- None required for decision itself.

#### Тесты с легким рефакторингом

- None required for decision itself.

#### Тесты с тяжелым рефакторингом

- None required for decision itself.

---

## Итоговый линейный порядок

```text
M63 Evidence Cleanup
M64 Urban Patrol v0
M65 Urban Search v1
M66 Replay / Analysis for Urban
M67 Urban Multi-Agent / Dynamic Avoidance
M68 Algorithm Depth on Urban + Existing Missions
M69 Benchmark / Research Evidence Refresh
M70 Next Branch Decision
```

Если нужно укоротить план до ближайших практических задач:

```text
M63 -> M64 -> M65
```

Это достаточный ближайший scope, чтобы проект получил первую реально новую
прикладную mission family without hardware.

---

## Что не делать до M70

- Не идти в real hardware/HIL.
- Не обещать production-grade safety.
- Не писать свой low-level flight controller.
- Не делать full lidar/raycast/SLAM/CV.
- Не начинать с arbitrary polygon/navmesh engine.
- Не строить UI/visualizer как главный milestone.
- Не запускать 1000 seeds без нового содержательного behavior.
- Не стабилизировать публичный API до реальной новой mission family.

---

## Итоговая формулировка

Это уже не список независимых направлений для немедленного выбора.

Это линейный Urban-focused roadmap:

```text
cleanup -> urban patrol -> urban search -> urban replay/analysis
-> urban multi-agent -> algorithm depth -> benchmark refresh -> next decision
```

Смысл плана: сохранить проект как исследовательский simulation/coordination
workspace, но сделать миссии менее абстрактными и ближе к реальным задачам
дронов, не подменяя PX4, hardware safety, real perception or physical flight
control.
