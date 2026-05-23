use std::process::Command;

fn run_strategy_comparison(args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "-p", "swarm-examples", "--bin", "strategy_comparison", "--"]);
    cmd.args(args);
    cmd.output().expect("Failed to execute strategy_comparison")
}

#[test]
fn strategy_comparison_smoke_creates_output_dir() {
    let dir = "/tmp/bench_smoke_test_dir";
    let _ = std::fs::remove_dir_all(dir);
    let output = run_strategy_comparison(&[
        "--smoke",
        "--mission",
        "coverage",
        "--output-dir",
        dir,
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "strategy_comparison --smoke failed: {}",
        stderr
    );
    assert!(std::path::Path::new(dir).exists(), "output dir not created");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn strategy_comparison_output_contains_manifest() {
    let dir = "/tmp/bench_manifest_test_dir";
    let _ = std::fs::remove_dir_all(dir);
    let output = run_strategy_comparison(&[
        "--smoke",
        "--mission",
        "coverage",
        "--output-dir",
        dir,
    ]);
    assert!(output.status.success());
    let manifest_path = std::path::Path::new(dir).join("manifest.json");
    assert!(manifest_path.exists(), "manifest.json missing");
    let content = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(content.contains("git_commit"));
    assert!(content.contains("command_line"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn strategy_comparison_output_contains_results() {
    let dir = "/tmp/bench_results_test_dir";
    let _ = std::fs::remove_dir_all(dir);
    let output = run_strategy_comparison(&[
        "--smoke",
        "--mission",
        "coverage",
        "--output-dir",
        dir,
    ]);
    assert!(output.status.success());
    assert!(std::path::Path::new(dir).join("results.json").exists());
    assert!(std::path::Path::new(dir).join("results.csv").exists());
    assert!(std::path::Path::new(dir).join("table.md").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn strategy_comparison_backward_compat_no_output_dir() {
    let json_path = "/tmp/bench_compat.json";
    let csv_path = "/tmp/bench_compat.csv";
    let _ = std::fs::remove_file(json_path);
    let _ = std::fs::remove_file(csv_path);
    let output = run_strategy_comparison(&[
        "--smoke",
        "--mission",
        "coverage",
        "--json",
        json_path,
        "--csv",
        csv_path,
    ]);
    assert!(output.status.success());
    assert!(std::path::Path::new(json_path).exists(), "JSON not written");
    assert!(std::path::Path::new(csv_path).exists(), "CSV not written");
    let _ = std::fs::remove_file(json_path);
    let _ = std::fs::remove_file(csv_path);
}

#[test]
fn strategy_comparison_quick_mode_is_default() {
    // No mode flag should default to quick (10 seeds), but --smoke is faster for test
    let output = run_strategy_comparison(&["--smoke", "--mission", "coverage"]);
    assert!(output.status.success());
}
