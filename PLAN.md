# Context

Планируем milestone **M59 - Live PX4/SIH Failure & Reallocation** по
`docs_raw/DRONE_A.19.md`.

Текущая база уже закрывает предыдущие предпосылки:

- M57: `sitl_supervisor` имеет внутреннюю controller boundary для mock flow:
  `AgentController`, `MockAgentController`, общий supervisor loop,
  `SupervisorMetrics`, fake-controller тесты.
- M58: `sitl_supervisor --connection --execute` умеет sequential local
  PX4/SIH execute для нескольких агентов, пишет common SITL event log и
  `sitl_multi_agent_run_report.v1`.
- После M58 follow-up common multi-agent event log уже несет per-agent
  attribution для mission/progress/task/failure events через
  `multi_agent_*` variants.
- Runtime/mock reallocation уже доказана: lost agent -> release unfinished
  tasks -> reassign recoverable tasks -> event log/metrics.

Главный gap M59: live PX4/SIH supervisor сейчас не является настоящим
reallocation loop. В `crates/swarm-examples/src/sitl_supervisor.rs`
`LiveAgentController::run()` выполняет агента как one-shot workflow и возвращает
только итоговый `LiveAgentRun`. Это достаточно для M58 happy/partial-failure
reporting, но недостаточно для M59, потому что supervisor не может во время
live run:

- получать per-agent heartbeat/progress ticks;
- marked lost after timeout;
- release unfinished tasks lost agent;
- вызвать runtime reallocation;
- построить survivor mission update;
- загрузить updated mission survivor-агенту;
- продолжить tracking survivor progress.

Scope M59 остается **controlled local PX4/SIH only**. Не заявляем hardware
readiness, не делаем distributed onboard reallocation, collision avoidance,
production failover, Gazebo/HIL/real hardware required gate.

По survivor mission update выбираем **Option A - mission replacement**:

- остановить/abort current survivor mission where needed;
- clear/upload replacement mission with deterministic remaining task set;
- start/continue again;
- в naming/events оставить место для будущего supplementary upload.

User-facing флаг для включения поведения: `--reupload-on-failure`. В M59 он
внутренне реализует mission replacement, а не true supplementary append.

# Investigation Context

`INVESTIGATION.md` в workspace отсутствует.

Локально изучены:

- `docs_raw/DRONE_A.19.md` как контекст линейного плана;
- `README.md`, `docs/SITL_SETUP.md`, `docs/REPLAY.md`,
  `docs/HARDWARE_READINESS.md`, `docs/STATUS.md`;
- `crates/swarm-examples/src/bin/sitl_supervisor.rs`;
- `crates/swarm-examples/src/sitl_supervisor.rs`;
- `crates/swarm-examples/src/sitl_report.rs`;
- `crates/swarm-examples/src/sitl_observability.rs`;
- runtime reallocation path в `crates/swarm-runtime/src/coordinator.rs` и
  `crates/swarm-runtime/src/node.rs`;
- MAVLink upload/execute API в `crates/swarm-comms/src/mavlink.rs`.

Важные текущие факты:

- `SupervisorMetrics` сейчас содержит только
  `heartbeat_count`, `completed_task_count`, `lost_agent_count`,
  `reassignment_count`, `tasks_recovered`, `reallocation_latency_ticks`.
  M59 нужно добавить live-specific metrics:
  `released_tasks`, `reassigned_tasks`, `survivor_mission_updates`,
  `final_completed_after_reallocation`.
- `SitlMultiAgentRunReport` сейчас не содержит reallocation section. Нужно
  добавить additive/defaulted fields, чтобы старые report JSON оставались
  читаемыми where practical.
- `SitlEvent` уже содержит `agent_lost`, `task_released`,
  `task_reassigned`, `reallocation_completed`. Для mission replacement нужны
  новые события уровня survivor update, лучше нейтральные:
  `survivor_mission_update_started` /
  `survivor_mission_update_completed`, с `policy: "mission_replacement"`.
- Existing `MavlinkTransport` уже имеет `upload_mission`,
  `execute_uploaded_mission`, `upload_and_execute_mission`, `abort_mission`;
  mission replacement можно строить без нового MAVLink protocol layer.
- Автоматические тесты должны быть portable и не зависеть от локального PX4.
  Manual PX4/SIH artifact остается отдельным controlled/local шагом.

# Affected Components

- `crates/swarm-examples/src/bin/sitl_supervisor.rs`
  - CLI flags and validation for `--reupload-on-failure`;
  - optional controlled failure injection flags for local/fake/manual workflow;
  - usage text and negative CLI tests.
- `crates/swarm-examples/src/sitl_supervisor.rs`
  - live supervisor state machine;
  - live controller boundary;
  - runtime reallocation integration for live flow;
  - mission replacement planning;
  - fake live controller tests;
  - PX4 live controller mission update implementation behind
    `mavlink-transport`.
- `crates/swarm-examples/src/sitl_report.rs`
  - additive report fields for reallocation metrics and survivor mission
    updates;
  - serialization/roundtrip tests.
- `crates/swarm-examples/src/sitl_observability.rs`
  - event schema additions for survivor mission update started/completed;
  - summary counters and formatting;
  - roundtrip/summary tests.
- `crates/swarm-examples/src/sitl_multi_agent.rs`
  - only if mission replacement planning needs helper accessors for
    per-agent task order, task->waypoint mapping, or survivor task subsets.
- `crates/swarm-comms/src/mavlink.rs`
  - only if existing `upload_mission` / `execute_uploaded_mission` /
    `abort_mission` APIs cannot express mission replacement cleanly.
  - Preferred: reuse existing APIs; avoid protocol changes unless forced.
- `crates/swarm-examples/tests/sitl_agent.rs`
  - subprocess CLI validation and portable supervisor tests.
- `crates/swarm-examples/tests/replay_cli.rs`
  - replay summary expectations if output changes.
- `crates/swarm-examples/tests/sitl_docs.rs`
  - docs boundary assertions for M59 wording and non-hardware scope.
- Documentation:
  - `README.md`;
  - `docs/SITL_SETUP.md`;
  - `docs/REPLAY.md`;
  - `docs/STATUS.md`;
  - `docs/HARDWARE_READINESS.md`;
  - optionally `docs/EXTENSION_GUIDE.md` only if touched by final docs index.
- Result artifacts:
  - `results/m59_px4_sih_failure_reallocation_YYYY-MM-DD/README.md`;
  - generated report/log/summary artifacts only after a controlled local run.

# Implementation Steps

1. Re-check current M58 behavior and protect it with baseline tests.
   - Run quick local checks before edits:
     - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_supervisor`
     - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor`
   - Do not run PX4/SIH during baseline unless explicitly doing the manual
     artifact step.

2. Replace the live one-shot controller boundary with a stepwise live boundary.
   - File: `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Keep `AgentController` / `MockAgentController` behavior stable.
   - Refactor `LiveAgentController` from `run() -> LiveAgentRun` into methods
     conceptually equivalent to:
     - `upload_initial(&mut self, waypoints)`;
     - `start(&mut self)`;
     - `poll(&mut self, tick) -> LiveAgentProgress`;
     - `replace_mission(&mut self, MissionReplacementPlan)`;
     - `abort(&mut self, reason)`;
     - `finish_report(&self) -> LiveAgentRun`.
   - If a full trait rewrite is too invasive, add a new internal trait such as
     `LiveSupervisorController` and adapt existing `Px4AgentController` and
     fake controllers to it. Prefer not to keep two divergent live paths long
     term.
   - Preserve M58 public CLI and report behavior for no-failure happy path.

3. Introduce live supervisor state.
   - File: `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Add structs/enums such as:
     - `LiveAgentState { Pending, Uploaded, Started, InProgress, Completed, Lost, Failed, Aborted }`;
     - `LiveAgentProgress { agent_id, heartbeat_seen, completed_task_ids, current_seq, final_status }`;
     - `LiveSupervisorState`;
     - `MissionUpdatePolicy::MissionReplacement`;
     - `MissionReplacementPlan`.
   - Track per-agent:
     - last heartbeat/progress tick;
     - completed task ids;
     - current assigned unfinished task ids;
     - whether agent is eligible for survivor mission update;
     - final status/error.

4. Factor runtime reallocation handling into reusable helpers.
   - File: `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Current mock path already maps `CoordinatorOutput` into:
     - `agent_lost`;
     - `task_released`;
     - `task_reassigned`;
     - `reallocation_completed`;
     - `SupervisorMetrics`.
   - Extract helper(s) so live path does not duplicate this logic:
     - process runtime output;
     - compute recovered tasks;
     - compute failed -> survivor assignments;
     - record event log;
     - update metrics.
   - Reuse `AgentNode` + `MockMavlinkTransport` as the internal runtime
     coordinator bridge if practical, as mock already does. Alternative:
     use `Coordinator` directly and call allocator explicitly, but avoid
     changing runtime semantics.

5. Implement controlled lost-agent detection in live loop.
   - File: `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Trigger lost agent from:
     - heartbeat timeout;
     - no-progress timeout;
     - controller disconnect/error;
     - controlled fake/controller failure injection.
   - Stop polling/updating the lost controller after it is marked lost.
   - Do not let completed tasks from lost agent be released again.
   - Only release unfinished tasks assigned to lost agent.

6. Implement mission replacement planning.
   - File: `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Optional helper module if the function grows:
     `crates/swarm-examples/src/sitl_mission_update.rs`.
   - Inputs:
     - manifest task order and task->waypoint mapping;
     - runtime reassignment output;
     - per-agent completed task ids;
     - target survivor id.
   - Output:
     - target survivor id;
     - recovered task ids;
     - replacement task ids;
     - replacement waypoints with deterministic seq numbering from `0..n`;
     - policy string `mission_replacement`.
   - Deterministic ordering:
     - keep survivor's existing unfinished assigned tasks first, in manifest
       order;
     - append recovered tasks in manifest order;
     - deduplicate task ids;
     - exclude completed tasks.
   - Invariants:
     - no duplicate task ids in replacement plan;
     - no task assigned to lost agent remains assigned to lost agent;
     - recovered task ids are assigned to target survivor in runtime registry;
     - replacement plan can reconstruct `(agent_id, seq) -> task_id`.

7. Apply mission replacement to fake live controllers.
   - File: `crates/swarm-examples/src/sitl_supervisor.rs` tests.
   - Extend `FakeLiveAgentController` so tests can assert:
     - failure before start;
     - failure during progress;
     - failure after completing one task;
     - survivor received exactly one replacement mission update;
     - recovered task appears in survivor update;
     - completed task is not reuploaded.
   - This is the main acceptance path for automated M59 coverage.

8. Apply mission replacement to `Px4AgentController`.
   - File: `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Reuse existing `MavlinkTransport` API:
     - `abort_mission` where needed before replacement;
     - `upload_mission` with `clear_existing: true`;
     - `execute_uploaded_mission` to restart/continue replacement mission.
   - Preserve safety and hardware-candidate gates before any upload.
   - If telemetry/progress APIs are not granular enough, add a small observer or
     progress adapter rather than spawning `sitl_agent`.
   - If full live polling is too large, implement fake-live M59 first and leave
     PX4 replacement behind an explicit typed `FeatureMissing`/`unsupported`
     error only if documented. This should be treated as incomplete for M59
     unless accepted explicitly.

9. Add CLI flags and validation.
   - File: `crates/swarm-examples/src/bin/sitl_supervisor.rs`.
   - Add:
     - `--reupload-on-failure` valid only with `--connection --execute`;
     - optional controlled local/fake injection flags, for example
       `--simulate-agent-disconnect <agent_id>` and
       `--simulate-disconnect-after-ticks <N>`, valid only with
       `--connection --execute` and documented as controlled local debug.
   - Keep existing `--fail-agent` mock-only unless intentionally generalized.
   - Add negative subprocess tests for invalid combinations.
   - Add clear usage text.

10. Extend metrics and final report.
    - Files:
      - `crates/swarm-examples/src/sitl_supervisor.rs`;
      - `crates/swarm-examples/src/sitl_report.rs`.
    - Add metrics:
      - `released_tasks`;
      - `reassigned_tasks`;
      - `reassignment_count`;
      - `reallocation_latency_ticks`;
      - `tasks_recovered`;
      - `survivor_mission_updates`;
      - `final_completed_after_reallocation`.
    - Report JSON should include a `reallocation` section or additive fields.
    - Prefer `#[serde(default)]` for newly added fields where needed so older
      stored reports remain readable by tests/tools where practical.
    - Update `known_limitations` from M58 wording:
      - no longer say live failed-agent reallocation is only mock-covered after
        M59;
      - still say controlled local PX4/SIH only, no hardware readiness.

11. Extend event log schema and replay summary.
    - File: `crates/swarm-examples/src/sitl_observability.rs`.
    - Keep existing events:
      - `agent_lost`;
      - `task_released`;
      - `task_reassigned`;
      - `reallocation_completed`.
    - Add neutral mission update events:
      - `survivor_mission_update_started { step, agent_id, policy, task_ids }`;
      - `survivor_mission_update_completed { step, agent_id, policy, task_ids, mission_item_count }`.
    - Do not name these `supplementary_*` in M59 if implementation is mission
      replacement.
    - Update `format_sitl_summary` with:
      - `survivor_mission_updates`;
      - recovered task count remains visible.
    - Add roundtrip/summary tests.

12. Preserve no-failure M58 behavior.
    - Existing M58 happy-path fake live test should still pass.
    - Existing live report with no failure should still have:
      - `overall_status = completed`;
      - `failed_agents = 0`;
      - no reallocation events;
      - no survivor mission update events.
    - Existing dry-run/mock behavior should remain compatible.

13. Add controlled local PX4/SIH artifact path.
    - Directory:
      `results/m59_px4_sih_failure_reallocation_YYYY-MM-DD/`.
    - Include:
      - `README.md`;
      - exact commands;
      - PX4 source path/version/commit if available;
      - endpoints/system ids;
      - whether failure was process-kill, endpoint close, or controlled
        disconnect flag;
      - run report;
      - event log;
      - replay summary;
      - limitations.
    - If the manual PX4/SIH run would exceed the current execution budget or
      PX4 is not already available/running, do not fake the artifact. Document
      the skipped manual artifact and leave M59 incomplete until captured.

14. Update documentation.
    - `README.md`:
      - add M59 milestone row/status;
      - add short command for controlled local failure/reallocation;
      - update limitations: still no hardware readiness.
    - `docs/SITL_SETUP.md`:
      - add M59 section after M58;
      - document `--reupload-on-failure`;
      - document failure injection/manual process-kill workflow;
      - troubleshooting for timeout/reupload/duplicate ownership.
    - `docs/REPLAY.md`:
      - add mission update events;
      - add expected replay summary lines for M59.
    - `docs/STATUS.md`:
      - update M59 status after implementation;
      - keep live PX4/SIH scope precise.
    - `docs/HARDWARE_READINESS.md`:
      - state that M59 is controlled local SITL/SIH reallocation, not hardware
        readiness.
    - `crates/swarm-examples/tests/sitl_docs.rs`:
      - assert key docs phrases and event names.

15. Final verification and commit.
    - For Rust changes run:
      - `timeout 300s cargo fmt --all`;
      - `timeout 300s /home/formi/.local/bin/runlim cargo clippy --workspace --all-targets --all-features -- -D warnings`.
    - Run targeted tests listed below with:
      `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test ...`.
    - Run `git diff --check`.
    - Ensure no `*.proptest-regressions`.
    - Commit code/docs/results together when M59 is complete, or commit plan
      separately if only planning.

# Testing Strategy

Все автоматические тесты должны быть portable: no local PX4, no `$HOME`, no
absolute external paths, no existing simulator process, no network endpoint.
Manual PX4/SIH verification is allowed only as a documented non-CI artifact.

## Category 1 - tests that need no refactoring

Эти тесты уже существуют или требуют только ожидания, что они остаются green.
Они должны запускаться вместе с реализацией как regression guard:

1. Runtime reallocation:
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-runtime reallocation`
   - Covers `reallocation_recovers_failed_agent_tasks_by_survivor`,
     unassignable released task behavior, duplicate ownership invariants.
2. Existing mock supervisor failure/reallocation:
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_supervisor`
   - Must keep `mock_supervisor_returns_metrics_after_reallocation` and
     `fake_supervisor_boundary_reallocates_after_progress_loss` green.
3. Existing subprocess supervisor tests:
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor`
   - Must keep dry-run/mock/connection validation behavior green.
4. Existing replay event roundtrip:
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_observability`
5. Existing docs boundary:
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`

## Category 2 - tests that need light refactoring

Эти тесты должны быть добавлены в M59 implementation.

1. Fake live controller: lost before start.
   - One agent is marked lost before `start`.
   - Its unfinished task is released.
   - Runtime assigns it to survivor.
   - Survivor receives one mission replacement update.
   - Final report shows `lost_agents=1`, `reassignment_count=1`,
     `survivor_mission_updates=1`.

2. Fake live controller: lost during progress.
   - Failed agent emits heartbeat/progress for N ticks, then disconnects.
   - Timeout marks it lost.
   - Completed tasks stay completed.
   - Only unfinished tasks are released/reassigned.
   - Event log contains `agent_lost`, `task_released`, `task_reassigned`,
     `reallocation_completed`, `survivor_mission_update_started`,
     `survivor_mission_update_completed`.

3. Fake live controller: lost after completing one task.
   - Lost agent has at least two tasks.
   - Completed task is not included in replacement mission.
   - Remaining task is recovered by survivor.
   - Replacement mission has deterministic seq numbers and no duplicate task ids.

4. Mission replacement planner unit tests.
   - Happy path: survivor unfinished tasks first, recovered tasks appended.
   - Negative path: recovered task not assigned to target survivor is rejected or
     skipped with typed error.
   - Edge case: empty recovered task list produces no mission update.
   - Edge case: duplicate recovered task id is deduplicated.
   - Edge case: completed task is excluded.

5. Report aggregation tests.
   - `SitlMultiAgentRunReport` serializes reallocation section.
   - Roundtrip preserves recovered task ids and mission update count.
   - Old/no-reallocation report path remains valid with default fields where
     compatibility is implemented.

6. Replay summary tests.
   - Synthetic live-style M59 log reports:
     - `agent_lost=1`;
     - `task_released>=1`;
     - `task_reassigned>=1`;
     - `reallocation_completed=1`;
     - `tasks_recovered>=1`;
     - `survivor_mission_updates=1`.

7. CLI validation tests.
   - `--reupload-on-failure` rejected outside `--connection --execute`.
   - controlled disconnect flags rejected outside live execute.
   - `--simulate-agent-disconnect` unknown agent rejected before upload.
   - `--reupload-on-failure` without a recoverable survivor produces typed
     error in fake/unit path.

8. No-failure regression tests.
   - M58 fake live happy path still emits no reallocation/update events.
   - Existing report/status remains `completed`.

## Category 3 - tests that need heavy refactoring

Эти тесты полезны, но не должны блокировать portable M59 implementation unless
explicitly chosen.

1. Ignored/manual two-PX4/SIH integration test.
   - Requires local PX4 checkout, build, two running SIH instances, ports,
     process control.
   - Mark ignored or env-gated.
   - Must not run in default CI.

2. Process-control harness for deterministic PX4 kill/restart.
   - Starts two PX4 SIH instances.
   - Kills or disconnects one at a deterministic phase.
   - Collects logs and artifacts.
   - This is expensive and machine-specific; use only for manual artifact.

3. Property test for arbitrary failure timing.
   - Generates failure tick, completed task prefix, survivor assignment.
   - Asserts no duplicate ownership and no completed task reupload.
   - Likely needs more testability around live state machine and replacement
     planner before it is cheap.

4. Mixed fake-real integration.
   - One fake failing controller plus one real survivor PX4 controller.
   - Useful intermediate step, but more complex than pure fake tests and less
     representative than full two-PX4/SIH manual artifact.

# Required Runs, Builds, Installs, And Artifacts

- Default automated implementation checks must stay under `timeout 300s` per
  command and use `/home/formi/.local/bin/runlim` for `cargo test` / `cargo run`
  as required by workflow.
- No installation should be required for portable tests.
- Manual local PX4/SIH artifact may require existing PX4 checkout/build and
  running simulator processes. If setup/build/startup would be long, document it
  in `results/m59.../README.md` or docs and do not hide it as an automated test.
- If manual artifact cannot be captured in the implementation environment, M59
  should be reported as code-complete but artifact-incomplete, not as fully
  done.

# Risks And Tradeoffs

- **Live controller refactor risk:** changing `LiveAgentController` from
  one-shot to stepwise may regress M58 happy path. Mitigation: preserve
  no-failure fake live tests and subprocess CLI tests.
- **Mission replacement disruptiveness:** Option A clears/replaces survivor
  mission and may interrupt already-running PX4 state. Mitigation: document as
  controlled local SIH behavior, emit explicit update events, and keep
  supplementary upload as future work.
- **Duplicate ownership / duplicate upload risk:** recovered tasks must not
  duplicate survivor's existing unfinished tasks or completed tasks. Mitigation:
  mission replacement planner unit tests and runtime ownership assertions.
- **Report/schema compatibility:** adding report fields can break strict JSON
  consumers. Mitigation: additive/defaulted fields where practical and
  serialization tests.
- **Event naming risk:** names like `supplementary_upload_*` would be misleading
  for mission replacement. Mitigation: use neutral
  `survivor_mission_update_*` with `policy: "mission_replacement"`.
- **Manual artifact flakiness:** process kill, endpoint close, or PX4 SIH timing
  can be flaky. Mitigation: portable fake-live tests are primary automated
  proof; manual artifact is documented separately with exact commands and logs.
- **Safety semantics:** reuploading recovered tasks changes live mission after
  initial safety validation. Mitigation: validate replacement plan task subset
  with the same `SitlSafetyGate` before upload.
- **Timeout semantics:** heartbeat/no-progress thresholds that are good for fake
  tests may be too small for PX4/SIH. Mitigation: expose CLI durations and
  document recommended controlled-local values.

# Open Questions

1. Should `--reupload-on-failure` be required for all M59 live reallocation, or
   should reallocation be default-on after M59? Recommendation: require explicit
   flag first for safety/debuggability.
2. Should M59 keep report schema version `sitl_multi_agent_run_report.v1` with
   additive/defaulted fields, or bump to v2? Recommendation: additive/defaulted
   fields unless reviewer requires v2.
3. Which controlled manual artifact is feasible in the implementation
   environment:
   - pure `--simulate-agent-disconnect`;
   - endpoint close;
   - process kill of one PX4 SIH instance?
4. Should replacement mission abort survivor first unconditionally, or only when
   survivor is already executing? Recommendation: explicit abort/clear/upload/
   execute for M59 because it is easiest to reason about.
5. Do we need a first-class `LiveSupervisorMetrics` separate from
   `SupervisorMetrics`, or can `SupervisorMetrics` be extended? Recommendation:
   extend carefully if names remain accurate; split if live-only fields make the
   mock summary confusing.
