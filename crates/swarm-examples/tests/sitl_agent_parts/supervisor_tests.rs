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

fn write_multi_agent_execute_config_with_connections(
    agent_0_connection: &str,
    agent_1_connection: &str,
) -> tempfile::NamedTempFile {
    let file = tempfile::NamedTempFile::new().unwrap();
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
      "lifecycle": "execute",
      "task_ids": ["wp-0"]
    }},
    {{
      "agent_id": "agent-1",
      "system_id": 2,
      "component_id": 1,
      "connection_string": "{agent_1_connection}",
      "start_delay_ms": 0,
      "lifecycle": "execute",
      "task_ids": ["wp-1"]
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

fn assert_sitl_supervisor_cli_error(args: &[&str], expected: &str) {
    assert_sitl_supervisor_cli_error_code(args, expected, 2);
}

fn assert_sitl_supervisor_cli_error_code(args: &[&str], expected: &str, expected_code: i32) {
    let output = run_sitl_supervisor(args);
    assert!(
        !output.status.success(),
        "sitl_supervisor unexpectedly succeeded for args: {args:?}"
    );
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "unexpected sitl_supervisor exit code for args: {args:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "expected stderr to contain '{expected}' for args {args:?}, got:\n{stderr}"
    );
    assert!(
        stderr.contains("usage: sitl_supervisor"),
        "expected usage text for args {args:?}, got:\n{stderr}"
    );
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
fn multi_agent_sitl_supervisor_mock_reallocates_after_agent_loss_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let dir = tempfile::tempdir().unwrap();
    let replay_log = dir.path().join("supervisor.sitl-log.json");
    let output = run_sitl_supervisor(&[
        "--mock",
        "--scenario",
        scenario.path().to_str().unwrap(),
        "--config",
        config.path().to_str().unwrap(),
        "--fail-agent",
        "agent-0",
        "--fail-after-ticks",
        "0",
        "--heartbeat-timeout-ticks",
        "1",
        "--max-ticks",
        "6",
        "--replay-log",
        replay_log.to_str().unwrap(),
    ]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "supervisor failure run failed: {stderr}"
    );
    assert!(stderr.contains("SUPERVISOR_METRICS"));
    assert!(stderr.contains("lost_agents=1"));
    assert!(stderr.contains("reassignment_count=1"));
    assert!(stderr.contains("tasks_recovered=wp-0"));
    assert!(stderr.contains("final_status=completed"));

    let log = swarm_examples::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
    let summary = swarm_examples::sitl_observability::summarize_sitl_event_log(&log);
    assert_eq!(summary.agent_lost, 1);
    assert_eq!(summary.task_released, 1);
    assert_eq!(summary.task_reassigned, 1);
    assert_eq!(summary.reallocation_completed, 1);
    assert_eq!(summary.tasks_recovered, 1);
    assert_eq!(summary.reallocation_latency_ticks, Some(0));
}

#[test]
fn multi_agent_sitl_supervisor_output_dir_layout_and_force_test() {
    let scenario = write_multi_agent_sitl_scenario();
    let config = write_multi_agent_config(false);
    let dir = tempfile::tempdir().unwrap();
    let output_dir = dir.path().join("runs");
    let output_dir = output_dir.to_string_lossy().into_owned();
    let scenario = scenario.path().to_str().unwrap().to_owned();
    let config = config.path().to_str().unwrap().to_owned();

    let args = [
        "--mock",
        "--scenario",
        &scenario,
        "--config",
        &config,
        "--output-dir",
        &output_dir,
        "--run-id",
        "run-m60",
    ];

    let output = run_sitl_supervisor(&args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "supervisor output-dir run failed: {stderr}"
    );

    let run_dir = std::path::Path::new(&output_dir).join("run-m60");
    let manifest_path = run_dir.join("manifest.json");
    let replay_log = run_dir.join("events.sitl-log.json");
    let summary_path = run_dir.join("replay-summary.txt");
    assert!(manifest_path.exists());
    assert!(replay_log.exists());
    assert!(summary_path.exists());
    let log = swarm_examples::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
    assert_eq!(log.run_id, "run-m60");
    let summary = std::fs::read_to_string(&summary_path).unwrap();
    assert!(summary.contains("SITL run: run-m60"));
    assert!(summary.contains("Multi-agent:"));

    let duplicate = run_sitl_supervisor(&args);
    assert!(!duplicate.status.success());
    assert_eq!(duplicate.status.code(), Some(40));
    let duplicate_stderr = String::from_utf8_lossy(&duplicate.stderr);
    assert!(duplicate_stderr.contains("output path already exists"));
    assert!(!duplicate_stderr.contains("usage: sitl_supervisor"));

    let forced = run_sitl_supervisor(&[
        "--mock",
        "--scenario",
        &scenario,
        "--config",
        &config,
        "--output-dir",
        &output_dir,
        "--run-id",
        "run-m60",
        "--force",
    ]);
    let forced_stderr = String::from_utf8_lossy(&forced.stderr);
    assert!(
        forced.status.success(),
        "supervisor forced output-dir run failed: {forced_stderr}"
    );
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
