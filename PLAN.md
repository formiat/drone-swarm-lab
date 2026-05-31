# Context

Планируем milestone **M60 - PX4/SIH Supervisor Hardening** по
`docs_raw/DRONE_A.19.md`.

Цель M60 - сделать local live/SIH `sitl_supervisor` достаточно устойчивым для
повторяемых research runs: ошибки должны быть typed/actionable, exit codes -
стабильными, artifacts - воспроизводимыми, а docs/tests - честно описывающими
текущий scope.

Текущий важный контекст:

- M58 live multi-agent supervisor уже есть как experimental local
  `sitl_supervisor --connection --execute`.
- M59 сейчас является **partial foundation**, а не full stepwise live loop:
  `--reupload-on-failure` работает после terminal one-shot failed agent run и
  может заменить mission state у pending survivor before it starts.
- M60 не должен превращаться в M59b: full stepwise live loss detection,
  active-survivor abort/clear/upload/execute replacement и real PX4/SIH
  failure artifact остаются отдельной follow-up работой.
- M60 должен harden текущий M58/M59 foundation workflow так, чтобы следующий
  local/manual run не требовал ручного разбора stdout и локального состояния.

Scope M60 остается **local PX4 SITL / PX4 SIH only**. Не заявляем hardware
readiness, не делаем HIL/Gazebo required validation, public API semver promise,
benchmark publication или расширение hardware checklist.

# Investigation Context

`INVESTIGATION.md` отсутствует.

Прочитаны:

- `/home/formi/Documents/RustProjects/drone/.agent-io/inbox.txt`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`;
- `docs_raw/DRONE_A.19.md`;
- текущие `README.md`, `docs/STATUS.md`, `docs/SITL_SETUP.md`,
  `docs/REPLAY.md`, `docs/HARDWARE_READINESS.md`;
- `crates/swarm-examples/src/bin/sitl_supervisor.rs`;
- `crates/swarm-examples/src/sitl_supervisor.rs`;
- `crates/swarm-examples/src/sitl_report.rs`;
- `crates/swarm-examples/src/sitl_observability.rs`;
- `crates/swarm-examples/tests/sitl_agent.rs`;
- `crates/swarm-examples/tests/sitl_docs.rs`.

Notion/GitLab: в prompt нет Notion task id и нет GitLab/MR target;
`notion_policy=optional`, поэтому внешние Notion/GitLab чтения не нужны и не
выполнялись.

Текущие факты по коду:

- `crates/swarm-examples/src/bin/sitl_supervisor.rs::main` печатает
  `error: {error}`, usage и всегда делает `std::process::exit(1)` для любых
  ошибок. M60 должен добавить классификацию ошибок и стабильные exit codes.
- `SitlError` уже typed, но слишком широкий для supervisor CLI policy:
  `MultiAgentConfigInvalid`, `ConnectionFailed`, `FeatureMissing`,
  `SafetyValidationFailed`, `RunReportWrite`, `ReplayLogWrite` и другие
  варианты не дают однозначного exit code без дополнительного mapping layer.
- `write_sitl_event_log`, `write_sitl_run_report`,
  `write_sitl_multi_agent_run_report` и `write_or_print_manifest` уже создают
  parent directories, но overwrite policy отсутствует: existing files silently
  overwritten.
- `SupervisorLiveConfig` уже содержит `run_id: Option<String>`, но CLI не
  принимает `--run-id`; default run id сейчас derived from scenario and can
  collide between repeated runs.
- `SitlMultiAgentRunReport` уже содержит `schema_version`, `run_id`, `mode`,
  `agents`, `overall_status`, `event_log_path`, `reallocation`,
  `known_limitations`, но M60 требует hardening:
  `task_ownership`, `events_summary`, `final_status`, `limitations`.
- `SitlEventLogSummary` already exists for replay summary; нужно переиспользовать
  или serialize compatible subset into report instead of duplicating counters.
- Existing tests already cover many CLI negative cases and report serialization,
  so M60 should extend them rather than replacing the current test shape.

# Affected Components

- `crates/swarm-examples/src/bin/sitl_supervisor.rs`
  - CLI flags: `--run-id`, `--output-dir`, `--force`;
  - stable output layout orchestration;
  - typed supervisor CLI error mapping;
  - stable exit code handling via `std::process::ExitCode`;
  - subprocess-visible error messages and usage behavior.
- `crates/swarm-examples/src/sitl_plan.rs`
  - likely no large rewrite, but `SitlError` may need small additive variants
    or helper methods if existing variants cannot express supervisor categories.
- `crates/swarm-examples/src/sitl_supervisor.rs`
  - propagate enough failure context from live/fake controllers for error
    classification;
  - ensure partial live/fake failures still produce structured reports where
    possible;
  - produce report-ready ownership and event summary data.
- `crates/swarm-examples/src/sitl_report.rs`
  - add `task_ownership`, `events_summary`, `final_status`, `limitations`;
  - keep backward compatibility for `overall_status` and `known_limitations`
    unless intentionally migrated with explicit tests.
- `crates/swarm-examples/src/sitl_observability.rs`
  - expose a serializable `SitlEventSummaryReport` or make existing summary
    serde-friendly if appropriate;
  - keep replay CLI output stable unless intentionally changed.
- `crates/swarm-examples/tests/sitl_agent.rs`
  - subprocess CLI tests for exit code matrix, output path behavior, `--force`,
    `--run-id`, `--output-dir`, and no-overwrite behavior.
- `crates/swarm-examples/tests/sitl_docs.rs`
  - docs assertions for M60 hardening, local-only scope, output layout,
    troubleshooting and no hardware readiness claim.
- Documentation:
  - `README.md`;
  - `docs/SITL_SETUP.md`;
  - `docs/STATUS.md`;
  - `docs/REPLAY.md`;
  - `docs/HARDWARE_READINESS.md`;
  - optional new `docs/SITL_SUPERVISOR_OUTPUTS.md` only if output layout section
    becomes too large for `docs/SITL_SETUP.md`.

# Implementation Steps

1. Baseline audit and current behavior lock.
   - Files: no code changes in this step.
   - Read current supervisor CLI tests around `multi_agent_sitl_supervisor_*`.
   - Confirm existing negative behavior and output behavior before edits with
     targeted tests under timeout:
     - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor`
     - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_supervisor`
   - Do not run live PX4/SIH in default implementation checks.

2. Add supervisor error classification layer.
   - Files:
     - `crates/swarm-examples/src/bin/sitl_supervisor.rs`;
     - optionally `crates/swarm-examples/src/sitl_supervisor_error.rs` if the
       mapping grows beyond the binary.
   - Add types conceptually equivalent to:
     - `SupervisorErrorKind`;
     - `SupervisorExitCode`;
     - `SupervisorCliError`.
   - Required categories:
     - bad config;
     - invalid lifecycle combination;
     - endpoint unavailable;
     - heartbeat timeout;
     - mission upload failed;
     - command rejected;
     - progress timeout;
     - abort failed;
     - partial run failed;
     - artifact write failed.
   - Use existing `SitlError` as source; do not erase detailed messages.
   - Error text must include actionable context: path/agent id/task id/endpoint
     where available.

3. Implement consistent exit codes.
   - File: `crates/swarm-examples/src/bin/sitl_supervisor.rs`.
   - Replace direct `std::process::exit(1)` with `ExitCode`.
   - Proposed initial mapping:
     - `0`: success;
     - `2`: CLI/config/schema/lifecycle error;
     - `3`: safety validation error;
     - `20`: PX4 endpoint unavailable or feature missing for connection mode;
     - `21`: mission upload or command rejected before useful execution;
     - `22`: heartbeat/telemetry/progress timeout before completion;
     - `23`: abort failed;
     - `30`: runtime failure after start / partial agent failure;
     - `40`: artifact/report/replay write failure.
   - If these numeric values conflict with existing project conventions, keep
     the categories but adjust numbers in one place and document them.
   - Add tests asserting actual process status codes for representative cases.

4. Add stable result layout support while preserving existing flags.
   - File: `crates/swarm-examples/src/bin/sitl_supervisor.rs`.
   - Add optional:
     - `--output-dir <path>`;
     - `--run-id <id>`;
     - `--force`.
   - Existing `--manifest`, `--run-report`, `--replay-log` remain supported.
   - If `--output-dir` is provided and individual paths are not provided, derive:
     - `<output-dir>/<run-id>/manifest.json`;
     - `<output-dir>/<run-id>/run-report.json`;
     - `<output-dir>/<run-id>/events.sitl-log.json`;
     - `<output-dir>/<run-id>/replay-summary.txt` if summary writing is cheap.
   - If `--run-id` is absent, generate a stable but collision-resistant id:
     `sitl-supervisor-<scenario-name>-<utc-rfc3339-basic-or-epoch-seconds>`.
   - Refuse to overwrite existing artifact files unless `--force` is set.
   - Use `PathBuf` internally; keep tests portable with `tempfile`.

5. Centralize checked artifact writing.
   - Files:
     - `crates/swarm-examples/src/bin/sitl_supervisor.rs`;
     - `crates/swarm-examples/src/sitl_report.rs`;
     - `crates/swarm-examples/src/sitl_observability.rs`.
   - Add a small helper/API for:
     - create parent dirs;
     - detect existing files;
     - enforce overwrite policy;
     - return typed artifact write errors.
   - Avoid duplicating directory creation logic across manifest/report/log.
   - Keep current no-parent-path behavior (`manifest.json` in current dir)
     working.

6. Harden multi-agent report schema.
   - File: `crates/swarm-examples/src/sitl_report.rs`.
   - Add additive fields:
     - `task_ownership`: from `manifest.ownership_summary`;
     - `events_summary`: serializable summary of the common SITL event log;
     - `final_status`: preferred M60 field, mirroring current `overall_status`
       until a later schema version removes the old name;
     - `limitations`: preferred M60 field, mirroring current
       `known_limitations`.
   - Keep serde defaults for new fields where practical.
   - Keep `overall_status` and `known_limitations` for compatibility unless
     reviewer explicitly accepts a schema-breaking bump.
   - Add report roundtrip tests for:
     - success;
     - partial failure;
     - reallocation fields;
     - old JSON missing new fields.

7. Attach event summaries to reports.
   - Files:
     - `crates/swarm-examples/src/sitl_observability.rs`;
     - `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Reuse `summarize_sitl_event_log` or extract an internal summary from
     `SitlEventRecorder` before writing.
   - Include counters relevant to M60:
     - run started/finished;
     - per-agent started/finished;
     - mission items sent;
     - waypoint/task completed;
     - failures;
     - reallocation counters;
     - survivor mission updates;
     - final status.
   - Ensure report summary and replay summary agree in tests.

8. Improve partial failure reporting.
   - File: `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Ensure terminal partial failures still produce structured report when the
     supervisor has enough context to do so.
   - For hard pre-run validation failures, do not fake a report; return typed
     config/safety error.
   - For after-start failures:
     - keep per-agent final status;
     - include error string;
     - set `final_status` / `overall_status` to `partial_failed`,
       `failed`, `timeout`, or `completed_with_reallocation` consistently;
     - map the CLI exit code to runtime failure after start.

9. Add fake controller error matrix.
   - File: `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Extend fake live controller tests to cover:
     - endpoint unavailable / connection open failure;
     - mission upload failed;
     - command rejected;
     - heartbeat timeout;
     - progress timeout;
     - abort failed;
     - partial run failed after one completed task.
   - These tests must not depend on local PX4, network sockets, `$HOME`, or
     external simulator state.

10. Add subprocess CLI and output path tests.
    - File: `crates/swarm-examples/tests/sitl_agent.rs`.
    - Add tests for:
      - exit code mapping for representative CLI/config/safety/feature errors;
      - `--run-id` appears in report/log metadata;
      - `--output-dir` creates stable layout under tempdir;
      - existing output file is rejected without `--force`;
      - existing output file is overwritten with `--force`;
      - direct `--manifest` without parent continues to work.
    - Keep tests self-contained and portable.

11. Update docs and README.
    - Files:
      - `README.md`;
      - `docs/SITL_SETUP.md`;
      - `docs/STATUS.md`;
      - `docs/REPLAY.md`;
      - `docs/HARDWARE_READINESS.md`.
    - Required docs content:
      - exact local PX4/SIH startup commands for one and two local instances;
      - multi-instance endpoint/system-id setup;
      - stable result layout examples;
      - `--run-id`, `--output-dir`, `--force`;
      - exit code table;
      - troubleshooting:
        - port conflicts;
        - heartbeat timeout;
        - wrong `system_id`;
        - upload/command rejection;
        - no-progress timeout;
        - output overwrite refusal;
      - interpreting reallocation artifacts;
      - explicit not-hardware statement.
    - Docs must continue to say local PX4/SIH only; no real hardware readiness.

12. Add docs tests.
    - File: `crates/swarm-examples/tests/sitl_docs.rs`.
    - Assert key M60 phrases:
      - `PX4/SIH Supervisor Hardening`;
      - `--output-dir`;
      - `--run-id`;
      - `--force`;
      - `exit code`;
      - `port conflicts`;
      - `wrong system id`;
      - `not hardware-ready` or existing equivalent.

13. Final verification and commit.
    - Rust changes require:
      - `timeout 300s cargo fmt --all`;
      - `timeout 300s /home/formi/.local/bin/runlim cargo clippy --workspace --all-targets --all-features -- -D warnings`.
    - Targeted tests:
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor`
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_supervisor`
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_report`
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_observability`
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`
    - Additional checks:
      - `git diff --check`;
      - `find . -name '*.proptest-regressions' -print`.
    - No live PX4/SIH run is required for default M60 implementation checks.
      Manual/ignored live checks must be documented if added.

# Testing Strategy

Все автоматические тесты должны быть portable: no local PX4, no simulator
process, no network endpoint dependency, no `$HOME`, no machine-specific
absolute paths, no pre-existing local state. Manual PX4/SIH verification is
allowed only as optional/ignored/manual evidence and must not replace practical
automated coverage.

## Category 1 - Tests That Need No Refactoring

Эти тесты уже существуют или требуют только небольшого расширения assertions.
Они должны запускаться вместе с M60 implementation:

1. CLI rejects missing values and conflicting modes.
   - Existing scope:
     `cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor`
   - Keep coverage for missing `--scenario`, `--config`, `--manifest`,
     conflicting modes, invalid live/mock combinations.

2. Config validation errors include agent/task context.
   - Existing tests cover duplicate ownership, missing/invalid agent/task ids,
     unsafe task subset and hardware-candidate rejection.
   - M60 should add exit-code assertions without changing fixture portability.

3. Replay summary handles failure reports.
   - Existing scope:
     `cargo test -p swarm-examples sitl_observability`
   - Keep reallocation/failure summary tests green while adding report
     `events_summary` compatibility checks.

4. Report writers create parent directories.
   - Existing `sitl_report` tests cover parent creation.
   - M60 should add overwrite-policy tests around the new checked writer helper.

5. Docs boundary test.
   - Existing `sitl_docs` should remain green and gain M60 hardening anchors.

## Category 2 - Tests That Need Light Refactoring

Эти тесты должны быть реализованы вместе с M60:

1. Supervisor exit code mapping unit/subprocess tests.
   - Bad config / CLI -> config exit code.
   - Safety validation -> safety exit code.
   - Feature missing or endpoint unavailable -> PX4 unavailable exit code.
   - Upload/command rejection -> mission rejected exit code.
   - Progress timeout -> timeout exit code.
   - Partial failure after start -> runtime failure exit code.

2. Fake controller error matrix.
   - Extend existing fake live controller to emit controlled failure statuses.
   - Verify per-agent report status, final status, error message, and exit
     category for each representative failure.

3. Multi-agent report schema snapshot-style tests.
   - Current report with new fields serializes expected names:
     `task_ownership`, `events_summary`, `final_status`, `limitations`.
   - Old JSON without new fields still deserializes where defaults are promised.
   - `final_status` matches `overall_status` during compatibility period.
   - `limitations` matches `known_limitations` during compatibility period.

4. Event summary consistency tests.
   - Build a fake/foundation run, write event log, summarize it, and assert that
     report `events_summary` contains the same key counters.

5. Output path behavior tests using temp directories.
   - `--output-dir` creates `<run-id>/manifest.json`,
     `<run-id>/run-report.json`, `<run-id>/events.sitl-log.json`.
   - Existing files are rejected without `--force`.
   - Existing files are overwritten with `--force`.
   - `--run-id` controls artifact names/metadata.
   - Direct path flags still work for backward compatibility.

6. Docs tests.
   - Assert README/SITL docs mention M60 flags, exit codes, troubleshooting and
     not-hardware boundary.

## Category 3 - Tests That Need Heavy Refactoring

Эти тесты полезны, но не должны блокировать portable M60:

1. End-to-end supervisor harness with multiple fake agents and randomized
   errors.
   - Generate arbitrary failure matrix and assert final report/event consistency.
   - Useful after the fake controller boundary is made richer.

2. Manual/ignored live PX4/SIH negative cases.
   - Endpoint unavailable with a real local port.
   - Wrong system id with running PX4.
   - Command rejected by PX4.
   - No-progress timeout during local SIH execute.
   - Must be ignored/env-gated and documented with exact local prerequisites.

3. Process-control harness for two PX4/SIH instances.
   - Starts/stops local PX4 SIH processes and validates artifact layout.
   - Too machine-specific for default CI; keep manual only.

4. Full compatibility matrix for old report JSON artifacts.
   - Could load a curated set of historical reports if they become stable
     fixtures. Do not depend on machine-local result directories.

# Required Runs, Builds, Installs, And Artifacts

- No installs should be required for M60 portable implementation.
- Default implementation checks must stay under `timeout 300s` per command.
- All `cargo test` / `cargo run` invocations must use
  `/home/formi/.local/bin/runlim`; all `cargo test` commands must set
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1`.
- Manual live PX4/SIH runs are not required for M60 default completion.
- If a manual PX4/SIH negative artifact is attempted, document:
  - PX4 checkout/path/version;
  - exact startup commands;
  - endpoints/system ids;
  - supervisor command;
  - expected exit code;
  - report/log paths;
  - limitations.
- If a manual run, build, install or simulator startup would take longer than 5
  minutes, do not perform it in the implementation round; document it as a
  manual prerequisite.

# Risks And Tradeoffs

- **Exit code compatibility:** changing all errors from exit code `1` to
  categorized codes can break scripts that only expect `1`. Mitigation:
  document table in README/SITL docs and test representative codes.
- **Report schema compatibility:** adding `task_ownership`, `events_summary`,
  `final_status`, and `limitations` can affect strict JSON consumers.
  Mitigation: additive fields with serde defaults; keep old fields during
  compatibility period.
- **Overwrite policy behavior change:** refusing overwrite by default is safer
  but can surprise users who reused fixed output paths. Mitigation: clear error
  message and `--force`.
- **Run id generation:** timestamp-based ids improve repeatability of artifact
  capture but can make tests flaky if asserted literally. Mitigation: tests use
  explicit `--run-id`.
- **Scope creep into M59b:** heartbeat/progress hardening can drift into full
  stepwise live reallocation. Mitigation: M60 only classifies/report failures
  for current foundation unless a separate M59b plan is accepted.
- **Docs overclaim risk:** hardening docs can make the workflow sound
  production-ready. Mitigation: keep local PX4/SIH and not-hardware wording in
  every relevant doc.
- **Duplication risk:** report/log/manifest writers already create directories;
  adding checked writes can duplicate logic. Mitigation: centralize helper and
  reuse it.
- **Manual artifact flakiness:** live PX4/SIH negative cases depend on local
  simulator timing and ports. Mitigation: keep default proof in fake/mock tests.

# Open Questions

1. Exact exit code numbers: are the proposed values acceptable, or should the
   project use a smaller table such as `2/3/10/20/30/40`?
2. Should `--force` apply to all artifact paths, including explicit
   `--manifest`, `--run-report`, `--replay-log`, or only to `--output-dir`
   generated layout? Recommendation: apply to all writes for consistency.
3. Should `--output-dir` be allowed together with explicit artifact paths?
   Recommendation: allow it, with explicit paths overriding generated paths.
4. Should `final_status`/`limitations` replace `overall_status`/
   `known_limitations` immediately or be additive aliases through M60?
   Recommendation: additive aliases through M60 to avoid schema breakage.
5. Should M60 write `replay-summary.txt` automatically when a replay log is
   written, or should it only expose a command in docs? Recommendation: write it
   for `--output-dir` stable layout, leave direct path mode unchanged unless
   implementation is trivial.
6. Should endpoint unavailable be distinguishable from feature missing when the
   binary lacks `mavlink-transport`? Recommendation: both map to the PX4
   unavailable category but keep different messages.
