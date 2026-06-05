use std::fs;
use std::path::{Path, PathBuf};

use swarm_command_plane::{
    build_swarm_command_plan, AgentCommandAssignment, SwarmAbortPolicy, SwarmCommandFanoutInput,
    SwarmCommandPlan, SwarmCommandRole, SwarmOwnershipKind, SwarmOwnershipRecord,
    SwarmOwnershipRef, SwarmOwnershipStatus, SynchronizedCommandKind, SynchronizedCommandResult,
};
use swarm_comms::{
    compile_mavlink_common_plan, MavlinkCommonCommand, MavlinkCommonCommandName,
    MavlinkCommonPlanOptions, MavlinkCompatibilityClass, MavlinkCoordinateOrigin,
    MavlinkExpectedAck, MavlinkExpectedAckKind, MavlinkPlanPhase,
};
use swarm_examples::artifact_validator::{
    validate_artifact_pack, ArtifactPackPaths, ArtifactValidationMode, ArtifactValidationOptions,
    RULE_BUILD_PROFILE_MISSING, RULE_COMPLETED_TASK_MISSING_EVENT, RULE_DEGRADED_EVENT_MISSING,
    RULE_DEGRADED_FINAL_STATUS_MISMATCH, RULE_DEGRADED_RECORD_MISSING,
    RULE_DEGRADED_RECOVERY_TASK_MISMATCH, RULE_DRY_RUN_POLICY_MISSING,
    RULE_DRY_RUN_SAFETY_REPORT_FAILED, RULE_DUAL_STACK_ABORT_POLICY_MISMATCH,
    RULE_DUAL_STACK_ABORT_REPLACEMENT_MISSING, RULE_DUAL_STACK_FC_CONTRACT_HIDDEN_CAVEAT,
    RULE_DUAL_STACK_FC_CONTRACT_MISSING, RULE_DUAL_STACK_HARDWARE_CLAIM_UNSAFE,
    RULE_DUAL_STACK_IR_HASH_MISMATCH, RULE_DUAL_STACK_PROFILE_MISMATCH,
    RULE_DUAL_STACK_PROFILE_MISSING, RULE_DUAL_STACK_REPLACEMENT_POLICY_MISMATCH,
    RULE_FINAL_STATUS_MISMATCH, RULE_MANIFEST_MISSING, RULE_MAVLINK_PLAN_MISSING,
    RULE_MAVLINK_PLAN_ORDER_UNSAFE, RULE_MAVLINK_PLAN_TELEMETRY_MISSING,
    RULE_MAVLINK_PROFILE_HARDWARE_BLOCKING, RULE_MAVLINK_PROFILE_MISSING,
    RULE_MAVLINK_PROFILE_RESULT_MISMATCH, RULE_MAVLINK_PROFILE_UNSUPPORTED,
    RULE_REPLACEMENT_SEQ_MISMATCH, RULE_REPLAY_SUMMARY_COUNT_MISMATCH, RULE_SAFETY_REPORT_MISSING,
    RULE_SWARM_ACK_MISMATCH, RULE_SWARM_AGENT_PLAN_MISSING, RULE_SWARM_DUPLICATE_OWNERSHIP,
    RULE_SWARM_HANDOFF_MISSING, RULE_SWARM_SYNC_PARTIAL_UNREPORTED,
    RULE_SWARM_TOPOLOGY_BLOCKED_UNREPORTED, RULE_SWARM_TOPOLOGY_ROUTE_MISSING,
    RULE_SWARM_TRANSPORT_ASSUMPTION_MISSING, RULE_URBAN_DECONFLICTION_DUPLICATE_SEGMENT_OWNER,
    RULE_URBAN_GEO_ROUTE_METADATA_MISSING, RULE_URBAN_MOCK_PERCEPTION_MISSING,
    RULE_URBAN_WGS84_GEO_MISSING,
};
use swarm_examples::sitl_dual_stack_evidence::{
    write_dual_stack_evidence_pack, SITL_DUAL_STACK_EVIDENCE_FILE,
};
use swarm_examples::sitl_multi_agent::{
    MultiAgentLifecycle, MultiAgentSitlManifest, MultiAgentSitlManifestAgent,
    MultiAgentSitlTopologySummary, SitlArtifactMetadata, TaskOwnershipSummary,
    MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION,
};
use swarm_examples::sitl_observability::{
    format_sitl_summary, summarize_sitl_event_log, SitlEvent, SitlEventLog, SitlEventLogMode,
};
use swarm_examples::sitl_plan::{SitlDryRunArtifact, SitlGlobalWaypointSummary, SitlWaypointItem};
use swarm_examples::sitl_report::{
    SitlMultiAgentAgentReport, SitlMultiAgentReallocationReport, SitlMultiAgentRunReport,
};
use swarm_examples::sitl_supervisor::{
    DegradedRunRecord, SitlDegradedRunReport, SupervisorDecision, SupervisorFailureMode,
};
use swarm_mission_ir::{
    AltitudeReference, CommandId, CompletionTolerance, CoordinateFrame, GeoPosition, LocalPosition,
    MissionCommand, MissionCommandEntry, MissionCommandPlan, MissionCommandSummary, MissionId,
    Position, TerminalState, TimeoutAction, TimeoutPolicy,
};
use swarm_safety::preflight::SafetyValidationReport;
use swarm_sim::GeoOrigin;
use swarm_types::UrbanGeoPoint;

#[test]
fn valid_tiny_supervisor_pack_passes() {
    let fixture = ArtifactFixture::new();

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert!(report.passed, "{:?}", report.violations);
}

#[test]
fn missing_manifest_fails_with_rule_id() {
    let fixture = ArtifactFixture::new();
    fs::remove_file(fixture.output_dir.join("manifest.json")).unwrap();

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_MANIFEST_MISSING);
}

#[test]
fn missing_manifest_metadata_fails_in_strict_mode() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        manifest.artifact_metadata.build_profile.clear();
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_BUILD_PROFILE_MISSING);
}

#[test]
fn command_plane_agent_count_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        manifest.command_plane.as_mut().unwrap().agent_plan_count = 1;
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SWARM_AGENT_PLAN_MISSING);
}

#[test]
fn command_plane_duplicate_ownership_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        manifest
            .command_plane_artifact
            .as_mut()
            .unwrap()
            .ownership
            .push(command_plane_ownership("agent-1", "wp-0"));
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SWARM_DUPLICATE_OWNERSHIP);
}

#[test]
fn command_plane_released_active_without_handoff_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        let artifact = manifest.command_plane_artifact.as_mut().unwrap();
        artifact.ownership = vec![
            SwarmOwnershipRecord {
                status: SwarmOwnershipStatus::Released,
                ..command_plane_ownership("agent-0", "wp-0")
            },
            command_plane_ownership("agent-1", "wp-0"),
        ];
        artifact.agents[0].ownership_refs.clear();
        artifact.agents[1].ownership_refs = vec![SwarmOwnershipRef {
            kind: SwarmOwnershipKind::Task,
            resource_id: "wp-0".to_owned(),
        }];
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SWARM_HANDOFF_MISSING);
}

#[test]
fn command_plane_ack_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        manifest.command_plane_artifact.as_mut().unwrap().agents[0]
            .expected_acks
            .clear();
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SWARM_ACK_MISMATCH);
}

#[test]
fn command_plane_partial_sync_result_without_window_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        manifest
            .command_plane_artifact
            .as_mut()
            .unwrap()
            .sync_results
            .push(SynchronizedCommandResult {
                kind: SynchronizedCommandKind::AbortAll,
                succeeded: Vec::new(),
                failed: vec![swarm_types::AgentId::from("agent-0".to_owned())],
                timed_out: Vec::new(),
                accepted: false,
            });
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SWARM_SYNC_PARTIAL_UNREPORTED);
}

#[test]
fn swarm_topology_missing_route_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        manifest
            .command_plane_artifact
            .as_mut()
            .unwrap()
            .command_routes
            .retain(|route| route.to_agent_id.to_string() != "agent-1");
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SWARM_TOPOLOGY_ROUTE_MISSING);
}

#[test]
fn swarm_topology_route_without_link_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        let artifact = manifest.command_plane_artifact.as_mut().unwrap();
        let topology = artifact.topology.as_mut().unwrap();
        topology
            .links
            .retain(|link| !(link.from_node_id == "gcs" && link.to_node_id == "agent:agent-0"));
        artifact.summary.topology_link_count = topology.links.len();
        if let Some(summary) = manifest.command_plane.as_mut() {
            summary.topology_link_count = topology.links.len();
        }
        if let Some(summary) = manifest.topology.as_mut() {
            summary.link_count = topology.links.len();
        }
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SWARM_TOPOLOGY_ROUTE_MISSING);
}

#[test]
fn swarm_topology_blocked_route_without_event_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(mark_agent_0_route_blocked);
    fixture.write_event_log(|_| {});

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SWARM_TOPOLOGY_BLOCKED_UNREPORTED);
}

#[test]
fn swarm_topology_blocked_route_with_matching_event_passes() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(mark_agent_0_route_blocked);
    fixture.write_event_log(|log| {
        log.events.push(SitlEvent::SwarmCommandRouteBlocked {
            step: 6,
            route_id: "route:gcs:agent-0".to_owned(),
            from_node_id: "gcs".to_owned(),
            to_agent_id: "agent-0".to_owned(),
            reason: "centralized_gcs_route_unavailable".to_owned(),
        });
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert!(
        report
            .violations
            .iter()
            .all(|violation| violation.rule_id != RULE_SWARM_TOPOLOGY_BLOCKED_UNREPORTED),
        "{:?}",
        report.violations
    );
}

#[test]
fn swarm_topology_missing_delay_drop_policy_fails_in_strict_mode() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        let artifact = manifest.command_plane_artifact.as_mut().unwrap();
        let topology = artifact.topology.as_mut().unwrap();
        topology.transport.max_delay_ms = None;
        topology.transport.drop_rate = None;
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SWARM_TRANSPORT_ASSUMPTION_MISSING);
}

#[test]
fn swarm_topology_missing_transport_boundary_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_manifest(|manifest| {
        manifest
            .command_plane_artifact
            .as_mut()
            .unwrap()
            .topology
            .as_mut()
            .unwrap()
            .transport
            .hardware_boundary
            .clear();
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SWARM_TRANSPORT_ASSUMPTION_MISSING);
}

#[test]
fn final_status_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_report(|report| {
        report.final_status = "failed".to_owned();
        report.overall_status = "failed".to_owned();
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_FINAL_STATUS_MISMATCH);
}

#[test]
fn completed_task_count_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_report(|report| {
        report.total_completed_tasks = 1;
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_COMPLETED_TASK_MISSING_EVENT);
}

#[test]
fn replay_summary_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fs::write(fixture.output_dir.join("replay-summary.txt"), "wrong\n").unwrap();

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_REPLAY_SUMMARY_COUNT_MISMATCH);
}

#[test]
fn replacement_seq_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_event_log(|log| {
        log.events.push(SitlEvent::MultiAgentMissionItemSent {
            step: 5,
            agent_id: "agent-1".to_owned(),
            seq: 2,
            task_id: Some("wp-0".to_owned()),
        });
        log.events.push(SitlEvent::MultiAgentTaskCompleted {
            step: 6,
            agent_id: "agent-1".to_owned(),
            seq: 0,
            task_id: "wp-0".to_owned(),
        });
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_REPLACEMENT_SEQ_MISMATCH);
}

#[test]
fn missing_safety_report_fails_for_supervisor_run() {
    let fixture = ArtifactFixture::new();
    fs::remove_file(fixture.output_dir.join("safety_validation_report.v1.json")).unwrap();

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::SupervisorRun,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_SAFETY_REPORT_MISSING);
}

#[test]
fn valid_degraded_supervisor_pack_passes() {
    let fixture = ArtifactFixture::new();
    fixture.write_degraded_pack(|_, _| {});

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert!(report.passed, "{:?}", report.violations);
}

#[test]
fn degraded_report_without_degraded_replay_events_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_degraded_pack(|log, _| {
        log.events.retain(|event| {
            !matches!(
                event,
                SitlEvent::SupervisorFailureDetected { .. }
                    | SitlEvent::SupervisorFailureClassified { .. }
                    | SitlEvent::SupervisorRecoveryStarted { .. }
                    | SitlEvent::SupervisorRecoveryCompleted { .. }
            )
        });
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_DEGRADED_EVENT_MISSING);
}

#[test]
fn degraded_final_status_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_degraded_pack(|_, report| {
        report.degraded.records[0].final_status = "unexpected".to_owned();
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_DEGRADED_FINAL_STATUS_MISMATCH);
}

#[test]
fn degraded_recovered_task_mismatch_fails() {
    let fixture = ArtifactFixture::new();
    fixture.write_degraded_pack(|_, report| {
        report.degraded.records[0]
            .tasks_recovered
            .push("missing-recovered-task".to_owned());
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions::default(),
    );

    assert_rule(&report, RULE_DEGRADED_RECOVERY_TASK_MISMATCH);
}

#[test]
fn historical_failed_pack_without_degraded_record_warns_but_passes() {
    let fixture = ArtifactFixture::new();
    fixture.write_event_log(|log| {
        log.events.push(SitlEvent::MultiAgentRunFinished {
            step: 6,
            overall_status: "partial_failed".to_owned(),
        });
    });
    fixture.write_report(|report| {
        report.failed_agents = 1;
        report.final_status = "partial_failed".to_owned();
        report.overall_status = "partial_failed".to_owned();
    });

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&fixture.output_dir),
        ArtifactValidationOptions {
            allow_historical: true,
            ..Default::default()
        },
    );

    assert!(report.passed, "{:?}", report.violations);
    assert_rule(&report, RULE_DEGRADED_RECORD_MISSING);
}

#[test]
fn valid_dry_run_artifact_with_m81_plan_passes() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    write_json(
        &output_dir.join("sitl_dry_run_artifact.v1.json"),
        &dry_run_artifact_fixture(true),
    );

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            strict: true,
            ..Default::default()
        },
    );

    assert!(report.passed, "{:?}", report.violations);
}

#[test]
fn dry_run_artifact_missing_policy_summary_fails_strict_current_validation() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    artifact.command_ir_summary = None;
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DRY_RUN_POLICY_MISSING);
}

#[test]
fn dry_run_artifact_failed_safety_report_fails() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    artifact.safety_report.passed = false;
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DRY_RUN_SAFETY_REPORT_FAILED);
}

#[test]
fn dry_run_artifact_missing_telemetry_milestones_fails_for_mission_items() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    artifact
        .mavlink_common_plan
        .as_mut()
        .unwrap()
        .telemetry_milestones
        .clear();
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_MAVLINK_PLAN_TELEMETRY_MISSING);
}

#[test]
fn dry_run_artifact_missing_m82_compatibility_fails_current_validation() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    artifact.mavlink_common_plan.as_mut().unwrap().compatibility = None;
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_MAVLINK_PROFILE_MISSING);
}

#[test]
fn dry_run_artifact_historical_missing_m82_compatibility_warns_but_passes() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    artifact.mavlink_common_plan.as_mut().unwrap().compatibility = None;
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            allow_historical: true,
            ..Default::default()
        },
    );

    assert!(report.passed, "{:?}", report.violations);
    assert_rule(&report, RULE_MAVLINK_PROFILE_MISSING);
}

#[test]
fn dry_run_artifact_with_unsupported_profile_result_fails() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    let plan = artifact.mavlink_common_plan.as_mut().unwrap();
    plan.mission_items[0].frame = "MAV_FRAME_UNSUPPORTED_TEST".to_owned();
    let report = swarm_comms::classify_mavlink_plan_compatibility(
        plan,
        swarm_comms::MavlinkCapabilityProfileId::MavlinkCommonGeneric.profile(),
    );
    plan.compatibility = Some(report);
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_MAVLINK_PROFILE_UNSUPPORTED);
}

#[test]
fn dual_stack_evidence_pack_passes() {
    let fixture = tempfile::tempdir().unwrap();
    write_dual_stack_evidence_pack(
        public_scenario_path("scenarios/primitive.takeoff-hold-land.json"),
        "agent-0",
        fixture.path(),
        true,
    )
    .unwrap();

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert!(report.passed, "{:?}", report.violations);
}

#[test]
fn dual_stack_evidence_swapped_profile_path_fails() {
    let fixture = dual_stack_fixture_json();
    let path = fixture.path().join(SITL_DUAL_STACK_EVIDENCE_FILE);
    let mut json = read_json_value(&path);
    json["profiles"][0]["dry_run_artifact_path"] =
        serde_json::json!("ardupilot/sitl_dry_run_artifact.v1.json");
    write_json(&path, &json);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_PROFILE_MISMATCH);
}

#[test]
fn dual_stack_evidence_missing_referenced_dry_run_fails() {
    let fixture = dual_stack_fixture_json();
    fs::remove_file(fixture.path().join("px4/sitl_dry_run_artifact.v1.json")).unwrap();

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_PROFILE_MISSING);
}

#[test]
fn dual_stack_evidence_missing_px4_profile_fails() {
    let fixture = dual_stack_fixture_json();
    let path = fixture.path().join(SITL_DUAL_STACK_EVIDENCE_FILE);
    let mut json = read_json_value(&path);
    json["profiles"]
        .as_array_mut()
        .unwrap()
        .retain(|profile| profile["mavlink_profile"] != "px4");
    write_json(&path, &json);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_PROFILE_MISSING);
}

#[test]
fn dual_stack_evidence_missing_ardupilot_profile_fails() {
    let fixture = dual_stack_fixture_json();
    let path = fixture.path().join(SITL_DUAL_STACK_EVIDENCE_FILE);
    let mut json = read_json_value(&path);
    json["profiles"]
        .as_array_mut()
        .unwrap()
        .retain(|profile| profile["mavlink_profile"] != "ardupilot");
    write_json(&path, &json);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_PROFILE_MISSING);
}

#[test]
fn dual_stack_evidence_unsafe_ardupilot_hardware_claim_fails() {
    let fixture = dual_stack_fixture_json();
    let path = fixture.path().join(SITL_DUAL_STACK_EVIDENCE_FILE);
    let mut json = read_json_value(&path);
    json["profiles"][1]["hardware_facing_allowed"] = serde_json::json!(true);
    write_json(&path, &json);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_HARDWARE_CLAIM_UNSAFE);
}

#[test]
fn dual_stack_evidence_replacement_policy_mismatch_fails() {
    let fixture = dual_stack_fixture_json();
    let path = fixture.path().join(SITL_DUAL_STACK_EVIDENCE_FILE);
    let mut json = read_json_value(&path);
    json["profiles"][0]["abort_replacement"]["replacement_policy"] =
        serde_json::json!("manual_evidence_required");
    write_json(&path, &json);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_REPLACEMENT_POLICY_MISMATCH);
}

#[test]
fn dual_stack_evidence_missing_abort_replacement_fails() {
    let fixture = dual_stack_fixture_json();
    let path = fixture.path().join(SITL_DUAL_STACK_EVIDENCE_FILE);
    let mut json = read_json_value(&path);
    json.as_object_mut().unwrap().remove("abort_replacement");
    write_json(&path, &json);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_ABORT_REPLACEMENT_MISSING);
}

#[test]
fn dual_stack_evidence_mismatched_abort_policy_fails() {
    let fixture = dual_stack_fixture_json();
    let path = fixture.path().join(SITL_DUAL_STACK_EVIDENCE_FILE);
    let mut json = read_json_value(&path);
    json["abort_replacement"]["timeout_policy"]["completion_timeout_secs"] =
        serde_json::json!(999.0);
    write_json(&path, &json);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_ABORT_POLICY_MISMATCH);
}

#[test]
fn dual_stack_evidence_missing_fc_contract_section_fails() {
    let fixture = dual_stack_fixture_json();
    let path = fixture.path().join(SITL_DUAL_STACK_EVIDENCE_FILE);
    let mut json = read_json_value(&path);
    json["profiles"][0]
        .as_object_mut()
        .unwrap()
        .remove("fc_safety_contract");
    write_json(&path, &json);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_FC_CONTRACT_MISSING);
}

#[test]
fn dual_stack_evidence_hidden_fc_contract_caveat_fails() {
    let fixture = dual_stack_fixture_json();
    let path = fixture.path().join(SITL_DUAL_STACK_EVIDENCE_FILE);
    let mut json = read_json_value(&path);
    json["profiles"][1]["fc_safety_contract"]["caveats"] = serde_json::json!([]);
    write_json(&path, &json);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_FC_CONTRACT_HIDDEN_CAVEAT);
}

#[test]
fn dual_stack_evidence_mismatched_ir_hash_fails() {
    let fixture = dual_stack_fixture_json();
    let path = fixture.path().join(SITL_DUAL_STACK_EVIDENCE_FILE);
    let mut json = read_json_value(&path);
    json["command_ir_hash"] = serde_json::json!("different");
    write_json(&path, &json);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(fixture.path()),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DualStackEvidence,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_DUAL_STACK_IR_HASH_MISMATCH);
}

#[test]
fn dry_run_artifact_stale_m82_compatibility_frame_fails() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    let plan = artifact.mavlink_common_plan.as_mut().unwrap();
    plan.mission_items[0].frame = "MAV_FRAME_UNSUPPORTED_TEST".to_owned();
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_MAVLINK_PROFILE_RESULT_MISMATCH);
}

#[test]
fn dry_run_artifact_stale_m82_compatibility_command_id_fails() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    let plan = artifact.mavlink_common_plan.as_mut().unwrap();
    plan.mission_items[0].command_id = "stale-goto-0".to_owned();
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_MAVLINK_PROFILE_RESULT_MISMATCH);
}

#[test]
fn dry_run_artifact_duplicate_m82_compatibility_result_same_length_fails() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    let report = artifact
        .mavlink_common_plan
        .as_mut()
        .unwrap()
        .compatibility
        .as_mut()
        .unwrap();
    assert!(
        report.command_results.len() > 1,
        "fixture must expose a same-length duplicate mutation"
    );
    report.command_results[1] = report.command_results[0].clone();
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_MAVLINK_PROFILE_RESULT_MISMATCH);
}

#[test]
fn dry_run_artifact_hardware_allowed_flag_must_reflect_unknown_profile_results() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    let report = artifact
        .mavlink_common_plan
        .as_mut()
        .unwrap()
        .compatibility
        .as_mut()
        .unwrap();
    report.command_results[0].classification =
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware;
    report.hardware_facing_allowed = true;
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_MAVLINK_PROFILE_HARDWARE_BLOCKING);
}

#[test]
fn dry_run_artifact_rejects_post_route_lifecycle_command_in_prelude() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    let plan = artifact.mavlink_common_plan.as_mut().unwrap();
    assert!(!plan.mission_items.is_empty());
    plan.command_prelude.push(MavlinkCommonCommand {
        command_id: "bad-land".to_owned(),
        command: MavlinkCommonCommandName::NavLand,
        phase: MavlinkPlanPhase::CommandPrelude,
        params: [Some(0.0); 7],
    });
    plan.expected_acks.push(MavlinkExpectedAck {
        phase: MavlinkPlanPhase::CommandPrelude,
        kind: MavlinkExpectedAckKind::CommandAck,
        command_id: Some("bad-land".to_owned()),
        command: Some(MavlinkCommonCommandName::NavLand),
        seq: None,
    });
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            strict: true,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_MAVLINK_PLAN_ORDER_UNSAFE);
}

#[test]
fn dry_run_artifact_missing_m81_plan_fails() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    write_json(
        &output_dir.join("sitl_dry_run_artifact.v1.json"),
        &dry_run_artifact_fixture(false),
    );

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_MAVLINK_PLAN_MISSING);
}

#[test]
fn urban_wgs84_dry_run_without_waypoint_geo_fails() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    artifact.mission = "urban-patrol".to_owned();
    artifact.export_kind = "urban_route".to_owned();
    artifact.coordinate_mode = "wgs84_node_geo".to_owned();
    artifact.start_waypoint.as_mut().unwrap().geo = None;
    artifact.end_waypoint.as_mut().unwrap().geo = None;
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_URBAN_WGS84_GEO_MISSING);
}

#[test]
fn urban_search_without_mock_perception_metadata_fails() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = dry_run_artifact_fixture(true);
    artifact.mission = "urban-search".to_owned();
    artifact.export_kind = "urban_route".to_owned();
    artifact.coordinate_mode = "local_with_origin".to_owned();
    artifact.urban_mock_perception = None;
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_URBAN_MOCK_PERCEPTION_MISSING);
}

#[test]
fn urban_wgs84_mavlink_mission_item_mismatch_fails() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("dry-run");
    fs::create_dir_all(&output_dir).unwrap();
    let mut artifact = urban_wgs84_dry_run_artifact_fixture();
    artifact.mavlink_common_plan.as_mut().unwrap().mission_items[0].lat_e7 += 1;
    write_json(&output_dir.join("sitl_dry_run_artifact.v1.json"), &artifact);

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::DryRun,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_URBAN_GEO_ROUTE_METADATA_MISSING);
}

#[test]
fn benchmark_pack_without_urban_ownership_overlap_passes_without_sitl_manifest() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("benchmark-pack");
    write_urban_ownership_pack(
        &output_dir,
        serde_json::json!([
            {
                "edge_id": "road-n0-n1",
                "agent_id": "agent-0",
                "acquired_tick": 0,
                "released_tick": 10,
                "held_ticks": 10
            },
            {
                "edge_id": "road-n0-n1",
                "agent_id": "agent-1",
                "acquired_tick": 10,
                "released_tick": 20,
                "held_ticks": 10
            }
        ]),
    );

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::BenchmarkPack,
            strict: true,
            ..Default::default()
        },
    );

    assert!(report.passed, "{:?}", report.violations);
}

#[test]
fn benchmark_pack_duplicate_urban_segment_owner_fails() {
    let fixture = tempfile::tempdir().unwrap();
    let output_dir = fixture.path().join("benchmark-pack");
    write_urban_ownership_pack(
        &output_dir,
        serde_json::json!([
            {
                "edge_id": "road-n0-n1",
                "agent_id": "agent-0",
                "acquired_tick": 0,
                "released_tick": 10,
                "held_ticks": 10
            },
            {
                "edge_id": "road-n0-n1",
                "agent_id": "agent-1",
                "acquired_tick": 9,
                "released_tick": 20,
                "held_ticks": 11
            }
        ]),
    );

    let report = validate_artifact_pack(
        &ArtifactPackPaths::from_output_dir(&output_dir),
        ArtifactValidationOptions {
            mode: ArtifactValidationMode::BenchmarkPack,
            ..Default::default()
        },
    );

    assert_rule(&report, RULE_URBAN_DECONFLICTION_DUPLICATE_SEGMENT_OWNER);
}

fn assert_rule(
    report: &swarm_examples::artifact_validator::ArtifactValidationReport,
    rule_id: &str,
) {
    assert!(
        report
            .violations
            .iter()
            .any(|violation| violation.rule_id == rule_id),
        "missing {rule_id}; violations={:?}",
        report.violations
    );
}

fn public_scenario_path(path: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn dual_stack_fixture_json() -> tempfile::TempDir {
    let fixture = tempfile::tempdir().unwrap();
    write_dual_stack_evidence_pack(
        public_scenario_path("scenarios/primitive.takeoff-hold-land.json"),
        "agent-0",
        fixture.path(),
        true,
    )
    .unwrap();
    fixture
}

fn read_json_value(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn write_urban_ownership_pack(output_dir: &Path, records: serde_json::Value) {
    let analysis_dir = output_dir.join("urban_analysis");
    fs::create_dir_all(&analysis_dir).unwrap();
    write_json(
        &analysis_dir.join("manifest.json"),
        &serde_json::json!({
            "schema_version": "urban_analysis.v1",
            "separation_threshold_m": 5.0,
            "artifacts": [
                {
                    "replay_log_index": 0,
                    "run_id": "urban-deconflict-test",
                    "scenario_name": "urban_deconflict_test",
                    "route_trace_json": "urban_analysis/000.route-trace.json",
                    "route_trace_csv": "urban_analysis/000.route-trace.csv",
                    "judge_report_json": "urban_analysis/000.judge-report.json",
                    "judge_report_csv": "urban_analysis/000.judge-report.csv",
                    "segment_ownership_json": "urban_analysis/000.segment-ownership.json",
                    "segment_ownership_csv": "urban_analysis/000.segment-ownership.csv",
                    "event_counts": {},
                    "separation_summary": {
                        "threshold_m": 5.0,
                        "separation_violation_count": 0,
                        "route_conflict_count": 0,
                        "conflicts": []
                    }
                }
            ]
        }),
    );
    write_json(
        &analysis_dir.join("000.segment-ownership.json"),
        &serde_json::json!({
            "run_id": "urban-deconflict-test",
            "scenario_name": "urban_deconflict_test",
            "records": records
        }),
    );
}

fn dry_run_artifact_fixture(include_m81_plan: bool) -> SitlDryRunArtifact {
    let origin = GeoOrigin {
        lat_deg: 47.397_742,
        lon_deg: 8.545_594,
        alt_m: 0.0,
    };
    let waypoint = SitlWaypointItem {
        seq: 0,
        task_id: "wp-0".to_owned(),
        x: 10.0,
        y: 20.0,
        z: 5.0,
        geo: None,
        source: "pose_task".to_owned(),
        edge_id: None,
        from_node_id: None,
        to_node_id: None,
        segment_index: None,
        point_index_on_segment: None,
    };
    let ir_plan = MissionCommandPlan {
        schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
        mission_id: MissionId::from("dry-run-test".to_owned()),
        coordinate_frame: CoordinateFrame::LocalEnu,
        altitude_reference: AltitudeReference::RelativeHome,
        timeout_policy: TimeoutPolicy {
            command_timeout_secs: 5.0,
            completion_timeout_secs: 120.0,
            on_timeout: TimeoutAction::Abort,
        },
        expected_terminal_state: TerminalState::Landed,
        completion_tolerance: CompletionTolerance {
            position_m: 1.0,
            altitude_m: 0.5,
        },
        commands: vec![MissionCommandEntry {
            command_id: CommandId::from("goto-0".to_owned()),
            command: MissionCommand::GoTo {
                position: Position::Local(LocalPosition {
                    x_m: waypoint.x,
                    y_m: waypoint.y,
                    z_m: waypoint.z,
                }),
            },
            source_task_id: Some(waypoint.task_id.clone()),
            source_route_id: None,
            source_agent_id: Some("agent-0".to_owned()),
        }],
    };
    let mavlink_common_plan = include_m81_plan.then(|| {
        compile_mavlink_common_plan(
            &ir_plan,
            &MavlinkCommonPlanOptions {
                home_origin: Some(MavlinkCoordinateOrigin {
                    lat_deg: origin.lat_deg,
                    lon_deg: origin.lon_deg,
                    alt_m: origin.alt_m,
                }),
                ..Default::default()
            },
        )
        .unwrap()
    });

    SitlDryRunArtifact {
        schema_version: "sitl_dry_run_artifact.v1".to_owned(),
        source_scenario_path: PathBuf::from("scenario.json"),
        suite_name: "Dry Run".to_owned(),
        scenario_name: "dry_run_0".to_owned(),
        mission: "sitl".to_owned(),
        profile: "test".to_owned(),
        agent_id: "agent-0".to_owned(),
        export_kind: "pose_tasks".to_owned(),
        planner_or_adapter: "test".to_owned(),
        route_length_m: None,
        segment_count: None,
        waypoint_count: 1,
        waypoints: vec![waypoint.clone()],
        start_waypoint: Some(waypoint.clone()),
        end_waypoint: Some(waypoint),
        start_global: Some(SitlGlobalWaypointSummary {
            lat_deg: origin.lat_deg,
            lon_deg: origin.lon_deg,
            relative_alt_m: 5.0,
        }),
        end_global: Some(SitlGlobalWaypointSummary {
            lat_deg: origin.lat_deg,
            lon_deg: origin.lon_deg,
            relative_alt_m: 5.0,
        }),
        altitude_source: "pose.z".to_owned(),
        geo_origin: Some(origin),
        effective_geo_origin: origin,
        coordinate_frame: "local_simulation".to_owned(),
        coordinate_mode: "local_with_origin".to_owned(),
        urban_mission_template: None,
        urban_blocked_route_policy: None,
        urban_mock_perception: None,
        safety_report: SafetyValidationReport::ok(),
        command: vec!["sitl_agent".to_owned(), "--dry-run".to_owned()],
        git_commit: Some("0123456789abcdef".to_owned()),
        command_ir_summary: Some(MissionCommandSummary::from_plan(&ir_plan)),
        mavlink_common_plan,
    }
}

fn urban_wgs84_dry_run_artifact_fixture() -> SitlDryRunArtifact {
    let origin = GeoOrigin {
        lat_deg: 47.397_742,
        lon_deg: 8.545_594,
        alt_m: 0.0,
    };
    let geo = UrbanGeoPoint {
        lat_deg: 47.397_742,
        lon_deg: 8.545_859,
        alt_m: 5.0,
    };
    let waypoint = SitlWaypointItem {
        seq: 0,
        task_id: "urban-route-0-road-n0-n1-1".to_owned(),
        x: 20.0,
        y: 0.0,
        z: 5.0,
        geo: Some(geo),
        source: "urban_route".to_owned(),
        edge_id: Some("road-n0-n1".to_owned()),
        from_node_id: Some("n0".to_owned()),
        to_node_id: Some("n1".to_owned()),
        segment_index: Some(0),
        point_index_on_segment: Some(1),
    };
    let ir_plan = MissionCommandPlan {
        schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
        mission_id: MissionId::from("urban-wgs84-test".to_owned()),
        coordinate_frame: CoordinateFrame::Wgs84,
        altitude_reference: AltitudeReference::RelativeHome,
        timeout_policy: TimeoutPolicy {
            command_timeout_secs: 5.0,
            completion_timeout_secs: 120.0,
            on_timeout: TimeoutAction::Abort,
        },
        expected_terminal_state: TerminalState::Landed,
        completion_tolerance: CompletionTolerance {
            position_m: 1.0,
            altitude_m: 0.5,
        },
        commands: vec![MissionCommandEntry {
            command_id: CommandId::from("follow-route-0".to_owned()),
            command: MissionCommand::FollowRoute {
                route_id: swarm_mission_ir::RouteId::from("urban-route-export:dijkstra".to_owned()),
                waypoints: vec![swarm_mission_ir::MissionWaypoint {
                    position: Position::Geo(GeoPosition {
                        lat_deg: geo.lat_deg,
                        lon_deg: geo.lon_deg,
                        alt_m: geo.alt_m,
                    }),
                    acceptance_radius_m: None,
                }],
            },
            source_task_id: None,
            source_route_id: Some("urban-route-export:dijkstra".to_owned()),
            source_agent_id: Some("agent-0".to_owned()),
        }],
    };
    let mavlink_common_plan = compile_mavlink_common_plan(
        &ir_plan,
        &MavlinkCommonPlanOptions {
            home_origin: Some(MavlinkCoordinateOrigin {
                lat_deg: origin.lat_deg,
                lon_deg: origin.lon_deg,
                alt_m: origin.alt_m,
            }),
            ..Default::default()
        },
    )
    .unwrap();

    SitlDryRunArtifact {
        schema_version: "sitl_dry_run_artifact.v1".to_owned(),
        source_scenario_path: PathBuf::from("scenarios/urban.geo-perimeter.json"),
        suite_name: "Urban Geo".to_owned(),
        scenario_name: "urban_geo_perimeter".to_owned(),
        mission: "urban-patrol".to_owned(),
        profile: "geo-perimeter".to_owned(),
        agent_id: "agent-0".to_owned(),
        export_kind: "urban_route".to_owned(),
        planner_or_adapter: "urban_route_export:dijkstra".to_owned(),
        route_length_m: Some(20.0),
        segment_count: Some(1),
        waypoint_count: 1,
        waypoints: vec![waypoint.clone()],
        start_waypoint: Some(waypoint.clone()),
        end_waypoint: Some(waypoint),
        start_global: Some(SitlGlobalWaypointSummary {
            lat_deg: geo.lat_deg,
            lon_deg: geo.lon_deg,
            relative_alt_m: geo.alt_m,
        }),
        end_global: Some(SitlGlobalWaypointSummary {
            lat_deg: geo.lat_deg,
            lon_deg: geo.lon_deg,
            relative_alt_m: geo.alt_m,
        }),
        altitude_source: "urban_route_export.default_altitude_m".to_owned(),
        geo_origin: Some(origin),
        effective_geo_origin: origin,
        coordinate_frame: "local_simulation".to_owned(),
        coordinate_mode: "wgs84_node_geo".to_owned(),
        urban_mission_template: Some("perimeter_patrol".to_owned()),
        urban_blocked_route_policy: Some("wait".to_owned()),
        urban_mock_perception: None,
        safety_report: SafetyValidationReport::ok(),
        command: vec!["sitl_agent".to_owned(), "--dry-run".to_owned()],
        git_commit: Some("0123456789abcdef".to_owned()),
        command_ir_summary: Some(MissionCommandSummary::from_plan(&ir_plan)),
        mavlink_common_plan: Some(mavlink_common_plan),
    }
}

struct ArtifactFixture {
    _tempdir: tempfile::TempDir,
    output_dir: PathBuf,
}

impl ArtifactFixture {
    fn new() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let output_dir = tempdir.path().join("run-1");
        fs::create_dir_all(&output_dir).unwrap();
        let fixture = Self {
            _tempdir: tempdir,
            output_dir,
        };
        fixture.write_manifest(|_| {});
        fixture.write_event_log(|_| {});
        fixture.write_report(|_| {});
        fixture.write_replay_summary();
        fixture.write_safety_report();
        fs::write(fixture.output_dir.join("scenario.snapshot.json"), "{}\n").unwrap();
        fs::write(fixture.output_dir.join("config.snapshot.json"), "{}\n").unwrap();
        fs::write(
            fixture.output_dir.join("command.txt"),
            "sitl_supervisor --mock\n",
        )
        .unwrap();
        fixture
    }

    fn write_manifest(&self, mutate: impl FnOnce(&mut MultiAgentSitlManifest)) {
        let command_plane_artifact = test_command_plane_artifact();
        let mut manifest = MultiAgentSitlManifest {
            schema_version: MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION.to_owned(),
            scenario_path: PathBuf::from("scenarios/sitl.multi-agent.json"),
            scenario_name: "sitl_multi_agent".to_owned(),
            mission: "sitl".to_owned(),
            profile: "multi-agent".to_owned(),
            agents_count: 2,
            agents: vec![
                manifest_agent("agent-0", 1, &["wp-0"]),
                manifest_agent("agent-1", 2, &["wp-1"]),
            ],
            ownership_summary: TaskOwnershipSummary {
                total_pose_tasks: 2,
                assigned_task_count: 2,
                unassigned_pose_tasks: Vec::new(),
                duplicate_task_ids: Vec::new(),
            },
            command_plane: Some(command_plane_artifact.summary.clone()),
            topology: command_plane_artifact.topology.as_ref().map(|topology| {
                MultiAgentSitlTopologySummary {
                    kind: topology.kind.clone(),
                    node_count: topology.nodes.len(),
                    link_count: topology.links.len(),
                    route_count: command_plane_artifact.command_routes.len(),
                    degraded_route_count: command_plane_artifact
                        .command_routes
                        .iter()
                        .filter(|route| route.degraded)
                        .count(),
                }
            }),
            command_plane_artifact: Some(command_plane_artifact),
            artifact_metadata: SitlArtifactMetadata {
                command: vec!["sitl_supervisor".to_owned(), "--mock".to_owned()],
                git_commit: Some("0123456789abcdef".to_owned()),
                build_profile: "debug".to_owned(),
                run_id: Some("run-1".to_owned()),
                scenario_snapshot_path: Some(PathBuf::from("scenario.snapshot.json")),
                config_snapshot_path: Some(PathBuf::from("config.snapshot.json")),
                command_path: Some(PathBuf::from("command.txt")),
            },
        };
        mutate(&mut manifest);
        write_json(&self.output_dir.join("manifest.json"), &manifest);
    }

    fn write_event_log(&self, mutate: impl FnOnce(&mut SitlEventLog)) {
        let mut log = base_event_log();
        mutate(&mut log);
        write_json(&self.output_dir.join("events.sitl-log.json"), &log);
        let summary = summarize_sitl_event_log(&log);
        let mut report = base_report(summary);
        report.total_completed_tasks = log
            .events
            .iter()
            .filter(|event| matches!(event, SitlEvent::MultiAgentTaskCompleted { .. }))
            .count();
        write_json(&self.output_dir.join("run-report.json"), &report);
        fs::write(
            self.output_dir.join("replay-summary.txt"),
            format!("{}\n", format_sitl_summary(&summarize_sitl_event_log(&log))),
        )
        .unwrap();
    }

    fn write_report(&self, mutate: impl FnOnce(&mut SitlMultiAgentRunReport)) {
        let log = read_fixture_log(&self.output_dir);
        let mut report = base_report(summarize_sitl_event_log(&log));
        mutate(&mut report);
        write_json(&self.output_dir.join("run-report.json"), &report);
    }

    fn write_replay_summary(&self) {
        let log = read_fixture_log(&self.output_dir);
        fs::write(
            self.output_dir.join("replay-summary.txt"),
            format!("{}\n", format_sitl_summary(&summarize_sitl_event_log(&log))),
        )
        .unwrap();
    }

    fn write_safety_report(&self) {
        write_json(
            &self.output_dir.join("safety_validation_report.v1.json"),
            &SafetyValidationReport::ok(),
        );
    }

    fn write_degraded_pack(
        &self,
        mutate: impl FnOnce(&mut SitlEventLog, &mut SitlMultiAgentRunReport),
    ) {
        let mut log = base_degraded_event_log();
        let summary = summarize_sitl_event_log(&log);
        let mut report = base_degraded_report(summary);
        mutate(&mut log, &mut report);
        report.events_summary = summarize_sitl_event_log(&log);
        write_json(&self.output_dir.join("events.sitl-log.json"), &log);
        write_json(&self.output_dir.join("run-report.json"), &report);
        fs::write(
            self.output_dir.join("replay-summary.txt"),
            format!("{}\n", format_sitl_summary(&summarize_sitl_event_log(&log))),
        )
        .unwrap();
    }
}

fn manifest_agent(agent_id: &str, system_id: u8, task_ids: &[&str]) -> MultiAgentSitlManifestAgent {
    MultiAgentSitlManifestAgent {
        agent_id: agent_id.to_owned(),
        system_id,
        component_id: 1,
        connection_string: format!("udpin:127.0.0.1:1455{system_id}"),
        start_delay_ms: 0,
        lifecycle: MultiAgentLifecycle::Execute,
        task_ids: task_ids
            .iter()
            .map(|task_id| (*task_id).to_owned())
            .collect(),
        command_role: None,
        abort_policy: None,
        waypoint_count: task_ids.len(),
        waypoints: Vec::new(),
        standalone_command: Vec::new(),
    }
}

fn test_command_plane_artifact() -> SwarmCommandPlan {
    let assignments = vec![
        command_plane_assignment("agent-0", "wp-0"),
        command_plane_assignment("agent-1", "wp-1"),
    ];
    let ownership = vec![
        command_plane_ownership("agent-0", "wp-0"),
        command_plane_ownership("agent-1", "wp-1"),
    ];
    build_swarm_command_plan(SwarmCommandFanoutInput {
        plan_id: "sitl_multi_agent:sitl:multi-agent".to_owned(),
        assignments,
        ownership,
        global_abort_policy: SwarmAbortPolicy::AbortMission,
        sync_operations: Vec::new(),
        topology: None,
        mavlink_options: MavlinkCommonPlanOptions::default(),
    })
    .unwrap()
}

fn mark_agent_0_route_blocked(manifest: &mut MultiAgentSitlManifest) {
    let artifact = manifest.command_plane_artifact.as_mut().unwrap();
    let topology = artifact.topology.as_mut().unwrap();
    topology
        .links
        .retain(|link| !(link.from_node_id == "gcs" && link.to_node_id == "agent:agent-0"));
    let route = artifact
        .command_routes
        .iter_mut()
        .find(|route| route.route_id == "route:gcs:agent-0")
        .unwrap();
    route.allowed = false;
    route.degraded = true;
    route.via_node_ids.clear();
    route.reason = "centralized_gcs_route_unavailable".to_owned();

    artifact.summary.topology_link_count = topology.links.len();
    artifact.summary.degraded_route_count = artifact
        .command_routes
        .iter()
        .filter(|route| route.degraded)
        .count();
    if let Some(summary) = manifest.command_plane.as_mut() {
        *summary = artifact.summary.clone();
    }
    if let Some(summary) = manifest.topology.as_mut() {
        summary.link_count = topology.links.len();
        summary.degraded_route_count = artifact.summary.degraded_route_count;
    }
}

fn command_plane_assignment(agent_id: &str, task_id: &str) -> AgentCommandAssignment {
    AgentCommandAssignment {
        agent_id: swarm_types::AgentId::from(agent_id.to_owned()),
        role: SwarmCommandRole::Scout,
        command_plan: command_plane_command_plan(agent_id, task_id),
        abort_policy: SwarmAbortPolicy::AbortMission,
        ownership_refs: vec![SwarmOwnershipRef {
            kind: SwarmOwnershipKind::Task,
            resource_id: task_id.to_owned(),
        }],
    }
}

fn command_plane_command_plan(agent_id: &str, task_id: &str) -> MissionCommandPlan {
    MissionCommandPlan {
        schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
        mission_id: MissionId::from(format!("test:{agent_id}")),
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
            command_id: CommandId::from(format!("{agent_id}:{task_id}:arm")),
            command: MissionCommand::Arm,
            source_task_id: Some(task_id.to_owned()),
            source_route_id: None,
            source_agent_id: Some(agent_id.to_owned()),
        }],
    }
}

fn command_plane_ownership(agent_id: &str, task_id: &str) -> SwarmOwnershipRecord {
    SwarmOwnershipRecord {
        agent_id: swarm_types::AgentId::from(agent_id.to_owned()),
        kind: SwarmOwnershipKind::Task,
        resource_id: task_id.to_owned(),
        status: SwarmOwnershipStatus::Active,
        tick: 0,
        reason: "test_assignment".to_owned(),
    }
}

fn base_event_log() -> SitlEventLog {
    SitlEventLog {
        schema_version: "sitl_event_log.v1".to_owned(),
        run_id: "run-1".to_owned(),
        scenario_path: PathBuf::from("scenarios/sitl.multi-agent.json"),
        scenario_name: "sitl_multi_agent".to_owned(),
        mission: "sitl".to_owned(),
        profile: "multi-agent".to_owned(),
        agent_id: "supervisor".to_owned(),
        connection_string: None,
        mode: SitlEventLogMode::ConnectionExecute,
        events: vec![
            SitlEvent::MultiAgentRunStarted {
                step: 0,
                agent_count: 2,
                scenario: "sitl_multi_agent".to_owned(),
            },
            SitlEvent::MultiAgentMissionItemSent {
                step: 1,
                agent_id: "agent-0".to_owned(),
                seq: 0,
                task_id: Some("wp-0".to_owned()),
            },
            SitlEvent::MultiAgentTaskCompleted {
                step: 2,
                agent_id: "agent-0".to_owned(),
                seq: 0,
                task_id: "wp-0".to_owned(),
            },
            SitlEvent::MultiAgentMissionItemSent {
                step: 3,
                agent_id: "agent-1".to_owned(),
                seq: 0,
                task_id: Some("wp-1".to_owned()),
            },
            SitlEvent::MultiAgentTaskCompleted {
                step: 4,
                agent_id: "agent-1".to_owned(),
                seq: 0,
                task_id: "wp-1".to_owned(),
            },
            SitlEvent::MultiAgentRunFinished {
                step: 5,
                overall_status: "completed".to_owned(),
            },
        ],
    }
}

fn base_degraded_event_log() -> SitlEventLog {
    let mut log = base_event_log();
    log.events = vec![
        SitlEvent::MultiAgentRunStarted {
            step: 0,
            agent_count: 2,
            scenario: "sitl_multi_agent".to_owned(),
        },
        SitlEvent::MultiAgentMissionItemSent {
            step: 1,
            agent_id: "agent-0".to_owned(),
            seq: 0,
            task_id: Some("wp-0".to_owned()),
        },
        SitlEvent::MultiAgentMissionItemSent {
            step: 2,
            agent_id: "agent-1".to_owned(),
            seq: 0,
            task_id: Some("wp-1".to_owned()),
        },
        SitlEvent::SupervisorFailureDetected {
            step: 3,
            agent_id: "agent-0".to_owned(),
            mode: "heartbeat_lost".to_owned(),
            completed_task_ids: Vec::new(),
        },
        SitlEvent::SupervisorFailureClassified {
            step: 4,
            agent_id: "agent-0".to_owned(),
            mode: "heartbeat_lost".to_owned(),
            decision: "continue_with_survivor".to_owned(),
        },
        SitlEvent::SupervisorRecoveryStarted {
            step: 5,
            agent_id: "agent-1".to_owned(),
            policy: "mission_replacement".to_owned(),
            task_ids: vec!["wp-1".to_owned(), "wp-0".to_owned()],
        },
        SitlEvent::SupervisorRecoveryCompleted {
            step: 6,
            agent_id: "agent-1".to_owned(),
            recovered_task_ids: vec!["wp-0".to_owned()],
            latency_ticks: Some(0),
        },
        SitlEvent::MultiAgentMissionItemSent {
            step: 7,
            agent_id: "agent-1".to_owned(),
            seq: 0,
            task_id: Some("wp-1".to_owned()),
        },
        SitlEvent::MultiAgentMissionItemSent {
            step: 8,
            agent_id: "agent-1".to_owned(),
            seq: 1,
            task_id: Some("wp-0".to_owned()),
        },
        SitlEvent::MultiAgentTaskCompleted {
            step: 9,
            agent_id: "agent-1".to_owned(),
            seq: 0,
            task_id: "wp-1".to_owned(),
        },
        SitlEvent::MultiAgentTaskCompleted {
            step: 10,
            agent_id: "agent-1".to_owned(),
            seq: 1,
            task_id: "wp-0".to_owned(),
        },
        SitlEvent::SupervisorFinalStatus {
            step: 11,
            status: "completed_with_reallocation".to_owned(),
            degraded: true,
        },
        SitlEvent::MultiAgentRunFinished {
            step: 12,
            overall_status: "completed_with_reallocation".to_owned(),
        },
    ];
    log
}

fn base_report(
    summary: swarm_examples::sitl_observability::SitlEventLogSummary,
) -> SitlMultiAgentRunReport {
    SitlMultiAgentRunReport {
        schema_version: "sitl_multi_agent_run_report.v1".to_owned(),
        run_id: "run-1".to_owned(),
        scenario_path: PathBuf::from("scenarios/sitl.multi-agent.json"),
        scenario_name: "sitl_multi_agent".to_owned(),
        config_path: PathBuf::from("scenarios/sitl.multi-agent.config.json"),
        mission: "sitl".to_owned(),
        profile: "multi-agent".to_owned(),
        mode: "connection_execute".to_owned(),
        agents: vec![agent_report("agent-0", 1), agent_report("agent-1", 2)],
        total_completed_tasks: 2,
        failed_agents: 0,
        aborted_agents: 0,
        overall_status: "completed".to_owned(),
        event_log_path: Some(PathBuf::from("events.sitl-log.json")),
        task_ownership: TaskOwnershipSummary {
            total_pose_tasks: 2,
            assigned_task_count: 2,
            unassigned_pose_tasks: Vec::new(),
            duplicate_task_ids: Vec::new(),
        },
        command_plane: None,
        command_plane_artifact: None,
        events_summary: summary,
        final_status: "completed".to_owned(),
        reallocation: SitlMultiAgentReallocationReport::default(),
        degraded: swarm_examples::sitl_supervisor::SitlDegradedRunReport::default(),
        limitations: vec!["local PX4/SIH only".to_owned()],
        known_limitations: vec!["local PX4/SIH only".to_owned()],
    }
}

fn base_degraded_report(
    summary: swarm_examples::sitl_observability::SitlEventLogSummary,
) -> SitlMultiAgentRunReport {
    let mut report = base_report(summary);
    report.failed_agents = 1;
    report.overall_status = "completed_with_reallocation".to_owned();
    report.final_status = "completed_with_reallocation".to_owned();
    report.total_completed_tasks = 2;
    report.reallocation.lost_agent_count = 1;
    report.reallocation.released_tasks = vec!["wp-0".to_owned()];
    report.reallocation.reassigned_tasks = vec!["wp-0".to_owned()];
    report.reallocation.reassignment_count = 1;
    report.reallocation.tasks_recovered = vec!["wp-0".to_owned()];
    report.degraded = SitlDegradedRunReport {
        records: vec![DegradedRunRecord {
            failure_mode: SupervisorFailureMode::HeartbeatLost,
            decision: SupervisorDecision::ContinueWithSurvivor,
            detected_tick: Some(3),
            detected_after_ms: None,
            affected_agent_id: "agent-0".to_owned(),
            tasks_completed_before_failure: Vec::new(),
            tasks_recovered: vec!["wp-0".to_owned()],
            tasks_abandoned: Vec::new(),
            replacement_mission_id: Some("replacement:agent-0:agent-1".to_owned()),
            recovery_latency_ticks: Some(0),
            final_status: "completed_with_reallocation".to_owned(),
        }],
        failure_mode_counts: [("heartbeat_lost".to_owned(), 1)].into(),
        decision_counts: [("continue_with_survivor".to_owned(), 1)].into(),
        tasks_abandoned: Vec::new(),
        recovery_failed_count: 0,
    };
    report
}

fn agent_report(agent_id: &str, system_id: u8) -> SitlMultiAgentAgentReport {
    SitlMultiAgentAgentReport {
        agent_id: agent_id.to_owned(),
        connection_string: format!("udpin:127.0.0.1:1455{system_id}"),
        system_id,
        component_id: 1,
        lifecycle: "execute".to_owned(),
        mission_item_count: 1,
        completed_task_count: 1,
        final_status: "completed".to_owned(),
        error: None,
        failure_mode: None,
        tasks_abandoned: Vec::new(),
    }
}

fn read_fixture_log(output_dir: &Path) -> SitlEventLog {
    let text = fs::read_to_string(output_dir.join("events.sitl-log.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    let json = serde_json::to_string_pretty(value).unwrap();
    fs::write(path, json).unwrap();
}
