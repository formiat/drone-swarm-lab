# План реализации M90/M92/M95/M97

## Context

Задача этого раунда - не реализовывать код, а собрать перечисленные в inbox
доработки в один детальный план реализации. Референсный документ:
`docs_raw/DRONE_B.27.md`. В текущем HEAD уже есть часть фундамента M90-M97,
поэтому план ниже исходит из фактического состояния кода, а не из утверждений
документов:

- `MavlinkPlanExecutor`, `AckProvider`, `MockAckProvider`,
  `ScriptedAckProvider`, `MavlinkPlanExecutionReport` уже существуют в
  `crates/swarm-comms/src/mavlink_executor.rs:21`,
  `crates/swarm-comms/src/mavlink_executor.rs:92`,
  `crates/swarm-comms/src/mavlink_executor.rs:117`,
  `crates/swarm-comms/src/mavlink_executor.rs:198`.
- `sitl_agent --execute` пока не использует этот executor boundary: путь
  `crates/swarm-examples/src/sitl_agent_runtime/connection.rs:155` все еще
  вызывает `MavlinkGoldenPathDriver`, а тот на
  `crates/swarm-examples/src/sitl_agent_runtime/connection.rs:301` уходит в
  `MavlinkTransport::upload_and_execute_mission_observed`.
- Реальный `MavlinkTransport` уже имеет низкоуровневые upload/execute методы в
  `crates/swarm-comms/src/mavlink/transport.rs:103`, `:146`, `:162`, но его
  generic `Transport::send` намеренно не является MAVLink command pipe
  (`crates/swarm-comms/src/mavlink/transport.rs:216`).
- Для ACK reuse уже есть helpers: `send_command_and_wait_observed` и
  `wait_command_ack` в `crates/swarm-comms/src/mavlink/commands.rs:86`,
  `:155`; mission upload handshake уже есть в
  `crates/swarm-comms/src/mavlink/mission_upload.rs:403`.
- `drone_agent` пока является thin binary с raw/self heartbeat:
  `crates/swarm-examples/src/bin/drone_agent.rs:72`; typed protocol уже есть в
  `crates/swarm-comms/src/swarm_protocol.rs:442` и
  `DuplicateSuppressor` в `crates/swarm-comms/src/swarm_protocol.rs:492`.
- Satcom profile рассинхронизирован: комментарий обещает 8% loss, код ставит
  5% в `crates/swarm-comms/src/drone_link.rs:167`, `:174`; пользовательская
  документация тоже говорит 8% в `docs/DRONE_LINK.md:52`.
- `UrbanOperationalEvidence.execution_report` существует, но runtime builder
  выставляет `None` в `crates/swarm-sim/src/urban/operational_evidence.rs:131`,
  `:143`; writer берет только replay logs в
  `crates/swarm-examples/src/strategy_comparison_runtime/urban_artifacts.rs:229`.
- `HardwareReadinessStatus::ExecuteValidatedLocally` сейчас используется для
  primitive mission даже при `MockAckProvider`:
  `crates/swarm-examples/src/hardware_entry.rs:57`, `:134`, `:193`.
- `artifact_validator` не имеет `--mode execute`:
  `crates/swarm-examples/src/artifact_validator.rs:145` и
  `crates/swarm-examples/src/bin/artifact_validator.rs:120`.

Предлагаемый контракт по timeout semantics: на уровне `MavlinkPlanExecutor`
ACK timeout после исчерпания retry budget считается управляемым abort
execution boundary, то есть `MavlinkExecutionOutcome::Aborted`, а не
`Failed`. Низкоуровневые ошибки транспорта, невозможность отправить сообщение,
ошибка соединения или malformed MAVLink response остаются `Failed` в
transport-backed adapter. Это согласует ожидание B.27 с текущим
`MissionExecuteLifecycleState::Aborted` при timeout и отделяет "FC не ответил
на ожидаемый ACK" от "transport сломан".

## Investigation context

`INVESTIGATION.md` в workspace отсутствует, поэтому отдельных входных
исследовательских выводов нет. Текущий `PLAN.md` до этого раунда тоже
отсутствовал; файл создается с нуля.

Notion protocol и GitLab protocol прочитаны. Inbox не содержит конкретной
Notion task или GitLab MR, поэтому Notion/GitLab чтение не требуется
(`notion_policy: optional`) и удаленные SSH/HTTP обращения не выполняются.

## Affected components

- `crates/swarm-comms/src/mavlink_executor.rs` - executor contract, timeout
  semantics, transport-backed ACK provider boundary, FC config provider
  contracts.
- `crates/swarm-comms/src/mavlink/commands.rs` - reuse/visibility of command
  ACK helpers for real `AckProvider`.
- `crates/swarm-comms/src/mavlink/mission_upload.rs` - reuse/extension of
  mission upload handshake for executor upload phase and fence upload.
- `crates/swarm-comms/src/mavlink/transport.rs` - factory methods that expose
  transport-backed executor/config providers without abusing `Transport::send`.
- `crates/swarm-comms/src/mavlink_parameters.rs`,
  `crates/swarm-comms/src/mavlink_geofence.rs` - typed param/fence plans and
  result/error shapes.
- `crates/swarm-examples/src/sitl_agent_runtime/connection.rs` - bridge from
  `sitl_agent --execute` to `MavlinkCommonPlan -> MavlinkPlanExecutor ->
  MavlinkPlanExecutionReport`.
- `crates/swarm-examples/src/sitl_plan.rs` - reusable helper for compiling
  `SitlPlan` into `MavlinkCommonPlan` without going through dry-run-only
  artifact code.
- `crates/swarm-examples/src/artifact_validator.rs` and
  `crates/swarm-examples/src/bin/artifact_validator.rs` - new
  `--mode execute`, stricter M97 validation.
- `crates/swarm-comms/src/drone_link.rs` and `docs/DRONE_LINK.md` - satcom
  profile contract.
- `crates/swarm-examples/src/bin/drone_agent.rs` plus a new testable runtime
  module under `crates/swarm-examples/src/drone_agent_runtime/` - typed M92
  protocol loop.
- `crates/swarm-comms/src/swarm_protocol.rs` - existing envelope and duplicate
  suppressor to reuse in M92.
- `crates/swarm-sim/src/urban/operational_evidence.rs` and
  `crates/swarm-examples/src/strategy_comparison_runtime/urban_artifacts.rs` -
  M95 evidence generated from real scenario runs and enriched with execution
  reports.
- `scenarios/urban.perimeter-patrol.network.json` and
  `scenarios/urban.corridor-inspection.network.json` - target M95 network
  scenarios.
- `crates/swarm-examples/src/sitl_dual_stack_evidence.rs` - distinguish
  mock/local executor evidence from transport-backed execution evidence.
- `crates/swarm-examples/src/hardware_entry.rs` - readiness status split and
  evidence references.
- `docs/STATUS.md`, `docs/ARTIFACT_VALIDATION.md`,
  `docs/HARDWARE_READINESS.md`, `docs/OPERATIONAL_RUNBOOKS.md`, `README.md` -
  user-facing status and command docs.

## Implementation steps

1. Normalize M90 executor timeout contract.

   Files and anchors:
   `crates/swarm-comms/src/mavlink_executor.rs:49`,
   `crates/swarm-comms/src/mavlink_executor.rs:256`,
   `crates/swarm-comms/src/mavlink_executor.rs:295`,
   `crates/swarm-comms/src/mavlink_executor.rs:335`,
   `crates/swarm-comms/src/mavlink_executor.rs:375`,
   tests near `crates/swarm-comms/src/mavlink_executor.rs:738`.

   Material result:
   - All executor-level ACK timeouts after retry budget return
     `MavlinkExecutionOutcome::Aborted { at_step, reason }`.
   - `MavlinkExecutionOutcome::Failed` remains reserved for adapter/transport
     failures outside the logical ACK sequence.
   - Existing test `executor_aborts_on_first_timeout` must assert the exact
     `Aborted` outcome, not only `lifecycle_state == Aborted`.
   - Add one retry-budget test: timeout then accepted -> `Retried`; repeated
     timeout past budget -> `Aborted`.

   Contract sketch:

   ```rust
   fn timeout_outcome(step_index: usize, reason: String) -> MavlinkExecutionOutcome {
       MavlinkExecutionOutcome::Aborted {
           at_step: step_index,
           reason,
       }
   }
   ```

2. Add a transport-backed M90 ACK provider/factory without changing
   `Transport::send` semantics.

   Files and anchors:
   `crates/swarm-comms/src/mavlink_executor.rs:117`,
   `crates/swarm-comms/src/mavlink/transport.rs:81`,
   `crates/swarm-comms/src/mavlink/commands.rs:86`,
   `crates/swarm-comms/src/mavlink/mission_upload.rs:403`,
   `crates/swarm-comms/src/mavlink/types.rs:62`,
   `crates/swarm-comms/src/mavlink/types.rs:101`.

   Material result:
   - Add a real adapter, for example `MavlinkTransportAckProvider<'a>`, behind
     `#[cfg(feature = "mavlink-transport")]`.
   - It implements `AckProvider` by sending real MAVLink commands/upload/start
     through the existing `MavlinkTransport` connection APIs.
   - It maps `COMMAND_ACK`, `MISSION_ACK`, command rejection and timeout to
     `MavlinkExecutionStepResult`.
   - It does not use `MavlinkTransport`'s generic `Transport::send`, because
     that method intentionally rejects raw swarm messages for MAVLink.

   Shape sketch:

   ```rust
   #[cfg(feature = "mavlink-transport")]
   pub struct MavlinkTransportAckProvider<'a> {
       transport: &'a mut MavlinkTransport,
       upload_options: MissionUploadOptions,
       lifecycle_options: MissionLifecycleOptions,
       observer: &'a mut dyn MavlinkMissionObserver,
   }

   impl AckProvider for MavlinkTransportAckProvider<'_> {
       fn ack_prelude_command(&mut self, command: &MavlinkCommonCommand)
           -> MavlinkExecutionStepResult
       {
           // MavlinkCommonCommand -> COMMAND_LONG/COMMAND_INT
           // send_command_and_wait_observed(...)
           // map accepted/rejected/timeout into MavlinkExecutionStepResult
       }

       fn ack_mission_upload(&mut self) -> MavlinkExecutionStepResult {
           // MavlinkCommonPlan.mission_items -> MissionItem list
           // upload_mission_items_with_connection_observed(...)
       }
   }
   ```

   If `AckProvider` needs access to mission items during `ack_mission_upload`,
   prefer a small `ExecutionPhaseProvider`/factory over stuffing plan state into
   global variables. The executor should remain deterministic and testable.

3. Bridge `sitl_agent --execute` to `MavlinkCommonPlan` execution.

   Files and anchors:
   `crates/swarm-examples/src/sitl_agent_runtime/connection.rs:31`,
   `crates/swarm-examples/src/sitl_agent_runtime/connection.rs:155`,
   `crates/swarm-examples/src/sitl_agent_runtime/connection.rs:289`,
   `crates/swarm-examples/src/sitl_plan.rs:896`,
   `crates/swarm-examples/src/sitl_plan.rs:907`,
   `crates/swarm-comms/src/mavlink_common_plan.rs:30`.

   Material result:
   - Extract a reusable helper from dry-run artifact generation, for example
     `compile_sitl_plan_to_mavlink_common_plan(plan, profile)`.
   - In `LifecycleMode::Execute`, compile the selected `SitlPlan` into
     `MavlinkCommonPlan`, construct the transport-backed provider, execute via
     `MavlinkPlanExecutor`, and write the resulting
     `MavlinkPlanExecutionReport` into the run report/artifact.
   - Keep the old golden-path upload/lifecycle code only as a compatibility
     fallback or remove it after tests are migrated.
   - Replay events must still include upload/start/completion/abort phases, but
     their source becomes executor steps rather than a second lifecycle path.

   Expected execution flow:

   ```text
   SitlPlan -> MissionCommandPlan IR
            -> MavlinkCommonPlan
            -> MavlinkPlanExecutor<MavlinkTransportAckProvider>
            -> MavlinkPlanExecutionReport
            -> run report + replay log + optional execute artifact
   ```

4. Implement real M90 FC config provider over MAVLink transport.

   Files and anchors:
   `crates/swarm-comms/src/mavlink_executor.rs:552`,
   `crates/swarm-comms/src/mavlink_geofence.rs`,
   `crates/swarm-comms/src/mavlink_parameters.rs`,
   `crates/swarm-comms/src/mavlink/mission_upload.rs`,
   `crates/swarm-comms/src/mavlink/commands.rs`.

   Material result:
   - Add `MavlinkTransportFcConfigProvider` behind
     `#[cfg(feature = "mavlink-transport")]`.
   - `execute_geofence_upload` must support fence item upload via MAVLink fence
     mission protocol where supported, then `MAV_CMD_DO_FENCE_ENABLE`.
   - `execute_param_snapshot` must send `PARAM_REQUEST_READ` and collect matching
     `PARAM_VALUE` per `FcParamRequirement`.
   - `execute_param_write` must send `PARAM_SET` and require a matching updated
     `PARAM_VALUE` confirmation.
   - Typed errors must distinguish unsupported, timeout, rejected, mismatch and
     transport failure. No `panic!`/`expect` in runtime paths.

   Error shape sketch:

   ```rust
   #[derive(Debug, thiserror::Error)]
   pub enum MavlinkFcConfigTransportError {
       #[error("FC config operation unsupported: {operation}")]
       Unsupported { operation: String },
       #[error("timeout waiting for {expected}")]
       Timeout { expected: String },
       #[error("FC rejected {operation}: {reason}")]
       Rejected { operation: String, reason: String },
       #[error("parameter value mismatch for {param_id}")]
       ParamMismatch { param_id: FcParamId },
       #[error("transport error: {message}")]
       Transport { message: String },
   }
   ```

5. Add M90 execute artifact schema and validator mode.

   Files and anchors:
   `crates/swarm-examples/src/artifact_validator.rs:145`,
   `crates/swarm-examples/src/bin/artifact_validator.rs:120`,
   `crates/swarm-comms/src/mavlink_executor.rs:92`,
   `docs/ARTIFACT_VALIDATION.md`.

   Material result:
   - Add a serializable execute artifact, for example
     `MavlinkExecutionArtifact { schema_version, execution_mode, profile_id,
     plan_id, git_commit, command, execution_report, caveats }`.
   - Add `ArtifactValidationMode::Execute` and CLI parser value
     `--mode execute`.
   - Validator checks:
     - schema version is known;
     - report `plan_id` is non-empty;
     - step indexes are contiguous and ordered;
     - lifecycle state matches terminal outcome;
     - upload/start phases exist when plan has mission items/start command;
     - retry count equals observed retry attempts;
     - `Aborted` has a failing/timeout/rejected terminal step;
     - `Completed`/`Retried` has no rejected/timeout terminal step.

   Validation sketch:

   ```rust
   fn validate_execution_report(report: &MavlinkPlanExecutionReport) {
       assert_contiguous_step_indexes(&report.steps);
       assert_lifecycle_matches_outcome(&report.lifecycle_state, &report.overall);
       assert_retry_count_consistent(report);
   }
   ```

6. Split M96 evidence modes: mock/local executor versus real transport.

   Files and anchors:
   `crates/swarm-examples/src/sitl_dual_stack_evidence.rs:372`,
   `crates/swarm-examples/src/sitl_dual_stack_evidence.rs:389`,
   `crates/swarm-examples/src/sitl_dual_stack_evidence.rs:820`,
   docs in `docs/STATUS.md` and `docs/ARTIFACT_VALIDATION.md`.

   Material result:
   - Add an explicit execution evidence mode enum, for example
     `LocalMockExecutor`, `ScriptedProfileExecutor`, `TransportBacked`.
   - PX4/SITL evidence must not be described as transport-backed unless the
     new M90 provider was used.
   - Dual-stack evidence validator must reject claims where `execution_mode`
     says transport-backed but the report was produced by `MockAckProvider`.
   - Existing local evidence remains valid, but caveats become machine-readable
     rather than prose-only.

7. Fix M92 satcom profile mismatch.

   Files and anchors:
   `crates/swarm-comms/src/drone_link.rs:167`,
   `crates/swarm-comms/src/drone_link.rs:174`,
   `docs/DRONE_LINK.md:52`.

   Material result:
   - Choose 8% as the canonical satcom packet loss because both code comment and
     docs already state 8%.
   - Change `packet_loss_rate` from `0.05` to `0.08`.
   - Add a small test accessor or deterministic behavior test so the profile
     contract cannot silently drift again. If exposing raw profile internals is
     undesirable, add `InternetLikeMock::profile_summary()` returning a typed
     immutable summary.

8. Replace `drone_agent` raw heartbeat loop with typed M92 protocol runtime.

   Files and anchors:
   `crates/swarm-examples/src/bin/drone_agent.rs:72`,
   `crates/swarm-examples/src/bin/drone_agent.rs:109`,
   `crates/swarm-comms/src/swarm_protocol.rs:442`,
   `crates/swarm-comms/src/swarm_protocol.rs:492`,
   `crates/swarm-comms/src/drone_link.rs:302`.

   Material result:
   - Extract a testable module, for example
     `crates/swarm-examples/src/drone_agent_runtime/mod.rs`.
   - Binary keeps CLI parsing and delegates to runtime.
   - Runtime sends typed `SwarmMessageEnvelope` with `Heartbeat`/`Presence`
     instead of raw JSON/string payloads.
   - Runtime receives envelopes, rejects malformed/unknown schema payloads,
     drops duplicates through `DuplicateSuppressor`, and handles at least:
     `MissionOffer`, `StateRequest`, `SegmentGrant`, `SegmentDeny`.
   - `RunReport` gains structured counters:
     `messages_sent`, `messages_received`, `duplicates_dropped`,
     `malformed_dropped`, `mission_offers_seen`, `state_requests_answered`,
     `segment_grants_seen`, `segment_denies_seen`.

   Loop sketch:

   ```rust
   for tick in 0..max_ticks {
       while let Some(raw) = transport.poll()? {
           let Some(env) = SwarmMessageEnvelope::from_raw_message(&raw) else {
               report.malformed_dropped += 1;
               continue;
           };
           if duplicates.is_duplicate(&env.envelope_id) {
               report.duplicates_dropped += 1;
               continue;
           }
           protocol.handle(env, &mut transport, &mut report)?;
       }

       protocol.send_heartbeat(tick, &mut transport, &mut report)?;
       if is_presence_tick(tick) {
           protocol.send_presence(tick, &mut transport, &mut report)?;
       }
   }
   ```

9. Add M95 Urban evidence generation from actual network scenario runs.

   Files and anchors:
   `crates/swarm-examples/src/strategy_comparison_runtime/urban_artifacts.rs:229`,
   `crates/swarm-sim/src/urban/operational_evidence.rs:47`,
   `crates/swarm-sim/src/urban/operational_evidence.rs:131`,
   `scenarios/urban.perimeter-patrol.network.json`,
   `scenarios/urban.corridor-inspection.network.json`.

   Material result:
   - Keep replay-only builder pure, but add a helper that attaches execution
     evidence: `with_execution_report(evidence, report)`.
   - Update `write_urban_operational_evidence` or runner output plumbing so
     network scenarios can write `urban_operational_evidence.v1.json` from the
     actual run, not only synthetic validator fixtures.
   - For urban route/IR, compile to `MavlinkCommonPlan` and run the M90 executor
     in local/mock mode by default; when transport-backed mode is configured,
     attach the real transport-backed report.
   - Evidence must state whether `execution_report` is
     `mock_executor`, `scripted_executor`, or `transport_backed`.

10. Add optional UDP/multi-agent smoke path for M95.

    Files and anchors:
    `crates/swarm-comms/src/drone_link.rs:312`,
    `crates/swarm-examples/src/bin/drone_agent.rs:122`,
    `crates/swarm-sim/src/runner/urban_patrol.rs` protocol event handling,
    scenarios under `scenarios/urban.*.network.json`.

    Material result:
    - Add a self-contained localhost integration test or ignored smoke binary
      that starts multiple agent runtime instances using `UdpDroneLink`.
    - It should verify at least one ownership/segment handoff and one recovery
      or denial path through the real link abstraction.
    - The default CI test can use in-memory or loopback UDP with OS-assigned
      local ports; any long-running manual version must be `#[ignore]` or a
      documented command, not a flaky default test.

11. Split M97 readiness statuses and make strong statuses strict.

    Files and anchors:
    `crates/swarm-examples/src/hardware_entry.rs:57`,
    `crates/swarm-examples/src/hardware_entry.rs:134`,
    `crates/swarm-examples/src/hardware_entry.rs:165`,
    `crates/swarm-examples/src/hardware_entry.rs:193`,
    `crates/swarm-examples/src/artifact_validator.rs:1057`,
    `crates/swarm-examples/src/artifact_validator.rs:1164`.

    Material result:
    - Add a weaker status for mock/local executor evidence, for example
      `MockExecutionValidated` or `ExecutorValidatedLocally`.
    - Keep `ExecuteValidatedLocally` only for live local SITL/transport-backed
      evidence with explicit artifact references.
    - `build_hardware_entry_pack` must classify primitive MockAckProvider
      evidence as the weaker status.
    - Add evidence references to the pack, for example:

      ```rust
      pub struct HardwareEntryEvidenceRef {
          pub kind: String,       // execute_artifact, sitl_run, dual_stack, urban
          pub path: String,
          pub profile_id: Option<String>,
          pub execution_mode: Option<String>,
      }
      ```

    - Validator rules for strong statuses
      `ExecuteValidatedLocally` and `DegradedPartiallyEvidenced`:
      - `selected_autopilot`, `selected_airframe`, `selected_link_class` are
        all present and non-empty;
      - `fence_and_failsafe_verified == true`;
      - `manual_abort_procedure_rehearsed == true`;
      - `blockers` is empty;
      - at least one evidence reference points to live local SITL or
        transport-backed execution artifact.

12. Update docs and status after code changes.

    Files:
    `README.md`, `docs/STATUS.md`, `docs/ARTIFACT_VALIDATION.md`,
    `docs/HARDWARE_READINESS.md`, `docs/OPERATIONAL_RUNBOOKS.md`,
    `docs/DRONE_LINK.md`.

    Material result:
    - Docs explicitly distinguish mock/local executor evidence, transport-backed
      SITL evidence and real hardware evidence.
    - `artifact_validator --mode execute` is documented with an example command.
    - M92 protocol loop and satcom profile are documented with exact numbers.
    - M95 urban evidence docs state when `execution_report` is expected and what
      execution mode produced it.
    - M97 docs explain the stricter hardware-entry gate and why mock execution
      is not a live readiness claim.

## Testing strategy

### 1. Tests that need no refactoring - planned with the main changes

- `swarm-comms::mavlink_executor`:
  - update `executor_aborts_on_first_timeout` to assert exact
    `MavlinkExecutionOutcome::Aborted`;
  - add retry-budget tests for timeout-then-accepted and repeated timeout;
  - add serialization roundtrip for any new execute artifact/report fields.
- Transport-backed M90 fake tests:
  - fake MAVLink connection returns accepted `COMMAND_ACK` and `MISSION_ACK`;
  - fake rejects one command and maps to `Rejected`;
  - fake times out on upload/start and maps to `Timeout` then executor
    `Aborted`;
  - fake transport send failure maps to adapter-level `Failed` path.
- FC config tests:
  - geofence upload sends expected fence/mission handshake and enable command;
  - param snapshot sends `PARAM_REQUEST_READ` and matches returned
    `PARAM_VALUE`;
  - param write sends `PARAM_SET` and requires matching confirmation;
  - unsupported/rejected/timeout/mismatch produce typed errors.
- `artifact_validator` tests:
  - `--mode execute` accepts a valid execute artifact;
  - rejects unordered/non-contiguous step indexes;
  - rejects `Completed` with rejected/timeout terminal step;
  - rejects missing upload/start phase when report claims mission execution.
- M92 tests:
  - `drone_agent` runtime sends `SwarmMessageEnvelope` heartbeat/presence;
  - duplicate envelope is dropped by `DuplicateSuppressor`;
  - malformed payload increments `malformed_dropped`;
  - `MissionOffer`, `StateRequest`, `SegmentGrant`, `SegmentDeny` update
    structured report counters.
- Satcom contract test:
  - `InternetLikeMock::with_satcom_profile` exposes or verifies 8% loss in a
    stable way.
- M95 tests:
  - build urban operational evidence from `urban.perimeter-patrol.network.json`
    run output and assert non-empty sector assignments;
  - attach a mock executor report and assert `execution_report.is_some()`;
  - same for `urban.corridor-inspection.network.json`.
- M96 tests:
  - generated dual-stack evidence distinguishes `local_mock_executor` from
    `transport_backed`;
  - validator rejects a transport-backed claim produced by `MockAckProvider`.
- M97 tests:
  - primitive pack with `MockAckProvider` receives weaker readiness status;
  - strong readiness status without selected airframe/link/fence/abort/evidence
    refs is rejected;
  - strong readiness status with all gates and explicit live/transport evidence
    refs is accepted.

### 2. Tests that need light refactoring

- Extract `drone_agent` runtime from the binary into a module to make protocol
  loop tests call pure Rust functions rather than spawning a process.
- Add a fake `MavlinkVehicleConnection` helper shared by command ACK, mission
  upload, FC config and transport-backed executor tests.
- Extract execution report validation helpers from `artifact_validator` so M90,
  M96 and M97 tests can reuse the same assertions without duplicating JSON
  fixtures.
- Add a small urban runner helper that returns replay logs plus generated
  operational evidence in memory before writing files.
- Add typed profile summary for `InternetLikeMock` if direct packet-loss
  assertions otherwise require reaching into private fields.

### 3. Tests that need heavy refactoring

- Full `sitl_agent --execute` e2e over a live PX4/ArduPilot SITL endpoint is not
  suitable for default automated tests; it should be a documented manual or
  ignored integration test until CI can provision SITL deterministically.
- UDP multi-process smoke with real OS processes may be flaky on shared CI.
  Prefer in-process runtime tests first; keep process-level smoke as ignored or
  manual unless the repo gains a stable test harness for dynamic ports and
  process cleanup.
- Hardware-entry validation with real live SITL artifacts needs a structured
  artifact registry/current-vs-historical classifier. Until then, tests should
  use small checked-in JSON fixtures or in-memory temporary directories.

Verification commands for implementation rounds:

```bash
cargo fmt --all
make clippy
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples artifact_validator
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples drone_agent
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban
```

If M90 live transport evidence is produced manually, add a separate documented
command with `--features mavlink-transport` and store resulting artifacts under
`results/`, but do not make that live SITL run a default CI requirement.

## Risks and tradeoffs

- Integrating `sitl_agent --execute` with `MavlinkPlanExecutor` can change
  event ordering and run-report fields. Mitigation: keep compatibility fields
  during transition and validate replay/report schemas with targeted tests.
- Transport-backed `AckProvider` may duplicate parts of old
  upload/lifecycle helpers if not factored carefully. Mitigation: reuse
  `send_command_and_wait_observed`, mission upload helpers and typed error
  mappers instead of rewriting MAVLink handshakes from scratch.
- Changing timeout from `Failed` to `Aborted` is a contract change for existing
  consumers of `MavlinkPlanExecutionReport`. Mitigation: document the split:
  ACK timeout means supervised abort; connection/send/parser failure means
  failed transport.
- Real FC config writes are dangerous if accidentally run against hardware.
  Mitigation: keep hardware gates explicit, require selected target/profile,
  document dry-run/mock defaults, and make live commands opt-in.
- Typed `drone_agent` protocol loop may expose previously ignored malformed
  payloads and duplicates. Mitigation: counters and non-panicking drops.
- UDP smoke tests can be flaky if they rely on fixed ports or process timing.
  Mitigation: use OS-assigned local ports and in-process runtime tests by
  default.
- M97 stricter validator can invalidate older M97 artifacts. Mitigation: mark
  old artifacts historical or regenerate them with weaker status names and
  explicit evidence refs.
- Adding execution reports to Urban evidence can make artifacts larger and
  slightly slower to generate. Mitigation: keep execution mode configurable and
  default to local/mock executor when no transport endpoint is provided.

## Open questions

- Exact name for the weaker M97 status: `ExecutorValidatedLocally` is more
  neutral, `MockExecutionValidated` is more explicit. I recommend
  `MockExecutionValidated` for artifacts produced by `MockAckProvider`, and
  reserving `ExecuteValidatedLocally` for live local SITL/transport-backed
  execution.
- Should `sitl_agent --execute` keep an escape hatch for the old golden-path
  lifecycle during migration, for example `--legacy-lifecycle`, or should M90
  replace it outright after tests pass?
- Should `artifact_validator --mode execute` validate only standalone execute
  artifacts, or also embedded execution reports inside M96/M95/M97 packs via a
  shared helper? I recommend both: CLI mode for standalone artifacts and shared
  helper for embedded reports.
- For real fence upload, MAVLink fence mission semantics differ by autopilot and
  firmware version. M90 should implement the Common/PX4 path first and classify
  unsupported cases explicitly for ArduPilot rather than pretending full
  portability.
- Should UDP/multi-agent smoke be part of default test suite or an ignored
  integration test? I recommend default in-process protocol tests plus ignored
  process-level UDP smoke until CI stability is proven.
