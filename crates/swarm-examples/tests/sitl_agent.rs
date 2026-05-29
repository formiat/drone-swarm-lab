use std::process::Command;

fn sitl_agent_binary() -> &'static str {
    env!("CARGO_BIN_EXE_sitl_agent")
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

fn run_sitl_agent(args: &[&str]) -> std::process::Output {
    Command::new(sitl_agent_binary())
        .args(args)
        .output()
        .expect("failed to execute sitl_agent")
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
