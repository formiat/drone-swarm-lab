# PLAN.md - M49 SITL Observability & Replay

## Context

Идем по ветке 6 Real SITL / PX4 из `docs_raw/DRONE_A.17.md`.

M43-M48 уже закрыли основной single-agent PX4 SITL foundation:

- `sitl_agent --mock` and `--dry-run`;
- typed SITL plan/safety/connection errors;
- MAVLink mission upload protocol behind `mavlink-transport`;
- pre-upload safety validation;
- opt-in `--execute` lifecycle: upload -> arm/takeoff/start -> post-start heartbeat;
- telemetry progress loop with `MISSION_CURRENT`, `MISSION_ITEM_REACHED`, completion/failure mapping and RTL abort;
- M48 final run report via `--run-report <path>`;
- internal M48 golden-path driver seam for fake tests.

M49 должен сделать SITL behavior inspectable after a run. Сейчас M48 дает финальный
summary report, но не сохраняет последовательность событий: handshake, mission
requests, command lifecycle, telemetry progress, abort/failure. Это затрудняет
разбор PX4/SITL failures: оператор видит итоговый status, но не видит, на каком
MAVLink/SITL шаге поведение изменилось.

Главный результат M49:

```text
sitl_agent --connection ... --execute --replay-log target/sitl/run.sitl-log.json
-> compact JSON event log
-> replay --sitl-summary target/sitl/run.sitl-log.json
-> human-readable SITL progress/failure summary
```

M49 не должен заменять M48 final run report. `--run-report` остается compact final
summary, а новый `--replay-log` становится ordered event trace for debugging.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует.

Перед планированием прочитаны:

- `docs_raw/DRONE_A.17.md`;
- `README.md`;
- `docs/REPLAY.md`;
- `docs/SITL_SETUP.md`;
- `crates/swarm-examples/src/bin/sitl_agent.rs`;
- `crates/swarm-examples/src/bin/replay.rs`;
- `crates/swarm-examples/tests/sitl_agent.rs`;
- `crates/swarm-examples/tests/replay_cli.rs`;
- `crates/swarm-examples/src/sitl_report.rs`;
- `crates/swarm-examples/src/sitl_progress.rs`;
- `crates/swarm-comms/src/mavlink.rs`;
- `crates/swarm-replay/src/event_log.rs`;
- `crates/swarm-replay/src/replay.rs`;
- `crates/swarm-replay/src/lib.rs`;
- обязательные Notion/GitLab protocol docs.

Notion/GitLab:

- `notion_policy=optional`;
- task id / MR target в prompt отсутствуют;
- Notion/GitLab CLI для этого plan round не нужны.

Текущее состояние:

- `swarm-replay` уже имеет общий simulation `EventLog` schema `0.2`, replay state,
  summary, snapshot and ASCII grid.
- Existing replay log events являются tick/simulation-oriented и не описывают
  MAVLink/SITL protocol events.
- `replay` CLI сейчас поддерживает только `--log <path>` plus `--summary`,
  `--tick`, `--follow`.
- `sitl_agent` сейчас принимает `--run-report <path>`, но не имеет
  `--replay-log <path>`.
- M48 production execute path идет через `SitlGoldenPathDriver`, а tests имеют
  fake driver seam. Это хорошая точка для M49 orchestration-level event tests.
- Mission upload protocol events (`MISSION_CLEAR_ALL`, `MISSION_COUNT`,
  `MISSION_REQUEST(_INT)`, `MISSION_ITEM_INT`, `MISSION_ACK`) возникают внутри
  `swarm-comms/src/mavlink.rs`. Если логировать только в `sitl_agent`, часть
  handshake details будет недоступна. Поэтому M49 нужен небольшой
  observability hook/recorder around MAVLink upload/lifecycle internals.

## Affected components

- `crates/swarm-examples/src/sitl_observability.rs` (new)
  - SITL-specific event log schema;
  - event enum with `snake_case` serde representation;
  - ordered event builder/recorder;
  - JSON writer that creates parent directories;
  - compact summary model and formatter.

- `crates/swarm-examples/src/lib.rs`
  - export `sitl_observability` module for tests and CLI.

- `crates/swarm-examples/src/bin/sitl_agent.rs`
  - parse `--replay-log <path>`;
  - validate mode support;
  - wire recorder into mock, upload-only and execute paths;
  - write event log on success and bounded failures;
  - keep `--run-report` independent from `--replay-log`.

- `crates/swarm-comms/src/mavlink.rs`
  - add minimal optional observer/recorder hook for mission upload and lifecycle events;
  - emit protocol-level events:
    - heartbeat seen;
    - mission clear sent;
    - mission count sent;
    - mission item requested;
    - mission item sent;
    - mission ack received;
    - arm/takeoff/start/abort command sent and acknowledged/rejected/timeout.

- `crates/swarm-comms/src/lib.rs`
  - export any new MAVLink observability types if they live in `swarm-comms`.

- `crates/swarm-replay/src/lib.rs` and/or `crates/swarm-examples/src/sitl_observability.rs`
  - choose where summary parsing lives.
  - Preferred: keep SITL log schema in `swarm-examples` unless it is clearly useful
    as a generic replay crate format. If `replay` binary needs it, `swarm-examples`
    can depend on its own library module.

- `crates/swarm-examples/src/bin/replay.rs`
  - add `--sitl-summary <log>` mode;
  - print compact text summary without requiring `--log`;
  - reject conflicting normal replay modes clearly.

- `crates/swarm-examples/tests/sitl_agent.rs`
  - CLI validation and mock/fake event log integration tests.

- `crates/swarm-examples/tests/replay_cli.rs`
  - `replay --sitl-summary` success/failure CLI tests.

- `docs/SITL_SETUP.md`
  - document `--replay-log <path>`;
  - show combined M49 command with `--run-report` and `--replay-log`;
  - explain final report vs replay log boundary.

- `docs/REPLAY.md`
  - add SITL event log schema section;
  - add `replay --sitl-summary <log>` examples and expected output.

- `README.md`
  - update Real PX4 / Replay status wording for M49;
  - add short SITL observability command/example;
  - keep live PX4 verification caveat honest.

## Implementation steps

1. Add SITL event log schema in `crates/swarm-examples/src/sitl_observability.rs`.
   - Define `SITL_EVENT_LOG_SCHEMA_VERSION`, likely `sitl_event_log.v1`.
   - Add `SitlEventLog`:
     - `schema_version`;
     - `run_id`;
     - `scenario_path`;
     - `scenario_name`;
     - `mission`;
     - `profile`;
     - `agent_id`;
     - `connection_string`;
     - `mode`;
     - `events`.
   - Add ordered `SitlEvent` enum with `#[serde(rename_all = "snake_case", tag = "type")]`.
   - Include at minimum:
     - `connection_opened`;
     - `heartbeat_seen`;
     - `mission_clear_sent`;
     - `mission_count_sent`;
     - `mission_item_requested`;
     - `mission_item_sent`;
     - `mission_ack_received`;
     - `command_sent`;
     - `command_ack_received`;
     - `current_seq_changed`;
     - `waypoint_reached`;
     - `task_completed`;
     - `abort_requested`;
     - `disconnected`;
     - `failure`.
   - Use monotonically increasing event index or `step` instead of wall-clock timestamps for
     deterministic tests. If a real timestamp is added later, make it optional and separate.

2. Add writer and summary helpers in `crates/swarm-examples/src/sitl_observability.rs`.
   - `write_sitl_event_log(path: impl AsRef<Path>, log: &SitlEventLog)`.
   - Create parent directory if needed, matching the behavior of M48 report writer.
   - Return typed `SitlEventLogError`, not `anyhow`.
   - `summarize_sitl_event_log(log: &SitlEventLog) -> SitlEventLogSummary`.
   - `format_sitl_summary(summary: &SitlEventLogSummary) -> String`.
   - Summary fields should include:
     - total events;
     - connection opened count;
     - heartbeat seen count;
     - mission item requests/sent;
     - mission ack accepted/rejected;
     - commands sent;
     - command failures/rejections;
     - current seq changes;
     - waypoints reached;
     - tasks completed;
     - abort requested count;
     - disconnect/failure count;
     - final status inferred from terminal event if present.

3. Decide where MAVLink protocol events are produced.
   - Preferred implementation:
     - add a small `MavlinkMissionObserver` trait or callback-style recorder in
       `crates/swarm-comms/src/mavlink.rs`;
     - default no-op observer for existing API;
     - observed variants of upload/lifecycle helpers used by `sitl_agent`;
     - existing public methods remain behavior-compatible.
   - Keep observer types small and string/enum based so `swarm-comms` does not depend on
     `swarm-examples`.
   - If adding public observer types is too much for M49, use an internal callback in
     `sitl_agent` driver seam for high-level events and explicitly document that low-level
     per-message mission handshake visibility is partial. But this should be fallback, not
     preferred path, because M49 explicitly asks for mission clear/count/request/item/ack events.

4. Wire `--replay-log <path>` into `crates/swarm-examples/src/bin/sitl_agent.rs`.
   - Add `CliArgs.replay_log: Option<String>`.
   - Parse missing value with existing `MissingArgument` style.
   - Include flag in usage.
   - Validate supported modes:
     - support `--mock --replay-log` for portable test coverage;
     - support `--connection ... [--upload-only|--execute] --replay-log`;
     - reject `--dry-run --replay-log` unless the implementation intentionally writes
       a dry-run planning log. Recommended: reject dry-run in M49 to keep semantics focused
       on behavior after a run, not plan preview.
   - Make validation messages explicit and testable.

5. Add mock-mode event logging in `crates/swarm-examples/src/bin/sitl_agent.rs`.
   - Emit deterministic events:
     - `connection_opened` with `mode=mock`;
     - one `mission_item_sent` per waypoint or a mock-specific upload event if that is clearer;
     - `task_completed` for mock accepted waypoints only if mock path semantically marks them
       completed; otherwise record `mission_item_sent` and final mock success event.
   - This gives portable `--mock --replay-log` CLI coverage without PX4.

6. Add connection upload-only event logging.
   - Emit `connection_opened` after `MavlinkTransport::new` succeeds.
   - Record upload handshake events from the observer:
     - heartbeat seen;
     - mission clear sent;
     - mission count sent;
     - mission request per seq;
     - mission item sent per seq;
     - final mission ack received.
   - On upload failure, still write log when context exists, ending with `failure`.

7. Add execute event logging through the M48 golden-path driver.
   - Extend `SitlGoldenPathRun` or driver context with a mutable event recorder.
   - Record lifecycle:
     - arm command sent / ack;
     - takeoff command sent / ack;
     - start command sent / ack;
     - post-start heartbeat seen;
     - abort requested and abort result.
   - Record telemetry:
     - heartbeat seen;
     - `current_seq_changed`;
     - waypoint reached;
     - task completed;
     - mission complete/rejected;
     - disconnect/no-progress/failure.
   - On all bounded failures where M48 writes a final report, also write replay log if requested.

8. Ensure event log write timing is robust.
   - The log should be written:
     - on mock success;
     - on upload-only success/failure after plan context exists;
     - on execute success/failure after plan context exists;
     - on report write failure only if log writing itself can still succeed.
   - If log writing fails, return a typed `SitlError::ReplayLogWrite` or similar.
   - Do not invent logs for failures before plan/context is available.

9. Extend `replay` CLI in `crates/swarm-examples/src/bin/replay.rs`.
   - Add `--sitl-summary <log>`.
   - It should not require `--log`.
   - Print compact text:
     - run id / scenario / agent / mode;
     - events total;
     - upload handshake counts;
     - commands summary;
     - telemetry progress summary;
     - abort/failure summary.
   - Reject or clearly define behavior for combinations like `--sitl-summary` with `--tick`,
     `--follow`, or normal `--summary`.

10. Update documentation.
    - `README.md`:
      - mention M49 SITL observability/replay in status row or Real PX4 description;
      - show a short command with both `--run-report` and `--replay-log`;
      - mention `replay --sitl-summary`.
    - `docs/SITL_SETUP.md`:
      - add M49 section after M48 report section;
      - explain final report vs replay log;
      - include example log path and summary command.
    - `docs/REPLAY.md`:
      - add SITL event log schema;
      - list event types and fields;
      - add summary output example.

11. Keep scope boundaries explicit.
    - Do not add interactive UI.
    - Do not add map overlay.
    - Do not add long-term telemetry store.
    - Do not turn SITL event log into a full hardware flight recorder.
    - Do not require live PX4 for default tests.

## Testing strategy

### 1. Tests that need no refactoring

These should be implemented with the main M49 changes.

- `crates/swarm-examples/src/sitl_observability.rs`
  - event log serialization roundtrip;
  - enum values serialize in `snake_case`;
  - writer creates missing parent directory in test-owned tempdir;
  - summary counts mission upload events:
    `mission_count_sent`, `mission_item_requested`, `mission_item_sent`, `mission_ack_received`;
  - summary counts waypoint reached events;
  - summary counts task completed events;
  - summary counts command sent/ack/rejected events;
  - failure event summary test;
  - abort event summary test;
  - malformed/unknown schema behavior if strict schema validation is added.

- `crates/swarm-examples/src/bin/sitl_agent.rs` unit tests
  - CLI accepts `--mock --replay-log <path>`;
  - CLI accepts `--connection ... --execute --replay-log <path>` before feature gate;
  - CLI rejects missing `--replay-log` value;
  - CLI rejects unsupported `--dry-run --replay-log` if dry-run logging is not implemented;
  - mock run writes expected deterministic events to a tempdir path;
  - fake golden-path success writes connection/upload/lifecycle/telemetry/task-completed events;
  - fake upload rejected writes terminal failure event and skips telemetry events;
  - fake lifecycle failure writes command/abort/failure events;
  - fake telemetry disconnect/no-progress writes disconnect/failure/abort events.

- `crates/swarm-examples/tests/replay_cli.rs`
  - `replay --sitl-summary <log>` prints run/scenario/agent/mode;
  - summary output includes mission upload count;
  - summary output includes waypoint reached count;
  - summary output includes failures/abort count;
  - invalid SITL log path exits non-zero;
  - invalid JSON exits non-zero with clear error;
  - `--sitl-summary` conflicting with `--tick`/`--follow` exits with clear error.

- `docs` sanity tests if the repository already has a docs command checker:
  - documented `--replay-log` and `--sitl-summary` flags appear in README/SITL/REPLAY docs.
  - If no docs test harness exists, do not introduce a broad one just for M49 unless cheap.

### 2. Tests that need light refactoring

These are expected if current seams are too narrow but should stay within M49.

- Add an event recorder fixture shared by `sitl_agent` unit tests:
  - build log from `test_plan()`;
  - push events by type;
  - assert event order and counts.
- Extend M48 `FakeGoldenPathDriver` to script events in addition to outcomes.
- Add `MavlinkMissionObserver` fake in `swarm-comms` tests:
  - assert observer sees heartbeat, mission clear/count/request/item/ack in order;
  - assert command observer sees arm/takeoff/start/abort command events.
- Add replay fixture builders by event type:
  - upload-only success log;
  - execute success log;
  - upload rejected log;
  - lifecycle abort log;
  - telemetry disconnect/no-progress log.
- Refactor `replay` CLI parsing into a small parse function if direct process tests become
  too noisy or slow.

### 3. Tests that need heavy refactoring

These should be documented but not default M49 acceptance criteria.

- Real PX4 SITL integration test that asserts a live run writes all expected event classes.
  Gate it behind `#[ignore]` or explicit env vars because it requires external simulator state.
- End-to-end replay over a real PX4 log captured from manual run.
- Interactive visualization tests. Not in M49 scope because UI/map overlay are explicitly excluded.
- Long-running telemetry store retention tests. Not in M49 scope.

Autotest gap:

- Exact live PX4 event ordering cannot be fully proven by portable unit tests. M49 should still
  cover protocol observer ordering with fake MAVLink connection and full `sitl_agent` orchestration
  with fake driver. Live PX4 verification remains manual/ignored because it depends on local PX4
  simulator, ports and backend timing.

## Risks and tradeoffs

- **Schema overlap with existing `swarm-replay::EventLog`.** Reusing the generic simulation event
  schema would avoid a second format but would force MAVLink protocol events into tick-oriented
  fields. A separate `SitlEventLog` is cleaner for M49, as long as docs clearly explain the two log
  types.
- **Where to place SITL log types.** Putting SITL log schema in `swarm-replay` makes replay CLI
  ownership clearer but may pollute a generic replay crate with PX4-specific concepts. Putting it
  in `swarm-examples` keeps scope narrow. Decide during implementation based on dependency shape.
- **Protocol event completeness.** If observer hooks are only in `sitl_agent`, mission request/item
  details can be incomplete. Prefer observer hooks inside `swarm-comms` upload/lifecycle internals.
- **Log write failures can mask original mission failures.** If mission fails and log write also
  fails, error reporting must preserve enough context. Consider including original failure in the
  log-write error message or writing log before returning mission failure.
- **Event volume.** M49 log should be compact. Log semantic events, not every raw MAVLink packet.
- **Backward compatibility.** `replay --log` existing behavior must remain unchanged.
- **Feature gates.** Real MAVLink protocol observer tests likely need `mavlink-transport`; mock and
  summary tests should remain portable without PX4.
- **Manual/live gap.** Without live PX4 run, M49 can prove portable behavior and fake protocol
  ordering, but not all PX4-specific timing quirks.

## Open questions

- Should SITL event log schema live in `swarm-examples` or `swarm-replay`?
- Should `--replay-log` be supported for `--dry-run`, or rejected because dry-run has no runtime
  behavior after a run?
- Should `--replay-log` write on upload-only mode as well as execute mode? Recommended: yes,
  because mission upload handshake is one of the required M49 event groups.
- Should `--run-report` and `--replay-log` share a `run_id`, and where should that `run_id` come
  from? Recommended: deterministic string based on scenario/agent/mode unless a user-provided run id
  is added later.
- Should log events include wall-clock timestamps? Recommended: no for M49 portable tests; use
  deterministic sequence numbers and add timestamps later only behind a stable clock abstraction.
- How strict should `replay --sitl-summary` be with schema version mismatches?
- Should command events record MAVLink command enum debug names, numeric ids, or both?
