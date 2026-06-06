# DRONE_B.26 — Агент как автономная единица: протокол, FSM, multi-process

Дата фиксации: 2026-06-06

Источник: анализ реализации M80–M89, сравнение DRONE_B.25 и DRONE_C.25,
аудит текущего кода.

## Контекст

M80–M89 закрыли compile-time слой:

```text
Mission Command IR → MAVLink Common Compiler → Capability Profiles
→ FC Contract / Geofence / Params → Swarm Command Plane
→ Swarm Topologies → Dual-Stack Evidence
```

Всё это — планирование и компиляция на стороне GCS. Агент по-прежнему
не является автономной единицей: N агентов живут в одном процессе через
`ScenarioRunner`, деконфликтинг Urban-сегментов работает через shared
memory (`UrbanSegmentLockRegistry`), при потере GCS агент не имеет
собственной failsafe-логики, протокол общения ограничен CBBA/Gossip/Heartbeat
для allocation, но не покрывает mission-state, segment coordination и
network availability.

Следующий шаг — **агент как автономная единица**:

```text
каждый дрон = независимый процесс
  с собственным typed protocol (DroneMessage)
  с собственным FSM (AgentMissionState + failsafe)
  с pluggable transport (InMem → UDP → Serial → Mesh/LTE)
  с real deconfliction через network, не через shared memory
```

Этот слой строится поверх существующего `Transport` trait и `AgentNode<T>`.
Никакого нового железа не требуется: UDP — это localhost.

## Архитектурная граница

Проект по-прежнему НЕ реализует:

- flight controller, стабилизацию, управление моторами;
- реальный радио-стек (mesh firmware, RF propagation);
- certified collision avoidance;
- производственную сертификацию безопасности.

Проект реализует:

- typed protocol между агентами, independent of transport;
- autonomous agent FSM с конфигурируемыми failsafe-политиками;
- pluggable transport (InMem / UDP / placeholder Serial);
- Urban deconfliction через network messages вместо shared memory;
- execution loop для MavlinkCommonPlan с mock ACK provider (dry-run).

Ключевое правило транспорта:

```text
Конкретная сеть (mesh, LTE, Wi-Fi, serial) — это деталь реализации.
DroneLink / Transport — это стабильный контракт.
Протокол сообщений не должен знать, по какому каналу они идут.
```

## Non-Goals

- Не реализовывать RF mesh firmware или antenna model.
- Не делать distributed consensus с гарантиями (BFT, Raft).
- Не реализовывать real-time formation control без telemetry loop.
- Не делать полный MAVLink transport от нуля — он уже есть
  за `mavlink-transport` feature flag.
- Не делать PX4/ArduPilot SDK интеграцию.
- Не делать hardware serial без конкретного железа.
- Не делать multi-level carrier hierarchy (mothership of motherships).

## Milestone Chain

```text
M89 Dual-Stack Evidence Pack (завершён)
  -> M90 Agent Communication Protocol
    -> M91 Autonomous Agent FSM
      -> M92 Multi-Process Swarm (UDP DroneLink)
        -> M93 Urban Execution Pipeline
```

---

## M90 — Agent Communication Protocol

### Goal

Определить полный типизированный протокол общения дронов — `DroneMessage` —
который покрывает все сценарии agent-to-agent и agent-to-GCS коммуникации
независимо от транспорта.

Текущий `RuntimeMessage` (Heartbeat / Gossip / Cbba) решает только задачу
distributed task allocation. Для реальной работы дронов нужно:

- синхронизация mission state между агентами и GCS;
- сегментная координация (reserve / grant / deny / release) через сеть, не через
  shared memory;
- health и network availability reporting;
- команды от GCS / mothership к агентам с ACK;
- state reconciliation после потери и восстановления связи.

### Scope

1. Core message type:

   ```rust
   /// Schema version for drone-to-drone protocol messages.
   pub const DRONE_PROTOCOL_SCHEMA_VERSION: &str = "drone_protocol.v1";

   #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
   #[serde(tag = "kind", rename_all = "snake_case")]
   pub enum DroneMessage {
       // Health and presence
       Heartbeat {
           sender_tick: u64,
           generation: u64,
           mission_state: AgentMissionState,
       },
       StatusReport {
           mission_state: AgentMissionState,
           position: Option<CommandPosition>,
           current_segment: Option<UrbanEdgeId>,
           gcs_reachable: bool,
           tick: u64,
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
           granted_at_tick: u64,
       },
       SegmentDeny {
           edge_id: UrbanEdgeId,
           to: AgentId,
           holder: AgentId,
           reason: SegmentDenyReason,
       },
       SegmentRelease {
           edge_id: UrbanEdgeId,
           released_at_tick: u64,
       },

       // Mission commands (GCS / mothership → agent)
       MissionAssign {
           plan_id: String,
           plan: MissionCommandPlan,
           assigned_at_tick: u64,
       },
       MissionAbort {
           reason: String,
           abort_action: AbortAction,
       },
       MissionAck {
           plan_id: String,
           accepted: bool,
           note: Option<String>,
       },

       // GCS / network availability
       GcsAvailable {
           tick: u64,
       },
       GcsUnavailable {
           last_contact_tick: u64,
       },
       NeighborAlive {
           agent_id: AgentId,
           tick: u64,
       },
       NeighborLost {
           agent_id: AgentId,
           last_seen_tick: u64,
       },

       // State reconciliation after reconnect
       StateRequest {
           from: AgentId,
       },
       StateResponse {
           mission_state: AgentMissionState,
           completed_segments: Vec<UrbanEdgeId>,
           active_locks: Vec<UrbanEdgeId>,
           last_tick: u64,
       },
   }
   ```

2. Agent mission state:

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(tag = "state", rename_all = "snake_case")]
   pub enum AgentMissionState {
       Idle,
       WaitingForMission,
       ExecutingSegment {
           segment_id: UrbanEdgeId,
           started_at_tick: u64,
       },
       WaitingForSegment {
           edge_id: UrbanEdgeId,
           blocked_by: AgentId,
           since_tick: u64,
       },
       Replanning {
           reason: ReplanReason,
       },
       GcsLost {
           since_tick: u64,
           policy_engaged: String,
       },
       Aborting {
           reason: String,
       },
       Completed {
           mission_id: String,
           finished_at_tick: u64,
       },
       Failed {
           reason: String,
       },
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
   pub enum ReplanReason {
       SegmentBlocked,
       MissionReassigned,
       GcsCommand,
   }
   ```

3. Message envelope с версионированием:

   ```rust
   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct DroneMessageEnvelope {
       pub schema_version: String,    // "drone_protocol.v1"
       pub envelope_id: String,       // UUID или tick-based
       pub from: AgentId,
       pub to: AgentId,
       pub sent_at_tick: u64,
       pub message: DroneMessage,
   }

   impl DroneMessageEnvelope {
       pub fn into_raw_message(self) -> RawMessage {
           RawMessage {
               from: self.from.clone(),
               to: self.to.clone(),
               payload: serde_json::to_vec(&self).unwrap_or_default(),
           }
       }

       pub fn from_raw_message(raw: &RawMessage) -> Option<Self> {
           serde_json::from_slice(&raw.payload).ok()
       }
   }
   ```

4. Размещение:

   - `DroneMessage`, `AgentMissionState`, `SegmentDenyReason`, `ReplanReason`
     → новый модуль `swarm-comms/src/drone_protocol.rs`.
   - `DroneMessageEnvelope` — там же.
   - Экспортируется из `swarm-comms::drone_protocol`.
   - Существующий `RuntimeMessage` остаётся в `swarm-runtime` для CBBA/Gossip;
     `DroneMessage` — mission-level протокол, оба могут сосуществовать.

5. Replay events:

   - `DroneProtocolMessage { envelope_id, from, to, kind, tick }` —
     новый вариант в `swarm-replay::Event`;
   - позволяет восстановить полную картину коммуникации из replay log.

### Non-Goals

- Нет transport implementation в этом milestone.
- Нет execution logic (только protocol types).
- Нет backward-compat breaking изменений в `RuntimeMessage`.
- Нет distributed consensus поверх `DroneMessage`.

### Done Criteria

- `DroneMessage` и `AgentMissionState` определены, serde snake_case, без потерь.
- `DroneMessageEnvelope` кодируется в `RawMessage` и декодируется обратно.
- Все варианты `AgentMissionState` покрыты тестами serde roundtrip.
- `DroneProtocolMessage` replay event добавлен в `swarm-replay`.
- `docs/DRONE_PROTOCOL.md` объясняет назначение каждой группы сообщений
  и разницу между `RuntimeMessage` (allocation) и `DroneMessage` (mission).

### Automated Tests

#### Tests That Need No Refactoring

- `drone_message_serde_roundtrip_all_variants`: каждый вариант
  сериализуется/десериализуется без потерь.
- `agent_mission_state_serde_roundtrip_all_variants`.
- `segment_deny_reason_serde_roundtrip`.
- `drone_message_envelope_into_raw_and_back`: `into_raw_message` →
  `from_raw_message` возвращает исходный envelope.
- `drone_message_envelope_unknown_schema_returns_none`: payload с другим
  schema_version gracefully возвращает `None`.
- `segment_reserve_grant_deny_release_ordering`: последовательность Reserve →
  Grant → Release через `InMemNetwork` сохраняет порядок при latency=0.
- `mission_assign_ack_serde_roundtrip`.
- `state_response_contains_all_active_locks`.
- `replay_event_drone_protocol_message_emitted`.
- `drone_protocol_schema_version_constant_matches_doc`.

#### Tests That Need Light Refactoring

- Вспомогательный builder для DroneMessageEnvelope в тестах.
- InMemNetwork fixture с двумя агентами для DroneMessage exchange тестов.
- Replay assertion helper для DroneProtocolMessage events.

#### Tests That Need Heavy Refactoring

- Schema versioning migration (drone_protocol.v1 → v2) roundtrip.
- DroneMessage fuzz test (arbitrary payload → graceful None, не panic).
- Property test: любая последовательность Reserve/Grant/Release сохраняет
  инварианты (`edge_id` и `from/to` консистентны).

---

## M91 — Autonomous Agent FSM

### Goal

Каждый агент имеет собственный state machine с конфигурируемыми
failsafe-политиками для потери GCS, потери mothership и потери соседей.

Это ключевой элемент "реального дронового кода": агент не замирает при
потере связи, а применяет заранее сконфигурированную политику. GCS может
восстановить связь и получить `StateResponse` с тем, что агент сделал пока
был недоступен.

```text
GCS недоступен T тиков
  -> применить GcsLostPolicy
  -> ContinueMission | HoverInPlace | ReturnToLaunch | AbortImmediate

GCS восстановлен
  -> StateRequest → StateResponse (completed_segments, active_locks)
  -> GCS принимает решение: continue / reassign / merge
```

### Scope

1. Failsafe policies:

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "kind")]
   pub enum GcsLostPolicy {
       /// Continue executing the current plan until it completes or another
       /// condition triggers abort. Safe only for deterministic, bounded missions.
       ContinueMission {
           /// Maximum ticks without GCS before escalating to ReturnToLaunch.
           max_gcs_lost_ticks: u64,
       },
       /// Halt at current position. Resume when GCS returns.
       HoverInPlace {
           max_gcs_lost_ticks: u64,
       },
       /// Initiate RTL after the configured number of ticks without GCS.
       ReturnToLaunch {
           after_ticks: u64,
       },
       /// Immediately abort and RTL on any GCS loss event.
       AbortImmediate,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "kind")]
   pub enum MothershipLostPolicy {
       /// Wait at staging waypoint until mothership reconnects.
       WaitAtStaging {
           max_ticks: u64,
       },
       /// Proceed with own mission autonomously (only if plan is self-contained).
       ProceedAutonomously,
       /// Return to launch immediately.
       ReturnToLaunch,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case", tag = "kind")]
   pub enum NeighborLostPolicy {
       /// Release segment locks held by the lost agent and allow others to proceed.
       ReleaseLocksAndContinue,
       /// Wait for the neighbor to reconnect before taking its locks.
       WaitForReconnect { max_ticks: u64 },
       /// Treat neighbor loss as a mission-level abort trigger.
       AbortMission,
   }
   ```

2. Agent autonomy config (добавляется в `RunConfig`):

   ```rust
   #[derive(Clone, Debug, Default, Serialize, Deserialize)]
   pub struct AgentAutonomyConfig {
       /// How to react when GCS heartbeat is lost.
       #[serde(default)]
       pub gcs_lost_policy: GcsLostPolicy,
       /// How child agents react when mothership link is lost.
       #[serde(default)]
       pub mothership_lost_policy: MothershipLostPolicy,
       /// How to react when a neighbor agent is lost (for deconfliction).
       #[serde(default)]
       pub neighbor_lost_policy: NeighborLostPolicy,
       /// Ticks of silence before declaring GCS lost.
       #[serde(default = "default_gcs_heartbeat_timeout")]
       pub gcs_heartbeat_timeout_ticks: u64,
       /// Ticks of silence before declaring a peer agent lost.
       #[serde(default = "default_peer_heartbeat_timeout")]
       pub peer_heartbeat_timeout_ticks: u64,
   }
   ```

3. FSM transitions в `AgentNode<T>`:

   Каждый `tick()` агент:
   a. Проверяет `last_gcs_heartbeat_tick` — если превышен порог, переходит
      в `AgentMissionState::GcsLost` и применяет `GcsLostPolicy`.
   b. Проверяет `last_peer_heartbeat_ticks` по каждому peer — если превышен
      порог, эмитирует `NeighborLost` event и применяет `NeighborLostPolicy`.
   c. При восстановлении GCS (получен `GcsAvailable`):
      - отправляет `StateResponse` с `completed_segments` и `active_locks`;
      - переходит из `GcsLost` обратно в `ExecutingSegment` или
        `WaitingForMission` в зависимости от `mission_state`.

4. Reconciliation report:

   ```rust
   /// Summary of what an agent did while GCS was unavailable.
   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct StateReconcileReport {
       pub agent_id: AgentId,
       pub gcs_lost_ticks: u64,
       pub policy_applied: String,
       pub completed_segments: Vec<UrbanEdgeId>,
       pub active_locks_at_reconnect: Vec<UrbanEdgeId>,
       pub mission_state_at_reconnect: AgentMissionState,
   }
   ```

5. Replay events (добавляются в `swarm-replay::Event`):

   - `AgentGcsLost { agent_id, tick, policy }`;
   - `AgentGcsReconnected { agent_id, tick, gcs_lost_ticks }`;
   - `AgentNeighborLost { agent_id, lost_neighbor_id, tick }`;
   - `AgentFailsafePolicyEngaged { agent_id, policy, tick }`;
   - `AgentStateReconciled { agent_id, tick, report }`.

6. Integration:

   - `RunConfig` получает поле `autonomy: AgentAutonomyConfig`.
   - `ScenarioRunner` передаёт `AgentAutonomyConfig` в `AgentNode`.
   - Существующие `partition_events` в `RunConfig` используются для
     инъекции GCS-loss сценариев в тестах.
   - Метрики: `gcs_lost_count`, `gcs_lost_total_ticks`, `neighbor_lost_count`,
     `failsafe_policy_rtl_count` добавляются в `RunMetrics`.

### Non-Goals

- Нет real MAVLink RTL команды без живого FC.
- Нет distributed consensus при одновременной потере нескольких агентов.
- Нет certified failsafe behaviour.
- Нет изменений в существующих CBBA/Gossip тестах.

### Done Criteria

- `GcsLostPolicy`, `MothershipLostPolicy`, `NeighborLostPolicy` определены,
  serde snake_case.
- `AgentAutonomyConfig` добавлен в `RunConfig`, существующие сценарии
  десериализуются без изменений (все поля `#[serde(default)]`).
- Агент с `GcsLostPolicy::ReturnToLaunch { after_ticks: 10 }` переходит
  в `GcsLost` при partition на 10+ тиков.
- `StateReconcileReport` генерируется при reconnect.
- Replay log содержит `AgentGcsLost` и `AgentGcsReconnected` события.
- Существующие сценарии без `autonomy` поля проходят без изменений.

### Automated Tests

#### Tests That Need No Refactoring

- `gcs_lost_policy_rtl_engages_after_threshold`: partition на N тиков →
  `AgentMissionState::GcsLost` с `policy_engaged = "return_to_launch"`.
- `gcs_lost_policy_continue_mission_does_not_abort_before_threshold`.
- `gcs_lost_policy_abort_immediate_transitions_on_first_lost_tick`.
- `gcs_reconnect_emits_state_reconcile_report`.
- `state_reconcile_report_contains_completed_segments`.
- `neighbor_lost_policy_releases_locks_on_peer_timeout`.
- `neighbor_lost_policy_wait_does_not_release_before_timeout`.
- `mothership_lost_policy_proceed_autonomously_continues_mission`.
- `mothership_lost_policy_wait_at_staging_holds`.
- `replay_contains_agent_gcs_lost_event`.
- `replay_contains_agent_gcs_reconnected_event`.
- `gcs_lost_total_ticks_metric_is_non_zero_after_partition`.
- `existing_scenarios_load_without_autonomy_field`.

#### Tests That Need Light Refactoring

- Fixture builder для сценариев с partition + `AgentAutonomyConfig`.
- Replay assertion helper для `AgentGcsLost` / `AgentGcsReconnected`.
- `PartitionEvent` helper: GCS partition → reconnect cycle.

#### Tests That Need Heavy Refactoring

- Property test: случайные partition/reconnect на 3 агентах → инварианты
  состояний сохраняются (нет Invalid transitions).
- Stress test: 8 агентов, GCS drop на 50 тиков, reconnect, reconcile →
  все агенты corrrectly reconcile.
- Mothership + 2 sub-agents: mothership loss → sub-agents wait → reconnect.

---

## M92 — Multi-Process Swarm (UDP DroneLink)

### Goal

Реализовать `UdpDroneLink` — вторую реализацию `Transport` trait,
работающую через UDP между процессами.

После этого milestone каждый агент запускается как **отдельный
Rust-процесс**. Несколько терминалов = несколько "дронов" на localhost.
Это первый шаг к реальному multi-machine deployment без железа.

```text
До M92:  1 процесс = N агентов в shared memory
После:   1 процесс = 1 агент, N процессов = N дронов
```

Когда появится реальное железо, companion computer на каждом дроне
запускает тот же agent binary — меняется только конфиг, не код.

### Scope

1. `UdpDroneLink`:

   ```rust
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
       #[error("io error: {0}")]
       Io(#[from] std::io::Error),
       #[error("unknown peer: {0}")]
       UnknownPeer(AgentId),
       #[error("payload too large: {0} bytes")]
       PayloadTooLarge(usize),
   }
   ```

   - `send()`: ищет `SocketAddr` по `msg.to`, UDP sendto, не блокирует.
   - `poll()`: nonblocking `recv_from`, одно сообщение за вызов,
     буферизует остаток в `recv_buffer`.
   - Максимальный размер payload: 65507 байт (UDP MTU). При превышении →
     `PayloadTooLarge`. Большие сообщения (MissionAssign с планом) →
     будущий фрагментационный слой или TCP fallback; пока явная ошибка.
   - Неизвестный `to` → `UnknownPeer`, не panic.

2. `SerialDroneLink` — placeholder:

   ```rust
   pub struct SerialDroneLink {
       own_id: AgentId,
   }

   impl Transport for SerialDroneLink {
       type Error = SerialDroneLinkError;
       fn send(&mut self, _msg: RawMessage) -> Result<(), SerialDroneLinkError> {
           Err(SerialDroneLinkError::NotImplemented)
       }
       fn poll(&mut self) -> Result<Option<RawMessage>, SerialDroneLinkError> {
           Err(SerialDroneLinkError::NotImplemented)
       }
   }
   ```

3. `NullDroneLink` — для тестов, отбрасывает все сообщения:

   ```rust
   pub struct NullDroneLink { pub own_id: AgentId }
   impl Transport for NullDroneLink {
       type Error = Infallible;
       fn send(&mut self, _msg: RawMessage) -> Result<(), Infallible> { Ok(()) }
       fn poll(&mut self) -> Result<Option<RawMessage>, Infallible> { Ok(None) }
   }
   ```

4. `DroneLinkConfig` в Scenario DSL:

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
       /// Serial port placeholder, returns NotImplemented.
       Serial {
           path: String,
           baud: u32,
       },
   }
   ```

   Добавляется в `RunConfig`:
   ```rust
   #[serde(default)]
   pub drone_link: DroneLinkConfig,
   ```

5. Agent binary entry point (`swarm-examples/src/bin/drone_agent.rs`):

   Минимальный binary: читает конфиг из файла, строит `AgentNode<UdpDroneLink>`,
   запускает tick loop. Пока без real MAVLink execution (dry-run mode):

   ```text
   drone_agent --config agent-0.json --agent-id agent-0
   ```

   Конфиг содержит: `drone_link`, `autonomy`, `assigned_plan` (если есть),
   `max_ticks`. Подходит для запуска в нескольких терминалах.

6. Loopback integration test:

   Запускает два `UdpDroneLink` в двух потоках на `127.0.0.1:0` (OS-assigned
   ports), отправляет N сообщений, проверяет что все доставлены.

### Non-Goals

- Нет шифрования или аутентификации UDP пакетов.
- Нет TCP fallback для больших сообщений (явная ошибка на > MTU).
- Нет fragmentation/reassembly.
- Нет мультикаст/бродкаст (только unicast).
- Нет stable public API для `drone_agent` binary.
- Нет real serial port (SerialDroneLink возвращает NotImplemented).

### Done Criteria

- `UdpDroneLink` компилируется и реализует `Transport`.
- Loopback тест: 2 потока, N сообщений, все доставлены.
- `UnknownPeer` возвращается при отправке неизвестному агенту, не panic.
- `PayloadTooLarge` возвращается при превышении UDP MTU.
- `SerialDroneLink` компилируется, возвращает `NotImplemented`.
- `NullDroneLink` компилируется, отбрасывает всё.
- `DroneLinkConfig` сериализуется snake_case, добавлен в `RunConfig`.
- Существующие сценарии с `Simulated` конфигом (default) не ломаются.
- `drone_agent` binary компилируется и запускается в dry-run режиме.
- `UdpDroneLink` loopback тест: агент-0 → агент-1, доставлено.
- `CBBA` тесты проходят без изменений с `InMemAgentTransport`.
- `docs/DRONE_LINK.md`: объясняет 4 реализации и как добавить новую.

### Automated Tests

#### Tests That Need No Refactoring

- `udp_drone_link_loopback_send_recv`: два агента, один отправляет, другой
  получает через UDP loopback.
- `udp_drone_link_unknown_peer_returns_error`: `send` к неизвестному id →
  `UnknownPeer`, не panic.
- `udp_drone_link_payload_too_large_returns_error`.
- `null_drone_link_send_drops_silently`.
- `null_drone_link_poll_returns_none`.
- `serial_drone_link_send_returns_not_implemented`.
- `serial_drone_link_poll_returns_not_implemented`.
- `drone_link_config_serde_roundtrip_simulated`.
- `drone_link_config_serde_roundtrip_udp`.
- `drone_link_config_serde_roundtrip_serial`.
- `drone_link_config_default_is_simulated`.
- `existing_scenarios_load_with_default_drone_link`.

#### Tests That Need Light Refactoring

- `cbba_converges_over_udp_drone_link`: два потока, CBBA сходится через
  реальный UDP.
- DroneLink test harness (абстрагирует port allocation).
- `drone_agent` CLI smoke test: `--config` → dry-run запускается.

#### Tests That Need Heavy Refactoring

- CBBA convergence под симулированным пакетными потерями через `tc netem`.
- Multi-agent drone_agent: 3 терминальных процесса, DroneMessage exchange.
- Chaos test: произвольные delays и drops → M91 FSM корректно реагирует.

---

## M93 — Urban Execution Pipeline

### Goal

Замкнуть Urban pipeline:

```text
Urban mission plan
  → MissionCommandPlan (M80)
    → MavlinkCommonPlan (M81/M86)
      → step-by-step execution loop (dry-run / mock ACK / live FC)
        → per-step telemetry milestones
          → evidence artifact
```

И заменить Urban deconfliction через shared memory (`UrbanSegmentLockRegistry`)
на network-level координацию через `DroneMessage` протокол (M90): агенты
отправляют `SegmentReserve`, `SegmentGrant`, `SegmentDeny`, `SegmentRelease`
через `Transport`, а не через общий объект в памяти.

### Scope

1. Execution loop для `MavlinkCommonPlan`:

   ```rust
   pub struct MavlinkPlanExecutor<T: Transport> {
       transport: T,
       ack_provider: Box<dyn AckProvider>,
   }

   /// Provides ACKs for plan steps (mock for dry-run, real for SITL/hardware).
   pub trait AckProvider {
       fn ack_command(
           &mut self,
           command: &MavlinkCommonCommand,
       ) -> MavlinkExecutionStepResult;

       fn ack_mission_upload(&mut self) -> MavlinkExecutionStepResult;
   }

   #[derive(Clone, Debug, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum MavlinkExecutionStepResult {
       Accepted,
       Rejected  { reason: String },
       Timeout   { after_ticks: u64 },
       Skipped   { reason: String },
   }

   pub struct MavlinkPlanExecutionReport {
       pub plan_id: String,
       pub steps: Vec<MavlinkExecutionStep>,
       pub overall: MavlinkExecutionOutcome,
       pub telemetry_milestones_reached: Vec<MavlinkTelemetryMilestoneKind>,
   }

   #[derive(Clone, Debug, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum MavlinkExecutionOutcome {
       Completed,
       Aborted  { at_step: usize, reason: String },
       Failed   { at_step: usize, reason: String },
   }
   ```

   - `MockAckProvider` — принимает все команды, все upload фазы.
     Используется для dry-run без FC.
   - `ScriptedAckProvider` — принимает список ожидаемых команд с
     заданными результатами. Используется в детерминированных тестах.

2. Segment coordinator (заменяет shared memory):

   Один агент (или GCS) берёт роль `SegmentCoordinator`.
   Остальные агенты отправляют `DroneMessage::SegmentReserve` координатору.
   Координатор отвечает `SegmentGrant` или `SegmentDeny`.

   ```rust
   pub struct SegmentCoordinator {
       /// key: `UrbanEdgeId`
       active_locks: HashMap<UrbanEdgeId, UrbanSegmentLock>,
       policy: UrbanRightOfWayPolicy,
       /// key: `AgentId`
       priorities: HashMap<AgentId, u8>,
   }

   impl SegmentCoordinator {
       pub fn handle_reserve(
           &mut self,
           request: &DroneMessage,
           tick: u64,
       ) -> DroneMessage; // SegmentGrant or SegmentDeny

       pub fn handle_release(
           &mut self,
           edge_id: &UrbanEdgeId,
           agent_id: &AgentId,
       );
   }
   ```

   Существующий `UrbanSegmentLockRegistry` остаётся для backward
   compatibility в single-process scenarios — `SegmentCoordinator`
   является его network-facing аналогом.

3. Urban evidence artifact (расширяет dry-run артефакт):

   ```rust
   pub struct UrbanExecutionEvidence {
       pub schema_version: String,   // "urban_execution_evidence.v1"
       pub mission_id: String,
       pub git_commit: String,
       pub created_at: DateTime<Utc>,
       pub urban_route_summary: UrbanRouteSummary,
       pub command_ir_summary: CommandIrSummary,
       pub mavlink_plan_summary: MavlinkPlanSummary,
       pub execution_report: MavlinkPlanExecutionReport,
       pub deconfliction_mode: DeconflictionMode,
       pub segment_coordination_events: Vec<SegmentCoordinationEvent>,
       pub preflight_report: SafetyValidationReport,
       pub caveats: Vec<String>,
   }

   #[derive(Clone, Debug, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum DeconflictionMode {
       /// Single-process shared memory (backward compat).
       SharedMemory,
       /// Network-level protocol (M90 DroneMessage).
       NetworkProtocol { coordinator_id: AgentId },
   }
   ```

4. Integration:

   - `sitl_agent --dry-run --urban --execution-evidence` генерирует
     `urban_execution_evidence.v1.json`.
   - `artifact_validator --mode urban-execution-evidence` валидирует
     структуру артефакта.
   - Для single-process backward compat: `DeconflictionMode::SharedMemory`
     с существующим `UrbanSegmentLockRegistry`.
   - Для network mode: `SegmentCoordinator` через `UdpDroneLink` или
     `InMemAgentTransport`.

5. Urban scenario fixtures:

   - `scenarios/urban.geo-block-loop.execution.json` — гео-референсная
     блок-патруль миссия с execution evidence;
   - `scenarios/urban.multi-agent-deconflict.network.json` — два агента,
     сегментная координация через `DroneMessage`.

### Non-Goals

- Нет live flight execution без реального FC.
- Нет real telemetry loop (это M43–M48 layer).
- Нет certified geofence enforcement.
- Нет полного GIS/OSM парсера.
- Нет certified collision avoidance.
- Нет замены `UrbanSegmentLockRegistry` — только дополнение.

### Done Criteria

- `MockAckProvider` принимает все команды, `ScriptedAckProvider` —
  только заданные.
- `MavlinkPlanExecutor` выполняет `takeoff-hold-land` plan с mock ACK
  и возвращает `Completed`.
- `MavlinkPlanExecutor` возвращает `Aborted` при первом Timeout.
- `SegmentCoordinator` корректно отвечает Grant/Deny на Reserve запросы.
- Два агента через `InMemAgentTransport` не занимают один сегмент
  одновременно при network-level координации.
- `UrbanExecutionEvidence` генерируется и валидируется.
- `artifact_validator --mode urban-execution-evidence` проверяет
  `deconfliction_mode`, `execution_report.overall`, `preflight_report`.
- Существующие Urban сценарии не регрессируют.

### Automated Tests

#### Tests That Need No Refactoring

- `mock_ack_provider_accepts_all_commands`.
- `scripted_ack_provider_returns_configured_result`.
- `executor_completes_takeoff_hold_land_with_mock_ack`.
- `executor_aborts_on_first_timeout`.
- `executor_skips_unsupported_feature_with_caveat`.
- `segment_coordinator_grants_first_request`.
- `segment_coordinator_denies_concurrent_request_to_held_segment`.
- `segment_coordinator_grants_after_release`.
- `two_agents_via_inmem_no_simultaneous_segment_hold`.
- `urban_execution_evidence_serde_roundtrip`.
- `urban_execution_evidence_validates`.
- `deconfliction_mode_network_records_coordinator_id`.
- `shared_memory_deconfliction_backward_compat`.

#### Tests That Need Light Refactoring

- Mock AckProvider fixture builder.
- SegmentCoordinator test harness с `InMemAgentTransport`.
- `artifact_validator` urban-execution-evidence mode integration test.
- Urban geo scenario + execution evidence assertion helper.

#### Tests That Need Heavy Refactoring

- Two-agent execution over `UdpDroneLink`: независимые процессы,
  segment coordination через UDP, evidence содержит оба агента.
- ScenarioRunner → execution evidence pipeline end-to-end.
- Stress test: 8 агентов, random segment order, network-level coordinator,
  no simultaneous holds.

---

## Ожидаемый уровень после M90–M93

После этого плана проект всё ещё не является:

- production drone system;
- certified safety stack;
- hardware-proven swarm controller;
- real RF mesh networking stack;
- system ready for uncontrolled field use.

Но он станет значительно ближе к "боевому" коду:

- каждый агент — независимый процесс с собственным agent loop;
- typed protocol покрывает все сценарии drone-to-drone общения;
- failsafe FSM конфигурируется и тестируется под partition injection;
- UDP транспорт позволяет запустить рой на нескольких машинах / терминалах
  без изменения алгоритмов;
- Urban deconfliction работает через настоящий network protocol;
- execution loop замыкает цепочку от Mission IR до step-by-step evidence.

Когда появится железо, следующий шаг очевиден:

```text
drone_agent binary + UdpDroneLink
  → заменить bind_addr на реальный IP companion computer
    → connect UdpDroneLink к MavlinkTransport на том же агенте
      → первый полёт одного дрона с autonomous FSM
        → multi-drone только после отдельного safety review
```

## Что не делать в этом плане

- Не реализовывать RF mesh без конкретного radio-железа.
- Не делать distributed consensus (BFT, Paxos, Raft).
- Не делать real-time formation control без telemetry loop.
- Не добавлять новые benchmark reruns без новых behaviour или новых
  interpretation вопросов.
- Не обещать semver-stable API до стабилизации agent/transport boundaries.
- Не переходить к multi-drone hardware до single-drone controlled experiment.
- Не реализовывать фрагментацию UDP пакетов (явная ошибка при > MTU,
  future work).
