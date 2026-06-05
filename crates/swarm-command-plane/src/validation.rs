use std::collections::{HashMap, HashSet};

use swarm_types::AgentId;
use thiserror::Error;

use crate::types::{SwarmAbortPolicy, SwarmCommandPlan, SwarmOwnershipKind, SwarmOwnershipStatus};

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
    #[error("abort mission policy requires at least one agent command plan")]
    MissingAbortTargets,
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
    for record in &plan.ownership {
        if record.status != SwarmOwnershipStatus::Active {
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
