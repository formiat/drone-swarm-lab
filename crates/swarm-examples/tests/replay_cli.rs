use std::process::Command;

fn run_replay(args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--bin", "replay", "--"]);
    cmd.args(args);
    cmd.output().expect("Failed to execute replay")
}

fn create_test_replay_log(path: &str) {
    use swarm_replay::{Event, EventLogBuilder};
    use swarm_types::{AgentId, Pose};

    let mut builder = EventLogBuilder::new("test-run", 42, "test_scenario");
    builder.push(Event::TickStart { tick: 0 });
    builder.push(Event::PoseUpdated {
        agent_id: AgentId::from("agent-0".to_owned()),
        pose: Pose { x: 0.0, y: 0.0 , ..Default::default()},
        tick: 0,
    });
    builder.push(Event::TickStart { tick: 50 });
    builder.push(Event::PoseUpdated {
        agent_id: AgentId::from("agent-0".to_owned()),
        pose: Pose { x: 10.0, y: 10.0 , ..Default::default()},
        tick: 50,
    });
    builder.push(Event::TickStart { tick: 100 });
    let log = builder.build();
    let json = serde_json::to_string_pretty(&log).unwrap();
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, json).unwrap();
}

#[test]
fn replay_cli_summary_outputs_ticks() {
    let log_path = "/tmp/replay_test_dir/coverage_with_failure_0.replay.json";
    create_test_replay_log(log_path);
    let output = run_replay(&["--log", log_path, "--summary"]);
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
    let log_path = "/tmp/replay_test_dir/coverage_with_failure_0.replay.json";
    create_test_replay_log(log_path);
    let output = run_replay(&["--log", log_path, "--tick", "50"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Snapshot at tick 50"));
}

#[test]
fn replay_cli_invalid_log_exits_error() {
    let output = run_replay(&["--log", "/tmp/nonexistent_replay.json", "--summary"]);
    assert!(!output.status.success());
}
