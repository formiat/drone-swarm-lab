use swarm_comms::{compile_mavlink_common_plan, MavlinkCommonPlanOptions};
use swarm_mission_ir::MissionCommandPlan;
use swarm_types::AgentId;

use crate::summary::summarize_swarm_command_plan;
use crate::topology::route_command_plan;
use crate::types::{
    SwarmAbortPolicy, SwarmAgentCommandPlan, SwarmCommandPlan, SwarmCommandRole,
    SwarmOwnershipRecord, SwarmOwnershipRef, SwarmSupervisorState, SwarmTopologyConfig,
    SynchronizedCommandWindow, SWARM_COMMAND_PLANE_SCHEMA_VERSION,
};
use crate::validation::{validate_swarm_command_plan, SwarmCommandPlaneError};

/// One command-plane input assignment for an agent.
#[derive(Clone, Debug)]
pub struct AgentCommandAssignment {
    pub agent_id: AgentId,
    pub role: SwarmCommandRole,
    pub command_plan: MissionCommandPlan,
    pub abort_policy: SwarmAbortPolicy,
    pub ownership_refs: Vec<SwarmOwnershipRef>,
}

/// Input used to build a complete M87 swarm command plan.
#[derive(Clone, Debug)]
pub struct SwarmCommandFanoutInput {
    pub plan_id: String,
    pub assignments: Vec<AgentCommandAssignment>,
    pub ownership: Vec<SwarmOwnershipRecord>,
    pub global_abort_policy: SwarmAbortPolicy,
    pub sync_operations: Vec<SynchronizedCommandWindow>,
    pub topology: Option<SwarmTopologyConfig>,
    pub mavlink_options: MavlinkCommonPlanOptions,
}

/// Build a validated M87 command-plane artifact from per-agent assignments.
pub fn build_swarm_command_plan(
    input: SwarmCommandFanoutInput,
) -> Result<SwarmCommandPlan, SwarmCommandPlaneError> {
    let mut agents = Vec::with_capacity(input.assignments.len());
    for assignment in input.assignments {
        let mavlink_plan =
            compile_mavlink_common_plan(&assignment.command_plan, &input.mavlink_options).map_err(
                |error| SwarmCommandPlaneError::UnsupportedSchema {
                    actual: error.to_string(),
                },
            )?;
        agents.push(SwarmAgentCommandPlan {
            agent_id: assignment.agent_id,
            role: assignment.role,
            expected_acks: mavlink_plan.expected_acks.clone(),
            telemetry_milestones: mavlink_plan.telemetry_milestones.clone(),
            command_plan: assignment.command_plan,
            mavlink_plan,
            abort_policy: assignment.abort_policy,
            ownership_refs: assignment.ownership_refs,
        });
    }

    let topology = input
        .topology
        .unwrap_or_else(|| SwarmTopologyConfig::centralized_gcs_for_agents(&agents));
    let command_routes = route_command_plan(&topology, &agents);

    let mut plan = SwarmCommandPlan {
        schema_version: SWARM_COMMAND_PLANE_SCHEMA_VERSION.to_owned(),
        plan_id: input.plan_id,
        supervisor_state: SwarmSupervisorState::Planned,
        agents,
        ownership: input.ownership,
        handoffs: Vec::new(),
        global_abort_policy: input.global_abort_policy,
        sync_operations: input.sync_operations,
        sync_results: Vec::new(),
        topology: Some(topology),
        command_routes,
        summary: Default::default(),
    };
    plan.summary = summarize_swarm_command_plan(&plan);
    validate_swarm_command_plan(&plan)?;
    Ok(plan)
}
