use std::process::Command;

fn run_replay(args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_replay"));
    cmd.args(args);
    cmd.output().expect("Failed to execute replay")
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
