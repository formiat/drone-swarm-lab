use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use swarm_types::{AgentId, TaskId};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuntimeMessage {
    #[serde(rename = "hb")]
    Heartbeat { sender_tick: u64, generation: u64 },
    #[serde(rename = "gossip")]
    Gossip {
        assignments: HashMap<TaskId, AgentId>,
        generations: HashMap<AgentId, u64>,
    },
}

impl RuntimeMessage {
    pub fn from_payload(payload: &[u8]) -> Option<Self> {
        serde_json::from_slice(payload).ok()
    }

    pub fn to_payload(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn heartbeat(sender_tick: u64, generation: u64) -> Vec<u8> {
        Self::Heartbeat {
            sender_tick,
            generation,
        }
        .to_payload()
    }

    pub fn gossip(
        assignments: HashMap<TaskId, AgentId>,
        generations: HashMap<AgentId, u64>,
    ) -> Vec<u8> {
        Self::Gossip {
            assignments,
            generations,
        }
        .to_payload()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_message_hb_serde_roundtrip() {
        let payload = RuntimeMessage::heartbeat(42, 1);
        let msg = RuntimeMessage::from_payload(&payload).unwrap();
        match msg {
            RuntimeMessage::Heartbeat {
                sender_tick,
                generation,
            } => {
                assert_eq!(sender_tick, 42);
                assert_eq!(generation, 1);
            }
            _ => panic!("expected heartbeat"),
        }
    }

    #[test]
    fn runtime_message_gossip_serde_roundtrip() {
        let assignments = HashMap::from([(
            TaskId::from("t0".to_owned()),
            AgentId::from("a0".to_owned()),
        )]);
        let generations = HashMap::from([(AgentId::from("a0".to_owned()), 2)]);
        let payload = RuntimeMessage::gossip(assignments.clone(), generations.clone());
        let msg = RuntimeMessage::from_payload(&payload).unwrap();
        match msg {
            RuntimeMessage::Gossip {
                assignments: a,
                generations: g,
            } => {
                assert_eq!(a, assignments);
                assert_eq!(g, generations);
            }
            _ => panic!("expected gossip"),
        }
    }

    #[test]
    fn unknown_payload_returns_none_not_panics() {
        let result = RuntimeMessage::from_payload(b"not json");
        assert!(result.is_none());
    }
}
