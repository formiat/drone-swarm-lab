use swarm_types::AgentId;

use crate::types::{
    SwarmAbortPolicy, SwarmCommandPlan, SwarmCommandRole, SwarmOwnershipHandoff,
    SwarmOwnershipRecord, SwarmOwnershipStatus,
};
use crate::validation::{ensure_replacement_candidate_for_agent, SwarmCommandPlaneError};

/// Deterministic failure decision for one failed agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SwarmFailureDecision {
    AbortAgentOnly {
        agent_id: AgentId,
    },
    AbortMission {
        failed_agent_id: AgentId,
        abort_agent_ids: Vec<AgentId>,
    },
    ContinueDegraded {
        failed_agent_id: AgentId,
        released: Vec<SwarmOwnershipRecord>,
    },
    ReplaceFromReserve {
        failed_agent_id: AgentId,
        replacement_agent_id: AgentId,
        handoffs: Vec<SwarmOwnershipHandoff>,
    },
}

/// Apply an agent failure according to the failed agent's per-agent policy.
pub fn apply_agent_failure(
    plan: &SwarmCommandPlan,
    failed_agent_id: &AgentId,
) -> Result<SwarmFailureDecision, SwarmCommandPlaneError> {
    let failed_agent = plan
        .agents
        .iter()
        .find(|agent| &agent.agent_id == failed_agent_id)
        .ok_or_else(|| SwarmCommandPlaneError::MissingFailedAgent {
            agent_id: failed_agent_id.clone(),
        })?;
    match failed_agent.abort_policy {
        SwarmAbortPolicy::AbortAgentOnly => Ok(SwarmFailureDecision::AbortAgentOnly {
            agent_id: failed_agent_id.clone(),
        }),
        SwarmAbortPolicy::AbortMission => Ok(SwarmFailureDecision::AbortMission {
            failed_agent_id: failed_agent_id.clone(),
            abort_agent_ids: plan
                .agents
                .iter()
                .map(|agent| agent.agent_id.clone())
                .collect(),
        }),
        SwarmAbortPolicy::ContinueDegraded => Ok(SwarmFailureDecision::ContinueDegraded {
            failed_agent_id: failed_agent_id.clone(),
            released: released_ownership(plan, failed_agent_id),
        }),
        SwarmAbortPolicy::ReplaceFromReserve => {
            ensure_replacement_candidate_for_agent(plan, failed_agent_id)?;
            let replacement_agent_id = plan
                .agents
                .iter()
                .filter(|agent| &agent.agent_id != failed_agent_id)
                .find(|agent| {
                    matches!(
                        agent.role,
                        SwarmCommandRole::Reserve | SwarmCommandRole::Recovery
                    )
                })
                .map(|agent| agent.agent_id.clone())
                .ok_or(SwarmCommandPlaneError::MissingReplacementAgent)?;
            let handoffs = plan
                .ownership
                .iter()
                .filter(|record| {
                    &record.agent_id == failed_agent_id
                        && record.status == SwarmOwnershipStatus::Active
                })
                .map(|record| SwarmOwnershipHandoff {
                    from_agent_id: failed_agent_id.clone(),
                    to_agent_id: replacement_agent_id.clone(),
                    kind: record.kind.clone(),
                    resource_id: record.resource_id.clone(),
                    tick: record.tick,
                    reason: "agent_failure_replacement".to_owned(),
                })
                .collect();
            Ok(SwarmFailureDecision::ReplaceFromReserve {
                failed_agent_id: failed_agent_id.clone(),
                replacement_agent_id,
                handoffs,
            })
        }
    }
}

fn released_ownership(
    plan: &SwarmCommandPlan,
    failed_agent_id: &AgentId,
) -> Vec<SwarmOwnershipRecord> {
    plan.ownership
        .iter()
        .filter(|record| {
            &record.agent_id == failed_agent_id && record.status == SwarmOwnershipStatus::Active
        })
        .cloned()
        .collect()
}
