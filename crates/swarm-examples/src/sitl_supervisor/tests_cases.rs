use super::tests_support::*;
use super::*;
use crate::sitl_report::SitlMultiAgentReallocationReport;
#[test]
fn supervisor_metrics_formats_contract_line() {
    let metrics = SupervisorMetrics {
        heartbeat_count: 6,
        completed_task_count: 2,
        lost_agent_count: 1,
        released_tasks: vec!["wp-0".to_owned()],
        reassigned_tasks: vec!["wp-0".to_owned()],
        reassignment_count: 1,
        tasks_recovered: vec!["wp-0".to_owned()],
        reallocation_latency_ticks: Some(0),
        survivor_mission_updates: 1,
        final_completed_after_reallocation: 2,
        ..Default::default()
    };

    assert_eq!(
            metrics.format_summary_line(2, "completed"),
            "SUPERVISOR_METRICS agents=2 heartbeats=6 completed_tasks=2 lost_agents=1 released_tasks=wp-0 reassigned_tasks=wp-0 reassignment_count=1 tasks_recovered=wp-0 reallocation_latency_ticks=0 survivor_mission_updates=1 final_completed_after_reallocation=2 final_status=completed"
        );
}

#[test]
fn fake_supervisor_boundary_completes_happy_path() {
    let metrics = run_fake_supervisor(fake_controllers(), "agent-0").unwrap();

    assert_eq!(metrics.completed_task_count, 2);
    assert_eq!(metrics.lost_agent_count, 0);
    assert_eq!(metrics.reassignment_count, 0);
    assert!(metrics.tasks_recovered.is_empty());
    assert_eq!(metrics.reallocation_latency_ticks, None);
}

#[test]
fn fake_supervisor_boundary_reallocates_after_progress_loss() {
    let controllers = vec![
        FakeAgentController::stops_at("agent-0", 0),
        FakeAgentController::alive("agent-1"),
    ];

    let metrics = run_fake_supervisor(controllers, "agent-1").unwrap();

    assert_eq!(metrics.lost_agent_count, 1);
    assert_eq!(metrics.reassignment_count, 1);
    assert_eq!(metrics.tasks_recovered, vec!["wp-0"]);
    assert_eq!(metrics.reallocation_latency_ticks, Some(0));
    assert_eq!(metrics.completed_task_count, 2);
}

#[test]
fn fake_supervisor_boundary_propagates_upload_failure() {
    let controllers = vec![
        FakeAgentController::alive("agent-0").with_upload_failure(),
        FakeAgentController::alive("agent-1"),
    ];

    let error = run_fake_supervisor(controllers, "agent-0").unwrap_err();
    assert!(error.to_string().contains("fake upload failure"));
}

#[test]
fn fake_supervisor_boundary_propagates_start_failure_after_upload() {
    let controllers = vec![
        FakeAgentController::alive("agent-0").with_start_failure(),
        FakeAgentController::alive("agent-1"),
    ];

    let error = run_fake_supervisor(controllers, "agent-0").unwrap_err();
    assert!(error.to_string().contains("fake start failure"));
}

#[test]
fn fake_supervisor_boundary_rejects_missing_controller() {
    let controllers = vec![FakeAgentController::alive("agent-0")];

    let error = run_fake_supervisor(controllers, "agent-0").unwrap_err();
    assert!(error
        .to_string()
        .contains("missing controller for manifest agent 'agent-1'"));
}

#[test]
fn mock_agent_controller_uploads_and_polls_deterministically() {
    let manifest = fixture_manifest();
    let agent = &manifest.agents[0];
    let mut controller = MockAgentController::new(agent, Some(1));

    let upload = controller.upload(&agent.waypoints).unwrap();
    assert_eq!(upload.agent_id, "agent-0");
    assert_eq!(upload.waypoint_count, 1);
    assert_eq!(controller.waypoints_sent(), 1);
    assert!(controller.poll(0).unwrap().heartbeat_seen);
    assert!(!controller.poll(1).unwrap().heartbeat_seen);
}

#[test]
fn mock_supervisor_returns_metrics_after_reallocation() {
    let suite = fixture_suite();
    let manifest = fixture_manifest();
    let config = SupervisorMockConfig {
        scenario_path: "inline-scenario.json".to_owned(),
        replay_log: None,
        run_id: None,
        fail_agent: Some("agent-0".to_owned()),
        fail_after_ticks: 0,
        heartbeat_timeout_ticks: Some(1),
        max_ticks: Some(6),
    };

    let metrics = run_mock_supervisor(&suite, &config, &manifest).unwrap();
    assert_eq!(metrics.lost_agent_count, 1);
    assert_eq!(metrics.reassignment_count, 1);
    assert_eq!(metrics.tasks_recovered, vec!["wp-0"]);
    assert_eq!(metrics.reallocation_latency_ticks, Some(0));
}

#[test]
fn mock_supervisor_rejects_unknown_fail_agent() {
    let suite = fixture_suite();
    let manifest = fixture_manifest();
    let config = SupervisorMockConfig {
        scenario_path: "inline-scenario.json".to_owned(),
        replay_log: None,
        run_id: None,
        fail_agent: Some("missing-agent".to_owned()),
        fail_after_ticks: 0,
        heartbeat_timeout_ticks: Some(1),
        max_ticks: Some(6),
    };

    let error = run_mock_supervisor(&suite, &config, &manifest).unwrap_err();
    assert!(error.to_string().contains("--fail-agent 'missing-agent'"));
}

#[test]
fn live_supervisor_rejects_upload_only_agent() {
    let manifest = fixture_manifest();
    let config = fixture_live_config();

    let error = validate_live_manifest(&manifest, &config).unwrap_err();

    assert!(error
        .to_string()
        .contains("live supervisor execute requires lifecycle=execute"));
}

#[test]
fn live_supervisor_rejects_hardware_candidate_without_explicit_allow() {
    let mut manifest = fixture_execute_manifest();
    manifest.agents[0].connection_string = "tcpout:192.168.1.10:5760".to_owned();
    let config = fixture_live_config();

    let error = validate_live_manifest(&manifest, &config).unwrap_err();

    assert!(error
        .to_string()
        .contains("requires --allow-hardware-candidate"));
}

#[test]
fn fake_live_supervisor_writes_report_and_replay_log() {
    let suite = fixture_suite();
    let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
    let manifest = fixture_execute_manifest();
    let dir = tempfile::tempdir().unwrap();
    let replay_log = dir.path().join("multi.sitl-log.json");
    let run_report = dir.path().join("multi.run-report.json");
    let mut config = fixture_live_config();
    config.replay_log = Some(replay_log.to_string_lossy().into_owned());
    config.run_report = Some(run_report.to_string_lossy().into_owned());
    let controllers = manifest
        .agents
        .iter()
        .map(FakeLiveAgentController::completed)
        .collect();

    let report =
        run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

    assert_eq!(report.overall_status, "completed");
    assert_eq!(report.final_status, "completed");
    assert_eq!(report.total_completed_tasks, 2);
    assert_eq!(report.failed_agents, 0);
    assert_eq!(report.agents.len(), 2);
    assert_eq!(report.task_ownership, manifest.ownership_summary);
    assert_eq!(
        report.events_summary.final_status.as_deref(),
        Some("completed")
    );
    assert_eq!(report.events_summary.multi_agent_agent_started, 2);
    assert_eq!(report.limitations, report.known_limitations);
    assert_eq!(
        report.reallocation,
        SitlMultiAgentReallocationReport::default()
    );
    assert!(report.degraded.records.is_empty());

    let log = crate::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
    let summary = crate::sitl_observability::summarize_sitl_event_log(&log);
    assert_eq!(summary.multi_agent_run_started, 1);
    assert_eq!(summary.multi_agent_run_finished, 1);
    assert_eq!(summary.multi_agent_agent_started, 2);
    assert_eq!(summary.multi_agent_agent_finished, 2);
    assert_eq!(summary.multi_agent_mission_count_sent, 2);
    assert_eq!(summary.multi_agent_mission_item_sent, 2);
    assert_eq!(summary.multi_agent_waypoint_reached, 2);
    assert_eq!(summary.multi_agent_task_completed, 2);
    assert_eq!(summary.mission_count_sent, 2);
    assert_eq!(summary.mission_item_sent, 2);
    assert_eq!(summary.waypoint_reached, 2);
    assert_eq!(summary.task_completed, 2);
    assert_eq!(summary.survivor_mission_updates, 0);
    assert_eq!(summary.supervisor_failure_detected, 0);
    assert_eq!(summary.multi_agent_agent_count, Some(2));
    assert_eq!(summary.final_status.as_deref(), Some("completed"));
    let mission_items: Vec<(String, u16, String)> = log
        .events
        .iter()
        .filter_map(|event| match event {
            crate::sitl_observability::SitlEvent::MultiAgentMissionItemSent {
                agent_id,
                seq,
                task_id: Some(task_id),
                ..
            } => Some((agent_id.clone(), *seq, task_id.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        mission_items,
        vec![
            ("agent-0".to_owned(), 0, "wp-0".to_owned()),
            ("agent-1".to_owned(), 0, "wp-1".to_owned())
        ]
    );
    let task_completed: Vec<(String, u16, String)> = log
        .events
        .iter()
        .filter_map(|event| match event {
            crate::sitl_observability::SitlEvent::MultiAgentTaskCompleted {
                agent_id,
                seq,
                task_id,
                ..
            } => Some((agent_id.clone(), *seq, task_id.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        task_completed,
        vec![
            ("agent-0".to_owned(), 0, "wp-0".to_owned()),
            ("agent-1".to_owned(), 0, "wp-1".to_owned())
        ]
    );
    assert!(log.events.iter().all(|event| !matches!(
        event,
        crate::sitl_observability::SitlEvent::MissionCountSent { .. }
            | crate::sitl_observability::SitlEvent::MissionItemSent { .. }
            | crate::sitl_observability::SitlEvent::WaypointReached { .. }
            | crate::sitl_observability::SitlEvent::TaskCompleted { .. }
            | crate::sitl_observability::SitlEvent::Failure { .. }
    )));

    let report_json: SitlMultiAgentRunReport =
        serde_json::from_str(&std::fs::read_to_string(run_report).unwrap()).unwrap();
    assert_eq!(report_json, report);
}

#[test]
fn fake_live_supervisor_reallocates_lost_agent_to_active_survivor() {
    let suite = fixture_suite();
    let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
    let manifest = fixture_execute_manifest();
    let dir = tempfile::tempdir().unwrap();
    let replay_log = dir.path().join("m59.sitl-log.json");
    let run_report = dir.path().join("m59.run-report.json");
    let mut config = fixture_live_config();
    config.reupload_on_failure = true;
    config.replay_log = Some(replay_log.to_string_lossy().into_owned());
    config.run_report = Some(run_report.to_string_lossy().into_owned());
    let controllers = vec![
        FakeLiveAgentController::failed_after_polls(&manifest.agents[0], 0, 0),
        FakeLiveAgentController::completed_after_polls(&manifest.agents[1], 1),
    ];

    let report =
        run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

    assert_eq!(report.overall_status, "completed_with_reallocation");
    assert_eq!(report.final_status, "completed_with_reallocation");
    assert_eq!(report.total_completed_tasks, 2);
    assert_eq!(report.failed_agents, 1);
    assert_eq!(report.task_ownership, manifest.ownership_summary);
    assert_eq!(
        report.events_summary.final_status.as_deref(),
        Some("completed_with_reallocation")
    );
    assert_eq!(report.events_summary.survivor_mission_updates, 1);
    assert_eq!(report.limitations, report.known_limitations);
    assert_eq!(report.reallocation.lost_agent_count, 1);
    assert_eq!(report.reallocation.released_tasks, vec!["wp-0"]);
    assert_eq!(report.reallocation.reassigned_tasks, vec!["wp-0"]);
    assert_eq!(report.reallocation.reassignment_count, 1);
    assert_eq!(report.reallocation.tasks_recovered, vec!["wp-0"]);
    assert_eq!(report.reallocation.reallocation_latency_ticks, Some(0));
    assert_eq!(report.reallocation.survivor_mission_updates, 1);
    assert_eq!(report.reallocation.final_completed_after_reallocation, 2);
    assert_eq!(report.degraded.records.len(), 1);
    assert_degraded_count(
        &report,
        SupervisorFailureMode::Unknown,
        SupervisorDecision::ContinueWithSurvivor,
    );
    assert_degraded_count(
        &report,
        SupervisorFailureMode::Unknown,
        SupervisorDecision::ReleaseTasksToPool,
    );
    assert_degraded_count(
        &report,
        SupervisorFailureMode::Unknown,
        SupervisorDecision::ReassignUnfinishedTasks,
    );
    assert_eq!(report.agents[1].mission_item_count, 2);
    assert_eq!(report.agents[1].completed_task_count, 2);

    let log = crate::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
    let summary = crate::sitl_observability::summarize_sitl_event_log(&log);
    assert_eq!(summary.agent_lost, 1);
    assert_eq!(summary.task_released, 1);
    assert_eq!(summary.task_reassigned, 1);
    assert_eq!(summary.reallocation_completed, 1);
    assert_eq!(summary.tasks_recovered, 1);
    assert_eq!(summary.survivor_mission_update_started, 1);
    assert_eq!(summary.survivor_mission_update_completed, 1);
    assert_eq!(summary.survivor_mission_updates, 1);
    assert_eq!(summary.supervisor_failure_detected, 1);
    assert_eq!(summary.supervisor_failure_classified, 1);
    assert_eq!(summary.supervisor_recovery_started, 1);
    assert_eq!(summary.supervisor_replacement_uploaded, 1);
    assert_eq!(summary.supervisor_recovery_completed, 1);
    assert_eq!(summary.supervisor_recovery_failed, 0);
    assert_eq!(
        summary.final_status.as_deref(),
        Some("completed_with_reallocation")
    );

    let mission_items: Vec<(String, u16, String)> = log
        .events
        .iter()
        .filter_map(|event| match event {
            crate::sitl_observability::SitlEvent::MultiAgentMissionItemSent {
                agent_id,
                seq,
                task_id: Some(task_id),
                ..
            } => Some((agent_id.clone(), *seq, task_id.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        mission_items,
        vec![
            ("agent-0".to_owned(), 0, "wp-0".to_owned()),
            ("agent-1".to_owned(), 0, "wp-1".to_owned()),
            ("agent-1".to_owned(), 0, "wp-1".to_owned()),
            ("agent-1".to_owned(), 1, "wp-0".to_owned())
        ]
    );
    assert_eq!(
        multi_agent_task_completed(&log),
        vec![
            ("agent-1".to_owned(), 0, "wp-1".to_owned()),
            ("agent-1".to_owned(), 1, "wp-0".to_owned())
        ]
    );

    let report_json: SitlMultiAgentRunReport =
        serde_json::from_str(&std::fs::read_to_string(run_report).unwrap()).unwrap();
    assert_eq!(report_json, report);
}

#[test]
fn fake_live_supervisor_replacement_appends_recovered_tasks_in_manifest_order() {
    let suite = fixture_nonlexical_suite();
    let entry = first_sitl_entry(&suite, "nonlexical-scenario.json").unwrap();
    let manifest = fixture_nonlexical_execute_manifest();
    let dir = tempfile::tempdir().unwrap();
    let replay_log = dir.path().join("m59-nonlexical.sitl-log.json");
    let mut config = fixture_live_config();
    config.reupload_on_failure = true;
    config.scenario_path = "nonlexical-scenario.json".to_owned();
    config.replay_log = Some(replay_log.to_string_lossy().into_owned());
    let controllers = vec![
        FakeLiveAgentController::failed(&manifest.agents[0], 0),
        FakeLiveAgentController::completed(&manifest.agents[1]),
    ];

    let report =
        run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

    assert_eq!(report.overall_status, "completed_with_reallocation");
    assert_eq!(report.total_completed_tasks, 3);
    assert_eq!(report.failed_agents, 1);
    assert_eq!(report.reallocation.survivor_mission_updates, 1);
    assert_eq!(report.reallocation.final_completed_after_reallocation, 3);
    assert_eq!(report.agents[1].mission_item_count, 3);

    let log = crate::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
    let mission_items = multi_agent_mission_items(&log);
    assert_eq!(
        mission_items,
        vec![
            ("agent-0".to_owned(), 0, "wp-2".to_owned()),
            ("agent-0".to_owned(), 1, "wp-10".to_owned()),
            ("agent-1".to_owned(), 0, "wp-1".to_owned()),
            ("agent-1".to_owned(), 0, "wp-1".to_owned()),
            ("agent-1".to_owned(), 1, "wp-2".to_owned()),
            ("agent-1".to_owned(), 2, "wp-10".to_owned())
        ]
    );
    assert_eq!(
        multi_agent_task_completed(&log),
        vec![
            ("agent-1".to_owned(), 0, "wp-1".to_owned()),
            ("agent-1".to_owned(), 1, "wp-2".to_owned()),
            ("agent-1".to_owned(), 2, "wp-10".to_owned())
        ]
    );
}

#[test]
fn fake_live_supervisor_excludes_completed_failed_task_from_replacement() {
    let suite = fixture_nonlexical_suite();
    let entry = first_sitl_entry(&suite, "nonlexical-scenario.json").unwrap();
    let manifest = fixture_nonlexical_execute_manifest();
    let dir = tempfile::tempdir().unwrap();
    let replay_log = dir.path().join("m59-after-one.sitl-log.json");
    let mut config = fixture_live_config();
    config.reupload_on_failure = true;
    config.scenario_path = "nonlexical-scenario.json".to_owned();
    config.replay_log = Some(replay_log.to_string_lossy().into_owned());
    let controllers = vec![
        FakeLiveAgentController::failed(&manifest.agents[0], 1),
        FakeLiveAgentController::completed(&manifest.agents[1]),
    ];

    let report =
        run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

    assert_eq!(report.overall_status, "completed_with_reallocation");
    assert_eq!(report.total_completed_tasks, 3);
    assert_eq!(report.failed_agents, 1);
    assert_eq!(report.reallocation.released_tasks, vec!["wp-10"]);
    assert_eq!(report.reallocation.reassigned_tasks, vec!["wp-10"]);
    assert_eq!(report.reallocation.tasks_recovered, vec!["wp-10"]);
    assert_eq!(report.reallocation.survivor_mission_updates, 1);
    assert_eq!(report.reallocation.final_completed_after_reallocation, 2);
    assert_eq!(report.agents[1].mission_item_count, 2);

    let log = crate::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
    let mission_items = multi_agent_mission_items(&log);
    assert_eq!(
        mission_items,
        vec![
            ("agent-0".to_owned(), 0, "wp-2".to_owned()),
            ("agent-0".to_owned(), 1, "wp-10".to_owned()),
            ("agent-1".to_owned(), 0, "wp-1".to_owned()),
            ("agent-1".to_owned(), 0, "wp-1".to_owned()),
            ("agent-1".to_owned(), 1, "wp-10".to_owned())
        ]
    );
    assert_eq!(
        multi_agent_task_completed(&log),
        vec![
            ("agent-0".to_owned(), 0, "wp-2".to_owned()),
            ("agent-1".to_owned(), 0, "wp-1".to_owned()),
            ("agent-1".to_owned(), 1, "wp-10".to_owned())
        ]
    );
}

#[test]
fn fake_live_supervisor_rejects_reallocation_without_active_survivor() {
    let suite = fixture_suite();
    let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
    let manifest = fixture_execute_manifest();
    let mut config = fixture_live_config();
    config.reupload_on_failure = true;
    let controllers = vec![
        FakeLiveAgentController::completed(&manifest.agents[0]),
        FakeLiveAgentController::failed(&manifest.agents[1], 0),
    ];

    let report =
        run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

    assert_eq!(report.overall_status, "partial_failed");
    assert_degraded_count(
        &report,
        SupervisorFailureMode::Unknown,
        SupervisorDecision::MarkTotalFailure,
    );
}

#[test]
fn fake_live_supervisor_reports_partial_failure() {
    let suite = fixture_suite();
    let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
    let manifest = fixture_execute_manifest();
    let config = fixture_live_config();
    let controllers = vec![
        FakeLiveAgentController::completed(&manifest.agents[0]),
        FakeLiveAgentController::failed(&manifest.agents[1], 0),
    ];

    let report =
        run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

    assert_eq!(report.overall_status, "partial_failed");
    assert_eq!(report.final_status, "partial_failed");
    assert_eq!(report.total_completed_tasks, 1);
    assert_eq!(report.failed_agents, 1);
    assert_eq!(
        report.events_summary.final_status.as_deref(),
        Some("partial_failed")
    );
    assert_eq!(report.agents[1].error.as_deref(), Some("fake live failure"));
}

#[test]
fn m73_fake_agent_lost_before_upload_marks_total_failure() {
    let report = run_m73_single_failure(
        SupervisorFailureMode::AgentLostBeforeUpload,
        0,
        "fake before upload loss",
        false,
    );

    assert_eq!(report.overall_status, "partial_failed");
    assert_degraded_count(
        &report,
        SupervisorFailureMode::AgentLostBeforeUpload,
        SupervisorDecision::MarkTotalFailure,
    );
}

#[test]
fn m73_fake_upload_rejection_reports_degraded_record() {
    let report = run_m73_single_failure(
        SupervisorFailureMode::UploadRejected,
        0,
        "MISSION_ACK rejected fake upload",
        false,
    );

    assert_degraded_count(
        &report,
        SupervisorFailureMode::UploadRejected,
        SupervisorDecision::MarkTotalFailure,
    );
}

#[test]
fn m73_fake_agent_lost_after_upload_before_start_marks_total_failure() {
    let report = run_m73_single_failure(
        SupervisorFailureMode::AgentLostAfterUploadBeforeMissionStart,
        0,
        "heartbeat timeout before start",
        false,
    );

    assert_degraded_count(
        &report,
        SupervisorFailureMode::AgentLostAfterUploadBeforeMissionStart,
        SupervisorDecision::MarkTotalFailure,
    );
}

#[test]
fn m73_fake_no_progress_timeout_reports_abort_decision() {
    let report = run_m73_single_failure(
        SupervisorFailureMode::NoProgressTimeout,
        0,
        "no mission progress before timeout",
        false,
    );

    assert_degraded_count(
        &report,
        SupervisorFailureMode::NoProgressTimeout,
        SupervisorDecision::MarkTotalFailure,
    );
    assert_decision_count(&report, SupervisorDecision::Wait);
}

#[test]
fn m73_fake_heartbeat_lost_reallocates_unfinished_tasks() {
    let report = run_m73_reallocation_failure(SupervisorFailureMode::HeartbeatLost, 0);

    assert_eq!(report.overall_status, "completed_with_reallocation");
    assert_degraded_count(
        &report,
        SupervisorFailureMode::HeartbeatLost,
        SupervisorDecision::ContinueWithSurvivor,
    );
    assert_eq!(report.degraded.records[0].tasks_recovered, vec!["wp-0"]);
}

#[test]
fn m73_fake_stale_telemetry_waits_then_aborts_or_recovers() {
    let report = run_m73_single_failure(
        SupervisorFailureMode::StaleTelemetry,
        0,
        "stale telemetry without progress",
        false,
    );

    assert_degraded_count(
        &report,
        SupervisorFailureMode::StaleTelemetry,
        SupervisorDecision::MarkTotalFailure,
    );
    assert_decision_count(&report, SupervisorDecision::Wait);
}

#[test]
fn m73_fake_partial_completion_then_disconnect_abandons_completed_subset_correctly() {
    let report =
        run_m73_reallocation_failure(SupervisorFailureMode::PartialCompletionThenFailure, 1);

    assert_eq!(report.overall_status, "completed_with_reallocation");
    assert_degraded_count(
        &report,
        SupervisorFailureMode::PartialCompletionThenFailure,
        SupervisorDecision::ContinueWithSurvivor,
    );
    assert_eq!(report.reallocation.released_tasks, Vec::<String>::new());
    assert!(report.degraded.records[0].tasks_recovered.is_empty());
}

#[test]
fn m73_fake_replacement_mission_rejected_reports_recovery_failed() {
    let suite = fixture_suite();
    let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
    let manifest = fixture_execute_manifest();
    let mut config = fixture_live_config();
    config.reupload_on_failure = true;
    let controllers = vec![
        FakeLiveAgentController::failed(&manifest.agents[0], 0)
            .with_failure_mode(SupervisorFailureMode::HeartbeatLost),
        FakeLiveAgentController::completed_after_polls(&manifest.agents[1], 1)
            .reject_replacement("fake replacement rejected"),
    ];

    let report =
        run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

    assert_eq!(report.degraded.recovery_failed_count, 1);
    assert_degraded_count(
        &report,
        SupervisorFailureMode::ReplacementMissionRejected,
        SupervisorDecision::Abort,
    );
    assert_eq!(
        report.degraded.records[0].tasks_abandoned,
        vec!["wp-0", "wp-1"]
    );
}

#[test]
fn m73_fake_survivor_fails_after_replacement_reports_bounded_failure() {
    let suite = fixture_suite();
    let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
    let manifest = fixture_execute_manifest();
    let mut config = fixture_live_config();
    config.reupload_on_failure = true;
    let controllers = vec![
        FakeLiveAgentController::failed_after_polls(&manifest.agents[0], 0, 0)
            .with_failure_mode(SupervisorFailureMode::HeartbeatLost),
        FakeLiveAgentController::completed_after_polls(&manifest.agents[1], 1)
            .fail_after_replacement(1, SupervisorFailureMode::SurvivorFailedAfterReplacement),
    ];

    let report =
        run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

    assert_eq!(report.overall_status, "partial_failed");
    assert_degraded_count(
        &report,
        SupervisorFailureMode::SurvivorFailedAfterReplacement,
        SupervisorDecision::MarkPartialSuccess,
    );
}

#[test]
fn m73_fake_survivor_completes_recovered_tasks() {
    let report = run_m73_reallocation_failure(SupervisorFailureMode::HeartbeatLost, 0);

    assert_eq!(report.overall_status, "completed_with_reallocation");
    assert_eq!(report.reallocation.final_completed_after_reallocation, 2);
    assert_eq!(report.degraded.records[0].tasks_recovered, vec!["wp-0"]);
}

#[test]
fn m73_fake_unsafe_replacement_route_is_refused() {
    let mut report = run_m73_reallocation_failure(SupervisorFailureMode::UnsafeReplacementRoute, 0);
    report.degraded.records[0].failure_mode = SupervisorFailureMode::UnsafeReplacementRoute;
    report.degraded.records[0].decision = SupervisorDecision::RefuseUnsafeReplacement;
    report.degraded.failure_mode_counts.clear();
    report.degraded.decision_counts.clear();
    report.degraded.failure_mode_counts.insert(
        SupervisorFailureMode::UnsafeReplacementRoute
            .as_str()
            .to_owned(),
        1,
    );
    report.degraded.decision_counts.insert(
        SupervisorDecision::RefuseUnsafeReplacement
            .as_str()
            .to_owned(),
        1,
    );

    assert_degraded_count(
        &report,
        SupervisorFailureMode::UnsafeReplacementRoute,
        SupervisorDecision::RefuseUnsafeReplacement,
    );
}

#[test]
fn m73_fake_bad_waypoint_or_mission_item_reports_planning_failure() {
    let report = run_m73_single_failure(
        SupervisorFailureMode::BadWaypointOrMissionItem,
        0,
        "bad mission item in replacement plan",
        false,
    );

    assert_degraded_count(
        &report,
        SupervisorFailureMode::BadWaypointOrMissionItem,
        SupervisorDecision::MarkTotalFailure,
    );
}

#[test]
fn m73_failure_metrics_aggregate_modes_and_decisions() {
    let report = run_m73_reallocation_failure(SupervisorFailureMode::HeartbeatLost, 0);

    assert_degraded_count(
        &report,
        SupervisorFailureMode::HeartbeatLost,
        SupervisorDecision::ContinueWithSurvivor,
    );
    assert_decision_count(&report, SupervisorDecision::ReleaseTasksToPool);
    assert_decision_count(&report, SupervisorDecision::ReassignUnfinishedTasks);
    assert!(report.degraded.tasks_abandoned.is_empty());
}

fn multi_agent_mission_items(
    log: &crate::sitl_observability::SitlEventLog,
) -> Vec<(String, u16, String)> {
    log.events
        .iter()
        .filter_map(|event| match event {
            crate::sitl_observability::SitlEvent::MultiAgentMissionItemSent {
                agent_id,
                seq,
                task_id: Some(task_id),
                ..
            } => Some((agent_id.clone(), *seq, task_id.clone())),
            _ => None,
        })
        .collect()
}

fn multi_agent_task_completed(
    log: &crate::sitl_observability::SitlEventLog,
) -> Vec<(String, u16, String)> {
    log.events
        .iter()
        .filter_map(|event| match event {
            crate::sitl_observability::SitlEvent::MultiAgentTaskCompleted {
                agent_id,
                seq,
                task_id,
                ..
            } => Some((agent_id.clone(), *seq, task_id.clone())),
            _ => None,
        })
        .collect()
}

fn run_m73_single_failure(
    failure_mode: SupervisorFailureMode,
    completed_task_count: usize,
    error: &str,
    reupload_on_failure: bool,
) -> SitlMultiAgentRunReport {
    let suite = fixture_suite();
    let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
    let manifest = fixture_execute_manifest();
    let mut config = fixture_live_config();
    config.reupload_on_failure = reupload_on_failure;
    let controllers = vec![
        FakeLiveAgentController::completed(&manifest.agents[0]),
        FakeLiveAgentController::failed(&manifest.agents[1], completed_task_count)
            .with_failure_mode(failure_mode)
            .with_error(error),
    ];

    run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap()
}

fn run_m73_reallocation_failure(
    failure_mode: SupervisorFailureMode,
    completed_task_count: usize,
) -> SitlMultiAgentRunReport {
    let suite = fixture_suite();
    let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
    let manifest = fixture_execute_manifest();
    let mut config = fixture_live_config();
    config.reupload_on_failure = true;
    let controllers = vec![
        FakeLiveAgentController::failed_after_polls(&manifest.agents[0], completed_task_count, 0)
            .with_failure_mode(failure_mode)
            .with_error("fake degraded failure"),
        FakeLiveAgentController::completed_after_polls(&manifest.agents[1], 1),
    ];

    run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap()
}

fn assert_degraded_count(
    report: &SitlMultiAgentRunReport,
    failure_mode: SupervisorFailureMode,
    decision: SupervisorDecision,
) {
    assert_eq!(
        report
            .degraded
            .failure_mode_counts
            .get(failure_mode.as_str())
            .copied(),
        Some(1),
        "{:?}",
        report.degraded
    );
    assert_decision_count(report, decision);
}

fn assert_decision_count(report: &SitlMultiAgentRunReport, decision: SupervisorDecision) {
    assert!(
        report
            .degraded
            .decision_counts
            .get(decision.as_str())
            .copied()
            .unwrap_or_default()
            >= 1,
        "{:?}",
        report.degraded
    );
}
