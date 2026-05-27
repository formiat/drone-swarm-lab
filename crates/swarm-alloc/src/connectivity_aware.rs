use swarm_comms::{ConnectivityModel, ConnectivitySnapshot};
use swarm_types::{AgentId, Health, Role, TaskId};

use crate::{AllocationAgent, AllocationTask, Allocator, AuctionAllocator, ConnectivityContext};

/// Connectivity-aware allocator that optimizes relay placement for mesh reachability.
pub struct ConnectivityAwareAllocator {
    pub base_allocator: AuctionAllocator,
}

impl Allocator for ConnectivityAwareAllocator {
    fn allocate(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)> {
        self.base_allocator.allocate(tasks, agents)
    }

    fn allocate_with_connectivity(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
        connectivity: &ConnectivityContext,
    ) -> Vec<(TaskId, AgentId)> {
        let mut assignments = Vec::new();

        // Separate relay tasks from scout tasks
        let (relay_tasks, scout_tasks): (Vec<_>, Vec<_>) = tasks
            .iter()
            .partition(|at| at.task.required_role == Some(Role::Relay));

        // Allocate scout tasks using base allocator
        let scout_refs: Vec<AllocationTask<'_>> = scout_tasks.into_iter().cloned().collect();
        let scout_assignments = self.base_allocator.allocate(&scout_refs, agents);
        assignments.extend(scout_assignments);

        // Allocate relay tasks with connectivity optimization
        for relay_task in &relay_tasks {
            let capable_relay_agents: Vec<&AllocationAgent> =
                agents.iter().filter(|a| a.role == Role::Relay).collect();

            if capable_relay_agents.is_empty() {
                continue;
            }

            let best_agent = if let Some(task_pose) = relay_task.task.pose {
                // Simulate moving each candidate to the task pose and compute reachability
                let mut best_score = -1.0f64;
                let mut best_agent_id = None;

                for candidate in &capable_relay_agents {
                    let score = simulate_reachability_with_agent_at_pose(
                        connectivity,
                        agents,
                        &candidate.id,
                        task_pose,
                    );
                    if score > best_score {
                        best_score = score;
                        best_agent_id = Some(candidate.id.clone());
                    }
                }
                best_agent_id
            } else {
                // No pose specified: fall back to base allocator for this task
                let single_task = vec![(*relay_task).clone()];
                let result = self.base_allocator.allocate(&single_task, agents);
                result.into_iter().next().map(|(_, agent_id)| agent_id)
            };

            if let Some(agent_id) = best_agent {
                assignments.push((relay_task.task.id.clone(), agent_id));
            }
        }

        assignments
    }
}

/// Simulate what the network availability would be if `agent_id` were at `new_pose`.
fn simulate_reachability_with_agent_at_pose(
    connectivity: &ConnectivityContext,
    agents: &[AllocationAgent],
    agent_id: &AgentId,
    new_pose: swarm_types::Pose,
) -> f64 {
    let mut agent_entries = Vec::new();

    for agent in agents {
        let pose = if &agent.id == agent_id {
            new_pose
        } else {
            agent.pose
        };
        agent_entries.push((agent.id.clone(), pose, agent.comms_range, Health::Alive));
    }

    let snapshot = ConnectivitySnapshot {
        agent_entries,
        ground_nodes: connectivity.snapshot.ground_nodes.clone(),
        base_id: connectivity.base_id.to_string(),
        base_pose: connectivity.snapshot.base_pose,
    };

    let reachability = ConnectivityModel::reachability_from_base(&snapshot);
    let agent_ids: Vec<AgentId> = agents.iter().map(|a| a.id.clone()).collect();
    ConnectivityModel::availability_fraction(&reachability, &agent_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::{Pose, Task, TaskStatus};

    fn make_context(base_pose: Pose) -> ConnectivityContext {
        ConnectivityContext {
            snapshot: ConnectivitySnapshot {
                agent_entries: vec![],
                ground_nodes: vec![],
                base_id: "base".to_owned(),
                base_pose,
            },
            base_id: AgentId::from("base".to_owned()),
        }
    }

    fn relay_agent(id: &str, x: f64, y: f64) -> AllocationAgent {
        AllocationAgent {
            id: AgentId::from(id.to_owned()),
            pose: Pose {
                x,
                y,
                ..Default::default()
            },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Relay,
            comms_range: 10.0,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        }
    }

    fn scout_agent(id: &str, x: f64, y: f64) -> AllocationAgent {
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
            comms_range: 10.0,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        }
    }

    fn relay_task(id: &str, x: f64, y: f64) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: Some(Role::Relay),
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

    #[test]
    fn connectivity_aware_prefers_relay_for_relay_task() {
        let mut allocator = ConnectivityAwareAllocator {
            base_allocator: AuctionAllocator::default(),
        };

        let relay = relay_agent("relay", 0.0, 0.0);
        let scout = scout_agent("scout", 0.0, 0.0);
        let task = relay_task("relay-task", 5.0, 0.0);

        let tasks = vec![AllocationTask { task: &task }];
        let agents = vec![relay.clone(), scout];
        let ctx = make_context(Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        });

        let result = allocator.allocate_with_connectivity(&tasks, &agents, &ctx);
        assert_eq!(result.len(), 1);
        assert_eq!(*result[0].1, "relay");
    }

    #[test]
    fn connectivity_aware_scout_task_ignores_role() {
        let mut allocator = ConnectivityAwareAllocator {
            base_allocator: AuctionAllocator::default(),
        };

        let relay = relay_agent("relay", 0.0, 0.0);
        let scout = scout_agent("scout", 1.0, 0.0);
        let mut task = relay_task("scout-task", 5.0, 0.0);
        task.required_role = Some(Role::Scout);

        let tasks = vec![AllocationTask { task: &task }];
        let agents = vec![relay, scout.clone()];
        let ctx = make_context(Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        });

        let result = allocator.allocate_with_connectivity(&tasks, &agents, &ctx);
        assert_eq!(result.len(), 1);
        assert_eq!(*result[0].1, "scout");
    }
}
