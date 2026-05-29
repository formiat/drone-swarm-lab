# PLAN.md - M46 Flight Sequence: arm / takeoff / execute / abort

## Context

M46 продолжает выбранную ветку Real SITL / PX4 из
`docs_raw/DRONE_A.17.md`. Текущий статус перед M46:

- M43 уже дал `sitl_agent --mock`, `--dry-run`, typed SITL errors and portable
  dry-run contract.
- M44 уже добавил feature-gated PX4 mission upload через
  `MavlinkTransport::upload_mission(...)` и MAVLink mission handshake.
- M45 уже добавил pre-upload safety validation, `--safety-config`, typed safety
  errors and strict JSON parsing.

Сейчас `sitl_agent --connection ...` делает только безопасный upload mission в
PX4 SITL. Он не arm-ит vehicle, не делает takeoff, не запускает mission и не
имеет управляемого abort path. M46 должен добавить controlled lifecycle после
успешного upload, сохранив текущий upload-only режим как безопасный default.

Целевая семантика M46:

- upload failed -> не отправлять arm/takeoff/start commands;
- arm failed -> завершиться с понятной ошибкой, без silent success;
- takeoff/start failed -> выполнить abort command и завершиться non-zero;
- telemetry/command ack timeout -> выполнить abort там, где vehicle уже мог
  перейти в активное состояние;
- `--upload-only` остается default-like безопасным путем;
- `--execute` явно включает arm/takeoff/start lifecycle.

Важно: M46 не должен заявлять готовность к real hardware. Это все еще
экспериментальный PX4 SITL workflow.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует, поэтому дополнительных результатов
исследования нет.

Локальный код, на который опирается план:

- `crates/swarm-examples/src/bin/sitl_agent.rs`:
  - сейчас парсит `--mock`, `--dry-run`, `--connection`, `--scenario`,
    `--agent-id`, `--safety-config`;
  - для `--connection` сначала валидирует connection string и safety config,
    затем вызывает `MavlinkTransport::upload_mission(...)`;
  - usage string пока не знает lifecycle options.
- `crates/swarm-examples/src/sitl_plan.rs`:
  - содержит `SitlMode`, `SitlPlan`, `SitlError`, connection validation and
    dry-run formatting;
  - lifecycle-specific CLI/config types пока отсутствуют.
- `crates/swarm-examples/src/sitl_safety.rs`:
  - валидирует mission до upload;
  - M46 должен вызываться только после успешной M45 validation.
- `crates/swarm-comms/src/mavlink.rs`:
  - содержит `MissionUploadOptions`, `MissionUploadReport`,
    `MavlinkMissionError`, `MavlinkTransport::upload_mission(...)`;
  - имеет private `MavlinkMissionConnection` seam and fake connection tests for
    mission upload;
  - уже есть `recv_matching(...)`, который можно переиспользовать для
    `COMMAND_ACK`;
  - command helpers and lifecycle orchestration пока отсутствуют.
- `docs/SITL_SETUP.md` and `README.md`:
  - описывают current PX4 SITL mode как upload-only;
  - должны быть обновлены, чтобы явно показать `--upload-only`, `--execute`,
    `--no-arm`, `--abort-after`, `--timeout` and failure behavior.

Notion/GitLab:

- `notion_policy=optional`, в prompt нет Notion task id.
- GitLab/MR target в prompt нет.
- Поэтому Notion/GitLab CLI читать не нужно; обязательные local protocol docs
  прочитаны как инструкции.

## Affected components

- `crates/swarm-comms/src/mavlink.rs`
  - command message builders;
  - command ack waiting;
  - command/lifecycle errors;
  - lifecycle orchestration over the same fake-able MAVLink connection seam.
- `crates/swarm-comms/src/lib.rs`
  - re-export новых public types/functions under `mavlink-transport` feature.
- `crates/swarm-examples/src/bin/sitl_agent.rs`
  - lifecycle option parsing;
  - upload-only vs execute branching;
  - mapping lifecycle errors into `SitlError::ConnectionFailed` or a more
    specific SITL lifecycle error.
- `crates/swarm-examples/src/sitl_plan.rs`
  - shared lifecycle CLI/config structs and typed validation errors, если
    парсинг в бинаре станет слишком большим.
- `crates/swarm-examples/tests/sitl_agent.rs`
  - CLI parsing and validation regression tests.
- `docs/SITL_SETUP.md`
  - main operator-facing SITL lifecycle documentation.
- `README.md`
  - Quick Start and Current Status update for M46.

## Implementation steps

1. `crates/swarm-comms/src/mavlink.rs`: добавить command/lifecycle data model.
   - Ввести `MavlinkCommandError` или расширить существующий typed error набор
     отдельным command/lifecycle enum, не смешивая silent string errors with
     protocol errors.
   - Ввести `CommandOptions` или `MissionLifecycleOptions`:
     `target_system`, `target_component`, `timeout`, optional retry policy,
     takeoff altitude and abort mode.
   - Для takeoff altitude использовать deterministic default:
     `max(first_waypoint.z, 2.5m)` на уровне caller/lifecycle options. Не
     добавлять новый CLI флаг в M46, если без него можно сохранить scope.

2. `crates/swarm-comms/src/mavlink.rs`: добавить pure command helpers.
   - `arm_command(target_system, target_component)`.
   - `disarm_command(target_system, target_component)`.
   - `takeoff_command(target_system, target_component, altitude_m)`.
   - `start_mission_command(target_system, target_component)`.
   - `abort_command(target_system, target_component)`.
   - Основной abort для M46: RTL через `MAV_CMD_NAV_RETURN_TO_LAUNCH`. Если при
     реализации окажется, что PX4 SITL лучше принимает mission stop before RTL,
     добавить это как явно документированный command sequence, но не прятать в
     best-effort без ошибок.

3. `crates/swarm-comms/src/mavlink.rs`: реализовать `wait_command_ack(command, timeout)`.
   - Ждать matching `COMMAND_ACK.command`.
   - Игнорировать unrelated MAVLink messages and unrelated command acks до
     timeout.
   - `MAV_RESULT_ACCEPTED` -> success.
   - Остальные `MAV_RESULT_*` -> typed rejected error with command and result.
   - Timeout -> typed ack timeout error.
   - Переиспользовать существующий `recv_matching(...)`, но не ломать mission
     upload tests.

4. `crates/swarm-comms/src/mavlink.rs`: добавить lifecycle orchestration helper.
   - Возможное API:
     `execute_uploaded_mission(options: MissionLifecycleOptions) ->
     MissionLifecycleReport`.
   - Sequence:
     1. send arm command, wait ack, unless `no_arm=true`;
     2. send takeoff command, wait ack;
     3. send start mission command, wait ack;
     4. if `abort_after` set, wait that duration and send abort;
     5. return report with command statuses.
   - Failure behavior:
     - arm rejected/timeout -> return error, do not abort unless command result
       indicates vehicle may already be active;
     - takeoff rejected/timeout -> send abort, return original failure plus
       abort result if available;
     - start rejected/timeout -> send abort, return original failure plus abort
       result if available;
     - abort failure must be visible in the error/report, not swallowed.

5. `crates/swarm-comms/src/mavlink.rs`: wire lifecycle into `MavlinkTransport`.
   - Add `MavlinkTransport::execute_uploaded_mission(...)` or
     `MavlinkTransport::run_lifecycle(...)`.
   - Keep `upload_mission(...)` unchanged for M44/M45 callers.
   - Re-export public types in `crates/swarm-comms/src/lib.rs` behind
     `mavlink-transport`.

6. `crates/swarm-examples/src/bin/sitl_agent.rs`: add lifecycle CLI options.
   - `--upload-only`: explicit upload-only mode. Should be equivalent to the
     current `--connection` behavior.
   - `--execute`: after upload, run arm/takeoff/start lifecycle.
   - `--no-arm`: only valid with `--execute`; skips arm command for controlled
     SITL experiments.
   - `--abort-after <seconds>`: only valid with `--execute`; schedules abort
     after successful start. Useful for fake tests and bounded manual SITL runs.
   - `--timeout <seconds>`: command ack timeout. Must be positive and finite.
   - Reject conflicting `--upload-only` + `--execute`.
   - Keep default without either flag as upload-only to avoid surprising users
     who currently expect connection mode to upload but not fly.

7. `crates/swarm-examples/src/bin/sitl_agent.rs`: integrate failure behavior.
   - Current order must remain:
     scenario load -> connection string validation -> safety config load ->
     pre-upload safety validation -> build plan -> mission upload.
   - Only after successful upload and only under `--execute`, run lifecycle.
   - Map lifecycle errors to clear CLI stderr and non-zero exit.
   - Ensure upload failure does not emit arm/takeoff/start commands.
   - Ensure takeoff/start failures attempt abort.

8. `crates/swarm-examples/src/sitl_plan.rs`: keep CLI parsing maintainable.
   - If `sitl_agent.rs` parser becomes too large, move lifecycle option structs
     and validation into `sitl_plan.rs`.
   - Add typed `SitlError` variants for invalid lifecycle options:
     conflicting execution mode, missing lifecycle value, invalid timeout,
     invalid abort duration, lifecycle option without `--execute`.

9. `docs/SITL_SETUP.md`: update M46 operator documentation.
   - Mode matrix should say PX4 SITL now supports upload-only and experimental
     execute lifecycle.
   - Add examples:
     - upload-only;
     - execute with default arm/takeoff/start;
     - execute with `--no-arm`;
     - execute with `--abort-after <seconds>`;
     - short timeout for test/debug.
   - Document exact failure behavior and that M47 telemetry/task completion is
     not yet implemented.
   - Keep real hardware warning explicit.

10. `README.md`: актуализировать публичную сводку.
    - Quick Start PX4 SITL section should mention upload-only vs execute.
    - Current Status row `Real PX4` should move from "mission upload only" to
      "experimental upload + controlled single-agent lifecycle; no telemetry
      task mapping yet".
    - Known limitations should still say no real hardware workflow and no
      multi-agent SITL.

11. Verification commands for the implementation round.
    - Always run formatter:
      `timeout 300s cargo fmt --all`.
    - Run targeted no-feature tests:
      `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent`.
    - Run feature-gated command/lifecycle tests:
      `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms --features mavlink-transport mavlink`.
    - Run feature-gated SITL CLI tests:
      `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --features mavlink-transport --test sitl_agent`.
    - Run workspace clippy:
      `timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings`.

## Testing strategy

### 1. Tests that need no refactoring

These should be implemented together with the main M46 code.

- `crates/swarm-comms/src/mavlink.rs`
  - `arm_command()` builds `COMMAND_LONG` with
    `MAV_CMD_COMPONENT_ARM_DISARM` and `param1=1.0`.
  - `disarm_command()` builds `MAV_CMD_COMPONENT_ARM_DISARM` and `param1=0.0`.
  - `takeoff_command(altitude)` builds `MAV_CMD_NAV_TAKEOFF` and puts altitude
    into the expected MAVLink field.
  - `start_mission_command()` builds the chosen mission-start command and uses
    configured target system/component.
  - `abort_command()` builds RTL/abort command and uses configured target
    system/component.
  - `wait_command_ack()` accepts matching `COMMAND_ACK` with
    `MAV_RESULT_ACCEPTED`.
  - `wait_command_ack()` rejects matching `COMMAND_ACK` with non-accepted
    result.
  - `wait_command_ack()` ignores unrelated messages and unrelated command acks.
  - `wait_command_ack()` times out when matching ack never arrives.
  - lifecycle happy path sends arm -> takeoff -> start in order.
  - lifecycle with `no_arm=true` skips arm and still sends takeoff -> start.
  - lifecycle arm failure sends no takeoff/start.
  - lifecycle takeoff failure sends abort.
  - lifecycle start failure sends abort.
  - lifecycle `abort_after` sends abort after successful start.

- `crates/swarm-examples/tests/sitl_agent.rs`
  - CLI accepts `--connection ... --upload-only`.
  - CLI accepts `--connection ... --execute`.
  - CLI rejects `--upload-only --execute`.
  - CLI rejects `--no-arm` without `--execute`.
  - CLI rejects `--abort-after` without `--execute`.
  - CLI rejects missing/invalid `--abort-after` value.
  - CLI rejects missing/invalid `--timeout` value.
  - No-feature build still returns stable `feature missing` after safety passes,
    not lifecycle-specific noise.

### 2. Tests that need light refactoring

These are still expected in M46 if the implementation touches the relevant seam.

- Refactor private `MavlinkMissionConnection` into a more general private
  `MavlinkVehicleConnection` or equivalent so mission upload and command
  lifecycle can share the fake connection in tests.
- Extend existing `FakeMissionConnection` in `crates/swarm-comms/src/mavlink.rs`
  so it can script `COMMAND_ACK` messages and assert sent `COMMAND_LONG`
  messages.
- Add a small lifecycle command assertion helper to avoid brittle duplicate
  pattern matching in every test.
- If CLI execution path needs feature-gated fake integration, add a lightweight
  local fake MAVLink script/helper that returns heartbeat, mission requests,
  mission ack and command acks. It must be self-contained and not require PX4.
- Add a helper for `sitl_agent` CLI option assertions so new lifecycle parsing
  tests do not duplicate scenario fixture setup.

### 3. Tests that need heavy refactoring

These should be planned/documented but not required as default CI in M46.

- Real PX4 SITL lifecycle integration test:
  - start PX4 SITL;
  - upload mission;
  - arm;
  - takeoff/start mission;
  - optionally abort after bounded time;
  - verify PX4 state/mode/ack behavior.
- This test should be `#[ignore]`, manual, or separated from default CI because
  it depends on external simulator processes, ports and machine setup.
- It should become a candidate for M48 Single-Agent PX4 SITL Golden Path, not a
  hard blocker for M46 portable regression coverage.

## Risks and tradeoffs

- **PX4 command semantics may differ from generic MAVLink expectations.**
  `MAV_CMD_MISSION_START` may not be sufficient in every PX4 SITL setup if mode
  switching is required. Keep the first implementation explicit and tested
  through fake acks; document PX4-specific follow-up if live SITL requires
  `SET_MODE`/custom mode handling.
- **Default behavior must stay safe.** Existing `--connection` users should not
  unexpectedly arm/takeoff after upgrading. Therefore default remains
  upload-only unless `--execute` is provided.
- **Abort is not a certified failsafe.** Sending RTL/abort over MAVLink is useful
  in SITL, but does not equal real hardware emergency handling. Docs must keep
  the hardware warning.
- **Error reporting can become too stringly typed.** Prefer typed command errors
  in `swarm-comms` and only format them at the CLI boundary.
- **Command timeout values affect test speed and live reliability.** Tests should
  use millisecond timeouts through fake connections; docs examples can use
  operator-friendly seconds.
- **M46 overlaps with M47 telemetry.** M46 should only wait for command acks and
  bounded abort timing. Task status mapping and mission progress telemetry stay
  in M47.

## Open questions

- Should M46 expose `--takeoff-altitude <meters>` immediately? Default plan: do
  not expose it yet; derive takeoff altitude from the first waypoint with a
  small floor such as `2.5m`, and document this. Add the CLI option later only
  if real PX4 SITL testing shows the derived value is too implicit.
- Should mission start use only `MAV_CMD_MISSION_START`, or should M46 also send
  PX4-specific mode change before mission start? Default plan: start with
  explicit `start_mission_command()` and typed ack handling; add
  `set_auto_mode` only if fake/live testing demonstrates it is necessary.
- Should abort be RTL-only or mission-stop-then-RTL? Default plan: RTL-only for
  M46 to keep the lifecycle deterministic; expand later if PX4 SITL behavior
  requires a two-command abort sequence.
