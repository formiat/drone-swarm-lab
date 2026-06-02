use super::super::*;

pub(in crate::runner) fn record_inspection_edge_visits<T: Transport>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    inspection_state: &mut InspectionState,
    current_tick: u64,
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
) -> u64 {
    let mut revisit_count = 0;

    for (node, agent_id) in nodes {
        if crashed_agents.contains(agent_id) {
            continue;
        }
        let assigned_tasks: Vec<_> = node
            .coordinator
            .registry
            .tasks()
            .filter(|task| task.assigned_to.as_ref() == Some(agent_id))
            .filter(|task| task.edge_id.is_some())
            .cloned()
            .collect();
        for task in assigned_tasks {
            let Some(ref edge_id) = task.edge_id else {
                continue;
            };
            let Some(entry) = node.coordinator.membership.get(agent_id) else {
                continue;
            };
            let task_pose = task.pose.unwrap_or(entry.pose);
            let edge = inspection_state
                .graph
                .edges
                .iter()
                .find(|edge| &edge.id == edge_id);
            let Some(edge) = edge else {
                continue;
            };
            let threshold = (edge.length_m * 0.1).max(1.0);
            let distance = entry.pose.distance_to(&task_pose);
            if distance < threshold {
                let count = inspection_state
                    .visit_counts
                    .entry(edge_id.clone())
                    .or_insert(0);
                *count += 1;
                if !inspection_state.covered.insert(edge_id.clone()) {
                    revisit_count += 1;
                }
                if let Some(builder) = log_builder {
                    builder.push(swarm_replay::Event::EdgeVisited {
                        edge_id: edge_id.to_string(),
                        agent_id: agent_id.clone(),
                        tick: current_tick,
                    });
                    builder.push(swarm_replay::Event::TaskCompleted {
                        task_id: task.id.clone(),
                        agent_id: agent_id.clone(),
                        tick: current_tick,
                    });
                }
                node.coordinator.registry.complete_assigned_task(&task.id);
            }
        }
    }

    revisit_count
}
