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
        topology_kind: plan.topology.as_ref().map(|topology| topology.kind.clone()),
        topology_node_count: plan
            .topology
            .as_ref()
            .map_or(0, |topology| topology.nodes.len()),
        topology_link_count: plan
            .topology
            .as_ref()
            .map_or(0, |topology| topology.links.len()),
        command_route_count: plan.command_routes.len(),
        degraded_route_count: plan
            .command_routes
            .iter()
            .filter(|route| route.degraded)
            .count(),
        mothership_dependency_count: plan
            .topology
            .as_ref()
            .map_or(0, |topology| topology.mothership_dependencies.len()),
    }
}
