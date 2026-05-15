use std::collections::HashMap;

use swarm_types::{Agent, AgentId, Capability, Health, Role};

#[derive(Clone, Debug)]
pub struct AgentEntry {
    pub role: Role,
    pub health: Health,
    pub capabilities: Vec<Capability>,
    pub last_heartbeat_tick: u64,
}

pub struct MembershipView {
    agents: HashMap<AgentId, AgentEntry>,
}

impl MembershipView {
    pub fn new(agents: Vec<Agent>) -> Self {
        let agents = agents
            .into_iter()
            .map(|agent| {
                (
                    agent.id,
                    AgentEntry {
                        role: agent.role,
                        health: agent.health,
                        capabilities: agent.capabilities,
                        last_heartbeat_tick: 0,
                    },
                )
            })
            .collect();
        Self { agents }
    }

    pub fn record_heartbeat(&mut self, agent_id: &AgentId, tick: u64) {
        if let Some(entry) = self.agents.get_mut(agent_id) {
            entry.last_heartbeat_tick = tick;
        }
    }

    pub fn mark_dead(&mut self, agent_id: &AgentId) {
        if let Some(entry) = self.agents.get_mut(agent_id) {
            entry.health = Health::Dead;
        }
    }

    pub fn alive_agents(&self) -> impl Iterator<Item = (&AgentId, &AgentEntry)> {
        self.agents
            .iter()
            .filter(|(_, entry)| entry.health == Health::Alive)
    }

    pub fn get(&self, agent_id: &AgentId) -> Option<&AgentEntry> {
        self.agents.get(agent_id)
    }

    pub fn is_alive(&self, agent_id: &AgentId) -> bool {
        self.get(agent_id)
            .is_some_and(|entry| entry.health == Health::Alive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::Pose;

    fn agent(id: &str) -> Agent {
        Agent {
            id: AgentId::from(id.to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose { x: 0.0, y: 0.0 },
            capabilities: Vec::new(),
            current_task: None,
        }
    }

    #[test]
    fn membership_record_heartbeat() {
        let mut view = MembershipView::new(vec![agent("agent-0")]);
        let id = AgentId::from("agent-0".to_owned());

        view.record_heartbeat(&id, 42);

        assert_eq!(view.get(&id).unwrap().last_heartbeat_tick, 42);
    }

    #[test]
    fn membership_mark_dead() {
        let mut view = MembershipView::new(vec![agent("agent-0")]);
        let id = AgentId::from("agent-0".to_owned());

        view.mark_dead(&id);

        assert!(!view.is_alive(&id));
    }

    #[test]
    fn membership_alive_iter_excludes_dead() {
        let mut view = MembershipView::new(vec![agent("agent-0"), agent("agent-1")]);
        view.mark_dead(&AgentId::from("agent-1".to_owned()));

        let alive: Vec<_> = view.alive_agents().map(|(id, _)| id.to_string()).collect();

        assert_eq!(alive, vec!["agent-0"]);
    }
}
