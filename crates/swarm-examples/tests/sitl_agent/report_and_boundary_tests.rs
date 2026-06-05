use super::supervisor_tests::*;

fn public_scenario(path: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
        .to_string_lossy()
        .into_owned()
}

fn write_preflight_geofence_violation_scenario() -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        r#"{
  "schema_version": "0.1",
  "name": "Preflight Failure",
  "description": "preflight failure fixture",
  "scenarios": [
    {
      "mission": "sitl",
      "profile": "preflight",
      "scenario": {
        "name": "preflight_failure",
        "seed": 0,
        "agents": [
          {
            "id": "agent-0",
            "role": "scout",
            "health": "alive",
            "pose": { "x": 0.0, "y": 0.0 },
            "capabilities": [],
            "current_task": null,
            "battery": 100.0,
            "comms_range": 1000.0,
            "generation": 1,
            "speed": 0.0,
            "max_range": 1000.0,
            "battery_drain_rate": 0.0
          }
        ],
        "tasks": [
          {
            "id": "wp-outside",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 100.0, "y": 100.0, "z": 5.0 },
            "grid_cell": null
          }
        ],
        "ground_nodes": [],
        "base_station": null
      },
      "run_config": {
        "max_ticks": 50,
        "safety_config": {
          "geofence": {
            "bounds": { "min_x": 0.0, "max_x": 10.0, "min_y": 0.0, "max_y": 10.0 }
          }
        }
      }
    }
  ]
}"#,
    )
    .unwrap();
    file
}

fn write_invalid_primitive_scenario() -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        r#"{
  "schema_version": "0.1",
  "name": "Invalid Primitive",
  "description": "invalid primitive fixture",
  "scenarios": [
    {
      "mission": "waypoint-square",
      "profile": "invalid-primitive",
      "scenario": {
        "name": "invalid_primitive",
        "seed": 0,
        "agents": [
          {
            "id": "agent-0",
            "role": "scout",
            "health": "alive",
            "pose": { "x": 0.0, "y": 0.0, "z": 0.0 },
            "capabilities": [],
            "current_task": null,
            "battery": 100.0,
            "comms_range": 1000.0,
            "generation": 1,
            "speed": 0.0,
            "max_range": 1000.0,
            "battery_drain_rate": 0.0
          }
        ],
        "tasks": [],
        "ground_nodes": [],
        "base_station": null
      },
      "run_config": {
        "max_ticks": 50,
        "primitive_mission": {
          "kind": "waypoint_square",
          "altitude_m": 0.0,
          "side_m": -1.0
        }
      }
    }
  ]
}"#,
    )
    .unwrap();
    file
}

#[test]
fn preflight_failure_exits_nonzero_with_rule_ids() {
    let scenario = write_preflight_geofence_violation_scenario();
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-0",
    ]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("geofence.waypoint_outside"));
}

#[test]
fn invalid_primitive_params_exit_with_validation_code() {
    let scenario = write_invalid_primitive_scenario();
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-0",
    ]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("run_config.primitive_mission.altitude_m"));
    assert!(stderr.contains("run_config.primitive_mission.side_m"));
}

#[test]
fn valid_scenario_passes_preflight_and_succeeds() {
    let scenario = write_sitl_scenario();
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-0",
    ]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("preflight_safety: passed"));
}

#[test]
fn safety_report_written_when_output_dir_requested() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let output_dir = tempfile::tempdir().unwrap();
    let output = run_sitl_supervisor(&[
        "--dry-run",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--config",
        config.path().to_str().unwrap(),
        "--output-dir",
        output_dir.path().to_str().unwrap(),
        "--run-id",
        "m71-preflight",
    ]);

    assert!(output.status.success());
    let report_path = output_dir
        .path()
        .join("m71-preflight")
        .join("safety_validation_report.v1.json");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(report_path).unwrap()).unwrap();
    assert_eq!(json["passed"], true);
    assert!(json["violations"].as_array().unwrap().is_empty());
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
    assert_eq!(json["safety_report"]["passed"], true);
    assert_eq!(json["start_waypoint"]["edge_id"], "road-n0-n1");
    assert_eq!(json["end_waypoint"]["edge_id"], "road-n3-n0");
    assert_eq!(
        json["mavlink_common_plan"]["schema_version"],
        "mavlink_common_plan.v1"
    );
    assert_eq!(
        json["mavlink_common_plan"]["backend_profile"],
        "mavlink_common_generic"
    );
    assert_eq!(
        json["mavlink_common_plan"]["compatibility"]["profile"],
        "mavlink_common_generic"
    );
    assert!(
        json["mavlink_common_plan"]["compatibility"]["command_results"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert!(json["mavlink_common_plan"]["command_ir_hash"]
        .as_str()
        .is_some_and(|value| value.len() == 64));
    assert_eq!(
        json["mavlink_common_plan"]["command_prelude"][0]["command"],
        "MAV_CMD_COMPONENT_ARM_DISARM"
    );
    assert_eq!(
        json["mavlink_common_plan"]["command_prelude"][1]["command"],
        "MAV_CMD_NAV_TAKEOFF"
    );
    assert_eq!(
        json["mavlink_common_plan"]["mission_items"][0]["command"],
        "MAV_CMD_NAV_WAYPOINT"
    );
    assert!(json["mavlink_common_plan"]["expected_acks"]
        .as_array()
        .is_some_and(|acks| !acks.is_empty()));
    assert_eq!(
        json["mavlink_common_plan"]["validation_result"]["passed"],
        true
    );
}

fn assert_dry_run_artifact_valid(output_dir: &std::path::Path) {
    let report = swarm_examples::artifact_validator::validate_artifact_pack(
        &swarm_examples::artifact_validator::ArtifactPackPaths::from_output_dir(output_dir),
        swarm_examples::artifact_validator::ArtifactValidationOptions {
            mode: swarm_examples::artifact_validator::ArtifactValidationMode::DryRun,
            strict: true,
            ..Default::default()
        },
    );
    assert!(report.passed, "{:?}", report.violations);
}

fn run_public_dry_run_artifact(path: &str) -> (tempfile::TempDir, serde_json::Value) {
    let scenario = public_scenario(path);
    let output_dir = tempfile::tempdir().unwrap();
    let artifact = output_dir.path().join("sitl_dry_run_artifact.v1.json");
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        &scenario,
        "--agent-id",
        "agent-0",
        "--dry-run-artifact",
        artifact.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "dry-run failed for {path}: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&artifact).unwrap()).unwrap();
    assert_dry_run_artifact_valid(output_dir.path());
    (output_dir, json)
}

#[test]
fn urban_geo_perimeter_dry_run_uses_wgs84_node_coordinates() {
    let (_output_dir, json) = run_public_dry_run_artifact("scenarios/urban.geo-perimeter.json");

    assert_eq!(json["mission"], "urban-patrol");
    assert_eq!(json["profile"], "geo-perimeter");
    assert_eq!(json["coordinate_mode"], "wgs84_node_geo");
    assert_eq!(json["command_ir_summary"]["coordinate_frame"], "wgs84");
    assert!(json["start_waypoint"]["geo"].is_object());
    assert_eq!(json["start_waypoint"]["geo"]["lat_deg"], 47.397742);
    assert_eq!(json["start_waypoint"]["geo"]["lon_deg"], 8.545859);
    assert_eq!(
        json["mavlink_common_plan"]["mission_items"][0]["lat_e7"],
        473977420
    );
    assert_eq!(
        json["mavlink_common_plan"]["mission_items"][0]["lon_e7"],
        85458590
    );
    assert_eq!(
        json["mavlink_common_plan"]["mission_items"][0]["relative_alt_m"],
        5.0
    );
}

#[test]
fn urban_local_with_origin_dry_run_remains_unchanged() {
    let (_output_dir, json) = run_public_dry_run_artifact("scenarios/urban.patrol.json");

    assert_eq!(json["mission"], "urban-patrol");
    assert_eq!(json["coordinate_mode"], "local_with_origin");
    assert_eq!(json["command_ir_summary"]["coordinate_frame"], "local_enu");
    assert!(json["start_waypoint"]["geo"].is_null());
    assert_eq!(json["start_waypoint"]["x"], 20.0);
    assert_eq!(
        json["mavlink_common_plan"]["mission_items"][0]["lat_e7"],
        473977420
    );
    assert_eq!(
        json["mavlink_common_plan"]["mission_items"][0]["lon_e7"],
        85458594
    );
}

#[test]
fn urban_geo_search_artifact_records_mock_perception_metadata() {
    let (_output_dir, json) = run_public_dry_run_artifact("scenarios/urban.geo-search-bus.json");

    assert_eq!(json["mission"], "urban-search");
    assert_eq!(json["profile"], "geo-search-bus");
    assert_eq!(json["coordinate_mode"], "wgs84_node_geo");
    assert_eq!(json["urban_mission_template"], "search_until_target");
    assert_eq!(json["urban_mock_perception"]["detector_seed"], 8402);
    assert_eq!(json["urban_mock_perception"]["detection_range_m"], 4.0);
    assert_eq!(json["urban_mock_perception"]["detection_probability"], 1.0);
    assert_eq!(json["urban_mock_perception"]["false_positive_rate"], 0.0);
    assert_eq!(json["urban_mock_perception"]["target_count"], 1);
}

#[test]
fn urban_geo_block_loop_mavlink_plan_contains_route_metadata() {
    let (_output_dir, json) = run_public_dry_run_artifact("scenarios/urban.geo-block-loop.json");

    assert_eq!(json["mission"], "urban-patrol");
    assert_eq!(json["profile"], "geo-block-loop");
    assert_eq!(json["coordinate_mode"], "wgs84_node_geo");
    assert_eq!(json["urban_mission_template"], "block_loop");
    assert_eq!(json["urban_blocked_route_policy"], "wait");
    assert_eq!(json["segment_count"], 4);
    assert_eq!(json["waypoint_count"], 4);
    assert_eq!(json["waypoints"].as_array().unwrap().len(), 4);
    assert_eq!(
        json["mavlink_common_plan"]["mission_items"]
            .as_array()
            .unwrap()
            .len(),
        json["waypoints"].as_array().unwrap().len()
    );
    assert_eq!(json["waypoints"][0]["edge_id"], "road-n0-n1");
    assert_eq!(json["waypoints"][3]["edge_id"], "road-n3-n0");
}

#[test]
fn primitive_canonical_dry_run_artifacts_compile_to_mavlink_plans() {
    for (scenario, mission, expected_body_kind, expected_waypoints) in [
        (
            "scenarios/primitive.takeoff-hold-land.json",
            "takeoff-hold-land",
            "hold",
            1,
        ),
        ("scenarios/primitive.orbit.json", "orbit", "orbit", 36),
        (
            "scenarios/primitive.square.json",
            "waypoint-square",
            "follow_route",
            5,
        ),
    ] {
        let scenario = public_scenario(scenario);
        let output_dir = tempfile::tempdir().unwrap();
        let artifact = output_dir.path().join("sitl_dry_run_artifact.v1.json");
        let output = run_sitl_agent(&[
            "--dry-run",
            "--scenario",
            &scenario,
            "--agent-id",
            "agent-0",
            "--dry-run-artifact",
            artifact.to_str().unwrap(),
            "--mavlink-profile",
            "px4",
        ]);

        assert!(
            output.status.success(),
            "dry-run failed for {mission}: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&artifact).unwrap()).unwrap();
        assert_eq!(json["mission"], mission);
        assert_eq!(json["safety_report"]["passed"], true);
        assert!(json["command_ir_summary"].is_object());
        assert_eq!(
            json["command_ir_summary"]["expected_terminal_state"],
            "landed"
        );
        assert_eq!(
            json["command_ir_summary"]["timeout_policy"]["on_timeout"],
            "abort"
        );
        assert_eq!(
            json["command_ir_summary"]["commands_by_kind"][expected_body_kind],
            1
        );
        assert_eq!(json["mavlink_common_plan"]["backend_profile"], "px4");
        assert!(json["mavlink_common_plan"]["expected_acks"]
            .as_array()
            .is_some_and(|acks| !acks.is_empty()));
        assert!(json["mavlink_common_plan"]["telemetry_milestones"]
            .as_array()
            .is_some_and(|milestones| !milestones.is_empty()));
        assert_eq!(
            json["mavlink_common_plan"]["mission_items"]
                .as_array()
                .unwrap()
                .len(),
            expected_waypoints
        );

        let report = swarm_examples::artifact_validator::validate_artifact_pack(
            &swarm_examples::artifact_validator::ArtifactPackPaths::from_output_dir(
                output_dir.path(),
            ),
            swarm_examples::artifact_validator::ArtifactValidationOptions {
                mode: swarm_examples::artifact_validator::ArtifactValidationMode::DryRun,
                strict: true,
                ..Default::default()
            },
        );
        assert!(report.passed, "{:?}", report.violations);
    }
}

#[test]
fn dry_run_artifact_can_select_px4_mavlink_profile() {
    let scenario = public_scenario("scenarios/urban.patrol.json");
    let report_dir = tempfile::tempdir().unwrap();
    let artifact = report_dir.path().join("urban-px4-dry-run.json");
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        &scenario,
        "--agent-id",
        "agent-0",
        "--dry-run-artifact",
        artifact.to_str().unwrap(),
        "--mavlink-profile",
        "px4",
    ]);

    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&artifact).unwrap()).unwrap();
    assert_eq!(json["mavlink_common_plan"]["backend_profile"], "px4");
    assert_eq!(
        json["mavlink_common_plan"]["compatibility"]["profile"],
        "px4"
    );
    assert_eq!(
        json["mavlink_common_plan"]["compatibility"]["overall_classification"],
        "supported_with_caveats"
    );
    assert!(json["mavlink_common_plan"]["compatibility"]["caveats"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str().unwrap().contains("PX4")));
}

#[test]
fn dry_run_artifact_can_select_ardupilot_mavlink_profile() {
    let scenario = public_scenario("scenarios/primitive.takeoff-hold-land.json");
    let report_dir = tempfile::tempdir().unwrap();
    let artifact = report_dir.path().join("primitive-ardupilot-dry-run.json");
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        &scenario,
        "--agent-id",
        "agent-0",
        "--dry-run-artifact",
        artifact.to_str().unwrap(),
        "--mavlink-profile",
        "ardupilot",
    ]);

    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&artifact).unwrap()).unwrap();
    assert_eq!(json["mavlink_common_plan"]["backend_profile"], "ardupilot");
    assert_eq!(
        json["mavlink_common_plan"]["compatibility"]["profile"],
        "ardupilot"
    );
    assert_eq!(
        json["command_ir_summary"]["timeout_policy"]["on_timeout"],
        "abort"
    );
    assert_eq!(
        json["command_ir_summary"]["expected_terminal_state"],
        "landed"
    );
    assert!(json["mavlink_common_plan"]["compatibility"]["caveats"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str().unwrap().contains("ArduPilot")));
}

#[test]
fn dry_run_rejects_invalid_mavlink_profile() {
    let scenario = write_sitl_scenario();
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-0",
        "--mavlink-profile",
        "nope",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid MAVLink capability profile 'nope'"));
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
