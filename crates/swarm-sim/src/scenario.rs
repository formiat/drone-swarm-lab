use serde::{Deserialize, Serialize};
use swarm_types::{Agent, GroundNode, Pose, Task};

/// WGS84 origin used to convert local simulation coordinates into global missions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeoOrigin {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub alt_m: f64,
}

/// A self-contained simulation scenario with initial fleet and task state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub seed: u64,
    pub agents: Vec<Agent>,
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub ground_nodes: Vec<GroundNode>,
    #[serde(default)]
    pub base_station: Option<Pose>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo_origin: Option<GeoOrigin>,
}

impl Scenario {
    /// Create an empty scenario with no agents and no tasks.
    pub fn empty(name: impl Into<String>, seed: u64) -> Self {
        Self {
            name: name.into(),
            seed,
            agents: Vec::new(),
            tasks: Vec::new(),
            ground_nodes: Vec::new(),
            base_station: None,
            geo_origin: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_empty_has_no_agents() {
        let s = Scenario::empty("test", 0);
        assert!(s.agents.is_empty());
    }

    #[test]
    fn scenario_empty_has_no_tasks() {
        let s = Scenario::empty("test", 0);
        assert!(s.tasks.is_empty());
    }
}
