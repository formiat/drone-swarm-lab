use swarm_comms::ConnectivitySnapshot;
use swarm_types::{AdapterRegistry, AgentId, Capability, MissionAdapter, Pose, Role, Task, TaskId};

pub use swarm_types::AllocationAgent;

/// Enriched task context passed to allocators.
#[derive(Clone)]
pub struct AllocationTask<'a> {
    pub task: &'a Task,
}

/// Connectivity context passed to allocators that need network awareness.
pub struct ConnectivityContext {
    pub snapshot: ConnectivitySnapshot,
    pub base_id: AgentId,
}

/// value: `(task_id, agent_id)` — allocation decisions
pub trait Allocator {
    fn allocate(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)>;

    /// v0.5 extension for connectivity-aware allocation.
    /// Default implementation delegates to `allocate`, preserving backward compatibility.
    fn allocate_with_connectivity(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
        _connectivity: &ConnectivityContext,
    ) -> Vec<(TaskId, AgentId)> {
        self.allocate(tasks, agents)
    }

    /// v0.10 extension for allocator-specific metrics.
    /// Default returns (0, false, 0). CBBA overrides with real values.
    fn allocation_metrics(&self) -> (u64, bool, u64) {
        (0, false, 0)
    }

    /// Whether this allocator uses distributed message exchange.
    fn is_distributed(&self) -> bool {
        false
    }

    /// v0.27 extension for mission-semantic allocation.
    /// Default implementation delegates to `allocate`, preserving backward compatibility.
    fn allocate_with_adapter(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
        _adapter: &dyn MissionAdapter,
    ) -> Vec<(TaskId, AgentId)> {
        self.allocate(tasks, agents)
    }

    /// v0.33 extension for registry-based mission-semantic allocation.
    /// Default implementation delegates to `allocate`, preserving backward compatibility.
    fn allocate_with_registry(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
        registry: &AdapterRegistry,
    ) -> Vec<(TaskId, AgentId)> {
        use std::collections::HashSet;
        // Collect unique kinds present in the task pool.
        let kinds: HashSet<_> = tasks.iter().filter_map(|t| t.task.kind.as_ref()).collect();
        match kinds.len() {
            // No tasks have a kind → backward-compatible plain allocation.
            0 => self.allocate(tasks, agents),
            // All tasks share the same kind → use that kind's adapter for scoring.
            1 => {
                let kind = kinds.into_iter().next().unwrap();
                self.allocate_with_adapter(tasks, agents, registry.get(kind))
            }
            // Mixed kinds → fall back to plain allocation to avoid cross-kind scoring errors.
            _ => self.allocate(tasks, agents),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GreedyAllocator {
    pub comms_penalty_weight: f64,
}

impl Default for GreedyAllocator {
    fn default() -> Self {
        Self {
            comms_penalty_weight: 0.0,
        }
    }
}

impl Allocator for GreedyAllocator {
    fn allocate(
        &mut self,
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

            let agent = self
                .best_greedy_agent(at.task, &capable, global_idx)
                .unwrap_or(capable[global_idx % capable.len()]);
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

    fn allocate_with_adapter(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
        adapter: &dyn MissionAdapter,
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

            let best = capable
                .iter()
                .max_by(|a, b| {
                    greedy_adapter_score(
                        self.comms_penalty_weight,
                        adapter,
                        at.task,
                        a,
                        global_idx,
                        &capable,
                    )
                    .partial_cmp(&greedy_adapter_score(
                        self.comms_penalty_weight,
                        adapter,
                        at.task,
                        b,
                        global_idx,
                        &capable,
                    ))
                    .unwrap()
                })
                .copied()
                .unwrap_or(capable[global_idx % capable.len()]);

            tracing::debug!(
                task_id = %at.task.id,
                agent_id = %best.id,
                "task allocated (adapter-aware)"
            );
            assignments.push((at.task.id.clone(), best.id.clone()));
            global_idx += 1;
        }

        assignments
    }

    fn allocate_with_connectivity(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
        _connectivity: &ConnectivityContext,
    ) -> Vec<(TaskId, AgentId)> {
        // GreedyAllocator uses its local communication-range scoring when configured.
        self.allocate_with_registry(tasks, agents, &AdapterRegistry::new())
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
    /// Weight for assigning tasks beyond an agent's communication range.
    pub comms_penalty_weight: f64,
}

impl Default for AuctionAllocator {
    fn default() -> Self {
        Self {
            weight_distance: 1.0,
            weight_battery: 0.5,
            weight_role: 0.3,
            comms_penalty_weight: 0.0,
        }
    }
}

impl Allocator for AuctionAllocator {
    fn allocate(
        &mut self,
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

    fn allocate_with_adapter(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
        adapter: &dyn MissionAdapter,
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
                .filter(|agent| {
                    has_all_capabilities(agent, &at.task.required_capabilities)
                        && has_required_role(agent, &at.task.required_role)
                        && agent.battery > 0.0
                })
                .map(|agent| {
                    let base_cost = adapter.route_cost(agent.pose, at.task);
                    let battery_penalty = self.weight_battery * (1.0 - agent.battery / 100.0);
                    let role_bonus = if at.task.preferred_role.as_ref() == Some(&agent.role) {
                        -self.weight_role
                    } else {
                        0.0
                    };
                    let score_bonus = -adapter.score(agent, at.task) * 0.001;
                    let comms_penalty =
                        communication_penalty(self.comms_penalty_weight, at.task, agent);
                    (
                        agent,
                        base_cost + battery_penalty + role_bonus + score_bonus + comms_penalty,
                    )
                })
                .filter(|(_, cost)| cost.is_finite())
                .min_by(|(_, ca), (_, cb)| ca.partial_cmp(cb).unwrap());

            if let Some((agent, _)) = best {
                tracing::debug!(
                    task_id = %at.task.id,
                    agent_id = %agent.id,
                    "task allocated (adapter-aware)"
                );
                assignments.push((at.task.id.clone(), agent.id.clone()));
            }
        }

        assignments
    }

    fn allocate_with_connectivity(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
        _connectivity: &ConnectivityContext,
    ) -> Vec<(TaskId, AgentId)> {
        // AuctionAllocator uses its local communication-range scoring when configured.
        self.allocate_with_registry(tasks, agents, &AdapterRegistry::new())
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

        let task_pose = task.pose.unwrap_or(Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        });
        let dx = agent.pose.x - task_pose.x;
        let dy = agent.pose.y - task_pose.y;
        let distance_cost = self.weight_distance * (dx * dx + dy * dy).sqrt();

        let battery_cost = self.weight_battery * (1.0 - agent.battery / 100.0);

        let role_bonus = if task.preferred_role.as_ref() == Some(&agent.role) {
            -self.weight_role
        } else {
            0.0
        };

        distance_cost
            + battery_cost
            + role_bonus
            + communication_penalty(self.comms_penalty_weight, task, agent)
    }
}

impl GreedyAllocator {
    fn best_greedy_agent<'a>(
        &self,
        task: &Task,
        capable: &[&'a AllocationAgent],
        global_idx: usize,
    ) -> Option<&'a AllocationAgent> {
        if self.comms_penalty_weight <= 0.0 {
            return None;
        }
        capable
            .iter()
            .enumerate()
            .min_by(|(idx_a, a), (idx_b, b)| {
                let score_a =
                    greedy_comms_score(self.comms_penalty_weight, task, a, global_idx, *idx_a);
                let score_b =
                    greedy_comms_score(self.comms_penalty_weight, task, b, global_idx, *idx_b);
                score_a.partial_cmp(&score_b).unwrap()
            })
            .map(|(_, agent)| *agent)
    }
}

fn communication_penalty(weight: f64, task: &Task, agent: &AllocationAgent) -> f64 {
    if weight <= 0.0 {
        return 0.0;
    }
    let Some(task_pose) = task.pose else {
        return 0.0;
    };
    if !agent.comms_range.is_finite() || agent.comms_range <= 0.0 {
        return 0.0;
    }
    let over_range = agent.pose.distance_to(&task_pose) - agent.comms_range;
    weight * over_range.max(0.0)
}

fn greedy_comms_score(
    weight: f64,
    task: &Task,
    agent: &AllocationAgent,
    global_idx: usize,
    capable_idx: usize,
) -> f64 {
    let rotation_penalty = if capable_idx >= global_idx {
        capable_idx - global_idx
    } else {
        capable_idx + global_idx
    } as f64
        * 1e-9;
    communication_penalty(weight, task, agent) + rotation_penalty
}

fn greedy_adapter_score(
    comms_penalty_weight: f64,
    adapter: &dyn MissionAdapter,
    task: &Task,
    agent: &AllocationAgent,
    global_idx: usize,
    capable: &[&AllocationAgent],
) -> f64 {
    let capable_idx = capable
        .iter()
        .position(|candidate| candidate.id == agent.id)
        .unwrap_or(0);
    adapter.score(agent, task)
        - greedy_comms_score(
            comms_penalty_weight,
            task,
            agent,
            global_idx % capable.len(),
            capable_idx,
        )
}

pub(crate) fn has_all_capabilities(agent: &AllocationAgent, required: &[Capability]) -> bool {
    required.iter().all(|cap| agent.capabilities.contains(cap))
}

pub(crate) fn has_required_role(agent: &AllocationAgent, required: &Option<Role>) -> bool {
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
            grid_cell: None,
            edge_id: None,
            kind: None,
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
            grid_cell: None,
            edge_id: None,
            kind: None,
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
            pose: Some(Pose {
                x,
                y,
                ..Default::default()
            }),
            grid_cell: None,
            edge_id: None,
            kind: None,
        }
    }

    fn agent(id: &str) -> AllocationAgent {
        AllocationAgent {
            id: AgentId::from(id.to_owned()),
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        }
    }

    fn agent_with_cap(id: &str, cap: &str) -> AllocationAgent {
        AllocationAgent {
            id: AgentId::from(id.to_owned()),
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            battery: 100.0,
            capabilities: vec![Capability::from(cap.to_owned())],
            role: Role::Scout,
            comms_range: f64::INFINITY,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        }
    }

    fn agent_at(id: &str, x: f64, y: f64) -> AllocationAgent {
        AllocationAgent {
            id: AgentId::from(id.to_owned()),
            pose: Pose {
                x,
                y,
                ..Default::default()
            },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        }
    }

    fn at(t: &Task) -> AllocationTask<'_> {
        AllocationTask { task: t }
    }

    #[test]
    fn greedy_assigns_to_alive_agents() {
        let tasks = [task("t0", 1), task("t1", 1), task("t2", 1)];
        let agents = [agent("a0"), agent("a1"), agent("a2")];
        let result =
            GreedyAllocator::default().allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &agents);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn greedy_no_agents_returns_empty() {
        let tasks = [task("t0", 1)];
        let result =
            GreedyAllocator::default().allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &[]);
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
        let result =
            GreedyAllocator::default().allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &agents);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn greedy_capability_gate_passes() {
        let t = task_with_cap("t0", "thermal");
        let a = agent_with_cap("a0", "thermal");
        let result = GreedyAllocator::default().allocate(&[at(&t)], &[a]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn greedy_capability_gate_blocks() {
        let t = task_with_cap("t0", "thermal");
        let a = agent("a0");
        let result = GreedyAllocator::default().allocate(&[at(&t)], &[a]);
        assert!(result.is_empty());
    }

    #[test]
    fn greedy_with_rich_context_same_behavior() {
        let tasks = [task("t0", 1), task("t1", 1), task("t2", 1)];
        let agents = [agent("a0"), agent("a1")];
        let result =
            GreedyAllocator::default().allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &agents);
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
    fn comms_penalty_zero_no_effect() {
        let t = task_at("t0", 10.0, 0.0);
        let mut near = agent_at("near", 9.0, 0.0);
        near.comms_range = 1.0;
        let mut far = agent_at("far", 30.0, 0.0);
        far.comms_range = 100.0;

        let baseline =
            AuctionAllocator::default().allocate(&[at(&t)], &[near.clone(), far.clone()]);
        let configured = AuctionAllocator {
            comms_penalty_weight: 0.0,
            ..AuctionAllocator::default()
        }
        .allocate(&[at(&t)], &[near, far]);

        assert_eq!(baseline, configured);
    }

    #[test]
    fn comms_penalty_reduces_score_beyond_range() {
        let t = task_at("t0", 10.0, 0.0);
        let mut close_out_of_range = agent_at("close", 9.0, 0.0);
        close_out_of_range.comms_range = 0.1;
        let mut farther_in_range = agent_at("in-range", 20.0, 0.0);
        farther_in_range.comms_range = 100.0;

        let result = AuctionAllocator {
            comms_penalty_weight: 100.0,
            ..AuctionAllocator::default()
        }
        .allocate(&[at(&t)], &[close_out_of_range, farther_in_range]);

        assert_eq!(*result[0].1, "in-range");
    }

    #[test]
    fn comms_penalty_infinite_range_no_effect() {
        let t = task_at("t0", 10.0, 0.0);
        let near = agent_at("near", 9.0, 0.0);
        let far = agent_at("far", 50.0, 0.0);

        let result = AuctionAllocator {
            comms_penalty_weight: 100.0,
            ..AuctionAllocator::default()
        }
        .allocate(&[at(&t)], &[near, far]);

        assert_eq!(*result[0].1, "near");
    }

    #[test]
    fn greedy_comms_penalty_prefers_in_range_agent() {
        let t = task_at("t0", 10.0, 0.0);
        let mut first_out_of_range = agent_at("first", 0.0, 0.0);
        first_out_of_range.comms_range = 1.0;
        let mut second_in_range = agent_at("second", 30.0, 0.0);
        second_in_range.comms_range = 100.0;

        let result = GreedyAllocator {
            comms_penalty_weight: 100.0,
        }
        .allocate(&[at(&t)], &[first_out_of_range, second_in_range]);

        assert_eq!(*result[0].1, "second");
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
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        };
        let a_mapper = AllocationAgent {
            id: AgentId::from("mapper".to_owned()),
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Mapper,
            comms_range: f64::INFINITY,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
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
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        };
        let low = AllocationAgent {
            id: AgentId::from("low".to_owned()),
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            battery: 10.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
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
        let result = GreedyAllocator::default().allocate(&[at(&t)], &[a_scout]);
        assert!(result.is_empty());
    }

    #[test]
    fn greedy_required_role_allows_relay() {
        let mut t = task("t0", 1);
        t.required_role = Some(Role::Relay);
        let a_relay = AllocationAgent {
            id: AgentId::from("relay".to_owned()),
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Relay,
            comms_range: f64::INFINITY,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        };
        let result = GreedyAllocator::default().allocate(&[at(&t)], &[a_relay]);
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
        let result =
            GreedyAllocator::default().allocate(&tasks.iter().map(at).collect::<Vec<_>>(), &[a]);
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

    #[test]
    fn greedy_is_not_distributed() {
        assert!(!GreedyAllocator::default().is_distributed());
    }
}
