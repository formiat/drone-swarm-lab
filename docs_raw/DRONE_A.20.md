# DRONE_A.20 - Возможные векторы развития проекта после M57-M62

Дата: 2026-05-31

Контекст: M57-M62 в текущем scope закрыты. У проекта уже есть simulation core,
mission DSL, allocator/planner слой, regression/benchmark infrastructure,
replay/reporting, local PX4/SIH single-agent и multi-agent evidence, а также
controlled live failure/reallocation artifact.

Этот документ не выбирает единственный обязательный путь. Он фиксирует
несколько реалистичных векторов развития и рекомендуемый порядок, если цель -
двигаться без реального физического hardware, но повышать прикладную ценность
проекта.

---

## Главный принцип

Проекту не нужно превращаться в замену PX4 или в полноценный физический
симулятор дрона.

Правильное разделение слоев:

| Слой | Где должен жить |
|---|---|
| Стабилизация, attitude/rate control, motor physics | PX4 / autopilot |
| Следование waypoint-миссии | PX4 / autopilot |
| Реальное распознавание препятствий, lidar, SLAM, object detection | внешний perception stack |
| Карта, миссионные ограничения, allowed/forbidden zones | этот проект |
| Mission-level route planning | этот проект |
| Mission-level decision logic | этот проект |
| Геометрический simulation judge | этот проект |
| Multi-agent coordination, task allocation, reallocation | этот проект |

Иными словами: проект не должен делать low-level flight control. Но он вполне
может и должен развивать mission-level navigation: как построить маршрут, как
учесть карту, как проверить физическую валидность маршрута, как принять решение
по mock perception event, как распределить зоны между агентами, как
перераспределить задачи при потере агента.

---

## Текущее состояние как стартовая точка

Сильные стороны:

- сценарии и mission DSL уже есть;
- `TaskKind`, `MissionAdapter`, `AdapterRegistry` уже есть;
- allocation/planning слой уже есть;
- simulation runner и metrics/report export уже есть;
- replay/event log уже есть;
- M61 extension guide уже описывает, как добавлять миссии, стратегии и метрики;
- local PX4/SIH workflow уже доказан для single-agent, multi-agent execute и
  controlled failure/reallocation;
- benchmark baseline на 500 seeds уже есть.

Ограничения:

- нет полноценной карты как набора полигонов/коридоров;
- нет path planner по геометрической среде;
- нет continuous collision judge;
- нет mock perception interface как first-class concept;
- нет dynamic semantic objects вроде bus;
- нет новой прикладной миссии после extension guide;
- flood до сих пор не реализован как отдельная миссия, а wording в части
  документации/кода все еще исторически смешивает wildfire/flood;
- benchmark refresh был сделан до новой mission-realism ветки, поэтому он
  валиден как baseline текущего HEAD, но не как финальная исследовательская
  картина будущих миссий.

---

## Вектор 1 - Disaster Mapping Cleanup / Wildfire v2 Hardening

### Суть

Закрыть старый долг из disaster mapping ветки: привести wildfire/flood wording,
priority semantics и success semantics к честному состоянию.

Это не самый интересный новый функционал, но это хороший короткий cleanup перед
крупной новой веткой. Он уменьшает расхождение между README, docs и кодом.

### Что сделать

1. Принять решение по flood:
   - вариант A: явно убрать flood из обещаний и оставить как future work;
   - вариант B: сделать minimal flood mission.
2. Если выбираем A:
   - поправить README quick start и status wording;
   - поправить doc comments вроде "wildfire / flood mapping";
   - зафиксировать в `docs/STATUS.md`, что flood out-of-scope/future.
3. Если выбираем B:
   - добавить `FloodConfig`;
   - добавить flood scenarios;
   - добавить metrics для flooded/critical zones;
   - добавить replay events;
   - добавить regression smoke.
4. Проверить wildfire priority:
   - сейчас priority уже влияет на `WildfireAdapter::score`;
   - можно добавить configurable `priority_weight`, если нужен отдельный
     исследовательский knob;
   - добавить тест, что high-priority mapping zone выигрывает при равном
     расстоянии.
5. Уточнить success semantics wildfire:
   - small-static;
   - medium-dynamic;
   - realism profile.

### Что даст

- меньше исторического шума;
- честный status по flood;
- более надежную disaster mapping ветку;
- более чистую базу перед новым mission type.

### Где пригодится

- перед публикацией README/status;
- перед новым benchmark refresh;
- перед Urban Navigation, если хотим не тащить старые "wildfire/flood" долги.

### Риски

- minimal flood может расползтись в отдельную крупную миссию;
- `priority_weight` может создать новый параметр без реальной исследовательской
  пользы, если не делать сравнение в benchmark.

### Рекомендация

Сейчас выбрать вариант A: cleanup и future-work wording. Minimal flood делать
только если disaster mapping снова станет основным направлением.

---

## Вектор 2 - Urban Navigation / Mission Realism

### Суть

Добавить миссии, где дрон не просто посещает абстрактные точки, а движется в
карте с ограничениями:

- квартал задан polygon;
- здания/статические препятствия заданы forbidden polygons;
- дороги/воздушные коридоры заданы allowed areas;
- дрон летит на фиксированной высоте;
- collision проверяется геометрическим judge;
- perception блоки сначала mock-овые: obstacle detector, bus detector.

Это не low-level autopilot и не замена PX4. Это mission-level navigation и
decision logic.

### Почему это хорошо вписывается

Текущие миссии в основном имеют вид:

- посетить точки;
- покрыть зоны;
- просканировать SAR клетки;
- пройти inspection edges;
- сопоставить tasks и agents.

Urban Navigation добавляет недостающий слой:

- карта как среда;
- валидность маршрута;
- collision/no-collision;
- allowed/forbidden geometry;
- реакция на mock perception event.

Это напрямую повышает реализм simulation layer, но не требует real hardware,
Gazebo или визуализации.

### Возможная структура

#### Urban Map DSL

Новые сущности:

- `UrbanMap`;
- `Polygon`;
- `ForbiddenZone`;
- `AllowedCorridor`;
- `StaticObstacle`;
- `DynamicObject`;
- `PatrolRoute`.

Минимально:

```text
urban_map:
  frame: local_xy
  altitude_m: 30
  boundary: polygon
  allowed_corridors: [polygon]
  forbidden_zones: [polygon]
```

В Rust это может быть отдельный модуль в `swarm-sim` или `swarm-types`, но не
сразу публичный crate.

#### Geometric Judge

Нужен deterministic judge:

- segment пересек forbidden polygon -> collision;
- point вне allowed corridor -> violation;
- distance до другого drone < min_separation -> separation violation;
- route полностью внутри allowed area -> valid.

На первом этапе можно использовать простую собственную 2D geometry реализацию
или аккуратно взять lightweight crate, если он уже совместим с workspace.

#### Urban Patrol v1

Задача: "облети квартал".

В simulation terms:

- построить patrol route вокруг boundary/corridor;
- пройти route без collision;
- completion: все route segments пройдены;
- failure: collision или leaving allowed area;
- metrics: route_completed, collision_count, forbidden_zone_violations,
  route_length, completion_tick.

#### Urban Patrol v2 / Mock Lidar

Добавить perception boundary:

- `ObstacleDetector` trait;
- mock detector на основе карты;
- event: obstacle_detected;
- policy: stop, replan, mark blocked, choose alternate corridor.

Важно: detector не должен претендовать на настоящий lidar. Это mock perception
interface, чтобы проверить decision logic.

#### Bus Search

Задача: "облетай квартал пока не встретишь автобус".

Новые элементы:

- dynamic semantic object: `Bus`;
- bus route/schedule по дороге;
- `BusDetector` trait;
- detection range/cone;
- mission completion: bus detected;
- metrics: time_to_detect_bus, false_detections, route_length_before_detection,
  collision_count.

### Что даст

- реалистичную прикладную миссию без hardware;
- первый нормальный layer для route validity;
- основу для mock perception;
- понятные failure modes;
- хорошие метрики для benchmark;
- будущую возможность export route -> PX4 waypoints.

### Где пригодится

- городское патрулирование;
- inspection над дорожными коридорами;
- search by semantic object;
- multi-agent deconfliction;
- будущий SITL export.

### Что не делать

- не делать физику моторов;
- не делать настоящий lidar;
- не делать SLAM;
- не делать collision avoidance как certified safety layer;
- не обещать hardware readiness;
- не начинать с PX4. Сначала pure simulation.

### Риски

- geometry может разрастись;
- route planner может стать отдельным большим проектом;
- mock perception легко перепутать с реальным sensor stack;
- без хорошего replay/debug output сценарии будет сложно анализировать.

### Рекомендация

Это лучший основной следующий вектор. Он дает проекту "реальность" на уровне
миссии, не ломая архитектурную границу с PX4.

---

## Вектор 3 - New Mission / Domain Expansion

### Суть

Добавить принципиально новый mission type через extension path из M61.

Кандидаты:

1. **Urban Patrol / Bus Search** - практичная mission-realism ветка.
2. **Logistics / Delivery** - pickup/dropoff, precedence constraints, capacity.
3. **Pursuit** - moving targets, intercept/escort.

### Urban Patrol как New Mission

Плюсы:

- наиболее прикладная;
- хорошо связана с картами и физической валидностью маршрута;
- может использовать mock perception;
- естественно связана с будущим PX4 waypoint export.

Минусы:

- требует geometry/map/judge layer;
- шире, чем простой `TaskKind`.

### Logistics / Delivery

Плюсы:

- хорошо проверяет DSL и task dependencies;
- требует capacity/deadline semantics;
- не требует geometry так сильно, как Urban Patrol.

Минусы:

- менее дроновая/физическая;
- больше про task constraints, чем про движение в среде.

### Pursuit

Плюсы:

- динамические цели;
- проверяет reactive planning.

Минусы:

- может превратиться в toy chase model;
- без geometry/perception выглядит менее реалистично, чем Urban Patrol.

### Рекомендация

Выбрать Urban Patrol как M63-like New Mission. Logistics оставить хорошим
следующим кандидатом, если после Urban Navigation понадобится проверить
precedence/capacity constraints.

---

## Вектор 4 - Replay / Debuggability / Analysis

### Суть

Улучшить объяснимость результатов без полноценной визуализации.

Это особенно важно для Urban Navigation: если нет UI и "мультика", нужны
хорошие textual/JSON artifacts.

### Что сделать

1. Route trace export:
   - planned route;
   - executed route;
   - per-segment status.
2. Collision report:
   - collision point;
   - obstacle id;
   - segment id;
   - tick.
3. Judge summary:
   - forbidden zone violations;
   - allowed corridor exits;
   - min separation over run;
   - dynamic object detections.
4. Replay summary extensions:
   - urban route completed;
   - obstacle detected;
   - bus detected;
   - route replanned;
   - collision.
5. ASCII/table output:
   - not full visualization;
   - enough to inspect timeline and route status.

### Что даст

- легче debug-ить новые миссии;
- проще сравнивать runs;
- лучше артефакты для benchmark;
- меньше зависимости от UI.

### Где пригодится

- Urban Patrol;
- Bus Search;
- failure/reallocation traces;
- benchmark interpretation.

### Рекомендация

Делать не отдельным огромным этапом, а сразу рядом с Urban Navigation. Каждая
новая mission feature должна иметь replay/report representation.

---

## Вектор 5 - Algorithm Depth

### Суть

Улучшать не миссии, а сами алгоритмы и стратегии.

Возможные направления:

- communication-aware allocation;
- message budget как cost/penalty;
- mission-specific planner modes;
- better CBBA/greedy comparison;
- hierarchical coordination для 8+ agents;
- more robust dynamic reallocation;
- multi-failure recovery;
- local deconfliction между drones.

### Что сделать

1. Communication-aware allocation:
   - добавить `message_budget`;
   - учитывать network/packet loss в score;
   - метрики: message_count, success_under_loss, allocation_latency.
2. Mission-specific planners:
   - SAR uncertainty-aware planner;
   - wildfire priority-aware planner;
   - urban corridor-aware planner.
3. Hierarchical coordination:
   - cluster agents;
   - local coordinator per cluster;
   - compare against flat CBBA/greedy.
4. Reallocation depth:
   - repeated failures;
   - partial completion before failure;
   - reassignment under connectivity constraints.

### Что даст

- настоящие algorithmic claims;
- более сильный benchmark;
- возможность показать, где одна стратегия лучше другой.

### Риски

- без новых realistic missions улучшения могут быть абстрактными;
- алгоритмы могут усложниться без видимой пользовательской ценности;
- нужен хороший benchmark, иначе преимущества трудно доказать.

### Рекомендация

Делать после Urban Navigation v1/v2. Тогда алгоритмы будут улучшаться на более
реалистичной среде, а не только на старых point/zone scenarios.

---

## Вектор 6 - Benchmark / Research Evidence

### Суть

Повысить строгость исследовательских claims.

Что уже есть:

- 500-seed release baseline;
- regression determinism sweeps;
- benchmark export/reporting.

Что можно добавить:

- 1000-seed publication-like run;
- confidence intervals;
- degradation curves;
- sweeps по packet loss, agent count, map size;
- strategy comparison report.

### Что сделать

1. Определить supported mission-strategy pairs.
2. Исключить/пометить unsupported pairs.
3. Добавить statistical summary:
   - mean;
   - stddev;
   - confidence interval;
   - min/max;
   - failure rate.
4. Добавить degradation suites:
   - packet loss;
   - number of agents;
   - obstacle density для Urban Navigation;
   - dynamic object frequency для Bus Search.
5. Обновить `docs/BENCHMARK_RESULTS.md`.

### Что даст

- более серьезные research claims;
- доказательства улучшений алгоритмов;
- понятную картину сильных/слабых мест.

### Риски

- прогоны дорогие по времени;
- benchmark без новых миссий/алгоритмов может быть малоинформативным;
- можно легко получить "большую таблицу", но не получить новое понимание.

### Рекомендация

Не делать следующим крупным этапом. Делать после Urban Navigation и/или
Algorithm Depth. 500 seeds достаточно как validation baseline, 1000 seeds -
только перед publication-like claims.

---

## Вектор 7 - PX4/SITL Hardening

### Суть

Дальше развивать real SITL путь, но без hardware.

Что уже сделано:

- single-agent PX4/SIH;
- multi-agent PX4/SIH execute;
- controlled failure/reallocation;
- output dirs, run ids, reports, replay summaries;
- artifact discipline.

Что можно добавить:

- repeated M58/M59 runs;
- broader failure matrix;
- automated local scripts;
- PX4 version matrix;
- maybe Gazebo, если понадобится;
- better troubleshooting docs.

### Что даст

- больше уверенности в SITL workflow;
- меньше ручной возни;
- более надежные artifacts.

### Риски

- diminishing returns: основной PX4/SITL gap уже закрыт;
- можно потратить много времени на инфраструктуру вместо новых mission/algorithm
  capabilities;
- Gazebo легко расширит scope.

### Рекомендация

Делать точечно, когда это нужно для конкретной mission evidence. Не выбирать
как главный следующий вектор прямо сейчас.

---

## Вектор 8 - Platform/API Packaging

### Суть

Превратить stable-ish extension guide в более строгий plugin/API boundary.

Варианты:

- выделить отдельные public crates;
- сделать examples для внешней mission/strategy;
- стабилизировать schema compatibility tests;
- подготовить crate publishing checklist.

### Что даст

- внешним пользователям проще добавлять миссии/стратегии;
- меньше coupling между crates;
- понятнее долгосрочная архитектура.

### Риски

- преждевременная стабилизация API;
- много организационной работы;
- мало новой исследовательской ценности.

### Рекомендация

Пока не делать. M61 extension guide достаточно хорош для in-repository work.
Вернуться к этому после одной реальной новой миссии через extension path.

---

## Рекомендуемый порядок

Если нужен линейный путь без выбора большого направления прямо сейчас:

1. **Disaster Mapping Cleanup**
   - убрать/уточнить flood wording;
   - зафиксировать flood as future work;
   - добавить точечные wildfire priority/success tests.
2. **Urban Map DSL**
   - polygons;
   - allowed/forbidden zones;
   - fixed altitude;
   - simple local XY frame.
3. **Geometric Judge**
   - segment vs polygon;
   - allowed corridor validation;
   - collision metrics.
4. **Urban Patrol v1**
   - "облети квартал";
   - route generation;
   - route completion;
   - collision/no-collision outcome.
5. **Urban Patrol v2 / Mock Perception**
   - obstacle detector trait;
   - obstacle events;
   - stop/replan/mark blocked policy.
6. **Bus Search**
   - dynamic bus object;
   - bus detector trait;
   - patrol until bus detected;
   - time_to_detect_bus metric.
7. **Replay/Analysis for Urban Missions**
   - route trace;
   - collision report;
   - detection timeline.
8. **Algorithm Depth**
   - urban-aware planner;
   - communication-aware allocation;
   - multi-agent deconfliction/reallocation.
9. **Benchmark Refresh**
   - 500-seed validation after new mission;
   - 1000-seed only for publication-like claims.

---

## Тестовая стратегия

### Тесты без рефакторинга

- Scenario DSL parse/validation для новых fixtures.
- `TaskKind`/`MissionAdapter` unit tests.
- Route/collision judge deterministic unit tests.
- Replay event serialization roundtrip.
- Report/export header tests for new metrics.
- Regression smoke for small Urban Patrol scenario.
- Existing SITL and benchmark smoke remain green.

### Тесты с легким рефакторингом

- Geometry fixture helpers for polygons/corridors.
- Urban map builder fixtures.
- Route assertion helpers:
  - segment stays inside allowed area;
  - segment avoids forbidden zones;
  - route visits required patrol points.
- Mock perception fixtures:
  - obstacle visible/invisible;
  - bus detected/not detected.
- Mission outcome helpers:
  - completed;
  - collision;
  - bus_found;
  - timeout.

### Тесты с тяжелым рефакторингом

- Property tests for random polygon/corridor layouts.
- Multi-agent deconfliction property tests.
- Dynamic object schedule property tests.
- Comparative benchmark validation for urban planners.
- PX4/SIH export validation for generated urban waypoint routes.

---

## Итоговая рекомендация

Лучший следующий основной вектор: **Urban Navigation / Mission Realism**.

Почему:

- он не конфликтует с PX4/SITL направлением;
- он не требует hardware;
- он не дублирует PX4 low-level control;
- он добавляет реализм на уровне миссии;
- он использует M61 extension path на практике;
- он даст более содержательную новую миссию, чем абстрактный Pursuit;
- он подготовит почву для будущих algorithm depth и benchmark claims.

Короткая формула:

```text
Не пишем свой PX4.
Пишем mission-level карту, route planning, mock perception, judge и decision logic.
```

Так проект останется исследовательским симулятором координации и принятия
решений, но станет ближе к реальным задачам дронов.
