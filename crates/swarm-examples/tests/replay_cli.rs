use std::process::Command;

use swarm_examples::sitl_observability::{SitlEvent, SitlEventLog, SitlEventLogMode};

const M58_SITL_LOG: &str = include_str!(
    "../../../results/m58_multi_agent_px4_sih_execute_2026-05-31/m58-multi-agent-px4-sih-execute/events.sitl-log.json"
);
const M58_REPLAY_SUMMARY: &str = include_str!(
    "../../../results/m58_multi_agent_px4_sih_execute_2026-05-31/m58-multi-agent-px4-sih-execute/replay-summary.txt"
);
const M59_SITL_LOG: &str = include_str!(
    "../../../results/m59_px4_sih_failure_reallocation_2026-05-31/m59-px4-sih-failure-reallocation/events.sitl-log.json"
);
const M59_REPLAY_SUMMARY: &str = include_str!(
    "../../../results/m59_px4_sih_failure_reallocation_2026-05-31/m59-px4-sih-failure-reallocation/replay-summary.txt"
);

fn run_replay(args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_replay"));
    cmd.args(args);
    cmd.output().expect("Failed to execute replay")
}

fn parse_committed_sitl_log(content: &str) -> SitlEventLog {
    serde_json::from_str(content).expect("committed SITL event log should parse")
}

fn create_test_replay_log(path: &std::path::Path) {
    use swarm_replay::{Event, EventLogBuilder};
    use swarm_types::{AgentId, Pose};

    let mut builder = EventLogBuilder::new("test-run", 42, "test_scenario");
    builder.push(Event::TickStart { tick: 0 });
    builder.push(Event::PoseUpdated {
        agent_id: AgentId::from("agent-0".to_owned()),
        pose: Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        },
        tick: 0,
    });
    builder.push(Event::TickStart { tick: 50 });
    builder.push(Event::PoseUpdated {
        agent_id: AgentId::from("agent-0".to_owned()),
        pose: Pose {
            x: 10.0,
            y: 10.0,
            ..Default::default()
        },
        tick: 50,
    });
    builder.push(Event::TickStart { tick: 100 });
    let log = builder.build();
    let json = serde_json::to_string_pretty(&log).unwrap();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, json).unwrap();
}

fn create_urban_replay_log(path: &std::path::Path) {
    use swarm_replay::{Event, EventLogBuilder};
    use swarm_types::{AgentId, Pose, UrbanBusId, UrbanEdgeId, UrbanNodeId};

    let agent_id = AgentId::from("agent-0".to_owned());
    let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
    let bus_id = UrbanBusId::from("bus-0".to_owned());
    let mut builder = EventLogBuilder::new("urban-run", 0, "urban_patrol_small_block");
    builder.push(Event::UrbanRoutePlanned {
        agent_id: agent_id.clone(),
        tick: 0,
        edge_ids: vec![edge_id.clone()],
        route_length_m: 20.0,
    });
    builder.push(Event::PoseUpdated {
        agent_id: AgentId::from("agent-1".to_owned()),
        pose: Pose {
            x: 5.0,
            y: 5.0,
            ..Default::default()
        },
        tick: 0,
    });
    builder.push(Event::UrbanSegmentEntered {
        agent_id: agent_id.clone(),
        tick: 0,
        segment_index: 0,
        edge_id: edge_id.clone(),
        from: UrbanNodeId::from("n0".to_owned()),
        to: UrbanNodeId::from("n1".to_owned()),
    });
    builder.push(Event::UrbanSegmentCompleted {
        agent_id: agent_id.clone(),
        tick: 10,
        segment_index: 0,
        edge_id,
    });
    builder.push(Event::BusObserved {
        agent_id: agent_id.clone(),
        tick: 10,
        bus_id: bus_id.clone(),
        pose: Pose {
            x: 20.0,
            y: 0.0,
            ..Default::default()
        },
        distance_m: 0.0,
        detector_seed: 66,
    });
    builder.push(Event::BusDetected {
        agent_id: agent_id.clone(),
        tick: 10,
        bus_id: bus_id.clone(),
        pose: Pose {
            x: 20.0,
            y: 0.0,
            ..Default::default()
        },
        distance_m: 0.0,
        detector_seed: 66,
    });
    builder.push(Event::BusFalsePositive {
        agent_id: agent_id.clone(),
        tick: 11,
        pose: Pose {
            x: 20.0,
            y: 0.0,
            ..Default::default()
        },
        detector_seed: 66,
    });
    builder.push(Event::UrbanSearchCompleted {
        agent_id: agent_id.clone(),
        tick: 10,
        detected: true,
        bus_id: Some(bus_id),
        reason: "detected".to_owned(),
        distance_travelled_m: 20.0,
    });
    builder.push(Event::UrbanPatrolCompleted {
        agent_id,
        tick: 10,
        route_length_m: 20.0,
        distance_travelled_m: 20.0,
    });
    let log = builder.build();
    let json = serde_json::to_string_pretty(&log).unwrap();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, json).unwrap();
}

fn create_test_sitl_log(path: &std::path::Path) {
    use swarm_examples::sitl_observability::{
        SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
    };

    let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
        run_id: "sitl-test-run".to_owned(),
        scenario_path: std::path::PathBuf::from("scenarios/sitl.waypoints.json"),
        scenario_name: "sitl_waypoints_test".to_owned(),
        mission: "sitl".to_owned(),
        profile: "waypoints".to_owned(),
        agent_id: "agent-0".to_owned(),
        connection_string: Some("udp:127.0.0.1:14550".to_owned()),
        mode: SitlEventLogMode::ConnectionExecute,
    });
    recorder.push_connection_opened();
    recorder.push_mission_count_sent(2);
    recorder.push_mission_item_requested(0);
    recorder.push_mission_item_sent(0, Some("wp-0".to_owned()));
    recorder.push_waypoint_reached(0, Some("wp-0".to_owned()));
    recorder.push_task_completed(0, "wp-0");
    recorder.push_agent_lost("agent-1");
    recorder.push_task_released("wp-1", "agent-1");
    recorder.push_task_reassigned("wp-1", "agent-1", "agent-0", 0);
    recorder.push_reallocation_completed("agent-1", 1, vec!["wp-1".to_owned()], 0);
    recorder.push_abort_requested(Some("Accepted".to_owned()));
    recorder.push_failure("failed", "test failure");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    swarm_examples::sitl_observability::write_sitl_event_log(path, recorder.log()).unwrap();
}

#[test]
fn replay_cli_summary_outputs_ticks() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("coverage_with_failure_0.replay.json");
    create_test_replay_log(&log_path);
    let log_str = log_path.to_str().unwrap();
    let output = run_replay(&["--log", log_str, "--summary"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "replay --summary failed: {}",
        stderr
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Total ticks:"));
    assert!(stdout.contains("Events:"));
}

#[test]
fn replay_cli_summary_outputs_urban_counts() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("urban.replay.json");
    create_urban_replay_log(&log_path);
    let output = run_replay(&["--log", log_path.to_str().unwrap(), "--summary"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "replay --summary failed: {}",
        stderr
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Urban routes planned: 1"));
    assert!(stdout.contains("Urban segments entered: 1"));
    assert!(stdout.contains("Urban segments completed: 1"));
    assert!(stdout.contains("Urban patrol completions: 1"));
    assert!(stdout.contains("Urban completion ticks: [10]"));
    assert!(stdout.contains("Bus observations: 1"));
    assert!(stdout.contains("Bus detections: 1"));
    assert!(stdout.contains("Bus false positives: 1"));
    assert!(stdout.contains("Urban search completions: 1"));
    assert!(stdout.contains("Urban search detection ticks: [10]"));
}

#[test]
fn replay_cli_timeline_outputs_urban_events() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("urban.replay.json");
    create_urban_replay_log(&log_path);
    let output = run_replay(&["--log", log_path.to_str().unwrap(), "--timeline"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "replay --timeline failed: {}",
        stderr
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("=== Timeline ==="));
    assert!(stdout.contains("UrbanRoutePlanned"));
    assert!(stdout.contains("UrbanSegmentCompleted"));
    assert!(stdout.contains("BusDetected"));
}

#[test]
fn replay_cli_timeline_filters_by_agent() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("urban.replay.json");
    create_urban_replay_log(&log_path);
    let output = run_replay(&[
        "--log",
        log_path.to_str().unwrap(),
        "--timeline",
        "--agent",
        "agent-1",
    ]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("agent=agent-1"));
    assert!(stdout.contains("PoseUpdated"));
    assert!(!stdout.contains("agent=agent-0"));
}

#[test]
fn replay_cli_timeline_filters_by_urban_category() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("urban.replay.json");
    create_urban_replay_log(&log_path);
    let output = run_replay(&[
        "--log",
        log_path.to_str().unwrap(),
        "--timeline",
        "--category",
        "urban",
    ]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("category=urban"));
    assert!(stdout.contains("UrbanRoutePlanned"));
    assert!(!stdout.contains("PoseUpdated"));
}

#[test]
fn replay_cli_timeline_rejects_unknown_category() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("urban.replay.json");
    create_urban_replay_log(&log_path);
    let output = run_replay(&[
        "--log",
        log_path.to_str().unwrap(),
        "--timeline",
        "--category",
        "bad",
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown replay category"));
}

#[test]
fn replay_cli_timeline_filters_reject_sitl_summary() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("sitl-log.json");
    create_test_sitl_log(&log_path);

    let output = run_replay(&["--sitl-summary", log_path.to_str().unwrap(), "--timeline"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot be combined"));
}

#[test]
fn replay_cli_timeline_filters_require_timeline() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("urban.replay.json");
    create_urban_replay_log(&log_path);
    let output = run_replay(&["--log", log_path.to_str().unwrap(), "--agent", "agent-0"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("require --timeline"));
}

#[test]
fn replay_cli_tick_outputs_ascii() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("coverage_with_failure_0.replay.json");
    create_test_replay_log(&log_path);
    let log_str = log_path.to_str().unwrap();
    let output = run_replay(&["--log", log_str, "--tick", "50"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Snapshot at tick 50"));
}

#[test]
fn replay_cli_invalid_log_exits_error() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let nonexistent = tmp_dir.path().join("nonexistent_replay.json");
    let output = run_replay(&["--log", nonexistent.to_str().unwrap(), "--summary"]);
    assert!(!output.status.success());
}

#[test]
fn replay_cli_sitl_summary_outputs_counts() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("sitl-log.json");
    create_test_sitl_log(&log_path);

    let output = run_replay(&["--sitl-summary", log_path.to_str().unwrap()]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("SITL run: sitl-test-run"));
    assert!(stdout.contains("Scenario: sitl_waypoints_test"));
    assert!(stdout.contains("requested=1"));
    assert!(stdout.contains("waypoint_reached=1"));
    assert!(stdout.contains("agent_lost=1"));
    assert!(stdout.contains("tasks_recovered=1"));
    assert!(stdout.contains("aborts=1"));
    assert!(stdout.contains("final_status=failed"));
}

#[test]
fn replay_cli_sitl_summary_rejects_conflicting_modes() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let log_path = tmp_dir.path().join("sitl-log.json");
    create_test_sitl_log(&log_path);

    let output = run_replay(&["--sitl-summary", log_path.to_str().unwrap(), "--summary"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot be combined"));
}

#[test]
fn replay_cli_sitl_summary_invalid_log_exits_error() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let nonexistent = tmp_dir.path().join("missing-sitl-log.json");

    let output = run_replay(&["--sitl-summary", nonexistent.to_str().unwrap()]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Failed to read SITL replay log"));
}

#[test]
fn committed_m58_m59_sitl_summaries_have_expected_categories() {
    for (summary, run_id, final_status) in [
        (
            M58_REPLAY_SUMMARY,
            "m58-multi-agent-px4-sih-execute",
            "final_status=completed",
        ),
        (
            M59_REPLAY_SUMMARY,
            "m59-px4-sih-failure-reallocation",
            "final_status=completed_with_reallocation",
        ),
    ] {
        assert!(summary.contains(run_id), "summary missing run id {run_id}");
        assert!(
            summary.contains("Multi-agent:"),
            "summary missing multi-agent category"
        );
        assert!(
            summary.contains("Multi-agent events:"),
            "summary missing multi-agent event category"
        );
        assert!(
            summary.contains(final_status),
            "summary missing final status {final_status}"
        );
    }

    for required in [
        "agent_lost=1",
        "task_released=2",
        "task_reassigned=2",
        "tasks_recovered=2",
        "survivor_mission_updates=1",
    ] {
        assert!(
            M59_REPLAY_SUMMARY.contains(required),
            "M59 summary missing {required}"
        );
    }
}

#[test]
fn committed_m58_m59_sitl_event_logs_parse_and_keep_expected_events() {
    let m58 = parse_committed_sitl_log(M58_SITL_LOG);
    assert_eq!(m58.schema_version, "sitl_event_log.v1");
    assert_eq!(m58.mode, SitlEventLogMode::ConnectionExecute);
    assert!(m58.events.iter().any(|event| matches!(
        event,
        SitlEvent::MultiAgentRunStarted { agent_count: 2, .. }
    )));
    assert_eq!(
        m58.events
            .iter()
            .filter(|event| matches!(event, SitlEvent::MultiAgentAgentStarted { .. }))
            .count(),
        2
    );
    assert_eq!(
        m58.events
            .iter()
            .filter(|event| matches!(event, SitlEvent::MultiAgentTaskCompleted { .. }))
            .count(),
        4
    );
    assert!(m58.events.iter().any(|event| matches!(
        event,
        SitlEvent::MultiAgentRunFinished {
            overall_status,
            ..
        } if overall_status == "completed"
    )));

    let m59 = parse_committed_sitl_log(M59_SITL_LOG);
    assert_eq!(m59.schema_version, "sitl_event_log.v1");
    assert_eq!(m59.mode, SitlEventLogMode::ConnectionExecute);
    for expected in [
        "agent_lost",
        "task_released",
        "task_reassigned",
        "survivor_mission_update_started",
        "survivor_mission_update_completed",
        "reallocation_completed",
    ] {
        let present = m59.events.iter().any(|event| {
            matches!(
                (expected, event),
                ("agent_lost", SitlEvent::AgentLost { .. })
                    | ("task_released", SitlEvent::TaskReleased { .. })
                    | ("task_reassigned", SitlEvent::TaskReassigned { .. })
                    | (
                        "survivor_mission_update_started",
                        SitlEvent::SurvivorMissionUpdateStarted { .. }
                    )
                    | (
                        "survivor_mission_update_completed",
                        SitlEvent::SurvivorMissionUpdateCompleted { .. }
                    )
                    | (
                        "reallocation_completed",
                        SitlEvent::ReallocationCompleted { .. }
                    )
            )
        });
        assert!(present, "M59 event log missing {expected}");
    }
}

#[test]
fn committed_m59_replacement_completion_seq_matches_replacement_mission() {
    let log = parse_committed_sitl_log(M59_SITL_LOG);

    for (task_id, expected_seq) in [("wp-0", 2), ("wp-1", 3)] {
        assert!(log.events.iter().any(|event| matches!(
            event,
            SitlEvent::MultiAgentMissionItemSent {
                agent_id,
                seq,
                task_id: Some(sent_task_id),
                ..
            } if agent_id == "agent-1" && sent_task_id == task_id && *seq == expected_seq
        )));
        assert!(log.events.iter().any(|event| matches!(
            event,
            SitlEvent::MultiAgentTaskCompleted {
                agent_id,
                seq,
                task_id: completed_task_id,
                ..
            } if agent_id == "agent-1" && completed_task_id == task_id && *seq == expected_seq
        )));
    }
}
