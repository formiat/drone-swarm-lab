# DRONE_A.13.linear — Итоговый линейный план без выбора большой ветки

Дата фиксации: 2026-05-23

## Контекст

Документ фиксирует линейный план после сравнения:

- `docs_raw/DRONE_A.12.md`;
- `docs_raw/DRONE_B.12.md`;
- итоговой оценки веток в `docs_raw/DRONE_A.13.branches.md`.

Публикационный polishing сейчас откладывается. План ниже не про release packaging, а про следующий
инженерно-исследовательский цикл.

## Короткий вывод

Если сейчас не хочется выбирать направление, лучше идти так:

```text
M25 Correctness & Metrics Audit
-> M26 Mission Semantics Layer
-> M27 Planner / Algorithm Quality
-> M28 Stress & Regression Harness
-> M29 New Mission Prototype
-> M30 Simulation Realism Foundation
-> M31 Decision Point
```

Почему именно так:

- сначала надо исправить методологические дыры в уже существующих результатах;
- потом явно описать mission/task semantics, чтобы стратегии не гадали о смысле задач;
- потом улучшать planner/algorithm quality;
- потом зафиксировать regression harness;
- только после этого добавлять новую миссию или realism;
- SITL и Visualization пока остаются боковыми направлениями, а не частью обязательного линейного ствола.

## M25 — Correctness & Metrics Audit

Цель:

> сделать текущие benchmark outcomes методологически понятными.

Основные проблемы:

- CBBA и centralized показывают 0% success на SAR grid tasks;
- inspection может показывать `success=0.0` при `edge_coverage=1.0`;
- часть результатов выглядит как смешение algorithm failure, metric bug и unsupported semantics;
- README/benchmark report должны объяснять не только цифры, но и применимость стратегий.

Что сделать:

1. Разобрать SAR `grid_cell` path:
   - как DSL task превращается в allocation task;
   - что видят CBBA/centralized;
   - где теряется pose/scan semantics;
   - как определяется completion.
2. Разобрать inspection metric path:
   - что означает `success`;
   - что означает `edge_coverage`;
   - когда mission считается successful;
   - почему возможен конфликт `success=0.0` при `edge_coverage=1.0`.
3. Ввести или обновить support matrix:
   - mission;
   - strategy;
   - статус;
   - expected behavior;
   - known limitations.
4. Разделить в отчётах:
   - unsupported strategy/mission pair;
   - algorithm quality failure;
   - metric bug;
   - scenario too hard;
   - safety rejection.
5. Обновить benchmark docs так, чтобы спорные результаты были объяснимы.

Ожидаемые изменения в коде:

- fixes в conversion/scoring/completion paths;
- validation или explicit unsupported markers;
- metric consistency checks;
- новые regression tests;
- возможно небольшое изменение benchmark output schema.

Критерии готовности:

- SAR + CBBA/centralized больше не выглядят как немотивированный 0% success;
- inspection success согласован с edge coverage;
- support matrix существует и используется в docs/reporting;
- regression tests ловят оба известных класса ошибок;
- benchmark output можно объяснить без ручного знания внутренностей кода.

Тесты:

### Tests that need no refactoring

- unit/regression test: SAR grid task сохраняет `grid_cell` и usable pose до allocator/completion layer;
- integration test: SAR + CBBA на маленьком scenario имеет объяснимый status;
- integration test: SAR + centralized на маленьком scenario имеет объяснимый status;
- unit test: inspection `edge_coverage=1.0` согласован с mission success;
- test на support matrix для известных supported/experimental/unsupported пар;
- test на benchmark report classification.

### Tests that need light refactoring

- shared scenario builders для SAR/inspection;
- helper assertions для metric consistency;
- reusable benchmark run fixture без записи в machine-specific paths;
- fake allocation result builder for success/coverage combinations.

### Tests that need heavy refactoring

- property tests на consistency между task semantics, allocator output и completion;
- broad cross-product tests всех mission/strategy пар;
- long-run metric consistency regression across many seeds.

## M26 — Mission Semantics Layer

Цель:

> перестать трактовать все задачи как generic allocation tasks.

Проблема:

В проекте уже есть разные смысловые типы задач:

- coverage cell;
- SAR scan;
- SAR confirmation scan;
- inspection edge;
- relay placement;
- waypoint.

Если allocator/scoring/completion path не знает тип задачи, он может построить формально валидный,
но семантически неправильный план.

Что сделать:

1. Ввести явную модель task semantics:
   - `TaskKind` или аналог;
   - required fields per kind;
   - completion semantics;
   - route/scoring semantics.
2. Сделать adapters:
   - scenario task -> mission task;
   - mission task -> allocation task;
   - mission task -> route cost;
   - mission task -> completion condition;
   - mission task -> replay event fields.
3. Уточнить validation:
   - SAR scan требует `grid_cell`;
   - inspection edge требует `edge_id`;
   - waypoint требует pose;
   - relay placement требует role/capability constraints.
4. Обновить benchmark/replay output:
   - task kind;
   - semantic status;
   - reason for skipped/unsupported task.
5. Обновить документацию Mission DSL:
   - какие типы задач есть;
   - какие поля обязательны;
   - какие стратегии их поддерживают.

Ожидаемые изменения в коде:

- новый module/type для mission semantics;
- migration/adaptation старых сценариев;
- более строгая validation;
- изменения в allocator input или wrapper layer;
- обновление tests/fixtures.

Критерии готовности:

- task kind явно доступен там, где считается route/scoring/completion;
- validation ловит mismatch task kind / fields;
- старые сценарии либо работают без изменений, либо имеют понятную миграцию;
- SAR/inspection/relay/waypoint semantics покрыты тестами;
- docs описывают модель без необходимости читать код.

Тесты:

### Tests that need no refactoring

- validation tests для каждого task kind;
- serialization tests для task kind в scenario/replay outputs;
- adapter tests для SAR scan, inspection edge, relay placement, waypoint;
- completion tests для каждого task kind.

### Tests that need light refactoring

- shared fixtures для mission task construction;
- builder API для scenario snippets;
- reusable assertions для validation error messages;
- benchmark smoke test с mixed task kinds.

### Tests that need heavy refactoring

- property tests: valid scenario tasks always produce semantically valid allocation tasks;
- compatibility tests для старых scenario files across schema versions;
- full mission lifecycle tests from DSL -> allocation -> replay -> report.

## M27 — Planner / Algorithm Quality

Цель:

> улучшить качество маршрутов, bundles и перераспределения после стабилизации semantics.

Что сделать:

1. Route quality:
   - общий route cost;
   - mission-specific route cost;
   - 2-opt или другой локальный improvement pass;
   - comparison current ordering vs improved ordering.
2. Feasibility:
   - battery;
   - return-to-base reserve;
   - max route length;
   - capability constraints;
   - safety constraints.
3. Dynamic reallocation:
   - отказ агента;
   - partial replanning;
   - task recovery;
   - избегать полного restart там, где возможно.
4. CBBA/communication depth:
   - convergence under packet loss;
   - stale bid handling;
   - message budget;
   - communication-aware quality tradeoff.
5. Metrics:
   - route length;
   - wasted travel;
   - infeasible route count;
   - reassignment count;
   - convergence p95;
   - message count.

Критерии готовности:

- route quality улучшается хотя бы на одном SAR/inspection benchmark;
- dynamic reallocation работает на маленьком deterministic scenario;
- новые metrics видны в benchmark output;
- regressions покрыты тестами;
- старые стратегии не ломаются без явного migration path.

Тесты:

### Tests that need no refactoring

- unit tests для route cost;
- unit tests для 2-opt/local improvement на маленьких маршрутах;
- integration test: failed agent tasks are reassigned;
- tests на infeasible route rejection;
- tests на message/convergence metrics.

### Tests that need light refactoring

- fake agent failure scenarios;
- reusable route fixtures;
- benchmark assertions для route-quality improvement;
- fake communication profiles для deterministic packet loss.

### Tests that need heavy refactoring

- property tests для CBBA convergence under partitions;
- stochastic stress tests with many seeds;
- comparative benchmark tests current vs upgraded planner;
- hierarchical coordination tests.

## M28 — Stress & Regression Harness

Цель:

> превратить benchmark из набора запусков в инженерный контроль качества.

Что сделать:

1. Regression suites:
   - SAR;
   - inspection;
   - emergency mesh;
   - CBBA robustness;
   - safety;
   - replay.
2. Stress profiles:
   - packet loss;
   - partitions;
   - low battery;
   - dense tasks;
   - sparse agents;
   - noisy sensors;
   - partial agent failure.
3. Thresholds:
   - success rate;
   - edge coverage;
   - PoD;
   - entropy;
   - safety violations;
   - convergence p95;
   - message count;
   - reassignment count.
4. Baseline comparison:
   - generated local baseline;
   - checked-in small baseline, если размер разумный;
   - clear process for baseline update.
5. Reporting:
   - what regressed;
   - what improved;
   - what is unchanged;
   - which suite failed and why.

Критерии готовности:

- small regression suite запускается быстро локально;
- long suite может запускаться вручную;
- threshold failures понятны;
- baseline update process описан;
- результаты portable и не зависят от `$HOME` или локальных абсолютных путей.

Тесты:

### Tests that need no refactoring

- unit tests для threshold comparison;
- tests для baseline summary parser;
- CLI smoke test для small regression suite;
- tests для deterministic seed handling.

### Tests that need light refactoring

- tempdir-managed benchmark output fixtures;
- shared result summary builders;
- helpers для comparing benchmark manifests;
- compact golden summaries.

### Tests that need heavy refactoring

- long-run CI-like tests;
- broad seed-matrix regression tests;
- statistical tests for confidence intervals;
- cross-version benchmark compatibility tests.

## M29 — New Mission Prototype

Цель:

> проверить, что система действительно расширяется на новую миссию, а не только поддерживает уже реализованные сценарии.

Рекомендованная миссия:

> Wildfire / flood mapping.

Почему не pursuit/logistics первым:

- wildfire/flood ближе к текущим SAR/coverage primitives;
- можно переиспользовать BeliefMap-like model;
- естественно появляются risk zones and changing priorities;
- хорошо проверяются DSL, semantics, allocation, safety and replay;
- не нужно сразу вводить сложную динамику moving targets или pickup/dropoff dependencies.

Что сделать:

1. Domain model:
   - hazard map;
   - changing threat level;
   - priority zones;
   - detection/update events.
2. DSL:
   - scenario fields for hazard map;
   - time/update model;
   - task generation parameters.
3. Allocation:
   - mapping tasks;
   - re-prioritization;
   - compatibility with existing strategies;
   - explicit unsupported markers where needed.
4. Replay:
   - hazard map events;
   - agent observations;
   - updated task priorities.
5. Benchmark:
   - small scenario;
   - medium scenario;
   - baseline comparison across strategies.

Критерии готовности:

- новая миссия описывается через DSL;
- есть минимум два сценария;
- benchmark запускается хотя бы для stable strategies;
- replay содержит enough events для анализа;
- docs объясняют semantics и limitations.

Тесты:

### Tests that need no refactoring

- DSL parse/validation tests для wildfire/flood scenario;
- task generation tests;
- completion semantics tests;
- replay event serialization tests;
- benchmark smoke test для small scenario.

### Tests that need light refactoring

- hazard map builders;
- fake observation/update event helpers;
- reusable mission benchmark fixtures;
- assertions для priority updates.

### Tests that need heavy refactoring

- dynamic re-prioritization property tests;
- multi-seed mission stability tests;
- comparative tests across all strategies;
- visual/replay overlay tests.

## M30 — Simulation Realism Foundation

Цель:

> добавить базовый realism без ухода в полноценный physical simulator.

Что сделать:

1. Spatial model:
   - altitude;
   - optional `Pose3`;
   - compatibility with current 2D scenarios.
2. Battery model v2:
   - hover cost;
   - climb cost;
   - cruise cost;
   - return reserve.
3. Sensor model:
   - range;
   - field-of-view;
   - altitude-dependent detection;
   - noisy observations.
4. Environment:
   - wind/noise;
   - visibility;
   - blocked regions;
   - time-varying no-fly zones.
5. Reporting:
   - realism profile in benchmark manifest;
   - metrics showing realism impact.

Критерии готовности:

- старые scenarios продолжают работать;
- realism можно включить явно;
- есть один realism-enabled scenario;
- test suite покрывает battery/sensor/environment invariants;
- benchmark manifest показывает active realism profile.

Тесты:

### Tests that need no refactoring

- unit tests for battery cost components;
- unit tests for sensor detection probability boundaries;
- validation tests for 2D compatibility;
- manifest serialization tests for realism profile.

### Tests that need light refactoring

- shared pose fixtures;
- deterministic noise providers;
- environment profile builders;
- scenario migration helpers.

### Tests that need heavy refactoring

- stochastic realism regression tests;
- long-run comparison old vs realism-enabled model;
- multi-agent communication/environment interaction tests;
- SITL-aligned trajectory tests.

## M31 — Decision Point

Цель:

> после M25-M30 выбрать следующий большой фокус на основании фактического состояния проекта.

Что оценить:

- стали ли benchmark results методологически чище;
- какие стратегии реально усилились;
- насколько Mission DSL выдержал новую миссию;
- насколько выросла сложность поддержки;
- какие gaps остались самыми болезненными;
- есть ли смысл идти в publication/research benchmark depth;
- есть ли смысл идти в real SITL/PX4;
- нужен ли UI для анализа;
- нужна ли стабилизация API.

Возможные решения:

1. **Research track**

   Если результаты стали сильными и воспроизводимыми:

   - long-run benchmark;
   - confidence intervals;
   - report/paper-like artifact.

2. **Robotics/SITL track**

   Если хочется приблизиться к внешнему execution backend:

   - real `MavlinkTransport`;
   - single-agent PX4 golden path;
   - later multi-agent SITL.

3. **Visualization/operator track**

   Если анализ replay стал узким местом:

   - interactive replay viewer;
   - BeliefMap/InspectionGraph overlays;
   - strategy comparison UI.

4. **Platform/API track**

   Если появляются внешние пользователи или новые стратегии:

   - extension points;
   - stable schemas;
   - external strategy harness.

Критерии готовности:

- написан короткий decision report;
- выбран следующий track;
- старые alternatives не потеряны;
- следующий milestone имеет конкретный scope.

Тесты:

### Tests that need no refactoring

- не применимо как кодовый milestone, если M31 остаётся аналитическим checkpoint.

### Tests that need light refactoring

- можно добавить checks/scripts только если decision report включает machine-readable summaries.

### Tests that need heavy refactoring

- не требуется до выбора следующего track.

## Что не входит в линейный ствол сейчас

### Real SITL / PX4

Причина:

- важная ветка, но дорогая и зависимая от внешнего окружения;
- mock SITL уже даёт portable проверку;
- real PX4 лучше делать после mission semantics и safety clarity.

### Visualization

Причина:

- полезна, но пользователь явно отложил polishing;
- UI лучше строить после стабилизации replay schemas;
- сейчас важнее исправить benchmark correctness.

### Publication / Release Candidate polishing

Причина:

- отложено явно;
- сначала надо убрать methodological bugs;
- иначе публикационная упаковка будет скрывать слабые места, а не решать их.

## Итоговая рекомендация

Следующий практический шаг:

> M25 Correctness & Metrics Audit.

Начать лучше с двух конкретных расследований:

1. Почему SAR + CBBA/centralized дают 0% success.
2. Почему inspection может иметь `success=0.0` при `edge_coverage=1.0`.

После этого M26 станет либо маленькой формализацией уже найденных fixes, либо полноценным выделением
mission semantics layer. В обоих случаях это будет лучше, чем сейчас выбирать между SITL, UI,
новой миссией и simulation realism.
