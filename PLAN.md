# Context

Проект сейчас выглядит как исследовательская mission-level платформа для
симуляции, планирования, координации и PX4/SITL-проверок дронов. Последние
этапы закрыли рабочие сценарии, replay/status honesty и pre-hardware план, но
структура кода всё ещё частично отражает историю роста проекта: есть
директории вида `*_parts`, крупные runtime-файлы, `#[path]`-подключения,
широкие `use super::*`, локальные `allow(unused_imports)` и местами вложенные
test-модули.

Цель этого плана - не менять поведение алгоритмов, CLI, форматы артефактов или
benchmark semantics. Цель - сделать архитектуру проекта более семантической:
модули должны называться по предметной ответственности, публичные границы
должны быть явнее, а бинарники должны стать тонкими оболочками над runtime/API.

Рефакторинг нужно делать итерационно. Самый безопасный порядок: сначала убрать
механический долг (`*_parts`, крупные файлы, `#[path]`), затем выделить
предметные подсистемы (`runner`, `mavlink`, `sitl_supervisor`), и только после
стабилизации рассматривать перенос SITL runtime в отдельный crate.

# Investigation context

Отдельного `INVESTIGATION.md` в корне нет.

Локальная проверка показала:

- workspace состоит из crate-ов `swarm-types`, `swarm-comms`, `swarm-sim`,
  `swarm-runtime`, `swarm-alloc`, `swarm-metrics`, `swarm-replay`,
  `swarm-scenarios`, `swarm-examples`, `swarm-safety`;
- `include!` в текущем коде не найден, то есть предыдущий механический разнос
  уже убрал самый грубый тип склейки файлов;
- остаются директории `metrics_parts`, `replay_parts`, `node_parts`,
  `dsl_parts`, `regression_parts`, `report_export_parts`, `urban_parts`,
  `sitl_observability_parts`, `strategy_comparison_parts`,
  `sitl_agent_parts`;
- остаются `#[path]`-модули, особенно в тестах и бинарниках;
- крупнейшие файлы всё ещё несут слишком много ответственностей, например
  `crates/swarm-sim/src/runner/scenario_runner_internal.rs`,
  `crates/swarm-sim/src/runner/tests.rs`,
  `crates/swarm-sim/src/benchmark.rs`,
  `crates/swarm-sim/src/regression_parts/suites_and_tests.rs`,
  `crates/swarm-examples/src/sitl_observability_parts/events_and_io.rs`,
  `crates/swarm-examples/src/bin/strategy_comparison_parts/cli_and_runs.rs`;
- пользовательские документы фиксируют текущий вектор как работу до железа:
  симуляция, миссии, replay, SITL/PX4 boundary, без замены PX4 motion control.

# Affected components

- `crates/swarm-metrics/src/metrics.rs` и
  `crates/swarm-metrics/src/metrics_parts/*`.
- `crates/swarm-replay/src/replay.rs` и
  `crates/swarm-replay/src/replay_parts/*`.
- `crates/swarm-runtime/src/node.rs` и
  `crates/swarm-runtime/src/node_parts/*`.
- `crates/swarm-sim/src/dsl.rs` и `crates/swarm-sim/src/dsl_parts/*`.
- `crates/swarm-sim/src/regression.rs` и
  `crates/swarm-sim/src/regression_parts/*`.
- `crates/swarm-sim/src/report_export.rs` и
  `crates/swarm-sim/src/report_export_parts/*`.
- `crates/swarm-sim/src/urban.rs` и `crates/swarm-sim/src/urban_parts/*`.
- `crates/swarm-sim/src/runner/*`, особенно
  `runner/scenario_runner_internal.rs`.
- `crates/swarm-comms/src/mavlink/*`.
- `crates/swarm-examples/src/sitl_supervisor/*`,
  `crates/swarm-examples/src/sitl_agent_runtime/*`,
  `crates/swarm-examples/src/sitl_observability_parts/*`,
  `crates/swarm-examples/src/bin/strategy_comparison.rs` и
  `strategy_comparison_parts/*`.
- Integration tests under `crates/swarm-examples/tests/*`.
- Potential future workspace changes in root `Cargo.toml` and
  `crates/swarm-examples/Cargo.toml` if `crates/swarm-sitl` is introduced.
- User-facing docs if crate/module boundaries change:
  `README.md`, `docs/STATUS.md`, `docs/EXTENSION_GUIDE.md`,
  `docs/SITL_SETUP.md`.

# Implementation steps

1. Establish a refactor baseline.

   Before moving code, run and record the current status of the relevant
   compile/test surface. Keep the first refactor commits behavior-preserving:
   no algorithm changes, no CLI flag changes, no report schema changes, no
   benchmark threshold changes.

   Recommended baseline commands:

   - `cargo fmt --all -- --check`;
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
   - targeted tests for crates touched in the first batch;
   - `rg -n "include!|#\\[path =|use super::\\*|allow\\(unused_imports\\)|allow\\(clippy::module_inception\\)" crates -g '*.rs'`.

2. Convert remaining `*_parts` directories into normal Rust module directories.

   This is the lowest-risk structural cleanup and should be done before deeper
   semantic changes. The intent is to move from `foo.rs + foo_parts/*` to
   `foo/mod.rs + foo/*.rs`, keeping public exports compatible.

   Proposed target layout:

   - `crates/swarm-metrics/src/metrics/mod.rs`;
   - `crates/swarm-metrics/src/metrics/run.rs`;
   - `crates/swarm-metrics/src/metrics/aggregate.rs`;
   - `crates/swarm-metrics/src/metrics/display.rs`;
   - `crates/swarm-metrics/src/metrics/tests.rs`;
   - `crates/swarm-replay/src/replay/mod.rs`;
   - `crates/swarm-replay/src/replay/state.rs`;
   - `crates/swarm-replay/src/replay/summary.rs`;
   - `crates/swarm-replay/src/replay/timeline.rs`;
   - `crates/swarm-replay/src/replay/render.rs`;
   - `crates/swarm-replay/src/replay/tests.rs`;
   - `crates/swarm-runtime/src/node/mod.rs`;
   - `crates/swarm-runtime/src/node/runtime.rs`;
   - `crates/swarm-runtime/src/node/gossip.rs`;
   - `crates/swarm-runtime/src/node/reallocation.rs`;
   - `crates/swarm-runtime/src/node/tests.rs`;
   - `crates/swarm-sim/src/dsl/mod.rs`;
   - `crates/swarm-sim/src/dsl/types.rs`;
   - `crates/swarm-sim/src/dsl/load.rs`;
   - `crates/swarm-sim/src/dsl/validate.rs`;
   - `crates/swarm-sim/src/dsl/urban_validate.rs`;
   - `crates/swarm-sim/src/dsl/tests.rs`;
   - `crates/swarm-sim/src/report_export/mod.rs`;
   - `crates/swarm-sim/src/report_export/json.rs`;
   - `crates/swarm-sim/src/report_export/csv.rs`;
   - `crates/swarm-sim/src/report_export/markdown.rs`;
   - `crates/swarm-sim/src/report_export/manifest.rs`;
   - `crates/swarm-sim/src/report_export/compare.rs`;
   - `crates/swarm-sim/src/report_export/focused.rs`;
   - `crates/swarm-sim/src/report_export/tests.rs`;
   - `crates/swarm-sim/src/urban/mod.rs`;
   - `crates/swarm-sim/src/urban/planner.rs`;
   - `crates/swarm-sim/src/urban/judge.rs`;
   - `crates/swarm-sim/src/urban/risk.rs`;
   - `crates/swarm-sim/src/urban/detection.rs`;
   - `crates/swarm-sim/src/urban/geometry.rs`;
   - `crates/swarm-sim/src/urban/tests.rs`.

   `regression_parts` should also be normalized even though it was not in the
   initial short list, because it is part of the same technical debt:

   - `crates/swarm-sim/src/regression/mod.rs`;
   - `crates/swarm-sim/src/regression/suites.rs`;
   - `crates/swarm-sim/src/regression/runner.rs`;
   - `crates/swarm-sim/src/regression/baseline.rs`;
   - `crates/swarm-sim/src/regression/thresholds.rs`;
   - `crates/swarm-sim/src/regression/report.rs`;
   - `crates/swarm-sim/src/regression/tests.rs`.

3. Split `runner/scenario_runner_internal.rs` by execution phase.

   `ScenarioRunner` should remain the public orchestration entry point, but the
   internal file should stop owning allocation, movement, event production,
   dynamic task mutation, safety checks and success semantics at once.

   Proposed target layout:

   - `crates/swarm-sim/src/runner/internal/mod.rs`;
   - `crates/swarm-sim/src/runner/internal/tick_loop.rs`;
   - `crates/swarm-sim/src/runner/internal/allocation.rs`;
   - `crates/swarm-sim/src/runner/internal/movement.rs`;
   - `crates/swarm-sim/src/runner/internal/mission_success.rs`;
   - `crates/swarm-sim/src/runner/internal/events.rs`;
   - `crates/swarm-sim/src/runner/internal/dynamic_tasks.rs`;
   - `crates/swarm-sim/src/runner/internal/safety.rs`;
   - `crates/swarm-sim/src/runner/internal/network.rs`.

   The desired result is that `tick_loop.rs` describes the loop shape, while
   each phase module owns one narrow set of decisions. This will make later
   mission types easier to add without turning the runner into a larger
   conditional block.

4. Make urban simulation modules explicitly domain-shaped.

   Urban work is becoming the most realistic no-hardware path, so it should not
   remain a grab bag. Keep it inside `swarm-sim`, but separate the concepts:

   - `crates/swarm-sim/src/urban/planner.rs` for route/waypoint planning;
   - `crates/swarm-sim/src/urban/judge.rs` for collision and validity checks;
   - `crates/swarm-sim/src/urban/risk.rs` for risk scoring;
   - `crates/swarm-sim/src/urban/detection.rs` for mocked detection events;
   - `crates/swarm-sim/src/urban/geometry.rs` for polygon/grid helpers;
   - optional `crates/swarm-sim/src/urban/patrol.rs` and `search.rs` when
     the scenario logic becomes large enough.

   This keeps the project aligned with the current direction: model realistic
   mission decisions without implementing low-level physical flight control.

5. Refactor `swarm-comms::mavlink` around client responsibilities.

   The MAVLink/PX4 layer should be split by protocol responsibility, not by
   historical helper boundaries. Preserve current public re-exports while
   moving implementation into:

   - `crates/swarm-comms/src/mavlink/errors.rs`;
   - `crates/swarm-comms/src/mavlink/types.rs`;
   - `crates/swarm-comms/src/mavlink/transport.rs`;
   - `crates/swarm-comms/src/mavlink/commands.rs`;
   - `crates/swarm-comms/src/mavlink/mission_items.rs`;
   - `crates/swarm-comms/src/mavlink/mission_upload.rs`;
   - `crates/swarm-comms/src/mavlink/lifecycle.rs`;
   - `crates/swarm-comms/src/mavlink/telemetry.rs`;
   - `crates/swarm-comms/src/mavlink/observer.rs`;
   - `crates/swarm-comms/src/mavlink/tests.rs` or narrower test modules.

   The key architectural rule: mission upload, vehicle lifecycle, telemetry
   polling and command helpers should be independently testable through fake
   transport/observer implementations.

6. Restructure `sitl_supervisor` around domain ports before any crate split.

   The supervisor is currently the strongest candidate for a hexagonal
   boundary. Do this inside `swarm-examples` first, because moving crates and
   changing architecture at the same time would create unnecessary churn.

   Proposed target layout:

   - `crates/swarm-examples/src/sitl_supervisor/mod.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/ports.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/controllers.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/mock.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/live.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/reallocation.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/validation.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/reports.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/events.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/artifacts.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/tests.rs`.

   Internal ports should include at least:

   - `AgentController`;
   - `MissionClient`;
   - `TelemetrySource`;
   - `EventSink`;
   - `SafetyGate`.

   Mock/fake/PX4 implementations should live behind these ports. This keeps
   real PX4/SITL behavior optional while making supervisor logic testable
   without live infrastructure.

7. Make `strategy_comparison` match the thin-binary pattern.

   Move the current binary-heavy implementation into library runtime modules:

   - `crates/swarm-examples/src/strategy_comparison_runtime/mod.rs`;
   - `crates/swarm-examples/src/strategy_comparison_runtime/cli.rs`;
   - `crates/swarm-examples/src/strategy_comparison_runtime/runs.rs`;
   - `crates/swarm-examples/src/strategy_comparison_runtime/artifacts.rs`;
   - `crates/swarm-examples/src/strategy_comparison_runtime/reports.rs`;
   - `crates/swarm-examples/src/strategy_comparison_runtime/regression.rs`;
   - `crates/swarm-examples/src/strategy_comparison_runtime/tests.rs`.

   `crates/swarm-examples/src/bin/strategy_comparison.rs` should become a thin
   `main` that parses CLI, calls runtime, maps errors to exit codes and exits.
   This should mirror the shape already used by `sitl_agent`.

8. Clean test module nesting and import hygiene.

   After structural moves, remove incidental debt instead of preserving it:

   - replace broad `use super::*` with explicit imports where practical;
   - remove `#![allow(unused_imports)]`;
   - remove `#[allow(clippy::module_inception)]` by renaming nested test
     modules or flattening them;
   - minimize `pub(super)` by moving helpers closer to consumers or creating
     small internal structs/functions with explicit visibility;
   - remove remaining `#[path]` module declarations where normal Rust module
     layout can represent the same structure.

9. Consider extracting `crates/swarm-sitl` only after the internal cleanup.

   A dedicated SITL crate is architecturally cleaner, but it is a larger API and
   dependency-boundary change. Treat it as phase two, not the first refactor.

   Candidate contents:

   - `sitl_agent_runtime`;
   - `sitl_supervisor`;
   - `sitl_connection`;
   - `sitl_plan`;
   - `sitl_progress`;
   - `sitl_report`;
   - `sitl_safety`;
   - `sitl_observability`;
   - `sitl_multi_agent`.

   `swarm-examples` can then keep only thin binaries/examples that depend on
   `swarm-sitl`. If this split happens, update root `Cargo.toml`,
   `crates/swarm-examples/Cargo.toml`, docs, integration tests and any
   crate-level public API references together.

10. Sync documentation and developer tooling.

   Update docs only after code boundaries actually move:

   - `docs/EXTENSION_GUIDE.md` for new extension points;
   - `docs/SITL_SETUP.md` if SITL commands or crate names change;
   - `docs/STATUS.md` if architecture status changes;
   - `README.md` only for user-facing command/API changes.

   Add or update a small local architecture check if useful, for example a
   script that reports large Rust files and remaining `*_parts`/`#[path]`
   patterns. This should be advisory unless the team wants it as a hard gate.

# Testing strategy

1. Tests that need no refactoring

   These should be planned together with the main structural changes and run
   after each focused batch:

   - `cargo fmt --all`;
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
   - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-metrics`;
   - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-replay`;
   - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-runtime`;
   - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim dsl`;
   - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim regression`;
   - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim report_export`;
   - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim urban`;
   - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim runner`;
   - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-comms --all-features`;
   - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-examples --all-targets`;
   - smoke checks that `strategy_comparison`, `sitl_agent` and
     `sitl_supervisor` still parse existing CLI flags;
   - text checks that no `include!` returns and that planned `*_parts`
     directories are removed.

2. Tests that need light refactoring

   These are not blockers for the first moves, but should be folded in as files
   are touched:

   - move large `tests.rs` files into domain-specific test modules matching the
     new production modules;
   - add public facade import tests for modules whose internal layout changes,
     so old public paths remain valid;
   - add fake transport tests for MAVLink mission upload, lifecycle and
     telemetry modules after they are split;
   - add supervisor port tests using mock `MissionClient`, `TelemetrySource`,
     `EventSink` and `SafetyGate`;
   - add binary-wrapper tests for `strategy_comparison` to verify the thin
     binary maps runtime errors to expected exit codes;
   - add architecture smoke tests that check there are no nested
     `tests::tests::...` names for refactored modules.

3. Tests that need heavy refactoring

   These are appropriate if the project proceeds to the `swarm-sitl` crate
   split or turns architecture checks into release gates:

   - new `crates/swarm-sitl` integration tests covering agent runtime,
     supervisor runtime, observability and artifact generation through the new
     crate boundary;
   - compatibility tests proving old `swarm-examples` binaries still behave as
     wrappers over `swarm-sitl`;
   - structured artifact roundtrip tests across `swarm-sitl`,
     `swarm-replay` and `swarm-sim`;
   - a workspace-level architecture lint that fails on new `*_parts`,
     unnecessary `#[path]`, or large files above an agreed threshold;
   - broader full-workspace regression runs after crate dependency boundaries
     are changed.

# Risks and tradeoffs

- This refactor has high churn. Even behavior-preserving moves can create merge
  conflicts and obscure small logic changes in review.
- Moving from file modules to directory modules can accidentally change
  visibility and public re-export behavior.
- Removing `use super::*` and `pub(super)` may reveal hidden coupling. That is
  useful, but it can expand the scope if done too aggressively in one commit.
- `swarm-comms::mavlink` is close to external PX4 behavior, so refactors there
  need fake-transport tests before large moves.
- `sitl_supervisor` ports are the right direction, but over-abstracting too
  early could make simple CLI workflows harder to follow.
- Extracting `swarm-sitl` is probably architecturally correct, but doing it
  before internal cleanup would combine crate dependency churn with semantic
  refactoring. It should be a later phase.
- Strict no-large-file gates can become busywork if applied before the target
  module boundaries are agreed. Start with reporting; promote to a gate later
  only if it helps.

# Open questions

- Should `crates/swarm-sitl` be created in this refactor wave, or should it wait
  until `sitl_supervisor` and `sitl_agent_runtime` are already clean internally?
- Should the project require zero remaining `#[path]` declarations everywhere,
  or only in production code and main integration tests?
- What file-size threshold is worth enforcing: 500 lines, 800 lines, 1000
  lines, or advisory-only reporting?
- Should `swarm-examples` remain the home for runnable binaries, or should the
  project eventually introduce a dedicated CLI crate?
- Which module paths are considered public compatibility surface and must be
  preserved through re-exports?
- Should `strategy_comparison_runtime` remain under `swarm-examples`, or should
  it eventually move into a benchmark/comparison crate?
