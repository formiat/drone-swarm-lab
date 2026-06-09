use serde::{Deserialize, Serialize};
use swarm_comms::{ConnectivityLossKind, SupervisorDecision, SupervisorReconcileResult};
use swarm_types::{
    AgentId, Pose, TaskId, UrbanBusId, UrbanEdgeId, UrbanNodeId, UrbanObstacleId,
    UrbanRightOfWayPolicy,
};

/// Schema version for the event log format.
pub const EVENT_LOG_SCHEMA_VERSION: &str = "0.2";

/// A complete event log for a single simulation run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventLog {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    pub run_id: String,
    pub seed: u64,
    pub scenario_name: String,
    pub events: Vec<Event>,
}

fn default_schema_version() -> String {
    EVENT_LOG_SCHEMA_VERSION.to_owned()
}

/// Individual events that can be recorded during a simulation run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Event {
    TickStart {
        tick: u64,
    },
    AgentFailed {
        agent_id: AgentId,
        tick: u64,
    },
    TaskAssigned {
        task_id: TaskId,
        agent_id: AgentId,
        tick: u64,
    },
    TaskStarted {
        task_id: TaskId,
        agent_id: AgentId,
        tick: u64,
    },
    TaskCompleted {
        task_id: TaskId,
        agent_id: AgentId,
        tick: u64,
    },
    TaskExpired {
        task_id: TaskId,
        tick: u64,
    },
    MessageSent {
        from: AgentId,
        to: AgentId,
        tick: u64,
        payload_len: usize,
    },
    MessageDropped {
        from: AgentId,
        to: AgentId,
        tick: u64,
        reason: DropReason,
    },
    PartitionAdded {
        agent_a: AgentId,
        agent_b: AgentId,
        tick: u64,
    },
    PartitionRemoved {
        agent_a: AgentId,
        agent_b: AgentId,
        tick: u64,
    },
    PoseUpdated {
        agent_id: AgentId,
        pose: Pose,
        tick: u64,
    },
    // SAR v2
    SarScan {
        agent_id: AgentId,
        cell: (u32, u32),
        tick: u64,
        detected: bool,
    },
    SarDetection {
        agent_id: AgentId,
        target_pose: Pose,
        tick: u64,
    },
    // Inspection
    EdgeVisited {
        edge_id: String,
        agent_id: AgentId,
        tick: u64,
    },
    // Safety
    SafetyViolation {
        agent_id: AgentId,
        violation_type: ViolationType,
        tick: u64,
    },
    // CBBA
    CbbaConverged {
        tick: u64,
    },
    CbbaBundleUpdated {
        agent_id: AgentId,
        bundle_size: usize,
        #[serde(default)]
        conflict_count: u64,
        tick: u64,
    },
    // M30: Wildfire Mapping
    AgentObservation {
        agent_id: AgentId,
        zone_id: String,
        tick: u64,
    },
    HazardMapUpdated {
        zone_id: String,
        new_threat_level: f64,
        new_priority: u8,
        tick: u64,
    },
    TaskPriorityUpdated {
        task_id: TaskId,
        old_priority: u8,
        new_priority: u8,
        tick: u64,
    },
    WildfirePriorityReallocationRequested {
        task_id: TaskId,
        old_priority: u8,
        new_priority: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        previous_agent_id: Option<AgentId>,
        tick: u64,
    },
    WildfirePriorityTaskReleased {
        task_id: TaskId,
        old_priority: u8,
        new_priority: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        previous_agent_id: Option<AgentId>,
        tick: u64,
    },
    // M65: Urban Patrol v0
    UrbanRoutePlanned {
        agent_id: AgentId,
        tick: u64,
        edge_ids: Vec<UrbanEdgeId>,
        route_length_m: f64,
    },
    UrbanSegmentEntered {
        agent_id: AgentId,
        tick: u64,
        segment_index: usize,
        edge_id: UrbanEdgeId,
        from: UrbanNodeId,
        to: UrbanNodeId,
    },
    UrbanSegmentCompleted {
        agent_id: AgentId,
        tick: u64,
        segment_index: usize,
        edge_id: UrbanEdgeId,
    },
    UrbanViolation {
        agent_id: AgentId,
        tick: u64,
        segment_index: Option<usize>,
        edge_id: Option<UrbanEdgeId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        obstacle_id: Option<UrbanObstacleId>,
        pose: Pose,
        reason: String,
    },
    UrbanPatrolCompleted {
        agent_id: AgentId,
        tick: u64,
        route_length_m: f64,
        distance_travelled_m: f64,
    },
    // M66: Urban Search v1
    BusObserved {
        agent_id: AgentId,
        tick: u64,
        bus_id: UrbanBusId,
        pose: Pose,
        distance_m: f64,
        detector_seed: u64,
    },
    BusDetected {
        agent_id: AgentId,
        tick: u64,
        bus_id: UrbanBusId,
        pose: Pose,
        distance_m: f64,
        detector_seed: u64,
    },
    BusFalsePositive {
        agent_id: AgentId,
        tick: u64,
        pose: Pose,
        detector_seed: u64,
    },
    UrbanSearchCompleted {
        agent_id: AgentId,
        tick: u64,
        detected: bool,
        bus_id: Option<UrbanBusId>,
        reason: String,
        distance_travelled_m: f64,
    },
    // M74: Urban Blocked-Route Decision Logic
    UrbanEdgeBlocked {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    UrbanEdgeUnblocked {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
    },
    UrbanObstacleDetected {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
        lookahead_segments: usize,
    },
    UrbanPolicyDecision {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
        /// "wait" | "replan" | "abort"
        policy: String,
    },
    UrbanRouteReplanned {
        agent_id: AgentId,
        tick: u64,
        edge_ids: Vec<UrbanEdgeId>,
        route_length_m: f64,
    },
    UrbanWaitStarted {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
    },
    UrbanWaitCompleted {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
        waited_ticks: u64,
    },
    UrbanNoRouteAvailable {
        agent_id: AgentId,
        tick: u64,
        from: UrbanNodeId,
        to: UrbanNodeId,
        reason: String,
    },
    // M85: Urban Multi-Agent Deconfliction
    UrbanSegmentLockAcquired {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
        policy: UrbanRightOfWayPolicy,
        reason: String,
    },
    UrbanSegmentLockReleased {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
        held_ticks: u64,
    },
    UrbanSegmentConflict {
        tick: u64,
        edge_id: UrbanEdgeId,
        holder_agent_id: AgentId,
        requester_agent_id: AgentId,
        policy: UrbanRightOfWayPolicy,
        reason: String,
    },
    UrbanDeconflictWait {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
        reason: String,
    },
    UrbanDeconflictReplan {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
        edge_ids: Vec<UrbanEdgeId>,
        route_length_m: f64,
        reason: String,
    },
    UrbanDeconflictAbort {
        agent_id: AgentId,
        tick: u64,
        edge_id: UrbanEdgeId,
        reason: String,
    },
    // M87: Swarm Command Plane
    SwarmCommandPlanDispatched {
        tick: u64,
        plan_id: String,
        agent_count: usize,
    },
    SwarmAgentCommandDispatched {
        tick: u64,
        plan_id: String,
        agent_id: AgentId,
        command_count: usize,
    },
    SwarmOwnershipAcquired {
        tick: u64,
        agent_id: AgentId,
        ownership_kind: String,
        resource_id: String,
        reason: String,
    },
    SwarmOwnershipReleased {
        tick: u64,
        agent_id: AgentId,
        ownership_kind: String,
        resource_id: String,
        reason: String,
    },
    SwarmOwnershipHandoff {
        tick: u64,
        from_agent_id: AgentId,
        to_agent_id: AgentId,
        ownership_kind: String,
        resource_id: String,
        reason: String,
    },
    SwarmSupervisorStateChanged {
        tick: u64,
        from: String,
        to: String,
        reason: String,
    },
    SwarmSyncCommandIssued {
        tick: u64,
        kind: String,
        agent_ids: Vec<AgentId>,
    },
    SwarmSyncCommandResult {
        tick: u64,
        kind: String,
        succeeded_agent_ids: Vec<AgentId>,
        failed_agent_ids: Vec<AgentId>,
        timed_out_agent_ids: Vec<AgentId>,
        partial_success: bool,
    },
    // M88: Logical Swarm Topologies
    SwarmTopologyConfigured {
        tick: u64,
        topology_kind: String,
        node_count: usize,
        link_count: usize,
    },
    SwarmCommandRouteSelected {
        tick: u64,
        route_id: String,
        from_node_id: String,
        to_agent_id: AgentId,
        via_node_ids: Vec<String>,
        degraded: bool,
    },
    SwarmCommandRouteBlocked {
        tick: u64,
        route_id: String,
        from_node_id: String,
        to_agent_id: AgentId,
        reason: String,
    },
    SwarmTopologyDegraded {
        tick: u64,
        topology_kind: String,
        affected_agent_ids: Vec<AgentId>,
        reason: String,
    },
    SwarmMothershipDependencyRecorded {
        tick: u64,
        parent_agent_id: AgentId,
        child_agent_id: AgentId,
        dependency_kind: String,
    },
    // M91: Swarm Communication Protocol
    SwarmProtocolMessage {
        tick: u64,
        from: AgentId,
        to: AgentId,
        /// Unique envelope id of the transported message.
        envelope_id: String,
        /// Value of the SwarmMessage `kind` discriminant (snake_case).
        kind: String,
    },
    LeaseGranted {
        tick: u64,
        lease_id: String,
        holder: AgentId,
        resource_id: String,
        /// Simulation tick at which the lease expires.
        expires_at_tick: u64,
    },
    LeaseExpired {
        tick: u64,
        lease_id: String,
        resource_id: String,
    },
    OwnershipConflict {
        tick: u64,
        resource_id: String,
        claimant_a: AgentId,
        claimant_b: AgentId,
    },
    // M93: Agent Autonomy FSM
    AgentGcsLost {
        agent_id: AgentId,
        tick: u64,
        /// Name of the policy that was engaged (snake_case).
        policy: String,
    },
    AgentGcsReconnected {
        agent_id: AgentId,
        tick: u64,
        gcs_lost_ticks: u64,
    },
    AgentContinuingUnderLease {
        agent_id: AgentId,
        lease_id: String,
        tick: u64,
    },
    AgentLeaseExpiredDuringGcsLoss {
        agent_id: AgentId,
        lease_id: String,
        /// Name of the GCS-lost policy applied upon lease expiry (snake_case).
        policy_applied: String,
        tick: u64,
    },
    AgentNeighborLost {
        agent_id: AgentId,
        lost_neighbor_id: AgentId,
        tick: u64,
    },
    AgentStateReconciled {
        agent_id: AgentId,
        tick: u64,
        gcs_lost_ticks: u64,
        policy_applied: String,
        active_lease_count: u64,
        mission_state_name: String,
    },
    // M94: Degraded / Partition Swarm Supervisor
    PartitionDetected {
        tick: u64,
        group_a: Vec<AgentId>,
        group_b: Vec<AgentId>,
    },
    PartitionHealed {
        tick: u64,
    },
    SupervisorDegradedDecision {
        tick: u64,
        condition: ConnectivityLossKind,
        decision: SupervisorDecision,
        resources: Vec<String>,
    },
    SupervisorReconciled {
        tick: u64,
        result_summary: SupervisorReconcileResult,
    },
    CommandSuppressed {
        tick: u64,
        resource_id: String,
        reason: String,
    },
}

/// Reason why a message was dropped.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DropReason {
    PacketLoss,
    Partition,
    LatencyExceeded,
}

/// Type of safety violation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ViolationType {
    NoFly,
    Geofence,
    Separation,
}

/// Builder for constructing an EventLog incrementally.
#[derive(Debug, Clone)]
pub struct EventLogBuilder {
    run_id: String,
    seed: u64,
    scenario_name: String,
    events: Vec<Event>,
}

impl EventLogBuilder {
    pub fn new(run_id: impl Into<String>, seed: u64, scenario_name: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            seed,
            scenario_name: scenario_name.into(),
            events: Vec::new(),
        }
    }

    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn build(self) -> EventLog {
        EventLog {
            schema_version: EVENT_LOG_SCHEMA_VERSION.to_owned(),
            run_id: self.run_id,
            seed: self.seed,
            scenario_name: self.scenario_name,
            events: self.events,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_log_builder_creates_log() {
        let mut builder = EventLogBuilder::new("test-run", 42, "coverage");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::AgentFailed {
            agent_id: AgentId::from("agent-0".to_owned()),
            tick: 5,
        });

        let log = builder.build();
        assert_eq!(log.run_id, "test-run");
        assert_eq!(log.seed, 42);
        assert_eq!(log.events.len(), 2);
        assert_eq!(log.schema_version, "0.2");
    }

    #[test]
    fn event_log_round_trip_serde() {
        let log = EventLog {
            schema_version: "0.2".to_owned(),
            run_id: "run-1".to_owned(),
            seed: 42,
            scenario_name: "test".to_owned(),
            events: vec![
                Event::TickStart { tick: 0 },
                Event::TaskAssigned {
                    task_id: TaskId::from("task-0".to_owned()),
                    agent_id: AgentId::from("agent-0".to_owned()),
                    tick: 1,
                },
            ],
        };

        let json = serde_json::to_string(&log).unwrap();
        let restored: EventLog = serde_json::from_str(&json).unwrap();
        assert_eq!(log, restored);
    }

    #[test]
    fn urban_events_round_trip_serde() {
        let agent_id = AgentId::from("agent-0".to_owned());
        let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
        let bus_id = UrbanBusId::from("bus-0".to_owned());
        let log = EventLog {
            schema_version: "0.2".to_owned(),
            run_id: "urban-run".to_owned(),
            seed: 7,
            scenario_name: "urban_patrol_small_block".to_owned(),
            events: vec![
                Event::UrbanRoutePlanned {
                    agent_id: agent_id.clone(),
                    tick: 0,
                    edge_ids: vec![edge_id.clone()],
                    route_length_m: 20.0,
                },
                Event::UrbanSegmentEntered {
                    agent_id: agent_id.clone(),
                    tick: 0,
                    segment_index: 0,
                    edge_id: edge_id.clone(),
                    from: UrbanNodeId::from("n0".to_owned()),
                    to: UrbanNodeId::from("n1".to_owned()),
                },
                Event::UrbanSegmentCompleted {
                    agent_id: agent_id.clone(),
                    tick: 10,
                    segment_index: 0,
                    edge_id: edge_id.clone(),
                },
                Event::UrbanViolation {
                    agent_id: agent_id.clone(),
                    tick: 11,
                    segment_index: Some(0),
                    edge_id: Some(edge_id),
                    obstacle_id: None,
                    pose: Pose {
                        x: 1.0,
                        ..Default::default()
                    },
                    reason: "test".to_owned(),
                },
                Event::UrbanPatrolCompleted {
                    agent_id: agent_id.clone(),
                    tick: 12,
                    route_length_m: 20.0,
                    distance_travelled_m: 20.0,
                },
                Event::BusObserved {
                    agent_id: agent_id.clone(),
                    tick: 13,
                    bus_id: bus_id.clone(),
                    pose: Pose {
                        x: 2.0,
                        ..Default::default()
                    },
                    distance_m: 1.0,
                    detector_seed: 9,
                },
                Event::BusDetected {
                    agent_id: agent_id.clone(),
                    tick: 13,
                    bus_id: bus_id.clone(),
                    pose: Pose {
                        x: 2.0,
                        ..Default::default()
                    },
                    distance_m: 1.0,
                    detector_seed: 9,
                },
                Event::BusFalsePositive {
                    agent_id: agent_id.clone(),
                    tick: 14,
                    pose: Pose {
                        x: 3.0,
                        ..Default::default()
                    },
                    detector_seed: 9,
                },
                Event::UrbanSearchCompleted {
                    agent_id,
                    tick: 14,
                    detected: true,
                    bus_id: Some(bus_id),
                    reason: "detected".to_owned(),
                    distance_travelled_m: 10.0,
                },
            ],
        };

        let json = serde_json::to_string(&log).unwrap();
        let restored: EventLog = serde_json::from_str(&json).unwrap();
        assert_eq!(log, restored);
        assert!(json.contains("urban_route_planned"));
        assert!(json.contains("urban_patrol_completed"));
        assert!(json.contains("bus_observed"));
        assert!(json.contains("urban_search_completed"));
    }

    #[test]
    fn legacy_urban_violation_without_obstacle_id_deserializes() {
        let json = r#"{
            "run_id": "legacy-urban",
            "seed": 0,
            "scenario_name": "urban_patrol_small_block",
            "events": [
                {
                    "type": "urban_violation",
                    "agent_id": "agent-0",
                    "tick": 3,
                    "segment_index": 0,
                    "edge_id": "road-n0-n1",
                    "pose": { "x": 1.0, "y": 2.0 },
                    "reason": "ObstacleIntersection"
                }
            ]
        }"#;

        let log: EventLog = serde_json::from_str(json).unwrap();
        assert_eq!(log.schema_version, "0.2");
        match &log.events[0] {
            Event::UrbanViolation { obstacle_id, .. } => {
                assert_eq!(obstacle_id, &None);
            }
            event => panic!("expected UrbanViolation, got {event:?}"),
        }
    }

    #[test]
    fn urban_violation_obstacle_id_roundtrips() {
        let log = EventLog {
            schema_version: "0.2".to_owned(),
            run_id: "urban-run".to_owned(),
            seed: 0,
            scenario_name: "urban_patrol_small_block".to_owned(),
            events: vec![Event::UrbanViolation {
                agent_id: AgentId::from("agent-0".to_owned()),
                tick: 3,
                segment_index: Some(0),
                edge_id: Some(UrbanEdgeId::from("road-n0-n1".to_owned())),
                obstacle_id: Some(UrbanObstacleId::from("building-center".to_owned())),
                pose: Pose {
                    x: 1.0,
                    y: 2.0,
                    ..Default::default()
                },
                reason: "ObstacleIntersection".to_owned(),
            }],
        };

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains("building-center"));
        let restored: EventLog = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, log);
    }

    // ── M91 replay events ─────────────────────────────────────────────────

    #[test]
    fn lease_granted_serde_roundtrip() {
        roundtrip(Event::LeaseGranted {
            tick: 10,
            lease_id: "lease-1".to_owned(),
            holder: agent_id(),
            resource_id: "edge-0".to_owned(),
            expires_at_tick: 110,
        });
    }

    #[test]
    fn lease_expired_serde_roundtrip() {
        roundtrip(Event::LeaseExpired {
            tick: 115,
            lease_id: "lease-1".to_owned(),
            resource_id: "edge-0".to_owned(),
        });
    }

    #[test]
    fn ownership_conflict_serde_roundtrip() {
        roundtrip(Event::OwnershipConflict {
            tick: 20,
            resource_id: "edge-0".to_owned(),
            claimant_a: agent_id(),
            claimant_b: AgentId::from("agent-1".to_owned()),
        });
    }

    #[test]
    fn partition_supervisor_events_serde_roundtrip() {
        roundtrip(Event::PartitionDetected {
            tick: 21,
            group_a: vec![agent_id()],
            group_b: vec![AgentId::from("agent-1".to_owned())],
        });
        roundtrip(Event::PartitionHealed { tick: 22 });
        roundtrip(Event::SupervisorDegradedDecision {
            tick: 23,
            condition: ConnectivityLossKind::SwarmPartitioned {
                group_sizes: vec![1, 1],
            },
            decision: SupervisorDecision::ContinueUnderLease,
            resources: vec!["edge-0".to_owned()],
        });
        roundtrip(Event::CommandSuppressed {
            tick: 24,
            resource_id: "edge-0".to_owned(),
            reason: "ambiguous_authority".to_owned(),
        });
    }

    #[test]
    fn swarm_protocol_message_serde_roundtrip() {
        roundtrip(Event::SwarmProtocolMessage {
            tick: 5,
            from: agent_id(),
            to: AgentId::from("gcs".to_owned()),
            envelope_id: "env-abc".to_owned(),
            kind: "heartbeat".to_owned(),
        });
    }

    #[test]
    fn event_log_schema_version_roundtrip() {
        let log = EventLog {
            schema_version: "0.2".to_owned(),
            run_id: "run-1".to_owned(),
            seed: 0,
            scenario_name: "s".to_owned(),
            events: vec![],
        };
        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains("schema_version"));
        let restored: EventLog = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.schema_version, "0.2");
    }

    #[test]
    fn legacy_event_log_without_schema_version_deserializes() {
        let json = r#"{"run_id":"legacy","seed":0,"scenario_name":"s","events":[]}"#;
        let log: EventLog = serde_json::from_str(json).unwrap();
        assert_eq!(log.schema_version, "0.2");
    }

    #[test]
    fn new_event_types_roundtrip() {
        let events = vec![
            Event::TaskStarted {
                task_id: TaskId::from("t0".to_owned()),
                agent_id: AgentId::from("a0".to_owned()),
                tick: 1,
            },
            Event::TaskCompleted {
                task_id: TaskId::from("t0".to_owned()),
                agent_id: AgentId::from("a0".to_owned()),
                tick: 2,
            },
            Event::TaskExpired {
                task_id: TaskId::from("t1".to_owned()),
                tick: 3,
            },
            Event::SarScan {
                agent_id: AgentId::from("a0".to_owned()),
                cell: (1, 2),
                tick: 4,
                detected: true,
            },
            Event::SarDetection {
                agent_id: AgentId::from("a0".to_owned()),
                target_pose: Pose {
                    x: 1.0,
                    y: 2.0,
                    ..Default::default()
                },
                tick: 5,
            },
            Event::EdgeVisited {
                edge_id: "e1".to_owned(),
                agent_id: AgentId::from("a0".to_owned()),
                tick: 6,
            },
            Event::SafetyViolation {
                agent_id: AgentId::from("a0".to_owned()),
                violation_type: ViolationType::NoFly,
                tick: 7,
            },
            Event::CbbaConverged { tick: 8 },
            Event::CbbaBundleUpdated {
                agent_id: AgentId::from("a0".to_owned()),
                bundle_size: 3,
                conflict_count: 2,
                tick: 9,
            },
            Event::AgentObservation {
                agent_id: AgentId::from("a0".to_owned()),
                zone_id: "zone-a".to_owned(),
                tick: 10,
            },
            Event::HazardMapUpdated {
                zone_id: "zone-a".to_owned(),
                new_threat_level: 0.8,
                new_priority: 6,
                tick: 11,
            },
            Event::TaskPriorityUpdated {
                task_id: TaskId::from("t2".to_owned()),
                old_priority: 3,
                new_priority: 6,
                tick: 12,
            },
            Event::WildfirePriorityReallocationRequested {
                task_id: TaskId::from("t2".to_owned()),
                old_priority: 3,
                new_priority: 8,
                previous_agent_id: Some(AgentId::from("a0".to_owned())),
                tick: 13,
            },
            Event::WildfirePriorityTaskReleased {
                task_id: TaskId::from("t2".to_owned()),
                old_priority: 3,
                new_priority: 8,
                previous_agent_id: Some(AgentId::from("a0".to_owned())),
                tick: 14,
            },
        ];
        let log = EventLog {
            schema_version: "0.2".to_owned(),
            run_id: "rt".to_owned(),
            seed: 0,
            scenario_name: "s".to_owned(),
            events,
        };
        let json = serde_json::to_string(&log).unwrap();
        let restored: EventLog = serde_json::from_str(&json).unwrap();
        assert_eq!(log, restored);
    }

    fn agent_id() -> AgentId {
        AgentId::from("agent-0".to_owned())
    }

    fn edge_id() -> UrbanEdgeId {
        UrbanEdgeId::from("e0".to_owned())
    }

    fn node_id(s: &str) -> UrbanNodeId {
        UrbanNodeId::from(s.to_owned())
    }

    fn roundtrip(event: Event) {
        let json = serde_json::to_string(&event).unwrap();
        let restored: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(event, restored);
    }

    #[test]
    fn urban_edge_blocked_serde_roundtrip() {
        roundtrip(Event::UrbanEdgeBlocked {
            agent_id: agent_id(),
            tick: 5,
            edge_id: edge_id(),
            reason: Some("construction".to_owned()),
        });
    }

    #[test]
    fn urban_edge_unblocked_serde_roundtrip() {
        roundtrip(Event::UrbanEdgeUnblocked {
            agent_id: agent_id(),
            tick: 15,
            edge_id: edge_id(),
        });
    }

    #[test]
    fn urban_obstacle_detected_serde_roundtrip() {
        roundtrip(Event::UrbanObstacleDetected {
            agent_id: agent_id(),
            tick: 6,
            edge_id: edge_id(),
            lookahead_segments: 2,
        });
    }

    #[test]
    fn urban_policy_decision_serde_roundtrip() {
        roundtrip(Event::UrbanPolicyDecision {
            agent_id: agent_id(),
            tick: 6,
            edge_id: edge_id(),
            policy: "wait".to_owned(),
        });
    }

    #[test]
    fn urban_route_replanned_serde_roundtrip() {
        roundtrip(Event::UrbanRouteReplanned {
            agent_id: agent_id(),
            tick: 7,
            edge_ids: vec![edge_id()],
            route_length_m: 20.0,
        });
    }

    #[test]
    fn urban_wait_started_serde_roundtrip() {
        roundtrip(Event::UrbanWaitStarted {
            agent_id: agent_id(),
            tick: 6,
            edge_id: edge_id(),
        });
    }

    #[test]
    fn urban_wait_completed_serde_roundtrip() {
        roundtrip(Event::UrbanWaitCompleted {
            agent_id: agent_id(),
            tick: 16,
            edge_id: edge_id(),
            waited_ticks: 10,
        });
    }

    #[test]
    fn urban_no_route_available_serde_roundtrip() {
        roundtrip(Event::UrbanNoRouteAvailable {
            agent_id: agent_id(),
            tick: 8,
            from: node_id("n0"),
            to: node_id("n2"),
            reason: "all paths blocked".to_owned(),
        });
    }

    #[test]
    fn urban_segment_lock_acquired_serde_roundtrip() {
        roundtrip(Event::UrbanSegmentLockAcquired {
            agent_id: agent_id(),
            tick: 9,
            edge_id: edge_id(),
            policy: UrbanRightOfWayPolicy::Priority,
            reason: "right-of-way winner".to_owned(),
        });
    }

    #[test]
    fn urban_segment_lock_released_serde_roundtrip() {
        roundtrip(Event::UrbanSegmentLockReleased {
            agent_id: agent_id(),
            tick: 19,
            edge_id: edge_id(),
            held_ticks: 10,
        });
    }

    #[test]
    fn urban_segment_conflict_serde_roundtrip() {
        roundtrip(Event::UrbanSegmentConflict {
            tick: 9,
            edge_id: edge_id(),
            holder_agent_id: agent_id(),
            requester_agent_id: AgentId::from("agent-1".to_owned()),
            policy: UrbanRightOfWayPolicy::FirstCome,
            reason: "segment already locked".to_owned(),
        });
    }

    #[test]
    fn urban_deconflict_wait_serde_roundtrip() {
        roundtrip(Event::UrbanDeconflictWait {
            agent_id: agent_id(),
            tick: 10,
            edge_id: edge_id(),
            reason: "segment locked".to_owned(),
        });
    }

    #[test]
    fn urban_deconflict_replan_serde_roundtrip() {
        roundtrip(Event::UrbanDeconflictReplan {
            agent_id: agent_id(),
            tick: 10,
            edge_id: edge_id(),
            edge_ids: vec![UrbanEdgeId::from("e1".to_owned())],
            route_length_m: 12.0,
            reason: "alternate route".to_owned(),
        });
    }

    #[test]
    fn urban_deconflict_abort_serde_roundtrip() {
        roundtrip(Event::UrbanDeconflictAbort {
            agent_id: agent_id(),
            tick: 10,
            edge_id: edge_id(),
            reason: "no alternate route".to_owned(),
        });
    }
}
