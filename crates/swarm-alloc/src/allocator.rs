use swarm_comms::ConnectivitySnapshot;
use swarm_types::{AgentId, Capability, Pose, Role, Task, TaskId};

/// Enriched task context passed to allocators.
#[derive(Clone)]
pub struct AllocationTask<'a> {
    pub task: &'a Task,
}

/// Enriched agent context passed to allocators.
///
/// Uses owned copies to avoid lifetime conflicts when building from MembershipView.
#[derive(Clone)]
pub struct AllocationAgent {
    pub id: AgentId,
    pub pose: Pose,
    pub battery: f64,
    pub capabilities: Vec<Capability>,
    pub role: Role,
    pub comms_range: f64,
}

/// Connectivity context passed to allocators that need network awareness.
pub struct ConnectivityContext {
    pub snapshot: ConnectivitySnapshot,
    pub base_id: AgentId,
}

/// value: `(task_id, agent_id)` — allocation decisions
pub trait Allocator {
    fn allocate(
        &self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)>;

    /// v0.5 extension for connectivity-aware allocation.
    /// Default implementation delegates to `allocate`, preserving backward compatibility.
    fn allocate_with_connectivity(
        &self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
        _connectivity: &ConnectivityContext,
    ) -> Vec<(TaskId, AgentId)> {
        self.allocate(tasks, agents)
    }
}

pub struct GreedyAllocator;

impl Allocator for GreedyAllocator {
    fn allocate(
        &self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)> {
        if agents.is_empty() {
            return Vec::new();
        }

        let mut ordered: Vec<&AllocationTask<'_>> = tasks.iter().collect();
        ordered.sort_by(|a, b| {
            b.task
                .priority
                .cmp(&a.task.priority)
                .then_with(|| a.task.id.to_string().cmp(&b.task.id.to_string()))
        });

        let mut assignments = Vec::new();
        let mut global_idx: usize = 0;

        for at in ordered {
            let capable: Vec<&AllocationAgent> = agents
                .iter()
                .filter(|agent| {
                    has_all_capabilities(agent, &at.task.required_capabilities)
                        && has_required_role(agent, &at.task.required_role)
                        && agent.battery > 0.0
                })
                .collect();

            if capable.is_empty() {
                continue;
            }

            let agent = capable[global_idx % capable.len()];
            tracing::debug!(
                task_id = %at.task.id,
                agent_id = %agent.id,
                "task allocated"
            );
            assignments.push((at.task.id.clone(), agent.id.clone()));
            global_idx += 1;
        }

        assignments
    }
}

/// Auction-based allocator using a cost function over distance, battery, and role.
pub struct AuctionAllocator {
    /// Weight for Euclidean distance from agent to task pose.
    pub weight_distance: f64,
    /// Weight for battery penalty (lower battery → higher cost).
    pub weight_battery: f64,
    /// Weight for role bonus (matching preferred_role → cost reduction).
    pub weight_role: f64,
}

impl Default for AuctionAllocator {
    fn default() -> Self {
        Self {
            weight_distance: 1.0,
            weight_battery: 0.5,
            weight_role: 0.3,
        }
    }
}

impl Allocator for AuctionAllocator {
    fn allocate(
        &self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)> {
        if agents.is_empty() {
            return Vec::new();
        }

        let mut ordered: Vec<&AllocationTask<'_>> = tasks.iter().collect();
        ordered.sort_by(|a, b| {
            b.task
                .priority
                .cmp(&a.task.priority)
                .then_with(|| a.task.id.to_string().cmp(&b.task.id.to_string()))
        });

        let mut assignments = Vec::new();

        for at in ordered {
            let best = agents
                .iter()
                .map(|agent| (agent, self.cost(at.task, agent)))
                .filter(|(_, cost)| cost.is_finite())
                .min_by(|(_, ca), (_, cb)| ca.partial_cmp(cb).unwrap());

            if let Some((agent, _)) = best {
                tracing::debug!(
                    task_id = %at.task.id,
                    agent_id = %agent.id,
                    "task allocated"
                );
                assignments.push((at.task.id.clone(), agent.id.clone()));
            }
        }

        assignments
    }
}

impl AuctionAllocator {
    fn cost(&self, task: &Task, agent: &AllocationAgent) -> f64 {
        if !has_all_capabilities(agent, &task.required_capabilities)
            || !has_required_role(agent, &task.required_role)
            || agent.battery <= 0.0
        {
            return f64::INFINITY;
        }

        let task_pose = task.pose.unwrap_or(Pose { x: 0.0, y: 0.0 });
        let dx = agent.pose.x - task_pose.x;
        let dy = agent.pose.y - task_pose.y;
        let distance_cost = self.weight_distance * (dx * dx + dy * dy).sqrt();

        let battery_cost = self.weight_battery * (1.0 - agent.battery / 100.0);

        let role_bonus = if task.preferred_role.as_ref() == Some(&agent.role) {
            -self.weight_role
        } else {
            0.0
        };

        distance_cost + battery_cost + role_bonus
    }
}

fn has_all_capabilities(agent: &AllocationAgent, required: &[Capability]) -> bool {
    required.iter().all(|cap| agent.capabilities.contains(cap))
}

fn has_required_role(agent: &AllocationAgent, required: &Option<Role>) -> bool {
    required.as_ref().is_none_or(|r| &agent.role == r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::TaskStatus;

    fn task(id: &str, priority: u8) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
        }
    }

    fn task_with_cap(id: &str, cap: &str) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![Capability::from(cap.to_owned())],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
        }
    }

    fn task_at(id: &str, x: f64, y: f64) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(Pose { x, y }),
        }
    }

    fn agent(id: &str) -> AllocationAgent {
        AllocationAgent {
            id: AgentId::from(id.to_owned()),
            pose: Pose { x: 0.0, y: 0.0 },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
        }
    }

    fn agent_with_cap(id: &str, cap: &str) -> AllocationAgent {
        AllocationAgent {
            id: AgentId::from(id.to_owned()),
            pose: Pose { x: 0.0, y: 0.0 },
            battery: 100.0,
            capabilities: vec![Capability::from(cap.to_owned())],
            role: Role::Scout,
            comms_range: f64::INFINITY,
        }
    }

    fn agent_at(id: &str, x: f64, y: f64) -> AllocationAgent {
        AllocationAgent {
            id: AgentId::from(id.to_owned()),
            pose: Pose { x, y },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
        }
    }

    fn at(t: &Task) -> AllocationTask<'_> {
        AllocationTask { task: t }
    }

    #[test]
    fn greedy_assigns_to_alive_agents() {
        let tasks = [task("t0", 1), task("t1", 1), task("t2", 1)];
        let agents = [agent("a0"), agent("a1"), agent("a2")];
        let result = GreedyAllocator.allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &agents);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn greedy_no_agents_returns_empty() {
        let tasks = [task("t0", 1)];
        let result = GreedyAllocator.allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn greedy_more_tasks_than_agents() {
        let tasks = [
            task("t0", 1),
            task("t1", 1),
            task("t2", 1),
            task("t3", 1),
            task("t4", 1),
        ];
        let agents = [agent("a0"), agent("a1")];
        let result = GreedyAllocator.allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &agents);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn greedy_capability_gate_passes() {
        let t = task_with_cap("t0", "thermal");
        let a = agent_with_cap("a0", "thermal");
        let result = GreedyAllocator.allocate(&[at(&t)], &[a]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn greedy_capability_gate_blocks() {
        let t = task_with_cap("t0", "thermal");
        let a = agent("a0");
        let result = GreedyAllocator.allocate(&[at(&t)], &[a]);
        assert!(result.is_empty());
    }

    #[test]
    fn greedy_with_rich_context_same_behavior() {
        let tasks = [task("t0", 1), task("t1", 1), task("t2", 1)];
        let agents = [agent("a0"), agent("a1")];
        let result = GreedyAllocator.allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &agents);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn auction_selects_closest_agent() {
        let t = task_at("t0", 10.0, 0.0);
        let near = agent_at("near", 9.0, 0.0);
        let far = agent_at("far", 50.0, 0.0);
        let result = AuctionAllocator::default().allocate(&[at(&t)], &[near, far]);
        assert_eq!(result.len(), 1);
        assert_eq!(*result[0].1, "near");
    }

    #[test]
    fn auction_selects_capable_agent() {
        let mut t = task_at("t0", 0.0, 0.0);
        t.required_capabilities = vec![Capability::from("thermal".to_owned())];
        let incapable = agent_at("close", 1.0, 0.0);
        let mut capable = agent_at("far", 100.0, 0.0);
        capable.capabilities = vec![Capability::from("thermal".to_owned())];
        let result = AuctionAllocator::default().allocate(&[at(&t)], &[incapable, capable]);
        assert_eq!(result.len(), 1);
        assert_eq!(*result[0].1, "far");
    }

    #[test]
    fn auction_skips_all_incapable() {
        let mut t = task("t0", 1);
        t.required_capabilities = vec![Capability::from("thermal".to_owned())];
        let a = agent("a0");
        let result = AuctionAllocator::default().allocate(&[at(&t)], &[a]);
        assert!(result.is_empty());
    }

    #[test]
    fn auction_role_bonus_applied() {
        let mut t = task("t0", 1);
        t.preferred_role = Some(Role::Mapper);
        let a_scout = AllocationAgent {
            id: AgentId::from("scout".to_owned()),
            pose: Pose { x: 0.0, y: 0.0 },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
        };
        let a_mapper = AllocationAgent {
            id: AgentId::from("mapper".to_owned()),
            pose: Pose { x: 0.0, y: 0.0 },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Mapper,
            comms_range: f64::INFINITY,
        };
        let result = AuctionAllocator::default().allocate(&[at(&t)], &[a_scout, a_mapper]);
        assert_eq!(result.len(), 1);
        assert_eq!(*result[0].1, "mapper");
    }

    #[test]
    fn auction_low_battery_penalized() {
        let t = task("t0", 1);
        let full = AllocationAgent {
            id: AgentId::from("full".to_owned()),
            pose: Pose { x: 0.0, y: 0.0 },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
        };
        let low = AllocationAgent {
            id: AgentId::from("low".to_owned()),
            pose: Pose { x: 0.0, y: 0.0 },
            battery: 10.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
        };
        let result = AuctionAllocator::default().allocate(&[at(&t)], &[full, low]);
        assert_eq!(result.len(), 1);
        assert_eq!(*result[0].1, "full");
    }

    #[test]
    fn no_duplicate_task_ownership() {
        let tasks: Vec<Task> = (0..5).map(|i| task(&format!("t{i}"), 1)).collect();
        let agents: Vec<AllocationAgent> = (0..3).map(|i| agent(&format!("a{i}"))).collect();
        let result = AuctionAllocator::default()
            .allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &agents);
        let unique_tasks: std::collections::HashSet<_> =
            result.iter().map(|(tid, _)| tid.to_string()).collect();
        assert_eq!(unique_tasks.len(), result.len());
    }

    #[test]
    fn greedy_required_role_blocks_scout() {
        let mut t = task("t0", 1);
        t.required_role = Some(Role::Relay);
        let a_scout = agent("scout");
        let result = GreedyAllocator.allocate(&[at(&t)], &[a_scout]);
        assert!(result.is_empty());
    }

    #[test]
    fn greedy_required_role_allows_relay() {
        let mut t = task("t0", 1);
        t.required_role = Some(Role::Relay);
        let a_relay = AllocationAgent {
            id: AgentId::from("relay".to_owned()),
            pose: Pose { x: 0.0, y: 0.0 },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Relay,
            comms_range: f64::INFINITY,
        };
        let result = GreedyAllocator.allocate(&[at(&t)], &[a_relay]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn auction_required_role_blocks_scout() {
        let mut t = task("t0", 1);
        t.required_role = Some(Role::Relay);
        let a_scout = agent("scout");
        let result = AuctionAllocator::default().allocate(&[at(&t)], &[a_scout]);
        assert!(result.is_empty());
    }

    #[test]
    fn battery_exhausted_agent_excluded_from_allocation() {
        let tasks = [task("t0", 1)];
        let mut a = agent("a0");
        a.battery = 0.0;
        let result = GreedyAllocator.allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &[a]);
        assert!(result.is_empty());
    }

    #[test]
    fn battery_exhausted_agent_excluded_from_auction() {
        let tasks = [task("t0", 1)];
        let mut a = agent("a0");
        a.battery = 0.0;
        let result =
            AuctionAllocator::default().allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &[a]);
        assert!(result.is_empty());
    }
}
