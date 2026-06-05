use crate::types::{
    SwarmCommandArtifactSummary, SwarmCommandPlan, SwarmOwnershipStatus,
    SWARM_COMMAND_PLANE_SCHEMA_VERSION,
};

/// Build a compact summary for embedding in manifests and reports.
pub fn summarize_swarm_command_plan(plan: &SwarmCommandPlan) -> SwarmCommandArtifactSummary {
    SwarmCommandArtifactSummary {
        schema_version: SWARM_COMMAND_PLANE_SCHEMA_VERSION.to_owned(),
        plan_id: plan.plan_id.clone(),
        agent_plan_count: plan.agents.len(),
        active_ownership_count: plan
            .ownership
            .iter()
            .filter(|record| record.status == SwarmOwnershipStatus::Active)
            .count(),
        handoff_count: plan.handoffs.len(),
        sync_operation_count: plan.sync_operations.len(),
    }
}
