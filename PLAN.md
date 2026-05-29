# PLAN.md - M47 Telemetry Loop & TaskStatus Mapping

## Context

M47 продолжает ветку Real SITL / PX4 из `docs_raw/DRONE_A.17.md` после уже
реализованных M43-M46.

Текущее состояние перед M47:

- M43: `sitl_agent --mock`, `--dry-run`, typed SITL errors and portable
  mission-plan contract.
- M44: feature-gated PX4 mission upload через `MavlinkTransport::upload_mission`.
- M45: pre-upload safety validation and `--safety-config`.
- M46: opt-in `--execute` lifecycle:
  - upload;
  - arm/takeoff/start command helpers;
  - `COMMAND_ACK` handling;
  - minimal post-start heartbeat guard;
  - abort/RTL on bounded command/heartbeat failures.

Пробел после M46: `sitl_agent --execute` уже может довести single-agent PX4 SITL
workflow до start mission, но не понимает mission progress. Он не мапит
`MISSION_CURRENT` / waypoint reached telemetry на `TaskStatus`, не печатает
meaningful progress, не отличает completed mission от stalled/disconnected run,
и не формирует final process result по task lifecycle.

Цель M47:

> связать PX4 telemetry/progress с внутренним task lifecycle.

M47 не должен превращаться в M49 observability/replay и не должен заявлять
hardware readiness. Это все еще single-agent PX4 SITL progress loop.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует, поэтому дополнительных входных
данных исследования нет.

`PLAN.md` перед этим раундом отсутствовал, поэтому создан новый плановый
артефакт для M47.

Локальный код, на который опирается план:

- `crates/swarm-comms/src/mavlink.rs`:
  - уже содержит `MavlinkTransport`, `MavlinkVehicleConnection`, mission upload,
    command helpers and M46 lifecycle;
  - уже импортирует `TaskStatus`, но сейчас использует его только для старого
    `mavlink_status_to_task_status` helper;
  - `MavlinkTransport::poll()` сейчас превращает raw MAVLink message в debug
    string `RawMessage`, что недостаточно для typed telemetry progress;
  - private fake connection already scripts MAVLink messages for unit tests.
- `crates/swarm-examples/src/bin/sitl_agent.rs`:
  - уже парсит `--execute`, `--upload-only`, `--no-arm`, `--abort-after`,
    `--timeout`;
  - после successful `upload_and_execute_mission` печатает lifecycle summary и
    завершается, не ожидая mission progress.
- `crates/swarm-examples/src/sitl_plan.rs`:
  - `SitlPlan` already contains ordered `waypoints: Vec<SitlWaypointItem>`,
    and each item has `seq` and `task_id`; это готовая база для
    `mission item seq -> task id`.
- `docs/SITL_SETUP.md` and `README.md`:
  - после M46 честно говорят, что telemetry task mapping еще не реализован;
  - M47 должен обновить этот статус and document new progress loop behavior.

Notion/GitLab:

- `notion_policy=optional`, task id в prompt отсутствует.
- GitLab/MR target в prompt отсутствует.
- Notion/GitLab CLI не нужны; обязательные local protocol docs прочитаны как
  инструкции.

## Affected components

- `crates/swarm-comms/src/mavlink.rs`
  - typed telemetry event enum;
  - MAVLink message -> telemetry event parser;
  - telemetry polling/wait helper over `MavlinkVehicleConnection`;
  - runtime telemetry errors;
  - fake connection tests for telemetry streams.
- `crates/swarm-comms/src/lib.rs`
  - re-export telemetry event/report/error types behind `mavlink-transport`.
- `crates/swarm-examples/src/sitl_progress.rs` (new file)
  - single-agent progress state;
  - `mission seq -> task id` mapping;
  - `task id -> TaskStatus` updates;
  - final mission/run status calculation.
- `crates/swarm-examples/src/lib.rs`
  - export new `sitl_progress` module.
- `crates/swarm-examples/src/bin/sitl_agent.rs`
  - execute mode should continue into telemetry progress loop after M46 start;
  - progress output to stderr/stdout;
  - non-zero exit on disconnect/rejected/aborted/stalled mission.
- `crates/swarm-examples/tests/sitl_agent.rs`
  - CLI validation tests for new telemetry timeout options if they are exposed.
- `README.md`
  - Quick Start and Current Status update for M47.
- `docs/SITL_SETUP.md`
  - operator-facing docs for telemetry loop, final status and limitations.

## Implementation steps

1. `crates/swarm-comms/src/mavlink.rs`: add typed telemetry events.
   - Add feature-gated enum, for example:
     - `MavlinkTelemetryEvent::Heartbeat`;
     - `MavlinkTelemetryEvent::MissionCurrent { seq: u16 }`;
     - `MavlinkTelemetryEvent::WaypointReached { seq: u16 }`;
     - `MavlinkTelemetryEvent::MissionComplete`;
     - `MavlinkTelemetryEvent::MissionRejected { reason: String }`;
     - `MavlinkTelemetryEvent::Disconnected`.
   - Keep this enum limited to progress signals. Do not add replay/event-log
     schema here; that belongs to M49.

2. `crates/swarm-comms/src/mavlink.rs`: parse MAVLink messages into telemetry events.
   - `HEARTBEAT` -> `Heartbeat`.
   - `MISSION_CURRENT` -> `MissionCurrent { seq }`.
   - `MISSION_ITEM_REACHED` -> `WaypointReached { seq }`.
   - Runtime `MISSION_ACK` with non-accepted result -> `MissionRejected`.
   - Optional: treat explicit abort/lifecycle failure from M46 as a failure input
     to progress mapping, but do not invent PX4-specific hidden states.
   - Add parser tests directly against `CommonMessage`.

3. `crates/swarm-comms/src/mavlink.rs`: add telemetry polling helper.
   - Add a method on `MavlinkTransport`, for example:
     `poll_telemetry_event(&mut self) -> Result<Option<MavlinkTelemetryEvent>, MavlinkTelemetryError>`.
   - Under the hood, use the same `try_recv_message()` seam.
   - Ignore unrelated MAVLink messages instead of returning them as errors.
   - Keep read/write transport failures typed.

4. `crates/swarm-comms/src/mavlink.rs`: add bounded telemetry monitor primitive.
   - Add a testable helper over fake connection, for example:
     `wait_next_telemetry_event(timeout)`.
   - It should differentiate:
     - no message yet;
     - heartbeat timeout / disconnected;
     - no progress timeout;
     - mission rejected.
   - Do not perform task mapping inside `swarm-comms`; keep it as protocol/event
     layer.

5. `crates/swarm-examples/src/sitl_progress.rs`: add single-agent progress state.
   - Define `SitlTaskProgress` or equivalent with:
     - ordered seq -> task id map from `SitlPlan.waypoints`;
     - per-task `TaskStatus`;
     - current seq;
     - completed count;
     - total count;
     - last heartbeat timestamp;
     - last progress timestamp;
     - final status.
   - Add update method:
     `apply_event(MavlinkTelemetryEvent, now) -> SitlProgressUpdate`.
   - Mapping rules:
     - `MissionCurrent { seq }`: current seq becomes `seq`; mapped task becomes
       `TaskStatus::InProgress` unless already completed/failed;
     - `WaypointReached { seq }`: mapped task becomes `TaskStatus::Completed`;
     - all waypoint tasks completed -> mission complete/success;
     - `MissionRejected` / aborted mission -> incomplete tasks become
       `TaskStatus::Failed`;
     - `Disconnected` -> incomplete active/in-progress tasks become
       `TaskStatus::Failed`.
   - Unknown/out-of-range seq should be a typed progress error, not panic.

6. `crates/swarm-examples/src/sitl_progress.rs`: add final result model.
   - Add `SitlMissionFinalStatus`:
     - `Completed`;
     - `Failed`;
     - `Disconnected`;
     - `Rejected`;
     - `TimedOutNoProgress`.
   - Add `SitlMissionProgressReport` with:
     - final status;
     - total tasks;
     - completed count;
     - failed count;
     - current task id if any;
     - optional failure reason.
   - This report is for CLI output and tests. M49 can later add a durable event
     log/replay schema.

7. `crates/swarm-examples/src/bin/sitl_agent.rs`: integrate progress loop in `--execute`.
   - Keep M46 order:
     safety validation -> mission upload -> arm/takeoff/start -> post-start
     heartbeat guard.
   - After M46 lifecycle succeeds, enter telemetry loop instead of immediately
     exiting success.
   - Loop behavior:
     - update progress on telemetry event;
     - print concise human-readable progress lines;
     - exit `0` only when all mission waypoint tasks completed;
     - on mission rejected / disconnected / no-progress timeout, send abort when
       appropriate and exit non-zero with clear error.

8. `crates/swarm-examples/src/bin/sitl_agent.rs`: add or reuse timeout options.
   - Reuse current `--timeout <seconds>` only for command ack/upload if that is
     clearer, or split telemetry timeouts explicitly:
     - `--telemetry-timeout <seconds>` for heartbeat/disconnect;
     - `--no-progress-timeout <seconds>` for stuck mission progress.
   - Preferred plan: add explicit telemetry options so operator-facing behavior
     is readable and tests can use tiny values.
   - Validate values as positive finite durations with typed `SitlError`.

9. `crates/swarm-examples/src/bin/sitl_agent.rs`: define abort behavior for telemetry failures.
   - Disconnect / heartbeat timeout after active mission -> attempt RTL abort and
     include abort result in error text.
   - No-progress timeout -> attempt RTL abort and include abort result.
   - Mission rejected -> mark incomplete tasks failed; abort only if vehicle may
     still be active and command layer supports it.
   - Do not hide abort failure as success.

10. `crates/swarm-examples/src/bin/sitl_agent.rs`: human-readable progress output.
    - Print stable compact lines, for example:
      - `progress: current seq=1 task_id=wp-1 completed=1/3`;
      - `progress: reached seq=1 task_id=wp-1 completed=2/3`;
      - `mission complete: completed=3 failed=0`;
      - `mission failed: reason=disconnected completed=1 failed=2 abort_result=...`.
    - Keep output simple. Do not build replay UI or map visualization in M47.

11. `README.md`: update public status.
    - Quick Start should say `--execute` now waits for telemetry progress and
      exits based on mission completion/failure.
    - Current Status row `Real PX4` should move from M46 lifecycle to M47
      telemetry progress mapping.
    - Known limitations should still say no real hardware workflow and no
      multi-agent SITL.

12. `docs/SITL_SETUP.md`: update operator docs.
    - Add telemetry loop section:
      - event types consumed;
      - task status mapping;
      - final exit code semantics;
      - timeout behavior;
      - abort on disconnect/stall.
    - Keep M47 boundary explicit:
      - no multi-agent telemetry merge;
      - no replay UI;
      - no hardware failsafe tuning.

13. Verification commands for implementation.
    - `timeout 300s cargo fmt --all`.
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms --features mavlink-transport mavlink`.
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_progress`.
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent`.
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --features mavlink-transport --test sitl_agent`.
    - `timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings`.

## Testing strategy

### 1. Tests that need no refactoring

These should be implemented with the main M47 code.

- `crates/swarm-comms/src/mavlink.rs`
  - `HEARTBEAT` parses to `MavlinkTelemetryEvent::Heartbeat`.
  - `MISSION_CURRENT(seq=1)` parses to `MissionCurrent { seq: 1 }`.
  - `MISSION_ITEM_REACHED(seq=1)` parses to `WaypointReached { seq: 1 }`.
  - non-accepted runtime `MISSION_ACK` parses to `MissionRejected`.
  - unrelated MAVLink messages are ignored by telemetry polling.

- `crates/swarm-examples/src/sitl_progress.rs`
  - `MissionCurrent { seq }` maps seq to task id and marks the task
    `TaskStatus::InProgress`.
  - `WaypointReached { seq }` marks the mapped task `TaskStatus::Completed`.
  - seq 0/1/2 reached events mark all three tasks completed and final status
    `Completed`.
  - rejected mission marks incomplete tasks `TaskStatus::Failed`.
  - disconnected event marks active/incomplete tasks `TaskStatus::Failed`.
  - out-of-range seq returns typed error.
  - duplicate `WaypointReached` for an already completed seq is idempotent.
  - final report counts completed/failed tasks correctly.

- `crates/swarm-examples/tests/sitl_agent.rs`
  - CLI accepts new telemetry timeout options if added.
  - CLI rejects missing/invalid telemetry timeout values.
  - No-feature build still returns feature-missing after valid parsing and
    safety validation.

### 2. Tests that need light refactoring

These are expected in M47 if the implementation touches the relevant seam.

- Extend existing fake MAVLink connection to script telemetry stream after
  lifecycle start:
  - heartbeat;
  - mission current seq changes;
  - waypoint reached seq events;
  - mission ack rejection;
  - no-message timeout.
- Add a fake clock or injectable `now` to progress state so no-progress and
  disconnect timeout tests do not sleep.
- Add task-status assertion helpers for `SitlTaskProgress`.
- Add a small fake execute+telemetry integration helper so tests can assert:
  - all waypoints reached -> process success model;
  - disconnect -> abort command sent and non-zero model;
  - no-progress timeout -> abort command sent and non-zero model.

### 3. Tests that need heavy refactoring

These should be planned/documented but not required as default CI in M47.

- Real PX4 SITL telemetry integration test:
  - start PX4 SITL;
  - upload and execute mission;
  - observe `MISSION_CURRENT` and waypoint reached telemetry;
  - assert final progress report;
  - keep ignored/manual by default.
- Hardware-in-the-loop telemetry tests are explicitly out of scope.
- Replay/UI tests are out of scope until M49.

## Risks and tradeoffs

- **PX4 telemetry semantics can vary.** Some setups may emit `MISSION_CURRENT`
  without `MISSION_ITEM_REACHED`, or vice versa. M47 should support both but
  treat completion conservatively.
- **False timeout risk.** Too-small no-progress/disconnect timeout values can
  abort a still-running mission. Defaults should be conservative, while tests
  can inject small durations/fake time.
- **Task mapping assumes uploaded mission item order equals `SitlPlan.waypoints`.**
  This is true for current M44/M46 upload path, but future mission item types
  must preserve seq/task id mapping.
- **Abort on telemetry failure is still not a certified failsafe.** It is a SITL
  control behavior, not a hardware safety guarantee.
- **M47 should not absorb M49.** Durable event logs, replay summary and UI belong
  to M49. M47 output should stay human-readable and immediate.
- **M47 should not absorb M48.** Manual real PX4 golden path remains M48; M47
  should be validated primarily with fake telemetry and portable tests.

## Open questions

- Which PX4 signal should define mission complete if `MISSION_ITEM_REACHED` for
  the final item is absent? Default plan: success only when all mapped waypoint
  seqs are reached; otherwise no-progress timeout fails the run.
- Should `MISSION_CURRENT` alone mark previous seq completed? Default plan: no.
  It marks current task in progress; completion requires `MISSION_ITEM_REACHED`
  or an explicit final success signal.
- Should telemetry timeout options be separate from `--timeout`? Preferred plan:
  yes, add explicit `--telemetry-timeout` and `--no-progress-timeout`, while
  leaving `--timeout` for upload/command ack waits.
- Should failed task statuses be persisted into scenario/report files? Default
  plan: no persistence in M47; produce in-memory final report and CLI output.
