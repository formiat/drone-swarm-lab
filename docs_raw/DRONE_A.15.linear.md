# DRONE_A.15.linear - статус M32-M39 и дальнейший линейный план

Дата фиксации: 2026-05-28

## Контекст

Этот документ продолжает `docs_raw/DRONE_A.14.linear.md` и отвечает на вопрос:

> всё ли сделано из линейного плана M32-M39, хорошо ли сделано, и какой следующий линейный план разумен без выбора большой ветки.

Проверялись:

- `docs_raw/DRONE_A.14.linear.md`;
- текущий локальный код на ветке `main`;
- свежая история коммитов;
- `README.md`;
- `docs/BENCHMARK_RESULTS.md`;
- regression/benchmark CLI entrypoints;
- текущее состояние тестов.

Свежая история показывает, что после `DRONE_A.14` были реализованы:

```text
18a6ea3 M32 Reporting & Metrics Hardening
795e0a8 M32b Benchmark Identity Hardening for --mission all
e6bcd3f M33 Mission Semantics Integration
b8f2c4d fix: M33 adapter completion and allocation regressions
03c250d M34 Planner Correctness v2
814a27d M35 Dynamic Mission Correctness
434a335 M36 Regression Harness v2
fcb4200 feat: M37 Realism Scenario Pack
45b60f4 unfinished work
37d35d9 fix: M38 compilation errors and test fixtures
89418b6 removed plan
```

Коммит `45b60f4` по содержанию является основной реализацией M38, но сообщение `unfinished work` плохо отражает статус и будет мешать будущей навигации по истории.

## Короткий вывод

Линейный план M32-M38 в основном реализован, но закрыт неравномерно.

Самый важный вывод: **текущий HEAD нельзя считать полностью зелёным и полностью закрывшим M36/M38**, потому что `cargo test --workspace` сейчас падает на regression integration tests.

Общая картина:

```text
M32 Reporting & Metrics Hardening      done, quality good
M33 Mission Semantics Integration      done, quality good enough
M34 Planner Correctness v2             done, but still narrow impact
M35 Dynamic Mission Correctness        partially done, known weak strategies remain
M36 Regression Harness v2              implemented, but currently not reliable enough
M37 Realism Scenario Pack              mostly done, still not calibrated analysis
M38 Wildfire / Flood v2                wildfire done, flood not really done
M39 Decision Point                     not done
```

Поэтому следующий шаг не должен быть 1000-seed publishable benchmark или новая большая ветка. Сначала нужно сделать короткий hardening-loop вокруг regression/determinism, затем зафиксировать decision report.

Предлагаемый следующий линейный план:

```text
M39a Regression Repair
-> M39b Decision / Audit Report
-> M40 Benchmark Determinism
-> M41 Algorithmic Gap Triage
-> M42 Regression Harness v3
-> M43 Realism Calibration
-> M44 Flood Decision
-> M45 Big Direction Decision
```

## Проверки текущего состояния

### Что проходит

`cargo clippy --all-targets -- -D warnings` проходит.

Отдельный запуск:

```bash
cargo run -p swarm-examples --bin regression_runner -- --jobs 4
```

может проходить и выдавать `overall_pass: true`.

Отдельные targeted tests для wildfire и support matrix проходят:

```bash
cargo test -p swarm-examples --test wildfire
cargo test -p swarm-examples --test support_matrix
```

Это значит, что большая часть feature-level кода действительно присутствует и работоспособна.

### Что не проходит

`cargo test --workspace` сейчас падает.

Подтверждённый failing test:

```text
crates/swarm-examples/tests/regression.rs::strategy_comparison_regression_flag
```

Причина по runtime output:

```text
cargo run -p swarm-examples --bin strategy_comparison -- --regression

# Regression Report
overall_pass: false

sar_ideal_greedy                  FAIL, task_completion_rate actual=0.000 threshold>=0.800
sar_standard_greedy               FAIL, task_completion_rate actual=0.000 threshold>=0.700
wildfire_small_static_greedy      FAIL, task_completion_rate actual=0.000 threshold>=0.800
wildfire_medium_dynamic_greedy    FAIL, task_completion_rate actual=0.000 threshold>=0.600
```

Даже при `--jobs 1` путь `strategy_comparison --regression` продолжает падать на wildfire suites:

```text
wildfire_small_static_greedy      FAIL, task_completion_rate actual=0.000
wildfire_medium_dynamic_greedy    FAIL, task_completion_rate actual=0.000
```

Это не совпадает с обычным smoke wildfire benchmark, где `strategy_comparison --smoke --mission wildfire` показывает `Completion = 1.000` для wildfire rows.

### Почему это важно

README сейчас документирует два regression entrypoint:

```bash
cargo run -p swarm-examples --bin regression_runner -- --jobs 4
cargo run -p swarm-examples --bin strategy_comparison -- --regression
```

и прямо говорит, что exit code равен `0`, если все suites проходят.

Фактически один из этих entrypoint сейчас не проходит. Значит:

- M36 нельзя считать закрытым как production-quality regression harness;
- README Current Status слишком оптимистичен;
- future benchmark numbers нельзя считать полностью trustworthy, пока regression path раздвоен;
- `cargo test --workspace` не является зелёным acceptance check.

### Вероятная причина regression divergence

В `regression_runner` regression builder поддерживает:

- coverage;
- emergency-mesh;
- sar;
- inspection;
- wildfire;
- realism wrapper через `suite.realism`.

В `strategy_comparison --regression` path логика похожая, но не идентичная. В частности, в текущем коде этот path не полностью повторяет wildfire/realism wiring. Поэтому wildfire suites могут попадать в fallback empty scenario или в неполный сценарный путь, что даёт `task_completion_rate = 0.000`.

Дополнительно наблюдалась нестабильность smoke-regression при параллельных test runs: один запуск `regression_runner --jobs 4` дал failures по `sar_standard_greedy` и `inspection_perimeter_experimental`, следующий такой же запуск прошёл. Это нужно трактовать как сигнал к проверке determinism/flakiness, а не как окончательно найденный root cause.

## Что сделано по M32-M38

## M32 - Reporting & Metrics Hardening

Статус: **сделано хорошо**.

Что готово:

- mixed-mission report identity исправлен;
- JSON/CSV/Markdown rows несут per-row mission/scenario;
- `--mission all` получил корректный `benchmark_run_id`;
- output directories создаются автоматически;
- custom seed count работает;
- wildfire/planner/realism-related fields экспортируются;
- есть integration tests вокруг benchmark pack.

Качество:

M32 выглядит одним из самых чисто закрытых пунктов. Он решал конкретную инфраструктурную проблему, и под неё есть тесты.

Оставшийся риск:

`docs/BENCHMARK_RESULTS.md` описывает 500-seed release run на старом commit `8fb5ab1`, то есть до M33-M38. Этот результат полезен как историческая валидация M32b, но уже не является актуальной full validation текущего HEAD.

Вывод:

M32 закрыт. Перед новым большим benchmark нужен свежий release run после repair/determinism.

## M33 - Mission Semantics Integration

Статус: **сделано, качество хорошее, но ownership слоя ещё не идеален**.

Что готово:

- есть concrete adapters:
  - coverage;
  - SAR;
  - inspection;
  - relay;
  - waypoint;
  - wildfire;
- есть `AdapterRegistry`;
- adapter path используется в allocation/scoring/completion;
- DSL validation проверяет kind-specific поля;
- есть unit tests для adapters;
- support matrix отражает часть known limitations.

Качество:

Это уже не просто интерфейс. M33 реально протащил mission semantics в runtime.

Оставшийся риск:

Часть semantics всё ещё живёт в runner, часть в adapter, часть в allocator. Это не критично сейчас, но если добавлять новые mission types, ownership надо будет формализовать:

- что считается adapter responsibility;
- что считается runner responsibility;
- что считается allocator/planner responsibility;
- какие metrics должны идти через mission semantics layer.

Вывод:

M33 можно считать закрытым как functional milestone. Архитектурная чистка нужна позже, когда появится ещё одна новая mission family.

## M34 - Planner Correctness v2

Статус: **сделано, но влияние planner layer всё ещё ограничено**.

Что готово:

- `RoutePlanner` trait;
- nearest-neighbour planner;
- 2-opt planner;
- battery-aware planner;
- ordered-subset feasibility;
- battery model v2 integration;
- route metrics:
  - route length;
  - wasted travel;
  - return reserve;
  - infeasible routes;
  - bundle travel distance.

Качество:

Базовая correctness часть выглядит нормально. Есть unit tests для planner behavior, включая battery-aware feasibility.

Оставшийся риск:

Planner пока не стал главным фактором качества всех strategies. Для части стратегий он скорее улучшает metrics/reporting или работает в ограниченном месте, чем радикально меняет allocation quality.

Вывод:

M34 закрыт как infrastructure/correctness milestone, но не как “мы решили routing quality для всех миссий”.

## M35 - Dynamic Mission Correctness

Статус: **частично закрыто**.

Что готово:

- mission-specific success semantics:
  - SAR через found targets / task completion;
  - inspection через coverage threshold;
  - wildfire через mapped ratio;
- support matrix tests;
- known unsupported statuses для SAR CBBA/centralized;
- dynamic mission metrics стали более честными, чем раньше.

Качество:

Хорошо, что проблемные стратегии не замаскированы как успешные. Документация и support matrix честно фиксируют, что часть сочетаний mission/strategy не supported или слабая.

Оставшиеся проблемы:

- SAR CBBA и centralized остаются near-zero на historical 500-seed run;
- emergency-mesh distributed strategies слабее centralized;
- inspection perimeter остаётся profile-sensitive;
- wildfire CBBA слаб на dynamic fire spread;
- не все слабые места исправлены алгоритмически, часть только классифицирована.

Вывод:

M35 закрыт как semantic correctness/classification, но не как algorithmic correctness.

## M36 - Regression Harness v2

Статус: **реализовано, но сейчас не закрыто качественно**.

Что готово:

- `RegressionSuite`;
- `ThresholdChecker`;
- `RegressionRunner`;
- default suites;
- committed baseline;
- threshold policy;
- regression CLI;
- tests на threshold violations, baseline roundtrip, no zero thresholds;
- wildfire/realism suites добавлены.

Что плохо:

- `cargo test --workspace` падает;
- `strategy_comparison --regression` падает;
- regression logic продублирована между двумя binaries;
- documented command в README не является passing command;
- smoke thresholds завязаны на seed 0 и для некоторых suites могут быть хрупкими;
- observed flakiness требует отдельного investigation.

Вывод:

M36 нужно считать открытым до M39a. Это главный immediate blocker.

## M37 - Realism Scenario Pack

Статус: **в основном сделано**.

Что готово:

- realism profiles:
  - light;
  - medium;
  - heavy;
- scenario files:
  - `scenarios/coverage.realism.json`;
  - `scenarios/sar.realism.json`;
  - `scenarios/inspection.realism.json`;
  - `scenarios/wildfire.realism.json`;
- battery/noise/wind/comms-jitter parameters;
- README section;
- manifest metadata.

Качество:

Как scenario pack и reproducibility layer это полезно.

Оставшийся риск:

Это ещё не calibrated realism analysis. Нет убедительного отчёта:

- ideal vs light/medium/heavy;
- impact на success/completion/route/battery;
- confidence intervals;
- mission-by-mission degradation;
- calibration against external assumptions.

Вывод:

M37 закрыт как infrastructure/scenario pack, но не как research-grade realism study.

## M38 - Wildfire / Flood v2

Статус: **wildfire сделан, flood фактически не сделан**.

Что готово по wildfire:

- `scenarios/wildfire.small-static.json`;
- `scenarios/wildfire.medium-dynamic.json`;
- `scenarios/wildfire.realism.json`;
- dynamic threat;
- spatial spread;
- wind influence;
- zone expansion;
- high-priority metrics;
- replay integration;
- README section;
- wildfire tests.

Качество wildfire:

Wildfire стал полноценнее, чем M30 prototype. Это уже usable simulated mission family внутри benchmark.

Проблема с названием:

M38 называется `Wildfire / Flood v2`, но отдельного flood scenario/model/adapter/profile нет. В README есть формулировка “wildfire / flood mapping”, но кодовая база сейчас wildfire-first.

Проблема с regression:

`regression_runner` может проходить wildfire suites, но `strategy_comparison --regression` на wildfire suites падает. Пока это не исправлено, M38 нельзя считать полностью integrated.

Вывод:

M38 лучше переименовать или уточнить:

- либо `M38 Wildfire v2`;
- либо добавить настоящий flood track в M44;
- либо явно документировать flood as future mission variant, not implemented.

## M39 - Decision Point

Статус: **не сделан**.

В истории нет отдельного decision report, который бы после M32-M38 честно зафиксировал:

- какие milestones закрыты;
- какие закрыты частично;
- какие результаты benchmark актуальны;
- какие результаты устарели;
- можно ли идти в 1000-seed run;
- какой следующий трек выбран;
- какие развилки остаются.

Вывод:

M39 нужно делать после M39a Regression Repair, иначе decision report будет опираться на не зелёный test state.

## Оценка качества текущего проекта

Текущий проект уже не просто демка одного алгоритма. Это оформленный research prototype с:

- mission DSL;
- несколькими mission families;
- несколькими allocation strategies;
- metrics/reporting;
- replay/debuggability;
- mock SITL scaffold;
- regression harness;
- realism profiles;
- wildfire dynamic mission.

Но это ещё не завершённый “готовый продукт”:

- `cargo test --workspace` сейчас не green;
- regression entrypoints разъехались;
- benchmark docs устарели относительно HEAD;
- часть README status overstated;
- flood часть M38 не реализована как отдельная сущность;
- SAR/CBBA/centralized и другие слабые стратегии скорее классифицированы, чем исправлены;
- realism не калиброван;
- publishable benchmark после M33-M38 ещё не прогонялся.

## Рекомендуемый дальнейший линейный план

## M39a - Regression Repair

Цель:

> вернуть проект в состояние, где `cargo test --workspace`, `regression_runner` и `strategy_comparison --regression` согласованно проходят или согласованно показывают одну и ту же контролируемую ошибку.

Что сделать:

1. Убрать дублирование regression execution между:
   - `crates/swarm-examples/src/bin/regression_runner.rs`;
   - `crates/swarm-examples/src/bin/strategy_comparison.rs`.
2. Вынести общий helper/library path:
   - build suite scenario;
   - apply realism if `suite.realism`;
   - run smoke/quick;
   - collect metrics map;
   - print/save baseline;
   - compute exit code.
3. Убедиться, что supported missions одинаковы в обоих CLI:
   - coverage;
   - emergency-mesh;
   - SAR;
   - inspection;
   - wildfire.
4. Починить wildfire regression path в `strategy_comparison --regression`.
5. Починить realism suite path в `strategy_comparison --regression`.
6. Проверить flakiness:
   - повторить `regression_runner --jobs 1`;
   - повторить `regression_runner --jobs 4`;
   - повторить `strategy_comparison --regression --jobs 1`;
   - повторить `strategy_comparison --regression --jobs 4`;
   - сравнить failing suites and metrics.
7. Если seed-0 smoke реально нестабилен, перевести такие suites в quick или ослабить/изменить metric только после измерения, а не “на глаз”.

Acceptance criteria:

- `cargo test --workspace` passes;
- `cargo clippy --all-targets -- -D warnings` passes;
- `cargo run -p swarm-examples --bin regression_runner -- --jobs 4` passes;
- `cargo run -p swarm-examples --bin strategy_comparison -- --regression --jobs 4` passes;
- both CLI paths return same suite names and same pass/fail state;
- wildfire regression suites use real wildfire scenarios;
- realism regression suite actually applies realism preset.

Tests that need no refactoring:

- integration test for `strategy_comparison --regression --jobs 1`;
- integration test for `strategy_comparison --regression --jobs 4`;
- integration test asserting wildfire suites do not return empty-scenario zero completion;
- integration test asserting regression output contains `wildfire_small_static_greedy` and passes;
- integration test asserting `realism_coverage_smoke` path is present.

Tests that need light refactoring:

- shared test helper for running CLI binaries;
- shared parsing helper for regression report output;
- parity test comparing `regression_runner` and `strategy_comparison --regression`;
- repeated-run smoke test gated as ignored or slow.

Tests that need heavy refactoring:

- direct library-level regression runner tests without spawning nested `cargo run`;
- deterministic replay/metrics snapshot for every default suite;
- property tests around suite builder consistency.

## M39b - Decision / Audit Report

Цель:

> после зелёного regression state честно зафиксировать, что именно стало состоянием проекта после M32-M38.

Что сделать:

1. Написать decision report:
   - status by milestone;
   - known limitations;
   - stale benchmark warning;
   - readiness for 1000-seed run;
   - whether flood is implemented or not;
   - whether M36/M38 should be marked stable.
2. Обновить README Current Status:
   - убрать overstatement;
   - уточнить M38 naming;
   - указать актуальный test command;
   - отметить benchmark docs as historical if not refreshed.
3. Решить, нужен ли отдельный `docs/STATUS.md` или продолжать через README.

Acceptance criteria:

- README не обещает failing commands;
- M37/M38 статус соответствует фактической реализации;
- benchmark docs явно говорят, к какому commit относятся;
- M39 decision file committed.

Tests:

### Tests that need no refactoring

- docs command smoke for documented regression commands.

### Tests that need light refactoring

- markdown command extraction smoke test, если решим автоматизировать README snippets.

### Tests that need heavy refactoring

- full docs-as-tests harness.

## M40 - Benchmark Determinism

Цель:

> подготовить benchmark path к новому 1000-seed release run на текущем HEAD.

Что сделать:

1. Проверить determinism:
   - jobs=1 vs jobs=4;
   - jobs=1 vs jobs=14;
   - repeated same command same seed count;
   - debug vs release drift;
   - row ordering stability;
   - JSON/CSV equality where expected.
2. Убрать nondeterministic sources:
   - unordered `HashMap` iteration in output where order matters;
   - random sources not seeded by scenario seed;
   - race-prone aggregation.
3. Разделить:
   - simulation stochasticity controlled by seed;
   - benchmark nondeterminism, which should be zero for same seed/config.
4. Добавить manifest fields:
   - git commit;
   - build profile;
   - jobs;
   - seed count;
   - command;
   - suite/scenario version if available.

Acceptance criteria:

- same release command with same seeds produces same aggregate metrics;
- jobs count does not affect aggregate metrics;
- output ordering is stable;
- benchmark manifest is enough to reproduce run.

Tests that need no refactoring:

- existing jobs=1 vs jobs=4 deterministic unit/integration test expanded to more missions;
- report row ordering test;
- manifest contains git/build/jobs/seed fields.

Tests that need light refactoring:

- helper to compare JSON reports ignoring timestamp/run id;
- release-mode smoke script outside normal unit tests.

Tests that need heavy refactoring:

- reproducibility harness for full benchmark packs;
- statistical diff tool for large runs.

## M41 - Algorithmic Gap Triage

Цель:

> не пытаться “чинить всё”, а зафиксировать и приоритизировать реальные слабые места, которые видны из metrics.

Known gaps from historical 500-seed run:

- CBBA coverage anomaly:
  - some rows have `Success = 0.000`, `Completion = 1.000`, `Coverage = 0.000`;
  - this may be metric/predicate bug rather than algorithm result.
- SAR:
  - auction/connectivity-aware/greedy around usable level;
  - CBBA and centralized near zero;
  - centralized static pre-plan may be fundamentally wrong for dynamic belief search.
- Emergency mesh:
  - centralized much stronger than distributed strategies;
  - distributed conflict/reallocation behavior needs analysis.
- Inspection perimeter:
  - profile-sensitive;
  - low edge coverage for several strategies.
- Wildfire:
  - greedy/auction/centralized/connectivity-aware strong historically;
  - CBBA weaker on dynamic spread.

What to do:

1. Create one small investigation report per gap.
2. Classify each gap:
   - metric bug;
   - scenario too hard/ill-posed;
   - algorithm mismatch;
   - implementation bug;
   - accepted known limitation.
3. Fix only high-confidence bugs before broad algorithm work.
4. Update support matrix and regression thresholds based on classification.

Acceptance criteria:

- every known large failure has a status;
- no suspicious metric mismatch remains unexplained;
- support matrix distinguishes unsupported vs failing vs experimental.

Tests that need no refactoring:

- targeted regression tests for known suspicious metrics;
- support matrix tests for known unsupported combinations.

Tests that need light refactoring:

- scenario-specific metric assertion helpers;
- gap reproduction CLI fixtures.

Tests that need heavy refactoring:

- algorithm-comparison oracle tests;
- mission-specific simulation invariants.

## M42 - Regression Harness v3

Цель:

> сделать regression harness не просто working, а useful for long-term development.

Что сделать:

1. Разделить suites:
   - smoke: fast structural health;
   - quick: stable behavioural thresholds;
   - benchmark: longer statistical evidence;
   - experimental: tracked but not gating.
2. Для нестабильных mission/profile/strategy combos не использовать seed-0 как единственный gate.
3. Ввести baseline deltas как first-class output.
4. Сделать failure output более action-oriented:
   - command to reproduce;
   - actual metrics;
   - expected threshold;
   - last baseline value.
5. Согласовать README threshold policy with actual behavior.

Acceptance criteria:

- regression suite не флейкает на нормальном локальном запуске;
- failure объясняет, какой command воспроизводит проблему;
- baseline обновляется только после зелёного state;
- experimental suites не ломают основной acceptance unless explicitly promoted.

Tests that need no refactoring:

- threshold checker tests;
- baseline delta tests;
- CLI failure exit-code tests.

Tests that need light refactoring:

- report parser for regression output;
- repeated-run check in ignored/slow test category.

Tests that need heavy refactoring:

- confidence interval based regression;
- historical baseline database.

## M43 - Realism Calibration

Цель:

> превратить realism profiles из “наборов параметров” в измеримый research layer.

Что сделать:

1. Run ideal vs light vs medium vs heavy for core missions:
   - coverage;
   - SAR;
   - inspection;
   - wildfire.
2. Measure:
   - success/completion degradation;
   - route length/wasted travel;
   - battery reserve;
   - communication availability;
   - detection time for SAR;
   - mapped ratio for wildfire.
3. Compare against README expected impact.
4. Adjust realism parameters or docs if observed effect does not match.
5. Produce realism report.

Acceptance criteria:

- realism impact is quantified;
- README expected impact is backed by actual data or softened;
- realism scenarios can be used in benchmark/regression without ambiguity.

Tests that need no refactoring:

- scenario JSON validation for realism files;
- manifest metadata assertions.

Tests that need light refactoring:

- ideal-vs-realism comparison helper;
- benchmark report summarizer.

Tests that need heavy refactoring:

- calibrated external model comparison;
- statistical realism analysis harness.

## M44 - Flood Decision

Цель:

> закрыть несоответствие между названием `Wildfire / Flood v2` и фактической реализацией.

Вариант A: переименовать.

- M38 становится `Wildfire v2`;
- README убирает “Flood” из stable feature;
- flood остаётся future branch.

Вариант B: реализовать minimal flood mission variant.

Минимально нужно:

- `TaskKind` or mission profile for flood mapping/rescue;
- flood scenario JSON;
- flood adapter or wildfire adapter extension;
- dynamic water spread or risk map;
- metrics:
  - flooded zones mapped;
  - priority/rescue zones observed;
  - time to map critical zones;
  - coverage of affected region;
- regression smoke suite.

Рекомендация:

Сначала сделать M39a-M43. Потом выбрать A или B. Сейчас лучше не вкладываться в flood, пока regression/determinism не стабилизированы.

Tests that need no refactoring:

- README/docs consistency check by manual review;
- scenario catalog test if flood JSON added.

Tests that need light refactoring:

- shared hazard-mapping adapter tests.

Tests that need heavy refactoring:

- general disaster-mapping mission abstraction over wildfire/flood.

## M45 - Big Direction Decision

Цель:

> после hardening and calibration выбрать большую ветку развития.

К этому моменту проект должен иметь:

- green workspace tests;
- stable regression;
- deterministic benchmark output;
- fresh release benchmark on current HEAD;
- known algorithmic gaps classified;
- realism impact measured;
- flood naming resolved.

Тогда можно выбирать направление:

1. Research benchmark track:
   - publishable 1000-seed/long-run results;
   - statistical reports;
   - algorithm comparison;
   - paper-like docs.
2. Visualization/replay product track:
   - web/CLI replay explorer;
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

До M45 не выбирать большую ветку. Сейчас самый прагматичный линейный путь - repair, determinism, benchmark credibility, then decision.

## Итоговый ответ

Что сделано:

- M32-M35 в целом сделаны;
- M37 mostly done;
- wildfire часть M38 mostly done;
- проект сильно продвинулся относительно `DRONE_A.14`.

Что не сделано или сделано недостаточно хорошо:

- M36 regression harness сейчас не является надёжным gate;
- `strategy_comparison --regression` broken;
- `cargo test --workspace` broken;
- M39 decision point not done;
- flood часть M38 отсутствует как отдельная реализация;
- benchmark docs stale relative to current HEAD;
- README status слишком оптимистичен.

Что делать дальше:

```text
1. M39a Regression Repair
2. M39b Decision / Audit Report
3. M40 Benchmark Determinism
4. M41 Algorithmic Gap Triage
5. M42 Regression Harness v3
6. M43 Realism Calibration
7. M44 Flood Decision
8. M45 Big Direction Decision
```

Этот порядок сохраняет линейность и не требует сейчас выбирать большую ветку развития. Он сначала чинит доверие к тестам и метрикам, затем обновляет benchmark evidence, и только после этого возвращает проект к выбору направления.
