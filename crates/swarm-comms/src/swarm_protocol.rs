/// Transport-agnostic typed swarm coordination protocol (M91).
///
/// Defines message types, lease semantics, envelope serialisation, and
/// duplicate suppression for drone / mothership / GCS coordination.
use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use derive_more::{AsRef, Deref, DerefMut, From, Into};
use serde::{Deserialize, Serialize};
use swarm_mission_ir::MissionCommandPlan;
use swarm_types::{AgentId, UrbanEdgeId};

use crate::mavlink_executor::MavlinkExecutionOutcome;
use crate::transport::RawMessage;

/// Schema version embedded in every `SwarmMessageEnvelope`.
pub const SWARM_PROTOCOL_SCHEMA_VERSION: &str = "swarm_protocol.v1";

// ─── Lease types ──────────────────────────────────────────────────────────────

/// Opaque identifier for an authority lease.
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, AsRef, Deref, DerefMut, From, Into,
)]
pub struct LeaseId(String);

/// Time-bounded exclusive authority over a named resource.
///
/// Prevents split-brain: when a lease expires the holder loses authority
/// deterministically, regardless of GCS availability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub lease_id: LeaseId,
    pub holder: AgentId,
    /// Identifies the guarded resource (task id, edge id, sector id, …).
    pub resource_id: String,
    /// Category of the resource: `"task"`, `"edge"`, `"sector"`, etc.
    pub resource_kind: String,
    pub granted_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl Lease {
    /// Returns `true` if the lease is still valid at the given instant.
    pub fn is_valid_at(&self, now: DateTime<Utc>) -> bool {
        now < self.expires_at
    }
}

// ─── Supporting value types ───────────────────────────────────────────────────

/// Geographic position used in mission commands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandPosition {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub alt_m: f64,
}

/// Recovery action executed when a mission is aborted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbortAction {
    ReturnToLaunch,
    HoverInPlace,
    Land,
    Continue,
}

/// Protocol-level agent role.
///
/// Mirrors `swarm_command_plane::SwarmCommandRole`; defined here to avoid a
/// circular crate dependency (`swarm-command-plane` → `swarm-comms`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolRole {
    Scout,
    Observer,
    Relay,
    Leader,
    Coordinator,
    Mothership,
    Carrier,
    Reserve,
    Recovery,
}

// ─── State enums ──────────────────────────────────────────────────────────────

/// Agent-level mission execution state with lease-aware variants.
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
    /// Agent has suspended mission progress while waiting for a lost peer to
    /// reconnect before a configured timeout.
    WaitingForNeighborReconnect {
        neighbor_id: AgentId,
        since_tick: u64,
        until_tick: u64,
    },
    /// Agent continues mission autonomously while GCS is unreachable;
    /// valid only while the lease has not expired.
    ContinuingUnderLease {
        lease_id: LeaseId,
        lease_expires_at: DateTime<Utc>,
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
        resource_id: String,
        finished_at_tick: u64,
    },
    Failed {
        reason: String,
    },
}

/// Reason an agent rejects a mission offer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionRejectReason {
    Overloaded,
    IncompatibleRole,
    LeaseExpired,
    DuplicateOffer,
}

/// Reason for releasing a lease.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseReason {
    Completed,
    Aborted,
    LeaseExpired,
    AgentFailed,
    Reassigned,
}

/// Reason for denying a segment reservation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentDenyReason {
    AlreadyHeld,
    PolicyDenied,
    CoordinatorUnavailable,
}

/// Reason an agent enters degraded mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DegradedReason {
    GcsUnavailable,
    CoordinatorUnavailable,
    MothershipUnavailable,
    PartitionDetected,
    LeaseExpirySoon,
}

/// Reason an agent initiates replanning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplanReason {
    SegmentBlocked,
    MissionReassigned,
    GcsCommand,
    LeaseExpired,
}

// ─── SwarmMessage ─────────────────────────────────────────────────────────────

/// All messages exchanged in the swarm coordination protocol.
///
/// Each variant is serialised with a `"kind"` discriminant in snake_case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SwarmMessage {
    // ── Presence and health ───────────────────────────────────────────────
    Heartbeat {
        tick: u64,
        generation: u64,
        mission_state: AgentMissionState,
    },
    /// Agent announces its role and capabilities.
    ///
    /// `role` mirrors `SwarmCommandRole` from `swarm-command-plane`;
    /// defined locally to avoid a circular crate dependency.
    Presence {
        role: ProtocolRole,
        capabilities: Vec<String>,
    },

    // ── Mission assignment lifecycle ──────────────────────────────────────
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

    // ── Ownership and lease management ────────────────────────────────────
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

    // ── Segment coordination ──────────────────────────────────────────────
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

    // ── Progress and status ───────────────────────────────────────────────
    ProgressUpdate {
        resource_id: String,
        /// 0..=100
        progress_pct: u8,
        position: Option<CommandPosition>,
        tick: u64,
    },
    ReplacementOffer {
        for_resource_id: String,
        plan: MissionCommandPlan,
    },

    // ── Supervisor signals ────────────────────────────────────────────────
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

    // ── State reconciliation ──────────────────────────────────────────────
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

// ─── Envelope ─────────────────────────────────────────────────────────────────

/// Transport envelope for a `SwarmMessage`.
///
/// Encodes routing (`from`, `to`), deduplication (`envelope_id`),
/// request/response linkage (`correlation_id`), schema versioning, and
/// a liveness hint (`ttl_ticks`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SwarmMessageEnvelope {
    /// Must equal `SWARM_PROTOCOL_SCHEMA_VERSION` for accepted messages.
    pub schema_version: String,
    /// Unique identifier for this envelope; used for duplicate suppression.
    pub envelope_id: String,
    /// Links a response back to the originating request envelope.
    pub correlation_id: Option<String>,
    pub from: AgentId,
    pub to: AgentId,
    pub sent_at: DateTime<Utc>,
    /// Maximum simulation ticks to hold the message before discarding.
    pub ttl_ticks: u32,
    pub message: SwarmMessage,
}

impl SwarmMessageEnvelope {
    /// Serialises this envelope into a `RawMessage`.
    ///
    /// Routing fields (`from`, `to`) are copied into the `RawMessage` header
    /// and also retained inside the JSON payload.
    pub fn into_raw_message(self) -> RawMessage {
        let payload =
            serde_json::to_vec(&self).expect("SwarmMessageEnvelope serialisation must not fail");
        RawMessage {
            from: self.from,
            to: self.to,
            payload,
        }
    }

    /// Deserialises from a `RawMessage`.
    ///
    /// Returns `None` for unknown schema versions or malformed payloads
    /// without panicking.
    pub fn from_raw_message(raw: &RawMessage) -> Option<Self> {
        let env: Self = serde_json::from_slice(&raw.payload).ok()?;
        if env.schema_version == SWARM_PROTOCOL_SCHEMA_VERSION {
            Some(env)
        } else {
            None
        }
    }
}

// ─── Duplicate suppressor ─────────────────────────────────────────────────────

/// Sliding-window duplicate suppressor keyed on `envelope_id`.
///
/// Silently drops envelopes whose `envelope_id` was already seen within the
/// window; does not panic or return an error.
pub struct DuplicateSuppressor {
    /// Ring buffer of seen envelope IDs in arrival order.
    seen: VecDeque<String>,
    /// Maximum number of IDs retained before the oldest is evicted.
    window: usize,
}

impl DuplicateSuppressor {
    /// Creates a suppressor with the given window size.
    pub fn new(window: usize) -> Self {
        Self {
            seen: VecDeque::with_capacity(window),
            window,
        }
    }

    /// Creates a suppressor with the default window of 256.
    pub fn with_default_window() -> Self {
        Self::new(256)
    }

    /// Returns `true` if `envelope_id` was already seen; otherwise records it.
    pub fn is_duplicate(&mut self, envelope_id: &str) -> bool {
        if self.seen.iter().any(|id| id == envelope_id) {
            return true;
        }
        if self.seen.len() >= self.window {
            self.seen.pop_front();
        }
        self.seen.push_back(envelope_id.to_owned());
        false
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(s: &str) -> AgentId {
        AgentId::from(s.to_owned())
    }

    fn lid(s: &str) -> LeaseId {
        LeaseId::from(s.to_owned())
    }

    fn edge(s: &str) -> UrbanEdgeId {
        UrbanEdgeId::from(s.to_owned())
    }

    fn future_lease() -> Lease {
        Lease {
            lease_id: lid("lease-1"),
            holder: agent("agent-0"),
            resource_id: "edge-0".to_owned(),
            resource_kind: "edge".to_owned(),
            granted_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
        }
    }

    fn expired_lease() -> Lease {
        Lease {
            lease_id: lid("lease-2"),
            holder: agent("agent-0"),
            resource_id: "edge-0".to_owned(),
            resource_kind: "edge".to_owned(),
            granted_at: Utc::now() - chrono::Duration::hours(2),
            expires_at: Utc::now() - chrono::Duration::hours(1),
        }
    }

    fn minimal_plan() -> MissionCommandPlan {
        serde_json::from_str(
            r#"{
                "schema_version": "mission_command_ir.v1",
                "mission_id": "m-0",
                "coordinate_frame": "local_ned",
                "altitude_reference": "relative_home",
                "timeout_policy": {
                    "command_timeout_secs": 5.0,
                    "completion_timeout_secs": 60.0,
                    "on_timeout": "abort"
                },
                "expected_terminal_state": "landed",
                "completion_tolerance": { "position_m": 1.0, "altitude_m": 0.5 },
                "commands": []
            }"#,
        )
        .expect("minimal plan must be valid JSON")
    }

    fn envelope(msg: SwarmMessage) -> SwarmMessageEnvelope {
        SwarmMessageEnvelope {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
            envelope_id: "env-1".to_owned(),
            correlation_id: None,
            from: agent("agent-0"),
            to: agent("gcs"),
            sent_at: Utc::now(),
            ttl_ticks: 10,
            message: msg,
        }
    }

    // ── Constant ──────────────────────────────────────────────────────────

    #[test]
    fn swarm_protocol_schema_version_constant_matches_doc() {
        assert_eq!(SWARM_PROTOCOL_SCHEMA_VERSION, "swarm_protocol.v1");
    }

    // ── Lease validity ────────────────────────────────────────────────────

    #[test]
    fn lease_is_valid_before_expiry() {
        assert!(future_lease().is_valid_at(Utc::now()));
    }

    #[test]
    fn lease_is_invalid_after_expiry() {
        assert!(!expired_lease().is_valid_at(Utc::now()));
    }

    // ── AgentMissionState serde ───────────────────────────────────────────

    #[test]
    fn agent_mission_state_serde_roundtrip_all_variants() {
        let states = vec![
            AgentMissionState::Idle,
            AgentMissionState::WaitingForMission,
            AgentMissionState::ExecutingSegment {
                segment_id: edge("e0"),
                lease_id: lid("l0"),
                started_at_tick: 10,
            },
            AgentMissionState::WaitingForSegment {
                edge_id: edge("e0"),
                blocked_by: agent("agent-1"),
                since_tick: 5,
            },
            AgentMissionState::ContinuingUnderLease {
                lease_id: lid("l1"),
                lease_expires_at: Utc::now(),
            },
            AgentMissionState::Replanning {
                reason: ReplanReason::SegmentBlocked,
            },
            AgentMissionState::GcsLost {
                since_tick: 100,
                policy_engaged: "continue_under_lease".to_owned(),
            },
            AgentMissionState::Aborting {
                reason: "commanded".to_owned(),
            },
            AgentMissionState::Completed {
                resource_id: "task-0".to_owned(),
                finished_at_tick: 200,
            },
            AgentMissionState::Failed {
                reason: "battery".to_owned(),
            },
        ];

        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            assert!(
                json.contains("\"state\":"),
                "missing state discriminant in: {json}"
            );
            let back: AgentMissionState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, back);
        }
    }

    // ── SwarmMessage serde ────────────────────────────────────────────────

    #[test]
    fn swarm_message_serde_roundtrip_all_variants() {
        let idle = AgentMissionState::Idle;
        let plan = minimal_plan();

        let messages: Vec<SwarmMessage> = vec![
            SwarmMessage::Heartbeat {
                tick: 42,
                generation: 1,
                mission_state: idle.clone(),
            },
            SwarmMessage::Presence {
                role: ProtocolRole::Scout,
                capabilities: vec!["urban_patrol".to_owned()],
            },
            SwarmMessage::MissionOffer {
                offer_id: "offer-0".to_owned(),
                plan: plan.clone(),
                lease_ttl_secs: 300,
            },
            SwarmMessage::MissionAccept {
                offer_id: "offer-0".to_owned(),
                lease_id: lid("l0"),
            },
            SwarmMessage::MissionReject {
                offer_id: "offer-0".to_owned(),
                reason: MissionRejectReason::Overloaded,
            },
            SwarmMessage::MissionResult {
                offer_id: "offer-0".to_owned(),
                outcome: MavlinkExecutionOutcome::Completed,
                completed_segments: vec![edge("e0")],
            },
            SwarmMessage::OwnershipClaim {
                resource_id: "edge-0".to_owned(),
                resource_kind: "edge".to_owned(),
                lease_id: lid("l1"),
                expires_at: Utc::now(),
            },
            SwarmMessage::OwnershipRelease {
                resource_id: "edge-0".to_owned(),
                lease_id: lid("l1"),
                reason: ReleaseReason::Completed,
            },
            SwarmMessage::LeaseRenew {
                lease_id: lid("l2"),
                new_expires_at: Utc::now(),
            },
            SwarmMessage::LeaseExpired {
                lease_id: lid("l3"),
                resource_id: "edge-0".to_owned(),
            },
            SwarmMessage::SegmentReserve {
                edge_id: edge("e0"),
                segment_index: 0,
                requester: agent("agent-0"),
                request_tick: 10,
            },
            SwarmMessage::SegmentGrant {
                edge_id: edge("e0"),
                to: agent("agent-0"),
                lease: future_lease(),
            },
            SwarmMessage::SegmentDeny {
                edge_id: edge("e0"),
                to: agent("agent-1"),
                holder: agent("agent-0"),
                reason: SegmentDenyReason::AlreadyHeld,
            },
            SwarmMessage::SegmentRelease {
                edge_id: edge("e0"),
                lease_id: lid("l4"),
            },
            SwarmMessage::ProgressUpdate {
                resource_id: "task-0".to_owned(),
                progress_pct: 50,
                position: Some(CommandPosition {
                    lat_deg: 55.0,
                    lon_deg: 37.0,
                    alt_m: 100.0,
                }),
                tick: 20,
            },
            SwarmMessage::ReplacementOffer {
                for_resource_id: "task-0".to_owned(),
                plan: plan.clone(),
            },
            SwarmMessage::AbortNotice {
                resource_id: "task-0".to_owned(),
                reason: "battery".to_owned(),
                abort_action: AbortAction::ReturnToLaunch,
            },
            SwarmMessage::DegradedNotice {
                reason: DegradedReason::GcsUnavailable,
                affected_resources: vec!["task-0".to_owned()],
            },
            SwarmMessage::TopologyUpdate {
                topology_kind: "star".to_owned(),
                reachable_agents: vec![agent("agent-1")],
            },
            SwarmMessage::StateRequest {
                from: agent("gcs"),
                session_id: "sess-0".to_owned(),
            },
            SwarmMessage::StateResponse {
                mission_state: idle,
                active_leases: vec![future_lease()],
                completed_resources: vec!["edge-0".to_owned()],
                last_tick: 100,
            },
        ];

        for msg in &messages {
            let json = serde_json::to_string(msg).unwrap();
            assert!(
                json.contains("\"kind\":"),
                "missing kind discriminant in: {json}"
            );
            let back: SwarmMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, &back);
        }
    }

    // ── Envelope round-trip ───────────────────────────────────────────────

    #[test]
    fn swarm_message_envelope_into_raw_and_from_raw() {
        let env = envelope(SwarmMessage::Heartbeat {
            tick: 1,
            generation: 0,
            mission_state: AgentMissionState::Idle,
        });
        let raw = env.clone().into_raw_message();
        let restored = SwarmMessageEnvelope::from_raw_message(&raw).unwrap();
        assert_eq!(restored.envelope_id, env.envelope_id);
        assert_eq!(restored.schema_version, SWARM_PROTOCOL_SCHEMA_VERSION);
        assert_eq!(restored.message, env.message);
    }

    #[test]
    fn envelope_with_unknown_schema_version_returns_none() {
        let mut env = envelope(SwarmMessage::DegradedNotice {
            reason: DegradedReason::GcsUnavailable,
            affected_resources: vec![],
        });
        env.schema_version = "swarm_protocol.v999".to_owned();
        let raw = env.into_raw_message();
        assert!(SwarmMessageEnvelope::from_raw_message(&raw).is_none());
    }

    #[test]
    fn from_raw_message_with_garbage_payload_returns_none() {
        let raw = RawMessage {
            from: agent("a"),
            to: agent("b"),
            payload: b"not json at all".to_vec(),
        };
        assert!(SwarmMessageEnvelope::from_raw_message(&raw).is_none());
    }

    // ── Duplicate suppressor ──────────────────────────────────────────────

    #[test]
    fn duplicate_envelope_id_is_dropped_silently() {
        let mut sup = DuplicateSuppressor::new(10);
        assert!(!sup.is_duplicate("env-1"));
        assert!(sup.is_duplicate("env-1"));
        assert!(!sup.is_duplicate("env-2"));
        assert!(sup.is_duplicate("env-2"));
    }

    #[test]
    fn duplicate_suppressor_window_evicts_oldest_id() {
        let mut sup = DuplicateSuppressor::new(3);
        sup.is_duplicate("a");
        sup.is_duplicate("b");
        sup.is_duplicate("c");
        // "a" should be evicted to make room for "d"
        sup.is_duplicate("d");
        // "a" is no longer in the window — not a duplicate
        assert!(!sup.is_duplicate("a"));
    }

    // ── Correlation id ────────────────────────────────────────────────────

    #[test]
    fn mission_offer_accept_correlation_id_matches() {
        let offer = envelope(SwarmMessage::MissionOffer {
            offer_id: "offer-123".to_owned(),
            plan: minimal_plan(),
            lease_ttl_secs: 60,
        });
        let accept = SwarmMessageEnvelope {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
            envelope_id: "accept-456".to_owned(),
            correlation_id: Some(offer.envelope_id.clone()),
            from: agent("agent-0"),
            to: agent("gcs"),
            sent_at: Utc::now(),
            ttl_ticks: 10,
            message: SwarmMessage::MissionAccept {
                offer_id: "offer-123".to_owned(),
                lease_id: lid("lease-7"),
            },
        };
        assert_eq!(
            accept.correlation_id.as_deref(),
            Some(offer.envelope_id.as_str())
        );
    }

    // ── Segment reserve→grant→deny→release via InMemNetwork ───────────────

    #[test]
    fn segment_reserve_grant_deny_release_roundtrip_via_inmem_network() {
        use crate::network::{InMemNetwork, NetworkConfig};
        use crate::Transport;
        use std::collections::HashSet;

        let config = NetworkConfig {
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            latency_per_hop: 0,
            seed: 42,
            partitions: HashSet::new(),
            comms_jitter_ticks: 0,
        };
        let mut net = InMemNetwork::new(config);

        let coord = agent("coordinator");
        let a0 = agent("agent-0");
        let a1 = agent("agent-1");

        // agent-0 reserves segment
        let reserve = SwarmMessageEnvelope {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
            envelope_id: "reserve-1".to_owned(),
            correlation_id: None,
            from: a0.clone(),
            to: coord.clone(),
            sent_at: Utc::now(),
            ttl_ticks: 10,
            message: SwarmMessage::SegmentReserve {
                edge_id: edge("e0"),
                segment_index: 0,
                requester: a0.clone(),
                request_tick: 1,
            },
        };
        let raw_reserve = reserve.into_raw_message();
        net.send(raw_reserve).unwrap();
        net.advance_tick();
        let received = net.drain_ready(&coord);
        assert_eq!(received.len(), 1);
        let decoded = SwarmMessageEnvelope::from_raw_message(&received[0]).unwrap();
        assert!(matches!(
            decoded.message,
            SwarmMessage::SegmentReserve { .. }
        ));

        // coordinator grants to agent-0
        let grant = SwarmMessageEnvelope {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
            envelope_id: "grant-1".to_owned(),
            correlation_id: None,
            from: coord.clone(),
            to: a0.clone(),
            sent_at: Utc::now(),
            ttl_ticks: 10,
            message: SwarmMessage::SegmentGrant {
                edge_id: edge("e0"),
                to: a0.clone(),
                lease: future_lease(),
            },
        };
        net.send(grant.into_raw_message()).unwrap();
        net.advance_tick();
        let recv_a0 = net.drain_ready(&a0);
        assert_eq!(recv_a0.len(), 1);
        assert!(matches!(
            SwarmMessageEnvelope::from_raw_message(&recv_a0[0])
                .unwrap()
                .message,
            SwarmMessage::SegmentGrant { .. }
        ));

        // agent-1 tries to reserve same segment — denied
        let deny = SwarmMessageEnvelope {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
            envelope_id: "deny-1".to_owned(),
            correlation_id: None,
            from: coord.clone(),
            to: a1.clone(),
            sent_at: Utc::now(),
            ttl_ticks: 10,
            message: SwarmMessage::SegmentDeny {
                edge_id: edge("e0"),
                to: a1.clone(),
                holder: a0.clone(),
                reason: SegmentDenyReason::AlreadyHeld,
            },
        };
        net.send(deny.into_raw_message()).unwrap();
        net.advance_tick();
        let recv_a1 = net.drain_ready(&a1);
        assert_eq!(recv_a1.len(), 1);
        assert!(matches!(
            SwarmMessageEnvelope::from_raw_message(&recv_a1[0])
                .unwrap()
                .message,
            SwarmMessage::SegmentDeny { .. }
        ));

        // agent-0 releases segment
        let release = SwarmMessageEnvelope {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
            envelope_id: "release-1".to_owned(),
            correlation_id: None,
            from: a0.clone(),
            to: coord.clone(),
            sent_at: Utc::now(),
            ttl_ticks: 10,
            message: SwarmMessage::SegmentRelease {
                edge_id: edge("e0"),
                lease_id: lid("lease-1"),
            },
        };
        net.send(release.into_raw_message()).unwrap();
        net.advance_tick();
        let recv_coord = net.drain_ready(&coord);
        assert_eq!(recv_coord.len(), 1);
        assert!(matches!(
            SwarmMessageEnvelope::from_raw_message(&recv_coord[0])
                .unwrap()
                .message,
            SwarmMessage::SegmentRelease { .. }
        ));
    }
}
