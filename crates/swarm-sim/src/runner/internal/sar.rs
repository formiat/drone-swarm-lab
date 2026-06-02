use rand::SeedableRng;

use super::super::*;

pub(in crate::runner) fn record_sar_scans<T: Transport>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    grid_state: &mut GridState,
    scenario_seed: u64,
    current_tick: u64,
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
) {
    for (node, agent_id) in nodes {
        if crashed_agents.contains(agent_id) {
            continue;
        }
        let assigned_tasks: Vec<_> = node
            .coordinator
            .registry
            .tasks()
            .filter(|task| task.assigned_to.as_ref() == Some(agent_id))
            .map(|task| (task.id.clone(), task.grid_cell))
            .collect();
        let mut scanned_tasks = Vec::new();
        for (task_id, grid_cell) in assigned_tasks {
            let Some((cell_x, cell_y)) = grid_cell else {
                continue;
            };
            let Some(entry) = node.coordinator.membership.get(agent_id) else {
                continue;
            };
            let cell_pose = grid_state.grid.cell_center(cell_x, cell_y);
            let distance = entry.pose.distance_to(&cell_pose);
            let threshold = grid_state.grid.cell_size * 0.1;
            if distance < threshold {
                let mut rng =
                    rand::rngs::StdRng::seed_from_u64(scenario_seed.wrapping_add(current_tick));
                let detected = grid_state.scan_cell(
                    agent_id.clone(),
                    cell_x,
                    cell_y,
                    &entry.role,
                    current_tick,
                    entry.pose.z,
                    &mut rng,
                );
                if let Some(builder) = log_builder {
                    builder.push(swarm_replay::Event::SarScan {
                        agent_id: agent_id.clone(),
                        cell: (cell_x, cell_y),
                        tick: current_tick,
                        detected,
                    });
                    if detected {
                        builder.push(swarm_replay::Event::SarDetection {
                            agent_id: agent_id.clone(),
                            target_pose: cell_pose,
                            tick: current_tick,
                        });
                    }
                }
                scanned_tasks.push((task_id, agent_id.clone()));
            }
        }
        for (task_id, scanned_by) in scanned_tasks {
            if let Some(builder) = log_builder {
                builder.push(swarm_replay::Event::TaskCompleted {
                    task_id: task_id.clone(),
                    agent_id: scanned_by,
                    tick: current_tick,
                });
            }
            node.coordinator.registry.complete_assigned_task(&task_id);
        }
    }
}
