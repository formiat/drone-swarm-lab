use super::super::*;

pub(in crate::runner) struct ConnectivityMetricsTick {
    pub availability: f64,
    pub disconnected_agents: u64,
    pub average_hop_count: Option<f64>,
}

pub(in crate::runner) fn update_connectivity_snapshot<T: Transport>(
    nodes: &[(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    bus: &Rc<RefCell<InMemNetwork>>,
    scenario: &Scenario,
    base_id: &AgentId,
    base_pose: swarm_types::Pose,
) {
    let first_alive = nodes.iter().find(|(_, id)| !crashed_agents.contains(id));
    if let Some((node, _)) = first_alive {
        let snapshot =
            connectivity_snapshot_from_node(node, crashed_agents, scenario, base_id, base_pose);
        bus.borrow_mut().set_connectivity_snapshot(snapshot);
    }
}

pub(in crate::runner) fn connectivity_metrics_tick<T: Transport>(
    nodes: &[(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    scenario: &Scenario,
    base_id: &AgentId,
    base_pose: swarm_types::Pose,
) -> Option<ConnectivityMetricsTick> {
    let first_alive = nodes.iter().find(|(_, id)| !crashed_agents.contains(id))?;
    let (node, _) = first_alive;
    let snapshot =
        connectivity_snapshot_from_node(node, crashed_agents, scenario, base_id, base_pose);
    let reachability = ConnectivityModel::reachability_from_base(&snapshot);
    let alive_agent_ids: Vec<AgentId> = node
        .coordinator
        .membership
        .alive_agents()
        .map(|(id, _)| id.clone())
        .collect();
    let availability = ConnectivityModel::availability_fraction(&reachability, &alive_agent_ids);

    let disconnected_agents = alive_agent_ids.len()
        - alive_agent_ids
            .iter()
            .filter(|id| reachability.contains_key(id.as_ref()))
            .count();
    let hop_sum: usize = alive_agent_ids
        .iter()
        .filter_map(|id| reachability.get(id.as_ref()))
        .sum();
    let reachable_count = alive_agent_ids
        .iter()
        .filter(|id| reachability.contains_key(id.as_ref()))
        .count();
    let average_hop_count =
        (reachable_count > 0).then_some(hop_sum as f64 / reachable_count as f64);

    Some(ConnectivityMetricsTick {
        availability,
        disconnected_agents: disconnected_agents as u64,
        average_hop_count,
    })
}

fn connectivity_snapshot_from_node<T: Transport>(
    node: &AgentNode<T>,
    crashed_agents: &HashSet<AgentId>,
    scenario: &Scenario,
    base_id: &AgentId,
    base_pose: swarm_types::Pose,
) -> ConnectivitySnapshot {
    let agent_entries: Vec<(AgentId, swarm_types::Pose, f64, Health)> = node
        .coordinator
        .membership
        .all_agents()
        .filter(|(id, _)| !crashed_agents.contains(id))
        .map(|(id, entry)| (id.clone(), entry.pose, entry.comms_range, Health::Alive))
        .collect();
    ConnectivitySnapshot {
        agent_entries,
        ground_nodes: scenario
            .ground_nodes
            .iter()
            .map(|ground_node| {
                (
                    ground_node.id.clone(),
                    ground_node.pose,
                    ground_node.comms_range,
                )
            })
            .collect(),
        base_id: base_id.to_string(),
        base_pose,
    }
}
