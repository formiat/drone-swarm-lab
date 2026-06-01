# План глобального разбиения крупных Rust-файлов

## Context

Задача: подготовить план глобального рефакторинга `.rs` файлов так, чтобы в проекте не осталось Rust-файлов больше 2000 строк, а рекомендуемая цель была не больше 1000 строк на файл.

Перед планированием прочитаны обязательные протоколы:

- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`.

Notion-задача и GitLab/MR в запросе не указаны, поэтому внешние чтения по Notion/GitLab не выполнялись.

Текущая инвентаризация `*.rs` показывает 86 Rust-файлов. Обязательный порог `> 2000` сейчас превышают:

- `crates/swarm-sim/src/runner.rs` - 3922 строки;
- `crates/swarm-examples/src/sitl_supervisor.rs` - 3119 строк;
- `crates/swarm-comms/src/mavlink.rs` - 2325 строк;
- `crates/swarm-examples/src/bin/sitl_agent.rs` - 2300 строк.

Рекомендуемый порог `> 1000` также превышают:

- `crates/swarm-examples/tests/sitl_agent.rs` - 1977 строк;
- `crates/swarm-sim/src/regression.rs` - 1549 строк;
- `crates/swarm-sim/src/report_export.rs` - 1452 строки;
- `crates/swarm-runtime/src/node.rs` - 1274 строки;
- `crates/swarm-examples/src/bin/strategy_comparison.rs` - 1257 строк;
- `crates/swarm-metrics/src/metrics.rs` - 1180 строк;
- `crates/swarm-sim/src/dsl.rs` - 1125 строк;
- `crates/swarm-examples/src/sitl_observability.rs` - 1124 строки;
- `crates/swarm-replay/src/replay.rs` - 1078 строк;
- `crates/swarm-sim/src/urban.rs` - 1071 строк.

Цель рефакторинга не в изменении поведения, а в снижении размера модулей, сохранении публичных API и добавлении автоматической проверки размера файлов.

## Investigation Context

`INVESTIGATION.md` в корне репозитория отсутствует, поэтому отдельный investigation-контекст не применялся.

По коду дополнительно проверены основные экспортные точки:

- `crates/swarm-sim/src/lib.rs`;
- `crates/swarm-examples/src/lib.rs`;
- `crates/swarm-comms/src/lib.rs`;
- `crates/swarm-runtime/src/lib.rs`.

Ключевое ограничение: при переносе кода из одиночных файлов в директории `mod.rs` нужно сохранить существующие пути модулей и публичные `pub use`, например `swarm_sim::runner::*`, `swarm_comms::mavlink::*`, `swarm_examples::sitl_supervisor::*`.

## Affected Components

- `crates/swarm-sim/src/runner.rs`: основной сценарный runner, mission success semantics, wildfire/urban state, allocation/safety helpers, scenario tests.
- `crates/swarm-examples/src/sitl_supervisor.rs`: live/mock supervisor, controllers, reallocation, validation, reports, event logs, tests.
- `crates/swarm-comms/src/mavlink.rs`: MAVLink types, transport abstraction, mission upload, telemetry, lifecycle commands, coordinate conversion, tests.
- `crates/swarm-examples/src/bin/sitl_agent.rs`: CLI binary, hardware boundary, mock/connection execution, golden path, telemetry, reports, tests.
- `crates/swarm-examples/tests/sitl_agent.rs`: large integration test suite for `sitl_agent`.
- `crates/swarm-sim/src/regression.rs`: regression config, baselines, deltas, suites, report generation, tests.
- `crates/swarm-sim/src/report_export.rs`: JSON/CSV/Markdown export, manifest/report comparison, tests.
- `crates/swarm-runtime/src/node.rs`: `AgentNode`, allocation outcomes, node tick/runtime tests.
- `crates/swarm-examples/src/bin/strategy_comparison.rs`: benchmark CLI, mission factories, regression mode, artifact writing.
- `crates/swarm-metrics/src/metrics.rs`: run/aggregate metrics and formatting.
- `crates/swarm-sim/src/dsl.rs`: scenario DSL parsing and validation, including urban checks.
- `crates/swarm-examples/src/sitl_observability.rs`: event schema, recorder, summary, IO helpers.
- `crates/swarm-replay/src/replay.rs`: replay state, summary, timeline, snapshots, rendering.
- `crates/swarm-sim/src/urban.rs`: urban planner, judge, route risk, bus detection, geometry helpers.

## Implementation Steps

1. Add a size guard before moving code:
   - create a small repository-local check, for example `scripts/check-rs-file-lines.sh`;
   - count each file independently, not via aggregate `xargs wc -l`, so the `wc` `total` row cannot affect the result;
   - fail on any `.rs` file above 2000 lines;
   - report files above 1000 lines as warnings and support a configurable stricter mode that turns those warnings into failures;
   - document the command in `README.md` or `docs/STATUS.md` only if it becomes part of regular maintenance.

2. Split `crates/swarm-sim/src/runner.rs` into a `runner/` module directory:
   - move the existing module root to `crates/swarm-sim/src/runner/mod.rs`;
   - extract config and public types to `crates/swarm-sim/src/runner/config.rs`;
   - extract mission success predicates to `crates/swarm-sim/src/runner/success.rs`;
   - extract inspection/wildfire/urban state helpers to `inspection.rs`, `wildfire.rs`, `urban_runtime.rs`;
   - extract metrics/report helpers to `metrics.rs`;
   - move tests to `runner/tests.rs` or several focused test modules;
   - keep `crates/swarm-sim/src/lib.rs` public exports compatible.

3. Split `crates/swarm-examples/src/sitl_supervisor.rs` into `sitl_supervisor/`:
   - move root declarations and public reexports to `crates/swarm-examples/src/sitl_supervisor/mod.rs`;
   - extract controller traits and implementations to `controllers.rs`;
   - extract PX4/live orchestration to `live.rs`;
   - extract mock orchestration to `mock.rs`;
   - extract failure/reallocation logic to `reallocation.rs`;
   - extract validation and ownership checks to `validation.rs`;
   - extract report/event-log output to `reports.rs`;
   - move tests to `tests.rs` or focused submodules;
   - preserve the binary-facing API used by `crates/swarm-examples/src/bin/sitl_supervisor.rs`.

4. Split `crates/swarm-comms/src/mavlink.rs` into `mavlink/`:
   - move root declarations and compatibility reexports to `crates/swarm-comms/src/mavlink/mod.rs`;
   - extract data types/options/errors to `types.rs`;
   - extract real/mock transport to `transport.rs`;
   - extract mission upload protocol to `mission_upload.rs`;
   - extract telemetry parsing/polling to `telemetry.rs`;
   - extract arm/mode/start/lifecycle commands to `lifecycle.rs` or `commands.rs`;
   - extract coordinate conversion helpers to `conversion.rs`;
   - split tests by protocol area;
   - keep feature-gated code under the same feature flags as today.

5. Turn `crates/swarm-examples/src/bin/sitl_agent.rs` into a thin binary:
   - move reusable runtime code into `crates/swarm-examples/src/sitl_agent_runtime/`;
   - keep CLI parsing in `sitl_agent_runtime/cli.rs`;
   - move mock execution to `mock.rs`;
   - move connection/PX4 execution to `connection.rs`;
   - move golden-path handling to `golden_path.rs`;
   - move telemetry progress loop to `telemetry.rs`;
   - move report writing to `reports.rs`;
   - move hardware boundary checks to `hardware_boundary.rs`;
   - keep `src/bin/sitl_agent.rs` as a small entrypoint that calls the library module.

6. Split recommended-threshold files in a second pass:
   - `crates/swarm-examples/tests/sitl_agent.rs` into multiple integration tests plus `tests/support/`;
   - `crates/swarm-sim/src/regression.rs` into `regression/{mod.rs,types.rs,baseline.rs,runner.rs,suites.rs,report.rs}`;
   - `crates/swarm-sim/src/report_export.rs` into `report_export/{mod.rs,json.rs,csv.rs,markdown.rs,manifest.rs,compare.rs}`;
   - `crates/swarm-runtime/src/node.rs` into `node/{mod.rs,config.rs,tick.rs,allocation.rs}`;
   - `crates/swarm-examples/src/bin/strategy_comparison.rs` into a thin binary plus `src/strategy_comparison/`;
   - `crates/swarm-metrics/src/metrics.rs` into `metrics/{mod.rs,run.rs,aggregate.rs,display.rs}`;
   - `crates/swarm-sim/src/dsl.rs` into `dsl/{mod.rs,suite.rs,validation.rs,urban_validation.rs}`;
   - `crates/swarm-examples/src/sitl_observability.rs` into `sitl_observability/{mod.rs,events.rs,recorder.rs,summary.rs,io.rs}`;
   - `crates/swarm-replay/src/replay.rs` into `replay/{mod.rs,state.rs,summary.rs,timeline.rs,snapshot.rs,render.rs}`;
   - `crates/swarm-sim/src/urban.rs` into `urban/{mod.rs,planner.rs,judge.rs,risk.rs,bus.rs,geometry.rs}`.

7. Preserve behavior during each extraction:
   - move code in small commits grouped by crate or component;
   - avoid logic rewrites while splitting modules;
   - keep public types and functions reexported from their old module paths;
   - run focused tests after each large file split before continuing.

8. Add final enforcement and documentation:
   - run the line-count guard in strict mode for the 2000-line rule;
   - decide whether the 1000-line recommendation is a warning or a CI gate;
   - update developer documentation with the accepted module-size policy;
   - leave any intentional exceptions explicitly documented.

## Testing Strategy

### 1. Tests that need no refactoring

Planned together with the main functional changes:

- line-count guard:
  - hard fail for files above 2000 lines:
    `rg --files -g '*.rs' | while IFS= read -r f; do printf '%s %s\n' "$(wc -l < "$f")" "$f"; done | awk '$1 > 2000 {print; bad=1} END {exit bad}'`;
  - warning mode for files above 1000 lines:
    `rg --files -g '*.rs' | while IFS= read -r f; do printf '%s %s\n' "$(wc -l < "$f")" "$f"; done | awk '$1 > 1000 {print}'`;
  - strict variant for files above 1000 lines:
    `rg --files -g '*.rs' | while IFS= read -r f; do printf '%s %s\n' "$(wc -l < "$f")" "$f"; done | awk '$1 > 1000 {print; bad=1} END {exit bad}'`;
- formatting:
  - verification command: `cargo fmt --all --check`;
  - developer action before committing Rust moves: `cargo fmt --all`;
- linting:
  - `make clippy`;
- focused crate tests after mandatory splits:
  - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim`;
  - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples`;
  - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms`;
  - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-runtime`.

### 2. Tests that need light refactoring

- move `#[cfg(test)]` blocks from large files into module-local `tests.rs`;
- split `crates/swarm-examples/tests/sitl_agent.rs` into focused integration test files;
- extract repeated integration-test helpers into `crates/swarm-examples/tests/support/mod.rs`;
- add compile-level checks that public paths still work from the old module roots;
- add a small test or script assertion that no tracked `.rs` file exceeds the hard 2000-line limit.

### 3. Tests that need heavy refactoring

- full workspace test run after all module moves:
  - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test --workspace`;
- feature-gated MAVLink transport checks, if the local environment supports them:
  - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms --features mavlink-transport`;
- optional replay/SITL-focused integration smoke after `sitl_supervisor` and `sitl_agent` extraction;
- optional benchmark smoke only if the refactor touches benchmark binary behavior, not as a default requirement for pure module moves.

## Что могло сломаться

- Старые публичные import paths могут перестать компилироваться, если не сохранить `pub use` на уровне исходных модулей.
- Feature-gated MAVLink-код может случайно оказаться вне нужного `#[cfg(feature = "...")]`.
- Binary module resolution для `src/bin/*.rs` отличается от library modules, поэтому `sitl_agent` и `strategy_comparison` лучше выносить в library runtime modules, а сами binaries оставлять тонкими.
- Большие тестовые блоки могут начать конфликтовать по private visibility после переноса в соседние модули.
- `super`, `crate::`, `pub(crate)` и `use` пути после механического переноса могут требовать аккуратной правки.
- Слишком агрессивное разбиение может ухудшить читаемость, если сделать много файлов без устойчивых доменных границ.

## Risks and Tradeoffs

- Основной риск - поведенческая регрессия из-за неверного переноса кода, хотя сама задача должна быть behavior-preserving.
- Жесткий лимит 1000 строк может привести к искусственным модулям; разумнее сначала сделать 2000 hard gate, а 1000 использовать как целевой ориентир.
- Поэтапные commits увеличат количество промежуточных изменений, но упростят review и поиск регрессий.
- Перенос binary-кода в library modules повысит тестируемость, но изменит внутреннюю архитектуру `swarm-examples`.
- Разбиение тестов полезно для поддержки, но может потребовать больше времени, чем разбиение production-кода.

## Open Questions

- Должен ли порог 1000 строк стать обязательным CI-gate, или это предупреждение с допустимыми исключениями?
- Нужно ли разбивать файлы `1000..2000` в том же PR/итерации, что и обязательные `>2000`, или сделать это отдельным follow-up?
- Нужно ли сохранять максимально подробную git history через `git mv` и минимальные перемещения блоков, даже если это временно даст менее чистые границы?
- Должна ли проверка размера файлов жить в `scripts/`, `xtask`, `make`, GitHub/GitLab CI или только в документации?
- Нужны ли дополнительные публичные API compatibility tests для внешних пользователей crate, если проект пока не публикуется как стабильная библиотека?
