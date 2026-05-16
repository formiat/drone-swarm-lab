use std::collections::HashMap;

use swarm_types::{Agent, AgentId, Capability, Health, Pose, Role};

#[derive(Clone, Debug)]
pub struct AgentEntry {
    pub role: Role,
    pub health: Health,
    pub capabilities: Vec<Capability>,
    pub last_heartbeat_tick: u64,
    pub battery: f64,
    pub pose: Pose,
    pub generation: u64,
}

pub struct MembershipView {
    /// key: `agent_id`
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
                        battery: agent.battery,
                        pose: agent.pose,
                        generation: agent.generation,
                    },
                )
            })
            .collect();
        Self { agents }
    }

    pub fn record_heartbeat(&mut self, agent_id: &AgentId, sender_tick: u64, generation: u64) {
        let Some(entry) = self.agents.get_mut(agent_id) else {
            return;
        };

        if generation < entry.generation {
            tracing::debug!(
                agent_id = %agent_id,
                generation,
                local_gen = entry.generation,
                "stale heartbeat ignored (old generation)"
            );
            return;
        }

        if generation > entry.generation {
            entry.generation = generation;
            entry.last_heartbeat_tick = sender_tick;
            if entry.health != Health::Alive {
                entry.health = Health::Alive;
            }
            tracing::debug!(agent_id = %agent_id, generation, "heartbeat recorded (new generation)");
            return;
        }

        if sender_tick > entry.last_heartbeat_tick {
            entry.last_heartbeat_tick = sender_tick;
            if entry.health != Health::Alive {
                entry.health = Health::Alive;
            }
            tracing::debug!(agent_id = %agent_id, sender_tick, "heartbeat recorded");
        } else {
            tracing::debug!(
                agent_id = %agent_id,
                sender_tick,
                local_tick = entry.last_heartbeat_tick,
                "stale heartbeat ignored (old tick)"
            );
        }
    }

    pub fn mark_dead(&mut self, agent_id: &AgentId) {
        if let Some(entry) = self.agents.get_mut(agent_id) {
            entry.health = Health::Dead;
            tracing::warn!(agent_id = %agent_id, "agent marked dead");
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

    pub fn generation_of(&self, agent_id: &AgentId) -> u64 {
        self.get(agent_id).map(|e| e.generation).unwrap_or(0)
    }

    pub fn all_generations(&self) -> impl Iterator<Item = (&AgentId, u64)> {
        self.agents.iter().map(|(id, e)| (id, e.generation))
    }
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
            generation: 1,
        }
    }

    #[test]
    fn membership_record_heartbeat() {
        let mut view = MembershipView::new(vec![agent("agent-0")]);
        let id = AgentId::from("agent-0".to_owned());

        view.record_heartbeat(&id, 42, 1);

        assert_eq!(view.get(&id).unwrap().last_heartbeat_tick, 42);
    }

    #[test]
    fn stale_heartbeat_with_lower_generation_is_ignored() {
        let mut view = MembershipView::new(vec![agent("agent-0")]);
        let id = AgentId::from("agent-0".to_owned());
        view.record_heartbeat(&id, 42, 2);

        view.record_heartbeat(&id, 99, 1);

        assert_eq!(view.get(&id).unwrap().last_heartbeat_tick, 42);
    }

    #[test]
    fn stale_heartbeat_with_old_tick_ignored() {
        let mut view = MembershipView::new(vec![agent("agent-0")]);
        let id = AgentId::from("agent-0".to_owned());
        view.record_heartbeat(&id, 10, 1);

        view.record_heartbeat(&id, 5, 1);

        assert_eq!(view.get(&id).unwrap().last_heartbeat_tick, 10);
    }

    #[test]
    fn fresh_heartbeat_with_higher_generation_updates() {
        let mut view = MembershipView::new(vec![agent("agent-0")]);
        let id = AgentId::from("agent-0".to_owned());
        view.record_heartbeat(&id, 5, 1);

        view.record_heartbeat(&id, 20, 2);

        let e = view.get(&id).unwrap();
        assert_eq!(e.last_heartbeat_tick, 20);
        assert_eq!(e.generation, 2);
    }

    #[test]
    fn heartbeat_idempotent_same_tick_same_gen() {
        let mut view = MembershipView::new(vec![agent("agent-0")]);
        let id = AgentId::from("agent-0".to_owned());
        view.record_heartbeat(&id, 7, 1);
        view.record_heartbeat(&id, 7, 1);

        assert_eq!(view.get(&id).unwrap().last_heartbeat_tick, 7);
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

    #[test]
    fn membership_entry_has_battery_and_pose() {
        let mut a = agent("a0");
        a.battery = 50.0;
        a.pose = Pose { x: 1.0, y: 2.0 };
        let view = MembershipView::new(vec![a]);
        let id = AgentId::from("a0".to_owned());
        let entry = view.get(&id).unwrap();
        assert_eq!(entry.battery, 50.0);
        assert_eq!(entry.pose.x, 1.0);
        assert_eq!(entry.pose.y, 2.0);
    }
}
