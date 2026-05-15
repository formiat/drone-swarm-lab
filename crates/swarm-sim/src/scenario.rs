use serde::{Deserialize, Serialize};
use swarm_types::{Agent, Task};

/// A self-contained simulation scenario with initial fleet and task state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub seed: u64,
    pub agents: Vec<Agent>,
    pub tasks: Vec<Task>,
}

impl Scenario {
    /// Create an empty scenario with no agents and no tasks.
    pub fn empty(name: impl Into<String>, seed: u64) -> Self {
        Self {
            name: name.into(),
            seed,
            agents: Vec::new(),
            tasks: Vec::new(),
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
