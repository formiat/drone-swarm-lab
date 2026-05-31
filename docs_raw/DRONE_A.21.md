# DRONE_A.21 - Итоговый набор вариантов развития после сравнения A/B/C

Дата: 2026-05-31

Основа: сравнение `docs_raw/DRONE_A.20.md`, `docs_raw/DRONE_B.20.md` и
`docs_raw/DRONE_C.20.md`.

Цель документа: зафиксировать не один жесткий roadmap, а итоговый набор
реалистичных направлений развития проекта после M57-M62, с рекомендуемым
порядком, критериями выбора, границами scope и тестовой стратегией.

---

## Краткий вывод

Лучший общий план как документ - `DRONE_C.20.md`.

Почему:

- он лучше всего балансирует cleanup/evidence, новую миссию, replay,
  algorithm depth, SITL hardening и benchmark;
- он не прыгает сразу в большую новую разработку, а сначала предлагает короткий
  стабилизирующий pass;
- он правильно предлагает начинать Urban Navigation с road graph и AABB
  buildings, а не с arbitrary polygons, raycast lidar и физики;
- он формулирует полезные milestone: Urban Patrol v0, Urban Search v1, Urban
  Multi-Agent / Avoidance v2, Benchmark Refresh.

Сильная часть `DRONE_A.20.md` - архитектурная граница:

```text
Не пишем свой PX4.
Пишем mission-level карту, route planning, mock perception, judge и decision logic.
```

Эту мысль нужно оставить как основной принцип проекта.

Сильная часть `DRONE_B.20.md` - конкретика по алгоритмам и исследовательским
разрывам:

- communication-aware allocation;
- mission-specific planners;
- CBBA convergence;
- SAR/wildfire benchmark interpretation;
- support matrix;
- confidence/statistical reporting.

Итоговая рекомендация:

```text
Evidence cleanup
-> Urban Patrol v0
-> Urban Search v1
-> Replay/Analysis for Urban
-> Urban Multi-Agent / Dynamic Avoidance
-> Algorithm Depth
-> Benchmark Refresh
-> branch decision: Logistics, Pursuit, PX4 hardening, API packaging
```

---

## Архитектурный принцип

Проект не должен становиться:

- заменой PX4;
- low-level flight controller;
- motor/attitude/physics simulator;
- SLAM/CV/lidar stack;
- hardware safety layer;
- визуальным симулятором ради визуального демо.

Проект должен развивать то, что находится выше автопилота:

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

Это означает: если задача звучит как "облети квартал", проект должен решать не
"как стабилизировать дрон в воздухе", а:

- как представить квартал как карту;
- какие области разрешены/запрещены;
- как построить mission-level маршрут;
- как проверить, что маршрут не пересекает запретные зоны;
- как симулировать обнаружение события, например автобуса;
- как записать replay и метрики;
- как распределить маршрут между несколькими агентами;
- как перекинуть работу при потере агента.

---

## Текущее состояние

После M57-M62 проект имеет хорошую foundation-базу:

- deterministic simulation core;
- scenario/mission DSL;
- `TaskKind`, `MissionAdapter`, `AdapterRegistry`;
- allocator/planner layer;
- runtime/reallocation logic;
- replay/event log/report export;
- benchmark/regression infrastructure;
- M61 extension guide;
- local PX4/SIH single-agent evidence;
- local PX4/SIH multi-agent execute evidence;
- controlled PX4/SIH failure/reallocation evidence;
- 500-seed release benchmark baseline.

При этом проект все еще ограничен:

- текущие simulation missions достаточно абстрактны;
- movement в основном direct-to-task, а не route-through-constrained-space;
- safety checks есть, но нет полноценного independent geometric judge для
  route validity;
- нет road graph/navmesh/polygon map model;
- нет first-class mock perception layer для obstacles/objects;
- нет semantic dynamic objects вроде bus;
- benchmark baseline не доказывает будущие Urban/Navigation claims;
- wildfire/flood wording и wildfire success semantics еще требуют cleanup, если
  disaster mapping остается user-facing claim.

---

## Как сравниваются A/B/C

### DRONE_A.20

Сильные стороны:

- хорошо формулирует архитектурную границу между PX4 и project layer;
- уверенно выбирает Urban Navigation / Mission Realism как лучший основной
  следующий вектор;
- правильно связывает route planning, mock perception, judge и decision logic;
- хорошо объясняет, почему это не конфликтует с текущим PX4/SITL путем.

Слабые стороны:

- слишком быстро предлагает polygons/corridors как starting point;
- меньше конкретики по algorithm gaps;
- evidence/cleanup слой есть, но менее четко оформлен как отдельный нулевой
  шаг;
- benchmark/research часть более общая, чем в B/C.

Что взять:

- главный архитектурный принцип;
- упор на Urban Navigation как основной next major vector;
- идею "облети квартал" и "облетай квартал пока не встретишь автобус";
- запрет на преждевременный full lidar/SLAM/physics scope.

### DRONE_B.20

Сильные стороны:

- больше всего конкретики по алгоритмическим разрывам;
- хорошо описаны communication-aware allocation, mission-specific planners,
  CBBA convergence и scale 8+/16+ agents;
- хорошо отмечены benchmark interpretation gaps: SAR success, wildfire success,
  CBBA coverage failure;
- полезно разделены разные приоритеты: алгоритмы, новая миссия, SITL, demo.

Слабые стороны:

- часть SITL-долгов уже устарела после последующих исправлений;
- слишком много внимания может уйти в algorithm depth до появления более
  реалистичной миссии;
- Perimeter Patrol описан как наиболее простой new mission, но без достаточного
  separation между road graph, judge, mock perception и future polygons.

Что взять:

- algorithm workstream;
- benchmark interpretation tasks;
- support matrix discipline;
- Logistics и Pursuit как последующие альтернативные ветки;
- идею не делать 1000-seed run до закрытия интерпретационных вопросов.

### DRONE_C.20

Сильные стороны:

- лучший баланс между всеми направлениями;
- Evidence / Cleanup вынесен как короткий подготовительный этап;
- Urban Navigation начинается с road graph и AABB buildings, что снижает риск
  geometry scope creep;
- есть clear milestone split: U1, U2, U3, M66-like benchmark refresh;
- тестовая стратегия дана по категориям;
- хорошо объяснено, почему не начинать с PX4 hardening, pure algorithm depth
  или full lidar/polygon physics.

Слабые стороны:

- часть текста на английском, если нужен единый русскоязычный стиль;
- Platform/API packaging почти не раскрыт;
- Disaster Mapping cleanup можно чуть сильнее связать с текущими
  README/status claims;
- Algorithm Depth стоит дополнить конкретикой из B.

Что взять:

- базовую структуру итогового плана;
- Evidence / Cleanup как Vector 0;
- Urban Patrol v0 -> Urban Search v1 -> Dynamic Avoidance v2;
- Decision Matrix / Suggested Milestone Split;
- road graph first, polygons later.

---

## Итоговый набор вариантов

Ниже не "все что когда-нибудь можно сделать", а набор направлений, которые
реально вытекают из текущего состояния проекта.

## Вариант 0 - Evidence / Cleanup

### Суть

Короткий стабилизирующий проход перед новой большой разработкой.

Цель не в том, чтобы "нализывать документацию", а в том, чтобы не тащить
устаревшие claims в следующий этап. Если сейчас начать Urban Navigation,
Algorithm Depth или Benchmark Refresh поверх рассинхронизированных README/status
claims, позже будет сложнее понять, что является фактом, а что историческим
артефактом.

### Что сделать

1. Проверить, что `docs/STATUS.md`, README и `docs/BENCHMARK_RESULTS.md`
   одинаково описывают текущее состояние:
   - M57-M62 закрыты;
   - M58/M59 имеют local PX4/SIH artifacts;
   - hardware/HIL не обещаны;
   - 500-seed baseline является текущим baseline, если simulation-affecting код
     после него не менялся.
2. Зафиксировать flood scope:
   - либо flood явно future/out-of-scope;
   - либо заводить отдельный minimal flood plan.
3. Зафиксировать wildfire success/completion semantics:
   - что считается success;
   - чем success отличается от completion;
   - почему benchmark numbers выглядят именно так.
4. Проверить replay artifacts M58/M59:
   - событие replacement mission;
   - completion seq для replacement tasks;
   - summary/replay не противоречат report.
5. Определить, что старые benchmark artifacts являются:
   - current evidence;
   - или historical evidence, если HEAD уже существенно изменился.

### Что даст

- чистую стартовую точку перед Urban Navigation;
- меньше риска ложных README/status claims;
- лучшее доверие к benchmark и SITL evidence;
- меньше вопросов при review будущих изменений.

### Где пригодится

- перед новой mission family;
- перед benchmark refresh;
- перед внешним README/publication pass;
- перед сравнением стратегий.

### Non-goals

- не запускать 1000 seeds просто ради числа;
- не приводить проект к publication polish;
- не делать hardware claims;
- не делать новый flood, если он не выбран как отдельная ветка.

### Done criteria

- README/status/benchmark docs не противоречат друг другу;
- flood либо реализован, либо явно future/out-of-scope;
- wildfire success semantics покрыты тестом или явно документированы рядом с
  метриками;
- M58/M59 replay/report artifacts проходят smoke validation;
- после cleanup понятно, какие claims являются current evidence.

### Автоматические тесты

#### Тесты без рефакторинга

- Docs smoke test на обязательные status/limitation фразы.
- Wildfire success/completion test на маленьком deterministic scenario.
- Replay summary test для M58/M59 event categories.
- Benchmark manifest/report identity smoke.

#### Тесты с легким рефакторингом

- Shared helper для проверки benchmark pack metadata.
- Shared docs assertion helper для README/status/benchmark consistency.
- Small wildfire fixture с явным expected success threshold.

#### Тесты с тяжелым рефакторингом

- Versioned artifact validator для historical/current benchmark packs.
- Cross-document status schema вместо свободных markdown tables.

### Рекомендация

Сделать первым коротким milestone. Не растягивать. Его смысл - расчистить
доказательную базу перед содержательной новой миссией.

---

## Вариант 1 - Urban Navigation / Search

### Суть

Основной рекомендуемый следующий вектор.

Идея:

- "облети квартал";
- затем "облетай квартал пока не встретишь автобус";
- затем multi-agent urban map with conflicts/replans.

Это не физический симулятор и не замена PX4. Это mission-level simulation:
карта известна, маршрут планируется в допустимой среде, judge независимо
проверяет route validity, perception events mock-овые, replay объясняет
решения.

### Почему это лучший следующий major vector

1. Он добавляет прикладной реализм, которого сейчас не хватает.
2. Он проверяет M61 extension path на реальной новой mission family.
3. Он не требует hardware, Gazebo, CV или настоящего lidar.
4. Он создает хорошую основу для algorithm depth:
   - route-aware planning;
   - multi-agent deconfliction;
   - reallocation in constrained map;
   - comms-aware behavior in realistic tasks.
5. Он хорошо стыкуется с будущим PX4 export: route можно превратить в waypoint
   mission, когда simulation semantics уже проверены.

### Почему начинать с road graph, а не polygons

Arbitrary polygons, segment intersection, raycast lidar and navmesh can quickly
turn into отдельный geometry engine. На первом этапе лучше:

- road graph как allowed traversal structure;
- nodes = intersections/route points;
- edges = allowed corridors;
- buildings = AABB no-fly zones;
- patrol route = ordered loop over graph nodes;
- judge проверяет, что агент не ушел с graph/corridor и не задел AABB building.

Это даст mission realism без резкого роста сложности. Polygons можно добавить
позже, когда road graph + judge + replay будут стабильны.

### Milestone U1 - Urban Patrol v0

Goal: один агент облетает квартал по route graph loop без нарушения map
constraints.

Scope:

1. Добавить `urban_patrol` scenario type/profile.
2. Добавить map model:
   - local XY frame;
   - road graph nodes;
   - road graph edges;
   - optional edge width/corridor width;
   - AABB buildings/no-fly zones;
   - patrol loop.
3. Добавить task generation:
   - route nodes -> mission tasks;
   - deterministic ordering;
   - stable task ids.
4. Добавить route planner:
   - Dijkstra/A* over road graph;
   - deterministic tie-breaking;
   - no dynamic obstacle avoidance yet.
5. Добавить independent judge:
   - route left allowed graph/corridor;
   - entered building/no-fly zone;
   - route incomplete;
   - optional separation check if multi-agent enabled.
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
   - `replan_count` = 0 на v0, но schema-ready.

Non-goals:

- no bus;
- no lidar;
- no dynamic obstacles;
- no polygons/navmesh unless needed for minimal AABB checks;
- no PX4 requirement;
- no visualization requirement.

Done criteria:

- small urban patrol scenario проходит deterministic simulation;
- invalid route через building ловится judge;
- replay/report показывают route, completion и violations;
- regression smoke portable;
- README/status явно отмечают, что это simulation-only mission.

### Milestone U2 - Urban Search v1

Goal: агент патрулирует квартал до обнаружения автобуса.

Scope:

1. Добавить `Bus` как semantic object:
   - id;
   - route node или edge position;
   - static position in v1 или deterministic route schedule;
   - appearance tick.
2. Добавить `BusDetector` mock:
   - detection range;
   - deterministic seed;
   - detection probability;
   - false positive rate;
   - no real CV.
3. Добавить mission policy:
   - patrol loop continues until bus detected;
   - detection completes mission;
   - optional report/stop action;
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

Non-goals:

- no camera simulation;
- no object detector model beyond deterministic mock;
- no real bus physics;
- no pursuit/intercept logic.

Done criteria:

- bus present -> deterministic detection under controlled seed;
- bus absent -> no false detection under zero false-positive profile;
- false-positive profile produces recorded false-positive events;
- mission success means bus detected and no judge violation;
- replay/report explain why run succeeded or failed.

### Milestone U3 - Urban Multi-Agent / Dynamic Avoidance v2

Goal: несколько агентов работают на одной urban map, избегая conflicts and
blocked routes на mission-decision level.

Scope:

1. Multi-agent patrol partition:
   - split loop by segments;
   - avoid duplicate ownership;
   - preserve stable task ids.
2. Temporary obstacles:
   - blocked edge;
   - blocked node;
   - appearance/disappearance tick.
3. Mock obstacle detector:
   - range over graph;
   - detects blocked edge ahead;
   - deterministic events.
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

Non-goals:

- no certified collision avoidance;
- no continuous local obstacle avoidance;
- no real lidar;
- no guarantee that behavior transfers to hardware.

Done criteria:

- two-agent urban patrol completes on simple map;
- blocked edge triggers replan or wait according to policy;
- duplicate ownership is prevented or detected;
- separation conflicts are visible in metrics/replay;
- failure/reallocation works with urban tasks at simulation level.

### Где это пригодится

- urban patrol;
- infrastructure inspection;
- perimeter monitoring;
- semantic search by object;
- multi-agent deconfliction;
- future PX4 waypoint export;
- benchmark scenarios that are less abstract than point/zone tasks.

### Риски

- geometry scope creep;
- mock perception can be mistaken for real perception;
- route planner can grow into a separate project;
- without replay/analysis, failures will be hard to interpret.

### Автоматические тесты

#### Тесты без рефакторинга

- Road graph DSL parse/validation from inline fixture.
- Deterministic route planner on a square block.
- Urban patrol completes valid loop.
- Judge reports no violation for valid route.
- Judge reports building/no-fly violation for invalid route.
- Replay serialization roundtrip for urban events.
- Bus detector deterministic detection/no-detection tests.
- Regression smoke for `urban_patrol_small`.

#### Тесты с легким рефакторингом

- Urban map builder fixtures.
- Shared route assertion helper.
- Shared judge assertion helper.
- Metrics assertion helper for urban outcomes.
- Deterministic mock detector fixture.

#### Тесты с тяжелым рефакторингом

- Polygon geometry property tests.
- Segment-vs-polygon intersection tests.
- Multi-agent route conflict property tests.
- Dynamic obstacle schedule property tests.
- Large urban map benchmark validation.

### Рекомендация

Выбрать как основной следующий major vector.

---

## Вариант 2 - Replay / Analysis

### Суть

Улучшить observability без GUI/visualization.

Urban Navigation, Algorithm Depth and SITL hardening all need better debugging
artifacts. Если нет визуального симулятора, replay/report должны объяснять:

- что планировалось;
- что было выполнено;
- где случилось нарушение;
- почему был replan;
- какой detector fired;
- какие tasks были reassigned.

### Что сделать

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

### Что даст

- легче debug-ить Urban Patrol/Search;
- benchmark failures become explainable;
- SITL artifacts are easier to review;
- no need to build UI too early.

### Где пригодится

- Urban Navigation;
- M58/M59 style artifacts;
- algorithm convergence analysis;
- benchmark report interpretation;
- support matrix decisions.

### Риски

- can become tooling-only work;
- replay schema changes require compatibility discipline;
- too much trace data can make reports noisy.

### Автоматические тесты

#### Тесты без рефакторинга

- Replay event roundtrip tests.
- Timeline formatting tests for small fixture.
- CSV header/row tests for new exports.
- Summary output tests for new event categories.

#### Тесты с легким рефакторингом

- Shared event-summary formatter.
- Compact route trace fixture.
- Compatibility fixture for old replay logs.

#### Тесты с тяжелым рефакторингом

- Versioned replay schema migration tests.
- Large replay performance tests.
- Cross-run replay diff tooling.

### Рекомендация

Не делать отдельной большой веткой перед Urban. Делать как supporting track:
каждая новая Urban feature должна добавлять replay/report representation.

---

## Вариант 3 - Algorithm Depth

### Суть

Углубить интеллект и измеримую дифференциацию стратегий.

Сейчас benchmark показывает, что greedy часто выглядит не хуже более сложных
стратегий. Это может означать:

- greedy действительно достаточно хорош на текущих абстрактных сценариях;
- сценарии не создают нужного давления;
- метрики не различают важные tradeoffs;
- некоторые стратегии еще не используют доступную информацию вроде
  `comms_range`.

### Workstream 3A - Communication-aware allocation

Текущий gap:

- `comms_range` есть у агентов;
- connectivity-aware allocator существует;
- но многие scoring paths ведут себя так, будто communication cost отсутствует;
- scout tasks в connectivity-aware path частично уходят в обычный greedy.

Что сделать:

1. Добавить communication penalty/message budget в scoring.
2. Penalize assignments outside reliable communication range.
3. Compare greedy/auction/connectivity-aware under:
   - packet loss;
   - partition-prone profiles;
   - urban map segmentation;
   - agent failure/reallocation.
4. Report tradeoff:
   - success;
   - message count;
   - agent availability;
   - completion under loss.

### Workstream 3B - Mission-specific planners

Текущий gap:

- многие стратегии используют похожую scoring формулу;
- mission semantics often differ only by coefficients;
- route optimization mostly centralized.

Что сделать:

- SAR: uncertainty/belief-entropy-aware planning;
- Wildfire: priority-triggered reallocation after threat updates;
- Inspection: route optimization for non-centralized strategies;
- Urban: route-graph-aware planner and replan policy.

### Workstream 3C - CBBA convergence and support matrix

Текущий gap:

- часть mission/strategy pairs слабые или unsupported;
- CBBA failures may be inherent, not just bugs;
- benchmark success/completion mismatch needs explanation.

Что сделать:

1. Separate unsupported-by-design from regression.
2. Add replay-driven diagnostics for delayed reconvergence.
3. Experiment with gossip interval and failure-triggered gossip bursts.
4. Re-benchmark CBBA after targeted changes.
5. Update support matrix:
   - supported;
   - experimental;
   - unsupported;
   - supported with caveats.

### Workstream 3D - Scale beyond small swarms

Что сделать:

- add 8-agent and 16-agent profiles;
- measure message count scaling;
- measure conflicts and allocation latency;
- consider hierarchical coordination only if benchmark shows need.

### Что даст

- stronger research story;
- clearer strategy differentiation;
- better benchmark claims;
- more realistic coordination behavior.

### Где пригодится

- after Urban Patrol/Search exists;
- before publication-like benchmark;
- for multi-agent constrained maps;
- for failure/reallocation under network loss.

### Риски

- expensive design without better scenario pressure;
- CBBA fixes can take time and still remain caveated;
- algorithm changes need careful benchmark interpretation.

### Автоматические тесты

#### Тесты без рефакторинга

- Unit tests for communication penalty scoring.
- Tiny controlled profile comparing message counts.
- Support-matrix tests for explicitly unsupported pairs.
- Failure-triggered gossip behavior test if policy is local.

#### Тесты с легким рефакторингом

- Shared scoring helper with deterministic inputs.
- Scenario fixtures for loss/partition profiles.
- Replay diagnostic helper for convergence events.

#### Тесты с тяжелым рефакторингом

- Property tests for CBBA convergence under message loss.
- Multi-agent scaling benchmark harness.
- Hierarchical coordination integration tests.

### Рекомендация

Не начинать с этого как с главного следующего major vector. Лучше сначала
сделать Urban Patrol/Search, чтобы algorithms оптимизировали более реалистичную
среду.

---

## Вариант 4 - Benchmark / Research Evidence

### Суть

Превратить simulation/SITL evidence в более сильный research artifact.

Важно: benchmark имеет смысл делать после того, как появились новые миссии или
алгоритмические изменения. Иначе большой прогон просто подтвердит уже известную
картину старых абстрактных сценариев.

### Что сделать

1. Интерпретировать текущие gaps:
   - SAR success near zero;
   - wildfire success vs completion;
   - CBBA coverage failures;
   - centralized as oracle vs realistic distributed strategies.
2. Define supported mission/strategy pairs.
3. Add confidence intervals:
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
   - bus appearance/detection profile for search.
5. Refresh benchmark:
   - 500 seeds for validation baseline;
   - 1000 seeds only for publication-like claims.

### Что даст

- более честные claims;
- better strategy comparison;
- publication-like artifacts if needed;
- support matrix based on evidence, not intuition.

### Риски

- long runs can hide weak semantics;
- confidence intervals require careful reporting;
- benchmark before new mission/algorithm work gives limited insight.

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

#### Тесты с тяжелым рефакторингом

- Statistical delta report validation.
- Multi-pack comparison tooling.
- Long-run reproducibility harness.

### Рекомендация

Делать после Urban Navigation and/or Algorithm Depth. Не запускать 1000 seeds
как следующий шаг без нового содержательного изменения.

---

## Вариант 5 - PX4 / SITL Hardening

### Суть

Углубить local PX4/SIH workflow, но не идти в реальное железо.

Это направление полезно, но сейчас не должно быть главным, потому что M58/M59
уже закрыли основной live multi-agent/failure foundation. Дальше SITL hardening
дает reliability/evidence, а не новую mission intelligence.

### Что сделать

1. Broader failure matrix:
   - fail before upload;
   - fail after upload before start;
   - fail during mission;
   - fail after partial completion;
   - survivor failure after replacement.
2. Repeated failure recovery:
   - more than one failed agent;
   - bounded replacement attempts;
   - partial success vs total failure.
3. Local harness:
   - script to launch two PX4/SIH instances;
   - port readiness checks;
   - consistent log capture;
   - cleanup on exit.
4. Artifact validator:
   - run report;
   - event log;
   - manifest;
   - replay summary consistency.
5. Telemetry robustness:
   - no-progress timeout tuning;
   - disconnect classification;
   - mission-current/reached correlation.

### Что даст

- more confidence in live workflow;
- easier reproduction of M58/M59-like artifacts;
- less manual setup cost;
- clearer failure taxonomy.

### Риски

- local PX4 tooling can be machine-dependent;
- slow manual runs;
- can consume time without improving simulation realism;
- Gazebo/HIL can expand scope too much.

### Автоматические тесты

#### Тесты без рефакторинга

- Fake live controller failures for each failure timing.
- Report schema tests for partial success/failure.
- Event-log summary tests for repeated reallocation.
- Artifact consistency tests over committed small fixtures.

#### Тесты с легким рефакторингом

- Shared fake controller scenario builder.
- Artifact validator library function.
- Timeout classification helper.

#### Тесты с тяжелым рефакторингом

- Ignored/manual PX4/SIH integration tests.
- Local PX4 launch harness with log capture.
- Multi-attempt supervisor runner for repeated failure cases.

### Рекомендация

Делать точечно, когда новый Urban/mission evidence требует SITL export or live
workflow validation. Не делать главным следующим vector.

---

## Вариант 6 - New Mission Alternatives

### Суть

Если Urban Navigation временно откладывается, есть две сильные alternative
mission families через M61 extension path:

- Logistics / Delivery;
- Multi-target Pursuit.

Обе полезны, но обе хуже Urban как следующий основной шаг, если цель -
приблизиться к реальным drone-like задачам без hardware.

### Logistics / Delivery

Domain:

- pickup/dropoff;
- precedence constraints;
- cargo capacity;
- deadlines/time windows later.

Что проверяет:

- task dependencies;
- stateful agent inventory;
- completion semantics beyond "visit point";
- allocator correctness under constraints.

Metrics:

- `delivery_rate`;
- `late_delivery_count`;
- `capacity_violation_count`;
- `precedence_violation_count`;
- `unserved_delivery_count`;
- `route_cost`.

Риски:

- can become VRP/scheduling project;
- less drone-specific than urban patrol/search;
- requires runtime task dependency tracking.

Когда выбирать:

- если цель - проверить DSL/runtime на dependencies and stateful tasks.

### Multi-target Pursuit

Domain:

- moving targets;
- intercept/escort;
- dynamic target appearance/disappearance;
- capture radius.

Что проверяет:

- reactive allocation;
- dynamic tasks;
- moving target completion semantics;
- auction/CBBA behavior under moving objectives.

Metrics:

- `capture_rate`;
- `time_to_intercept`;
- `target_lost_count`;
- `pursuit_distance`;
- `interception_efficiency`.

Риски:

- can become toy chase model;
- moving target tick semantics can be ambiguous;
- without map/perception layer it may be less realistic than Urban Search.

Когда выбирать:

- если цель - stress-test dynamic task allocation and reactive planning.

### Автоматические тесты

#### Тесты без рефакторинга

- DSL parse/validation for new mission type.
- Task generation tests.
- Completion semantics tests.
- Replay roundtrip tests.
- Small regression smoke.

#### Тесты с легким рефакторингом

- Mission fixture builders.
- Shared dependency/outcome assertion helpers.
- Dynamic target deterministic fixture.

#### Тесты с тяжелым рефакторингом

- Property tests for dependencies/moving targets.
- Multi-seed stability tests.
- Comparative strategy tests across profiles.

### Рекомендация

Оставить как вторую волну после Urban Patrol/Search, либо выбрать вместо Urban
только если хочется именно dependencies или moving targets.

---

## Вариант 7 - Platform / API Packaging

### Суть

Превратить extension guide в более строгую public plugin/API boundary.

### Что сделать

- clarify crate boundaries;
- provide external-ish mission/strategy examples;
- stabilize schema compatibility tests;
- document semantic versioning rules;
- consider publishing-ready crate layout later.

### Что даст

- easier external extension;
- cleaner architecture;
- less coupling between crates.

### Риски

- premature API stabilization;
- lots of organizational work;
- little immediate research value;
- current M61 guide is enough for in-repository missions.

### Автоматические тесты

#### Тесты без рефакторинга

- Extension fixture compile tests.
- Schema compatibility tests for current examples.
- Docs smoke tests for extension guide paths.

#### Тесты с легким рефакторингом

- Shared extension test fixture crate/module.
- Public API boundary compile checks.

#### Тесты с тяжелым рефакторингом

- Workspace split/publishing dry-run checks.
- Semver compatibility test harness.

### Рекомендация

Не делать сейчас. Вернуться после одной реальной новой mission family через
extension path.

---

## Итоговый рекомендуемый порядок

### Phase 1 - Short Foundation Cleanup

1. Evidence / Cleanup.
2. Wildfire/flood wording and success semantics.
3. M58/M59 replay/report smoke validation.

Цель: не начинать крупную новую миссию поверх мутных claims.

### Phase 2 - Urban Mission Core

1. Urban Patrol v0.
2. Road graph map model.
3. AABB buildings/no-fly zones.
4. Independent judge.
5. Replay/report/metrics.

Цель: первая новая прикладная mission family без hardware and without physics
scope creep.

### Phase 3 - Urban Search / Mock Perception

1. Bus entity.
2. BusDetector mock.
3. Search-until-detected policy.
4. Detection metrics and replay events.

Цель: decision logic based on simulated perception event.

### Phase 4 - Urban Multi-Agent / Dynamic Avoidance

1. Multi-agent route partition.
2. Blocked edge / temporary obstacle.
3. Wait/replan/yield policy.
4. Separation/conflict judge.
5. Reallocation integration.

Цель: route-level coordination and deconfliction in constrained map.

### Phase 5 - Algorithm Depth

1. Communication-aware scoring.
2. Urban-aware planning.
3. Mission-specific planners for SAR/wildfire/inspection.
4. CBBA convergence and support matrix.
5. Scale 8/16 agents if benchmark indicates need.

Цель: stronger algorithmic claims on more realistic scenarios.

### Phase 6 - Benchmark Refresh

1. 500-seed validation after new mission/algorithm work.
2. Confidence intervals.
3. Degradation curves.
4. 1000-seed only for publication-like claims.

Цель: research evidence after new behavior exists.

### Phase 7 - Branch Decision

После этого выбирать:

- Logistics / Delivery, если нужны dependencies/capacity;
- Pursuit, если нужны moving targets/reactive allocation;
- PX4/SITL hardening, если нужен deeper live workflow;
- API packaging, если нужен external extension surface;
- publication polish, если проект готовится к публичному релизу.

---

## Decision Matrix

| Вариант | User-visible value | Research value | Code risk | Runtime cost | Best timing |
|---|---:|---:|---:|---:|---|
| Evidence / Cleanup | Medium | High | Low | Low-Medium | Now |
| Urban Navigation / Search | High | High | Medium | Low-Medium | Next major |
| Replay / Analysis | Medium | Medium | Low-Medium | Low | Alongside Urban |
| Algorithm Depth | Medium | High | Medium-High | Medium | After Urban v0/v1 |
| Benchmark / Research Evidence | Medium | High | Low-Medium | High | After new behavior |
| PX4 / SITL Hardening | Medium | Medium | Medium-High | High | When live evidence is priority |
| Logistics / Delivery | Medium | Medium-High | High | Low-Medium | Alternative branch |
| Multi-target Pursuit | Medium | Medium-High | High | Low-Medium | Alternative branch |
| Platform/API Packaging | Low-Medium | Medium | Medium | Low | Later |

---

## Что не делать сейчас

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

Лучший итоговый путь:

```text
short cleanup -> Urban Patrol -> Urban Search -> replay/analysis
-> Urban multi-agent avoidance -> algorithm depth -> benchmark refresh
```

Этот путь сохраняет текущую архитектуру:

- PX4 остается execution/autopilot layer;
- Rust workspace остается simulation, mission, coordination, replay and metrics
  layer;
- physical reality and perception are mocked in explicit, testable interfaces;
- project gains realistic mission structure without pretending to solve
  certified autonomous flight or hardware safety.
