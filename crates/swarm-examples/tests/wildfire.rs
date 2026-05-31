use std::process::Command;
use tempfile::NamedTempFile;

#[test]
fn strategy_comparison_wildfire_smoke() {
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "swarm-examples",
            "--bin",
            "strategy_comparison",
            "--",
            "--smoke",
            "--mission",
            "wildfire",
        ])
        .output()
        .expect("failed to execute strategy_comparison");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("wildfire"),
        "Expected wildfire mission in output. stdout:\n{}",
        stdout
    );
    assert!(
        stdout.contains("small-static"),
        "Expected small-static profile. stdout:\n{}",
        stdout
    );
    assert!(
        output.status.success(),
        "Expected exit code 0. stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn wildfire_small_static_reports_completion_separately_from_success() {
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "swarm-examples",
            "--bin",
            "strategy_comparison",
            "--",
            "--smoke",
            "--mission",
            "wildfire",
        ])
        .output()
        .expect("failed to execute strategy_comparison");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "Expected exit code 0. stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut saw_small_static = false;
    let mut saw_success_completion_mismatch = false;
    for line in stdout
        .lines()
        .filter(|line| line.contains("wildfire/small-static"))
    {
        saw_small_static = true;
        let columns: Vec<_> = line.split('|').map(str::trim).collect();
        let success = columns
            .get(6)
            .expect("success column should be present in wildfire row");
        let completion = columns
            .get(7)
            .expect("completion column should be present in wildfire row");
        assert_eq!(
            *completion, "1.000",
            "Expected task completion 1.000 for small-static, got line: {line}"
        );
        saw_success_completion_mismatch |= success != completion;
    }
    assert!(
        saw_small_static,
        "Expected small-static profile in stdout:\n{stdout}"
    );
    assert!(
        saw_success_completion_mismatch,
        "Expected at least one small-static row where success differs from completion"
    );
}

#[test]
fn regression_runner_wildfire_suite() {
    // Create a temporary baseline with one wildfire suite
    let baseline_json = r#"{
      "version": "1.0",
      "created_at": "2025-05-26T12:00:00Z",
      "commit": "test",
      "results": {
        "wildfire_small_static": {
          "total_runs": 1,
          "success_rate": 1.0,
          "avg_hazard_zones_mapped": 2.0,
          "avg_detection_ticks": 0.0,
          "avg_reallocation_ticks": 0.0,
          "avg_messages_attempted": 0.0,
          "avg_messages_dropped": 0.0,
          "avg_tasks_injected": 0.0,
          "avg_tasks_expired": 0.0,
          "avg_conflicting_assignments": 0.0,
          "avg_network_availability": 0.0,
          "avg_relay_reallocation_ticks": 0.0,
          "avg_avg_hop_count": 0.0,
          "avg_disconnected_agents_max": 0.0,
          "avg_coverage_progress": 0.0,
          "avg_bytes_sent": 0.0,
          "avg_stale_state_age_ticks": 0.0,
          "avg_battery_margin_min": 0.0,
          "avg_battery_margin_avg": 0.0,
          "avg_task_completion_rate": 0.0,
          "avg_time_to_find": 0.0,
          "avg_probability_of_detection": 0.0,
          "avg_targets_found": 0.0,
          "avg_safety_violations": 0.0,
          "avg_belief_entropy_final": 0.0,
          "avg_false_positive_rate": 0.0,
          "avg_confirmation_scans": 0.0,
          "convergence_ticks_p50": 0.0,
          "convergence_ticks_p95": 0.0,
          "convergence_ticks_max": 0.0,
          "avg_bundle_travel_distance": 0.0,
          "avg_edge_coverage_rate": 0.0,
          "avg_missed_edges": 0.0,
          "avg_revisit_count": 0.0,
          "avg_route_efficiency": 0.0,
          "avg_route_length": 0.0,
          "avg_wasted_travel": 0.0,
          "avg_return_reserve": 0.0,
          "avg_infeasible_routes": 0.0,
          "avg_priority_updates": 0.0,
          "avg_final_threat_level": 0.0
        }
      }
    }"#;

    let tmp_file = NamedTempFile::new().unwrap();
    let baseline_path = tmp_file.path().to_str().unwrap().to_owned();
    std::fs::write(&baseline_path, baseline_json).unwrap();

    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "swarm-examples",
            "--bin",
            "regression_runner",
            "--",
            "--compare-baseline",
            &baseline_path,
        ])
        .output()
        .expect("failed to execute regression_runner");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("overall_pass: true") || stdout.contains("overall_pass: false"),
        "Expected a regression report. stdout:\n{}",
        stdout
    );
}
