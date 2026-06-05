use std::collections::{HashMap, HashSet};

use swarm_types::AgentId;
use thiserror::Error;

use crate::topology::has_mothership_cycle;
use crate::types::{
    SwarmAbortPolicy, SwarmCommandPlan, SwarmOwnershipKind, SwarmOwnershipStatus,
    SwarmTopologyConfig,
};

/// M87 command-plane validation error.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SwarmCommandPlaneError {
    #[error("unsupported command-plane schema '{actual}'")]
    UnsupportedSchema { actual: String },
    #[error("duplicate command plan for agent {agent_id}")]
    DuplicateAgentPlan { agent_id: AgentId },
    #[error("duplicate ownership for {kind:?}:{resource_id}")]
    DuplicateOwnership {
        kind: SwarmOwnershipKind,
        resource_id: String,
    },
    #[error(
        "command entry {command_id} has source_agent_id {source_agent_id}, expected {agent_id}"
    )]
    SourceAgentMismatch {
        command_id: String,
        source_agent_id: String,
        agent_id: AgentId,
    },
    #[error("agent {agent_id} references ownership {kind:?}:{resource_id} that is not active")]
    MissingActiveOwnership {
        agent_id: AgentId,
        kind: SwarmOwnershipKind,
        resource_id: String,
    },
    #[error("replacement policy requires reserve or recovery agent")]
    MissingReplacementAgent,
    #[error("failed agent {agent_id} is not present in command plan")]
    MissingFailedAgent { agent_id: AgentId },
    #[error("ownership handoff missing for {kind:?}:{resource_id} from {from_agent_id} to {to_agent_id}")]
    MissingHandoffEvidence {
        kind: SwarmOwnershipKind,
        resource_id: String,
        from_agent_id: AgentId,
        to_agent_id: AgentId,
    },
    #[error("abort mission policy requires at least one agent command plan")]
    MissingAbortTargets,
    #[error("topology node '{node_id}' is required but missing")]
    MissingTopologyNode { node_id: String },
    #[error("duplicate topology node '{node_id}'")]
    DuplicateTopologyNode { node_id: String },
    #[error("topology link references unknown node '{node_id}'")]
    UnknownTopologyLinkEndpoint { node_id: String },
    #[error("missing command route for agent {agent_id}")]
    MissingCommandRoute { agent_id: AgentId },
    #[error("blocked command route '{route_id}' must include a reason")]
    BlockedRouteWithoutReason { route_id: String },
    #[error("mothership dependency graph contains a cycle at agent {agent_id}")]
    MothershipDependencyCycle { agent_id: AgentId },
    #[error("mothership dependency references unknown agent {agent_id}")]
    UnknownMothershipDependencyAgent { agent_id: AgentId },
    #[error("topology transport assumptions are incomplete")]
    MissingTransportAssumption,
}

/// Validate a complete M87 command-plane artifact.
pub fn validate_swarm_command_plan(plan: &SwarmCommandPlan) -> Result<(), SwarmCommandPlaneError> {
    if plan.schema_version != crate::types::SWARM_COMMAND_PLANE_SCHEMA_VERSION {
        return Err(SwarmCommandPlaneError::UnsupportedSchema {
            actual: plan.schema_version.clone(),
        });
    }

    let mut agent_ids = HashSet::new();
    for agent in &plan.agents {
        if !agent_ids.insert(agent.agent_id.clone()) {
            return Err(SwarmCommandPlaneError::DuplicateAgentPlan {
                agent_id: agent.agent_id.clone(),
            });
        }
        for entry in &agent.command_plan.commands {
            let Some(source_agent_id) = &entry.source_agent_id else {
                continue;
            };
            if source_agent_id != &agent.agent_id.to_string() {
                return Err(SwarmCommandPlaneError::SourceAgentMismatch {
                    command_id: entry.command_id.to_string(),
                    source_agent_id: source_agent_id.clone(),
                    agent_id: agent.agent_id.clone(),
                });
            }
        }
    }

    let mut active_ownership = HashMap::new();
    let mut released_ownership: HashMap<(SwarmOwnershipKind, String), Vec<AgentId>> =
        HashMap::new();
    for record in &plan.ownership {
        if record.status == SwarmOwnershipStatus::Released {
            released_ownership
                .entry((record.kind.clone(), record.resource_id.clone()))
                .or_default()
                .push(record.agent_id.clone());
            continue;
        }
        let key = (record.kind.clone(), record.resource_id.clone());
        if active_ownership
            .insert(key.clone(), record.agent_id.clone())
            .is_some()
        {
            return Err(SwarmCommandPlaneError::DuplicateOwnership {
                kind: key.0,
                resource_id: key.1,
            });
        }
    }

    validate_handoff_evidence(plan, &active_ownership, &released_ownership)?;

    for agent in &plan.agents {
        for ownership in &agent.ownership_refs {
            let owner =
                active_ownership.get(&(ownership.kind.clone(), ownership.resource_id.clone()));
            if owner != Some(&agent.agent_id) {
                return Err(SwarmCommandPlaneError::MissingActiveOwnership {
                    agent_id: agent.agent_id.clone(),
                    kind: ownership.kind.clone(),
                    resource_id: ownership.resource_id.clone(),
                });
            }
        }
    }

    if plan.global_abort_policy == SwarmAbortPolicy::AbortMission && plan.agents.is_empty() {
        return Err(SwarmCommandPlaneError::MissingAbortTargets);
    }
    if plan.global_abort_policy == SwarmAbortPolicy::ReplaceFromReserve {
        ensure_replacement_candidate(plan)?;
    }
    for agent in &plan.agents {
        if agent.abort_policy == SwarmAbortPolicy::ReplaceFromReserve {
            ensure_replacement_candidate_for_agent(plan, &agent.agent_id)?;
        }
    }
    if let Some(topology) = &plan.topology {
        validate_topology(plan, topology)?;
    }

    Ok(())
}

fn validate_topology(
    plan: &SwarmCommandPlan,
    topology: &SwarmTopologyConfig,
) -> Result<(), SwarmCommandPlaneError> {
    if topology.transport.hardware_boundary.trim().is_empty() {
        return Err(SwarmCommandPlaneError::MissingTransportAssumption);
    }

    let mut node_ids = HashSet::new();
    for node in &topology.nodes {
        if !node_ids.insert(node.node_id.clone()) {
            return Err(SwarmCommandPlaneError::DuplicateTopologyNode {
                node_id: node.node_id.clone(),
            });
        }
    }
    if !node_ids.contains(&topology.gcs_node_id) {
        return Err(SwarmCommandPlaneError::MissingTopologyNode {
            node_id: topology.gcs_node_id.clone(),
        });
    }
    for link in &topology.links {
        for endpoint in [&link.from_node_id, &link.to_node_id] {
            if !node_ids.contains(endpoint) {
                return Err(SwarmCommandPlaneError::UnknownTopologyLinkEndpoint {
                    node_id: endpoint.clone(),
                });
            }
        }
    }

    let agent_ids: HashSet<_> = plan
        .agents
        .iter()
        .map(|agent| agent.agent_id.clone())
        .collect();
    for agent in &plan.agents {
        let has_node = topology
            .nodes
            .iter()
            .any(|node| node.agent_id.as_ref() == Some(&agent.agent_id));
        if !has_node {
            return Err(SwarmCommandPlaneError::MissingTopologyNode {
                node_id: format!("agent:{}", agent.agent_id),
            });
        }
        let Some(route) = plan
            .command_routes
            .iter()
            .find(|route| route.to_agent_id == agent.agent_id)
        else {
            return Err(SwarmCommandPlaneError::MissingCommandRoute {
                agent_id: agent.agent_id.clone(),
            });
        };
        if !route.allowed && route.reason.trim().is_empty() {
            return Err(SwarmCommandPlaneError::BlockedRouteWithoutReason {
                route_id: route.route_id.clone(),
            });
        }
    }

    for dependency in &topology.mothership_dependencies {
        for agent_id in [&dependency.parent_agent_id, &dependency.child_agent_id] {
            if !agent_ids.contains(agent_id) {
                return Err(SwarmCommandPlaneError::UnknownMothershipDependencyAgent {
                    agent_id: agent_id.clone(),
                });
            }
        }
    }
    if let Some(agent_id) = has_mothership_cycle(&topology.mothership_dependencies) {
        return Err(SwarmCommandPlaneError::MothershipDependencyCycle { agent_id });
    }

    Ok(())
}

fn validate_handoff_evidence(
    plan: &SwarmCommandPlan,
    active_ownership: &HashMap<(SwarmOwnershipKind, String), AgentId>,
    released_ownership: &HashMap<(SwarmOwnershipKind, String), Vec<AgentId>>,
) -> Result<(), SwarmCommandPlaneError> {
    for (key, from_agent_ids) in released_ownership {
        let Some(to_agent_id) = active_ownership.get(key) else {
            continue;
        };
        for from_agent_id in from_agent_ids {
            if from_agent_id == to_agent_id {
                continue;
            }
            let has_handoff = plan.handoffs.iter().any(|handoff| {
                handoff.kind == key.0
                    && handoff.resource_id == key.1
                    && handoff.from_agent_id == *from_agent_id
                    && handoff.to_agent_id == *to_agent_id
            });
            if !has_handoff {
                return Err(SwarmCommandPlaneError::MissingHandoffEvidence {
                    kind: key.0.clone(),
                    resource_id: key.1.clone(),
                    from_agent_id: from_agent_id.clone(),
                    to_agent_id: to_agent_id.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Check that at least one reserve/recovery candidate exists for replacement.
pub(crate) fn ensure_replacement_candidate(
    plan: &SwarmCommandPlan,
) -> Result<(), SwarmCommandPlaneError> {
    let has_candidate = plan.agents.iter().any(|agent| {
        matches!(
            agent.role,
            crate::types::SwarmCommandRole::Reserve | crate::types::SwarmCommandRole::Recovery
        )
    });
    if has_candidate {
        Ok(())
    } else {
        Err(SwarmCommandPlaneError::MissingReplacementAgent)
    }
}

pub(crate) fn ensure_replacement_candidate_for_agent(
    plan: &SwarmCommandPlan,
    failed_agent_id: &AgentId,
) -> Result<(), SwarmCommandPlaneError> {
    let has_candidate = plan.agents.iter().any(|agent| {
        &agent.agent_id != failed_agent_id
            && matches!(
                agent.role,
                crate::types::SwarmCommandRole::Reserve | crate::types::SwarmCommandRole::Recovery
            )
    });
    if has_candidate {
        Ok(())
    } else {
        Err(SwarmCommandPlaneError::MissingReplacementAgent)
    }
}
