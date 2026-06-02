# Context

Цель этого плана — скомпоновать три архитектурных ревью в один
исполняемый план рефакторинга. Проект уже прошёл крупную фазу очистки:
`include!`, `#[path]` и production-файлы с суффиксом `_and_` больше не
являются основной проблемой. Оставшийся долг сосредоточен в крупных
orchestration-модулях, где смешаны инициализация, execution loop, сбор
метрик, replay/reporting и CLI glue.

План не предполагает изменения алгоритмического поведения, форматов
артефактов, benchmark semantics или публичных CLI-контрактов без отдельного
решения. Основная стратегия: делать маленькие семантические split-ы с
автотестами после каждого шага, а не большой переписывающий рефакторинг.

Немедленный blocker из ревью: `swarm-examples` имеет build-break при
`--all-features` из-за некорректного пути к `SitlRunFinalStatus`. Его нужно
исправить до крупных архитектурных шагов.

# Investigation context

`INVESTIGATION.md` отсутствует. Входные данные — три ревью из inbox и
локальная инспекция текущего кода.

Проверенные факты из ревью:

- `crates/swarm-examples/src/sitl_agent_runtime/connection.rs` использует
  `super::reports::SitlRunFinalStatus` в нескольких местах, тогда как enum
  публично определён/доступен через `crate::sitl_report::SitlRunFinalStatus`.
  Это ломает сборку с `--all-features`.
- `crates/swarm-sim/src/runner/scenario_runner_internal.rs` всё ещё содержит
  большой `run_internal` (~602 строки). Final metrics уже вынесены, но
  `TickLoopState<A: Allocator>` и перенос основного tick-loop ещё не сделаны.
- `crates/swarm-sim/src/runner/scenario_runner_urban.rs` содержит две большие
  ветки выполнения: `run_urban_patrol` и `run_urban_search`.
- `crates/swarm-sim/src/runner/urban_helpers.rs` остаётся крупным helper-модулем
  с функциями для разных urban-подсценариев.
- `crates/swarm-sim/src/benchmark.rs` смешивает модель отчёта,
  Markdown-rendering, benchmark harness и aggregation.
- `crates/swarm-sim/src/report_export/export_formats.rs` смешивает JSON, CSV,
  Markdown и focused report rendering.
- `crates/swarm-sim/src/dsl/types.rs` смешивает DSL-типы, загрузку и
  валидацию.
- `crates/swarm-examples/src/strategy_comparison_runtime/runs.rs` содержит
  большой CLI execution flow и mission selection через `match`.
- `crates/swarm-examples` фактически является integration/product-like слоем
  для SITL, supervisor, observability и CLI, а не просто набором examples.

# Affected components

- `crates/swarm-examples/src/sitl_agent_runtime/connection.rs`
- `crates/swarm-examples/src/sitl_agent_runtime/reports.rs`
- `crates/swarm-examples/src/sitl_report.rs`
- `crates/swarm-sim/src/runner/scenario_runner_internal.rs`
- `crates/swarm-sim/src/runner/internal/mod.rs`
- `crates/swarm-sim/src/runner/internal/tick_loop.rs`
- `crates/swarm-sim/src/runner/internal/final_metrics.rs`
- `crates/swarm-sim/src/runner/internal/loop_state.rs` (новый файл)
- `crates/swarm-sim/src/runner/scenario_runner_urban.rs`
- `crates/swarm-sim/src/runner/urban_helpers.rs`
- `crates/swarm-sim/src/runner/urban_patrol.rs` (новый файл)
- `crates/swarm-sim/src/runner/urban_search.rs` (новый файл)
- `crates/swarm-sim/src/runner/urban_events.rs` (новый файл)
- `crates/swarm-sim/src/runner/urban_metrics.rs` (новый файл)
- `crates/swarm-sim/src/benchmark.rs`
- `crates/swarm-sim/src/benchmark/` (новая директория, если выбран module split)
- `crates/swarm-sim/src/report_export/export_formats.rs`
- `crates/swarm-sim/src/report_export/mod.rs`
- `crates/swarm-sim/src/report_export/json.rs` (новый файл)
- `crates/swarm-sim/src/report_export/csv.rs` (новый файл)
- `crates/swarm-sim/src/report_export/markdown.rs` (новый файл)
- `crates/swarm-sim/src/report_export/focused.rs` (новый файл)
- `crates/swarm-sim/src/dsl/types.rs`
- `crates/swarm-sim/src/dsl/load.rs` (новый файл)
- `crates/swarm-sim/src/dsl/validate.rs` (новый файл)
- `crates/swarm-sim/src/dsl/urban_validate.rs` (новый файл)
- `crates/swarm-sim/src/dsl/mod.rs`
- `crates/swarm-examples/src/strategy_comparison_runtime/runs.rs`
- `crates/swarm-examples/src/strategy_comparison_runtime/mod.rs`
- `crates/swarm-examples/src/strategy_comparison_runtime/missions.rs` (новый файл)
- `crates/swarm-examples/src/sitl_safety.rs` (низкий приоритет)
- `crates/swarm-examples/src/sitl_plan.rs` (низкий приоритет)
- `crates/swarm-examples/src/sitl_multi_agent.rs` (низкий приоритет)

# Implementation steps

## 1. Исправить build-break в all-features

Приоритет: обязательный первый шаг, потому что архитектурный рефакторинг
нельзя считать безопасным, если базовая feature-matrix уже сломана.

Что сделать:

1. В `crates/swarm-examples/src/sitl_agent_runtime/connection.rs` заменить
   все обращения `super::reports::SitlRunFinalStatus` на
   `crate::sitl_report::SitlRunFinalStatus`.
2. Проверить, нужен ли в файле `use std::path::Path`; если импорт
   неиспользуемый — удалить.
3. Не переэкспортировать enum из `reports.rs` ради обхода ошибки: источник
   типа должен оставаться явным, через `sitl_report`.

Ожидаемый результат:

- `cargo build --all-features -p swarm-examples` проходит.
- `cargo clippy --all-targets --all-features -- -D warnings` не падает на
  этом месте.

Автотесты:

- существующие tests для `sitl_agent_runtime`;
- feature build для `swarm-examples`;
- workspace clippy с `--all-features`, если длительность приемлема.

## 2. Завершить разбивку `scenario_runner_internal.rs`

Приоритет: высокий. Это главный архитектурный долг, по которому сошлись все
ревью. Цель — чтобы `run_internal` перестал владеть всем состоянием
симуляции и стал тонким coordinator-ом.

### 2.1. Ввести `TickLoopState<A: Allocator>`

Создать `crates/swarm-sim/src/runner/internal/loop_state.rs`.

`TickLoopState` должен владеть всем mutable-состоянием основного loop:

- `nodes`;
- `bus`;
- `allocator`;
- `clock`;
- `log_builder`;
- failure/detection/reallocation state;
- dynamic task counters;
- partition/connectivity counters;
- CBBA convergence state;
- SAR/inspection/wildfire state;
- safety counters;
- movement/battery counters;
- base station connectivity state.

Рекомендуемая форма:

```rust
pub(in crate::runner) struct TickLoopState<A: Allocator> {
    pub nodes: Vec<(AgentNode<InMemAgentTransport>, AgentId)>,
    pub bus: Rc<RefCell<InMemNetwork>>,
    pub allocator: SafetyAllocator<A>,
    pub clock: Clock,
    pub log_builder: Option<swarm_replay::EventLogBuilder>,
    // остальные поля из текущего run_internal
}
```

Добавить constructor:

```rust
impl<A: Allocator> TickLoopState<A> {
    pub(in crate::runner) fn new(
        scenario: &Scenario,
        config: &RunConfig,
        allocator: A,
        log_builder: Option<swarm_replay::EventLogBuilder>,
    ) -> Self
}
```

Граница ответственности `new`:

- создаёт `InMemNetwork`;
- создаёт `AgentNode` для каждого агента;
- применяет movement/CBBA/node config;
- инициализирует counters и mission states;
- не запускает ticks;
- не собирает final metrics.

### 2.2. Перенести основной tick loop в `internal/tick_loop.rs`

`tick_loop.rs` уже существует как helper-файл. Его нужно расширить:

```rust
pub(in crate::runner) fn run_tick_loop<A: Allocator>(
    state: &mut TickLoopState<A>,
    scenario: &Scenario,
    config: &RunConfig,
) {
    // тело текущего for _ in 0..config.max_ticks
}
```

Внутри loop не должно оставаться локальных counters, которые потом нужны
final metrics; такие значения должны жить в `TickLoopState`.

Критерии качества:

- тело loop переносится без изменения порядка операций;
- replay events остаются в том же порядке;
- условия остановки не меняются;
- значения metrics после smoke/quick прогонов совпадают с baseline до
  рефакторинга.

### 2.3. Сократить `run_internal`

После 2.1–2.2 `run_internal` должен:

1. выбрать urban-search/urban-patrol early path;
2. вычислить static urban foundation metrics, если нужно;
3. создать `TickLoopState`;
4. вызвать `run_tick_loop`;
5. извлечь final network counters из `state.bus`;
6. вычислить final success predicate;
7. передать owned state в `assemble_final_metrics`.

Целевой размер:

- хороший результат: `< 150` строк;
- идеальный результат: удалить `scenario_runner_internal.rs` или оставить
  thin wrapper `< 50` строк.

Не делать:

- не менять алгоритм allocator-ов;
- не менять success semantics;
- не менять layout replay event log;
- не менять public `ScenarioRunner::run*` API.

## 3. Разбить urban runner по миссиям и обязанностям

Приоритет: средний-высокий, особенно если следующая работа идёт в сторону
urban patrol/search/obstacle/bus scenarios.

Текущий `scenario_runner_urban.rs` объединяет:

- validation входного urban state;
- route planning;
- patrol loop;
- search loop;
- obstacle/judge checks;
- bus detection;
- replay events;
- финальную сборку metrics.

Целевая раскладка:

```text
crates/swarm-sim/src/runner/
  urban_patrol.rs   — run_urban_patrol + patrol loop
  urban_search.rs   — run_urban_search + search loop
  urban_events.rs   — push_* replay event helpers
  urban_metrics.rs  — urban_patrol_metrics / urban_search_metrics wrappers
  urban_helpers.rs  — только truly shared helpers
```

Порядок выполнения:

1. Перенести `run_urban_patrol` в `urban_patrol.rs`.
2. Перенести `run_urban_search` в `urban_search.rs`.
3. Перенести event helpers из `urban_helpers.rs` в `urban_events.rs`.
4. Перенести metric constructors в `urban_metrics.rs`.
5. Оставить в `urban_helpers.rs` только общие функции: speed, pose,
   route efficiency, shared analysis state.

Критерии качества:

- `scenario_runner_urban.rs` удалён или становится thin module wrapper;
- patrol и search можно менять независимо;
- urban tests проходят без изменения expected metrics;
- публичные exports `swarm_sim::...` не ломаются.

## 4. Разделить benchmark model, harness и rendering

Приоритет: средний. Это станет важным при добавлении новых миссий и новых
метрик.

Проблема: `benchmark.rs` одновременно содержит:

- `ComparisonReport`;
- `BenchmarkOptions`;
- `BenchmarkResult`;
- `BenchmarkHarness`;
- `impl Display for ComparisonReport`;
- aggregation helpers.

Целевая раскладка:

```text
crates/swarm-sim/src/benchmark/
  mod.rs          — re-exports
  report.rs       — ComparisonReport, BenchmarkOptions, BenchmarkResult
  harness.rs      — BenchmarkHarness и run_with_seeds
  aggregation.rs  — aggregate/merge helper functions
  markdown.rs     — Display/render Markdown table
```

Минимальный вариант без directory split:

- оставить `benchmark.rs` как `mod.rs`;
- добавить рядом `benchmark_report.rs`, `benchmark_harness.rs`,
  `benchmark_markdown.rs`.

Предпочтительный вариант — directory module, потому что файл уже 900+ строк.

Критерии качества:

- `swarm_sim::{BenchmarkHarness, ComparisonReport, BenchmarkOptions}` остаются
  доступными как раньше;
- JSON/CSV export не меняет schema;
- benchmark determinism tests остаются зелёными;
- Markdown output snapshot/structural tests проходят.

## 5. Разбить `report_export/export_formats.rs`

Приоритет: средний. Этот split ниже benchmark, но проще и менее рискованный.

Проблема: один файл содержит четыре разные ответственности:

- `export_json`;
- `export_csv`;
- `export_markdown`;
- `generate_focused_report`.

Целевая раскладка:

```text
crates/swarm-sim/src/report_export/
  json.rs
  csv.rs
  markdown.rs
  focused.rs
  mod.rs
```

Порядок:

1. Перенести `export_json` в `json.rs`.
2. Перенести `export_csv` и CSV row helpers в `csv.rs`.
3. Перенести `export_markdown` в `markdown.rs`.
4. Перенести `generate_focused_report` в `focused.rs`.
5. В `mod.rs` сохранить прежние `pub use`, чтобы public API не изменился.

Критерии качества:

- existing report export tests проходят;
- generated JSON/CSV/Markdown byte-for-byte совпадает с текущим для
  representative fixtures, если тесты уже это проверяют;
- если byte-for-byte snapshot отсутствует, добавить structural tests.

## 6. Разбить DSL: types / load / validate / urban_validate

Приоритет: средний. Нужен перед активным расширением DSL.

Проблема: `dsl/types.rs` содержит одновременно:

- типы `ScenarioSuite`, `ScenarioSuiteEntry`, validation structs;
- `load_scenario_suite`;
- `export_entry`, `export_suite`;
- `validate_scenario_suite`;
- urban-specific validation.

Целевая раскладка:

```text
crates/swarm-sim/src/dsl/
  mod.rs
  types.rs
  load.rs
  validate.rs
  urban_validate.rs
  export.rs        — опционально, если export logic достаточно отдельная
```

Порядок:

1. Оставить data structs/enums в `types.rs`.
2. Перенести file loading в `load.rs`.
3. Перенести generic validation в `validate.rs`.
4. Перенести urban start pose / urban search validation в `urban_validate.rs`.
5. Сохранить прежние re-exports в `dsl/mod.rs` и `swarm-sim/src/lib.rs`.

Критерии качества:

- DSL fixtures из `scenarios/*.json` загружаются как раньше;
- validation errors не меняют смысл;
- negative tests на invalid DSL остаются зелёными.

## 7. Ввести mission registry для `strategy_comparison_runtime`

Приоритет: средний-низкий. Делать после benchmark/DSL split или перед
добавлением следующей mission.

Проблема: `runs.rs` содержит большой `match mission`, где каждая ветка
собирает profile names и `ScenarioBuilder`.

Целевая идея:

```rust
pub(super) struct MissionDescriptor {
    pub mission: Mission,
    pub name: &'static str,
    pub profiles: fn(&CliArgs) -> Vec<String>,
    pub builder: fn() -> ScenarioBuilder,
}
```

или trait-вариант:

```rust
pub(super) trait MissionRuntime {
    fn mission(&self) -> Mission;
    fn name(&self) -> &'static str;
    fn profile_names(&self, cli: &CliArgs) -> Vec<String>;
    fn scenario_builder(&self) -> ScenarioBuilder;
}
```

Целевая раскладка:

```text
crates/swarm-examples/src/strategy_comparison_runtime/
  missions.rs
  runs.rs
  cli.rs
  strategies.rs
  urban_artifacts.rs
```

После refactor `runs.rs` должен только:

1. parse CLI;
2. выбрать descriptors;
3. применить realism wrapper;
4. вызвать `BenchmarkHarness`;
5. записать artifacts.

Критерии качества:

- добавление новой mission требует добавить descriptor, а не редактировать
  большой execution flow;
- existing CLI behavior не меняется;
- tests `strategy_comparison` и benchmark-pack остаются зелёными.

## 8. Низкоприоритетный split SITL flat modules

Приоритет: низкий. Делать только когда есть функциональная причина или когда
файлы начнут мешать изменениям.

Кандидаты:

- `crates/swarm-examples/src/sitl_safety.rs`
- `crates/swarm-examples/src/sitl_plan.rs`
- `crates/swarm-examples/src/sitl_multi_agent.rs`

Возможная раскладка:

```text
sitl_safety/
  mod.rs
  types.rs
  load.rs
  validate.rs
  gates.rs

sitl_plan/
  mod.rs
  error.rs
  connection.rs
  build.rs
  load.rs
  waypoints.rs

sitl_multi_agent/
  mod.rs
  config.rs
  manifest.rs
  ownership.rs
  validation.rs
```

Не делать сейчас:

- не переименовывать crate `swarm-examples`;
- не переносить весь SITL слой в новый crate без отдельного плана;
- не менять CLI paths и binary names.

## 9. Отдельно оценить судьбу `swarm-examples`

Приоритет: стратегический, не обязательный для текущего refactor pass.

Текущее состояние: `swarm-examples` содержит не только примеры, но и:

- SITL agent runtime;
- SITL supervisor;
- multi-agent manifest/config;
- safety gates;
- observability/event logs;
- strategy comparison CLI runtime.

Варианты:

1. Оставить как есть до появления внешних пользователей API.
2. Создать `swarm-sitl` и перенести туда SITL runtime/supervisor/safety.
3. Создать `swarm-cli` и оставить `swarm-examples` только для demo binaries.

Рекомендация: пока выбрать вариант 1. Возвращаться к варианту 2 только после
завершения runner/benchmark/DSL refactor или перед публикацией.

# Testing strategy

## 1. Tests that need no refactoring

Эти проверки должны запускаться вместе с соответствующими шагами.

Для шага 1:

```bash
cargo build --all-features -p swarm-examples
cargo clippy --all-targets --all-features -- -D warnings
```

Для runner split:

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim runner

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim
```

Для urban split:

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim urban

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim runner::tests
```

Для report export / benchmark split:

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim report_export

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim benchmark

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --test benchmark_pack
```

Для DSL split:

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim dsl
```

Для strategy comparison mission registry:

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --test wildfire

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --test regression

PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --test benchmark_pack
```

Финальная проверка после каждого крупного шага:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test --workspace
```

## 2. Tests that need light refactoring

Эти тесты стоит добавить/адаптировать по мере split-ов.

Runner:

- regression test, который сравнивает `RunMetrics` до/после `TickLoopState`
  на small coverage scenario;
- test на сохранение replay event order для failure/reallocation сценария;
- test на early stop conditions: all tasks assigned, failures detected,
  dynamic tasks injected, partitions resolved.

Urban:

- patrol happy-path: route completed, no violations;
- patrol negative-path: invalid start node / invalid start pose;
- search happy-path: bus detected без violation;
- search negative-path: static route violation;
- edge-case: empty route loop / no alive agent.

Benchmark/export:

- structural snapshot для Markdown header/columns;
- JSON/CSV schema roundtrip для representative `ComparisonReport`;
- focused report keeps expected metric identities.

DSL:

- invalid JSON load error;
- duplicate scenario names;
- invalid urban start pose;
- missing urban state for urban search;
- edge-case empty suite.

Strategy comparison registry:

- every `Mission` has descriptor;
- descriptor names match CLI mission names;
- all descriptors produce non-empty profile list;
- unknown profile fallback behavior remains unchanged.

## 3. Tests that need heavy refactoring

Эти тесты полезны, но их не стоит блокирующе требовать от первого pass-а.

- Golden benchmark comparison до/после refactor на большом matrix, если
  нужен byte-for-byte confidence для всех missions.
- Structured event-log snapshot suite для replay order across all mission
  families.
- Property tests для DSL validation model после выделения typed validation
  errors.
- Cross-crate API compatibility tests, если `swarm-examples` будет делиться на
  `swarm-sitl` / `swarm-cli`.
- Dedicated benchmark rendering snapshot framework, если Markdown/CSV output
  станет публичным стабильным контрактом.

# Что могло сломаться

Потенциальные регрессии после выполнения плана:

- Поведение симуляции: порядок tick phases, failure detection, task injection,
  reallocation timing, CBBA convergence tick.
  Проверка: `cargo test -p swarm-sim runner`, targeted regression metrics,
  replay event order tests.

- Метрики: `RunMetrics` поля могут получить прежние значения из неправильного
  state-поля после переноса в `TickLoopState`.
  Проверка: сравнение representative `RunMetrics` до/после refactor,
  benchmark smoke/quick tests.

- Replay/event logs: event order или seq/task mapping может измениться при
  переносе helpers.
  Проверка: replay summary tests, M58/M59 artifact parsing tests, targeted
  event category assertions.

- Public API: re-exports из `swarm_sim`, `swarm_examples` и module paths могут
  измениться.
  Проверка: `cargo check --workspace`, integration tests, examples build.

- CLI contracts: `strategy_comparison`, `regression_runner`, `sitl_agent`,
  `sitl_supervisor` могут поменять поведение при split-е runtime code.
  Проверка: существующие CLI integration tests и smoke commands через
  `/home/formi/.local/bin/runlim cargo test`.

- Форматы данных: JSON/CSV/Markdown export может изменить порядок колонок или
  имена полей.
  Проверка: report_export tests, benchmark_pack tests, schema/snapshot tests.

- Feature gates: `mavlink-transport` / `--all-features` может сломаться при
  перемещении SITL imports.
  Проверка: `cargo build --all-features -p swarm-examples` и clippy
  `--all-features`.

- Производительность: перенос state сам по себе не должен менять сложность, но
  accidental clones в `TickLoopState` могут увеличить память/время.
  Проверка: quick benchmark до/после, clippy warnings, code review на clones.

# Risks and tradeoffs

- `TickLoopState<A>` — самый рискованный шаг. Риск не в концепции, а в
  borrow checker и generic ownership вокруг `SafetyAllocator<A>`,
  `Rc<RefCell<InMemNetwork>>`, `AgentNode<InMemAgentTransport>` и final metrics
  ownership transfer. Если перенос начинает требовать сложных lifetimes,
  лучше остановиться на state struct + helper methods, а не вводить unsafe или
  чрезмерные abstractions.

- Не все крупные файлы являются проблемой. Большие test-файлы и семантически
  связные algorithm modules (`allocator.rs`, `cbba.rs`, `route_planner.rs`) не
  нужно дробить только по числу строк.

- `benchmark.rs` и `report_export/export_formats.rs` пересекаются по теме
  rendering. Нужно не создать две несовместимые модели output. Сначала
  сохранить существующие exports, потом уже улучшать model.

- `swarm-examples` как product-like crate выглядит неправильно по названию, но
  переименование/вынос может дать много churn без немедленной пользы. Это
  лучше оставить стратегическим follow-up.

- Для каждого шага важнее сохранить поведение, чем добиться идеального размера
  файла. Если split ухудшает читаемость или требует искусственных параметров,
  шаг нужно пересмотреть.

# Open questions

- Нужно ли добиваться строгого критерия `< 50` строк для
  `scenario_runner_internal.rs`, или достаточно сделать `run_internal`
  тонким coordinator-ом `< 150` строк?

- Делать ли `benchmark.rs` directory module (`benchmark/mod.rs`) сразу, или
  сначала менее инвазивный split в соседние `benchmark_*` файлы?

- Нужно ли вводить typed DSL validation errors вместо текущей модели в рамках
  DSL split, или это отдельный functional change?

- Оставлять ли `ComparisonReport::Display` как публичный Markdown renderer,
  или постепенно переводить callers на explicit `export_markdown()`?

- Когда возвращаться к переименованию/выносу `swarm-examples` в `swarm-sitl`:
  перед публикацией, перед hardware work, или только при появлении внешних
  пользователей API?

- Нужна ли отдельная regression baseline фиксация после завершения runner split,
  чтобы доказать, что refactor не изменил benchmark semantics?
