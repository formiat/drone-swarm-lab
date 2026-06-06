# DRONE_B.27 — Итоговый синтез: execution, protocol, transport, autonomy, urban ops

Дата фиксации: 2026-06-06

Источник: синтез DRONE_A.26, DRONE_B.26, DRONE_C.26.

Что взято из каждого:

- **A.26**: lease-based authority model как центральный механизм предотвращения
  split-brain; "Executable Swarm Runtime" как отдельный milestone; hardware-entry
  как discipline milestone.
- **B.26**: конкретные Rust-типы; `drone_agent` binary (агент = процесс);
  `MavlinkPlanExecutor` + `AckProvider` trait; `NullDroneLink` / `SerialDroneLink`
  placeholder; `SegmentCoordinator` как network-facing аналог shared memory.
- **C.26**: `InternetLikeMock` транспорт для cellular/LTE сценария;
  execute lifecycle classification (`planned / uploaded / started / completed /
  aborted / unsupported`); "recommended first slice".

## Архитектурная граница

### Autopilot owns

- stabilization, attitude/rate control, motor output;
- low-level waypoint following;
- EKF / local position estimate;
- onboard failsafes;
- airframe-specific mode semantics;
- airframe-specific tuning.

### This project owns

- mission intent and mission sequencing;
- mission compilation into MAVLink Common plans;
- FC-facing mission upload / fence / param orchestration;
- swarm communication protocol;
- task ownership, reassignment and recovery;
- lease-based authority across partitions;
- agent autonomy policy under degraded connectivity;
- Urban mission semantics and multi-drone coordination;
- replay, metrics, artifacts and evidence packs.

### Ключевые принципы

```text
1. Mission logic не должна знать, по какому каналу идут сообщения.
2. Lease-based authority — единственный механизм борьбы с split-brain.
3. Agent autonomy policy — конфигурируется, не хардкодится.
4. Urban — главный operational proving ground, не demo fixture.
5. Evidence — machine-checkable, не README-only.
```

## Non-Goals

- Нет FC firmware / motor control / offboard low-level loops.
- Нет real RF mesh implementation / antenna model.
- Нет certified obstacle avoidance / SLAM / CV.
- Нет vendor SDK как центральной архитектуры.
- Нет production certification claims.
- Нет claim что SITL = real airframe behavior.
- Нет new simulation physics / richer visual demos.
- Нет long benchmark reruns без новых behaviour.

## Milestone Chain

```text
M90 MAVLink Execution Bridge
  -> M91 Swarm Communication Protocol
    -> M92 Transport Abstraction + Agent Process
      -> M93 Agent Autonomy FSM
        -> M94 Degraded / Partition Swarm Supervisor
          -> M95 Urban Multi-Drone Operational Missions
            -> M96 Dual-Stack Execution Evidence
              -> M97 Hardware-Entry Evidence Pack
```

Почему такой порядок:

1. M90 первый: самый критический gap — `MavlinkCommonPlan` (M81) не подключён
   к `MavlinkTransport`; planning ≠ execution.
2. M91 следующий: typed protocol нужен для M92-M95; без lease-model M93/M94
   не имеют определённой семантики.
3. M92 после protocol: реализация под уже определённый контракт; `UdpDroneLink`
   нужен для M93 (testability под реальными partitions).
4. M93 после transport: failsafe FSM должен уметь тестироваться с реальными
   `add_partition()` / reconnect через `InMemNetwork` и `UdpDroneLink`.
5. M94 после FSM: partition supervisor строится поверх lease + FSM.
6. M95 после degraded supervisor: Urban coordination требует всех трёх слоёв.
7. M96 после Urban ops: evidence нужна для всех mission families.
8. M97 последний: hardware-entry gate требует полноты M90-M96.

---

## M90 — MAVLink Execution Bridge

### Goal

Подключить `MavlinkCommonPlan` (M81) к реальному upload/execute через
`MavlinkTransport`.

Сейчас `compile_mavlink_common_plan` производит детерминированный план с
фазами, expected ACKs и telemetry milestones — но дальше dry-run он не идёт.
`MavlinkTransport` (за feature `mavlink-transport`) умеет общаться с FC, но
не знает о `MavlinkCommonPlan`. M90 закрывает этот gap.

```text
До M90:  MavlinkCommonPlan → dry-run artifact (план без выполнения)
После:   MavlinkCommonPlan → MavlinkPlanExecutor → step-by-step FC ops
```

Параллельно: подключить FC config operations (geofence upload из
`MavlinkFencePlan`, param read/write из `FcParamRequirement` /
`FcParamSnapshot`) к реальным transport-facing code paths.

### Scope

1. `AckProvider` trait и реализации:

   ```rust
   /// Provides ACKs for each plan phase during execution.
   ///
   /// `MockAckProvider` accepts everything (dry-run).
   /// `ScriptedAckProvider` returns a predetermined sequence (tests).
   pub trait AckProvider {
       fn ack_prelude_command(
           &mut self,
           command: &MavlinkCommonCommand,
       ) -> MavlinkExecutionStepResult;

       fn ack_mission_upload(&mut self) -> MavlinkExecutionStepResult;

       fn ack_mission_start(&mut self) -> MavlinkExecutionStepResult;

       fn ack_postlude_command(
           &mut self,
           command: &MavlinkCommonCommand,
       ) -> MavlinkExecutionStepResult;
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "result")]
   pub enum MavlinkExecutionStepResult {
       Accepted,
       Rejected { reason: String },
       Timeout  { after_ms: u64 },
       Skipped  { reason: String },
   }

   /// Accepts every command and upload phase. Used in dry-run and fast tests.
   pub struct MockAckProvider;

   /// Returns predetermined results. Used in deterministic unit tests.
   pub struct ScriptedAckProvider {
       /// Each entry consumed in order.
       script: VecDeque<MavlinkExecutionStepResult>,
   }
   ```

2. `MavlinkPlanExecutor`:

   ```rust
   /// Executes a MavlinkCommonPlan phase by phase against an AckProvider.
   ///
   /// Does not own a transport: execution semantics are separated from
   /// the MAVLink wire protocol so they can be tested without a live FC.
   pub struct MavlinkPlanExecutor<A: AckProvider> {
       ack: A,
       retry_budget: u32,
   }

   impl<A: AckProvider> MavlinkPlanExecutor<A> {
       pub fn execute(
           &mut self,
           plan: &MavlinkCommonPlan,
       ) -> MavlinkPlanExecutionReport;
   }

   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct MavlinkPlanExecutionReport {
       pub plan_id: String,
       /// value: `(step_index, command_name_or_phase, result)`
       pub steps: Vec<(usize, String, MavlinkExecutionStepResult)>,
       pub overall: MavlinkExecutionOutcome,
       pub telemetry_milestones_reached: Vec<MavlinkTelemetryMilestoneKind>,
       pub retry_count: u32,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "outcome")]
   pub enum MavlinkExecutionOutcome {
       Completed,
       Aborted  { at_step: usize, reason: String },
       Failed   { at_step: usize, reason: String },
       Retried  { times: u32 },
   }
   ```

3. FC config execution paths:

   - `execute_geofence_upload(fence_plan: &MavlinkFencePlan, transport: &mut T)`
     → `GeofenceUploadResult`: применяет fence items через тот же
     MISSION_COUNT / REQUEST / ACK handshake, затем `MAV_CMD_DO_FENCE_ENABLE`.
   - `execute_param_snapshot(requirements: &[FcParamRequirement], transport: &mut T)`
     → `FcParamSnapshot`: читает каждый требуемый параметр с FC.
   - `execute_param_write(plan: &FcParamWritePlan, transport: &mut T)`
     → `FcParamWriteResult`.
   - Все три функции возвращают typed errors через `thiserror`, не `anyhow`.
   - При `FcContractViolation` → execution заблокирована, возвращается
     `MavlinkExecutionOutcome::Aborted`.

4. Execute lifecycle classification (из C.26):

   ```rust
   /// Lifecycle state of one mission execution attempt.
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum MissionExecuteLifecycleState {
       /// Plan compiled and validated, no upload attempted.
       Planned,
       /// Upload accepted by FC.
       Uploaded,
       /// Mission start command accepted.
       Started,
       /// Mission reached terminal condition successfully.
       Completed,
       /// Execution aborted by supervisor or FC.
       Aborted,
       /// Feature or command not supported by the selected profile.
       Unsupported,
   }
   ```

5. Stack-specific execution boundary:

   - PX4 path: первый и основной (опирается на M48/M58 baseline).
   - ArduPilot path: experimental, на той же `AckProvider` API, с явными
     `Skipped { reason: "ardupilot_mode_seq_differs" }` для несовместимых шагов.
   - Различия выражены в profile и execution policy, не скрыты в mission logic.

6. Integration:

   - `sitl_agent --execute` использует `MavlinkPlanExecutor` вместо прямых вызовов.
   - `artifact_validator --mode execute` проверяет `MavlinkPlanExecutionReport`
     и `MissionExecuteLifecycleState`.
   - Replay events: `MissionUploaded`, `MissionStarted`, `MissionCompleted`,
     `MissionAborted { step, reason }`, `FenceUploaded`, `ParamWritten`.

### Non-Goals

- Нет real hardware requirement в M90.
- Нет MAVLink library rewrite.
- Нет guarantee что PX4 и ArduPilot принимают каждую команду идентично.
- Нет FC-internal failsafe implementation в этом репозитории.

### Done Criteria

- `MavlinkPlanExecutor` с `MockAckProvider` выполняет `takeoff-hold-land`
  план и возвращает `Completed`.
- `MavlinkPlanExecutor` возвращает `Aborted` при первом `Timeout`.
- `ScriptedAckProvider` возвращает заданную последовательность.
- `execute_geofence_upload` и `execute_param_snapshot` компилируются и
  проходят mock-тесты.
- `FcContractViolation` блокирует execute, не паникует.
- `MissionExecuteLifecycleState` появляется в execute artifacts.
- PX4 path не регрессирует.
- ArduPilot path компилируется и документирует явные ограничения.

### Automated Tests

#### Tests That Need No Refactoring

- `executor_completes_takeoff_hold_land_with_mock_ack`.
- `executor_aborts_on_first_timeout`.
- `executor_skips_unsupported_feature_with_caveat`.
- `scripted_ack_provider_returns_configured_sequence`.
- `executor_retries_within_budget_and_succeeds`.
- `executor_fails_when_retry_budget_exhausted`.
- `geofence_upload_emits_fence_enable_after_items`.
- `geofence_upload_returns_typed_error_on_rejection`.
- `param_snapshot_reads_all_required_params`.
- `fc_contract_violation_blocks_execute`.
- `lifecycle_state_transitions_planned_to_completed`.
- `lifecycle_state_transitions_planned_to_aborted`.
- `ardupilot_incompatible_step_is_skipped_not_panicked`.

#### Tests That Need Light Refactoring

- Mock MAVLink connection fixture со scripted ACK flow.
- Execute-time assertion helper для upload/start/abort фаз.
- `artifact_validator --mode execute` integration test.

#### Tests That Need Heavy Refactoring

- Local PX4/SIH execute smoke с live fence/param path.
- Experimental local ArduPilot SITL execute smoke.
- Synthetic packet-loss/reorder execute-time stress harness.

---

## M91 — Swarm Communication Protocol

### Goal

Определить transport-agnostic typed protocol, по которому дроны, mothership
и GCS координируют mission work.

```text
Не выбираем mesh vs LTE vs serial.
Определяем: message model, ownership model, liveness model,
            lease semantics, degraded behavior, idempotency.
```

Lease-based authority — ключевой механизм: без лизов два агента после
partition могут оба считать себя владельцами одного сегмента. Лиз решает
это детерминированно: истёк лиз → потерял authority, независимо от наличия
GCS.

### Scope

1. Lease types:

   ```rust
   /// Unique lease identifier. Private inner type.
   #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize,
            AsRef, Deref, DerefMut, From, Into)]
   pub struct LeaseId(String);

   /// Authority grant for one ownership claim.
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   pub struct Lease {
       pub lease_id: LeaseId,
       pub holder: AgentId,
       pub resource_id: String,   // task_id, edge_id, sector_id
       pub resource_kind: String, // "task" | "edge" | "sector"
       pub granted_at: DateTime<Utc>,
       pub expires_at: DateTime<Utc>,
   }

   impl Lease {
       /// Returns true if the lease is still valid at the given instant.
       pub fn is_valid_at(&self, now: DateTime<Utc>) -> bool {
           now < self.expires_at
       }
   }
   ```

2. `SwarmMessage` enum — полный набор:

   ```rust
   pub const SWARM_PROTOCOL_SCHEMA_VERSION: &str = "swarm_protocol.v1";

   #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
   #[serde(tag = "kind", rename_all = "snake_case")]
   pub enum SwarmMessage {
       // Presence and health
       Heartbeat {
           tick: u64,
           generation: u64,
           mission_state: AgentMissionState,
       },
       Presence {
           role: SwarmCommandRole,
           capabilities: Vec<String>,
       },

       // Mission assignment lifecycle
       MissionOffer {
           offer_id: String,
           plan: MissionCommandPlan,
           lease_ttl_secs: u32,
       },
       MissionAccept {
           offer_id: String,
           lease_id: LeaseId,
       },
       MissionReject {
           offer_id: String,
           reason: MissionRejectReason,
       },
       MissionResult {
           offer_id: String,
           outcome: MavlinkExecutionOutcome,
           completed_segments: Vec<UrbanEdgeId>,
       },

       // Ownership and lease management
       OwnershipClaim {
           resource_id: String,
           resource_kind: String,
           lease_id: LeaseId,
           expires_at: DateTime<Utc>,
       },
       OwnershipRelease {
           resource_id: String,
           lease_id: LeaseId,
           reason: ReleaseReason,
       },
       LeaseRenew {
           lease_id: LeaseId,
           new_expires_at: DateTime<Utc>,
       },
       LeaseExpired {
           lease_id: LeaseId,
           resource_id: String,
       },

       // Segment coordination (network-level deconfliction)
       SegmentReserve {
           edge_id: UrbanEdgeId,
           segment_index: usize,
           requester: AgentId,
           request_tick: u64,
       },
       SegmentGrant {
           edge_id: UrbanEdgeId,
           to: AgentId,
           lease: Lease,
       },
       SegmentDeny {
           edge_id: UrbanEdgeId,
           to: AgentId,
           holder: AgentId,
           reason: SegmentDenyReason,
       },
       SegmentRelease {
           edge_id: UrbanEdgeId,
           lease_id: LeaseId,
       },

       // Progress and status
       ProgressUpdate {
           resource_id: String,
           progress_pct: u8,    // 0..=100
           position: Option<CommandPosition>,
           tick: u64,
       },
       ReplacementOffer {
           for_resource_id: String,
           plan: MissionCommandPlan,
       },

       // Supervisor signals
       AbortNotice {
           resource_id: String,
           reason: String,
           abort_action: AbortAction,
       },
       DegradedNotice {
           reason: DegradedReason,
           affected_resources: Vec<String>,
       },
       TopologyUpdate {
           topology_kind: String,
           reachable_agents: Vec<AgentId>,
       },

       // State reconciliation
       StateRequest {
           from: AgentId,
           session_id: String,
       },
       StateResponse {
           mission_state: AgentMissionState,
           active_leases: Vec<Lease>,
           completed_resources: Vec<String>,
           last_tick: u64,
       },
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum MissionRejectReason {
       Overloaded,
       IncompatibleRole,
       LeaseExpired,
       DuplicateOffer,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum ReleaseReason {
       Completed,
       Aborted,
       LeaseExpired,
       AgentFailed,
       Reassigned,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum SegmentDenyReason {
       AlreadyHeld,
       PolicyDenied,
       CoordinatorUnavailable,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum DegradedReason {
       GcsUnavailable,
       CoordinatorUnavailable,
       MothershipUnavailable,
       PartitionDetected,
       LeaseExpirySoon,
   }
   ```

3. `AgentMissionState` (из B.26, расширен lease-aware состояниями):

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(tag = "state", rename_all = "snake_case")]
   pub enum AgentMissionState {
       Idle,
       WaitingForMission,
       ExecutingSegment {
           segment_id: UrbanEdgeId,
           lease_id: LeaseId,
           started_at_tick: u64,
       },
       WaitingForSegment {
           edge_id: UrbanEdgeId,
           blocked_by: AgentId,
           since_tick: u64,
       },
       ContinuingUnderLease {
           /// Agent continues mission autonomously while GCS is unreachable.
           /// Valid only while lease has not expired.
           lease_id: LeaseId,
           lease_expires_at: DateTime<Utc>,
       },
       Replanning { reason: ReplanReason },
       GcsLost    { since_tick: u64, policy_engaged: String },
       Aborting   { reason: String },
       Completed  { resource_id: String, finished_at_tick: u64 },
       Failed     { reason: String },
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum ReplanReason {
       SegmentBlocked,
       MissionReassigned,
       GcsCommand,
       LeaseExpired,
   }
   ```

4. `SwarmMessageEnvelope` с полной семантикой:

   ```rust
   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct SwarmMessageEnvelope {
       pub schema_version: String,   // "swarm_protocol.v1"
       pub envelope_id: String,
       pub correlation_id: Option<String>, // links request/response
       pub from: AgentId,
       pub to: AgentId,
       pub sent_at: DateTime<Utc>,
       /// Ticks after which this message should be dropped if undelivered.
       pub ttl_ticks: u32,
       pub message: SwarmMessage,
   }

   impl SwarmMessageEnvelope {
       pub fn into_raw_message(self) -> RawMessage;
       pub fn from_raw_message(raw: &RawMessage) -> Option<Self>;
   }
   ```

5. Duplicate suppression:

   - Получатель хранит `HashSet<String>` последних N `envelope_id`.
   - При получении дубля — silently drops, не паникует, не возвращает ошибку.
   - Размер окна: конфигурируется, default 256.

6. Replay events:

   - `SwarmProtocolMessage { envelope_id, from, to, kind, tick }` в
     `swarm-replay::Event`;
   - `LeaseGranted { lease_id, holder, resource_id, expires_at, tick }`;
   - `LeaseExpired { lease_id, resource_id, tick }`;
   - `OwnershipConflict { resource_id, claimant_a, claimant_b, tick }`.

7. Размещение: новый модуль `swarm-comms/src/swarm_protocol.rs`.
   `RuntimeMessage` остаётся в `swarm-runtime` для CBBA/Gossip allocation;
   `SwarmMessage` — mission coordination level.

### Non-Goals

- Нет transport implementation в этом milestone.
- Нет consensus algorithm (BFT, Raft).
- Нет PKI / cryptographic trust.
- Нет backward-breaking изменений в `RuntimeMessage`.

### Done Criteria

- Все варианты `SwarmMessage` сериализуются snake_case без потерь.
- `Lease::is_valid_at` детерминированно возвращает false после `expires_at`.
- `SwarmMessageEnvelope` кодируется в `RawMessage` и декодируется обратно.
- Duplicate envelope_id обрабатывается без паники.
- Replay events для lease lifecycle присутствуют.
- `docs/SWARM_PROTOCOL.md` описывает каждую группу сообщений, lease model,
  duplicate suppression, что такое correlation_id.

### Automated Tests

#### Tests That Need No Refactoring

- `swarm_message_serde_roundtrip_all_variants`.
- `agent_mission_state_serde_roundtrip_all_variants`.
- `lease_is_valid_before_expiry`.
- `lease_is_invalid_after_expiry`.
- `swarm_message_envelope_into_raw_and_from_raw`.
- `envelope_with_unknown_schema_version_returns_none`.
- `duplicate_envelope_id_is_dropped_silently`.
- `mission_offer_accept_correlation_id_matches`.
- `lease_expired_event_emitted_in_replay`.
- `ownership_conflict_event_emitted_on_duplicate_claim`.
- `segment_reserve_grant_deny_release_roundtrip_via_inmem_network`.
- `swarm_protocol_schema_version_constant_matches_doc`.

#### Tests That Need Light Refactoring

- `SwarmMessageEnvelope` builder helper для тестов.
- `InMemNetwork` fixture с двумя агентами для полного exchange цикла.
- Replay assertion helper для protocol events.

#### Tests That Need Heavy Refactoring

- Schema versioning migration (swarm_protocol.v1 → v2).
- Fuzz test: arbitrary payload → graceful `None`, не panic.
- Property test: любая последовательность Claim/Release/Expire сохраняет
  инварианты (один ресурс — один активный лиз).

---

## M92 — Transport Abstraction + Agent Process

### Goal

Четыре реализации `Transport` trait + `drone_agent` binary, после которого
каждый агент — независимый процесс.

```text
До M92:  1 процесс = N агентов в shared memory
После:   1 процесс = 1 агент, N процессов = N дронов
```

Companion computer на реальном дроне запускает тот же binary — меняется
только конфиг адреса, не код.

### Scope

1. `UdpDroneLink`:

   ```rust
   /// UDP unicast transport between agent processes.
   pub struct UdpDroneLink {
       socket: UdpSocket,
       own_id: AgentId,
       /// key: `AgentId`
       peers: BTreeMap<AgentId, SocketAddr>,
       recv_buffer: VecDeque<RawMessage>,
   }

   impl UdpDroneLink {
       pub fn bind(
           own_id: AgentId,
           bind_addr: SocketAddr,
           peers: BTreeMap<AgentId, SocketAddr>,
       ) -> Result<Self, UdpDroneLinkError>;

       pub fn local_id(&self) -> &AgentId;
   }

   impl Transport for UdpDroneLink {
       type Error = UdpDroneLinkError;
       fn send(&mut self, msg: RawMessage) -> Result<(), UdpDroneLinkError>;
       fn poll(&mut self) -> Result<Option<RawMessage>, UdpDroneLinkError>;
   }

   #[derive(Debug, thiserror::Error)]
   pub enum UdpDroneLinkError {
       #[error("io: {0}")]
       Io(#[from] std::io::Error),
       #[error("unknown peer: {0}")]
       UnknownPeer(AgentId),
       #[error("payload too large: {0} bytes (max 65507)")]
       PayloadTooLarge(usize),
   }
   ```

   - `send()`: UDP sendto по адресу из `peers`, не блокирует.
   - `poll()`: nonblocking `recv_from`, один пакет за вызов, буферизует остаток.
   - `UnknownPeer` — typed error, не panic.
   - Размер payload > 65507 → `PayloadTooLarge`.

2. `InternetLikeMock` (из C.26):

   ```rust
   /// Simulates internet-like channel characteristics in-process.
   ///
   /// Models: high baseline latency, variable per-packet jitter, burst drops,
   /// occasional packet reordering. Useful for testing cellular/LTE scenarios.
   pub struct InternetLikeMock {
       inner: InMemNetwork,
       reorder_probability: f64,
       burst_drop_probability: f64,
       in_burst: bool,
       rng: SmallRng,
   }

   impl InternetLikeMock {
       pub fn with_lte_profile(seed: u64) -> Self;    // ~80ms latency, 3% loss
       pub fn with_satcom_profile(seed: u64) -> Self; // ~600ms latency, 8% loss
   }

   impl Transport for InternetLikeMock {
       type Error = Infallible;
       fn send(&mut self, msg: RawMessage) -> Result<(), Infallible>;
       fn poll(&mut self) -> Result<Option<RawMessage>, Infallible>;
   }
   ```

3. `SerialDroneLink` — placeholder:

   ```rust
   pub struct SerialDroneLink { pub own_id: AgentId }

   impl Transport for SerialDroneLink {
       type Error = SerialDroneLinkError;
       fn send(&mut self, _: RawMessage) -> Result<(), SerialDroneLinkError> {
           Err(SerialDroneLinkError::NotImplemented)
       }
       fn poll(&mut self) -> Result<Option<RawMessage>, SerialDroneLinkError> {
           Err(SerialDroneLinkError::NotImplemented)
       }
   }

   #[derive(Debug, thiserror::Error)]
   pub enum SerialDroneLinkError {
       #[error("serial transport not yet implemented")]
       NotImplemented,
   }
   ```

4. `NullDroneLink` — для тестов:

   ```rust
   /// Drops all outgoing messages and never returns incoming ones.
   pub struct NullDroneLink { pub own_id: AgentId }

   impl Transport for NullDroneLink {
       type Error = std::convert::Infallible;
       fn send(&mut self, _: RawMessage) -> Result<(), Infallible> { Ok(()) }
       fn poll(&mut self) -> Result<Option<RawMessage>, Infallible> { Ok(None) }
   }
   ```

5. `DroneLinkConfig` в Scenario DSL:

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "kind")]
   pub enum DroneLinkConfig {
       /// In-memory shared bus (default, backward compatible).
       Simulated,
       /// UDP unicast between processes on localhost or LAN.
       Udp {
           bind_addr: String,
           /// key: `AgentId`
           peers: BTreeMap<AgentId, String>,
       },
       /// Simulated internet-like channel (high latency, variable loss).
       InternetLikeMock {
           profile: InternetLikeMockProfile,
           seed: u64,
       },
       /// Serial port placeholder, returns NotImplemented.
       Serial {
           path: String,
           baud: u32,
       },
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum InternetLikeMockProfile {
       Lte,
       Satcom,
   }
   ```

   Добавляется в `RunConfig`:
   ```rust
   #[serde(default)]
   pub drone_link: DroneLinkConfig,
   ```

6. `drone_agent` binary (`swarm-examples/src/bin/drone_agent.rs`):

   ```text
   drone_agent --config agent-0.json [--dry-run]
   ```

   Конфиг JSON:
   ```json
   {
     "agent_id": "agent-0",
     "drone_link": { "kind": "udp", "bind_addr": "127.0.0.1:7001",
                     "peers": { "agent-1": "127.0.0.1:7002" } },
     "autonomy": { "gcs_lost_policy": { "kind": "return_to_launch", "after_ticks": 30 } },
     "max_ticks": 200
   }
   ```

   В `--dry-run` режиме: обменивается heartbeat/gossip с другими процессами
   через `UdpDroneLink`, ничего не отправляет на FC. Позволяет запустить
   N терминалов = N "дронов" без железа.

   Transport selection записывается в run report (`drone_link_kind` field).

### Non-Goals

- Нет шифрования / аутентификации UDP.
- Нет TCP fallback для больших payload (явная ошибка > MTU).
- Нет fragmentation/reassembly.
- Нет multicasr/broadcast (только unicast).
- Нет hardware serial deployment в этом milestone.
- Нет stable public API для `drone_agent`.

### Done Criteria

- `UdpDroneLink` loopback тест: 2 потока, N сообщений, все доставлены.
- `UnknownPeer` при отправке неизвестному агенту, не panic.
- `PayloadTooLarge` при превышении UDP MTU.
- `SerialDroneLink` компилируется, возвращает `NotImplemented`.
- `NullDroneLink` компилируется, отбрасывает всё.
- `InternetLikeMock` с LTE profile: средняя задержка ~80 тиков, ~3% drop
  на 1000 сообщений при seed=42.
- `DroneLinkConfig` сериализуется, добавлен в `RunConfig`.
- Существующие сценарии с `Simulated` (default) не ломаются.
- `drone_agent --dry-run --config ...` запускается и выходит с кодом 0.
- Transport kind записан в run report.
- `docs/DRONE_LINK.md`: 4 реализации, когда какую использовать,
  как добавить новую.

### Automated Tests

#### Tests That Need No Refactoring

- `udp_drone_link_loopback_roundtrip`.
- `udp_drone_link_unknown_peer_returns_error`.
- `udp_drone_link_payload_too_large_returns_error`.
- `null_drone_link_send_drops_silently`.
- `null_drone_link_poll_returns_none`.
- `serial_drone_link_returns_not_implemented`.
- `internet_like_mock_lte_delivers_messages_with_latency`.
- `internet_like_mock_lte_drops_approximately_3pct`.
- `drone_link_config_serde_roundtrip_all_variants`.
- `drone_link_config_default_is_simulated`.
- `existing_scenarios_load_without_drone_link_field`.
- `drone_agent_dry_run_exits_zero`.

#### Tests That Need Light Refactoring

- Port allocation helper для UDP tests (OS-assigned ports).
- `cbba_converges_over_udp_drone_link` (два потока).
- `drone_agent` CLI integration: `--config` → запуск + чистое завершение.

#### Tests That Need Heavy Refactoring

- CBBA convergence под `InternetLikeMock` LTE profile.
- 3-process `drone_agent` smoke: handshake через UDP.
- Chaos soak: случайные delays + drops → M91 protocol не паникует.

---

## M93 — Agent Autonomy FSM

### Goal

Каждый агент имеет конфигурируемую failsafe-политику и lease-aware автономию.

Агент продолжает миссию пока его лиз валиден — даже без GCS. Когда лиз
истекает, применяет `GcsLostPolicy`. При восстановлении связи отправляет
`StateResponse` для reconciliation, GCS принимает решение.

```text
Лиз валиден → ContinuingUnderLease (работает автономно)
Лиз истёк → применяет GcsLostPolicy (RTL / Hover / Abort)
GCS вернулся → StateRequest / StateResponse → reconciliation
```

### Scope

1. Failsafe policies:

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "kind")]
   pub enum GcsLostPolicy {
       /// Continue executing plan until lease expires or plan completes.
       ContinueMission { max_gcs_lost_ticks: u64 },
       /// Halt at current position and await GCS return or lease expiry.
       HoverInPlace    { max_gcs_lost_ticks: u64 },
       /// Initiate RTL after the configured number of ticks without GCS.
       ReturnToLaunch  { after_ticks: u64 },
       /// Immediately abort and RTL on any GCS loss event.
       AbortImmediate,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "kind")]
   pub enum MothershipLostPolicy {
       WaitAtStaging     { max_ticks: u64 },
       ProceedAutonomously,
       ReturnToLaunch,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "kind")]
   pub enum NeighborLostPolicy {
       /// Release segment locks held by the lost agent and continue.
       ReleaseLocksAndContinue,
       WaitForReconnect { max_ticks: u64 },
       AbortMission,
   }
   ```

2. `AgentAutonomyConfig` (добавляется в `RunConfig`):

   ```rust
   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct AgentAutonomyConfig {
       #[serde(default)]
       pub gcs_lost_policy: GcsLostPolicy,
       #[serde(default)]
       pub mothership_lost_policy: MothershipLostPolicy,
       #[serde(default)]
       pub neighbor_lost_policy: NeighborLostPolicy,
       /// Ticks without heartbeat before declaring GCS lost.
       #[serde(default = "default_gcs_heartbeat_timeout")]
       pub gcs_heartbeat_timeout_ticks: u64,
       /// Ticks without heartbeat before declaring a peer agent lost.
       #[serde(default = "default_peer_heartbeat_timeout")]
       pub peer_heartbeat_timeout_ticks: u64,
   }

   impl Default for AgentAutonomyConfig {
       fn default() -> Self { /* conservative defaults */ }
   }
   ```

3. FSM transitions в `AgentNode<T>`:

   На каждом `tick()`:
   a. Проверяет `last_gcs_heartbeat_tick`. Если превышен порог → переход в
      `AgentMissionState::GcsLost` + применяет политику.
   b. Если в `ContinuingUnderLease` и лиз истёк → применяет `GcsLostPolicy`.
   c. Проверяет `last_peer_heartbeat_ticks` по каждому peer → при таймауте:
      применяет `NeighborLostPolicy` (может release segment locks).
   d. При получении `Heartbeat` от GCS → переход из `GcsLost` обратно,
      отправляет `StateResponse`.

4. `StateReconcileReport`:

   ```rust
   /// What an agent did while GCS was unreachable.
   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct StateReconcileReport {
       pub agent_id: AgentId,
       pub gcs_lost_ticks: u64,
       pub policy_applied: String,
       pub completed_resources: Vec<String>,
       pub active_leases_at_reconnect: Vec<Lease>,
       pub mission_state_at_reconnect: AgentMissionState,
   }
   ```

5. Replay events:

   - `AgentGcsLost { agent_id, tick, policy }`;
   - `AgentGcsReconnected { agent_id, tick, gcs_lost_ticks }`;
   - `AgentContinuingUnderLease { agent_id, lease_id, tick }`;
   - `AgentLeaseExpiredDuringGcsLoss { agent_id, lease_id, policy_applied, tick }`;
   - `AgentNeighborLost { agent_id, lost_neighbor_id, tick }`;
   - `AgentStateReconciled { agent_id, tick, report }`.

6. Метрики (добавляются в `RunMetrics`):

   `gcs_lost_count`, `gcs_lost_total_ticks`, `neighbor_lost_count`,
   `failsafe_rtl_count`, `lease_expired_during_gcs_loss_count`.

### Non-Goals

- Нет real MAVLink RTL команды без живого FC.
- Нет distributed consensus при одновременной потере нескольких агентов.
- Нет certified failsafe behaviour.
- Нет изменений в CBBA/Gossip/Heartbeat tests.

### Done Criteria

- Агент с `ReturnToLaunch { after_ticks: 10 }` переходит в `GcsLost`
  при partition на 10+ тиков.
- Агент в `ContinuingUnderLease` остаётся активным пока лиз не истёк,
  потом применяет `GcsLostPolicy`.
- `StateReconcileReport` содержит `active_leases_at_reconnect`.
- Replay log содержит `AgentGcsLost` и `AgentContinuingUnderLease`.
- Метрики `gcs_lost_count` > 0 при partition сценарии.
- Существующие сценарии без `autonomy` поля десериализуются (все `#[serde(default)]`).

### Automated Tests

#### Tests That Need No Refactoring

- `gcs_lost_rtl_engages_after_threshold`.
- `gcs_lost_continue_does_not_abort_before_threshold`.
- `gcs_lost_abort_immediate_triggers_on_first_tick`.
- `continuing_under_lease_stays_active_while_lease_valid`.
- `lease_expiry_during_gcs_loss_applies_policy`.
- `gcs_reconnect_emits_state_reconcile_report`.
- `state_reconcile_report_contains_active_leases`.
- `neighbor_lost_releases_segment_locks`.
- `mothership_lost_wait_at_staging_holds`.
- `replay_contains_agent_gcs_lost`.
- `replay_contains_agent_continuing_under_lease`.
- `gcs_lost_count_metric_is_nonzero_after_partition`.
- `existing_scenarios_load_without_autonomy_field`.

#### Tests That Need Light Refactoring

- Fixture builder для partition + `AgentAutonomyConfig` сценариев.
- `PartitionEvent` helper: GCS partition → reconnect cycle.
- Replay assertion helper для FSM events.

#### Tests That Need Heavy Refactoring

- Property test: random partition/reconnect на 3 агентах → нет invalid FSM transitions.
- Stress test: 8 агентов, GCS loss на 50 тиков, reconnect, reconcile.
- Mothership + 2 sub-agents: mothership loss → children wait → reconnect.

---

## M94 — Degraded / Partition Swarm Supervisor

### Goal

Supervisor различает **node death** (агент умер) и **link loss** (связь
потеряна, агент жив). Сейчас эти два случая смешаны.

Под партицией два подмножества агентов могут независимо считать, что владеют
одним ресурсом. Lease-based authority (M91) определяет, кто прав
детерминированно: у кого лиз не истёк — у того authority.

```text
Split:
  Группа A видит только себя → продолжает под лизами
  Группа B видит только себя → продолжает под лизами
  Heal:
    Лиз A истёк → Группа B взяла ресурсы без конфликта
    Лиз B истёк раньше → Группа A взяла ресурсы без конфликта
    Оба валидны → supervisor выбирает по policy (старейший лиз wins)
```

### Scope

1. Degraded condition types:

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum ConnectivityLossKind {
       /// GCS heartbeat gone, agents still mutually reachable.
       GcsUnavailable,
       /// Coordinator gone, agents still active.
       CoordinatorUnavailable,
       /// Mothership gone, children still running.
       MothershipUnavailable,
       /// One drone isolated from all peers.
       DroneIsolated,
       /// Network partition splits the swarm into separate groups.
       SwarmPartitioned { group_sizes: Vec<usize> },
   }

   /// Distinguishes permanent agent failure from temporary link loss.
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum AgentAbsenceKind {
       /// Agent missed enough heartbeats to declare it dead.
       NodeFailure,
       /// Agent was reachable before partition event.
       LinkLoss { partition_tick: u64 },
   }
   ```

2. Supervisor policy decisions:

   - `ContinueUnderLease`: continue locally, send `DegradedNotice`.
   - `HoldAndWaitRenew`: pause at current position, await `LeaseRenew`.
   - `ReleaseAfterTimeout { ticks }`: release ownership after N ticks without renew.
   - `ReturnToLaunch`: abort current task, RTL.
   - `ForbidReassignment`: do not reassign until lease expiry confirmed.
   - `ReconcileOnReconnect`: on heal, compare states, non-destructively merge.

3. Supervisor invariants (enforced, не только asserted):

   - Один ресурс — не более одного активного лиза в любой момент с учётом
     partition (старейший лиз имеет приоритет при конфликте).
   - Нет silent task disappearance: каждый `ReleaseReason` записан.
   - Каждое degraded-решение имеет `DegradedReason` в replay.
   - Restored connectivity → `StateRequest` → `StateResponse` → merge,
     не blind overwrite.

4. Reconciliation logic:

   ```rust
   pub struct SupervisorReconcileResult {
       pub accepted: Vec<String>,  // resource_ids taken from reconnected agent
       pub rejected: Vec<String>,  // resource_ids: stale lease, already reassigned
       pub conflicts: Vec<OwnershipConflict>,
   }

   pub struct OwnershipConflict {
       pub resource_id: String,
       pub holder_a: AgentId,
       pub lease_a: Lease,
       pub holder_b: AgentId,
       pub lease_b: Lease,
       pub resolution: ConflictResolution,
   }

   #[derive(Clone, Debug, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum ConflictResolution {
       /// Older lease retains authority.
       OlderLeaseWins { winner: AgentId },
       /// Supervisor resets both and reassigns.
       SupervisorReset,
   }
   ```

5. Artifacts:

   - `PartitionReport { partition_tick, heal_tick, affected_agents, leases_at_partition }`;
   - `ReconciliationReport { reconnect_tick, result: SupervisorReconcileResult }`;
   - `DegradedDecisionLog { tick, condition, decision, affected_resources }`.

6. Replay events:

   - `PartitionDetected { tick, group_a, group_b }`;
   - `PartitionHealed { tick }`;
   - `SupervisorDegradedDecision { tick, condition, decision, resources }`;
   - `SupervisorReconciled { tick, result_summary }`;
   - `CommandSuppressed { tick, resource_id, reason }`.

### Non-Goals

- Нет consensus algorithm с гарантиями (BFT, Paxos).
- Нет cryptographic trust model.
- Нет real radio validation.
- Нет autonomous tactical swarm AI.

### Done Criteria

- Supervisor с `LinkLoss` не помечает агента умершим до истечения лиза.
- При partition: оба подмножества работают под лизами, после heal —
  reconciliation без duplicate authority.
- `OwnershipConflict` разрешается детерминированно через `OlderLeaseWins`.
- Replay содержит `PartitionDetected`, `PartitionHealed`, `SupervisorReconciled`.
- `CommandSuppressed` выдаётся при ambiguous authority.
- `artifact_validator` проверяет `PartitionReport` и `ReconciliationReport`.

### Automated Tests

#### Tests That Need No Refactoring

- `link_loss_does_not_mark_agent_dead_before_lease_expiry`.
- `node_failure_releases_resources_immediately`.
- `partition_both_groups_continue_under_leases`.
- `older_lease_wins_conflict_resolution`.
- `reconciliation_rejects_stale_lease_after_heal`.
- `reconciliation_accepts_valid_lease_after_heal`.
- `supervisor_reset_on_unresolvable_conflict`.
- `command_suppressed_on_ambiguous_authority`.
- `partition_report_in_replay`.
- `reconciliation_report_in_artifact`.
- `no_silent_task_disappearance_invariant`.

#### Tests That Need Light Refactoring

- Partition scenario fixture builder (partition_tick + heal_tick).
- Lease clock helper (injectable time source для детерминированных тестов).
- `artifact_validator` checks для partition/reconciliation sections.

#### Tests That Need Heavy Refactoring

- Split-brain simulation harness (8 агентов, 2 группы, multiple heal cycles).
- Reconciliation stress: repeated partition/heal с overlapping leases.
- Protocol + supervisor fuzz: delayed duplicates после partition.

---

## M95 — Urban Multi-Drone Operational Missions

### Goal

Urban перестаёт быть simulation testbed и становится главным operational
proving ground для swarm coordination.

```text
Может ли несколько дронов выполнить реалистичную Urban-миссию с ownership,
handoff, degraded comms и explicit supervisor policy?
```

Сегментный деконфликтинг переходит от shared memory к network-level
`SegmentCoordinator` через `SwarmMessage`.

### Scope

1. Mission families (минимум два обязательных):

   - `urban-perimeter-patrol`: N агентов, split по секторам, ownership не
     пересекается, handoff при потере агента.
   - `urban-corridor-inspection`: split по корридорам, у каждого агента
     свой route slice, замена при failure.
   - `urban-search-until-detection`: search по секторам, handoff upon
     mocked detection, reserve активируется при потере searcher.
   - `urban-blocked-route-recovery`: blocked edge → replan с передачей
     remaining segments reserve агенту.

2. Network-level `SegmentCoordinator`:

   ```rust
   /// Network-facing analog of UrbanSegmentLockRegistry.
   ///
   /// Accepts SegmentReserve/Release via SwarmMessage over Transport.
   /// Backward-compatible with SharedMemory deconfliction mode.
   pub struct SegmentCoordinator<T: Transport> {
       transport: T,
       /// key: `UrbanEdgeId`
       active_locks: HashMap<UrbanEdgeId, (UrbanSegmentLock, Lease)>,
       policy: UrbanRightOfWayPolicy,
       /// key: `AgentId`
       priorities: HashMap<AgentId, u8>,
   }

   impl<T: Transport> SegmentCoordinator<T> {
       pub fn handle_incoming(&mut self, tick: u64) -> Vec<CoordinatorEvent>;
   }

   pub enum CoordinatorEvent {
       GrantSent { edge_id: UrbanEdgeId, to: AgentId },
       DenySent  { edge_id: UrbanEdgeId, to: AgentId, reason: SegmentDenyReason },
       Released  { edge_id: UrbanEdgeId, agent_id: AgentId },
       LeaseExpired { edge_id: UrbanEdgeId, agent_id: AgentId },
   }
   ```

3. `DeconflictionMode` для backward compatibility:

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum DeconflictionMode {
       /// Single-process shared memory. Backward compatible.
       SharedMemory,
       /// Network protocol via SwarmMessage. Requires coordinator_id.
       NetworkProtocol { coordinator_id: AgentId },
   }
   ```

4. Urban evidence artifact:

   ```rust
   pub struct UrbanOperationalEvidence {
       pub schema_version: String,  // "urban_operational_evidence.v1"
       pub mission_id: String,
       pub mission_family: String,
       pub created_at: DateTime<Utc>,
       pub git_commit: String,
       pub deconfliction_mode: DeconflictionMode,
       pub agent_count: usize,
       /// value: `(agent_id, sector_or_route_slice_id, completed)`
       pub sector_assignments: Vec<(AgentId, String, bool)>,
       /// value: `(tick, from_agent, to_agent, resource_id)`
       pub handoff_events: Vec<(u64, AgentId, AgentId, String)>,
       pub coordination_delay_ticks: u64,
       pub degraded_outcomes: Vec<String>,
       pub execution_report: Option<MavlinkPlanExecutionReport>,
       pub preflight_report: SafetyValidationReport,
       pub caveats: Vec<String>,
   }
   ```

5. Degraded urban behavior:

   - Изолированный агент продолжает owned segment пока лиз валиден.
   - Checkpoint wait при ambiguous authority (координатор недоступен).
   - При `NoRouteAvailable` — explicit degraded outcome в evidence.
   - Reserve агент активируется только при явном `ReleaseReason::AgentFailed`.

6. Fixtures:

   - `scenarios/urban.perimeter-patrol.network.json` — perimeter patrol,
     2 агента, `DeconflictionMode::NetworkProtocol`.
   - `scenarios/urban.corridor-inspection.network.json` — corridor inspection,
     3 агента.

### Non-Goals

- Нет physical collision avoidance.
- Нет real sensor fusion.
- Нет full GIS/navmesh planner.
- Нет claim что Urban simulation = field deployment.

### Done Criteria

- Два Urban mission family работают end-to-end через supervisor policy.
- `SegmentCoordinator` через `InMemAgentTransport` не допускает
  simultaneous hold одного edge.
- Handoff при потере агента детерминирован и observable.
- `UrbanOperationalEvidence` генерируется и валидируется.
- `artifact_validator --mode urban-operational` проверяет структуру.
- Существующие single-process Urban сценарии не регрессируют.

### Automated Tests

#### Tests That Need No Refactoring

- `segment_coordinator_grants_first_request`.
- `segment_coordinator_denies_concurrent_to_held_segment`.
- `segment_coordinator_grants_after_release`.
- `segment_lease_expiry_frees_segment`.
- `perimeter_patrol_sector_ownership_is_disjoint`.
- `agent_failure_triggers_handoff_to_reserve`.
- `blocked_route_recovery_produces_replacement_mission`.
- `search_detection_triggers_sector_handoff`.
- `isolated_agent_continues_under_valid_lease`.
- `checkpoint_wait_on_coordinator_unavailable`.
- `no_safe_route_produces_explicit_degraded_outcome`.
- `urban_operational_evidence_serde_roundtrip`.
- `shared_memory_deconfliction_backward_compat`.

#### Tests That Need Light Refactoring

- Multi-agent Urban fixture builder с `NetworkProtocol` coordinator.
- Route-handoff assertion helper.
- `artifact_validator --mode urban-operational` integration test.

#### Tests That Need Heavy Refactoring

- 3-agent perimeter patrol over `UdpDroneLink`: независимые процессы,
  network-level coordination.
- Synthetic urban swarm suite: много map fragments, random failures.
- Long-running degraded urban patrol: repeated blocked edges + partitions.

---

## M96 — Dual-Stack Execution Evidence

### Goal

Перейти от dual-stack dry-run evidence (M89) к dual-stack execution evidence.

```text
До M96:  PX4 и ArduPilot различаются в compile-time profile annotations
После:   PX4 и ArduPilот имеют execute-time evidence на одной API-границе
```

Ключевая проверка: одинаковый `command_ir_hash` в обоих артефактах — доказательство
что это одна миссия, скомпилированная через два profile.

### Scope

1. Execute lifecycle classification (из C.26):

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum MissionExecuteLifecycleState {
       Planned,
       Uploaded,
       Started,
       Completed,
       Aborted,
       Unsupported,
   }
   ```

2. `DualStackExecutionEvidence`:

   ```rust
   pub struct DualStackExecutionEvidence {
       pub schema_version: String,  // "dual_stack_execution_evidence.v1"
       pub mission_id: String,
       pub command_ir_hash: String,  // must be identical across both stacks
       pub created_at: DateTime<Utc>,
       pub git_commit: String,
       pub px4: StackExecutionRecord,
       pub ardupilot: StackExecutionRecord,
       pub comparison: StackComparisonSummary,
   }

   pub struct StackExecutionRecord {
       pub profile_id: String,
       pub lifecycle_state: MissionExecuteLifecycleState,
       pub execution_report: MavlinkPlanExecutionReport,
       pub fc_contract_result: FcContractValidationResult,
       pub caveats: Vec<String>,
       pub unsupported_features: Vec<MavlinkUnsupportedFeature>,
   }

   pub struct StackComparisonSummary {
       pub same_command_ir_hash: bool,
       pub lifecycle_states_match: bool,
       pub step_count_delta: i32,
       pub caveat_count_delta: i32,
       pub unsupported_count_delta: i32,
       pub notable_differences: Vec<String>,
   }
   ```

3. PX4 path:

   - Сохранить и обновить существующий PX4/SIH baseline под M90-M95 слои.
   - `MissionExecuteLifecycleState::Completed` для successful local run.

4. ArduPilot path:

   - Experimental execute path на той же `MavlinkPlanExecutor` API.
   - Incompatible steps → `Skipped { reason: "ardupilot_…" }`, не panic.
   - `MissionExecuteLifecycleState::Unsupported` для неподдерживаемых features.
   - Local-only runbook, never presented as production proof.

5. `artifact_validator --mode dual-stack-execution` проверяет:

   - `same_command_ir_hash = true`;
   - оба `StackExecutionRecord` присутствуют;
   - `unsupported` не помечен как `Completed`;
   - caveats явно перечислены.

### Non-Goals

- Нет hardware proof.
- Нет guarantee что оба стека поддерживают всё одинаково.
- Нет automated external dependency в default тестах.

### Done Criteria

- `DualStackExecutionEvidence` генерируется для primitive mission.
- `command_ir_hash` идентичен в обоих records.
- ArduPilot incompatible steps → `Skipped`, не Aborted/panic.
- `artifact_validator --mode dual-stack-execution` passes.
- PX4 baseline не регрессирует.

### Automated Tests

#### Tests That Need No Refactoring

- `dual_stack_evidence_command_ir_hash_matches`.
- `px4_lifecycle_completes_takeoff_hold_land`.
- `ardupilot_lifecycle_skips_incompatible_steps`.
- `ardupilot_unsupported_feature_is_not_marked_completed`.
- `stack_comparison_summary_correct_delta_counts`.
- `dual_stack_evidence_serde_roundtrip`.
- `artifact_validator_dual_stack_execution_passes`.
- `artifact_validator_fails_on_mismatched_ir_hash`.

#### Tests That Need Light Refactoring

- Dual-stack evidence fixture builder.
- Lifecycle report assertion helper.
- Docs smoke: PX4 vs ArduPilot runtime caveats раздел.

#### Tests That Need Heavy Refactoring

- Local PX4 execute smoke aligned с M90 executor.
- Local ArduPilot execute smoke (experimental).
- Cross-stack comparison harness для всех primitive missions.

---

## M97 — Hardware-Entry Evidence Pack

### Goal

Machine-checkable артефакт перед первым контролируемым hardware-экспериментом.

```text
"Engineering says it probably works" → operator has documented basis for test.
```

Этот milestone не требует железа. Он создаёт gate discipline: `artifact_validator
--mode hardware-entry-pack` явно сообщает что готово и что блокирует.

### Scope

1. `HardwareEntryPack` schema:

   ```rust
   pub struct HardwareEntryPack {
       pub schema_version: String,  // "hardware_entry_pack.v1"
       pub pack_id: String,
       pub created_at: DateTime<Utc>,
       pub git_commit: String,

       // Mission coverage
       pub mission_families_covered: Vec<String>,
       pub primitive_evidence: Option<MavlinkPlanExecutionReport>,
       pub urban_evidence: Option<UrbanOperationalEvidence>,
       pub swarm_evidence: Option<SwarmCommandFanoutSummary>,
       pub dual_stack_evidence: Option<DualStackExecutionEvidence>,

       // FC configuration evidence
       pub fc_contract_result: FcContractValidationResult,
       pub param_snapshot: Option<FcParamSnapshot>,
       pub fence_plan: Option<MavlinkFenceArtifact>,

       // Protocol and topology
       pub swarm_protocol_assumptions: Vec<String>,
       pub topology_assumptions: Vec<String>,
       pub degraded_policy_matrix: Vec<DegradedPolicyEntry>,

       // Preflight and safety
       pub preflight_report: SafetyValidationReport,

       // Operational setup
       pub hardware_entry_checklist: HardwareEntryChecklist,
       pub run_command: String,

       // Classification and limitations
       pub readiness_status: HardwareReadinessStatus,
       pub caveats: Vec<String>,
       pub limitations: Vec<String>,
       pub blockers: Vec<String>,
   }

   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct DegradedPolicyEntry {
       pub condition: String,
       pub policy: String,
       pub tested: bool,
   }
   ```

2. `HardwareReadinessStatus` (из C.26, расширен):

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum HardwareReadinessStatus {
       /// Plan compiled, dry-run only. No upload evidence.
       DryRunOnly,
       /// Upload and execute validated in local SITL.
       ExecuteValidatedLocally,
       /// Degraded behavior partially evidenced under simulation.
       DegradedPartiallyEvidenced,
       /// Feature or behavior is unsupported or unknown.
       UnsupportedOrUnknown { detail: String },
       /// Explicitly blocked. Hardware entry should not proceed.
       Blocked { reason: String },
   }
   ```

3. `HardwareEntryChecklist`:

   ```rust
   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct HardwareEntryChecklist {
       pub selected_autopilot: Option<String>,
       pub selected_airframe: Option<String>,
       pub selected_link_class: Option<String>,  // "serial" | "udp" | "lte"
       pub coordinate_frame_policy: Option<String>,
       pub altitude_reference: Option<String>,
       pub fence_and_failsafe_verified: bool,
       pub manual_abort_procedure_rehearsed: bool,
       pub first_allowed_mission_type: Option<String>,
       pub single_drone_gate_passed: bool,
       pub multi_drone_review_required: bool,
   }
   ```

4. Mission families coverage:

   - Primitive: `takeoff-hold-land` минимум.
   - Urban single-drone: перед multi-drone.
   - Urban multi-drone: только после single-drone gate.
   - Swarm command-plane: только после Urban multi-drone gate.

5. `artifact_validator --mode hardware-entry-pack` проверяет:

   - `preflight_report.passed = true`;
   - `fc_contract_result` без blocking violations;
   - `readiness_status != Blocked`;
   - `first_allowed_mission_type` указан;
   - `single_drone_gate_passed = true` для multi-drone entry;
   - все `blockers` пусты для `ExecuteValidatedLocally` или выше.

6. Integration:

   - `sitl_agent --hardware-entry-pack --output-dir <dir>` генерирует пак.
   - Текущие `docs/HARDWARE_READINESS.md` и `docs/OPERATIONAL_RUNBOOKS.md`
     обновляются: ссылаются на M97 пак как machine-checkable основу.

### Non-Goals

- Нет real hardware flight в этом milestone.
- Нет production certification.
- Нет operator training substitute.
- Нет claim что SITL = safe hardware behavior.

### Done Criteria

- `HardwareEntryPack` генерируется для primitive mission.
- `HardwareEntryPack` генерируется для urban single-drone mission.
- `artifact_validator --mode hardware-entry-pack` проходит для valid pack.
- `artifact_validator` явно провалится при `readiness_status = Blocked`.
- `artifact_validator` явно провалится при missing `preflight_report`.
- `multi_drone_review_required = true` когда mission family = multi-drone.
- Docs обновлены: hardware entry без валидного пака явно запрещён.

### Automated Tests

#### Tests That Need No Refactoring

- `hardware_entry_pack_primitive_validates`.
- `hardware_entry_pack_urban_single_drone_validates`.
- `blocked_status_fails_validator`.
- `missing_preflight_fails_validator`.
- `missing_first_allowed_mission_fails_validator`.
- `multi_drone_pack_requires_single_drone_gate`.
- `hardware_entry_pack_serde_roundtrip`.
- `hardware_readiness_status_serde_roundtrip_all_variants`.
- `hardware_entry_checklist_serde_roundtrip`.

#### Tests That Need Light Refactoring

- `HardwareEntryPack` fixture builder.
- Common evidence-pack assertion helper.
- Docs smoke: hardware entry без пака явно запрещён.

#### Tests That Need Heavy Refactoring

- Current-head live-local evidence refresh pipeline.
- Schema compatibility tests across mission families.
- Replay-integrated evidence trace.

---

## Рекомендуемый первый slice

Если нужно выбрать один первый шаг (из C.26):

```text
1. M90: MavlinkPlanExecutor + MockAckProvider → takeoff-hold-land completes.
2. M90: execute_geofence_upload + execute_param_snapshot мокированы.
3. M91: SwarmMessage enum + Lease types → serde roundtrip тесты.
4. M92: UdpDroneLink loopback тест.
5. M93: GcsLostPolicy::ReturnToLaunch → partition test passes.
6. M95: SegmentCoordinator via InMemAgentTransport → no simultaneous hold.
```

Этот slice доказывает основную архитектурную цепочку без полного M91-M97.

## Ожидаемый уровень после M90–M97

После этого плана проект всё ещё не является:

- production drone system;
- certified safety stack;
- hardware-proven swarm controller;
- real RF mesh networking stack.

Но он станет серьёзным pre-hardware mission/supervisor/communication stack:

- `MavlinkCommonPlan` выполняется, не только компилируется;
- swarm protocol типизирован, lease-based, transport-agnostic;
- UDP транспорт позволяет запустить рой как N независимых процессов;
- агент работает автономно под лизом при потере GCS;
- partition не приводит к split-brain благодаря lease expiry;
- Urban coordination через настоящий network protocol;
- PX4 и ArduPilot имеют execute-time evidence на одной API;
- hardware entry — machine-checkable, не README-only.
