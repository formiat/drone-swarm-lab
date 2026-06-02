use std::collections::HashSet;

use swarm_alloc::{AllocationAgent, AllocationTask, Allocator, ConnectivityContext};
use swarm_comms::ConnectivitySnapshot;
use swarm_types::{AgentId, Health, Task};

use crate::Coordinator;

use super::runtime::AssignmentChange;

#[derive(Default)]
pub(super) struct AllocationOutcome {
    pub(super) assignments: Vec<AssignmentChange>,
    pub(super) conflicting_assignments: u64,
}

pub(super) fn allocate_unassigned<A: Allocator>(
    coordinator: &mut Coordinator,
    allocator: &mut A,
) -> AllocationOutcome {
    let mut tasks: Vec<Task> = coordinator
        .registry
        .unassigned()
        .into_iter()
        .cloned()
        .collect();
    tasks.sort_by(|left, right| left.id.as_ref().cmp(right.id.as_ref()));
    let allocation_tasks: Vec<AllocationTask<'_>> =
        tasks.iter().map(|task| AllocationTask { task }).collect();

    let mut agents: Vec<AllocationAgent> = coordinator
        .membership
        .alive_agents()
        .map(|(id, entry)| AllocationAgent {
            id: id.clone(),
            pose: entry.pose,
            battery: entry.battery,
            capabilities: entry.capabilities.clone(),
            role: entry.role.clone(),
            comms_range: entry.comms_range,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        })
        .collect();
    agents.sort_by(|left, right| left.id.as_ref().cmp(right.id.as_ref()));

    let agent_entries: Vec<(AgentId, swarm_types::Pose, f64, Health)> = coordinator
        .membership
        .alive_agents()
        .map(|(id, entry)| (id.clone(), entry.pose, entry.comms_range, Health::Alive))
        .collect();
    let base_id = agents
        .first()
        .map(|agent| agent.id.clone())
        .unwrap_or_else(|| AgentId::from("base".to_owned()));
    let base_pose = agents
        .first()
        .map(|agent| agent.pose)
        .unwrap_or(swarm_types::Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        });
    let connectivity = ConnectivityContext {
        snapshot: ConnectivitySnapshot {
            agent_entries,
            ground_nodes: vec![],
            base_id: base_id.to_string(),
            base_pose,
        },
        base_id: base_id.clone(),
    };

    let decisions = allocator.allocate_with_connectivity(&allocation_tasks, &agents, &connectivity);

    let mut seen = HashSet::new();
    let mut outcome = AllocationOutcome::default();
    for (task_id, agent_id) in decisions {
        if !seen.insert(task_id.clone()) {
            outcome.conflicting_assignments += 1;
            continue;
        }
        if coordinator
            .registry
            .assign(&task_id, agent_id.clone())
            .is_err()
        {
            outcome.conflicting_assignments += 1;
        } else {
            outcome
                .assignments
                .push(AssignmentChange { task_id, agent_id });
        }
    }
    outcome
}
