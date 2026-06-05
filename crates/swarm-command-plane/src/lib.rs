pub mod fanout;
pub mod policy;
pub mod summary;
pub mod sync;
pub mod types;
pub mod validation;

pub use fanout::{build_swarm_command_plan, AgentCommandAssignment, SwarmCommandFanoutInput};
pub use policy::{apply_agent_failure, SwarmFailureDecision};
pub use summary::summarize_swarm_command_plan;
pub use sync::{evaluate_synchronized_command, SyncAgentOutcome};
pub use types::{
    PartialSuccessPolicy, SwarmAbortPolicy, SwarmAgentCommandPlan, SwarmCommandArtifactSummary,
    SwarmCommandPlan, SwarmCommandRole, SwarmOwnershipHandoff, SwarmOwnershipKind,
    SwarmOwnershipRecord, SwarmOwnershipRef, SwarmOwnershipStatus, SwarmSupervisorState,
    SynchronizedCommandKind, SynchronizedCommandResult, SynchronizedCommandWindow,
    SWARM_COMMAND_PLANE_SCHEMA_VERSION,
};
pub use validation::{validate_swarm_command_plan, SwarmCommandPlaneError};

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use swarm_comms::MavlinkCommonPlanOptions;
    use swarm_mission_ir::{
        AltitudeReference, CommandId, CompletionTolerance, CoordinateFrame, MissionCommand,
        MissionCommandEntry, MissionCommandPlan, MissionId, TerminalState, TimeoutAction,
        TimeoutPolicy,
    };
    use swarm_types::AgentId;

    use super::*;

    fn agent_id(value: &str) -> AgentId {
        AgentId::from(value.to_owned())
    }

    fn command_plan(agent_id: &str, mission_id: &str) -> MissionCommandPlan {
        MissionCommandPlan {
            schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
            mission_id: MissionId::from(mission_id.to_owned()),
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
                command_id: CommandId::from(format!("{mission_id}-arm")),
                command: MissionCommand::Arm,
                source_task_id: Some(format!("{mission_id}-task")),
                source_route_id: None,
                source_agent_id: Some(agent_id.to_owned()),
            }],
        }
    }

    fn ownership(
        agent_id: &str,
        kind: SwarmOwnershipKind,
        resource_id: &str,
    ) -> SwarmOwnershipRecord {
        SwarmOwnershipRecord {
            agent_id: AgentId::from(agent_id.to_owned()),
            kind,
            resource_id: resource_id.to_owned(),
            status: SwarmOwnershipStatus::Active,
            tick: 1,
            reason: "initial_assignment".to_owned(),
        }
    }

    fn assignment(
        agent_id: &str,
        role: SwarmCommandRole,
        resource_id: &str,
    ) -> AgentCommandAssignment {
        AgentCommandAssignment {
            agent_id: AgentId::from(agent_id.to_owned()),
            role,
            command_plan: command_plan(agent_id, resource_id),
            abort_policy: SwarmAbortPolicy::AbortAgentOnly,
            ownership_refs: vec![SwarmOwnershipRef {
                kind: SwarmOwnershipKind::Task,
                resource_id: resource_id.to_owned(),
            }],
        }
    }

    fn fanout_input(policy: SwarmAbortPolicy) -> SwarmCommandFanoutInput {
        SwarmCommandFanoutInput {
            plan_id: "plan-1".to_owned(),
            assignments: vec![
                assignment("agent-0", SwarmCommandRole::Scout, "wp-0"),
                assignment("agent-1", SwarmCommandRole::Reserve, "wp-1"),
            ],
            ownership: vec![
                ownership("agent-0", SwarmOwnershipKind::Task, "wp-0"),
                ownership("agent-1", SwarmOwnershipKind::Task, "wp-1"),
            ],
            global_abort_policy: policy,
            sync_operations: vec![SynchronizedCommandWindow {
                kind: SynchronizedCommandKind::ArmAll,
                agent_ids: vec![agent_id("agent-0"), agent_id("agent-1")],
                timeout_ms: 1_000,
                partial_success_policy: PartialSuccessPolicy::RequireAll,
            }],
            mavlink_options: MavlinkCommonPlanOptions::default(),
        }
    }

    #[test]
    fn command_fanout_creates_one_plan_per_assignment() {
        let plan = build_swarm_command_plan(fanout_input(SwarmAbortPolicy::AbortMission)).unwrap();

        assert_eq!(plan.schema_version, SWARM_COMMAND_PLANE_SCHEMA_VERSION);
        assert_eq!(plan.agents.len(), 2);
        assert_eq!(plan.summary.agent_plan_count, 2);
        assert_eq!(
            plan.agents[0].expected_acks.len(),
            plan.agents[0].mavlink_plan.expected_acks.len()
        );
        assert_eq!(
            plan.agents[0].telemetry_milestones.len(),
            plan.agents[0].mavlink_plan.telemetry_milestones.len()
        );
    }

    #[test]
    fn duplicate_task_ownership_fails_validation() {
        let mut input = fanout_input(SwarmAbortPolicy::AbortMission);
        input
            .ownership
            .push(ownership("agent-1", SwarmOwnershipKind::Task, "wp-0"));

        let error = build_swarm_command_plan(input).unwrap_err();

        assert!(matches!(
            error,
            SwarmCommandPlaneError::DuplicateOwnership { .. }
        ));
    }

    #[test]
    fn duplicate_route_segment_ownership_fails_validation() {
        let mut plan =
            build_swarm_command_plan(fanout_input(SwarmAbortPolicy::AbortMission)).unwrap();
        plan.ownership = vec![
            ownership("agent-0", SwarmOwnershipKind::RouteSegment, "edge-0"),
            ownership("agent-1", SwarmOwnershipKind::RouteSegment, "edge-0"),
        ];
        plan.agents[0].ownership_refs.clear();
        plan.agents[1].ownership_refs.clear();

        let error = validate_swarm_command_plan(&plan).unwrap_err();

        assert!(matches!(
            error,
            SwarmCommandPlaneError::DuplicateOwnership { .. }
        ));
    }

    #[test]
    fn released_handoff_does_not_count_as_duplicate_active_ownership() {
        let mut plan =
            build_swarm_command_plan(fanout_input(SwarmAbortPolicy::AbortMission)).unwrap();
        plan.ownership = vec![
            SwarmOwnershipRecord {
                status: SwarmOwnershipStatus::Released,
                ..ownership("agent-0", SwarmOwnershipKind::Task, "wp-0")
            },
            ownership("agent-1", SwarmOwnershipKind::Task, "wp-0"),
        ];
        plan.handoffs.push(SwarmOwnershipHandoff {
            from_agent_id: agent_id("agent-0"),
            to_agent_id: agent_id("agent-1"),
            kind: SwarmOwnershipKind::Task,
            resource_id: "wp-0".to_owned(),
            tick: 2,
            reason: "replacement".to_owned(),
        });
        plan.agents[0].ownership_refs.clear();
        plan.agents[1].ownership_refs = vec![SwarmOwnershipRef {
            kind: SwarmOwnershipKind::Task,
            resource_id: "wp-0".to_owned(),
        }];

        validate_swarm_command_plan(&plan).unwrap();
    }

    #[test]
    fn failed_agent_triggers_replacement_policy() {
        let plan =
            build_swarm_command_plan(fanout_input(SwarmAbortPolicy::ReplaceFromReserve)).unwrap();

        let decision = apply_agent_failure(&plan, &agent_id("agent-0")).unwrap();

        assert!(matches!(
            decision,
            SwarmFailureDecision::ReplaceFromReserve { replacement_agent_id, .. }
                if replacement_agent_id == agent_id("agent-1")
        ));
    }

    #[test]
    fn global_abort_targets_all_agents() {
        let plan = build_swarm_command_plan(fanout_input(SwarmAbortPolicy::AbortMission)).unwrap();

        let decision = apply_agent_failure(&plan, &agent_id("agent-0")).unwrap();

        assert!(matches!(
            decision,
            SwarmFailureDecision::AbortMission { abort_agent_ids, .. }
                if abort_agent_ids == vec![agent_id("agent-0"), agent_id("agent-1")]
        ));
    }

    #[test]
    fn synchronized_takeoff_reports_partial_failure_deterministically() {
        let window = SynchronizedCommandWindow {
            kind: SynchronizedCommandKind::TakeoffAll,
            agent_ids: vec![agent_id("agent-0"), agent_id("agent-1")],
            timeout_ms: 5_000,
            partial_success_policy: PartialSuccessPolicy::AtLeast { agents: 1 },
        };
        let mut outcomes = HashMap::new();
        outcomes.insert(agent_id("agent-1"), SyncAgentOutcome::Failed);

        let result = evaluate_synchronized_command(&window, &outcomes);

        assert!(result.accepted);
        assert_eq!(result.succeeded, vec![agent_id("agent-0")]);
        assert_eq!(result.failed, vec![agent_id("agent-1")]);
        assert!(result.timed_out.is_empty());
    }

    #[test]
    fn role_and_state_serialize_as_snake_case() {
        let role = serde_json::to_string(&SwarmCommandRole::Mothership).unwrap();
        let policy = serde_json::to_string(&SwarmAbortPolicy::ContinueDegraded).unwrap();

        assert_eq!(role, "\"mothership\"");
        assert_eq!(policy, "\"continue_degraded\"");

        let state = serde_json::to_string(&SwarmSupervisorState::Replacing).unwrap();
        assert_eq!(state, "\"replacing\"");
    }
}
