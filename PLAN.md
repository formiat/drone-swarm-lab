# PLAN.md - M48 Single-Agent PX4 SITL Golden Path

## Context

Идем по ветке 6 Real SITL / PX4 из `docs_raw/DRONE_A.17.md`.

M43-M47 уже закрыли foundation:

- `sitl_agent --mock` and `--dry-run`;
- typed scenario/connection/safety errors;
- real MAVLink mission upload behind `mavlink-transport`;
- pre-upload safety validation;
- opt-in `--execute` lifecycle: upload -> arm/takeoff/start -> post-start heartbeat;
- telemetry progress loop with `MISSION_CURRENT`, `MISSION_ITEM_REACHED`, task-status mapping,
  disconnect/no-progress timeout and RTL abort.

M48 должен стать первым настоящим single-agent PX4 SITL golden path, где один агент проходит:

```text
scenario -> safety validation -> mission upload -> arm/takeoff/start -> telemetry
-> task completion -> final report
```

Главный результат M48 - не новый алгоритм, а воспроизводимый operator/developer workflow:
одна documented команда, один tested PX4 setup, один structured final report, четкая граница
между mock, dry-run, PX4 SITL and real hardware.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует.

`PLAN.md` перед этим раундом отсутствовал.

Прочитаны:

- `docs_raw/DRONE_A.17.md`;
- `docs/SITL_SETUP.md`;
- `README.md`;
- `crates/swarm-examples/src/bin/sitl_agent.rs`;
- `crates/swarm-examples/src/sitl_progress.rs`;
- обязательные local protocol docs для Notion/GitLab.

Notion/GitLab:

- `notion_policy=optional`;
- task id / MR target в prompt отсутствуют;
- Notion/GitLab CLI не нужны для этого plan round.

Текущее состояние кода:

- `sitl_agent --execute` уже умеет пройти M47 lifecycle/progress loop, но финальный отчет существует
  только как human-readable stderr lines.
- `SitlMissionProgressReport` уже содержит часть нужных полей: final status, total/completed/failed,
  current task id, failure reason.
- Нет stable serializable run-report schema для полного E2E результата.
- Нет fakeable full golden-path driver на уровне `sitl_agent`: есть fake telemetry runtime для loop tests,
  но upload/lifecycle/progress как единый command outcome пока не проверяются как one golden workflow.
- Docs еще описывают experimental PX4 execute path, но не фиксируют verified PX4 version/backend/startup
  command/result for M48.

## Affected components

- `crates/swarm-examples/src/sitl_report.rs` (new)
  - structured `SitlRunReport`;
  - serializable `SitlRunFinalStatus`;
  - success/failure constructors;
  - JSON writer/roundtrip tests.
- `crates/swarm-examples/src/lib.rs`
  - export report module.
- `crates/swarm-examples/src/bin/sitl_agent.rs`
  - add `--run-report <path>` or equivalent report-output option;
  - produce final run report on execute success and on bounded execute failure;
  - refactor connection execution into fakeable driver seam for tests;
  - keep mock/dry-run behavior portable and unchanged unless report output is explicitly requested.
- `crates/swarm-examples/tests/sitl_agent.rs`
  - CLI parsing tests for report option;
  - fake golden path command tests;
  - fake failure exit-code/report tests.
- `docs/SITL_SETUP.md`
  - tested PX4 setup section;
  - exact golden command;
  - expected report example;
  - troubleshooting for ports, heartbeat, mission requests, telemetry completion.
- `README.md`
  - Quick Start update for M48;
  - Current Status Real PX4 row -> M48 golden path;
  - limitations still explicitly say no real hardware support and no multi-agent SITL.
- Optional: `docs/STATUS.md`
  - update if it has a Real PX4 status row that would become stale.

## Implementation steps

1. Add structured final report model in `crates/swarm-examples/src/sitl_report.rs`.
   - Fields:
     - `schema_version`;
     - `scenario_path`;
     - `scenario_name`;
     - `mission`;
     - `profile`;
     - `agent_id`;
     - `connection_string`;
     - `mode`;
     - `mission_item_count`;
     - `completed_count`;
     - `failed_count`;
     - `final_status`;
     - `error`;
     - `abort_result`;
   - Use serde with snake_case enums.
   - Keep timestamps out unless the project already has a stable clock abstraction for this binary;
     M48 report should be deterministic and easy to test.

2. Add report writing helper.
   - API shape:
     - `write_sitl_run_report(path: impl AsRef<Path>, report: &SitlRunReport)`.
   - Write pretty JSON.
   - Create parent directory if needed.
   - Return typed `SitlError` variant for write/serialization failures.

3. Add CLI option in `crates/swarm-examples/src/bin/sitl_agent.rs`.
   - Preferred option: `--run-report <path>`.
   - Validate missing value with existing `MissingArgument` style.
   - Scope:
     - report option is useful for `--connection --execute`;
     - for `--mock`/`--dry-run`, either reject clearly or document as unsupported in M48.
   - Do not change default stdout/stderr output contract more than needed.

4. Refactor connection execution into a fakeable single-agent driver seam.
   - Extract a small internal abstraction from `run_connection` so tests can script:
     - mission upload accepted/rejected;
     - lifecycle success/failure;
     - telemetry progress success/failure;
     - abort result.
   - Keep production path backed by current `MavlinkTransport`.
   - Avoid leaking fake/test-only APIs into public crates unless the seam is also useful for future M49/M52.

5. Build `SitlRunReport` from the real execution outcome.
   - On success:
     - final status `completed`;
     - completed count from `SitlMissionProgressReport`;
     - error `null`;
     - abort result `null`.
   - On failure after scenario/report context is known:
     - final status should reflect the failure class when possible:
       `failed`, `disconnected`, `rejected`, `timed_out_no_progress`, or `aborted`;
     - include human-readable error string;
     - include abort result if an abort was attempted.
   - If failure happens before a mission plan exists, preserve normal typed CLI error and do not invent partial
     report unless the implementation can construct one safely.

6. Add fake golden path tests.
   - Script:
     - valid `scenarios/sitl.waypoints.json`-like fixture;
     - safety validation passes;
     - upload accepted;
     - lifecycle started;
     - telemetry reaches all waypoint seqs.
   - Assert:
     - process/model returns success;
     - report JSON has scenario, agent id, mission item count, completed count and final status.

7. Add fake failure tests.
   - Upload rejected -> non-zero/failure result and report `error`.
   - Lifecycle command rejected -> non-zero/failure result and report `error`.
   - Telemetry no-progress/disconnect -> non-zero/failure result, failed counts, abort result.
   - Report write failure -> typed error and non-zero exit.

8. Add final report serialization tests.
   - Roundtrip JSON for success report.
   - Roundtrip JSON for failure report.
   - Enum values serialized in `snake_case`.
   - Report schema remains portable: no absolute machine-specific paths in test fixtures except tempdir paths
     owned by the test.

9. Document tested PX4 setup in `docs/SITL_SETUP.md`.
   - Add a section "M48 Tested PX4 SITL Setup" with explicit placeholders to fill during implementation/manual run:
     - PX4 version/commit or release;
     - simulator backend;
     - startup command;
     - MAVLink endpoint and expected ports;
     - `sitl_agent` golden command;
     - expected progress lines;
     - expected report JSON excerpt;
     - troubleshooting.
   - Keep real hardware warning prominent.

10. Update `README.md`.
    - Quick Start step for PX4 SITL should show the M48 golden command.
    - Mention `--run-report <path>`.
    - Current Status row `Real PX4` should say M48 single-agent PX4 SITL golden path once verified.
    - Known limitations must still say:
      - no real hardware workflow;
      - no multi-agent SITL;
      - PX4 path is experimental and single-agent.

11. Perform manual PX4 SITL verification.
    - Start PX4 SITL using the documented command.
    - Run the golden command against `scenarios/sitl.waypoints.json`.
    - Capture:
      - PX4 version/backend;
      - exact connection string;
      - command output summary;
      - report JSON;
      - pass/fail notes.
    - If local PX4 is unavailable, do not fake this result. Mark M48 as not fully verified and document the blocker.

12. Keep M48 boundary explicit.
    - Do not add multi-agent SITL.
    - Do not claim real hardware readiness.
    - Do not expand to non-waypoint mission families.
    - Do not absorb M49 replay/event-log work; M48 final report is a final summary, not a durable telemetry replay log.

## Testing strategy

### 1. Tests that need no refactoring

These should be implemented with the main M48 changes.

- `crates/swarm-examples/src/sitl_report.rs`
  - success final report serializes to JSON with snake_case final status;
  - failure final report serializes with `error` and optional `abort_result`;
  - JSON roundtrip preserves scenario, agent id, item count, completed count and final status;
  - report writer creates a missing parent directory in a test-owned tempdir.

- `crates/swarm-examples/src/bin/sitl_agent.rs`
  - CLI accepts `--run-report <path>` for `--connection --execute`;
  - CLI rejects missing `--run-report` value;
  - CLI rejects or clearly handles `--run-report` outside the supported mode.

- Fake golden path model tests:
  - upload accepted + lifecycle success + waypoint telemetry complete -> success report;
  - report contains `scenario`, `agent_id`, `mission_item_count`, `completed_count`, `final_status=completed`;
  - mock/dry-run tests remain unchanged.

- Fake negative path model tests:
  - upload rejected -> failure report with error;
  - lifecycle command rejected -> failure report with error;
  - telemetry no-progress timeout -> failure report with abort result;
  - telemetry disconnect -> failure report with abort result;
  - report write failure -> typed error and non-zero path.

### 2. Tests that need light refactoring

These are expected if the existing `sitl_agent` structure blocks direct fake golden path tests.

- Extract a fakeable `SitlExecutionDriver` or equivalent internal trait from `run_connection`.
- Move report construction into pure helpers so tests can call them without spawning PX4.
- Extend current fake telemetry runtime into a full fake golden path runtime:
  - upload result;
  - lifecycle result;
  - telemetry stream;
  - abort result;
  - final report path.
- Add reusable temp scenario/report fixture.
- Add process-level integration tests only if they can stay portable and not require real sockets/PX4.

### 3. Tests that need heavy refactoring

These should be planned/documented but not default CI gates.

- Real PX4 SITL integration test:
  - start/connect to a real local PX4 SITL instance;
  - upload and execute `scenarios/sitl.waypoints.json`;
  - observe telemetry completion;
  - assert generated report.
  - Mark `#[ignore]` or gate by env vars because it needs external simulator state.
- CI-managed PX4 container or simulator orchestration.
- Hardware-in-the-loop tests are explicitly out of scope.

Manual verification gap:

- The actual "tested PX4 setup" cannot be proven by portable unit tests alone because it depends on a local PX4
  simulator, simulator backend and MAVLink ports. M48 should still add maximum fake/unit coverage, but the final
  PX4 golden-path claim must be backed by a documented manual run or explicitly marked blocked.

## Risks and tradeoffs

- **PX4 setup variability.** PX4 version, simulator backend, home position and MAVLink ports can change behavior.
  The docs must record exact tested setup, not generic "works on PX4".
- **Report semantics can drift from CLI errors.** If report construction and error handling duplicate logic,
  success/failure may disagree. Prefer one outcome model feeding both CLI exit and report JSON.
- **Failure-before-context cases.** Some errors happen before `SitlPlan` exists; do not write misleading partial reports.
- **Too much refactor risk.** A broad CLI rewrite would be dangerous. Keep the driver seam narrow and M48-focused.
- **Manual verification can be stale.** The docs should include date/version/backend and avoid broad hardware claims.
- **M48 vs M49 boundary.** Final report is a summary. Durable event logs/replay belong to M49.

## Open questions

- Which exact PX4 version/backend is available locally for the manual golden-path run?
- Should `--run-report` be required for M48 golden command, or optional but recommended?
- Should report JSON be written only for `--connection --execute`, or also for mock/dry-run in a later milestone?
- If PX4 emits final success without final `MISSION_ITEM_REACHED`, should M48 adjust completion semantics or keep
  M47's conservative "all waypoint reached" rule?
- Should `README.md` status move to M48 only after the manual PX4 run succeeds, or should implementation land with
  docs marking the manual verification as pending if PX4 is unavailable?
