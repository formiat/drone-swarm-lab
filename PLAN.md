# План M57 - Supervisor Controller Boundary

## Context

Планируем этап **M57 - Supervisor Controller Boundary** из
`docs_raw/DRONE_A.19.md`.

Текущий выбранный вектор: local PX4/SIH and mock/fake validation only. В M57 не
идем в physical hardware, HIL, Gazebo as required gate, flight certification,
production ground-control workflows or live PX4 implementation. Цель M57 -
подготовить внутреннюю границу для M58, не меняя внешнее поведение текущего
`sitl_supervisor --mock` / `--dry-run`.

Текущее состояние по локальному коду:

- `crates/swarm-examples/src/bin/sitl_supervisor.rs` содержит CLI parsing,
  manifest writing, mock execution, runtime coordinator setup, heartbeat
  simulation, failure/reallocation handling, event log writing and metrics output
  в одном binary file.
- `SupervisorMetrics` сейчас приватная структура binary и используется в
  основном для итоговой строки `SUPERVISOR_METRICS`.
- `crates/swarm-examples/src/sitl_multi_agent.rs` уже содержит reusable config /
  manifest model для `multi_sitl.v1`.
- `crates/swarm-examples/src/sitl_observability.rs` уже содержит event log schema,
  recorder and replay summary для SITL events.
- `crates/swarm-examples/tests/sitl_agent.rs` уже содержит subprocess coverage
  для multi-agent supervisor dry-run/mock/reallocation and duplicate ownership.

Цель M57: отделить supervisor state machine от конкретной реализации агента:

```text
Supervisor
  owns: run lifecycle, runtime coordinator, task ownership, event log, metrics

AgentController
  owns: one agent lifecycle/progress/abort/final state
```

## Investigation context

`INVESTIGATION.md` в workspace отсутствует, поэтому дополнительных входных
выводов investigation нет.

Обязательные локальные протоколы прочитаны:

- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`

Notion/GitLab context не читался: в пользовательском prompt нет Notion task id,
GitLab MR, review comments or discussions; `notion_policy` указан как optional.

## Affected components

- `crates/swarm-examples/src/bin/sitl_supervisor.rs`
  - должен остаться thin CLI wrapper after refactor;
  - должен сохранить существующий CLI contract.
- `crates/swarm-examples/src/lib.rs`
  - потребуется экспорт нового internal module if tests need direct access.
- `crates/swarm-examples/src/sitl_supervisor.rs` или
  `crates/swarm-examples/src/sitl_controller.rs`
  - новый module для supervisor state machine, controller trait, mock/fake
    controller types, metrics and testable helpers.
- `crates/swarm-examples/src/sitl_multi_agent.rs`
  - источник `MultiAgentSitlManifest`, `MultiAgentLifecycle` and task subsets;
  - менять только если нужен small shared type for controller plans.
- `crates/swarm-examples/src/sitl_observability.rs`
  - использовать existing recorder; менять только если M57 выявит missing helper
    для supervisor tests.
- `crates/swarm-examples/tests/sitl_agent.rs`
  - сохранить existing subprocess tests and add/adjust tests for refactored CLI.
- New or existing tests under `crates/swarm-examples/tests/`
  - добавить direct supervisor/controller tests if module exports allow it.
- Documentation:
  - `README.md`;
  - `docs/STATUS.md`;
  - `docs/SITL_SETUP.md`;
  - `docs/REPLAY.md`;
  - `docs/HARDWARE_READINESS.md`;
  - `docs/REGRESSION.md` only if regression guidance changes;
  - optionally `docs_raw/DRONE_A.19.md` only if the implementation changes the
    planned scope materially.

## Implementation steps

1. **Зафиксировать текущий behavior before refactor**
   - Paths:
     - `crates/swarm-examples/src/bin/sitl_supervisor.rs`
     - `crates/swarm-examples/tests/sitl_agent.rs`
   - Прогнать targeted tests before code changes:
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor_mock_runs_two_agents_with_distinct_subsets_test`
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor_mock_reallocates_after_agent_loss_test`
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor_duplicate_ownership_rejected_test`
   - Цель: иметь локальный baseline for stdout/stderr-sensitive subprocess tests.

2. **Создать testable supervisor module**
   - Preferred path: `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Alternative if name conflict becomes confusing:
     `crates/swarm-examples/src/sitl_controller.rs`.
   - Move from binary into module:
     - `SupervisorMode`;
     - supervisor CLI-independent config structs;
     - `SupervisorMetrics`;
     - mock supervisor run logic;
     - helper functions:
       - `assign_manifest_tasks`;
       - `active_agent_ids`;
       - `complete_one_task_per_active_agent`;
       - `manifest_tasks_completed`;
       - `manifest_seq_for_task`;
       - validation helpers.
   - Keep binary-specific `parse_args`, usage string and `main` in
     `crates/swarm-examples/src/bin/sitl_supervisor.rs` unless moving parser is
     clearly cleaner and low-risk.

3. **Introduce `AgentController` boundary without live PX4**
   - Path: new supervisor/controller module.
   - Define minimal internal trait or equivalent state abstraction. Exact names
     may change, but responsibilities must be clear:
     - `agent_id`;
     - `lifecycle`;
     - upload/prepare step for mock waypoints;
     - start/poll/progress step;
     - abort/final state hook for future M58.
   - Implement:
     - `MockAgentController` for current mock behavior;
     - `FakeAgentController` or test-only fake for unit tests.
   - Do **not** implement `Px4AgentController` in M57.
   - Do **not** add live `--connection` supervisor mode in M57.

4. **Make `SupervisorMetrics` reusable and assertable**
   - Path: new supervisor/controller module.
   - Make metrics returned from supervisor run, not only printed.
   - Keep existing `SUPERVISOR_METRICS` stderr line stable where practical:
     - field names unchanged;
     - ordering unchanged if possible;
     - `tasks_recovered=none` behavior unchanged.
   - Add helper method only if useful, for example `format_summary_line()`.

5. **Refactor `sitl_supervisor` binary to thin wrapper**
   - Path: `crates/swarm-examples/src/bin/sitl_supervisor.rs`.
   - Binary should:
     - parse CLI;
     - load suite/config/manifest;
     - dispatch dry-run or mock run via module;
     - write/print manifest;
     - preserve current errors and usage text.
   - Public CLI must remain compatible:
     - `--dry-run`;
     - `--mock`;
     - `--scenario`;
     - `--config`;
     - `--manifest`;
     - `--replay-log`;
     - `--fail-agent`;
     - `--fail-after-ticks`;
     - `--heartbeat-timeout-ticks`;
     - `--max-ticks`.

6. **Add direct unit/integration tests for state machine**
   - Paths:
     - new module unit tests, or
     - `crates/swarm-examples/tests/sitl_supervisor.rs`, if exported module API
       is cleaner.
   - Cover:
     - happy path: two agents complete assigned tasks;
     - negative path: configured lost agent releases tasks;
     - edge case: invalid fail agent rejected;
     - metrics aggregation independent of stderr parsing;
     - deterministic failure timing around `fail_after_ticks`.

7. **Keep existing subprocess regression tests**
   - Path: `crates/swarm-examples/tests/sitl_agent.rs`.
   - Existing subprocess tests remain valuable because they verify CLI behavior
     and stderr/stdout contract.
   - Update assertions only if refactor intentionally changes formatting; avoid
     unnecessary output churn.

8. **Update README and companion Markdown docs**
   - Required docs:
     - `README.md`;
     - `docs/STATUS.md`;
     - `docs/SITL_SETUP.md`;
     - `docs/REPLAY.md`;
     - `docs/HARDWARE_READINESS.md`.
   - Update content to say:
     - M57 introduced an internal supervisor/controller boundary;
     - external mock/dry-run behavior remains portable;
     - no live PX4 controller was added in M57;
     - physical hardware remains out of scope;
     - M58 is the future live PX4/SIH execute milestone.
   - Update `docs/REGRESSION.md` only if the command set or regression gate
     guidance changes.
   - If docs tests assert wording, update them with the docs.

9. **Run formatting and verification**
   - Because Rust files will change, run mutating formatter:
     - `cargo fmt --all`
   - Then run clippy using repo-approved command:
     - `make clippy`
     - If `make clippy` is unavailable or not repo-approved, use:
       `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - Run targeted tests:
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent`
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-runtime reallocation`
   - If new module tests are in crate unit tests:
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_supervisor`

10. **Commit implementation**
    - Include all changed tracked files.
    - Include `Cargo.lock` if it changes.
    - Do not commit `.agent-io/inbox.txt` or `.agent-io/outbox.txt`.
    - Do not push.

## Testing strategy

### 1. Tests that need no refactoring

- Existing subprocess tests for `sitl_supervisor --mock`:
  - two agents with distinct subsets;
  - deterministic reallocation after `--fail-agent`;
  - duplicate ownership rejection.
- Existing docs test:
  - `cargo test -p swarm-examples --test sitl_docs`.
- Existing runtime reallocation tests:
  - `cargo test -p swarm-runtime reallocation`.
- Existing replay summary assertions in `sitl_agent.rs` for
  `agent_lost`, `task_released`, `task_reassigned`,
  `reallocation_completed`, `tasks_recovered` and latency.

### 2. Tests that need light refactoring

- Unit test for supervisor happy path via fake/mock controllers without
  subprocess.
- Unit test for metrics aggregation:
  - heartbeats;
  - completed tasks;
  - lost agents;
  - reassignment count;
  - recovered task ids;
  - reallocation latency.
- Unit test for deterministic failure timing:
  - fail before first poll;
  - fail after one tick;
  - fail after some task completion.
- Test for invalid `fail_agent` against manifest without spawning binary.
- Test for preserving summary formatting via `SupervisorMetrics` formatter.

### 3. Tests that need heavy refactoring

- Property tests over generated failure schedules and task ownership states.
- Cross-check replay events against final task registry state for arbitrary
  generated manifests.
- Full state-machine model tests with generated controller responses.
- These are not required for M57 acceptance, but should remain documented as
  future hardening if supervisor behavior grows.

## Что могло сломаться

- **CLI behavior / scripts**: moving code out of `sitl_supervisor.rs` could change
  error messages, usage text, stdout/stderr ordering or exit codes.
  - Проверка: existing subprocess tests and manual `--dry-run`/`--mock` smoke if
    needed.
- **Mock supervisor semantics**: refactor could accidentally change heartbeat
  count, task completion order, recovered task id ordering or timeout behavior.
  - Проверка: deterministic reallocation test plus direct metrics assertions.
- **Replay/event log**: event ordering or summary counts could drift.
  - Проверка: replay summary assertions and `docs/REPLAY.md` examples.
- **Task ownership / duplicate assignment**: extracting helpers could break
  manifest task assignment or duplicate ownership rejection.
  - Проверка: duplicate ownership tests and direct `assign_manifest_tasks` tests.
- **Docs consistency**: README/STATUS/SITL_SETUP/REPLAY/HARDWARE_READINESS could
  disagree about whether live PX4 exists in supervisor.
  - Проверка: `sitl_docs` test plus manual review of affected docs.
- **API exposure inside `swarm-examples`**: exporting too much can look like
  public API stabilization, which M57 explicitly should not do.
  - Проверка: keep module docs/internal naming clear; do not promise semver.
- **Performance/resources**: M57 should not add long-running loops or PX4 starts.
  - Проверка: no live PX4 tests in default test suite; unit tests use fake/mock.
- **Data/filesystem behavior**: replay log directory creation and manifest writing
  must stay compatible.
  - Проверка: tempdir-based tests for `--manifest` and `--replay-log`.

## Risks and tradeoffs

- **Trait too early vs. needed boundary**: a trait can be premature if M58 needs
  different methods. Mitigation: keep `AgentController` internal and minimal.
- **Moving too much at once**: extracting CLI parser and state machine together
  increases churn. Mitigation: keep parser in binary unless direct tests require
  otherwise.
- **Exact output stability**: preserving stderr byte-for-byte may slow refactor.
  Mitigation: preserve important `SUPERVISOR_METRICS` contract and tests; allow
  non-contract debug line changes only if docs/tests are updated.
- **Visibility boundaries**: unit tests may require `pub(crate)` exports.
  Mitigation: expose narrow structs/helpers inside `swarm-examples`, not public
  cross-crate API promises.
- **Future M58 fit**: M57 should not implement PX4, but it should not block
  `Px4AgentController`. Mitigation: include upload/start/poll/abort concepts but
  leave live transport out.

## Open questions

1. Название нового module: `sitl_supervisor.rs` или `sitl_controller.rs`.
   Рекомендация: `sitl_supervisor.rs`, потому что module is about supervisor
   state machine, while future `Px4AgentController` can live inside it or a
   child module later.
2. Где держать `FakeAgentController`: unit-test-only inside module or reusable
   test helper under `tests/`.
   Рекомендация: начать с unit-test-only fake, вынести позже if duplicated.
3. Нужно ли переносить CLI parser out of binary in M57.
   Рекомендация: не переносить, чтобы снизить churn.
4. Нужно ли создавать new docs section "M57".
   Рекомендация: да, но коротко: internal boundary only, no live PX4 supervisor
   yet.
5. Нужно ли делать result artifact for M57.
   Рекомендация: нет, M57 is refactor/test/docs milestone, not a run milestone.
