# DRONE_B.15.linear — Линейный план без выбора большой ветки

Дата фиксации: 2026-05-28

## Короткий вывод

```
ML-1  Benchmark Trust
ML-2  Architecture Wiring
ML-3  Algorithmic Truth
ML-4  Regression Hardening
ML-5  Realism Pack
ML-6  Direction Decision
```

Порядок строго мотивирован зависимостями:

- нельзя доверять benchmark, пока report identity сломан и determinism не
  проверен → ML-1 первый;
- диагностика алгоритмических провалов без wired адаптеров — угадывание → ML-2
  до ML-3;
- calibrate regression thresholds нельзя, пока метрики нестабильны → ML-4
  после ML-1–ML-3;
- realism pack требует стабильной regression baseline → ML-5 после ML-4;
- выбор направления на основе фактического состояния, а не предположений → ML-6
  в конце.

SITL, Visualization, Research Benchmark, Platform/API и Disaster Mapping v2 —
в линейный ствол не входят. Они становятся равноправными ветками после ML-6.

---

## ML-1 — Benchmark Trust

**Цель:** сделать benchmark artifacts достоверными и воспроизводимыми.

### Проблемы

**1. Report identity bug.**

`ComparisonReport` хранит `mission_names: Vec<String>` на уровне всего report.
В `report_export.rs` и `benchmark.rs` при пустом `metrics.mission` fallback идёт
на `mission_names.first()`. При `--mission all` merge нескольких reports строки
для `sar`, `inspection`, `wildfire`, `emergency-mesh` могут получить
`mission="coverage"`. Это делает JSON/CSV/table ненадёжными.

**2. Нет детерминизм-теста.**

Нет формальной проверки: одинаковый конфиг с разным `--jobs` даёт одинаковые
aggregate metrics. Если есть недетерминированная агрегация — это скрытый bug.

**3. Wildfire success/completion mismatch.**

`medium-dynamic` показывает `Completion=1.0` при `Success=0.0`. Semantics не
определены — непонятно, что означает успешность wildfire миссии.

**4. Устаревшая документация.**

`docs/BENCHMARK_RESULTS.md` не отражает текущее состояние. README Known
Limitations противоречат статусу "Simulation Realism stable".

### Что сделать

1. Исправить per-row mission identity:
   - каждая row в JSON/CSV/Markdown должна содержать `mission` из исходного run,
     а не из `mission_names.first()`;
   - убрать fallback на `first()` когда `metrics.mission` заполнен корректно;
   - добавить integration test: `--smoke --mission all --output-dir tempdir` →
     JSON rows для SAR содержат `mission="sar"`, для inspection — `"inspection"`.
2. Добавить determinism test:
   - запустить с `--jobs 1` и `--jobs 4` на одних seeds;
   - aggregate metrics должны совпадать побитово или с epsilon;
   - если не совпадают — найти источник нondeterminism (unordered map iteration,
     order-dependent aggregation) и устранить.
3. Определить wildfire success semantics:
   - выбрать и зафиксировать критерий: все mapping tasks completed → success,
     или все high-priority zones mapped → success, или иной;
   - покрыть тестом: `small-static` и `medium-dynamic` имеют согласованные
     `success` и `completion`.
4. Обновить `docs/BENCHMARK_RESULTS.md` и README Known Limitations.
5. Обновить baseline после фиксов.

### Done criteria

- `--smoke --mission all --output-dir tempdir` даёт корректный per-row `mission`.
- JSON/CSV/Markdown согласованы по row identity.
- `--jobs 1` и `--jobs 4` дают одинаковые aggregate metrics.
- Wildfire `success` и `completion` согласованы на обоих сценариях.
- `docs/BENCHMARK_RESULTS.md` актуален.
- README не противоречит сам себе.

### Тесты

#### Без рефакторинга

- Integration test: `--smoke --mission all --output-dir tempdir`;
- JSON assertion: SAR rows имеют `mission="sar"`;
- CSV assertion: wildfire rows имеют `mission="wildfire"`;
- Determinism test: `--jobs 1` vs `--jobs 4` на одних seeds;
- Wildfire success/completion consistency test;
- Unit test на merge/report identity.

#### Лёгкий рефакторинг

- Заменить `/tmp/...` test paths на tempdir;
- Shared report fixture builders;
- JSON/CSV parsing helpers для тестов.

#### Тяжёлый рефакторинг

- Schema compatibility tests для report format;
- Property test на report row identity и uniqueness.

---

## ML-2 — Architecture Wiring

**Цель:** подключить реализованные адаптеры к реальным путям исполнения.

### Проблема

`CoverageAdapter`, `SarAdapter`, `InspectionAdapter`, `WildfireAdapter`,
`RelayAdapter`, `WaypointAdapter` — все реализованы в `swarm-types/src/adapter.rs`.
Но в `swarm-sim/src/runner.rs` adapter вызывается в одном месте. Completion
conditions, scoring и route cost остаются в ad hoc blocks в runner.

`BatteryAwarePlanner::order` проверяет feasibility — необходимо убедиться, что
проверка идёт по текущему кандидату маршрута, а не по исходному полному списку
задач. Если нет — исправить.

Без wiring адаптеров: диагностика алгоритмических провалов остаётся угадыванием,
потому что непонятно, какой контекст получает allocator.

### Что сделать

1. Провести `MissionAdapter::is_completed` через runner для всех mission types:
   - убрать или заменить ad hoc completion blocks;
   - runner вызывает `adapter.is_completed(task, &state)`;
   - старые сценарии должны работать без изменений.
2. Подключить adapter `score` для CBBA как минимум:
   - scoring идёт через adapter, не через hardcoded distance formula;
   - для других стратегий — там, где это не требует существенного рефакторинга.
3. Подключить `route_cost` из adapter там, где planner использует стоимость маршрута.
4. Проверить `BatteryAwarePlanner::order`:
   - feasibility проверяется по текущему усечённому маршруту;
   - добавить unit test: агент с ограниченной батареей отбрасывает ровно столько
     задач, сколько нужно для feasible route.
5. Проверить DSL validation через adapter:
   - `TaskKind::SarScan` без `grid_cell` → ошибка загрузки;
   - `TaskKind::InspectionEdge` без `edge_id` → ошибка загрузки;
   - `TaskKind::Waypoint` без `pose` → ошибка загрузки.

### Done criteria

- `MissionAdapter::is_completed` вызывается в runner для всех mission types.
- CBBA scoring идёт через adapter.
- `BatteryAwarePlanner` имеет unit test на feasibility logic.
- DSL validation ловит kind/fields mismatch.
- Старые сценарии без явного `kind` работают без изменений.

### Тесты

#### Без рефакторинга

- Unit test: adapter `is_completed` вызывается для SAR scan задачи в runner.
- Unit test: `BatteryAwarePlanner` — battery-constrained bundle усекается корректно.
- Validation test: `SarScan` без `grid_cell` → typed `ValidationError`.
- Validation test: `InspectionEdge` без `edge_id` → typed `ValidationError`.
- Regression: все `scenarios/*.json` загружаются без ошибок.

#### Лёгкий рефакторинг

- Shared task builders по `TaskKind`.
- In-memory `RunState` fixtures для adapter tests.
- Small mission lifecycle helpers.

#### Тяжёлый рефакторинг

- Full lifecycle tests: DSL → adapter → allocator → runner → metrics.
- Property tests: valid task kind → adapter не паникует.
- Compatibility suite для legacy scenarios без `kind`.

---

## ML-3 — Algorithmic Truth

**Цель:** разобрать видимые провалы с конкретным диагнозом, исправить что можно.

### Почему после ML-2

До wiring адаптеров диагностика SAR+CBBA требовала угадывания. После ML-2
ясно видно, какой контекст получает allocator, где теряется `grid_cell`, где
completion не срабатывает. Диагноз становится конкретным.

### Что сделать

1. Диагностировать SAR + CBBA/centralized:
   - пройти цепочку: task builder → adapter → allocator → completion check;
   - найти точку, где тип задачи игнорируется или теряется;
   - либо починить — если это мелкий mismatch;
   - либо зафиксировать точную причину с regression test — если требует
     архитектурного изменения.
2. Диагностировать inspection perimeter + CBBA:
   - allocation gap или battery/time constraint?
   - если battery — задокументировать с тестом;
   - если allocation gap — исправить scoring.
3. Для каждого gap class составить investigation note:
   - reproducible command или fixture;
   - expected behavior;
   - actual behavior;
   - likely cause;
   - confidence level;
   - recommended action.
4. Классифицировать каждый gap:
   - metric bug;
   - implementation bug;
   - algorithm mismatch;
   - scenario too hard;
   - accepted limitation.
5. Исправить только high-confidence bugs.
6. Обновить support matrix: `supported` / `experimental` / `unsupported with reason` /
   `failing due to known bug` / `not yet evaluated`.

### Done criteria

- Каждый major gap имеет классификацию и reproduction path.
- SAR + CBBA/centralized: либо работает объяснимо, либо зафиксирован точный
  root cause с тестом.
- Inspection перimeter + CBBA: причина задокументирована.
- Support matrix обновлена с конкретными причинами, не просто флагами.
- High-confidence metric bugs исправлены.
- Нет unsupported комбинации, которая называется stable.

### Тесты

#### Без рефакторинга

- Targeted metric consistency tests для SAR+CBBA.
- Support matrix assertions для known unsupported combinations.
- Unit tests для success/completion predicates.
- Regression test для каждого исправленного metric bug.

#### Лёгкий рефакторинг

- Scenario-specific metric assertion helpers.
- Small reproduction fixtures для gap classes.
- Support matrix fixture builder.

#### Тяжёлый рефакторинг

- Mission-specific simulation invariants.
- Property tests для success/completion/coverage consistency.

---

## ML-4 — Regression Hardening

**Цель:** превратить regression в реальный development gate.

### Почему после ML-1–ML-3

Пока метрики меняются из-за багов и wiring — калибровать thresholds бессмысленно:
они устареют при следующем исправлении. ML-4 делается на чистой базе.

### Текущее состояние

Regression harness есть: `RegressionSuite`, `ThresholdChecker`, `RegressionRunner`,
CLI `regression_runner`, baseline файл. Thresholds реальные (0.7–0.95), не нулевые.
Но:

- baseline привязан к старому commit;
- тесты используют `/tmp` напрямую;
- нет grouping suites по назначению;
- failure output не содержит reproduction command;
- wildfire не полностью в regression;
- suite `cbba_stress_pl_0_2` не моделирует packet loss как отдельный профиль.

### Что сделать

1. Обновить baseline на текущий commit после ML-1–ML-3.
2. Группировка suites:
   - `smoke` — структурные проверки, быстрые, всегда gate;
   - `quick` — behavioral проверки, стабильные, gate по умолчанию;
   - `experimental` — tracked, но не gate по умолчанию;
   - `validation` — долгие/ручные, не milestone.
3. Action-oriented failure output:
   - suite name;
   - strategy/profile/mission;
   - actual metric;
   - threshold;
   - delta;
   - reproduction command.
4. Baseline workflow:
   - update только из green state;
   - baseline stores metadata: git commit, date, seed range;
   - delta output readable.
5. Добавить wildfire suites как experimental.
6. Исправить `cbba_stress_pl_0_2`: моделировать packet loss 0.2 явно.
7. Заменить `/tmp/...` на tempdir в тестах.
8. CLI: `--list-suites`, `--suite smoke|quick|experimental`, machine-readable report.

### Done criteria

- Default regression не flakes при обычном использовании.
- Failure output содержит reproduction command.
- Suites сгруппированы по назначению.
- Experimental suites opt-in.
- Baseline workflow задокументирован и протестирован.
- Тесты не зависят от машинных путей.
- Wildfire и realism smoke суиты в experimental.

### Тесты

#### Без рефакторинга

- Threshold checker tests.
- Baseline delta tests.
- CLI exit-code tests.
- Suite grouping tests.
- Failure formatting tests с reproduction command.

#### Лёгкий рефакторинг

- Tempdir-based baseline update tests.
- Shared baseline fixtures.
- CLI fixture helpers.

#### Тяжёлый рефакторинг

- Automated flaky-suite detector.
- Baseline history store.
- End-to-end regression report golden tests.

---

## ML-5 — Realism Pack

**Цель:** превратить realism foundation в измеримый, воспроизводимый слой.

### Почему после ML-4

Realism calibration требует stable regression baseline. До ML-4 baseline меняется
при каждом исправлении. После ML-4 есть надёжная база для сравнения.

### Текущее состояние

Foundation готов: battery model v2, altitude sensor penalty, wind drift, pose noise,
comms jitter, time-gated no-fly zones, `--realism` preset, сценарные файлы
для каждой миссии. Но нет сравнительного анализа ideal vs realism, нет
определения expected effects, docs противоречивы.

### Что сделать

1. Определить expected realism effects для каждого профиля (light/medium/heavy):
   - effect on success/completion;
   - effect on route length;
   - effect on battery reserve;
   - effect on communication availability;
   - effect on detection time.
2. Сравнительный benchmark: ideal vs light vs medium vs heavy для каждой mission family.
3. Для каждой mission family описать expected degradation:
   - какие метрики должны двигаться;
   - какие должны оставаться стабильными;
   - какие слишком шумные для default gate.
4. Добавить realism metadata в manifest: active profile, параметры.
5. Обновить docs:
   - что моделируется;
   - что не моделируется;
   - какие assumptions.
6. Исправить README Known Limitations: убрать "2D world / no real-world noise"
   там, где это уже не так.
7. Stable realism smoke в regression; нестабильные — только experimental.

### Done criteria

- Expected realism effects задокументированы по профилям.
- Comparative benchmark воспроизводим из manifest.
- README не противоречит сам себе.
- Stable realism smoke в regression проходит.
- Нестабильные realism checks помечены experimental.

### Тесты

#### Без рефакторинга

- Scenario JSON validation для realism files.
- Manifest metadata assertions для realism fields.
- Realism preset smoke test.
- Profile selection tests.

#### Лёгкий рефакторинг

- Ideal-vs-realism comparison helper.
- Deterministic fixture для realism profile selection.
- Manifest assertion helpers.

#### Тяжёлый рефакторинг

- Stochastic realism regression.
- Full comparative analysis с multiple seeds.

---

## ML-6 — Direction Decision

**Цель:** выбрать одну из веток как главный следующий фокус.

После ML-1–ML-5 проект должен иметь:
- достоверные benchmark artifacts;
- wired architecture с реально работающими адаптерами;
- классифицированные algorithmic gaps;
- надёжный regression gate;
- измеримый realism layer.

Только на этой базе имеет смысл выбирать стратегическое направление.

### Что оценить

| Вопрос | Влияет на |
|---|---|
| Насколько чисты алгоритмические результаты? | Ветка 3 — Research Benchmark |
| Насколько болезненен анализ replay вручную? | Ветка 5 — Visualization |
| Стабильны ли semantics для SITL upload? | Ветка 6 — Real SITL / PX4 |
| Есть ли внешние пользователи? | Ветка 7 — Platform / API |
| Нужна ли более глубокая динамика миссий? | Ветка 2 — Disaster Mapping v2 |

### Возможные решения

| Если главное | Следующая ветка |
|---|---|
| Доказательные результаты | Ветка 3 — Research Benchmark |
| Анализ поведения агентов | Ветка 5 — Replay / Visualization |
| Реальные роботы / PX4 | Ветка 6 — Real SITL / PX4 |
| Внешние пользователи | Ветка 7 — Platform / API |
| Динамические миссии | Ветка 2 — Disaster Mapping v2 |

### Done criteria

- Выбрана одна основная ветка.
- Остальные явно deferred с указанием условий возврата.
- README/project description соответствует выбранному направлению.
- Следующий milestone имеет implementation-level scope.

---

## Итоговый порядок

```
ML-1  Benchmark Trust        — исправить баги в report/determinism
ML-2  Architecture Wiring    — подключить MissionAdapter, BatteryAwarePlanner
ML-3  Algorithmic Truth      — диагностика и исправление gaps
ML-4  Regression Hardening   — надёжный development gate
ML-5  Realism Pack           — измеримый realism layer
ML-6  Direction Decision     — выбор следующей большой ветки
```

Первый практический шаг:

> ML-1 — исправить per-row mission identity в `--mission all`.

Это конкретный, локализованный дефект в `report_export.rs` и `benchmark.rs`,
который делает benchmark artifacts ненадёжными. Всё остальное строится поверх этого.
