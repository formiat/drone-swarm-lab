use swarm_types::{AgentId, Pose, TaskId};

use crate::{AllocationAgent, AllocationTask, Allocator};

/// Centralized oracle planner that knows the full scenario state.
///
/// This is a benchmark-only baseline: it has perfect information about
/// all agent poses, all task poses, and no communication constraints.
/// It solves a bipartite matching problem greedily (Hungarian algorithm
/// can be added later if `petgraph` becomes a dependency).
///
/// No decentralized strategy should beat this in ideal conditions.
pub struct CentralizedPlanner {
    /// Pre-computed optimal assignments from scenario data.
    assignments: Vec<(TaskId, AgentId)>,
}

impl CentralizedPlanner {
    /// Build a planner from the full set of tasks and agents.
    pub fn new(tasks: &[AllocationTask<'_>], agents: &[AllocationAgent]) -> Self {
        let mut assignments = Vec::new();

        // Sort tasks by priority then id for determinism
        let mut ordered: Vec<&AllocationTask<'_>> = tasks.iter().collect();
        ordered.sort_by(|a, b| {
            b.task
                .priority
                .cmp(&a.task.priority)
                .then_with(|| a.task.id.to_string().cmp(&b.task.id.to_string()))
        });

        // Greedy assignment: for each task pick the best agent.
        // Unlike one-to-one matching, agents can receive multiple tasks
        // (same semantics as GreedyAllocator and AuctionAllocator).
        for at in ordered {
            let best = agents
                .iter()
                .filter(|agent| {
                    has_all_capabilities(agent, &at.task.required_capabilities)
                        && has_required_role(agent, &at.task.required_role)
                })
                .map(|agent| (agent, cost(at.task, agent)))
                .filter(|(_, cost)| cost.is_finite())
                .min_by(|(_, ca), (_, cb)| ca.partial_cmp(cb).unwrap());

            if let Some((agent, _)) = best {
                assignments.push((at.task.id.clone(), agent.id.clone()));
            }
        }

        Self { assignments }
    }
}

impl Allocator for CentralizedPlanner {
    fn allocate(
        &self,
        tasks: &[AllocationTask<'_>],
        _agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)> {
        // Only return assignments for tasks that are currently unassigned
        // (avoids duplicate-assignment conflicts when called from every agent)
        let unassigned_ids: std::collections::HashSet<String> =
            tasks.iter().map(|at| at.task.id.to_string()).collect();
        self.assignments
            .iter()
            .filter(|(task_id, _)| unassigned_ids.contains(task_id.as_ref()))
            .cloned()
            .collect()
    }
}

impl super::Strategy for CentralizedPlanner {
    fn name(&self) -> &'static str {
        "centralized"
    }

    fn description(&self) -> &'static str {
        "Oracle centralized planner with full global knowledge (benchmark baseline)"
    }
}

fn cost(task: &swarm_types::Task, agent: &AllocationAgent) -> f64 {
    let task_pose = task.pose.unwrap_or(Pose { x: 0.0, y: 0.0 });
    let dx = agent.pose.x - task_pose.x;
    let dy = agent.pose.y - task_pose.y;
    let distance_cost = (dx * dx + dy * dy).sqrt();

    let battery_cost = 1.0 - agent.battery / 100.0;

    let role_bonus = if task.preferred_role.as_ref() == Some(&agent.role) {
        -0.3
    } else {
        0.0
    };

    distance_cost + battery_cost + role_bonus
}

fn has_all_capabilities(agent: &AllocationAgent, required: &[swarm_types::Capability]) -> bool {
    required.iter().all(|cap| agent.capabilities.contains(cap))
}

fn has_required_role(agent: &AllocationAgent, required: &Option<swarm_types::Role>) -> bool {
    required.as_ref().is_none_or(|r| &agent.role == r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::{Pose, Role, Task, TaskStatus};

    fn task(id: &str) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
            grid_cell: None,
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

    #[test]
    fn centralized_assigns_all_tasks_when_enough_agents() {
        let t1 = task("t1");
        let t2 = task("t2");
        let tasks = vec![AllocationTask { task: &t1 }, AllocationTask { task: &t2 }];
        let agents = vec![agent("a1"), agent("a2")];

        let planner = CentralizedPlanner::new(&tasks, &agents);
        let result = planner.allocate(&tasks, &agents);

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn centralized_returns_empty_when_no_agents() {
        let t1 = task("t1");
        let tasks = vec![AllocationTask { task: &t1 }];
        let agents: Vec<AllocationAgent> = vec![];

        let planner = CentralizedPlanner::new(&tasks, &agents);
        let result = planner.allocate(&tasks, &agents);

        assert!(result.is_empty());
    }
}
