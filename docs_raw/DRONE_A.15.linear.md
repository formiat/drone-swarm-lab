# DRONE_A.15.linear - текущий статус и дальнейший линейный план

Дата фиксации: 2026-05-28

## Контекст

Этот документ корректирует предыдущую версию `DRONE_A.15.linear.md`.

Что изменено:

- убрана историческая сводка коммитов;
- убраны старые benchmark/run summaries;
- benchmark-прогоны больше не трактуются как отдельные milestones;
- прогоны оставлены только как способ проверки готовности конкретных инженерных этапов;
- линейный план сфокусирован на исправлении текущих проблем кода, regression harness, determinism, semantics and documentation consistency.

Вопрос, на который отвечает документ:

> что реально готово из плана M32-M39, что сделано недостаточно хорошо, и какой следующий линейный план разумен без выбора большой ветки развития.

Проверялись:

- текущий локальный код;
- `docs_raw/DRONE_A.14.linear.md`;
- `README.md`;
- `docs/BENCHMARK_RESULTS.md`;
- regression/benchmark CLI entrypoints;
- текущий статус тестов.

Важно: этот документ не является benchmark report. Он описывает engineering status and next implementation plan. Если нужны численные benchmark results, их лучше держать отдельно в `docs/BENCHMARK_RESULTS.md` или отдельном results report.

## Короткий вывод

План M32-M38 в основном реализован, но неравномерно.

Состояние по этапам:

```text
M32 Reporting & Metrics Hardening      done, quality good
M33 Mission Semantics Integration      done, quality good enough
M34 Planner Correctness v2             done, but planner impact remains limited
M35 Dynamic Mission Correctness        partially done, weak strategies remain classified rather than fixed
M36 Regression Harness v2              implemented, but not reliable enough yet
M37 Realism Scenario Pack              mostly done, not calibrated analysis yet
M38 Wildfire / Flood v2                wildfire mostly done, flood not really implemented
M39 Decision Point                     not done
```

Самый важный blocker: текущий regression path не является надёжным acceptance gate. Отдельные команды могут проходить, но workspace-level проверка и `strategy_comparison --regression` показывают расхождение между regression entrypoints.

Поэтому следующий линейный план должен начинаться не с новых больших возможностей и не с большого benchmark-прогона, а с восстановления доверия к тестам, regression harness and deterministic reporting.

Предлагаемый следующий линейный план:

```text
M39a Regression Repair
-> M39b Status / Decision Report
-> M40 Deterministic Reporting & Benchmark Credibility
-> M41 Algorithmic Gap Triage
-> M42 Regression Harness v3
-> M43 Realism Calibration
-> M44 Flood Naming / Scope Decision
-> M45 Big Direction Decision
```

Большие seed-прогоны не являются milestones в этом плане. Они являются validation artifacts, которые можно запускать после того, как соответствующий этап сделал код достаточно стабильным и воспроизводимым.

## Текущее качество проверки

Что выглядит здоровым:

- feature-level код по M32-M38 в основном присутствует;
- `clippy` проходит;
- отдельные targeted tests для wildfire/support matrix проходят;
- отдельный `regression_runner` может проходить;
- CLI, exporters, adapters, planners, scenario packs and metrics уже образуют работающий research prototype.

Что выглядит проблемно:

- `cargo test --workspace` сейчас нельзя считать зелёным acceptance signal;
- `strategy_comparison --regression` расходится с `regression_runner`;
- regression execution path продублирован между двумя binaries;
- `README.md` обещает regression commands, один из которых сейчас не является надёжно passing;
- smoke thresholds местами хрупкие;
- часть статусов в README выглядит оптимистичнее фактического состояния;
- flood часть M38 не имеет отдельной реализации;
- текущие benchmark artifacts не должны подменять свежую проверку текущего HEAD.

Вывод:

Перед любым новым большим направлением нужно сначала закрыть regression/consistency loop.

## Что готово из M32-M38

## M32 - Reporting & Metrics Hardening

Статус: **закрыто хорошо**.

Что готово:

- mixed-mission report identity исправлен;
- JSON/CSV/Markdown rows несут per-row mission/scenario;
- `--mission all` получил корректный benchmark identity;
- output directories создаются автоматически;
- custom seed count работает;
- wildfire/planner/realism-related fields экспортируются;
- есть integration coverage вокруг benchmark pack.

Качество:

M32 решал конкретную инфраструктурную проблему и закрыл её достаточно чисто. Это один из самых устойчивых этапов текущей серии.

Оставшиеся замечания:

- benchmark artifacts должны рассматриваться как результаты конкретной проверки, а не как milestone сами по себе;
- перед новыми большими выводами по качеству алгоритмов нужен fresh validation на текущем HEAD после repair/determinism.

Итог:

M32 можно считать закрытым.

## M33 - Mission Semantics Integration

Статус: **закрыто функционально**.

Что готово:

- concrete adapters:
  - coverage;
  - SAR;
  - inspection;
  - relay;
  - waypoint;
  - wildfire;
- `AdapterRegistry`;
- adapter path используется в allocation/scoring/completion;
- DSL validation проверяет kind-specific поля;
- есть unit coverage для adapters;
- support matrix отражает часть known limitations.

Качество:

Это уже не просто trait-заготовка. Mission semantics реально участвует в runtime.

Оставшиеся замечания:

- часть semantics живёт в runner;
- часть semantics живёт в adapter;
- часть scoring/planning behavior живёт в allocator/planner;
- ownership слоя пока не формализован полностью.

Это допустимо для текущей стадии, но при добавлении следующей mission family ownership нужно будет уточнить.

Итог:

M33 закрыт как functional milestone. Архитектурная чистка нужна позже, когда появится реальная потребность расширять semantics layer.

## M34 - Planner Correctness v2

Статус: **закрыто как infrastructure/correctness, не закрыто как algorithm-quality breakthrough**.

Что готово:

- `RoutePlanner` trait;
- nearest-neighbour planner;
- 2-opt planner;
- battery-aware planner;
- ordered-subset feasibility;
- battery model integration;
- route metrics:
  - route length;
  - wasted travel;
  - return reserve;
  - infeasible routes;
  - bundle travel distance.

Качество:

Базовая correctness часть выглядит нормально. Есть тесты для planner behavior and battery-aware feasibility.

Оставшиеся замечания:

- planner пока не стал главным quality driver для всех strategies;
- часть влияния planner видна скорее через metrics/reporting, чем через существенное улучшение всех алгоритмов;
- route/planner слой требует дальнейшей проверки на реальных слабых mission/profile combinations.

Итог:

M34 закрыт как слой инфраструктуры. Улучшение конкретных алгоритмов нужно планировать отдельно через gap triage.

## M35 - Dynamic Mission Correctness

Статус: **частично закрыто**.

Что готово:

- mission-specific success semantics:
  - SAR через targets/task completion;
  - inspection через coverage threshold;
  - wildfire через mapped ratio;
- support matrix tests;
- documented unsupported/weak combinations;
- dynamic mission metrics стали честнее.

Качество:

Хорошо, что проблемные strategy/mission combinations не замаскированы как успешные. Это важный шаг от “демка всегда зелёная” к честному research prototype.

Оставшиеся проблемы:

- SAR CBBA and centralized remain weak/unsupported;
- emergency-mesh distributed strategies require analysis;
- inspection perimeter remains profile-sensitive;
- wildfire CBBA on dynamic scenarios still needs investigation;
- часть слабых мест классифицирована, но не исправлена алгоритмически.

Итог:

M35 закрыт как semantic correctness/classification. Он не закрывает algorithmic quality gaps.

## M36 - Regression Harness v2

Статус: **реализовано, но не закрыто качественно**.

Что готово:

- `RegressionSuite`;
- `ThresholdChecker`;
- `RegressionRunner`;
- default suites;
- baseline support;
- threshold policy;
- regression CLI;
- tests на threshold violations, baseline roundtrip, no-zero thresholds;
- wildfire/realism suites добавлены.

Что плохо:

- regression logic продублирована в двух binaries;
- `strategy_comparison --regression` расходится с `regression_runner`;
- documented CLI path не является надёжным passing path;
- smoke thresholds завязаны на очень маленькие samples;
- failure output полезен, но пока не даёт полностью удобный reproducible diagnosis;
- regression suite пока не является достаточно надёжным daily gate.

Итог:

M36 нужно считать открытым до M39a. Это главный immediate blocker.

## M37 - Realism Scenario Pack

Статус: **в основном закрыто как scenario pack**.

Что готово:

- realism profiles:
  - light;
  - medium;
  - heavy;
- scenario files for core missions;
- battery/noise/wind/comms-jitter parameters;
- README section;
- manifest metadata.

Качество:

Как infrastructure and scenario pack это полезно.

Оставшиеся замечания:

- realism ещё не калиброван;
- нет полноценного ideal-vs-realism analysis;
- expected impact в docs требует подтверждения текущими measured results;
- realism suites должны быть стабильно встроены в regression path.

Итог:

M37 закрыт как scenario pack. Research-grade realism analysis остаётся отдельным будущим этапом.

## M38 - Wildfire / Flood v2

Статус: **wildfire mostly done, flood not done**.

Что готово по wildfire:

- wildfire scenario files;
- dynamic threat;
- spatial spread;
- wind influence;
- zone expansion;
- high-priority metrics;
- replay integration;
- README section;
- wildfire tests.

Качество wildfire:

Wildfire стал полноценной simulated mission family внутри проекта.

Проблема с названием:

M38 называется `Wildfire / Flood v2`, но отдельного flood model/scenario/adapter/profile нет. Фактическая реализация сейчас wildfire-first.

Проблема с integration:

Пока regression entrypoints расходятся, wildfire нельзя считать идеально интегрированным во все CLI paths.

Итог:

M38 нужно уточнить:

- либо переименовать в `Wildfire v2`;
- либо добавить настоящий flood scope позже;
- либо явно документировать, что flood пока не реализован.

## M39 - Decision Point

Статус: **не сделан**.

Что должно быть в M39:

- честный status by milestone;
- список known limitations;
- решение по M38 naming/scope;
- решение, какие regression commands считаются official;
- решение, какие benchmark artifacts считаются validation-only;
- выбор, готов ли проект к следующей большой ветке.

Итог:

M39 нужно делать после M39a Regression Repair, иначе decision report будет фиксировать неустойчивое состояние.

## Рекомендуемый дальнейший линейный план

## M39a - Regression Repair

Цель:

> вернуть проект в состояние, где workspace tests, `regression_runner` and `strategy_comparison --regression` согласованно проверяют один и тот же набор suites.

Что сделать:

1. Убрать дублирование regression execution между:
   - `crates/swarm-examples/src/bin/regression_runner.rs`;
   - `crates/swarm-examples/src/bin/strategy_comparison.rs`.
2. Вынести общий helper/library path:
   - build suite scenario;
   - apply realism if `suite.realism`;
   - run smoke/quick mode;
   - collect metrics map;
   - compute pass/fail;
   - print report;
   - update baseline if requested.
3. Убедиться, что supported missions одинаковы в обоих CLI:
   - coverage;
   - emergency-mesh;
   - SAR;
   - inspection;
   - wildfire.
4. Починить wildfire regression path.
5. Починить realism regression path.
6. Проверить, что suite names, strategies, metrics and exit codes совпадают между CLI paths.
7. Если seed-0 smoke suites реально нестабильны, перевести их в более устойчивый mode или изменить threshold policy после измерения.

Acceptance criteria:

- `cargo test --workspace` passes;
- `cargo clippy --all-targets -- -D warnings` passes;
- `regression_runner` and `strategy_comparison --regression` return matching pass/fail state;
- wildfire suites use real wildfire scenarios;
- realism suite actually applies realism preset;
- README documented regression commands match reality.

Tests that need no refactoring:

- integration test for `strategy_comparison --regression`;
- integration test asserting wildfire regression suites do not return empty-scenario zero completion;
- integration test asserting `realism_coverage_smoke` path is present;
- existing regression tests adjusted to assert parity.

Tests that need light refactoring:

- shared helper for running CLI binaries in tests;
- shared parser for regression report output;
- parity test comparing both regression entrypoints;
- optional ignored repeated-run test for flakiness detection.

Tests that need heavy refactoring:

- direct library-level regression runner tests without spawning nested `cargo run`;
- deterministic metrics snapshots for default suites;
- property tests around suite builder consistency.

## M39b - Status / Decision Report

Цель:

> после regression repair честно зафиксировать состояние проекта и убрать overstated statuses.

Что сделать:

1. Написать decision/status report:
   - status by milestone;
   - known limitations;
   - M36 status after repair;
   - M38 naming/scope;
   - official regression commands;
   - benchmark artifacts as validation outputs, not milestones.
2. Обновить README Current Status:
   - убрать overstatement;
   - уточнить M38 naming;
   - указать актуальный test/regression command set;
   - отделить feature status от benchmark artifact status.
3. Уточнить `docs/BENCHMARK_RESULTS.md`:
   - он хранит results, not roadmap;
   - каждый result привязан к конкретному code state;
   - отсутствие свежего большого прогона не является незакрытым milestone.

Acceptance criteria:

- README не обещает failing commands;
- M37/M38 status соответствует фактической реализации;
- benchmark docs не выглядят как roadmap milestones;
- M39 decision/status file committed.

Tests that need no refactoring:

- manual docs review;
- smoke verification documented commands.

Tests that need light refactoring:

- script/helper for validating README command snippets, if worth automating.

Tests that need heavy refactoring:

- full docs-as-tests harness.

## M40 - Deterministic Reporting & Benchmark Credibility

Цель:

> сделать benchmark/reporting path воспроизводимым и пригодным для последующих validation runs.

Важно: **M40 не является milestone “сделать большой прогон”**. Большой прогон может быть результатом проверки после M40, но не должен быть самой задачей.

Что сделать:

1. Проверить и зафиксировать determinism rules:
   - same seed/config should produce same aggregate metrics;
   - jobs count should not affect metrics;
   - output row ordering should be stable;
   - timestamps/run ids should be isolated from metric equality checks.
2. Убрать nondeterministic sources where they affect reports:
   - unordered map iteration in visible outputs;
   - unseeded randomness;
   - race-prone aggregation;
   - inconsistent profile ordering.
3. Разделить:
   - stochastic simulation controlled by scenario seed;
   - benchmark/report nondeterminism, which should be eliminated;
   - validation runs, which are artifacts after code is stable.
4. Улучшить manifest metadata:
   - git commit if available;
   - build profile if available;
   - jobs;
   - seed count;
   - mission/scenario suite;
   - command shape.

Acceptance criteria:

- same command with same seed/config gives same metrics;
- jobs count does not change aggregate metrics;
- report ordering is stable;
- JSON/CSV/Markdown identities agree;
- manifest is sufficient to reproduce a validation run.

Tests that need no refactoring:

- extend jobs=1 vs jobs=N determinism tests;
- report row ordering test;
- manifest metadata assertions;
- JSON/CSV identity parity tests.

Tests that need light refactoring:

- helper to compare JSON reports while ignoring timestamp/run id;
- deterministic fixture builders for multi-mission reports.

Tests that need heavy refactoring:

- reproducibility harness for full benchmark packs;
- statistical diff tool for long validation runs.

## M41 - Algorithmic Gap Triage

Цель:

> классифицировать реальные weak spots before trying to fix everything.

Known classes of gaps:

- metric/predicate anomalies:
  - success/completion/coverage can disagree in suspicious ways;
  - these need root-cause checks before treating them as algorithm results.
- SAR weak strategies:
  - some strategy/mission combinations are known weak or unsupported;
  - need distinguish unsupported design from implementation bug.
- emergency-mesh distributed quality:
  - distributed strategies need analysis against centralized baseline behavior.
- inspection perimeter:
  - profile-sensitive and currently difficult.
- wildfire dynamic behavior:
  - dynamic spread changes strategy quality and should be inspected separately from static wildfire.

What to do:

1. Create one small investigation per gap class.
2. Classify each finding:
   - metric bug;
   - scenario too hard/ill-posed;
   - algorithm mismatch;
   - implementation bug;
   - accepted known limitation.
3. Fix high-confidence bugs.
4. Update support matrix and regression thresholds after classification.
5. Avoid turning long validation runs into milestones; use them only to confirm fixes after the classification is done.

Acceptance criteria:

- every major weak spot has a status;
- suspicious metric mismatches are explained or fixed;
- support matrix distinguishes unsupported vs failing vs experimental;
- regression suites reflect accepted support boundaries.

Tests that need no refactoring:

- targeted regression tests for known suspicious metrics;
- support matrix tests for unsupported combinations;
- mission-specific metric consistency tests.

Tests that need light refactoring:

- scenario-specific metric assertion helpers;
- small reproduction fixtures for each gap.

Tests that need heavy refactoring:

- algorithm-comparison oracle tests;
- mission-specific simulation invariants;
- scenario minimization tooling for hard failures.

## M42 - Regression Harness v3

Цель:

> сделать regression harness useful as a stable development gate.

Что сделать:

1. Разделить suites:
   - smoke: fast structural health;
   - quick: stable behavioural thresholds;
   - experimental: tracked but not gating by default;
   - validation: longer manual/CI-optional artifacts, not milestones.
2. Для нестабильных combinations не использовать single seed as hard gate.
3. Ввести baseline deltas as first-class output.
4. Сделать failure output action-oriented:
   - command to reproduce;
   - suite name;
   - actual metric;
   - threshold;
   - baseline value if available.
5. Согласовать README threshold policy with actual behavior.

Acceptance criteria:

- default regression does not flake in normal local usage;
- failure output is enough to reproduce the issue;
- baseline update process is clear;
- experimental suites do not break default gate unless promoted.

Tests that need no refactoring:

- threshold checker tests;
- baseline delta tests;
- CLI failure exit-code tests.

Tests that need light refactoring:

- report parser for regression output;
- repeated-run check in ignored/slow category;
- baseline update smoke with tempdir.

Tests that need heavy refactoring:

- confidence-interval-based regression;
- versioned baseline store;
- automated flaky-suite detector.

## M43 - Realism Calibration

Цель:

> превратить realism profiles из набора параметров в измеримый model layer.

Что сделать:

1. Define expected realism effects by mission:
   - coverage;
   - SAR;
   - inspection;
   - wildfire.
2. Compare ideal/light/medium/heavy in controlled validation checks.
3. Measure:
   - success/completion degradation;
   - route length/wasted travel;
   - battery reserve;
   - communication availability;
   - detection time for SAR;
   - mapped ratio for wildfire.
4. Update realism parameters or documentation if observed behavior does not match expectations.
5. Make realism suites safe to include in regression as non-flaky checks.

Acceptance criteria:

- realism effects are measured and explained;
- README expected impact is backed by data or softened;
- realism profiles are reproducible;
- realism regression suite has a clear threshold policy.

Tests that need no refactoring:

- scenario JSON validation for realism files;
- manifest metadata assertions;
- realism preset smoke test.

Tests that need light refactoring:

- ideal-vs-realism comparison helper;
- report summarizer for realism deltas;
- deterministic fixture for realism profile selection.

Tests that need heavy refactoring:

- calibrated external model comparison;
- statistical realism analysis harness;
- mission-specific realism acceptance tests.

## M44 - Flood Naming / Scope Decision

Цель:

> закрыть несоответствие между названием `Wildfire / Flood v2` and actual implementation.

Вариант A: переименовать.

- M38 becomes `Wildfire v2`;
- README removes flood from stable feature wording;
- flood remains a future branch.

Вариант B: реализовать minimal flood mission variant.

Минимально нужно:

- flood task/model/scope;
- flood scenario JSON;
- adapter or disaster-mapping abstraction;
- dynamic water/risk spread;
- flood-specific metrics;
- regression smoke suite.

Рекомендация:

Сначала сделать M39a-M43. Потом выбрать A or B. Сейчас не стоит добавлять flood before regression and determinism are stable.

Tests that need no refactoring:

- scenario catalog test if flood scenario is added;
- docs consistency review if flood wording is removed.

Tests that need light refactoring:

- shared hazard/disaster-mapping adapter tests.

Tests that need heavy refactoring:

- general disaster-mapping mission abstraction over wildfire/flood.

## M45 - Big Direction Decision

Цель:

> после repair and calibration выбрать большую ветку развития.

К этому моменту проект должен иметь:

- green workspace tests;
- stable default regression;
- deterministic benchmark/reporting path;
- clarified M38 flood/wildfire scope;
- classified algorithmic gaps;
- measured realism impact;
- docs that do not overstate status.

Possible next directions:

1. Research benchmark track:
   - statistical reports;
   - algorithm comparison;
   - publishable-style analysis;
   - long validation runs as artifacts, not milestones.
2. Visualization/replay product track:
   - replay explorer;
   - scenario browser;
   - benchmark dashboard.
3. Public API/library track:
   - stable crates API;
   - examples;
   - semver;
   - docs.rs readiness.
4. SITL/PX4 integration track:
   - multi-agent SITL;
   - robust command lifecycle;
   - better operational errors;
   - safety boundaries.

Рекомендация:

До M45 не выбирать большую ветку. Сейчас самый прагматичный линейный путь:

```text
repair regression
-> clarify status
-> make reporting deterministic
-> classify algorithmic gaps
-> harden regression
-> calibrate realism
-> resolve flood scope
-> choose big direction
```

## Итог

Что сделано:

- M32 закрыт хорошо;
- M33 закрыт функционально;
- M34 закрыт как planner infrastructure;
- M35 закрыт как semantic classification, but not algorithmic gap closure;
- M37 mostly done as scenario pack;
- wildfire часть M38 mostly done.

Что ещё не закрыто:

- M36 as reliable regression gate;
- `strategy_comparison --regression` parity with `regression_runner`;
- M39 decision/status report;
- flood scope in M38;
- deterministic reporting confidence;
- algorithmic gap classification;
- realism calibration.

Следующий линейный план:

```text
1. M39a Regression Repair
2. M39b Status / Decision Report
3. M40 Deterministic Reporting & Benchmark Credibility
4. M41 Algorithmic Gap Triage
5. M42 Regression Harness v3
6. M43 Realism Calibration
7. M44 Flood Naming / Scope Decision
8. M45 Big Direction Decision
```

Прогоны остаются важными, но их роль другая: они подтверждают этапы после исправлений. Они не должны сами становиться milestones в линейном плане.
