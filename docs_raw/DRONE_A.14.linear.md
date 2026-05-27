# DRONE_A.14.linear - Линейный план после M31

Дата фиксации: 2026-05-27

## Контекст

Этот документ фиксирует линейный план без выбора большой ветки после анализа:

- raw-документов `DRONE_A.9*` - `DRONE_A.13*`;
- raw-документов `DRONE_B.9*` - `DRONE_B.13*`;
- текущего локального кода;
- текущих тестов и CLI entrypoints.

Проверки на момент анализа:

```text
cargo test --workspace                         passed
cargo clippy --all-targets -- -D warnings      passed
cargo run -q -p swarm-examples --bin regression_runner -- --jobs 4
                                                passed
cargo run -q -p swarm-examples --bin strategy_comparison -- --smoke --mission all --jobs 4
                                                runs, but report rows have wrong mission/scenario identity
```

## Короткий вывод

Предыдущий линейный план был:

```text
M25 Benchmark Parallelization
-> M26 Mission / Strategy Correctness
-> M27 Mission Semantics Layer
-> M28 Planner Quality Upgrade
-> M29 Stress & Regression Harness
-> M30 New Mission Prototype
-> M31 Simulation Realism Foundation
-> M32 Decision Point
```

Фактически M25-M31 уже реализованы, но не все одинаково глубоко.

Текущий лучший линейный план:

```text
M32 Reporting & Metrics Hardening
-> M33 Mission Semantics Integration
-> M34 Planner Correctness v2
-> M35 Dynamic Mission Correctness
-> M36 Regression Harness v2
-> M37 Realism Scenario Pack
-> M38 Wildfire / Flood v2
-> M39 Decision Point
```

Почему так:

- нельзя строить новый benchmark/research layer, пока `--mission all` неправильно экспортирует mission/scenario;
- `MissionAdapter` пока интерфейс, а не реально используемый semantic layer;
- planner layer есть, но route metrics and feasibility ещё слабые;
- wildfire/flood есть как prototype, но dynamic mission semantics неустойчивы;
- regression harness есть, но thresholds and baselines требуют калибровки;
- realism есть как preset/foundation, но не как scenario pack and analysis.

## Что считается уже закрытым

### Закрыто хорошо

- M18-M24: platform consolidation, DSL validation, benchmark pack, replay CLI, mock SITL, README golden path.
- M25: rayon parallelization and `--jobs`.

### Закрыто частично

- M26: correctness mostly documented/classified, but not all algorithmic issues fixed.
- M27: semantics interface exists, but adapters are not integrated.
- M28: planner trait and implementations exist, but planner impact is narrow.
- M29: regression harness runs, but thresholds are weak.
- M30: wildfire prototype exists, but not complete mission semantics.
- M31: realism foundation exists, but not scenario/research layer.

### Не закрыто

- decision point after M25-M31 was not really performed;
- current state needs hardening before choosing publication/SITL/visualization/API as the next main track.

## M32 - Reporting & Metrics Hardening

Цель:

> сделать benchmark output достоверным после появления `--mission all`, wildfire and realism.

Проблема:

`strategy_comparison --smoke --mission all --output-dir ...` запускается, но в JSON/CSV/table строки
для `sar`, `inspection`, `wildfire`, `emergency-mesh` получают `mission="coverage"` and
`scenario="coverage"`. Это происходит потому, что exporter берёт первый mission/scenario из
`ComparisonReport`, а merged report содержит много миссий.

Это критично:

- benchmark pack выглядит воспроизводимым, но rows имеют неверные labels;
- downstream analysis может сравнивать не те миссии;
- baseline/regression reports могут быть неверно интерпретированы;
- свежий `docs/BENCHMARK_RESULTS.md` нельзя честно обновлять до фикса.

Что сделать:

1. Изменить представление report rows:
   - добавить per-row mission;
   - добавить per-row scenario;
   - сохранить strategy/profile/metrics;
   - не использовать `mission_names.first()` для каждой строки.
2. Исправить `merge_reports`:
   - не терять исходный mission;
   - не создавать ambiguous profile names;
   - сохранять stable row keys.
3. Обновить exporters:
   - JSON;
   - CSV;
   - Markdown;
   - manifest.
4. Добавить missing metrics в exports:
   - wildfire/hazard metrics;
   - planner metrics;
   - realism metadata.
5. Обновить docs:
   - README Current Status;
   - README Known Limitations;
   - docs/BENCHMARK_RESULTS.md.

Критерии готовности:

- smoke all output has correct per-row mission and scenario;
- JSON/CSV/table agree on row identity;
- wildfire metrics are exported;
- planner metrics are exported or explicitly marked unavailable;
- report tests cover mixed-mission reports.

Тесты:

### Tests that need no refactoring

- CLI integration test: `--smoke --mission all --output-dir tempdir`;
- JSON row assertion for SAR rows;
- CSV row assertion for wildfire rows;
- markdown row assertion for mixed missions;
- unit test for `merge_reports`.

### Tests that need light refactoring

- replace `/tmp/...` test paths with test-owned tempdirs;
- create shared report fixture builders;
- create JSON/CSV parsing helpers for tests.

### Tests that need heavy refactoring

- schema compatibility tests for report v0.1 vs next version;
- golden benchmark pack comparison;
- property test for report row identity and uniqueness.

## M33 - Mission Semantics Integration

Цель:

> превратить `MissionAdapter` из заготовки в реально используемый semantic layer.

Проблема:

Сейчас есть:

- `TaskKind`;
- `RunState`;
- `MissionAdapter` trait;
- `Allocator::allocate_with_adapter`.

Но нет:

- concrete adapters;
- реального вызова adapter path в runner;
- adapter-driven completion;
- adapter-driven scoring;
- adapter-driven route cost;
- adapter-driven validation.

Что сделать:

1. Реализовать adapters:
   - `CoverageAdapter`;
   - `SarAdapter`;
   - `InspectionAdapter`;
   - `RelayAdapter`;
   - `WaypointAdapter`;
   - `WildfireAdapter`.
2. Определить ownership слоя:
   - что делает adapter;
   - что остаётся в runner;
   - что остаётся в allocator.
3. Провести adapter path через runner:
   - build `RunState`;
   - call `is_completed`;
   - call `route_cost` where applicable;
   - use `score` for mission-aware assignment where applicable.
4. Решить API для allocators:
   - либо strategies получают adapter;
   - либо adapter применяется до allocator как transformation/scoring layer.
5. Обновить DSL validation:
   - task kind;
   - required fields;
   - compatibility.

Критерии готовности:

- есть at least 4 concrete adapters: coverage, SAR, inspection, wildfire;
- runner uses adapters for completion checks;
- validation catches kind/field mismatch;
- old scenarios remain compatible;
- support matrix reflects adapter coverage.

Тесты:

### Tests that need no refactoring

- unit tests for each adapter's `task_kind`;
- unit tests for adapter completion with in-memory `RunState`;
- validation tests for missing `grid_cell`, `edge_id`, `pose`;
- serialization tests for `TaskKind`.

### Tests that need light refactoring

- shared task builders by kind;
- reusable RunState fixtures;
- small mission lifecycle helpers.

### Tests that need heavy refactoring

- full DSL -> adapter -> allocation -> runner -> replay -> report tests;
- property tests for valid task kind fields;
- compatibility suite for legacy scenarios without `kind`.

## M34 - Planner Correctness v2

Цель:

> сделать route planning measurable, correct and useful beyond a narrow CBBA hook.

Проблема:

M28 added a planner layer, but:

- planner is primarily wired into CBBA;
- route metrics are rough;
- `avg_wasted_travel` is always effectively zero;
- `infeasible_routes` is not meaningful;
- `BatteryAwarePlanner::order` needs verification and likely fix.

Что сделать:

1. Fix battery-aware feasibility:
   - feasibility should be checked on candidate ordered subset;
   - route should drop tasks until the remaining route is feasible;
   - reserve fraction should be honored.
2. Make route metrics meaningful:
   - route length;
   - wasted travel;
   - return reserve;
   - infeasible routes;
   - dropped-by-planner tasks.
3. Compare planners:
   - nearest-neighbour;
   - two-opt;
   - battery-aware.
4. Decide planner scope:
   - CBBA only;
   - CBBA + centralized;
   - generic post-allocation route ordering.
5. Revisit SAR unsupported statuses:
   - static centralized SAR may remain unsupported;
   - CBBA SAR may need mission-aware release/replan semantics;
   - document final decision.

Критерии готовности:

- battery-aware planner has strong unit tests;
- benchmark exposes non-zero route metrics where expected;
- planner comparison is reproducible;
- SAR unsupported statuses are either fixed or intentionally documented with tests.

Тесты:

### Tests that need no refactoring

- `BatteryAwarePlanner::order` infeasible route test;
- route metrics unit tests;
- CBBA with `--planner two-opt` smoke test;
- SAR CBBA/centralized documented status tests.

### Tests that need light refactoring

- route fixture builders;
- small deterministic battery-constrained scenarios;
- benchmark parser helpers for route metrics.

### Tests that need heavy refactoring

- planner property tests;
- dynamic replanning tests after task release;
- long-run planner comparison.

## M35 - Dynamic Mission Correctness

Цель:

> устранить semantic mismatch in dynamic missions, especially SAR and wildfire.

Проблема:

Dynamic tasks and dynamic completion are now central:

- SAR releases scanned tasks and reassigns agents;
- wildfire changes priorities and threat levels;
- inspection covers edges based on movement;
- task completion and success are not always semantically aligned.

Known symptoms:

- SAR CBBA/centralized remain unsupported;
- wildfire medium-dynamic can show completion and success mismatch;
- inspection perimeter can have high edge coverage but low success due to battery/time constraints.

Что сделать:

1. Define dynamic mission success semantics:
   - SAR;
   - inspection;
   - wildfire.
2. Fix wildfire medium-dynamic:
   - decide what success means;
   - align completion/success;
   - update metrics.
3. Revisit SAR release/replan:
   - scanned cell should not create stale reassignment loop;
   - CBBA re-convergence should not inflate unassigned timeout incorrectly;
   - centralized should be either dynamic or explicitly static unsupported.
4. Add support matrix reasons:
   - unsupported because static pre-plan;
   - unsupported because delayed reconvergence;
   - experimental because physically constrained.

Критерии готовности:

- dynamic mission success rules are documented;
- wildfire medium-dynamic has explainable success/completion behavior;
- SAR unsupported statuses are precise and tested;
- support matrix is generated or at least tested against code expectations.

Тесты:

### Tests that need no refactoring

- wildfire success/completion consistency test;
- SAR release/replan deterministic tests;
- inspection perimeter edge coverage vs success tests;
- support matrix tests.

### Tests that need light refactoring

- dynamic mission fixture builders;
- small deterministic SAR and wildfire scenarios;
- shared mission outcome assertions.

### Tests that need heavy refactoring

- dynamic replanning property tests;
- multi-seed dynamic mission regression;
- strategy cross-product support matrix generation.

## M36 - Regression Harness v2

Цель:

> make regression meaningful enough to guard future work.

Проблема:

Regression currently runs and passes, but:

- some thresholds are too weak;
- baseline is old;
- stress profile naming does not always match actual scenario conditions;
- tests use `/tmp`;
- wildfire and realism are not fully represented.

Что сделать:

1. Calibrate thresholds:
   - no meaningless `success_rate >= 0.0`;
   - separate smoke thresholds from quick thresholds;
   - add mission-specific metrics.
2. Refresh baseline:
   - after M32-M35;
   - commit current baseline;
   - document update process.
3. Expand suites:
   - wildfire small-static;
   - wildfire medium-dynamic;
   - realism smoke;
   - SAR supported strategies;
   - inspection perimeter experimental thresholds.
4. Fix portability:
   - use tempdir-managed test paths;
   - no machine-specific absolute paths.
5. Improve failure output:
   - include metric, actual, threshold, delta;
   - include suite mode and seed range.

Критерии готовности:

- regression fails on intentionally bad thresholds;
- default thresholds are meaningful;
- baseline commit matches current code;
- tests are portable.

Тесты:

### Tests that need no refactoring

- threshold checker tests;
- forced failure tests;
- baseline compare tests.

### Tests that need light refactoring

- tempdir conversion;
- shared baseline fixtures;
- suite builders.

### Tests that need heavy refactoring

- statistical threshold tests;
- 1000-seed regression mode;
- historical baseline migration tests.

## M37 - Realism Scenario Pack

Цель:

> turn M31 realism foundation into reproducible scenarios and analysis.

Проблема:

Realism exists mostly as `--realism` preset and low-level fields:

- battery model v2;
- altitude;
- wind/noise;
- comms jitter;
- sensor altitude penalty;
- time-gated no-fly zones.

But there is no dedicated scenario pack or benchmark analysis.

Что сделать:

1. Add realism profiles:
   - light;
   - medium;
   - heavy.
2. Add scenario JSON files:
   - coverage realism;
   - SAR realism;
   - inspection realism;
   - wildfire realism.
3. Add output metadata:
   - realism profile;
   - wind;
   - pose noise;
   - comms jitter;
   - battery model.
4. Compare:
   - baseline simulation;
   - realism-enabled simulation.
5. Update docs:
   - what is modeled;
   - what remains simplified;
   - expected impact.

Критерии готовности:

- realism scenarios load from `scenarios/`;
- benchmark outputs realism metadata;
- docs explain current realism limits;
- regression has at least one realism smoke suite.

Тесты:

### Tests that need no refactoring

- scenario load tests;
- battery model tests;
- sensor altitude tests;
- no-fly time window tests.

### Tests that need light refactoring

- deterministic noise fixtures;
- realism manifest assertions;
- scenario builder helpers.

### Tests that need heavy refactoring

- stochastic realism regression;
- full old vs realism comparison;
- SITL-aligned trajectory tests.

## M38 - Wildfire / Flood v2

Цель:

> make wildfire/flood a real first-class mission.

Проблема:

Wildfire currently proves extensibility, but it is still thin:

- profiles exist in code, not scenario catalog;
- dynamic threat is simple;
- success semantics need clarification;
- metrics exports are incomplete.

Что сделать:

1. Add scenario files:
   - `wildfire.small-static.json`;
   - `wildfire.medium-dynamic.json`;
   - optionally `flood.medium-dynamic.json`.
2. Add DSL docs:
   - hazard zones;
   - threat level;
   - priority;
   - dynamic update interval;
   - mapping completion.
3. Improve dynamic behavior:
   - task injection;
   - zone expansion;
   - priority-based reallocation;
   - optional time windows.
4. Improve metrics:
   - hazard zones mapped;
   - high-priority zones mapped;
   - priority updates;
   - final average threat;
   - time to map first high-risk zone.
5. Integrate replay:
   - hazard map summary;
   - hazard events in replay CLI;
   - optional ASCII overlay later.

Критерии готовности:

- wildfire scenario files are in catalog tests;
- wildfire benchmark is documented;
- metrics are exported in JSON/CSV/table;
- medium-dynamic behavior is explainable.

Тесты:

### Tests that need no refactoring

- scenario load tests;
- wildfire smoke tests;
- success/completion consistency tests;
- replay event roundtrip tests.

### Tests that need light refactoring

- hazard fixtures;
- benchmark output parsers;
- mission outcome assertions.

### Tests that need heavy refactoring

- dynamic hazard property tests;
- multi-seed wildfire benchmark;
- visualization overlay tests.

## M39 - Decision Point

Цель:

> выбрать следующий большой стратегический track после hardening.

К этому моменту должны быть закрыты:

- report identity;
- mission semantics integration;
- planner correctness;
- dynamic mission correctness;
- calibrated regression;
- realism scenarios;
- wildfire v2.

Что оценить:

1. Research readiness:
   - есть ли свежие benchmark numbers;
   - достаточно ли regression confidence;
   - можно ли делать publishable report.
2. Robotics readiness:
   - достаточно ли mission semantics для SITL upload;
   - достаточно ли safety validation;
   - стоит ли идти в PX4.
3. Product/platform readiness:
   - стабильны ли schemas;
   - можно ли документировать extension API;
   - нужен ли UI.
4. Algorithmic readiness:
   - остались ли SAR/CBBA/centralized gaps;
   - есть ли смысл делать dynamic reallocation and communication-aware CBBA.

Возможные следующие tracks:

| Если главное | Следующий track |
|---|---|
| Доказательные результаты | Research Benchmark Depth |
| Реальные роботы / SITL | Real SITL / PX4 Bridge |
| Удобный анализ | Replay / Visualization |
| Новые алгоритмы | Planner / Algorithm Research |
| Новые пользователи | Platform / API Extensibility |
| Более реалистичная симуляция | Simulation Realism v3 |

Критерии готовности:

- written decision report;
- selected next track;
- explicit non-goals for unselected tracks;
- next milestone has implementation-level scope.

Тесты:

### Tests that need no refactoring

- не требуется, если M39 остаётся аналитическим checkpoint.

### Tests that need light refactoring

- optional scripts for summarizing benchmark status.

### Tests that need heavy refactoring

- не требуется до выбора track.

## Что не делать в этом линейном плане

### Не начинать с PX4/SITL

Причина:

- внешний setup;
- harder portable tests;
- current semantics/reporting gaps would leak into SITL path.

### Не начинать с UI

Причина:

- replay schema and report schema still need hardening;
- UI would visualize unstable semantics.

### Не начинать с public API stabilization

Причина:

- MissionAdapter is not integrated yet;
- report schema needs correction;
- premature API stability would freeze wrong abstractions.

### Не делать publishable benchmark first

Причина:

- docs/BENCHMARK_RESULTS.md is stale;
- report identity bug affects mixed mission output;
- regression thresholds need calibration.

## Итоговый порядок

```text
M32 Reporting & Metrics Hardening
M33 Mission Semantics Integration
M34 Planner Correctness v2
M35 Dynamic Mission Correctness
M36 Regression Harness v2
M37 Realism Scenario Pack
M38 Wildfire / Flood v2
M39 Decision Point
```

Первый практический шаг:

> M32 Reporting & Metrics Hardening.

Начать нужно с исправления per-row `mission`/`scenario` в `--mission all`, потому что это сейчас
самый конкретный дефект, который мешает доверять benchmark artifacts.
