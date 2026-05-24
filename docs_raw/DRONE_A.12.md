# DRONE_A.12 — Варианты дальнейшего развития без публикационного polishing

Дата фиксации: 2026-05-23

## Короткий вывод

Публикацию, release-упаковку, "причесывание" и "нализывание" сейчас откладываем.

Смотрим не на release-план, а на следующий инженерный цикл.

Текущая база уже широкая:

- runtime;
- Mission DSL;
- safety layer;
- SAR v2;
- CBBA robustness;
- Infrastructure Inspection;
- benchmark pack;
- replay/debuggability;
- mock SITL.

Дальше можно думать двумя способами:

1. **Ветки развития** — если нужно выбрать стратегический фокус.
2. **Линейный план** — если пока не хотим выбирать и хотим усилить ядро.

## Ветки развития

### 1. Algorithm / Mission Correctness

Суть:

> исправлять известные слабые места стратегий на уже существующих миссиях.

Что сюда входит:

- CBBA / centralized на SAR grid tasks;
- CBBA на inspection perimeter;
- mission-specific semantics для `grid_cell`, `edge_id`, route ordering;
- strategy support matrix;
- fallback / recommendation:
  - какая стратегия подходит для какой миссии;
  - какая стратегия experimental;
  - какие комбинации сейчас не поддерживаются.

Зачем:

- сейчас проект умеет сравнивать стратегии, но некоторые комбинации явно слабые;
- это улучшает качество уже реализованного, а не добавляет новую витрину;
- это самый честный следующий инженерный фокус.

Где пригодится:

- SAR benchmark;
- inspection benchmark;
- CBBA research;
- strategy comparison;
- future real-world/SITL route, потому что слабые стратегии нельзя переносить в реальный контур.

### 2. Simulation Realism

Суть:

> сделать симуляцию ближе к реальным миссиям.

Что сюда входит:

- 3D pose / altitude;
- более реалистичная батарея:
  - hover;
  - climb;
  - payload;
  - return-to-base reserve;
- wind / noise;
- sensor range / field-of-view;
- dynamic obstacles;
- лучшее моделирование связи;
- environment profiles.

Зачем:

- повышает ценность симулятора как mission-level digital twin;
- позволяет сравнивать алгоритмы в более реалистичных условиях;
- делает результаты менее игрушечными.

Риск:

- резко увеличивает сложность;
- может размыть фокус, если начать до исправления mission/strategy correctness.

### 3. Real-World / SITL Bridge

Суть:

> двигать PX4 / MAVLink дальше mock/scaffold.

Что сюда входит:

- настоящий PX4 mission upload;
- telemetry -> `TaskStatus`;
- arm / takeoff / execute;
- single-agent SITL;
- multi-agent SITL;
- safety enforcement before upload;
- failure handling на SITL контуре.

Зачем:

- путь к реальным системам;
- проверка transport abstraction;
- проверка, что mission runtime может управлять внешним execution backend.

Риск:

- самая дорогая и хрупкая ветка;
- требует внешнего окружения;
- без сильного safety и mission correctness можно получить демонстрацию, но не надёжную систему.

### 4. Visualization / Operator Tooling

Суть:

> видеть миссию глазами.

Что сюда входит:

- replay UI;
- timeline;
- map/grid view;
- BeliefMap overlay;
- InspectionGraph overlay;
- comparison viewer для стратегий;
- failure/network/CBBA state visualization.

Зачем:

- помогает debugging;
- помогает демонстрации;
- помогает понимать странные benchmark outcomes;
- ускоряет разработку новых сценариев и стратегий.

Риск:

- если сейчас откладываем polishing, это не первый приоритет;
- можно потратить много времени на UI без улучшения ядра.

### 5. Research Benchmark Depth

Суть:

> не новые фичи, а серьёзное исследование на текущей базе.

Что сюда входит:

- full 1000-seed runs;
- confidence intervals;
- degradation curves;
- benchmark regression thresholds;
- подробный анализ стратегий;
- reproducible result packs.

Зачем:

- превращает платформу в доказательный research artifact;
- показывает, где стратегии реально выигрывают или проигрывают;
- даёт основу для статьи/отчёта.

Риск:

- близко к публикационной стадии;
- если сейчас не хотим думать о публикации, можно отложить до исправления correctness.

### 6. Platform / API Extensibility

Суть:

> сделать систему расширяемой для новых стратегий и миссий.

Что сюда входит:

- plugin-like strategy registration;
- stable internal APIs;
- scenario generators;
- validation hooks;
- external strategy harness;
- documented extension points.

Зачем:

- полезно, если проектом будут пользоваться другие разработчики;
- снижает стоимость добавления новых миссий/алгоритмов;
- делает архитектуру менее связанной с текущими hardcoded paths.

Риск:

- может превратиться в абстрактное платформостроение;
- лучше делать после того, как mission semantics и correctness ясны.

## Если не выбирать ветку: линейный план

Если пока не выбирать стратегическое направление, лучший линейный план:

```text
M25 Mission / Strategy Correctness
-> M26 Mission Semantics Layer
-> M27 Planner Quality Upgrade
-> M28 Stress & Regression Harness
-> M29 Simulation Realism Foundation
-> M30 Decision Point
```

## M25 — Mission / Strategy Correctness

Цель:

> закрыть самые заметные слабые места текущих стратегий.

Что сделать:

1. Разобраться, почему CBBA / centralized дают 0% success на SAR grid tasks.
2. Исправить обработку:
   - `grid_cell`;
   - task pose;
   - scan tasks;
   - SAR-specific completion conditions.
3. Разобраться с CBBA на inspection perimeter.
4. Добавить tests на проблемные mission-strategy пары:
   - SAR + CBBA;
   - SAR + centralized;
   - inspection perimeter + CBBA;
   - inspection perimeter + centralized/auction/greedy as baselines.
5. Добавить support matrix:
   - что expected stable;
   - что experimental;
   - что intentionally unsupported.

Почему первым:

- это не новая ветка;
- это исправление качества уже реализованного;
- слабые места уже видны в README/benchmark results;
- нет смысла добавлять реализм или SITL, пока базовые mission-strategy пары работают с очевидными провалами.

Done criteria:

- SAR grid tasks корректно обрабатываются CBBA/centralized или явно помечены unsupported с тестами;
- inspection perimeter имеет объяснимое поведение CBBA;
- benchmark report больше не содержит "0% success без ясной причины";
- support matrix документирована.

## M26 — Mission Semantics Layer

Цель:

> перестать трактовать все задачи как одинаковые allocation tasks.

Сейчас в проекте есть разные смыслы задач:

- coverage cell;
- SAR scan cell;
- confirmation scan;
- inspection edge;
- relay task;
- waypoint task.

Они имеют разную семантику, но часть алгоритмов видит их как обычные generic tasks.

Что сделать:

1. Ввести `TaskKind` или аналог:
   - `CoverageCell`;
   - `SarScan`;
   - `SarConfirmationScan`;
   - `InspectionEdge`;
   - `RelayPlacement`;
   - `Waypoint`.
2. Сделать adapters:
   - mission task -> allocation task;
   - mission task -> route cost;
   - mission task -> completion condition.
3. Добавить scoring hooks для разных миссий.
4. Уточнить validation:
   - SAR task должен иметь `grid_cell`;
   - inspection task должен иметь `edge_id`;
   - waypoint task должен иметь `pose`;
   - relay task должен иметь role/capability requirements.
5. Обновить стратегии так, чтобы они не игнорировали mission-specific fields.

Почему это важно:

> многие слабости алгоритмов сейчас похожи не на "плохой алгоритм", а на "алгоритм не понимает тип задачи".

Done criteria:

- mission-specific task semantics представлены явно;
- allocator/scoring path получает нужный контекст;
- validation ловит mismatch task kind / fields;
- regression tests покрывают SAR/inspection/relay/waypoint task kinds.

## M27 — Planner Quality Upgrade

Цель:

> улучшить планирование маршрутов и bundles после стабилизации mission semantics.

Что сделать:

1. Улучшить route ordering beyond greedy nearest-neighbour.
2. Добавить 2-opt для inspection/SAR bundles.
3. Учитывать:
   - battery;
   - return-to-base;
   - max range;
   - route feasibility.
4. Сделать route cost общей функцией.
5. Сравнить:
   - current ordering;
   - greedy TSP;
   - 2-opt;
   - mission-specific route cost.
6. Добавить metrics:
   - route length;
   - wasted travel;
   - return reserve;
   - infeasible route count.

Почему после M26:

- сначала надо стабилизировать смысл задач;
- потом улучшать планирование;
- иначе можно оптимизировать неправильную модель.

Done criteria:

- route planner улучшает inspection/SAR metrics;
- есть tests на route ordering;
- есть benchmark comparison current vs upgraded planner.

## M28 — Stress & Regression Harness

Цель:

> превратить текущий benchmark в инженерный контроль качества.

Что сделать:

1. Набор regression suites:
   - SAR;
   - inspection;
   - CBBA stress;
   - safety;
   - emergency mesh.
2. Thresholds:
   - success rate;
   - convergence p95;
   - message count;
   - conflict count;
   - safety violations;
   - edge coverage;
   - PoD / entropy.
3. Stress profiles:
   - packet loss;
   - partitions;
   - low battery;
   - dense tasks;
   - sparse agents;
   - noisy sensors.
4. Baseline artifact:
   - checked-in or generated reference summary;
   - compare current run vs baseline.
5. Report:
   - "what regressed";
   - "what improved";
   - "what stayed stable".

Это не публикационная полировка.

Это защита от деградаций.

Done criteria:

- regression run can fail on meaningful degradation;
- thresholds documented;
- baseline update process exists;
- CI/local command can run a small subset.

## M29 — Simulation Realism Foundation

Цель:

> добавить realism без ухода в полный real-world stack.

Что сделать:

1. Ввести altitude:
   - `Pose3` или расширение `Pose`;
   - compatibility layer для старых 2D scenarios.
2. Battery model v2:
   - hover cost;
   - climb cost;
   - cruise cost;
   - reserve for return-to-base.
3. Sensor model v3:
   - range;
   - field-of-view;
   - altitude-dependent detection probability.
4. Environment noise:
   - wind;
   - GPS/pose noise;
   - communication jitter.
5. Dynamic obstacles:
   - simple blocked regions;
   - time-varying no-fly zones.
6. Keep this as foundation:
   - do not build full physics simulator;
   - focus on mission-level realism.

Почему не раньше:

- realism усиливает сложность;
- без correctness/semantics можно сделать более реалистичную, но всё ещё неверную систему.

Done criteria:

- старые 2D scenarios работают без изменений или через migration;
- новые realism fields покрыты tests;
- хотя бы один scenario показывает отличие между old/new model.

## M30 — Decision Point

После M25-M29 уже будет понятнее, куда выгоднее идти дальше.

Возможные решения:

1. Если результаты стали сильными:

   > Research Benchmark Depth.

2. Если хочется внешних пользователей:

   > Platform / API Extensibility + Visualization.

3. Если хочется железо:

   > Real-World / SITL Bridge.

4. Если хочется сильнее симулятор:

   > Simulation Realism дальше.

M30 не обязательно должен быть кодовым milestone.

Это может быть аналитический checkpoint:

- что стало лучше;
- что осталось слабым;
- какая ветка теперь наиболее выгодна.

## Рекомендация

Если не выбирать направление сейчас, лучший следующий шаг:

> M25 Mission / Strategy Correctness.

Это не "причесывание" и не "публикация".

Это прямое усиление технического ядра.

Начать стоит с двух известных болевых точек:

1. **CBBA / centralized на SAR.**
2. **CBBA на inspection perimeter.**

После их анализа станет понятнее, нужен ли сначала `TaskKind` / mission semantics, или часть проблем можно закрыть локально в allocator/scoring/runner.
