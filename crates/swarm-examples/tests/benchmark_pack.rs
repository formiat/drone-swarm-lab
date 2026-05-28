use std::process::Command;

fn run_strategy_comparison(args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "run",
        "-p",
        "swarm-examples",
        "--bin",
        "strategy_comparison",
        "--",
    ]);
    cmd.args(args);
    cmd.output().expect("Failed to execute strategy_comparison")
}

#[test]
fn strategy_comparison_smoke_creates_output_dir() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().to_str().unwrap();
    let output =
        run_strategy_comparison(&["--smoke", "--mission", "coverage", "--output-dir", dir]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "strategy_comparison --smoke failed: {}",
        stderr
    );
    assert!(tmp.path().exists(), "output dir not created");
}

#[test]
fn strategy_comparison_output_contains_manifest() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().to_str().unwrap();
    let output =
        run_strategy_comparison(&["--smoke", "--mission", "coverage", "--output-dir", dir]);
    assert!(output.status.success());
    let manifest_path = tmp.path().join("manifest.json");
    assert!(manifest_path.exists(), "manifest.json missing");
    let content = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(content.contains("git_commit"));
    assert!(content.contains("command_line"));
}

#[test]
fn strategy_comparison_output_contains_results() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().to_str().unwrap();
    let output =
        run_strategy_comparison(&["--smoke", "--mission", "coverage", "--output-dir", dir]);
    assert!(output.status.success());
    assert!(tmp.path().join("results.json").exists());
    assert!(tmp.path().join("results.csv").exists());
    assert!(tmp.path().join("table.md").exists());
}

#[test]
fn strategy_comparison_backward_compat_no_output_dir() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_path = tmp.path().join("bench_compat.json");
    let csv_path = tmp.path().join("bench_compat.csv");
    let json_str = json_path.to_str().unwrap();
    let csv_str = csv_path.to_str().unwrap();
    let output = run_strategy_comparison(&[
        "--smoke",
        "--mission",
        "coverage",
        "--json",
        json_str,
        "--csv",
        csv_str,
    ]);
    assert!(output.status.success());
    assert!(json_path.exists(), "JSON not written");
    assert!(csv_path.exists(), "CSV not written");
}

#[test]
fn strategy_comparison_quick_mode_is_default() {
    // No mode flag should default to quick (10 seeds), but --smoke is faster for test
    let output = run_strategy_comparison(&["--smoke", "--mission", "coverage"]);
    assert!(output.status.success());
}

#[test]
fn strategy_comparison_accepts_custom_seed_count() {
    let dir = "target/test-output/custom_seed_count";
    let _ = std::fs::remove_dir_all(dir);
    let output = run_strategy_comparison(&[
        "--seeds",
        "2",
        "--mission",
        "coverage",
        "--jobs",
        "2",
        "--output-dir",
        dir,
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "strategy_comparison --seeds failed: {}",
        stderr
    );

    let json_path = std::path::Path::new(dir).join("results.json");
    let json_content = std::fs::read_to_string(&json_path).unwrap();
    let report: serde_json::Value = serde_json::from_str(&json_content).unwrap();
    assert_eq!(report["rows"][0]["seed_range_start"], 0);
    assert_eq!(report["rows"][0]["seed_range_end"], 2);
    assert_eq!(report["rows"][0]["total_runs"], 2);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn strategy_comparison_report_flag_creates_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let report_path = tmp.path().join("bench_report_test.md");
    let report_str = report_path.to_str().unwrap();
    let output =
        run_strategy_comparison(&["--smoke", "--mission", "coverage", "--report", report_str]);
    assert!(output.status.success());
    assert!(report_path.exists(), "report file not created");
    let content = std::fs::read_to_string(&report_path).unwrap();
    assert!(content.contains("# Benchmark Report"));
    assert!(content.contains("## coverage"));
}

#[test]
fn strategy_comparison_report_contains_key_questions() {
    let tmp = tempfile::TempDir::new().unwrap();
    let report_path = tmp.path().join("bench_report_questions.md");
    let report_str = report_path.to_str().unwrap();
    let output =
        run_strategy_comparison(&["--smoke", "--mission", "coverage", "--report", report_str]);
    assert!(output.status.success());
    let content = std::fs::read_to_string(&report_path).unwrap();
    assert!(content.contains("Where does CBBA win?"));
    assert!(content.contains("Where does CBBA lose?"));
    assert!(content.contains("SAR v2 vs SAR v1"));
}

#[test]
fn strategy_comparison_creates_parent_dirs_for_explicit_outputs() {
    let root = std::path::Path::new("target/test-output/strategy_comparison_nested_outputs");
    let json_path = root.join("json/results.json");
    let csv_path = root.join("csv/results.csv");
    let report_path = root.join("reports/focused.md");
    let _ = std::fs::remove_dir_all(root);

    let json_arg = json_path.to_str().unwrap().to_owned();
    let csv_arg = csv_path.to_str().unwrap().to_owned();
    let report_arg = report_path.to_str().unwrap().to_owned();

    let output = run_strategy_comparison(&[
        "--smoke",
        "--mission",
        "coverage",
        "--json",
        json_arg.as_str(),
        "--csv",
        csv_arg.as_str(),
        "--report",
        report_arg.as_str(),
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "strategy_comparison nested output paths failed: {}",
        stderr
    );
    assert!(json_path.exists(), "JSON file not written");
    assert!(csv_path.exists(), "CSV file not written");
    assert!(report_path.exists(), "report file not written");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn strategy_comparison_mission_all_has_all_benchmark_id() {
    let dir = "target/test-output/mission_all_identity";
    let _ = std::fs::remove_dir_all(dir);
    let output = run_strategy_comparison(&[
        "--smoke",
        "--mission",
        "all",
        "--jobs",
        "4",
        "--output-dir",
        dir,
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "strategy_comparison --mission all failed: {}",
        stderr
    );

    // Read results.json
    let json_path = std::path::Path::new(dir).join("results.json");
    assert!(json_path.exists(), "results.json missing");
    let json_content = std::fs::read_to_string(&json_path).unwrap();
    let report: serde_json::Value = serde_json::from_str(&json_content).unwrap();

    let benchmark_run_id = report["benchmark_run_id"].as_str().unwrap();
    assert!(
        benchmark_run_id.contains("_all_"),
        "benchmark_run_id should contain '_all_', got: {}",
        benchmark_run_id
    );
    assert!(
        !benchmark_run_id.contains("coverage"),
        "benchmark_run_id should not contain mission name 'coverage', got: {}",
        benchmark_run_id
    );

    // Check rows have correct mission and mission-scoped profiles
    let rows = report["rows"].as_array().unwrap();
    assert!(!rows.is_empty(), "rows should not be empty");

    let mut missions_found = std::collections::HashSet::new();
    for row in rows {
        let mission = row["mission"].as_str().unwrap();
        let profile = row["profile"].as_str().unwrap();
        let run_id = row["run_id"].as_str().unwrap();

        missions_found.insert(mission.to_owned());

        // Profile should be mission-scoped
        assert!(
            profile.starts_with(&format!("{}/", mission)),
            "profile '{}' should be mission-scoped for mission '{}'",
            profile,
            mission
        );

        // run_id should contain the mission name
        assert!(
            run_id.contains(mission),
            "run_id '{}' should contain mission '{}'",
            run_id,
            mission
        );
    }

    assert!(
        missions_found.contains("sar"),
        "should have SAR rows, found: {:?}",
        missions_found
    );
    assert!(
        missions_found.contains("wildfire"),
        "should have wildfire rows, found: {:?}",
        missions_found
    );
    assert!(
        missions_found.contains("coverage"),
        "should have coverage rows, found: {:?}",
        missions_found
    );

    // Check CSV has mission-scoped profiles too
    let csv_path = std::path::Path::new(dir).join("results.csv");
    assert!(csv_path.exists(), "results.csv missing");
    let csv_content = std::fs::read_to_string(&csv_path).unwrap();
    assert!(
        csv_content.contains("sar/ideal"),
        "CSV should contain mission-scoped profile 'sar/ideal'"
    );
    assert!(
        csv_content.contains("wildfire/small-static"),
        "CSV should contain mission-scoped profile 'wildfire/small-static'"
    );

    let _ = std::fs::remove_dir_all(dir);
}
