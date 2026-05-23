use std::process::Command;

fn run_replay(args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--bin", "replay", "--"]);
    cmd.args(args);
    cmd.output().expect("Failed to execute replay")
}

#[test]
fn replay_cli_summary_outputs_ticks() {
    let log_path = "/tmp/replay_test_dir/coverage_with_failure_0.replay.json";
    let output = run_replay(&["--log", log_path, "--summary"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "replay --summary failed: {}", stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Total ticks:"));
    assert!(stdout.contains("Events:"));
}

#[test]
fn replay_cli_tick_outputs_ascii() {
    let log_path = "/tmp/replay_test_dir/coverage_with_failure_0.replay.json";
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
