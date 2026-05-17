use derive_more::{AsRef, Deref, DerefMut, Display, From, Into};
use serde::{Deserialize, Serialize};

use crate::pose::Pose;
use crate::task::TaskId;

/// Unique identifier for a swarm agent.
#[derive(
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct AgentId(String);

/// Operational health state of an agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Alive,
    Degraded,
    Dead,
}

/// Mission role of an agent within the swarm.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Scout,
    Relay,
    Mapper,
    Inspector,
    Carrier,
}

/// A named capability that an agent can provide (e.g. "thermal", "optical").
#[derive(
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct Capability(String);

/// A swarm agent: an autonomous unit with identity, role, state, and capabilities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub role: Role,
    pub health: Health,
    pub pose: Pose,
    pub capabilities: Vec<Capability>,
    pub current_task: Option<TaskId>,
    /// Remaining battery level (0.0..=100.0). Static in Milestone 2; drain modelled in v0.3+.
    pub battery: f64,
    /// Communication range in meters. Default INFINITY means fully connected (backward compat).
    pub comms_range: f64,
    /// Generation (epoch). Incremented on restart. Heartbeats with lower generation are discarded.
    pub generation: u64,
}

/// A passive ground node that participates in the connectivity mesh but does not receive tasks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroundNode {
    pub id: String,
    pub pose: Pose,
    /// Communication range in meters.
    pub comms_range: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: &str) -> Agent {
        Agent {
            id: AgentId::from(id.to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose { x: 0.0, y: 0.0 },
            capabilities: Vec::new(),
            current_task: None,
            battery: 100.0,
            comms_range: f64::INFINITY,
            generation: 1,
        }
    }

    #[test]
    fn agent_id_newtype_roundtrip() {
        let id = AgentId::from("abc".to_owned());
        assert_eq!(*id, "abc");
    }

    #[test]
    fn health_serde_snake_case() {
        let json = serde_json::to_string(&Health::Alive).unwrap();
        assert_eq!(json, r#""alive""#);
    }

    #[test]
    fn role_serde_snake_case() {
        let json = serde_json::to_string(&Role::Scout).unwrap();
        assert_eq!(json, r#""scout""#);
    }

    #[test]
    fn agent_battery_default_100() {
        let a = agent("x");
        assert_eq!(a.battery, 100.0);
    }
}
