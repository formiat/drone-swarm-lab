use serde::{Deserialize, Serialize};
use swarm_types::{AgentId, Pose, TaskId};

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
        tick: u64,
    },
    // M30: Wildfire / Flood Mapping
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
                target_pose: Pose { x: 1.0, y: 2.0 },
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
}
