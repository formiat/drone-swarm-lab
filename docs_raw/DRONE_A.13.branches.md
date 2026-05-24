# DRONE_A.13.branches — Итоговые ветки дальнейшего развития

Дата фиксации: 2026-05-23

## Контекст

Этот документ собирает итоговый вариант после сравнения:

- `docs_raw/DRONE_A.12.md`;
- `docs_raw/DRONE_B.12.md`.

Публикационную упаковку, release polishing и "причесывание" пока откладываем. Вопрос сейчас не в том,
как сделать проект красивым для внешней аудитории, а в том, куда технически двигать ядро дальше.

Текущая база уже не пустая:

- есть runtime и модель миссий;
- есть Mission DSL;
- есть несколько стратегий allocation;
- есть Safety Layer;
- есть SAR v2;
- есть CBBA robustness;
- есть Infrastructure Inspection;
- есть benchmark pack;
- есть replay/debuggability;
- есть mock SITL;
- есть экспериментальный real PX4/SITL scaffold.

Поэтому дальше возможны не одна, а несколько осмысленных веток развития. Часть из них независимая,
часть лучше делать только после исправления текущих methodological bugs.

## Короткий вывод

Сейчас есть один обязательный общий ствол:

> исправить correctness / metrics / mission semantics на уже существующих миссиях.

После него остаются несколько равноправных направлений:

1. углублять алгоритмы;
2. добавлять новую миссию;
3. усиливать реализм симуляции;
4. двигать real SITL / PX4;
5. делать visualization / operator tooling;
6. готовить research benchmark depth;
7. превращать проект в расширяемую платформу/API.

Если не хочется выбирать стратегическую ветку прямо сейчас, рациональный ход — сначала пройти общий
ствол correctness/semantics. Он нужен почти для всех следующих направлений.

## Ветка 1 — Correctness / Metrics / Mission Semantics

Статус:

> обязательный общий ствол, а не опциональная ветка.

Суть:

> убрать методологические дыры в текущих миссиях, стратегиях и метриках до добавления нового слоя сложности.

Что сюда входит:

- SAR `grid_cell` handling для CBBA и centralized;
- inspection success metric;
- противоречие вида `success=0.0` при `edge_coverage=1.0`;
- единая семантика completion для разных типов задач;
- явное различение task kinds:
  - coverage cell;
  - SAR scan;
  - SAR confirmation scan;
  - inspection edge;
  - relay placement;
  - waypoint;
- support matrix для mission/strategy пар;
- явные статусы:
  - stable;
  - experimental;
  - unsupported;
  - known limitation;
- regression tests для уже известных странных benchmark outcomes.

Почему это важно:

- benchmark сравнение стратегий сейчас может быть методологически нечистым;
- часть провалов выглядит не как "алгоритм плохой", а как "алгоритм не понимает тип задачи";
- новые миссии, SITL или UI поверх некорректных semantics будут только маскировать проблему;
- это самый дешёвый способ повысить инженерную ценность уже написанного кода.

Где пригодится:

- SAR benchmark;
- inspection benchmark;
- comparison allocator strategies;
- future simulation realism;
- future SITL route, потому что реальные waypoint uploads не должны строиться из неверной mission semantics;
- future visualization, потому что UI должен показывать корректный смысл задачи и успеха.

Риски:

- можно обнаружить, что часть текущих стратегий вообще не должна считаться поддерживающей некоторые миссии;
- после честной support matrix часть красивых таблиц может выглядеть хуже;
- но это полезный риск: лучше явно знать границы системы.

Минимальный результат:

- SAR + CBBA/centralized либо работают объяснимо, либо явно помечены как unsupported/experimental;
- inspection success metric согласован с `edge_coverage`;
- в документации больше нет "0% success без причины";
- benchmark report различает algorithm failure, unsupported semantics и metric bug.

## Ветка 2 — Planner / Algorithm Quality

Статус:

> делать после correctness/semantics.

Суть:

> улучшать качество планирования, перераспределения и координации, когда смысл задач уже стабилен.

Что сюда входит:

- route ordering beyond nearest-neighbour;
- 2-opt или похожая локальная оптимизация для bundles;
- mission-specific route cost;
- feasibility checks:
  - battery;
  - return-to-base;
  - max range;
  - task deadlines;
  - safety constraints;
- dynamic reallocation on agent failure;
- partial replanning без полного перезапуска всего allocation;
- более честная CBBA convergence model;
- communication-aware allocation;
- hierarchical coordination:
  - local group leaders;
  - global coordinator;
  - ограниченный обмен сообщениями между группами.

Почему это важно:

- текущий проект уже умеет запускать несколько стратегий, но ещё не раскрывает сильную алгоритмическую глубину;
- dynamic reallocation и communication-aware allocation ближе к настоящим multi-agent systems;
- это направление делает проект сильнее как research prototype, а не просто как scenario runner.

Где пригодится:

- SAR с отказами агентов;
- inspection missions с длинными периметрами;
- emergency mesh при degraded network;
- будущие wildfire/flood mapping сценарии;
- stress benchmarks.

Риски:

- если делать до semantics, можно оптимизировать неправильную модель;
- алгоритмические изменения быстро усложняют тестирование;
- понадобится больше property/regression тестов, иначе улучшения будут нестабильными.

Минимальный результат:

- есть хотя бы один новый planner mode, который измеримо улучшает route metrics;
- dynamic reallocation покрыт тестами;
- benchmark показывает не только success rate, но и route quality / message cost / convergence behavior.

## Ветка 3 — New Mission

Статус:

> самостоятельная ветка, но лучше после correctness/semantics.

Суть:

> добавить новую миссию, чтобы проверить, что DSL и allocation ядро действительно обобщаются, а не заточены под текущие сценарии.

Кандидаты:

1. **Wildfire / flood mapping**

   Динамическая карта угрозы, агенты обследуют зоны, карта обновляется со временем, приоритеты задач меняются.

   Почему хороший первый кандидат:

   - естественно продолжает SAR/coverage;
   - использует BeliefMap-подобную модель;
   - требует dynamic reprioritization;
   - хорошо связывается с safety/risk;
   - полезен для research benchmark.

2. **Multi-target pursuit**

   Движущиеся цели, агенты должны догонять/сопровождать/перехватывать.

   Плюсы:

   - добавляет time-dependent tasks;
   - хорошо проверяет replanning;
   - подходит для алгоритмической глубины.

   Минусы:

   - потребует более серьёзной динамики целей;
   - может быстро потянуть simulation realism.

3. **Logistics / delivery**

   Pickup/dropoff, зависимости между задачами, capacity, deadlines.

   Плюсы:

   - добавляет precedence constraints;
   - хорошо проверяет task graph semantics;
   - полезно для platform/API direction.

   Минусы:

   - меньше связано с текущими SAR/inspection наработками;
   - может потребовать новый класс planner logic.

Рекомендованный первый вариант:

> Wildfire / flood mapping.

Почему:

- это наиболее близкое расширение текущей базы;
- оно проверит DSL, BeliefMap, safety, allocation и replay одновременно;
- оно меньше ломает предметную область проекта, чем logistics;
- оно менее требовательно к физике, чем pursuit.

Минимальный результат:

- одна новая миссия в DSL;
- сценарий small/medium;
- baseline benchmark по текущим стратегиям;
- replay-compatible events;
- regression tests на генерацию задач и completion semantics.

## Ветка 4 — Simulation Realism

Статус:

> полезно, но не раньше correctness/semantics.

Суть:

> сделать симуляцию менее игрушечной, не превращая проект сразу в полноценный физический симулятор.

Что сюда входит:

- altitude / `Pose3`;
- compatibility для старых 2D сценариев;
- battery model v2:
  - hover cost;
  - climb cost;
  - cruise cost;
  - return-to-base reserve;
- sensor model:
  - range;
  - field-of-view;
  - altitude-dependent detection probability;
  - false positives/false negatives;
- communication model:
  - latency;
  - jitter;
  - packet loss;
  - partitions;
- environment profiles:
  - wind;
  - visibility;
  - blocked/no-fly regions;
  - time-varying hazard zones.

Почему это важно:

- повышает ценность симулятора как mission-level digital twin;
- делает benchmark менее абстрактным;
- позволяет проверять устойчивость алгоритмов в более реалистичных условиях.

Риски:

- резко увеличивает пространство параметров;
- может сделать результаты хуже воспроизводимыми;
- без хорошего regression harness сложно понять, что именно сломалось.

Минимальный результат:

- старые 2D сценарии продолжают работать;
- есть один realism-enabled сценарий;
- есть тесты на battery/sensor/environment invariants;
- benchmark report явно показывает, какие realism features включены.

## Ветка 5 — Real SITL / PX4 Bridge

Статус:

> важная ветка, если хотим идти в сторону робототехнического workflow; не обязательна для ближайшего research-core цикла.

Суть:

> подключить настоящий `MavlinkTransport` в `sitl_agent` и получить end-to-end путь до PX4 SITL.

Что есть сейчас:

- mock SITL работает;
- real PX4 path экспериментальный;
- CLI принимает connection-like параметры, но полный real transport workflow ещё не является продуктовым.

Что сюда входит:

- настоящий mission upload в PX4;
- telemetry -> runtime status;
- arm/takeoff/execute для одного агента;
- корректный abort/failure path;
- validation before upload;
- safety constraints before upload;
- single-agent SITL golden path;
- позже multi-agent SITL.

Почему это важно:

- проверяет, что runtime можно связать с внешним execution backend;
- даёт мост к робототехнической демонстрации;
- выявляет несовпадения между симуляционной моделью и PX4 workflow.

Риски:

- требует внешнего окружения;
- тесты сложнее сделать portable;
- без strong safety и mission semantics это будет demo path, а не надёжная система.

Минимальный результат:

- documented PX4 SITL setup;
- один агент получает waypoints и проходит golden path;
- ошибки подключения и upload failures обрабатываются typed errors;
- mock path остаётся основным portable test backend.

## Ветка 6 — Visualization / Operator Tooling

Статус:

> полезно для debugging и демонстраций, но не главный next step, если polishing отложен.

Суть:

> сделать миссии и replay видимыми.

Что сюда входит:

- interactive replay UI;
- timeline;
- grid/map view;
- BeliefMap overlay;
- InspectionGraph overlay;
- agent positions;
- task state;
- CBBA bundle/conflict state;
- network/failure events;
- comparison viewer для стратегий.

Почему это важно:

- ускоряет расследование странных benchmark outcomes;
- помогает понимать mission behavior без чтения JSON/CSV;
- делает проект удобнее для демонстрации;
- полезно перед публикацией или external-user stage.

Риски:

- легко уйти в UI-polishing, который пользователь сейчас явно хочет отложить;
- без стабильных replay schemas UI придётся часто переделывать.

Минимальный результат:

- один replay viewer для существующих logs;
- отображение agents/tasks/timeline;
- поддержка SAR/inspection-specific overlays;
- UI не становится обязательным для headless benchmark path.

## Ветка 7 — Research Benchmark Depth

Статус:

> отложить до исправления correctness, но держать как будущую ветку.

Суть:

> не добавлять новые фичи, а сделать сильный доказательный benchmark artifact.

Что сюда входит:

- 1000-seed или больше long-run suites;
- confidence intervals;
- degradation curves;
- failure taxonomy;
- benchmark regression thresholds;
- reproducible result packs;
- scripts для regeneration;
- comparison against baselines;
- report explaining algorithm tradeoffs.

Почему это важно:

- превращает проект в исследовательский артефакт;
- показывает, где стратегии реально выигрывают;
- создаёт материал для статьи/отчёта;
- помогает не спорить на уровне отдельных smoke runs.

Риски:

- близко к публикационной стадии, которую сейчас откладываем;
- long-runs дорого поддерживать, если semantics ещё меняются.

Минимальный результат:

- стабильный benchmark command;
- сохранённый result pack;
- thresholds для regression detection;
- понятный report format.

## Ветка 8 — Platform / API Extensibility

Статус:

> полезно позже, если проект должен стать площадкой для внешних стратегий/миссий.

Суть:

> снизить стоимость добавления новых стратегий, миссий и experiment harnesses.

Что сюда входит:

- stable internal APIs;
- documented extension points;
- strategy registration;
- scenario generators;
- validation hooks;
- external strategy harness;
- reusable mission adapters;
- stable schema/versioning for scenario and replay files.

Почему это важно:

- делает проект удобным для других разработчиков;
- уменьшает количество hardcoded assumptions;
- помогает добавлять новые миссии без переписывания runner/benchmark каждый раз.

Риски:

- может превратиться в абстрактное платформостроение;
- преждевременная стабильная API может зацементировать неправильную mission model.

Минимальный результат:

- один documented path для новой стратегии;
- один documented path для новой миссии;
- schema compatibility policy;
- tests на extension points.

## Зависимости между ветками

```text
Correctness / Metrics / Semantics
  -> Planner / Algorithm Quality
  -> New Mission
  -> Research Benchmark Depth
  -> Simulation Realism

Replay schema stability
  -> Visualization / Operator Tooling

Mission semantics + safety clarity
  -> Real SITL / PX4 Bridge

Correctness + repeated extension experience
  -> Platform / API Extensibility
```

Более коротко:

- **сначала correctness/semantics** почти в любом сценарии;
- **planner/algorithm quality** лучше делать после semantics;
- **новую миссию** можно делать независимо, но полезнее после semantics;
- **simulation realism** лучше после regression harness;
- **real SITL** можно делать отдельно, но он дороже и менее portable;
- **visualization** полезна, но сейчас не должна вытеснять core work;
- **research benchmark depth** имеет смысл после исправления методологических дыр.

## Тестовые последствия по веткам

### Tests that need no refactoring

Эти тесты можно планировать вместе с ближайшими изменениями:

- regression tests для SAR + CBBA/centralized;
- regression tests для inspection success metric;
- tests на support matrix и unsupported combinations;
- snapshot/serialization tests для новых mission task kinds;
- CLI smoke tests для benchmark/replay paths;
- unit tests для route cost и completion conditions.

### Tests that need light refactoring

Эти тесты потребуют небольшой подготовки:

- shared fixtures/builders для mission tasks разных типов;
- reusable benchmark assertions для success/coverage/metric consistency;
- fake network profiles для CBBA stress;
- common replay event assertions;
- mock SITL fixtures for transport boundary.

### Tests that need heavy refactoring

Эти тесты станут реалистичными после архитектурных изменений:

- property tests для distributed CBBA under partitions/failures;
- multi-agent SITL integration tests;
- long-run benchmark regression tests with stored baselines;
- UI/replay rendering tests;
- external strategy harness compatibility tests.

## Итоговая рекомендация по веткам

Если выбирать прямо сейчас, я бы не выбирал между SITL, UI, новой миссией и simulation realism.

Я бы сначала сделал общий ствол:

> Correctness / Metrics / Mission Semantics.

После него естественное продолжение:

1. Planner / Algorithm Quality.
2. New Mission, лучше Wildfire / flood mapping.
3. Stress/benchmark depth или Simulation Realism в зависимости от того, что покажут результаты.

SITL и Visualization пока оставить как боковые ветки:

- SITL — если цель смещается к robotics workflow;
- Visualization — если становится больно анализировать replay/benchmark вручную.
