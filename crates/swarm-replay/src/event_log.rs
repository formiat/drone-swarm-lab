use serde::{Deserialize, Serialize};
use swarm_types::{AgentId, Pose, TaskId};

/// A complete event log for a single simulation run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventLog {
    pub run_id: String,
    pub seed: u64,
    pub scenario_name: String,
    pub events: Vec<Event>,
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
}

/// Reason why a message was dropped.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DropReason {
    PacketLoss,
    Partition,
    LatencyExceeded,
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
    }

    #[test]
    fn event_log_round_trip_serde() {
        let log = EventLog {
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
}
