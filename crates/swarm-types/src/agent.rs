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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
