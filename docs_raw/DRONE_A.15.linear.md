# DRONE_A.15.linear - дальнейший линейный план

Дата фиксации: 2026-05-28

## Назначение документа

Этот документ содержит объединённый дальнейший линейный план развития проекта на базе `DRONE_A.15.linear.md` и `DRONE_B.15.linear.md`.

Документ начинается с будущих milestones и не пересказывает уже выполненную работу.

Большие validation runs остаются важными, но они не являются самостоятельными milestones. Их роль - подтверждать качество после инженерных исправлений.

## Линейная последовательность

```text
M40 Benchmark Trust & Deterministic Reporting
-> M41 Semantics Audit & Algorithmic Gap Triage
-> M42 Regression Harness v3
-> M43 Realism Calibration Pack
-> M44 Flood Naming / Scope Decision
-> M45 Big Direction Decision
```

Порядок мотивирован зависимостями:

- нельзя интерпретировать benchmark artifacts, пока не проверены report identity, determinism and manifest reproducibility;
- диагностика алгоритмических провалов полезна только после короткого audit of semantics/planner wiring;
- regression thresholds нельзя нормально калибровать, пока метрики и gaps не классифицированы;
- realism calibration требует устойчивого regression/reporting foundation;
- flood scope лучше решать после clarity around disaster-mapping support;
- большую ветку развития нужно выбирать после того, как technical foundation перестал искажать картину.

## M40 - Benchmark Trust & Deterministic Reporting

Цель:

> сделать reporting and benchmark path воспроизводимым, чтобы дальнейшие результаты можно было считать технически надёжными.

Суть:

Benchmark/reporting уже умеет собирать данные, но следующий этап должен зафиксировать, что одинаковая конфигурация даёт одинаковые aggregate metrics независимо от уровня параллелизма, порядка обхода структур данных и формата экспорта.

Это не milestone "сделать большой прогон". Большой прогон может быть validation artifact после завершения M40, но сама задача M40 - инженерная: убрать источники nondeterminism and ambiguity.

Что сделать:

1. Зафиксировать determinism contract:
   - same seed/config produces same aggregate metrics;
   - jobs count must not change aggregate metrics;
   - JSON/CSV/Markdown must agree on row identity;
   - visible row order must be stable;
   - timestamps/run ids must be ignored in metric equality checks.
2. Проверить report identity:
   - every row has explicit mission/scenario/profile/strategy identity;
   - merged reports preserve source mission;
   - no fallback can silently relabel rows;
   - no duplicate report row keys.
3. Найти и убрать источники nondeterminism:
   - unordered map iteration in visible report output;
   - unseeded randomness;
   - order-dependent aggregation;
   - inconsistent profile ordering;
   - inconsistent strategy ordering;
   - accidental dependence on rayon scheduling.
4. Усилить manifest:
   - command shape;
   - mission/suite selection;
   - seed range or seed count;
   - jobs;
   - build profile when available;
   - git commit when available;
   - schema/report version if available.
5. Добавить report comparison helper:
   - compare metrics while ignoring timestamps;
   - compare JSON/CSV identities;
   - compare row counts;
   - compare mission/scenario/profile/strategy keys;
   - compare jobs=1 vs jobs=N outputs.
6. Обновить docs around validation:
   - validation run is an artifact;
   - artifact must reference exact code state;
   - artifact must be reproducible from manifest;
   - benchmark docs are results storage, not roadmap milestones.

Ожидаемый результат:

- можно доверять, что изменение метрик вызвано изменением кода/сценария, а не порядком потоков;
- report artifacts становятся пригодными для сравнения между запусками;
- следующий большой validation run будет интерпретируемым.

Не входит в scope:

- чинить алгоритмические слабости;
- добавлять новые mission families;
- делать publishable research report;
- выбирать большую ветку развития.

Acceptance criteria:

- same command with same seed/config gives same aggregate metrics;
- jobs count does not change metrics;
- output row ordering is stable;
- report identities match across JSON/CSV/Markdown;
- manifest contains enough metadata to reproduce a validation artifact;
- documentation separates engineering milestone from validation run.

Tests that need no refactoring:

- deterministic jobs comparison for existing benchmark paths;
- report row ordering test;
- JSON/CSV identity parity test;
- manifest metadata assertions;
- no duplicate report row keys test;
- integration test around output directory and machine-readable reports.

Tests that need light refactoring:

- helper to compare JSON reports while ignoring timestamp/run id;
- shared fixture builder for multi-mission reports;
- reusable manifest assertion helper;
- JSON/CSV parsing helpers for tests;
- tempdir-based output tests.

Tests that need heavy refactoring:

- reproducibility harness for complete benchmark packs;
- report schema compatibility tests;
- property tests for report row identity and uniqueness;
- statistical diff tooling for long validation artifacts.

## M41 - Semantics Audit & Algorithmic Gap Triage

Цель:

> сначала проверить wiring semantics/planner layers, затем разобрать слабые места алгоритмов и метрик без попытки чинить всё сразу.

Суть:

Проект содержит несколько mission families, strategies, adapters, planners and metrics. Перед глубоким algorithmic triage нужно сделать короткий architecture audit: правильные ли semantics попадают в runner/allocator/planner, не теряется ли task context, корректно ли считаются completion and route feasibility.

После этого можно классифицировать видимые провалы: где это баг метрики, где баг реализации, где неподходящий алгоритм, где слишком жёсткий сценарий, а где допустимое known limitation.

Что сделать:

1. Провести semantics/planner audit:
   - runner uses mission adapter completion where intended;
   - scoring path receives mission/task context;
   - route cost path uses current task semantics where applicable;
   - battery-aware feasibility checks the current candidate route, not stale full task list;
   - DSL validation catches task-kind field mismatch.
2. Проверить key wiring with focused tests:
   - SAR scan keeps grid cell context;
   - inspection edge keeps edge id context;
   - wildfire mapping keeps zone/threat context;
   - waypoint tasks keep pose context;
   - planner feasibility drops only tasks needed for feasible route.
3. Составить список gap classes:
   - suspicious metric mismatch;
   - unsupported strategy/mission combination;
   - weak distributed behavior;
   - profile-specific failure;
   - dynamic scenario weakness;
   - route/battery feasibility mismatch.
4. Для каждого gap class сделать короткую investigation note:
   - reproducible command or fixture;
   - expected behavior;
   - actual behavior;
   - likely cause;
   - confidence level;
   - recommended action.
5. Классифицировать каждый gap:
   - metric bug;
   - implementation bug;
   - algorithm mismatch;
   - scenario too hard or ill-posed;
   - accepted limitation;
   - needs more data.
6. Исправить только high-confidence bugs:
   - obvious metric extraction bugs;
   - obvious success predicate inconsistencies;
   - obvious assignment/completion mismatches;
   - support matrix mistakes;
   - small wiring mismatches.
7. Обновить support matrix:
   - supported;
   - experimental;
   - unsupported with reason;
   - failing due to known bug;
   - not yet evaluated.
8. Подготовить вход для regression update:
   - which gaps should become regression checks;
   - which gaps should remain experimental;
   - which gaps should be excluded from default gate.

Ожидаемый результат:

- weak spots перестают быть набором разрозненных наблюдений;
- становится понятно, что чинить кодом, что документировать, а что оставить outside default support;
- algorithm backlog получает concrete root-cause notes;
- regression v3 получает осмысленные suites instead of accidental thresholds.

Не входит в scope:

- полностью переписать strategies;
- решать все weak combinations;
- делать новые большие validation artifacts;
- менять public API без необходимости;
- строить новую mission family.

Acceptance criteria:

- key semantics/planner wiring is tested or explicitly scoped out;
- every major weak spot has a classification;
- every suspicious metric mismatch has a reproduction path;
- support matrix reflects current support boundaries;
- high-confidence bugs are fixed or isolated into concrete follow-ups;
- no known unsupported combination is presented as stable support.

Tests that need no refactoring:

- targeted metric consistency tests;
- support matrix assertions for known unsupported combinations;
- unit tests for success/completion predicates;
- unit tests for adapter completion on in-memory state;
- battery-aware planner feasibility test;
- regression test for each high-confidence metric bug fixed in this milestone.

Tests that need light refactoring:

- reusable scenario-specific metric assertion helpers;
- small reproduction fixtures for gap classes;
- helper to compare per-run metrics and aggregate metrics;
- support matrix fixture builder;
- shared task builders by `TaskKind`;
- in-memory `RunState` fixtures.

Tests that need heavy refactoring:

- algorithm-comparison oracle tests;
- full lifecycle tests: DSL -> adapter -> allocator -> runner -> metrics;
- mission-specific simulation invariants;
- scenario minimization tooling;
- property tests for success/completion/coverage consistency.

## M42 - Regression Harness v3

Цель:

> сделать regression harness устойчивым development gate, а не просто набором smoke checks.

Суть:

Regression harness должен помогать быстро понимать, сломался ли проект, где именно он сломался, и насколько это важно. Для этого нужно разделить типы suites, сделать failure output action-oriented, добавить нормальный baseline workflow, а нестабильные или экспериментальные проверки вынести из default gate.

Что сделать:

1. Разделить suites по назначению:
   - smoke: быстрые structural checks;
   - quick: стабильные behavioural checks;
   - experimental: tracked but non-gating by default;
   - validation: long/manual/CI-optional artifacts, not milestones.
2. Определить threshold policy:
   - no meaningless zero thresholds;
   - no single-seed gate for volatile behavior;
   - different thresholds for structural and behavioural checks;
   - explicit promotion path from experimental to default gate.
3. Улучшить failure output:
   - suite name;
   - strategy/profile/mission;
   - actual metric;
   - threshold;
   - delta;
   - reproduction command;
   - baseline comparison if available.
4. Улучшить baseline workflow:
   - update only from green state;
   - baseline stores metadata: git commit, date, seed range/count, suite group;
   - delta output readable;
   - missing baseline entries explicit;
   - baseline update can write to caller-provided path.
5. Обновить CLI:
   - `--list-suites`;
   - `--suite smoke|quick|experimental|validation`;
   - explicit opt-in for experimental/validation suites;
   - machine-readable report output;
   - stable exit code semantics.
6. Убрать machine-specific test assumptions:
   - no hardcoded `/tmp/...`;
   - use tempdir-owned paths;
   - portable fixtures only.
7. Обновить docs:
   - what default regression means;
   - when to update baseline;
   - how to reproduce failure;
   - how to promote a suite;
   - how validation artifacts relate to milestones.

Ожидаемый результат:

- default regression становится стабильной ежедневной проверкой;
- failure reports становятся actionable;
- experimental scenarios можно отслеживать без поломки основного gate;
- baseline workflow становится безопасным;
- future milestones получают надёжную safety net.

Не входит в scope:

- делать большой статистический framework;
- исправлять все algorithmic gaps;
- добавлять новую mission family.

Acceptance criteria:

- default regression does not flake in normal local usage;
- failure output includes reproduction command;
- suites are grouped by purpose;
- experimental suites are opt-in;
- baseline workflow is documented and tested;
- tests do not depend on machine-specific paths;
- CLI can emit both human-readable and machine-readable reports.

Tests that need no refactoring:

- threshold checker tests;
- baseline delta tests;
- CLI exit-code tests;
- suite grouping tests;
- failure formatting tests with reproduction command.

Tests that need light refactoring:

- regression report parser for tests;
- tempdir-based baseline update tests;
- shared baseline fixtures;
- ignored repeated-run check for flakiness;
- CLI fixture helpers.

Tests that need heavy refactoring:

- confidence-interval-based regression;
- automated flaky-suite detector;
- baseline history store;
- end-to-end regression report golden tests.

## M43 - Realism Calibration Pack

Цель:

> превратить realism profiles из набора параметров в измеримый, воспроизводимый model layer.

Суть:

Realism profiles задают noise, wind, comms jitter and battery behavior. Теперь нужно понять, насколько эти профили реально влияют на миссии, соответствуют ли ожиданиям, и можно ли безопасно использовать их в regression/validation.

Что сделать:

1. Определить expected realism effects для профилей:
   - light;
   - medium;
   - heavy.
2. Определить expected effects by metric:
   - effect on success/completion;
   - effect on route length;
   - effect on wasted travel;
   - effect on battery reserve;
   - effect on communication availability;
   - effect on detection time;
   - effect on mapping ratio.
3. Сравнить controlled profiles:
   - ideal;
   - light;
   - medium;
   - heavy.
4. Для каждой mission family описать expected degradation:
   - which metrics should move;
   - which metrics should remain stable;
   - which metrics are too noisy for default gate.
5. Проверить profile parameters:
   - pose noise;
   - wind vector;
   - comms jitter;
   - battery drain;
   - reserve fraction;
   - sensor penalties.
6. Усилить manifest metadata:
   - active realism profile;
   - profile parameters;
   - whether realism preset is enabled;
   - scenario/profile identity.
7. Обновить docs:
   - what each realism profile means;
   - what effects are expected;
   - how to run realism validation;
   - which realism suites are regression-safe;
   - what is not modeled.
8. Подготовить regression integration:
   - stable realism smoke checks;
   - optional realism quick checks;
   - non-gating realism validation artifacts.

Ожидаемый результат:

- realism перестаёт быть просто набором чисел;
- docs no longer overstate realism confidence;
- regression can include realism checks safely;
- future research-style comparisons get a calibrated foundation.

Не входит в scope:

- physical calibration against real drone logs;
- full weather/terrain model;
- production-grade flight dynamics;
- new sensor fusion stack.

Acceptance criteria:

- expected realism effects are documented by profile;
- realism effects are measured and explained;
- profile parameters are documented;
- manifest contains active realism metadata;
- realism validation commands are reproducible;
- stable realism checks are safe for regression;
- noisy realism checks are marked experimental/validation-only.

Tests that need no refactoring:

- scenario JSON validation for realism files;
- manifest metadata assertions for realism fields;
- realism preset smoke test;
- profile selection tests.

Tests that need light refactoring:

- ideal-vs-realism comparison helper;
- realism delta summarizer;
- deterministic fixture for realism profile selection;
- test helper for battery/noise/comms parameter assertions;
- manifest assertion helpers.

Tests that need heavy refactoring:

- stochastic realism regression;
- statistical realism analysis harness;
- calibrated external model comparison;
- mission-specific realism acceptance tests;
- synthetic sensor/noise validation suite.

## M44 - Flood Naming / Scope Decision

Цель:

> закрыть несоответствие между disaster-mapping wording and actual implemented scope.

Суть:

Нужно выбрать один из двух путей: либо честно оставить только wildfire scope and rename/docs-cleanup, либо добавить минимальную flood mission variant. До этого момента лучше не расширять disaster mapping, потому что regression, determinism and realism должны быть уже стабильнее.

Вариант A - rename/docs cleanup:

1. Уточнить feature naming:
   - wildfire remains implemented scope;
   - flood remains future work;
   - docs avoid implying separate flood support.
2. Обновить README and status docs.
3. Проверить scenario catalog and examples.
4. Убрать ambiguous wording from CLI/help if present.

Вариант B - minimal flood variant:

1. Определить flood mission scope:
   - mapping flooded zones;
   - identifying critical zones;
   - tracking spread/risk level;
   - optional rescue-priority tasks.
2. Добавить scenario/profile:
   - small-static;
   - medium-dynamic;
   - optional realism profile.
3. Добавить model:
   - water/risk spread;
   - priority updates;
   - affected zones;
   - time-to-map critical zones.
4. Добавить metrics:
   - flooded zones mapped;
   - critical zones mapped;
   - time to first critical zone;
   - final risk level;
   - zone observations.
5. Интегрировать with adapters/runner/replay/reporting.
6. Добавить regression smoke as experimental first.

Рекомендация:

Сначала выбрать вариант A unless there is a strong reason to expand scope. Minimal flood variant стоит делать только если disaster-mapping becomes a chosen project direction.

Ожидаемый результат:

- название и документация не обещают лишнего;
- либо flood явно out of scope, либо появляется минимальная настоящая реализация;
- future users understand what disaster-mapping support actually means.

Acceptance criteria:

- no ambiguous flood support claims remain;
- chosen scope is reflected in README/status/docs;
- if flood is implemented, it has scenario, metrics, replay/reporting integration and regression smoke;
- if flood is not implemented, it is clearly listed as future work.

Tests that need no refactoring:

- scenario catalog validation if flood scenario is added;
- docs/manual consistency review if wording is removed;
- report/export identity tests if flood rows are added.

Tests that need light refactoring:

- shared hazard/disaster-mapping adapter tests;
- reusable dynamic-zone scenario fixtures;
- replay event assertions for disaster-zone updates.

Tests that need heavy refactoring:

- general disaster-mapping abstraction over wildfire/flood;
- shared dynamic hazard model;
- property tests for spatial spread/risk updates.

## M45 - Big Direction Decision

Цель:

> выбрать следующую большую ветку развития после того, как foundation станет достаточно устойчивым.

Суть:

До этого этапа проект должен иметь reproducible benchmark artifacts, classified algorithmic gaps, stable regression gate, measurable realism layer and clarified disaster-mapping scope. Только после этого имеет смысл выбирать крупное направление.

Что оценить:

| Вопрос | Влияет на |
|---|---|
| Насколько чисты алгоритмические результаты? | Research Benchmark |
| Насколько болезненен анализ replay вручную? | Replay / Visualization |
| Стабильны ли semantics для SITL upload? | Real SITL / PX4 |
| Есть ли внешние пользователи? | Platform / API |
| Нужна ли более глубокая динамика миссий? | Disaster Mapping v2 |

Возможные направления:

1. Research benchmark track:
   - stronger statistical analysis;
   - strategy comparison;
   - reproducible validation artifacts;
   - paper-style reports.
2. Visualization / replay product track:
   - replay explorer;
   - scenario browser;
   - mission timeline;
   - benchmark dashboard.
3. Public API / library track:
   - stable crate boundaries;
   - semver policy;
   - docs.rs readiness;
   - examples as public contract.
4. SITL / PX4 integration track:
   - multi-agent SITL workflow;
   - better command lifecycle;
   - robust operational errors;
   - safety boundaries around real/simulated control.
5. Disaster Mapping v2 track:
   - deeper dynamic hazards;
   - flood or other hazard variants;
   - richer mission semantics for disaster response.

Что сделать:

1. Сравнить направления по критериям:
   - value;
   - implementation cost;
   - technical risk;
   - amount of missing infrastructure;
   - expected users;
   - validation strategy.
2. Выбрать one primary direction.
3. Зафиксировать explicit non-goals.
4. Сформировать следующий линейный план под выбранную ветку.
5. Обновить docs so project positioning matches chosen direction.

Ожидаемый результат:

- появляется не просто “ещё список задач”, а выбранный product/research direction;
- дальнейшие milestones перестают быть generic hardening;
- scope and non-goals become clear.

Acceptance criteria:

- one primary direction selected;
- alternatives documented as deferred with return conditions;
- next roadmap created for selected direction;
- README/project description matches chosen direction;
- validation approach for selected direction is explicit.

Tests that need no refactoring:

- no direct code tests required if this remains a planning milestone;
- docs command smoke only if README commands are changed.

Tests that need light refactoring:

- README snippet verification if commands are modified;
- examples smoke if public API direction is selected.

Tests that need heavy refactoring:

- depends on selected direction;
- visualization tests, API compatibility tests, SITL integration tests, disaster-scenario tests or statistical benchmark harness may become necessary after the decision.

## Итоговый порядок

```text
1. M40 Benchmark Trust & Deterministic Reporting
2. M41 Semantics Audit & Algorithmic Gap Triage
3. M42 Regression Harness v3
4. M43 Realism Calibration Pack
5. M44 Flood Naming / Scope Decision
6. M45 Big Direction Decision
```

Ключевая идея: сначала сделать результаты воспроизводимыми и понятными, затем проверить semantics wiring and classify algorithmic gaps, затем усилить regression, калибровать realism, уточнить disaster scope, и только после этого выбирать следующую крупную ветку.
