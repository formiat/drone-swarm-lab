# DRONE_A.14.branches - Ветки развития после M31

Дата фиксации: 2026-05-27

## Контекст

Этот документ обновляет карту направлений после изучения:

- `docs_raw/DRONE_A.9.branches.md`;
- `docs_raw/DRONE_A.9.linear.md`;
- `docs_raw/DRONE_A.10.linear.md`;
- `docs_raw/DRONE_A.11.publish.md`;
- `docs_raw/DRONE_A.12.md`;
- `docs_raw/DRONE_A.13.branches.md`;
- `docs_raw/DRONE_A.13.linear.md`;
- `docs_raw/DRONE_B.9.branches.md`;
- `docs_raw/DRONE_B.9.linear.md`;
- `docs_raw/DRONE_B.10.linear.md`;
- `docs_raw/DRONE_B.11.publish.md`;
- `docs_raw/DRONE_B.12.md`;
- `docs_raw/DRONE_B.13.branches.md`;
- `docs_raw/DRONE_B.13.linear.md`;
- текущего локального кода и последних коммитов.

Проверки на момент анализа:

```text
cargo test --workspace                         passed
cargo clippy --all-targets -- -D warnings      passed
cargo run -q -p swarm-examples --bin regression_runner -- --jobs 4
                                                passed
cargo run -q -p swarm-examples --bin strategy_comparison -- --smoke --mission all --jobs 4
                                                runs, but exposes report identity issue
```

## Короткий вывод

Предыдущий линейный ствол в основном пройден:

- M18-M24: scenario catalog, validation, SITL mock path, benchmark pack, report, replay, golden path;
- M25: benchmark parallelization через rayon;
- M26: strategy correctness hardening, support matrix, CBBA bundle-slot fix;
- M27: `TaskKind`, `RunState`, `MissionAdapter` trait;
- M28: `RoutePlanner`, 2-opt, battery-aware planner;
- M29: regression suites, thresholds, baseline support;
- M30: wildfire / flood mapping prototype;
- M31: simulation realism foundation.

Но статус не равномерный.

Часть пунктов действительно готова как стабильная инфраструктура. Часть сделана как skeleton/foundation
и требует следующего hardening pass. Поэтому новый набор веток не должен начинаться с большой новой фичи.
Сначала нужен общий слой:

> Reporting, metrics, semantics and correctness hardening.

После него можно выбирать дальнейший фокус:

1. глубже интегрировать mission semantics;
2. усиливать алгоритмы;
3. доводить wildfire/flood до настоящей динамической миссии;
4. углублять benchmark/research;
5. развивать realism;
6. делать replay/visualization;
7. идти в real SITL/PX4;
8. стабилизировать platform/API.

## Что уже сделано из линейных планов

### M18-M24

Статус:

> в целом закрыто.

Что есть:

- JSON scenario suites;
- schema version;
- validation API;
- scenario catalog smoke tests;
- mock SITL path;
- benchmark pack with manifest/results/table;
- replay CLI;
- README golden path;
- docs split: `docs/` для пользовательских документов, `docs_raw/` для рабочих отчётов.

Оценка:

> этот слой достаточно хороший для research prototype.

Что всё ещё требует внимания:

- docs/BENCHMARK_RESULTS.md устарел относительно текущего commit и новых фич;
- README местами противоречив: пишет Simulation Realism stable, но Known Limitations всё ещё говорят про "2D world" и "real-world noise is not modeled".

### M25 Benchmark Parallelization

Статус:

> сделано хорошо.

Что есть:

- `rayon`;
- `BenchmarkOptions.jobs`;
- `--jobs`;
- deterministic aggregation order;
- test `jobs=1` vs `jobs=4`.

Оценка:

> можно считать полноценным улучшением, а не только заготовкой.

### M26 Mission / Strategy Correctness

Статус:

> сделано частично.

Что есть:

- strategy support matrix;
- documented status для SAR + CBBA и SAR + centralized;
- tests на inspection consistency и SAR deterministic documented status;
- CBBA bundle-slot fix.

Что осталось:

- SAR + CBBA всё ещё unsupported;
- SAR + centralized всё ещё unsupported;
- часть проблем классифицирована, но не устранена;
- report identity для `--mission all` сломан: строки `sar`, `inspection`, `wildfire`, `emergency-mesh` экспортируются с `mission="coverage"`.

Оценка:

> хороший диагностический milestone, но не финальный correctness milestone.

### M27 Mission Semantics Layer

Статус:

> skeleton есть, полноценной интеграции нет.

Что есть:

- `TaskKind`;
- `RunState`;
- `MissionAdapter` trait;
- task kinds для coverage/SAR/inspection/relay/waypoint/mapping zone.

Что отсутствует:

- concrete `impl MissionAdapter`;
- runner не вызывает `MissionAdapter::is_completed`;
- allocators по умолчанию игнорируют `allocate_with_adapter`;
- scoring/route/completion logic всё ещё в основном размазана по runner/scenario-specific code.

Оценка:

> архитектурная точка входа создана, но это не stable semantics layer.

### M28 Planner Quality Upgrade

Статус:

> частично реализовано.

Что есть:

- `RoutePlanner`;
- `NearestNeighbourPlanner`;
- `TwoOptPlanner`;
- `BatteryAwarePlanner`;
- planner option в CLI;
- CBBA route ordering.

Что вызывает сомнение:

- planner реально подключён в основном к CBBA;
- route metrics в runner пока грубые;
- `avg_wasted_travel` и `infeasible_routes` практически не наполнены смыслом;
- `BatteryAwarePlanner::order` проверяет feasibility по исходному `tasks`, а не по текущему усечённому route, поэтому его нужно перепроверить и, вероятно, исправить.

Оценка:

> хороший foundation, но planner layer ещё не стал полноценной системой качества маршрутов.

### M29 Stress & Regression Harness

Статус:

> работает, но thresholds пока слабые.

Что есть:

- `RegressionSuite`;
- `ThresholdChecker`;
- `RegressionRunner`;
- CLI `regression_runner`;
- `strategy_comparison --regression`;
- baseline file;
- default suites.

Что осталось:

- часть thresholds слишком мягкая (`success_rate >= 0.0`);
- suite `cbba_stress_pl_0_2` фактически не моделирует packet loss 0.2 как отдельный профиль;
- baseline привязан к старому commit;
- tests местами используют `/tmp` напрямую;
- wildfire metrics не полноценно попали в default regression.

Оценка:

> regression harness как механизм есть, но его надо калибровать.

### M30 Wildfire / Flood Mapping

Статус:

> прототип есть.

Что есть:

- `WildfireProfile`;
- small-static и medium-dynamic;
- `TaskKind::MappingZone`;
- `WildfireState`;
- hazard zones;
- dynamic threat update;
- replay events;
- smoke tests.

Что осталось:

- `medium-dynamic` в smoke может давать `Completion=1.0`, но `Success=0.0`;
- task reprioritization есть, но не превращён в полноценную dynamic mission loop;
- нет scenario JSON в `scenarios/`;
- wildfire metrics не экспортируются полноценно в JSON/CSV/table;
- нет отдельной документации mission semantics.

Оценка:

> хорошая демонстрация расширяемости, но не законченная миссия уровня SAR/inspection.

### M31 Simulation Realism Foundation

Статус:

> foundation есть.

Что есть:

- `Pose.z`;
- battery model v2;
- altitude sensor penalty;
- wind drift;
- pose noise;
- comms jitter;
- time-gated no-fly zones;
- `--realism` preset.

Что осталось:

- realism почти не представлен через scenario DSL packs;
- нет dedicated realism benchmark report;
- README Known Limitations устарел;
- нет сравнительного анализа old vs realism-enabled;
- нет профилей типа light/medium/heavy realism.

Оценка:

> это хороший базовый слой, но не полноценный realism track.

## Ветка 1 - Reporting & Metrics Hardening

Статус:

> обязательная ближайшая ветка.

Суть:

> сделать отчёты и метрики честными после добавления новых mission types и `--mission all`.

Почему первая:

- `--mission all` сейчас запускается, но экспортирует неверный `mission`/`scenario` для строк не-coverage;
- `export_json`, `export_csv` и `Display` берут первый mission/scenario из report;
- это ломает доверие к benchmark pack;
- без этого нельзя делать fresh benchmark report или research baseline.

Что сделать:

1. Изменить модель report rows:
   - mission per row;
   - scenario per row;
   - profile без двойного mission-prefix или с явно описанной схемой;
   - stable row id.
2. Исправить `merge_reports`:
   - не терять mission identity;
   - не строить `coverage/sar/ideal` через косвенное поле;
   - сохранять per-mission metadata.
3. Обновить exporters:
   - JSON;
   - CSV;
   - Markdown table;
   - manifest.
4. Добавить wildfire metrics в export schema:
   - hazard zones mapped;
   - priority updates;
   - final threat level.
5. Обновить docs:
   - README current status;
   - Known Limitations;
   - BENCHMARK_RESULTS.md.

Done criteria:

- `--smoke --mission all --output-dir ...` даёт per-row `mission` = реальная миссия;
- JSON/CSV/table согласованы;
- manifest не вводит в заблуждение;
- wildfire metrics доступны в machine-readable output;
- есть regression test на mixed mission export.

Тесты без рефакторинга:

- integration test для `--smoke --mission all --output-dir`;
- JSON assertion: rows with `profile` containing `sar/` have `mission = "sar"`;
- CSV assertion на per-row mission/scenario;
- markdown assertion на mixed mission rows;
- unit test на `merge_reports`.

Тесты с лёгким рефакторингом:

- shared parser helpers для benchmark output;
- tempdir-managed output dirs вместо `/tmp`;
- helper для создания synthetic `ComparisonReport`.

Тесты с тяжёлым рефакторингом:

- schema compatibility tests для старого и нового report format;
- golden pack comparison across versions;
- property test на report row identity.

## Ветка 2 - Mission Semantics Deep Integration

Статус:

> важная архитектурная ветка после report hardening.

Суть:

> превратить `MissionAdapter` из интерфейса в реально используемый слой.

Что сделать:

1. Реализовать adapters:
   - `CoverageAdapter`;
   - `SarAdapter`;
   - `InspectionAdapter`;
   - `RelayAdapter`;
   - `WaypointAdapter`;
   - `WildfireAdapter`.
2. Провести adapters через runner:
   - completion checks;
   - scoring context;
   - route cost;
   - task validation;
   - replay event enrichment.
3. Обновить allocator boundary:
   - либо реально использовать `allocate_with_adapter`;
   - либо убрать/пересобрать API, если adapters должны жить выше allocators.
4. Перенести mission-specific completion из ad hoc runner blocks в adapter layer там, где это разумно.
5. Обновить SCENARIO_DSL docs:
   - task kind;
   - required fields;
   - completion semantics;
   - unsupported strategy combinations.

Почему это важно:

- текущие known failures SAR + CBBA/centralized похожи на mismatch semantics/planner;
- wildfire/flood будет расти, и ad hoc blocks в runner станут хрупкими;
- SITL conversion в waypoints тоже требует ясной task semantics.

Done criteria:

- есть concrete adapters;
- хотя бы SAR/inspection/wildfire completion использует adapter path;
- tests доказывают, что task kind определяет required fields и completion;
- README не называет Mission Semantics stable, пока adapter path не используется.

Тесты без рефакторинга:

- unit tests для каждого adapter;
- validation tests task kind -> required fields;
- completion tests для SAR cell, inspection edge, mapping zone, waypoint.

Тесты с лёгким рефакторингом:

- shared task builders по kind;
- in-memory RunState fixtures;
- scenario snippets для adapter lifecycle.

Тесты с тяжёлым рефакторингом:

- full lifecycle tests DSL -> adapter -> allocation -> runner -> replay -> report;
- property tests: valid task kind always has required semantic fields;
- backward compatibility tests для старых scenarios без `kind`.

## Ветка 3 - Planner & Algorithm Correctness v2

Статус:

> нужна после semantics, но часть можно делать параллельно с hardening.

Суть:

> сделать planner quality не только trait-ом, а реально измеряемым улучшением маршрутов и аллокации.

Что сделать:

- исправить/проверить `BatteryAwarePlanner`;
- считать route metrics не только через CBBA bundle distance;
- наполнить смыслом:
  - `avg_route_length`;
  - `avg_wasted_travel`;
  - `avg_return_reserve`;
  - `avg_infeasible_routes`;
- подключить planner choice к стратегиям, где это имеет смысл;
- сделать benchmark comparison:
  - nearest-neighbour;
  - two-opt;
  - battery-aware;
- отдельно решить судьбу SAR + centralized и SAR + CBBA:
  - либо починить;
  - либо оформить как intentional unsupported с более строгой причиной;
  - либо ввести mission-aware planner variant.

Done criteria:

- planner comparison показывает measurable route effect;
- battery-aware planner не назначает физически невозможные bundles;
- route metrics ненулевые и объяснимые;
- SAR unsupported statuses не являются просто "потом починим".

Тесты без рефакторинга:

- unit test на `BatteryAwarePlanner::order` после удаления задач;
- route metrics unit tests;
- CBBA planner choice CLI smoke;
- regression tests для SAR unsupported reasons.

Тесты с лёгким рефакторингом:

- route fixture builders;
- benchmark assertion helper для route metrics;
- fake battery-constrained scenario helpers.

Тесты с тяжёлым рефакторингом:

- property tests на planner feasibility;
- comparative long-run route quality benchmarks;
- dynamic replanning tests under failures and task releases.

## Ветка 4 - Dynamic Mission / Wildfire v2

Статус:

> продолжение M30.

Суть:

> довести wildfire/flood от prototype до полноценной dynamic mission.

Что сделать:

- добавить JSON scenarios в `scenarios/`;
- описать wildfire DSL;
- сделать priority updates meaningful для allocation, а не только event/field update;
- добавить dynamic task injection или dynamic zone expansion;
- определить success semantics:
  - mapped zones;
  - high-priority zones;
  - time to map;
  - final threat level;
  - safety violations;
- устранить mismatch `Completion=1.0`, `Success=0.0` на medium-dynamic;
- экспортировать wildfire metrics;
- добавить replay summary for hazard events.

Done criteria:

- wildfire small-static и medium-dynamic имеют понятные success rules;
- metrics видны в JSON/CSV/table;
- scenario files проходят catalog tests;
- regression suite покрывает wildfire.

Тесты без рефакторинга:

- wildfire scenario load test;
- smoke benchmark for small-static and medium-dynamic;
- success/completion consistency test;
- replay event roundtrip for hazard updates.

Тесты с лёгким рефакторингом:

- hazard map fixture builders;
- helper для parsing wildfire rows из benchmark output;
- threshold fixtures for wildfire regression.

Тесты с тяжёлым рефакторингом:

- property tests for dynamic hazard updates;
- long-run comparison static vs dynamic;
- visualization overlay tests after UI exists.

## Ветка 5 - Regression & Research Benchmark Depth

Статус:

> mechanism exists, calibration missing.

Суть:

> превратить regression harness из "запускается" в "ловит реальные деградации".

Что сделать:

- усилить thresholds:
  - убрать `success_rate >= 0.0` там, где это не проверка;
  - добавить mission-specific thresholds;
  - разделить smoke и quick critical suites;
- исправить profile modeling для CBBA stress;
- обновить baseline на текущий commit после hardening;
- добавить baseline update policy;
- добавить confidence intervals для quick/full;
- зафиксировать expected variance;
- добавить generated report по regression deltas.

Done criteria:

- regression реально падает на meaningful degradation;
- baseline соответствует текущему commit;
- thresholds объяснены в docs;
- test output не зависит от машинных абсолютных путей.

Тесты без рефакторинга:

- forced threshold failure unit tests;
- regression_runner CLI pass/fail tests;
- baseline compare tests.

Тесты с лёгким рефакторингом:

- перейти с `/tmp` на tempdir-managed paths;
- shared baseline fixtures;
- deterministic stress profile fixtures.

Тесты с тяжёлым рефакторингом:

- statistical regression tests;
- 1000-seed full-run comparison;
- historical baseline compatibility.

## Ветка 6 - Simulation Realism v2

Статус:

> foundation exists, scenario/research layer missing.

Суть:

> сделать realism не только CLI preset, а воспроизводимым набором сценариев и метрик.

Что сделать:

- добавить realism profiles:
  - light;
  - medium;
  - heavy;
- добавить scenario JSON с altitude/battery/sensor/noise;
- добавить benchmark comparison old vs realism-enabled;
- обновить docs:
  - что именно моделируется;
  - что не моделируется;
  - какие assumptions;
- добавить realism fields в manifest;
- добавить failure taxonomy для realism-induced failures.

Done criteria:

- есть dedicated realism scenarios;
- `--realism` отражён в output metadata;
- benchmark показывает impact realism features;
- README Known Limitations обновлён.

Тесты без рефакторинга:

- battery model v2 tests;
- altitude sensor penalty tests;
- wind/noise deterministic tests;
- no-fly time window tests.

Тесты с лёгким рефакторингом:

- deterministic noise provider fixtures;
- realism profile parser fixtures;
- manifest assertions.

Тесты с тяжёлым рефакторингом:

- stochastic realism regression;
- simulation-vs-SITL trajectory alignment;
- multi-agent environmental stress tests.

## Ветка 7 - Replay / Visualization

Статус:

> replay CLI есть, UI нет.

Суть:

> сделать поведение миссий видимым, особенно для SAR, inspection, wildfire and realism.

Что сделать:

- расширить replay summary новыми events/metrics;
- добавить hazard map summary;
- добавить inspection graph summary;
- добавить SAR belief summary;
- затем сделать UI:
  - egui или Bevy;
  - timeline;
  - map/grid view;
  - BeliefMap overlay;
  - InspectionGraph overlay;
  - Wildfire hazard overlay;
  - agent trajectories.

Done criteria для ближайшего шага:

- replay CLI показывает wildfire and realism events;
- event schema стабильна;
- UI не блокирует headless benchmark path.

Тесты без рефакторинга:

- replay summary tests для hazard events;
- replay JSON roundtrip tests;
- ASCII snapshot tests.

Тесты с лёгким рефакторингом:

- reusable replay fixtures;
- event log builders for mission-specific events.

Тесты с тяжёлым рефакторингом:

- UI rendering tests;
- screenshot/pixel tests;
- interactive timeline tests.

## Ветка 8 - Real SITL / PX4 Bridge

Статус:

> experimental branch, not next default.

Суть:

> превратить feature-gated PX4/MAVLink scaffold в реальный end-to-end workflow.

Что сделать:

- real `MavlinkTransport` connection workflow;
- mission upload:
  - MISSION_COUNT;
  - MISSION_ITEM_INT;
  - mission ack handling;
- telemetry -> task status;
- arm/takeoff/execute/abort;
- single-agent SITL golden path;
- later multi-agent SITL;
- clear safety validation before upload.

Почему не первая:

- требует внешнего PX4/SITL окружения;
- tests сложнее сделать portable;
- mission semantics and reporting still need hardening.

Done criteria:

- один агент проходит waypoints через PX4 SITL;
- mock path остаётся fully portable;
- docs distinguish mock, SITL and real hardware.

Тесты без рефакторинга:

- mock transport tests;
- waypoint conversion tests;
- CLI validation tests.

Тесты с лёгким рефакторингом:

- fake MAVLink transport;
- typed error fixtures;
- SITL command dry-run mode.

Тесты с тяжёлым рефакторингом:

- real PX4 SITL integration tests;
- multi-agent SITL;
- hardware-in-the-loop tests.

## Ветка 9 - Platform / API Extensibility

Статус:

> полезно после semantics hardening.

Суть:

> сделать проект удобной платформой для новых стратегий, миссий и экспериментов.

Что сделать:

- stable internal APIs:
  - strategy;
  - mission adapter;
  - runner;
  - report rows;
  - replay events;
- scenario generator API;
- extension docs:
  - how to add mission;
  - how to add strategy;
  - how to add metric;
- schema version policy;
- deprecation policy;
- plugin-like strategy registration.

Риск:

- преждевременная API stabilization может зацементировать неправильные semantics.

Done criteria:

- documented path для новой mission;
- documented path для новой strategy;
- stable report/replay schema;
- example external-ish strategy test.

Тесты без рефакторинга:

- doc tests or integration tests for extension examples;
- schema roundtrip tests;
- strategy registry tests.

Тесты с лёгким рефакторингом:

- example crate or fixture module;
- shared scenario generator fixtures.

Тесты с тяжёлым рефакторингом:

- external strategy harness;
- compatibility tests across schema versions;
- semver-oriented API checks.

## Зависимости между ветками

```text
Reporting & Metrics Hardening
  -> Regression & Benchmark Depth
  -> Research publication

Mission Semantics Deep Integration
  -> Planner & Algorithm Correctness v2
  -> Dynamic Mission / Wildfire v2
  -> Real SITL / PX4 Bridge
  -> Platform / API Extensibility

Replay schema stability
  -> Visualization

Simulation Realism v2
  -> Realism benchmark
  -> SITL comparison
```

Практически:

1. Ветка 1 должна быть первой.
2. Ветка 2 должна идти до серьёзных новых mission/algorithm branches.
3. Ветка 5 нужна перед любым publishable benchmark.
4. Ветки 7 и 8 можно начать отдельно, но они будут дороже, если report/semantics останутся нестабильными.

## Итоговая рекомендация

Ближайшее направление:

> Reporting & Metrics Hardening.

После него:

1. Mission Semantics Deep Integration.
2. Planner & Algorithm Correctness v2.
3. Regression & Research Benchmark Depth.
4. Dynamic Mission / Wildfire v2 или Simulation Realism v2, в зависимости от желаемого фокуса.

SITL/PX4 и Visualization лучше пока держать как боковые ветки, а не как следующий обязательный шаг.
