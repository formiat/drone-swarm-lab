#[test]
fn multi_agent_sitl_supervisor_rejects_missing_and_invalid_cli_args_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let scenario = scenario.path().to_str().unwrap().to_owned();
    let config = config.path().to_str().unwrap().to_owned();

    let cases: Vec<(Vec<&str>, &str)> = vec![
        (vec![], "missing SITL mode"),
        (
            vec!["--mock", "--config", &config],
            "missing required argument: --scenario",
        ),
        (
            vec!["--mock", "--scenario", &scenario],
            "missing required argument: --config",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--manifest",
            ],
            "missing required argument: --manifest",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--replay-log",
            ],
            "missing required argument: --replay-log",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--fail-agent",
            ],
            "missing required argument: --fail-agent",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--fail-after-ticks",
            ],
            "missing required argument: --fail-after-ticks",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--heartbeat-timeout-ticks",
            ],
            "missing required argument: --heartbeat-timeout-ticks",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--max-ticks",
            ],
            "missing required argument: --max-ticks",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--output-dir",
            ],
            "missing required argument: --output-dir",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--run-id",
            ],
            "missing required argument: --run-id",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--fail-after-ticks",
                "abc",
            ],
            "invalid --fail-after-ticks value 'abc'",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--heartbeat-timeout-ticks",
                "abc",
            ],
            "invalid --heartbeat-timeout-ticks value 'abc'",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--max-ticks",
                "abc",
            ],
            "invalid --max-ticks value 'abc'",
        ),
        (
            vec![
                "--dry-run",
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
            ],
            "conflicting SITL modes",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--unknown",
            ],
            "unknown argument: --unknown",
        ),
    ];

    for (args, expected) in cases {
        assert_sitl_supervisor_cli_error(&args, expected);
    }
}

#[test]
fn multi_agent_sitl_supervisor_connection_rejects_invalid_cli_combinations_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let scenario = scenario.path().to_str().unwrap().to_owned();
    let config = config.path().to_str().unwrap().to_owned();

    let cases: Vec<(Vec<&str>, &str)> = vec![
        (
            vec!["--connection", "--scenario", &scenario, "--config", &config],
            "lifecycle option --connection requires --execute",
        ),
        (
            vec![
                "--dry-run",
                "--execute",
                "--scenario",
                &scenario,
                "--config",
                &config,
            ],
            "lifecycle option --execute requires --connection",
        ),
        (
            vec![
                "--dry-run",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--run-report",
                "report.json",
            ],
            "run report option --run-report requires --connection",
        ),
        (
            vec![
                "--connection",
                "--execute",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--safety-config",
            ],
            "missing required argument: --safety-config",
        ),
        (
            vec![
                "--dry-run",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--safety-config",
                "safety.json",
            ],
            "--safety-config requires --connection --execute",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--safety-config",
                "safety.json",
            ],
            "--safety-config requires --connection --execute",
        ),
        (
            vec![
                "--connection",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--safety-config",
                "safety.json",
            ],
            "--safety-config requires --connection --execute",
        ),
        (
            vec![
                "--connection",
                "--execute",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--timeout",
                "0",
            ],
            "invalid duration for --timeout",
        ),
        (
            vec![
                "--connection",
                "--execute",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--fail-agent",
                "agent-0",
            ],
            "--fail-agent requires --mock",
        ),
        (
            vec![
                "--mock",
                "--scenario",
                &scenario,
                "--config",
                &config,
                "--reupload-on-failure",
            ],
            "--reupload-on-failure requires --connection --execute",
        ),
    ];

    for (args, expected) in cases {
        assert_sitl_supervisor_cli_error(&args, expected);
    }
}

#[test]
fn multi_agent_sitl_supervisor_connection_rejects_upload_only_manifest_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);

    let output = run_sitl_supervisor(&[
        "--connection",
        "--execute",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--config",
        config.path().to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("live supervisor execute requires lifecycle=execute"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
fn multi_agent_sitl_supervisor_connection_rejects_hardware_candidate_before_upload_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_execute_config_with_connections(
        "tcpout:192.168.1.10:5760",
        "udp:127.0.0.1:14560",
    );

    let output = run_sitl_supervisor(&[
        "--connection",
        "--execute",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--config",
        config.path().to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires --allow-hardware-candidate"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
fn multi_agent_sitl_supervisor_connection_rejects_unsafe_agent_subset_before_upload_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_execute_config_with_connections(
        "udp:127.0.0.1:14550",
        "udp:127.0.0.1:14560",
    );
    let safety = write_safety_config(
        r#"{
  "geofence": { "min_x": 0.0, "max_x": 20.0, "min_y": 0.0, "max_y": 25.0 }
}"#,
    );

    let output = run_sitl_supervisor(&[
        "--connection",
        "--execute",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--config",
        config.path().to_str().unwrap(),
        "--safety-config",
        safety.path().to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("safety validation failed"));
    assert!(stderr.contains("rule_id=outside_geofence"));
    assert!(stderr.contains("task_id=wp-1"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
#[cfg(not(feature = "mavlink-transport"))]
fn multi_agent_sitl_supervisor_connection_validates_before_feature_error_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_execute_config_with_connections(
        "udp:127.0.0.1:14550",
        "udp:127.0.0.1:14560",
    );
    let safety = write_safety_config("{}");

    let output = run_sitl_supervisor(&[
        "--connection",
        "--execute",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--config",
        config.path().to_str().unwrap(),
        "--safety-config",
        safety.path().to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(20));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("feature missing"));
    assert!(stderr.contains("mavlink-transport"));
    assert!(!stderr.contains("safety validation failed"));
}

#[test]
fn portable_sitl_regression_smoke() {
    let scenario = write_sitl_scenario();
    let suite = swarm_examples::sitl_plan::load_sitl_suite(scenario.path()).unwrap();
    let entry = swarm_examples::sitl_plan::first_sitl_entry(&suite, scenario.path()).unwrap();
    let plan =
        swarm_examples::sitl_plan::build_sitl_plan(&suite, scenario.path(), "agent-0").unwrap();
    let safety = swarm_examples::sitl_safety::load_sitl_safety_config(None).unwrap();

    swarm_examples::sitl_safety::validate_pre_upload_safety(entry, "agent-0", &safety).unwrap();
    assert_eq!(plan.agent_id, "agent-0");
    assert_eq!(plan.suite_name, "SITL Waypoints");
    assert_eq!(plan.scenario_name, "sitl_waypoints_test");
    assert_eq!(plan.mission, "sitl");
    assert_eq!(plan.profile, "waypoints");
    assert_eq!(plan.coordinate_frame.name(), "local_simulation");
    assert_eq!(plan.waypoints.len(), 2);
    assert_eq!(plan.waypoints[0].seq, 0);
    assert_eq!(plan.waypoints[0].task_id, "wp-0");
    assert_eq!(plan.waypoints[0].z, 3.5);
    assert_eq!(plan.waypoints[1].seq, 1);
    assert_eq!(plan.waypoints[1].task_id, "wp-1");
    assert_eq!(plan.waypoints[1].z, 0.0);

    let scenario_path = scenario.path().to_str().unwrap();
    let dry_run = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        scenario_path,
        "--agent-id",
        "agent-0",
    ]);
    let dry_run_stderr = String::from_utf8_lossy(&dry_run.stderr);
    assert!(dry_run.status.success(), "dry-run failed: {dry_run_stderr}");
    let dry_run_stdout = String::from_utf8_lossy(&dry_run.stdout);
    assert!(dry_run_stdout.contains("mode: dry-run"));
    assert!(dry_run_stdout.contains("suite_name: SITL Waypoints"));
    assert!(dry_run_stdout.contains("scenario_name: sitl_waypoints_test"));
    assert!(dry_run_stdout.contains("mission: sitl"));
    assert!(dry_run_stdout.contains("profile: waypoints"));
    assert!(dry_run_stdout.contains("coordinate_frame: local_simulation"));
    assert!(dry_run_stdout.contains("altitude_source: pose.z"));
    assert!(dry_run_stdout.contains("seq=0 task_id=wp-0 x=10.000 y=20.000 z=3.500"));
    assert!(dry_run_stdout.contains("seq=1 task_id=wp-1 x=30.000 y=40.000 z=0.000"));

    let replay_dir = tempfile::tempdir().unwrap();
    let replay_log = replay_dir.path().join("portable-smoke.sitl-log.json");
    let mock = run_sitl_agent(&[
        "--mock",
        "--scenario",
        scenario_path,
        "--agent-id",
        "agent-0",
        "--replay-log",
        replay_log.to_str().unwrap(),
    ]);
    let mock_stderr = String::from_utf8_lossy(&mock.stderr);
    assert!(mock.status.success(), "mock failed: {mock_stderr}");
    assert!(mock.stdout.is_empty());
    assert!(mock_stderr.contains("SITL Agent: agent-0 | 2 waypoints | mock=true"));
    assert!(mock_stderr.contains("WAYPOINT seq=0 x=10.0 y=20.0 z=3.5"));
    assert!(mock_stderr.contains("WAYPOINT seq=1 x=30.0 y=40.0 z=0.0"));
    assert!(mock_stderr.contains("Mock mode: 2 waypoints sent."));

    let log = swarm_examples::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
    assert_eq!(log.mode.as_str(), "mock");
    assert_eq!(log.scenario_name, "sitl_waypoints_test");
    assert_eq!(log.mission, "sitl");
    assert_eq!(log.profile, "waypoints");
    let summary = swarm_examples::sitl_observability::summarize_sitl_event_log(&log);
    assert_eq!(summary.connection_opened, 1);
    assert_eq!(summary.mission_count_sent, 1);
    assert_eq!(summary.mission_item_sent, 2);
    assert_eq!(summary.task_completed, 2);
    assert_eq!(summary.final_status, Some("completed".to_owned()));
}

#[test]
fn cli_validation_missing_mode() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&["--scenario", scenario, "--agent-id", "agent-0"]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing SITL mode"));
}

#[test]
#[cfg(not(feature = "mavlink-transport"))]
fn cli_validation_connection_without_feature() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("feature missing"));
    assert!(stderr.contains("mavlink-transport"));
    assert!(!stderr.contains("hardware candidate"));
}

#[test]
fn connection_rejects_unsafe_mission_before_feature_error_or_upload() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let safety = write_safety_config(
        r#"{
  "geofence": { "min_x": 0.0, "max_x": 20.0, "min_y": 0.0, "max_y": 25.0 }
}"#,
    );
    let safety = safety.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--safety-config",
        safety,
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("safety validation failed"));
    assert!(stderr.contains("rule_id=outside_geofence"));
    assert!(stderr.contains("task_id=wp-1"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
#[cfg(not(feature = "mavlink-transport"))]
fn connection_accepts_valid_safety_config_then_hits_existing_no_feature_error() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let safety = write_safety_config("{}");
    let safety = safety.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--safety-config",
        safety,
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("feature missing"));
    assert!(stderr.contains("mavlink-transport"));
    assert!(!stderr.contains("safety validation failed"));
}

#[test]
fn bad_safety_config_path_is_typed_error() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let missing_dir = tempfile::tempdir().unwrap();
    let missing_path = missing_dir.path().join("missing-safety-config.json");
    let missing_path = missing_path.to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--safety-config",
        missing_path,
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("safety config read failed"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
fn bad_safety_config_json_is_typed_error() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let safety = write_safety_config("{not-json");
    let safety = safety.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--safety-config",
        safety,
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("safety config parse failed"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
fn cli_validation_missing_safety_config_value() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--safety-config",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing required argument"));
    assert!(stderr.contains("--safety-config"));
}

#[test]
fn cli_validation_bad_connection_string() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "not-a-connection",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bad connection string"));
}

#[test]
fn cli_validation_incomplete_udp_connection_string() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bad connection string"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
fn hardware_candidate_remote_udp_requires_explicit_allow() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:192.168.1.10:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("hardware candidate connection"));
    assert!(stderr.contains("--allow-hardware-candidate"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
fn hardware_candidate_serial_requires_explicit_allow() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "serial:/dev/ttyUSB0:57600",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("hardware candidate connection"));
    assert!(stderr.contains("--allow-hardware-candidate"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
#[cfg(not(feature = "mavlink-transport"))]
fn hardware_candidate_opt_in_warns_then_reaches_feature_error() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--connection",
        "udp:192.168.1.10:14550",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--allow-hardware-candidate",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("WARNING"));
    assert!(stderr.contains("hardware_candidate"));
    assert!(stderr.contains("docs/HARDWARE_READINESS.md"));
    assert!(stderr.contains("feature missing"));
}

#[test]
fn hardware_candidate_flag_requires_connection_mode() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
        "--allow-hardware-candidate",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("connection option --allow-hardware-candidate requires"));
}

#[test]
fn multi_agent_config_implied_hardware_candidate_requires_explicit_allow() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config_with_connections(
        false,
        "udp:192.168.1.10:14550",
        "udp:127.0.0.1:14560",
    );
    let output = run_sitl_agent(&[
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-0",
        "--multi-agent-config",
        config.path().to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("hardware candidate connection"));
    assert!(stderr.contains("--allow-hardware-candidate"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
#[cfg(not(feature = "mavlink-transport"))]
fn multi_agent_config_implied_hardware_candidate_opt_in_warns() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config_with_connections(
        false,
        "udp:192.168.1.10:14550",
        "udp:127.0.0.1:14560",
    );
    let output = run_sitl_agent(&[
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-0",
        "--multi-agent-config",
        config.path().to_str().unwrap(),
        "--allow-hardware-candidate",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("WARNING"));
    assert!(stderr.contains("hardware_candidate"));
    assert!(stderr.contains("feature missing"));
}

#[test]
fn cli_validation_conflicting_modes() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&[
        "--mock",
        "--dry-run",
        "--scenario",
        scenario,
        "--agent-id",
        "agent-0",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("conflicting SITL modes"));
}

#[test]
#[cfg(not(feature = "mavlink-transport"))]
fn cli_accepts_upload_only_lifecycle_option_before_feature_gate() {
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
        "--timeout",
        "0.001",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("feature missing"));
    assert!(!stderr.contains("lifecycle option"));
}

#[test]
#[cfg(not(feature = "mavlink-transport"))]
fn cli_accepts_execute_lifecycle_option_before_feature_gate() {
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
        "--execute",
        "--no-arm",
        "--abort-after",
        "0",
        "--timeout",
        "0.001",
        "--telemetry-timeout",
        "0.001",
        "--no-progress-timeout",
        "0.001",
        "--run-report",
        report,
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("feature missing"));
    assert!(!stderr.contains("lifecycle option"));
}
