use std::process::Command;

fn sitl_agent_binary() -> &'static str {
    env!("CARGO_BIN_EXE_sitl_agent")
}

fn sitl_supervisor_binary() -> &'static str {
    env!("CARGO_BIN_EXE_sitl_supervisor")
}

fn write_sitl_scenario() -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        r#"{
  "schema_version": "0.1",
  "name": "SITL Waypoints",
  "description": "portable sitl_agent test fixture",
  "scenarios": [
    {
      "mission": "sitl",
      "profile": "waypoints",
      "scenario": {
        "name": "sitl_waypoints_test",
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
            "id": "wp-0",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 10.0, "y": 20.0, "z": 3.5 },
            "grid_cell": null
          },
          {
            "id": "wp-1",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 30.0, "y": 40.0 },
            "grid_cell": null
          }
        ],
        "ground_nodes": [],
        "base_station": null
      },
      "run_config": {
        "max_ticks": 50
      }
    }
  ]
}"#,
    )
    .unwrap();
    file
}

fn write_multi_agent_sitl_scenario() -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        r#"{
  "schema_version": "0.1",
  "name": "Multi SITL Waypoints",
  "description": "portable multi-agent sitl_agent test fixture",
  "scenarios": [
    {
      "mission": "sitl",
      "profile": "waypoints",
      "scenario": {
        "name": "multi_sitl_waypoints_test",
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
          },
          {
            "id": "agent-1",
            "role": "scout",
            "health": "alive",
            "pose": { "x": 1.0, "y": 1.0 },
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
            "id": "wp-0",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 10.0, "y": 20.0, "z": 3.5 },
            "grid_cell": null
          },
          {
            "id": "wp-1",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 30.0, "y": 40.0, "z": 4.5 },
            "grid_cell": null
          }
        ],
        "ground_nodes": [],
        "base_station": null
      },
      "run_config": {
        "max_ticks": 50
      }
    }
  ]
}"#,
    )
    .unwrap();
    file
}

fn write_multi_agent_config(duplicate: bool) -> tempfile::NamedTempFile {
    write_multi_agent_config_with_connections(
        duplicate,
        "udp:127.0.0.1:14550",
        "udp:127.0.0.1:14560",
    )
}

fn write_multi_agent_config_with_connections(
    duplicate: bool,
    agent_0_connection: &str,
    agent_1_connection: &str,
) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    let agent_1_task = if duplicate { "wp-0" } else { "wp-1" };
    std::fs::write(
        file.path(),
        format!(
            r#"{{
  "schema_version": "multi_sitl.v1",
  "agents": [
    {{
      "agent_id": "agent-0",
      "system_id": 1,
      "component_id": 1,
      "connection_string": "{agent_0_connection}",
      "start_delay_ms": 0,
      "lifecycle": "upload_only",
      "task_ids": ["wp-0"]
    }},
    {{
      "agent_id": "agent-1",
      "system_id": 2,
      "component_id": 1,
      "connection_string": "{agent_1_connection}",
      "start_delay_ms": 0,
      "lifecycle": "execute",
      "task_ids": ["{agent_1_task}"]
    }}
  ]
}}"#
        ),
    )
    .unwrap();
    file
}

fn write_safety_config(json: &str) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), json).unwrap();
    file
}

fn run_sitl_agent(args: &[&str]) -> std::process::Output {
    Command::new(sitl_agent_binary())
        .args(args)
        .output()
        .expect("failed to execute sitl_agent")
}

fn run_sitl_supervisor(args: &[&str]) -> std::process::Output {
    Command::new(sitl_supervisor_binary())
        .args(args)
        .output()
        .expect("failed to execute sitl_supervisor")
}

fn run_sitl_supervisor_in_dir(args: &[&str], dir: &std::path::Path) -> std::process::Output {
    Command::new(sitl_supervisor_binary())
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to execute sitl_supervisor")
}

fn public_scenario(path: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn dry_run_outputs_mission_upload_plan() {
    let scenario = write_sitl_scenario();
    let scenario = scenario.path().to_str().unwrap();
    let output = run_sitl_agent(&["--dry-run", "--scenario", scenario, "--agent-id", "agent-0"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "dry-run failed: {stderr}");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mode: dry-run"));
    assert!(stdout.contains("agent_id: agent-0"));
    assert!(stdout.contains("suite_name: SITL Waypoints"));
    assert!(stdout.contains("scenario_name: sitl_waypoints_test"));
    assert!(stdout.contains("coordinate_frame: local_simulation"));
    assert!(stdout.contains("altitude_source: pose.z"));
    assert!(stdout.contains("seq=0 task_id=wp-0 x=10.000 y=20.000 z=3.500"));
    assert!(stdout.contains("seq=1 task_id=wp-1 x=30.000 y=40.000 z=0.000"));
}

#[test]
fn public_px4_golden_fixture_has_explicit_altitudes() {
    let scenario = public_scenario("scenarios/sitl.px4-golden.json");
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        &scenario,
        "--agent-id",
        "agent-0",
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "dry-run failed: {stderr}");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("scenario_name: sitl_px4_golden_0"));
    assert!(stdout.contains("seq=0 task_id=wp-0 x=10.000 y=15.000 z=5.000"));
    assert!(stdout.contains("seq=1 task_id=wp-1 x=25.000 y=25.000 z=6.000"));
    assert!(stdout.contains("seq=2 task_id=wp-2 x=40.000 y=10.000 z=5.000"));
}

#[test]
fn public_multi_agent_fixture_builds_manifest() {
    let scenario = public_scenario("scenarios/sitl.multi-agent.json");
    let config = public_scenario("scenarios/sitl.multi-agent.config.json");
    let output = run_sitl_supervisor(&["--dry-run", "--scenario", &scenario, "--config", &config]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "supervisor dry-run failed: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""schema_version": "multi_sitl_manifest.v1""#));
    assert!(stdout.contains(r#""agents_count": 2"#));
    assert!(stdout.contains(r#""agent_id": "agent-0""#));
    assert!(stdout.contains(r#""agent_id": "agent-1""#));
    assert!(stdout.contains(r#""task_ids": ["#));
    assert!(stdout.contains(r#""wp-0""#));
    assert!(stdout.contains(r#""wp-3""#));
    assert!(stdout.contains(r#""unassigned_pose_tasks": []"#));
}

#[test]
fn public_multi_agent_fixture_can_run_mock_supervisor() {
    let scenario = public_scenario("scenarios/sitl.multi-agent.json");
    let config = public_scenario("scenarios/sitl.multi-agent.config.json");
    let output = run_sitl_supervisor(&["--mock", "--scenario", &scenario, "--config", &config]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "supervisor mock failed: {stderr}");
    assert!(stderr.contains("agent=agent-0"));
    assert!(stderr.contains("agent=agent-1"));
    assert!(stderr.contains("waypoints sent=2"));
}

#[test]
fn multi_agent_dry_run_output_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-0",
        "--multi-agent-config",
        config.path().to_str().unwrap(),
    ]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("agent_id: agent-0"));
    assert!(stdout.contains("seq=0 task_id=wp-0"));
    assert!(!stdout.contains("task_id=wp-1"));
    assert!(stderr.contains("Multi-agent SITL"));
    assert!(stderr.contains("connection=udp:127.0.0.1:14550"));
}

#[test]
fn multi_agent_duplicate_ownership_rejected_before_upload_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(true);
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-0",
        "--multi-agent-config",
        config.path().to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("duplicate ownership"));
    assert!(stderr.contains("task_id=wp-0"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
#[cfg(not(feature = "mavlink-transport"))]
fn multi_agent_safety_subset_allows_safe_agent_when_other_agent_is_unsafe() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let safety = write_safety_config(
        r#"{
  "geofence": { "min_x": 0.0, "max_x": 20.0, "min_y": 0.0, "max_y": 25.0 }
}"#,
    );
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14550",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-0",
        "--multi-agent-config",
        config.path().to_str().unwrap(),
        "--safety-config",
        safety.path().to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("feature missing"));
    assert!(!stderr.contains("safety validation failed"));
    assert!(!stderr.contains("task_id=wp-1"));
}

#[test]
fn multi_agent_safety_subset_rejects_selected_agent_unsafe_task() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let safety = write_safety_config(
        r#"{
  "geofence": { "min_x": 0.0, "max_x": 20.0, "min_y": 0.0, "max_y": 25.0 }
}"#,
    );
    let output = run_sitl_agent(&[
        "--connection",
        "udp:127.0.0.1:14560",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-1",
        "--multi-agent-config",
        config.path().to_str().unwrap(),
        "--safety-config",
        safety.path().to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("safety validation failed"));
    assert!(stderr.contains("rule_id=outside_geofence"));
    assert!(stderr.contains("task_id=wp-1"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
fn multi_agent_config_connection_used_when_cli_connection_missing_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let output = run_sitl_agent(&[
        "--dry-run",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-1",
        "--multi-agent-config",
        config.path().to_str().unwrap(),
    ]);

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("connection=udp:127.0.0.1:14560"));
    assert!(stderr.contains("lifecycle=Execute"));
}

#[test]
fn multi_agent_config_cli_connection_override_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let output = run_sitl_agent(&[
        "--connection",
        "bad",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--agent-id",
        "agent-0",
        "--multi-agent-config",
        config.path().to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bad connection string 'bad'"));
    assert!(!stderr.contains("feature missing"));
}

#[test]
fn multi_agent_sitl_supervisor_dry_run_manifest_stdout_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let output = run_sitl_supervisor(&[
        "--dry-run",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--config",
        config.path().to_str().unwrap(),
    ]);

    assert!(output.status.success());
    let manifest: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(manifest["schema_version"], "multi_sitl_manifest.v1");
    assert_eq!(manifest["agents_count"], 2);
    assert_eq!(manifest["agents"][0]["task_ids"][0], "wp-0");
    assert_eq!(manifest["agents"][1]["task_ids"][0], "wp-1");
}

#[test]
fn multi_agent_sitl_supervisor_dry_run_manifest_file_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let dir = tempfile::tempdir().unwrap();
    let manifest_path = dir.path().join("multi").join("manifest.json");
    let output = run_sitl_supervisor(&[
        "--dry-run",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--config",
        config.path().to_str().unwrap(),
        "--manifest",
        manifest_path.to_str().unwrap(),
    ]);

    assert!(output.status.success());
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["ownership_summary"]["assigned_task_count"], 2);
}

#[test]
fn multi_agent_sitl_supervisor_manifest_file_without_parent_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let dir = tempfile::tempdir().unwrap();
    let output = run_sitl_supervisor_in_dir(
        &[
            "--dry-run",
            "--scenario",
            scenario.path().to_str().unwrap(),
            "--config",
            config.path().to_str().unwrap(),
            "--manifest",
            "manifest.json",
        ],
        dir.path(),
    );

    assert!(output.status.success());
    assert!(dir.path().join("manifest.json").exists());
}

#[test]
fn multi_agent_sitl_supervisor_mock_runs_two_agents_with_distinct_subsets_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let output = run_sitl_supervisor(&[
        "--mock",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--config",
        config.path().to_str().unwrap(),
    ]);

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("agent=agent-0"));
    assert!(stderr.contains("agent=agent-1"));
    assert!(stderr.contains("task_id=wp-0"));
    assert!(stderr.contains("task_id=wp-1"));
}

#[test]
fn multi_agent_sitl_supervisor_duplicate_ownership_rejected_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(true);
    let output = run_sitl_supervisor(&[
        "--dry-run",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--config",
        config.path().to_str().unwrap(),
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("duplicate ownership"));
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
