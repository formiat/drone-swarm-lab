use rand::SeedableRng;
use swarm_types::{Task, TaskKind, TaskStatus};

use super::super::*;

pub(in crate::runner) fn record_sar_scans<T: Transport>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    grid_state: &mut GridState,
    scenario_seed: u64,
    current_tick: u64,
    dynamic_belief_updates: bool,
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
) {
    for (node, agent_id) in &mut *nodes {
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

    if dynamic_belief_updates {
        rerank_unfinished_sar_tasks_by_entropy(nodes, grid_state);
    }
}

pub(in crate::runner) fn entropy_to_priority(entropy: f64) -> u8 {
    (entropy.clamp(0.0, 1.0) * 10.0).round() as u8
}

pub(in crate::runner) fn rerank_unfinished_sar_tasks_by_entropy<T: Transport>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    grid_state: &GridState,
) {
    for (node, _) in nodes {
        rerank_sar_tasks_by_entropy(node.coordinator.registry.tasks_mut(), grid_state);
    }
}

pub(in crate::runner) fn rerank_sar_tasks_by_entropy<'a>(
    tasks: impl Iterator<Item = &'a mut Task>,
    grid_state: &GridState,
) {
    let Some(belief_map) = grid_state.belief_map.as_ref() else {
        return;
    };

    for task in tasks {
        if !matches!(
            task.kind,
            Some(TaskKind::SarScan | TaskKind::SarConfirmationScan)
        ) {
            continue;
        }
        if matches!(task.status, TaskStatus::Completed | TaskStatus::Failed) {
            continue;
        }
        let Some(cell) = task.grid_cell else {
            continue;
        };
        task.priority = entropy_to_priority(belief_map.entropy(cell));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_runtime::GridState;
    use swarm_types::{SearchGrid, SensorModel, TaskId};

    fn sar_task(id: &str, cell: (u32, u32), priority: u8) -> Task {
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
            grid_cell: Some(cell),
            edge_id: None,
            kind: Some(TaskKind::SarScan),
        }
    }

    #[test]
    fn sar_dynamic_belief_updates_change_task_order() {
        let grid = SearchGrid::new(2, 1, 10.0);
        let sensor = SensorModel::new_v2(0.5, 0.9, 0.1, 0.5, 0.1);
        let mut grid_state = GridState::new(grid, vec![], sensor).with_belief(0.1);
        let belief_map = grid_state.belief_map.as_mut().unwrap();
        belief_map.cells[0][0].posterior = 0.5;
        belief_map.cells[0][1].posterior = 0.01;
        let mut tasks = [
            sar_task("uncertain", (0, 0), 1),
            sar_task("certain", (1, 0), 9),
        ];

        rerank_sar_tasks_by_entropy(tasks.iter_mut(), &grid_state);

        assert!(tasks[0].priority > tasks[1].priority);
    }

    #[test]
    fn sar_static_belief_unchanged_with_flag_false() {
        let grid = SearchGrid::new(1, 1, 10.0);
        let sensor = SensorModel::new_v2(0.5, 0.9, 0.1, 0.5, 0.1);
        let grid_state = GridState::new(grid, vec![], sensor);
        let mut tasks = [sar_task("static", (0, 0), 7)];

        rerank_sar_tasks_by_entropy(tasks.iter_mut(), &grid_state);

        assert_eq!(tasks[0].priority, 7);
    }
}
