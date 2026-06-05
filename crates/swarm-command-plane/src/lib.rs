pub mod fanout;
pub mod policy;
pub mod summary;
pub mod sync;
pub mod topology;
pub mod types;
pub mod validation;

pub use fanout::{build_swarm_command_plan, AgentCommandAssignment, SwarmCommandFanoutInput};
pub use policy::{apply_agent_failure, SwarmFailureDecision};
pub use summary::summarize_swarm_command_plan;
pub use sync::{evaluate_synchronized_command, SyncAgentOutcome};
pub use topology::{agent_node_id, route_between, route_command_plan, DEFAULT_GCS_NODE_ID};
pub use types::{
    PartialSuccessPolicy, SwarmAbortPolicy, SwarmAgentCommandPlan, SwarmCommandArtifactSummary,
    SwarmCommandPlan, SwarmCommandRole, SwarmCommandRoute, SwarmMothershipDependency,
    SwarmOwnershipHandoff, SwarmOwnershipKind, SwarmOwnershipRecord, SwarmOwnershipRef,
    SwarmOwnershipStatus, SwarmSupervisorState, SwarmTopologyConfig, SwarmTopologyKind,
    SwarmTopologyLink, SwarmTopologyNode, SwarmTopologyNodeKind, SwarmTransportAssumptions,
    SwarmTransportDeliveryModel, SynchronizedCommandKind, SynchronizedCommandResult,
    SynchronizedCommandWindow, SWARM_COMMAND_PLANE_SCHEMA_VERSION,
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
        abort_policy: SwarmAbortPolicy,
    ) -> AgentCommandAssignment {
        AgentCommandAssignment {
            agent_id: AgentId::from(agent_id.to_owned()),
            role,
            command_plan: command_plan(agent_id, resource_id),
            abort_policy,
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
                assignment("agent-0", SwarmCommandRole::Scout, "wp-0", policy.clone()),
                assignment(
                    "agent-1",
                    SwarmCommandRole::Reserve,
                    "wp-1",
                    SwarmAbortPolicy::AbortAgentOnly,
                ),
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
            topology: None,
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
            plan.summary.topology_kind,
            Some(SwarmTopologyKind::CentralizedGcs)
        );
        assert_eq!(plan.summary.command_route_count, 2);
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
    fn per_agent_abort_policy_overrides_global_policy() {
        let mut input = fanout_input(SwarmAbortPolicy::AbortMission);
        input.assignments[0].abort_policy = SwarmAbortPolicy::AbortAgentOnly;
        let plan = build_swarm_command_plan(input).unwrap();

        let decision = apply_agent_failure(&plan, &agent_id("agent-0")).unwrap();

        assert!(matches!(
            decision,
            SwarmFailureDecision::AbortAgentOnly { agent_id: failed } if failed == agent_id("agent-0")
        ));
    }

    #[test]
    fn per_agent_replacement_policy_overrides_global_continue_degraded() {
        let mut input = fanout_input(SwarmAbortPolicy::ContinueDegraded);
        input.assignments[0].abort_policy = SwarmAbortPolicy::ReplaceFromReserve;
        let plan = build_swarm_command_plan(input).unwrap();

        let decision = apply_agent_failure(&plan, &agent_id("agent-0")).unwrap();

        assert!(matches!(
            decision,
            SwarmFailureDecision::ReplaceFromReserve { replacement_agent_id, .. }
                if replacement_agent_id == agent_id("agent-1")
        ));
    }

    #[test]
    fn missing_failed_agent_fails_policy_application() {
        let plan = build_swarm_command_plan(fanout_input(SwarmAbortPolicy::AbortMission)).unwrap();

        let error = apply_agent_failure(&plan, &agent_id("missing")).unwrap_err();

        assert!(matches!(
            error,
            SwarmCommandPlaneError::MissingFailedAgent { agent_id: missing } if missing == agent_id("missing")
        ));
    }

    #[test]
    fn global_replacement_policy_requires_reserve_or_recovery() {
        let mut input = fanout_input(SwarmAbortPolicy::ReplaceFromReserve);
        input.assignments[1].role = SwarmCommandRole::Observer;

        let error = build_swarm_command_plan(input).unwrap_err();

        assert_eq!(error, SwarmCommandPlaneError::MissingReplacementAgent);
    }

    #[test]
    fn per_agent_replacement_policy_requires_reserve_or_recovery() {
        let mut input = fanout_input(SwarmAbortPolicy::AbortMission);
        input.assignments[0].abort_policy = SwarmAbortPolicy::ReplaceFromReserve;
        input.assignments[1].role = SwarmCommandRole::Observer;

        let error = build_swarm_command_plan(input).unwrap_err();

        assert_eq!(error, SwarmCommandPlaneError::MissingReplacementAgent);
    }

    #[test]
    fn released_and_active_same_resource_requires_handoff() {
        let mut plan =
            build_swarm_command_plan(fanout_input(SwarmAbortPolicy::AbortMission)).unwrap();
        plan.ownership = vec![
            SwarmOwnershipRecord {
                status: SwarmOwnershipStatus::Released,
                ..ownership("agent-0", SwarmOwnershipKind::Task, "wp-0")
            },
            ownership("agent-1", SwarmOwnershipKind::Task, "wp-0"),
        ];
        plan.agents[0].ownership_refs.clear();
        plan.agents[1].ownership_refs = vec![SwarmOwnershipRef {
            kind: SwarmOwnershipKind::Task,
            resource_id: "wp-0".to_owned(),
        }];

        let error = validate_swarm_command_plan(&plan).unwrap_err();

        assert!(matches!(
            error,
            SwarmCommandPlaneError::MissingHandoffEvidence { resource_id, .. }
                if resource_id == "wp-0"
        ));
    }

    #[test]
    fn topology_validation_rejects_unknown_route_node() {
        let mut plan =
            build_swarm_command_plan(fanout_input(SwarmAbortPolicy::AbortMission)).unwrap();
        plan.command_routes[0]
            .via_node_ids
            .push("missing-node".to_owned());

        let error = validate_swarm_command_plan(&plan).unwrap_err();

        assert!(matches!(
            error,
            SwarmCommandPlaneError::UnknownCommandRouteNode { node_id, .. } if node_id == "missing-node"
        ));
    }

    #[test]
    fn topology_validation_rejects_route_without_available_link() {
        let mut plan =
            build_swarm_command_plan(fanout_input(SwarmAbortPolicy::AbortMission)).unwrap();
        let topology = plan.topology.as_mut().unwrap();
        topology.links.retain(|link| {
            !(link.from_node_id == DEFAULT_GCS_NODE_ID && link.to_node_id == "agent:agent-0")
        });

        let error = validate_swarm_command_plan(&plan).unwrap_err();

        assert!(matches!(
            error,
            SwarmCommandPlaneError::CommandRoutePathMismatch { .. }
        ));
    }

    #[test]
    fn topology_validation_rejects_mothership_child_route_bypassing_parent() {
        let mut input = fanout_input(SwarmAbortPolicy::AbortMission);
        let mut topology = SwarmTopologyConfig::centralized_gcs_for_agents(&[]);
        topology.kind = SwarmTopologyKind::Mothership;
        topology.nodes = vec![
            SwarmTopologyNode {
                node_id: DEFAULT_GCS_NODE_ID.to_owned(),
                agent_id: None,
                kind: SwarmTopologyNodeKind::Gcs,
                available: true,
            },
            SwarmTopologyNode {
                node_id: "agent:agent-0".to_owned(),
                agent_id: Some(agent_id("agent-0")),
                kind: SwarmTopologyNodeKind::Mothership,
                available: true,
            },
            SwarmTopologyNode {
                node_id: "agent:agent-1".to_owned(),
                agent_id: Some(agent_id("agent-1")),
                kind: SwarmTopologyNodeKind::Agent,
                available: true,
            },
        ];
        topology.links = vec![
            SwarmTopologyLink {
                from_node_id: DEFAULT_GCS_NODE_ID.to_owned(),
                to_node_id: "agent:agent-0".to_owned(),
                available: true,
                delay_ms: Some(0),
                drop_rate: Some(0.0),
                reason: None,
            },
            SwarmTopologyLink {
                from_node_id: DEFAULT_GCS_NODE_ID.to_owned(),
                to_node_id: "agent:agent-1".to_owned(),
                available: true,
                delay_ms: Some(0),
                drop_rate: Some(0.0),
                reason: None,
            },
            SwarmTopologyLink {
                from_node_id: "agent:agent-0".to_owned(),
                to_node_id: "agent:agent-1".to_owned(),
                available: true,
                delay_ms: Some(0),
                drop_rate: Some(0.0),
                reason: None,
            },
        ];
        topology.mothership_dependencies = vec![SwarmMothershipDependency {
            parent_agent_id: agent_id("agent-0"),
            child_agent_id: agent_id("agent-1"),
            dependency_kind: "deploy".to_owned(),
            reason: "child depends on parent carrier".to_owned(),
        }];
        input.topology = Some(topology);
        let mut plan = build_swarm_command_plan(input).unwrap();
        let child_route = plan
            .command_routes
            .iter_mut()
            .find(|route| route.to_agent_id == agent_id("agent-1"))
            .unwrap();
        child_route.via_node_ids = vec![DEFAULT_GCS_NODE_ID.to_owned(), "agent:agent-1".to_owned()];

        let error = validate_swarm_command_plan(&plan).unwrap_err();

        assert!(matches!(
            error,
            SwarmCommandPlaneError::CommandRoutePathMismatch { .. }
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
