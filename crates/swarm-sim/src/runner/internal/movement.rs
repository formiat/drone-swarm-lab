use super::super::*;

pub(in crate::runner) fn teleport_assigned_tasks_when_movement_disabled<T: Transport>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    enable_movement: bool,
) {
    if enable_movement {
        return;
    }

    for (node, agent_id) in nodes {
        if crashed_agents.contains(agent_id) {
            continue;
        }
        let assigned_tasks: Vec<(AgentId, Option<swarm_types::Pose>)> = node
            .coordinator
            .registry
            .tasks()
            .filter(|task| task.assigned_to.as_ref() == Some(agent_id))
            .map(|task| (agent_id.clone(), task.pose))
            .collect();
        for (_agent_id, pose) in assigned_tasks {
            if let Some(pose) = pose {
                node.coordinator.membership.update_pose(agent_id, pose);
            }
        }
    }
}

pub(in crate::runner) fn apply_environment_effects<T: Transport>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    wind: Option<(f64, f64, f64)>,
    pose_noise_m: f64,
    tick_duration_ms: u64,
    scenario_seed: u64,
    current_tick: u64,
) {
    if wind.is_none() && pose_noise_m <= 0.0 {
        return;
    }

    let dt = tick_duration_ms as f64 / 1000.0;
    let mut rng = rand::rngs::StdRng::seed_from_u64(
        scenario_seed
            .wrapping_add(current_tick)
            .wrapping_add(0xCAFE),
    );
    for (node, agent_id) in nodes {
        if crashed_agents.contains(agent_id) {
            continue;
        }
        node.coordinator
            .membership
            .apply_environment_effects(wind, pose_noise_m, &mut rng, dt);
    }
}
