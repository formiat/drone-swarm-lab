use std::collections::HashMap;

use swarm_types::{Agent, AgentId, Capability, Health, Pose, Role};

use crate::task_registry::TaskRegistry;

#[derive(Clone, Debug)]
pub struct AgentEntry {
    pub role: Role,
    pub health: Health,
    pub capabilities: Vec<Capability>,
    pub last_heartbeat_tick: u64,
    pub battery: f64,
    pub pose: Pose,
    pub comms_range: f64,
    pub generation: u64,
    pub speed: f64,
    pub max_range: f64,
    pub battery_drain_rate: f64,
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
                        comms_range: agent.comms_range,
                        generation: agent.generation,
                        speed: agent.speed,
                        max_range: agent.max_range,
                        battery_drain_rate: if agent.battery_drain_rate > 0.0 {
                            agent.battery_drain_rate
                        } else if agent.max_range > 0.0 {
                            100.0 / agent.max_range
                        } else {
                            0.0
                        },
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

    /// Update the pose of an alive agent (used for pose-update simulation in ScenarioRunner).
    pub fn update_pose(&mut self, agent_id: &AgentId, new_pose: Pose) {
        if let Some(entry) = self.agents.get_mut(agent_id) {
            if entry.health == Health::Alive {
                entry.pose = new_pose;
            }
        }
    }

    pub fn alive_agents(&self) -> impl Iterator<Item = (&AgentId, &AgentEntry)> {
        self.agents
            .iter()
            .filter(|(_, entry)| entry.health == Health::Alive)
    }

    pub fn all_agents(&self) -> impl Iterator<Item = (&AgentId, &AgentEntry)> {
        self.agents.iter()
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

    /// Move agents toward their assigned tasks. Updates pose and drains battery.
    /// Returns (exhausted agents, (agent_id, distance_moved) for all moving agents).
    pub fn apply_movement(
        &mut self,
        registry: &TaskRegistry,
        tick_duration_ms: u64,
    ) -> (Vec<AgentId>, Vec<(AgentId, f64)>) {
        let dt = tick_duration_ms as f64 / 1000.0;
        let mut exhausted = Vec::new();
        let mut distances = Vec::new();

        for (agent_id, entry) in self.agents.iter_mut() {
            if entry.health != Health::Alive || entry.speed <= 0.0 {
                continue;
            }

            let task_id = match registry
                .tasks()
                .find(|t| t.assigned_to.as_ref() == Some(agent_id))
            {
                Some(t) => t.id.clone(),
                None => continue,
            };

            let task = match registry.tasks().find(|t| t.id == task_id) {
                Some(t) => t,
                None => continue,
            };

            let target_pose = match task.pose {
                Some(p) => p,
                None => continue,
            };

            let dx = target_pose.x - entry.pose.x;
            let dy = target_pose.y - entry.pose.y;
            let distance_to_target = (dx * dx + dy * dy).sqrt();

            if distance_to_target < 1e-9 {
                continue;
            }

            let max_step = entry.speed * dt;
            let distance_moved = distance_to_target.min(max_step);

            let ratio = distance_moved / distance_to_target;
            entry.pose.x += dx * ratio;
            entry.pose.y += dy * ratio;

            let drain = distance_moved * entry.battery_drain_rate;
            entry.battery = (entry.battery - drain).max(0.0);

            if entry.battery <= 0.0 {
                exhausted.push(agent_id.clone());
            }
            distances.push((agent_id.clone(), distance_moved));
        }

        (exhausted, distances)
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
            comms_range: f64::INFINITY,
            generation: 1,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
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

    #[test]
    fn movement_speed_zero_no_movement() {
        use swarm_types::TaskStatus;

        let task = swarm_types::Task {
            id: swarm_types::TaskId::from("t0".to_owned()),
            status: TaskStatus::Assigned,
            assigned_to: Some(AgentId::from("agent-0".to_owned())),
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            grid_cell: None,
            edge_id: None,
            pose: Some(Pose { x: 10.0, y: 0.0 }),
        };
        let registry = TaskRegistry::new(vec![task]);
        let mut view = MembershipView::new(vec![agent("agent-0")]);
        let (exhausted, distances) = view.apply_movement(&registry, 1000);
        assert!(exhausted.is_empty());
        assert!(distances.is_empty());
    }

    #[test]
    fn movement_toward_target_updates_pose() {
        use swarm_types::{TaskId, TaskStatus};

        let task = swarm_types::Task {
            id: TaskId::from("t0".to_owned()),
            status: TaskStatus::Assigned,
            assigned_to: Some(AgentId::from("agent-0".to_owned())),
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            grid_cell: None,
            edge_id: None,
            pose: Some(Pose { x: 10.0, y: 0.0 }),
        };
        let registry = TaskRegistry::new(vec![task]);

        let mut a = agent("agent-0");
        a.speed = 5.0;
        a.max_range = 500.0;
        a.battery_drain_rate = 0.2;
        let mut view = MembershipView::new(vec![a]);

        let (exhausted, distances) = view.apply_movement(&registry, 1000);
        assert!(exhausted.is_empty());
        assert_eq!(distances.len(), 1);
        assert!((distances[0].1 - 5.0).abs() < 0.01); // speed*dt = 5*1 = 5m

        let entry = view.get(&AgentId::from("agent-0".to_owned())).unwrap();
        assert!(entry.pose.x > 0.0);
        assert!((entry.battery - 99.0).abs() < 0.1); // 5m * 0.2 = 1% drain
    }

    #[test]
    fn movement_reaches_target_snaps_pose() {
        use swarm_types::{TaskId, TaskStatus};

        let task = swarm_types::Task {
            id: TaskId::from("t0".to_owned()),
            status: TaskStatus::Assigned,
            assigned_to: Some(AgentId::from("agent-0".to_owned())),
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            grid_cell: None,
            edge_id: None,
            pose: Some(Pose { x: 3.0, y: 0.0 }),
        };
        let registry = TaskRegistry::new(vec![task]);

        let mut a = agent("agent-0");
        a.speed = 10.0;
        let mut view = MembershipView::new(vec![a]);

        view.apply_movement(&registry, 1000);
        let entry = view.get(&AgentId::from("agent-0".to_owned())).unwrap();
        assert!((entry.pose.x - 3.0).abs() < 0.01);
        assert!((entry.pose.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn movement_drains_battery() {
        use swarm_types::{TaskId, TaskStatus};

        let task = swarm_types::Task {
            id: TaskId::from("t0".to_owned()),
            status: TaskStatus::Assigned,
            assigned_to: Some(AgentId::from("agent-0".to_owned())),
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            grid_cell: None,
            edge_id: None,
            pose: Some(Pose { x: 100.0, y: 0.0 }),
        };
        let registry = TaskRegistry::new(vec![task]);

        let mut a = agent("agent-0");
        a.speed = 10.0;
        a.max_range = 50.0; // 100/50 = 2% per meter
        a.battery_drain_rate = 2.0;
        let mut view = MembershipView::new(vec![a]);

        view.apply_movement(&registry, 1000);
        let entry = view.get(&AgentId::from("agent-0".to_owned())).unwrap();
        assert!((entry.battery - 80.0).abs() < 0.1); // 10m * 2% = 20% drain
    }

    #[test]
    fn movement_exhausts_battery() {
        use swarm_types::{TaskId, TaskStatus};

        let task = swarm_types::Task {
            id: TaskId::from("t0".to_owned()),
            status: TaskStatus::Assigned,
            assigned_to: Some(AgentId::from("agent-0".to_owned())),
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            grid_cell: None,
            edge_id: None,
            pose: Some(Pose { x: 100.0, y: 0.0 }),
        };
        let registry = TaskRegistry::new(vec![task]);

        let mut a = agent("agent-0");
        a.speed = 10.0;
        a.battery = 5.0;
        a.battery_drain_rate = 2.0;
        let mut view = MembershipView::new(vec![a]);

        let (exhausted, _) = view.apply_movement(&registry, 1000);
        assert_eq!(exhausted, vec![AgentId::from("agent-0".to_owned())]);
    }

    #[test]
    fn movement_no_target_no_movement() {
        let mut a = agent("agent-0");
        a.speed = 5.0;
        let view_before = Pose { x: 0.0, y: 0.0 };
        let mut view = MembershipView::new(vec![a]);
        let (exhausted, distances) = view.apply_movement(&TaskRegistry::new(vec![]), 1000);
        assert!(exhausted.is_empty());
        assert!(distances.is_empty());
        let entry = view.get(&AgentId::from("agent-0".to_owned())).unwrap();
        assert!((entry.pose.x - view_before.x).abs() < 0.01);
    }
}
