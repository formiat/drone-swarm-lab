use super::supervisor_tests::*;

fn public_scenario(path: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
        .to_string_lossy()
        .into_owned()
}
#[test]
fn cli_rejects_missing_run_report_value() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--execute",
        "--run-report",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing required argument"));
    assert!(stderr.contains("--run-report"));
}

#[test]
fn cli_rejects_missing_replay_log_value() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--mock",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--replay-log",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing required argument"));
    assert!(stderr.contains("--replay-log"));
}

#[test]
fn cli_rejects_replay_log_for_dry_run() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let report_dir = tempfile::tempdir().unwrap();
    let replay_log = report_dir.path().join("sitl-log.json");
    let replay_log = replay_log.to_str().unwrap();
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--replay-log",
        replay_log,
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("replay log option --replay-log is not supported for dry-run"));
}

#[test]
fn dry_run_artifact_contains_export_metadata() {
    let scenario = public_scenario("scenarios/urban.patrol.json");
    let report_dir = tempfile::tempdir().unwrap();
    let artifact = report_dir.path().join("urban-dry-run.json");
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        &scenario,
        "--agent-id",
        "agent-0",
        "--dry-run-artifact",
        artifact.to_str().unwrap(),
    ]);

    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&artifact).unwrap()).unwrap();
    assert_eq!(json["schema_version"], "sitl_dry_run_artifact.v1");
    assert_eq!(json["mission"], "urban-patrol");
    assert_eq!(json["export_kind"], "urban_route");
    assert_eq!(json["planner_or_adapter"], "urban_route_export:dijkstra");
    assert_eq!(json["route_length_m"], 80.0);
    assert_eq!(json["segment_count"], 4);
    assert_eq!(json["waypoint_count"], 4);
    assert_eq!(json["effective_geo_origin"]["lat_deg"], 47.397742);
    assert_eq!(json["start_waypoint"]["edge_id"], "road-n0-n1");
    assert_eq!(json["end_waypoint"]["edge_id"], "road-n3-n0");
}

#[test]
fn dry_run_artifact_rejects_non_dry_run() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let report_dir = tempfile::tempdir().unwrap();
    let artifact = report_dir.path().join("sitl-dry-run.json");
    let output = run_sitl_agent(&[
        "--mock",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--dry-run-artifact",
        artifact.to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr
        .contains("dry-run artifact option --dry-run-artifact is only supported for --dry-run"));
}

#[test]
fn mock_run_writes_replay_log_events() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let report_dir = tempfile::tempdir().unwrap();
    let replay_log = report_dir.path().join("nested").join("sitl-log.json");
    let output = run_sitl_agent(&[
        "--mock",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--replay-log",
        replay_log.to_str().unwrap(),
    ]);

    assert!(output.status.success());
    let log = swarm_examples::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
    let summary = swarm_examples::sitl_observability::summarize_sitl_event_log(&log);
    assert_eq!(summary.connection_opened, 1);
    assert_eq!(summary.mission_count_sent, 1);
    assert_eq!(summary.mission_item_sent, 2);
    assert_eq!(summary.task_completed, 2);
    assert_eq!(summary.final_status, Some("completed".to_owned()));
}

#[test]
fn cli_rejects_run_report_without_execute() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let report_dir = tempfile::tempdir().unwrap();
    let report = report_dir.path().join("sitl-report.json");
    let report = report.to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--run-report",
        report,
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("run report option --run-report requires --connection"));
}

#[test]
fn cli_rejects_run_report_without_connection() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let report_dir = tempfile::tempdir().unwrap();
    let report = report_dir.path().join("sitl-report.json");
    let report = report.to_str().unwrap();
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--run-report",
        report,
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("lifecycle option --run-report requires --connection"));
}

#[test]
fn cli_rejects_missing_telemetry_timeout_value() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--execute",
        "--telemetry-timeout",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing required argument"));
    assert!(stderr.contains("--telemetry-timeout"));
}

#[test]
fn cli_rejects_invalid_no_progress_timeout_value() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--execute",
        "--no-progress-timeout",
        "0",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid duration"));
    assert!(stderr.contains("--no-progress-timeout"));
}

#[test]
fn cli_rejects_telemetry_timeout_without_execute() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--telemetry-timeout",
        "1",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("lifecycle option --telemetry-timeout requires --execute"));
}

#[test]
fn cli_rejects_conflicting_lifecycle_modes() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--upload-only",
        "--execute",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("conflicting lifecycle modes"));
}

#[test]
fn cli_rejects_no_arm_without_execute() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--no-arm",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("lifecycle option --no-arm requires --execute"));
}

#[test]
fn cli_rejects_abort_after_without_execute() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--abort-after",
        "1",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("lifecycle option --abort-after requires --execute"));
}

#[test]
fn cli_rejects_missing_abort_after_value() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--execute",
        "--abort-after",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing required argument"));
    assert!(stderr.contains("--abort-after"));
}

#[test]
fn cli_rejects_invalid_abort_after_value() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--execute",
        "--abort-after",
        "nan",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid duration"));
    assert!(stderr.contains("--abort-after"));
}

#[test]
fn cli_rejects_missing_timeout_value() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--timeout",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing required argument"));
    assert!(stderr.contains("--timeout"));
}

#[test]
fn cli_rejects_invalid_timeout_value() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--timeout",
        "0",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid duration"));
    assert!(stderr.contains("--timeout"));
}
