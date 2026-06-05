use std::collections::{HashMap, HashSet, VecDeque};

use swarm_types::AgentId;

use crate::types::{
    SwarmAgentCommandPlan, SwarmCommandRoute, SwarmMothershipDependency, SwarmTopologyConfig,
    SwarmTopologyKind, SwarmTopologyLink, SwarmTopologyNode, SwarmTopologyNodeKind,
    SwarmTransportAssumptions, SwarmTransportDeliveryModel,
};

pub const DEFAULT_GCS_NODE_ID: &str = "gcs";

impl SwarmTransportAssumptions {
    pub fn in_memory_logical() -> Self {
        Self {
            delivery_model: SwarmTransportDeliveryModel::InMemory,
            max_delay_ms: Some(0),
            drop_rate: Some(0.0),
            hardware_boundary:
                "logical command-plane routing only; no RF mesh or hardware transport guarantee"
                    .to_owned(),
        }
    }
}

impl SwarmTopologyConfig {
    pub fn centralized_gcs_for_agents(agents: &[SwarmAgentCommandPlan]) -> Self {
        let mut nodes = vec![SwarmTopologyNode {
            node_id: DEFAULT_GCS_NODE_ID.to_owned(),
            agent_id: None,
            kind: SwarmTopologyNodeKind::Gcs,
            available: true,
        }];
        nodes.extend(agents.iter().map(|agent| SwarmTopologyNode {
            node_id: agent_node_id(&agent.agent_id),
            agent_id: Some(agent.agent_id.clone()),
            kind: SwarmTopologyNodeKind::Agent,
            available: true,
        }));

        let mut links = Vec::new();
        for agent in agents {
            let node_id = agent_node_id(&agent.agent_id);
            links.push(SwarmTopologyLink {
                from_node_id: DEFAULT_GCS_NODE_ID.to_owned(),
                to_node_id: node_id.clone(),
                available: true,
                delay_ms: Some(0),
                drop_rate: Some(0.0),
                reason: Some("centralized_gcs_command_path".to_owned()),
            });
            links.push(SwarmTopologyLink {
                from_node_id: node_id,
                to_node_id: DEFAULT_GCS_NODE_ID.to_owned(),
                available: true,
                delay_ms: Some(0),
                drop_rate: Some(0.0),
                reason: Some("centralized_gcs_ack_path".to_owned()),
            });
        }

        Self {
            kind: SwarmTopologyKind::CentralizedGcs,
            gcs_node_id: DEFAULT_GCS_NODE_ID.to_owned(),
            nodes,
            links,
            transport: SwarmTransportAssumptions::in_memory_logical(),
            mothership_dependencies: Vec::new(),
        }
    }
}

pub fn agent_node_id(agent_id: &AgentId) -> String {
    format!("agent:{agent_id}")
}

pub fn route_command_plan(
    topology: &SwarmTopologyConfig,
    agents: &[SwarmAgentCommandPlan],
) -> Vec<SwarmCommandRoute> {
    agents
        .iter()
        .map(|agent| route_between(topology, &topology.gcs_node_id, &agent.agent_id))
        .collect()
}

pub fn route_between(
    topology: &SwarmTopologyConfig,
    from_node_id: &str,
    to_agent_id: &AgentId,
) -> SwarmCommandRoute {
    let route_id = format!("route:{from_node_id}:{to_agent_id}");
    let Some(target_node_id) = agent_node_id_in_topology(topology, to_agent_id) else {
        return SwarmCommandRoute {
            route_id,
            from_node_id: from_node_id.to_owned(),
            to_agent_id: to_agent_id.clone(),
            via_node_ids: Vec::new(),
            allowed: false,
            degraded: true,
            reason: "missing_agent_topology_node".to_owned(),
        };
    };

    let Some(path) = bfs_path(topology, from_node_id, &target_node_id) else {
        return SwarmCommandRoute {
            route_id,
            from_node_id: from_node_id.to_owned(),
            to_agent_id: to_agent_id.clone(),
            via_node_ids: Vec::new(),
            allowed: false,
            degraded: true,
            reason: blocked_reason(topology),
        };
    };

    let degraded = topology
        .links
        .iter()
        .any(|link| !link.available || link.drop_rate.unwrap_or(0.0) > 0.0);
    SwarmCommandRoute {
        route_id,
        from_node_id: from_node_id.to_owned(),
        to_agent_id: to_agent_id.clone(),
        via_node_ids: path,
        allowed: true,
        degraded,
        reason: route_reason(topology),
    }
}

pub fn has_mothership_cycle(dependencies: &[SwarmMothershipDependency]) -> Option<AgentId> {
    let mut graph: HashMap<AgentId, Vec<AgentId>> = HashMap::new();
    for dependency in dependencies {
        graph
            .entry(dependency.parent_agent_id.clone())
            .or_default()
            .push(dependency.child_agent_id.clone());
    }

    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    let mut starts: Vec<_> = graph.keys().cloned().collect();
    starts.sort_by_key(ToString::to_string);
    starts
        .into_iter()
        .find(|start| dfs_cycle(start, &graph, &mut visiting, &mut visited))
}

fn dfs_cycle(
    agent_id: &AgentId,
    graph: &HashMap<AgentId, Vec<AgentId>>,
    visiting: &mut HashSet<AgentId>,
    visited: &mut HashSet<AgentId>,
) -> bool {
    if visited.contains(agent_id) {
        return false;
    }
    if !visiting.insert(agent_id.clone()) {
        return true;
    }
    if let Some(children) = graph.get(agent_id) {
        let mut children = children.clone();
        children.sort_by_key(ToString::to_string);
        for child in children {
            if dfs_cycle(&child, graph, visiting, visited) {
                return true;
            }
        }
    }
    visiting.remove(agent_id);
    visited.insert(agent_id.clone());
    false
}

fn agent_node_id_in_topology(topology: &SwarmTopologyConfig, agent_id: &AgentId) -> Option<String> {
    topology
        .nodes
        .iter()
        .find(|node| node.agent_id.as_ref() == Some(agent_id))
        .map(|node| node.node_id.clone())
}

fn bfs_path(
    topology: &SwarmTopologyConfig,
    from_node_id: &str,
    to_node_id: &str,
) -> Option<Vec<String>> {
    let available_nodes: HashSet<&str> = topology
        .nodes
        .iter()
        .filter(|node| node.available)
        .map(|node| node.node_id.as_str())
        .collect();
    if !available_nodes.contains(from_node_id) || !available_nodes.contains(to_node_id) {
        return None;
    }

    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for link in topology.links.iter().filter(|link| link.available) {
        if available_nodes.contains(link.from_node_id.as_str())
            && available_nodes.contains(link.to_node_id.as_str())
        {
            adjacency
                .entry(link.from_node_id.as_str())
                .or_default()
                .push(link.to_node_id.as_str());
        }
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort();
        neighbors.dedup();
    }

    let mut queue = VecDeque::new();
    let mut previous: HashMap<&str, &str> = HashMap::new();
    let mut seen = HashSet::new();
    queue.push_back(from_node_id);
    seen.insert(from_node_id);

    while let Some(node_id) = queue.pop_front() {
        if node_id == to_node_id {
            return Some(reconstruct_path(from_node_id, to_node_id, &previous));
        }
        for neighbor in adjacency.get(node_id).into_iter().flatten() {
            if seen.insert(*neighbor) {
                previous.insert(*neighbor, node_id);
                queue.push_back(*neighbor);
            }
        }
    }

    None
}

fn reconstruct_path<'a>(
    from_node_id: &'a str,
    to_node_id: &'a str,
    previous: &HashMap<&'a str, &'a str>,
) -> Vec<String> {
    let mut path = vec![to_node_id];
    let mut cursor = to_node_id;
    while cursor != from_node_id {
        let Some(parent) = previous.get(cursor) else {
            break;
        };
        cursor = parent;
        path.push(cursor);
    }
    path.reverse();
    path.into_iter().map(ToOwned::to_owned).collect()
}

fn route_reason(topology: &SwarmTopologyConfig) -> String {
    match topology.kind {
        SwarmTopologyKind::CentralizedGcs => "centralized_gcs_route".to_owned(),
        SwarmTopologyKind::P2pLogical => "p2p_logical_route".to_owned(),
        SwarmTopologyKind::Mothership => "mothership_dependency_route".to_owned(),
        SwarmTopologyKind::Relay => "relay_route".to_owned(),
        SwarmTopologyKind::Mesh => "mesh_route".to_owned(),
    }
}

fn blocked_reason(topology: &SwarmTopologyConfig) -> String {
    match topology.kind {
        SwarmTopologyKind::CentralizedGcs => "centralized_gcs_route_unavailable".to_owned(),
        SwarmTopologyKind::P2pLogical => "p2p_logical_link_missing".to_owned(),
        SwarmTopologyKind::Mothership => "mothership_dependency_route_unavailable".to_owned(),
        SwarmTopologyKind::Relay => "relay_unavailable_or_partitioned".to_owned(),
        SwarmTopologyKind::Mesh => "mesh_partition_or_blocked_link".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use swarm_comms::{compile_mavlink_common_plan, MavlinkCommonPlanOptions};
    use swarm_mission_ir::{
        AltitudeReference, CommandId, CompletionTolerance, CoordinateFrame, MissionCommand,
        MissionCommandEntry, MissionCommandPlan, MissionId, TerminalState, TimeoutAction,
        TimeoutPolicy,
    };

    use super::*;
    use crate::types::{SwarmAbortPolicy, SwarmCommandRole};

    fn agent(id: &str) -> SwarmAgentCommandPlan {
        let command_plan = command_plan(id);
        let mavlink_plan =
            compile_mavlink_common_plan(&command_plan, &MavlinkCommonPlanOptions::default())
                .unwrap();
        SwarmAgentCommandPlan {
            agent_id: AgentId::from(id.to_owned()),
            role: SwarmCommandRole::Scout,
            command_plan,
            expected_acks: mavlink_plan.expected_acks.clone(),
            telemetry_milestones: mavlink_plan.telemetry_milestones.clone(),
            mavlink_plan,
            abort_policy: SwarmAbortPolicy::AbortMission,
            ownership_refs: Vec::new(),
        }
    }

    fn command_plan(agent_id: &str) -> MissionCommandPlan {
        MissionCommandPlan {
            schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
            mission_id: MissionId::from(format!("{agent_id}-mission")),
            coordinate_frame: CoordinateFrame::LocalNed,
            altitude_reference: AltitudeReference::RelativeHome,
            timeout_policy: TimeoutPolicy {
                command_timeout_secs: 5.0,
                completion_timeout_secs: 30.0,
                on_timeout: TimeoutAction::Abort,
            },
            expected_terminal_state: TerminalState::Landed,
            completion_tolerance: CompletionTolerance {
                position_m: 1.0,
                altitude_m: 0.5,
            },
            commands: vec![MissionCommandEntry {
                command_id: CommandId::from(format!("{agent_id}-arm")),
                command: MissionCommand::Arm,
                source_task_id: None,
                source_route_id: None,
                source_agent_id: Some(agent_id.to_owned()),
            }],
        }
    }

    #[test]
    fn centralized_topology_routes_all_commands_through_gcs() {
        let agents = vec![agent("agent-0"), agent("agent-1")];
        let topology = SwarmTopologyConfig::centralized_gcs_for_agents(&agents);

        let routes = route_command_plan(&topology, &agents);

        assert_eq!(routes.len(), 2);
        assert!(routes.iter().all(|route| route.allowed));
        assert_eq!(routes[0].via_node_ids[0], DEFAULT_GCS_NODE_ID);
    }

    #[test]
    fn mesh_topology_routes_over_logical_links_deterministically() {
        let agents = [agent("agent-0")];
        let target = agent_node_id(&agents[0].agent_id);
        let topology = SwarmTopologyConfig {
            kind: SwarmTopologyKind::Mesh,
            gcs_node_id: DEFAULT_GCS_NODE_ID.to_owned(),
            nodes: vec![
                SwarmTopologyNode {
                    node_id: DEFAULT_GCS_NODE_ID.to_owned(),
                    agent_id: None,
                    kind: SwarmTopologyNodeKind::Gcs,
                    available: true,
                },
                SwarmTopologyNode {
                    node_id: "relay:1".to_owned(),
                    agent_id: None,
                    kind: SwarmTopologyNodeKind::Relay,
                    available: true,
                },
                SwarmTopologyNode {
                    node_id: target.clone(),
                    agent_id: Some(agents[0].agent_id.clone()),
                    kind: SwarmTopologyNodeKind::Agent,
                    available: true,
                },
            ],
            links: vec![
                SwarmTopologyLink {
                    from_node_id: DEFAULT_GCS_NODE_ID.to_owned(),
                    to_node_id: "relay:1".to_owned(),
                    available: true,
                    delay_ms: None,
                    drop_rate: None,
                    reason: None,
                },
                SwarmTopologyLink {
                    from_node_id: "relay:1".to_owned(),
                    to_node_id: target,
                    available: true,
                    delay_ms: None,
                    drop_rate: None,
                    reason: None,
                },
            ],
            transport: SwarmTransportAssumptions::in_memory_logical(),
            mothership_dependencies: Vec::new(),
        };

        let route = route_between(&topology, DEFAULT_GCS_NODE_ID, &agents[0].agent_id);

        assert!(route.allowed);
        assert_eq!(
            route.via_node_ids,
            vec![
                DEFAULT_GCS_NODE_ID.to_owned(),
                "relay:1".to_owned(),
                agent_node_id(&agents[0].agent_id)
            ]
        );
        assert_eq!(route.reason, "mesh_route");
    }

    #[test]
    fn partition_blocks_command_path_and_marks_degraded() {
        let agents = vec![agent("agent-0")];
        let mut topology = SwarmTopologyConfig::centralized_gcs_for_agents(&agents);
        topology.kind = SwarmTopologyKind::Mesh;
        topology.links.clear();

        let route = route_between(&topology, DEFAULT_GCS_NODE_ID, &agents[0].agent_id);

        assert!(!route.allowed);
        assert!(route.degraded);
        assert_eq!(route.reason, "mesh_partition_or_blocked_link");
    }

    #[test]
    fn mothership_dependency_cycle_is_detected() {
        let dependencies = vec![
            SwarmMothershipDependency {
                parent_agent_id: AgentId::from("agent-0".to_owned()),
                child_agent_id: AgentId::from("agent-1".to_owned()),
                dependency_kind: "launch".to_owned(),
                reason: "test".to_owned(),
            },
            SwarmMothershipDependency {
                parent_agent_id: AgentId::from("agent-1".to_owned()),
                child_agent_id: AgentId::from("agent-0".to_owned()),
                dependency_kind: "recover".to_owned(),
                reason: "test".to_owned(),
            },
        ];

        assert_eq!(
            has_mothership_cycle(&dependencies),
            Some(AgentId::from("agent-0".to_owned()))
        );
    }

    #[test]
    fn topology_config_serializes_snake_case() {
        let topology = SwarmTopologyConfig::centralized_gcs_for_agents(&[agent("agent-0")]);
        let json = serde_json::to_string(&topology).unwrap();

        assert!(json.contains("\"centralized_gcs\""));
        assert!(json.contains("\"in_memory\""));
    }
}
