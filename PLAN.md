# Context

Три последовательных ревью зафиксировали оставшийся архитектурный долг после
основного рефакторинга. Цель этого плана — полностью устранить этот долг, не
меняя поведение алгоритмов, CLI, форматы артефактов и benchmark semantics.

Приоритет по ревью:

1. Завершить перенос логики из `runner/scenario_runner_internal.rs` в
   `runner/internal/` — это наиболее значимый незакрытый долг.
2. Разбить `sitl_supervisor_cli.rs` по образцу `strategy_comparison_runtime/`.
3. Устранить оставшиеся 4 `_and_` файла.
4. Убрать `use super::*` из production-кода (не из тестовых модулей).

# Investigation context

INVESTIGATION.md отсутствует. Контекст получен из трёх ревью (inbox) и
прямой инспекции кода.

Текущее состояние ключевых файлов:

- `crates/swarm-sim/src/runner/scenario_runner_internal.rs` — 898 строк.
  Один `impl ScenarioRunner` с `pub(super) fn run_internal`. Функция
  содержит инициализацию tick-loop state, главный tick loop с
  mission-specific ветвлениями и финальную сборку метрик. `runner/internal/`
  уже имеет 13 модулей, но они содержат вспомогательные функции, а не
  структуру состояния и сборку метрик.

- `crates/swarm-examples/src/sitl_supervisor_cli.rs` — 853 строки.
  Плоский файл с: `CliArgs`, `OutputPaths`, `SupervisorMode`,
  `parse_args()`, `validate_cli_arg_combinations()`, `resolve_output_paths()`,
  логикой exit codes, `usage()` и `run_cli()` entry point.
  `bin/sitl_supervisor.rs` уже тонкий (5 строк).

- `crates/swarm-replay/src/replay/state_and_render.rs` — 713 строк.
  Содержит: `ReplayState`, `ReplaySummary` (типы + impl для построения из
  event log), рендеринг/отображение replay.

- `crates/swarm-sim/src/regression/types_and_runner.rs` — 651 строк.
  Содержит: `Threshold`, `RegressionSuite`, `SuiteGroup`, `SuiteMode`
  (типы) и runner-логику.

- `crates/swarm-examples/src/sitl_agent_runtime/connection_and_reports.rs`
  — 599 строк. Содержит: `run_connection()` и connection workflow
  (`SitlGoldenPathDriver`, `MavlinkGoldenPathDriver`, `SitlGoldenPathRun`),
  а также report-структуры (`SitlExecutionSuccess`, `SitlExecutionFailure`,
  `SitlMissionStartReport`).

- `crates/swarm-examples/src/strategy_comparison_runtime/urban_artifacts_and_tests.rs`
  — 384 строки. Содержит urban artifacts и тесты в одном файле.

`use super::*` вне тестовых модулей: не обнаружено — все вхождения
находятся внутри `mod tests` или `#[cfg(test)]` блоков. Это приемлемо
согласно CLAUDE.md.

# Affected components

- `crates/swarm-sim/src/runner/scenario_runner_internal.rs`
- `crates/swarm-sim/src/runner/internal/` (все файлы)
- `crates/swarm-sim/src/runner/mod.rs`
- `crates/swarm-examples/src/sitl_supervisor_cli.rs`
- `crates/swarm-examples/src/lib.rs`
- `crates/swarm-replay/src/replay/state_and_render.rs`
- `crates/swarm-replay/src/replay/mod.rs`
- `crates/swarm-sim/src/regression/types_and_runner.rs`
- `crates/swarm-sim/src/regression/mod.rs`
- `crates/swarm-examples/src/sitl_agent_runtime/connection_and_reports.rs`
- `crates/swarm-examples/src/sitl_agent_runtime/mod.rs`
- `crates/swarm-examples/src/strategy_comparison_runtime/urban_artifacts_and_tests.rs`
- `crates/swarm-examples/src/strategy_comparison_runtime/mod.rs`

# Implementation steps

## 1. Завершить разбивку `runner/scenario_runner_internal.rs`

Цель: `scenario_runner_internal.rs` должен исчезнуть. Логика переносится
в `runner/internal/` и частично в `scenario_runner_public.rs` или новый
`runner/internal/loop_state.rs`.

### 1.1 Выделить tick-loop state struct

Создать `crates/swarm-sim/src/runner/internal/loop_state.rs`:

```
pub(in crate::runner) struct TickLoopState {
    // все мутабельные поля tick loop:
    // node list, crashed_agents, task assignments,
    // metrics accumulators, clock, etc.
}
```

Цель: `run_internal` перестаёт объявлять десятки `let mut` переменных
inline и работает с одним структурированным состоянием.

Зарегистрировать модуль в `runner/internal/mod.rs`:
```rust
mod loop_state;
pub(in crate::runner) use loop_state::*;
```

### 1.2 Выделить финальную сборку метрик

Создать `crates/swarm-sim/src/runner/internal/final_metrics.rs`:

Функция `assemble_final_metrics(state: &TickLoopState, config: &RunConfig)
-> RunMetrics` собирает итоговые метрики из накопленного состояния.

В неё переносится блок финальной сборки метрик из конца `run_internal`.
Это устраняет крупный `_and_` кластер вычислений внутри `run_internal`.

### 1.3 Перенести главный tick loop в `tick_loop.rs`

`crates/swarm-sim/src/runner/internal/tick_loop.rs` уже существует (55
строк вспомогательных функций). Расширить его:

- добавить `pub(in crate::runner) fn run_tick_loop<A: Allocator>(...) -> TickLoopState`
- функция принимает инициализированный `TickLoopState` и выполняет
  итерации до stop condition.

`run_internal` в итоге сводится к:
1. Early return для urban routes (уже есть).
2. Инициализация `TickLoopState`.
3. Вызов `run_tick_loop`.
4. Вызов `assemble_final_metrics`.
5. Return.

Если после этого `scenario_runner_internal.rs` стал тривиальной
оберткой (< 50 строк), инлайнить содержимое в `scenario_runner_public.rs`
и удалить файл. Если нет — оставить с трекером TODO.

### 1.4 Обновить `runner/mod.rs`

Убедиться, что все re-exports корректны после переноса. Запустить:
```
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim runner
```

---

## 2. Разбить `sitl_supervisor_cli.rs` → директория `sitl_supervisor_cli/`

По образцу `strategy_comparison_runtime/` (cli.rs + runs.rs + strategies.rs).

Целевая раскладка:

```
crates/swarm-examples/src/sitl_supervisor_cli/
    mod.rs          — pub fn run_cli() -> ExitCode; re-exports
    cli.rs          — CliArgs, SupervisorMode, parse_args(),
                      validate_cli_arg_combinations(), parse_u64_arg(),
                      parse_duration_arg(), set_mode(), usage(),
                      CliValidationArgs, LiveOptionFlags
    output.rs       — OutputPaths, resolve_output_paths(),
                      ensure_output_paths_available(),
                      ensure_output_path_available(), write_checked_file(),
                      write_replay_summary_if_requested(),
                      write_or_print_manifest(), manifest_write_error(),
                      replay_summary_write_error()
    run.rs          — run(), generated_run_id(), sanitize_run_id_component()
    exit_codes.rs   — supervisor_exit_code(),
                      classify_connection_failure_exit_code(),
                      report_failure_message(), report_failure_exit_code(),
                      prints_usage()
```

Шаги:

1. Создать директорию `crates/swarm-examples/src/sitl_supervisor_cli/`.
2. Перенести содержимое по модулям согласно раскладке выше.
3. `mod.rs` содержит только `pub fn run_cli()` и необходимые re-exports.
4. В `crates/swarm-examples/src/lib.rs` заменить
   `pub mod sitl_supervisor_cli;` на подключение директории (автоматически,
   т.к. Rust подберёт `sitl_supervisor_cli/mod.rs`).
5. `bin/sitl_supervisor.rs` не меняется (уже тонкий).
6. Запустить:
   ```
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-examples --all-targets
   ```

---

## 3. Разбить `replay/state_and_render.rs`

Целевая раскладка внутри `crates/swarm-replay/src/replay/`:

```
state.rs   — ReplayState + impl (построение из event log)
summary.rs — ReplaySummary + impl (построение из event log)
render.rs  — функции отображения / форматирования replay
```

Шаги:

1. Создать `state.rs` с `ReplayState` и её impl.
2. Создать `summary.rs` с `ReplaySummary` и её impl.
3. Создать `render.rs` с rendering-функциями.
4. Удалить `state_and_render.rs`.
5. Обновить `replay/mod.rs`: убрать `mod state_and_render`, добавить
   `mod state; mod summary; mod render;` и нужные pub use.
6. Запустить:
   ```
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-replay
   ```

---

## 4. Разбить `regression/types_and_runner.rs`

Целевая раскладка внутри `crates/swarm-sim/src/regression/`:

```
types.rs  — Threshold, RegressionSuite, SuiteGroup, SuiteMode,
             и все связанные типы/serde impl
runner.rs — runner-логика (функции запуска regression suites)
```

Шаги:

1. Создать `types.rs` с типами.
2. Создать `runner.rs` с runner-логикой.
3. Удалить `types_and_runner.rs`.
4. Обновить `regression/mod.rs`: убрать `mod types_and_runner`, добавить
   `mod types; mod runner;` и нужные pub use.
5. Запустить:
   ```
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim regression
   ```

---

## 5. Разбить `sitl_agent_runtime/connection_and_reports.rs`

Целевая раскладка внутри `crates/swarm-examples/src/sitl_agent_runtime/`:

```
connection.rs — run_connection(), SitlGoldenPathDriver trait,
                MavlinkGoldenPathDriver, SitlMavlinkObserver,
                SitlGoldenPathRun
reports.rs    — SitlExecutionSuccess, SitlExecutionFailure,
                SitlMissionStartReport
```

Шаги:

1. Создать `connection.rs` с connection workflow.
2. Создать `reports.rs` с report-структурами.
3. Удалить `connection_and_reports.rs`.
4. Обновить `sitl_agent_runtime/mod.rs`: убрать `mod connection_and_reports`,
   добавить `mod connection; mod reports;` и нужные pub use.
5. Запустить:
   ```
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-examples --all-targets
   ```

---

## 6. Разбить `strategy_comparison_runtime/urban_artifacts_and_tests.rs`

Целевая раскладка внутри
`crates/swarm-examples/src/strategy_comparison_runtime/`:

```
urban_artifacts.rs — urban artifact-логика (production code)
tests.rs           — тесты (перенести в отдельный файл или inline в
                     relevant production modules)
```

Шаги:

1. Создать `urban_artifacts.rs` с production-кодом.
2. Тесты либо в `urban_artifacts.rs` как inline `#[cfg(test)] mod tests`,
   либо в отдельный файл если их много.
3. Удалить `urban_artifacts_and_tests.rs`.
4. Обновить `strategy_comparison_runtime/mod.rs`.
5. Запустить:
   ```
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-examples --all-targets
   ```

---

## 7. Финальная верификация

После всех шагов:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --all-targets
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test --workspace
```

Убедиться:
- `scenario_runner_internal.rs` удалён или содержит < 50 строк.
- Нет файлов с суффиксом `_and_` в production-коде.
- Нет `#[path]`, нет `#![allow(unused_imports)]`.
- `use super::*` допускается только в `#[cfg(test)]` / `mod tests` блоках.

# Testing strategy

## 1. Тесты без рефакторинга (запустить вместе с каждым шагом)

Каждый шаг — behavior-preserving. После каждого шага запускать тесты
затронутого crate:

- После шага 1:
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim runner`
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim`

- После шага 2:
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-examples --all-targets`

- После шага 3:
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-replay`

- После шага 4:
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim regression`

- После шагов 5 и 6:
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-examples --all-targets`

Дополнительные проверки после каждого шага:
- `cargo clippy --all-targets -- -D warnings` — без новых предупреждений.
- `cargo fmt --all -- --check` — код отформатирован.
- Smoke check: `cargo build --all-targets` компилируется.

## 2. Тесты с лёгким рефакторингом

Если перенос runner-логики обнаруживает скрытые зависимости:

- Добавить unit-тест для `TickLoopState::new(...)` — корректная инициализация.
- Добавить unit-тест для `assemble_final_metrics(...)` с known state.
- Добавить import smoke-тесты: проверить что старые публичные пути
  (`crate::runner::...`) остаются доступными после переноса модулей.

## 3. Тесты с тяжёлым рефакторингом

Не требуются для этого рефакторинга. Все изменения — structural only,
без изменения алгоритмов или публичных API.

Gap: CLI-интеграционные тесты для `sitl_supervisor` (CLI флаги, exit codes)
сложны в автоматизации без PX4. Существующие тесты в
`crates/swarm-examples/tests/sitl_agent/` покрывают CLI parsing.
Дополнительных тестов не требуется — код не меняется, только перекладывается.

# Risks and tradeoffs

- **Видимость (visibility)**: перемещение кода между модулями может
  случайно изменить `pub(super)` / `pub(crate)` пути. Каждый шаг
  требует проверки через `cargo build` сразу после переноса.

- **Re-exports**: если внешние модули используют конкретные пути вместо
  top-level re-exports, перемещение сломает компиляцию. Это выявляется
  немедленно при `cargo build`.

- **Объём изменений**: 6 шагов — это значительный diff. Рекомендуется
  делать каждый шаг отдельным коммитом, чтобы упростить review и
  потенциальный rollback.

- **runner/internal/loop_state.rs**: структурирование tick-loop state
  в struct — наиболее рискованный шаг. Тип может требовать lifetime
  аннотаций или Rc/RefCell, если текущие переменные заимствуются
  перекрёстно. Если это усложняет код, допустимо ограничиться шагами
  1.2–1.3 без 1.1.

- **Тестовые файлы**: `urban_artifacts_and_tests.rs` содержит тесты —
  нужно убедиться что тесты не потеряются при разбивке.

- **`state_and_render.rs`**: публичные типы (`ReplayState`, `ReplaySummary`)
  могут использоваться в других crate-ах. Необходимо сохранить re-exports
  в `replay/mod.rs`.

# Open questions

- Допустимо ли оставить `scenario_runner_internal.rs` с < 50 строк
  (тривиальная обёртка) вместо его удаления? Либо инлайнить в
  `scenario_runner_public.rs`?

- Нужно ли выносить runner loop в `runner/internal/tick_loop.rs` как
  отдельную pub(in crate::runner) функцию, или достаточно только
  выделить `TickLoopState` и `assemble_final_metrics`?

- Стоит ли одновременно с шагом 6 разбить
  `strategy_comparison_runtime/runs.rs` (587 строк)?
  Ревью 2 называет его крупным, но он не имеет `_and_` в имени.

- Когда начинать `crates/swarm-sitl` (фаза 2)?
  Ревью согласны, что это следующий крупный архитектурный шаг, но не
  срочный. Зависит от M73+ работы по SITL harness.
