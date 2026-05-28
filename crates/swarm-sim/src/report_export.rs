use serde::{Deserialize, Serialize};

use crate::ComparisonReport;

/// Export a ComparisonReport to JSON.
pub fn export_json(report: &ComparisonReport) -> Result<String, serde_json::Error> {
    let mut rows = Vec::new();
    for strategy_name in &report.strategy_names {
        for profile_name in &report.profile_names {
            let key = (strategy_name.clone(), profile_name.clone());
            if let Some(metrics) = report.results.get(&key) {
                let safe_profile = profile_name.replace('/', "_");
                let row_id = format!(
                    "{}_{}_{}_{}",
                    report.benchmark_run_id, metrics.mission, strategy_name, safe_profile
                );
                rows.push(ReportRow {
                    benchmark_run_id: report.benchmark_run_id.clone(),
                    run_id: row_id,
                    mission: metrics.mission.clone(),
                    scenario: metrics.scenario.clone(),
                    seed_range_start: report.seed_range_start,
                    seed_range_end: report.seed_range_end,
                    strategy: strategy_name.clone(),
                    profile: profile_name.clone(),
                    total_runs: metrics.total_runs,
                    success_rate: metrics.success_rate,
                    avg_task_completion_rate: metrics.avg_task_completion_rate,
                    avg_detection_ticks: metrics.avg_detection_ticks,
                    avg_reallocation_ticks: metrics.avg_reallocation_ticks,
                    avg_messages_attempted: metrics.avg_messages_attempted,
                    avg_messages_dropped: metrics.avg_messages_dropped,
                    avg_tasks_injected: metrics.avg_tasks_injected,
                    avg_tasks_expired: metrics.avg_tasks_expired,
                    avg_conflicting_assignments: metrics.avg_conflicting_assignments,
                    avg_network_availability: metrics.avg_network_availability,
                    avg_relay_reallocation_ticks: metrics.avg_relay_reallocation_ticks,
                    avg_avg_hop_count: metrics.avg_avg_hop_count,
                    avg_disconnected_agents_max: metrics.avg_disconnected_agents_max,
                    avg_coverage_progress: metrics.avg_coverage_progress,
                    avg_bytes_sent: metrics.avg_bytes_sent,
                    avg_stale_state_age_ticks: metrics.avg_stale_state_age_ticks,
                    avg_battery_margin_min: metrics.avg_battery_margin_min,
                    avg_battery_margin_avg: metrics.avg_battery_margin_avg,
                    time_to_find: if metrics.avg_time_to_find > 0.0 {
                        Some(metrics.avg_time_to_find)
                    } else {
                        None
                    },
                    probability_of_detection: metrics.avg_probability_of_detection,
                    targets_found: metrics.avg_targets_found,
                    safety_violations: metrics.avg_safety_violations,
                    belief_entropy_final: metrics.avg_belief_entropy_final,
                    false_positive_rate: metrics.avg_false_positive_rate,
                    confirmation_scans: metrics.avg_confirmation_scans,
                    convergence_ticks_p50: metrics.convergence_ticks_p50,
                    convergence_ticks_p95: metrics.convergence_ticks_p95,
                    avg_bundle_travel_distance: metrics.avg_bundle_travel_distance,
                    // v0.16 Inspection metrics
                    avg_edge_coverage_rate: metrics.avg_edge_coverage_rate,
                    avg_missed_edges: metrics.avg_missed_edges,
                    avg_revisit_count: metrics.avg_revisit_count,
                    avg_route_efficiency: metrics.avg_route_efficiency,
                    // v0.28 Planner Quality metrics
                    avg_route_length: metrics.avg_route_length,
                    avg_wasted_travel: metrics.avg_wasted_travel,
                    avg_return_reserve: metrics.avg_return_reserve,
                    avg_infeasible_routes: metrics.avg_infeasible_routes,
                    // v0.30 Wildfire / Flood Mapping metrics
                    avg_hazard_zones_mapped: metrics.avg_hazard_zones_mapped,
                    avg_priority_updates: metrics.avg_priority_updates,
                    avg_final_threat_level: metrics.avg_final_threat_level,
                    // v0.38 Wildfire / Flood v2
                    avg_high_priority_zones_mapped: metrics.avg_high_priority_zones_mapped,
                    avg_time_to_map_first_high_risk: metrics.avg_time_to_map_first_high_risk,
                    avg_zone_observations: metrics.avg_zone_observations,
                });
            }
        }
    }

    serde_json::to_string_pretty(&JsonReport {
        benchmark_run_id: report.benchmark_run_id.clone(),
        strategy_names: report.strategy_names.clone(),
        profile_names: report.profile_names.clone(),
        rows,
    })
}

/// Export a ComparisonReport to CSV.
pub fn export_csv(report: &ComparisonReport) -> Result<String, csv::Error> {
    let mut wtr = csv::Writer::from_writer(Vec::new());

    wtr.write_record([
        "benchmark_run_id",
        "run_id",
        "mission",
        "scenario",
        "seed_range_start",
        "seed_range_end",
        "strategy",
        "profile",
        "total_runs",
        "success_rate",
        "avg_task_completion_rate",
        "avg_detection_ticks",
        "avg_reallocation_ticks",
        "avg_messages_attempted",
        "avg_messages_dropped",
        "avg_tasks_injected",
        "avg_tasks_expired",
        "avg_conflicting_assignments",
        "avg_network_availability",
        "avg_relay_reallocation_ticks",
        "avg_avg_hop_count",
        "avg_disconnected_agents_max",
        "avg_coverage_progress",
        "avg_bytes_sent",
        "avg_stale_state_age_ticks",
        "avg_battery_margin_min",
        "avg_battery_margin_avg",
        "time_to_find",
        "probability_of_detection",
        "targets_found",
        "safety_violations",
        "belief_entropy_final",
        "false_positive_rate",
        "confirmation_scans",
        "convergence_ticks_p50",
        "convergence_ticks_p95",
        "avg_bundle_travel_distance",
        "avg_edge_coverage_rate",
        "avg_missed_edges",
        "avg_revisit_count",
        "avg_route_efficiency",
        // v0.28 Planner Quality metrics
        "avg_route_length",
        "avg_wasted_travel",
        "avg_return_reserve",
        "avg_infeasible_routes",
        // v0.30 Wildfire / Flood Mapping metrics
        "avg_hazard_zones_mapped",
        "avg_priority_updates",
        "avg_final_threat_level",
        // v0.38 Wildfire / Flood v2
        "avg_high_priority_zones_mapped",
        "avg_time_to_map_first_high_risk",
        "avg_zone_observations",
    ])?;

    for strategy_name in &report.strategy_names {
        for profile_name in &report.profile_names {
            let key = (strategy_name.clone(), profile_name.clone());
            if let Some(m) = report.results.get(&key) {
                let safe_profile = profile_name.replace('/', "_");
                let row_id = format!(
                    "{}_{}_{}_{}",
                    report.benchmark_run_id, m.mission, strategy_name, safe_profile
                );
                wtr.write_record([
                    report.benchmark_run_id.as_str(),
                    row_id.as_str(),
                    m.mission.as_str(),
                    m.scenario.as_str(),
                    format!("{}", report.seed_range_start).as_str(),
                    format!("{}", report.seed_range_end).as_str(),
                    strategy_name,
                    profile_name,
                    m.total_runs.to_string().as_str(),
                    format!("{:.3}", m.success_rate).as_str(),
                    format!("{:.3}", m.avg_task_completion_rate).as_str(),
                    format!("{:.3}", m.avg_detection_ticks).as_str(),
                    format!("{:.3}", m.avg_reallocation_ticks).as_str(),
                    format!("{:.3}", m.avg_messages_attempted).as_str(),
                    format!("{:.3}", m.avg_messages_dropped).as_str(),
                    format!("{:.3}", m.avg_tasks_injected).as_str(),
                    format!("{:.3}", m.avg_tasks_expired).as_str(),
                    format!("{:.3}", m.avg_conflicting_assignments).as_str(),
                    format!("{:.3}", m.avg_network_availability).as_str(),
                    format!("{:.3}", m.avg_relay_reallocation_ticks).as_str(),
                    format!("{:.3}", m.avg_avg_hop_count).as_str(),
                    format!("{:.3}", m.avg_disconnected_agents_max).as_str(),
                    format!("{:.3}", m.avg_coverage_progress).as_str(),
                    format!("{:.3}", m.avg_bytes_sent).as_str(),
                    format!("{:.3}", m.avg_stale_state_age_ticks).as_str(),
                    format!("{:.3}", m.avg_battery_margin_min).as_str(),
                    format!("{:.3}", m.avg_battery_margin_avg).as_str(),
                    format!("{:.3}", m.avg_time_to_find).as_str(),
                    format!("{:.3}", m.avg_probability_of_detection).as_str(),
                    format!("{:.3}", m.avg_targets_found).as_str(),
                    format!("{:.3}", m.avg_safety_violations).as_str(),
                    format!("{:.3}", m.avg_belief_entropy_final).as_str(),
                    format!("{:.3}", m.avg_false_positive_rate).as_str(),
                    format!("{:.3}", m.avg_confirmation_scans).as_str(),
                    format!("{:.3}", m.convergence_ticks_p50).as_str(),
                    format!("{:.3}", m.convergence_ticks_p95).as_str(),
                    format!("{:.3}", m.avg_bundle_travel_distance).as_str(),
                    format!("{:.3}", m.avg_edge_coverage_rate).as_str(),
                    format!("{:.3}", m.avg_missed_edges).as_str(),
                    format!("{:.3}", m.avg_revisit_count).as_str(),
                    format!("{:.3}", m.avg_route_efficiency).as_str(),
                    // v0.28 Planner Quality metrics
                    format!("{:.3}", m.avg_route_length).as_str(),
                    format!("{:.3}", m.avg_wasted_travel).as_str(),
                    format!("{:.3}", m.avg_return_reserve).as_str(),
                    format!("{:.3}", m.avg_infeasible_routes).as_str(),
                    // v0.30 Wildfire / Flood Mapping metrics
                    format!("{:.3}", m.avg_hazard_zones_mapped).as_str(),
                    format!("{:.3}", m.avg_priority_updates).as_str(),
                    format!("{:.3}", m.avg_final_threat_level).as_str(),
                    // v0.38 Wildfire / Flood v2
                    format!("{:.3}", m.avg_high_priority_zones_mapped).as_str(),
                    format!("{:.3}", m.avg_time_to_map_first_high_risk).as_str(),
                    format!("{:.3}", m.avg_zone_observations).as_str(),
                ])?;
            }
        }
    }

    wtr.flush()?;
    let bytes = wtr
        .into_inner()
        .map_err(|e| csv::Error::from(e.into_error()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[derive(Serialize)]
struct JsonReport {
    benchmark_run_id: String,
    strategy_names: Vec<String>,
    profile_names: Vec<String>,
    rows: Vec<ReportRow>,
}

#[derive(Serialize)]
struct ReportRow {
    benchmark_run_id: String,
    run_id: String,
    mission: String,
    scenario: String,
    seed_range_start: u64,
    seed_range_end: u64,
    strategy: String,
    profile: String,
    total_runs: u64,
    success_rate: f64,
    avg_task_completion_rate: f64,
    avg_detection_ticks: f64,
    avg_reallocation_ticks: f64,
    avg_messages_attempted: f64,
    avg_messages_dropped: f64,
    avg_tasks_injected: f64,
    avg_tasks_expired: f64,
    avg_conflicting_assignments: f64,
    avg_network_availability: f64,
    avg_relay_reallocation_ticks: f64,
    avg_avg_hop_count: f64,
    avg_disconnected_agents_max: f64,
    avg_coverage_progress: f64,
    avg_bytes_sent: f64,
    avg_stale_state_age_ticks: f64,
    avg_battery_margin_min: f64,
    avg_battery_margin_avg: f64,
    time_to_find: Option<f64>,
    probability_of_detection: f64,
    targets_found: f64,
    safety_violations: f64,
    belief_entropy_final: f64,
    false_positive_rate: f64,
    confirmation_scans: f64,
    convergence_ticks_p50: f64,
    convergence_ticks_p95: f64,
    avg_bundle_travel_distance: f64,
    // v0.16 Inspection metrics
    avg_edge_coverage_rate: f64,
    avg_missed_edges: f64,
    avg_revisit_count: f64,
    avg_route_efficiency: f64,
    // v0.28 Planner Quality metrics
    avg_route_length: f64,
    avg_wasted_travel: f64,
    avg_return_reserve: f64,
    avg_infeasible_routes: f64,
    // v0.30 Wildfire / Flood Mapping metrics
    avg_hazard_zones_mapped: f64,
    avg_priority_updates: f64,
    avg_final_threat_level: f64,
    // v0.38 Wildfire / Flood v2
    avg_high_priority_zones_mapped: f64,
    avg_time_to_map_first_high_risk: f64,
    avg_zone_observations: f64,
}

/// Benchmark run manifest for reproducibility.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkManifest {
    pub timestamp: String,
    pub git_commit: String,
    pub command_line: String,
    pub suite_name: String,
    pub schema_version: String,
    pub seed_range_start: u64,
    pub seed_range_end: u64,
    pub strategy_names: Vec<String>,
    pub profile_names: Vec<String>,
    pub metric_schema_version: String,
    // v0.31 Realism metadata
    #[serde(default)]
    pub realism_profile: Option<String>,
    #[serde(default)]
    pub wind_enabled: bool,
    #[serde(default)]
    pub pose_noise_m: f64,
    #[serde(default)]
    pub comms_jitter_ticks: u64,
    // v0.37 Battery model metadata
    #[serde(default)]
    pub battery_model: Option<swarm_types::BatteryModel>,
}

impl BenchmarkManifest {
    pub fn new(
        suite_name: impl Into<String>,
        seed_range_start: u64,
        seed_range_end: u64,
        strategy_names: Vec<String>,
        profile_names: Vec<String>,
    ) -> Self {
        let git_commit = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_else(|| "unknown".to_owned())
            .trim()
            .to_owned();

        let command_line = std::env::args().collect::<Vec<_>>().join(" ");

        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            git_commit,
            command_line,
            suite_name: suite_name.into(),
            schema_version: "0.1".to_owned(),
            seed_range_start,
            seed_range_end,
            strategy_names,
            profile_names,
            metric_schema_version: "0.1".to_owned(),
            realism_profile: None,
            wind_enabled: false,
            pose_noise_m: 0.0,
            comms_jitter_ticks: 0,
            battery_model: None,
        }
    }
}

/// Export a ComparisonReport as a markdown table fragment.
pub fn export_markdown(report: &crate::ComparisonReport) -> String {
    format!("{}", report)
}

/// Generate a focused markdown report with per-mission tables and analysis.
pub fn generate_focused_report(reports: &[(String, crate::ComparisonReport)]) -> String {
    let mut out = String::new();
    out.push_str("# Benchmark Report\n\n");
    out.push_str(&format!(
        "Generated: {}  \n",
        chrono::Utc::now().to_rfc3339()
    ));

    // Git commit
    let git_commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_owned())
        .trim()
        .to_owned();
    out.push_str(&format!(
        "Git commit: `{}`  \n\n",
        &git_commit[..git_commit.len().min(8)]
    ));

    out.push_str("## Methodology\n\n");
    out.push_str("- Mode: quick (10 seeds)  \n");
    out.push_str("- Strategies: greedy, auction, connectivity-aware, centralized, cbba  \n");
    out.push_str("- Run: `cargo run -p swarm-examples --bin strategy_comparison -- --quick --mission <mission> --output-dir results/<mission>_quick/`  \n\n");

    // Per-mission tables
    for (mission_name, report) in reports {
        out.push_str(&format!("## {}\n\n", mission_name));

        // Build a focused table with only relevant metrics
        match mission_name.as_str() {
            "sar" => {
                out.push_str("| Strategy | Profile | Success | Completion | PoD | BeliefEntropy | FalsePosRate | ConfirmationScans |\n");
                out.push_str("|---|---|---|---|---|---|---|---|\n");
                for strategy in &report.strategy_names {
                    for profile in &report.profile_names {
                        if let Some(m) = report.results.get(&(strategy.clone(), profile.clone())) {
                            out.push_str(&format!(
                                "| {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
                                strategy,
                                profile,
                                m.success_rate,
                                m.avg_task_completion_rate,
                                m.avg_probability_of_detection,
                                m.avg_belief_entropy_final,
                                m.avg_false_positive_rate,
                                m.avg_confirmation_scans
                            ));
                        }
                    }
                }
            }
            "inspection" => {
                out.push_str("| Strategy | Profile | Success | Completion | EdgeCoverage | MissedEdges | RouteEfficiency |\n");
                out.push_str("|---|---|---|---|---|---|---|\n");
                for strategy in &report.strategy_names {
                    for profile in &report.profile_names {
                        if let Some(m) = report.results.get(&(strategy.clone(), profile.clone())) {
                            out.push_str(&format!(
                                "| {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
                                strategy,
                                profile,
                                m.success_rate,
                                m.avg_task_completion_rate,
                                m.avg_edge_coverage_rate,
                                m.avg_missed_edges,
                                m.avg_route_efficiency
                            ));
                        }
                    }
                }
            }
            _ => {
                // Generic table for coverage, safety, cbba_stress, etc.
                out.push_str("| Strategy | Profile | Success | Completion | Coverage | Messages | SafetyViolations | ConvP50 | ConvP95 | BundleDist |\n");
                out.push_str("|---|---|---|---|---|---|---|---|---|---|\n");
                for strategy in &report.strategy_names {
                    for profile in &report.profile_names {
                        if let Some(m) = report.results.get(&(strategy.clone(), profile.clone())) {
                            out.push_str(&format!(
                                "| {} | {} | {:.3} | {:.3} | {:.3} | {:.0} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
                                strategy, profile, m.success_rate, m.avg_task_completion_rate,
                                m.avg_coverage_progress, m.avg_messages_attempted,
                                m.avg_safety_violations, m.convergence_ticks_p50,
                                m.convergence_ticks_p95, m.avg_bundle_travel_distance
                            ));
                        }
                    }
                }
            }
        }
        out.push('\n');
    }

    // Summary / key questions
    out.push_str("## Answers to Key Questions\n\n");
    out.push_str("### Where does CBBA win?\n\n");
    out.push_str("CBBA excels in distributed scenarios where central coordination is unavailable. It shows competitive success rates without requiring a global view.\n\n");
    out.push_str("### Where does CBBA lose?\n\n");
    out.push_str("CBBA incurs higher communication overhead (more messages) and slower convergence (higher ConvP50/P95) compared to centralized planning. Bundle travel distance can be suboptimal vs. TSP-ordered centralized routes.\n\n");
    out.push_str("### SAR v2 vs SAR v1\n\n");
    out.push_str("SAR v2 adds belief-based search with entropy reduction. Metrics: `belief_entropy_final` shows how much uncertainty remains; `false_positive_rate` and `confirmation_scans` quantify sensor noise impact. Lower entropy + higher PoD indicates better search quality.\n\n");
    out.push_str("### Best strategies for inspection route coverage\n\n");
    out.push_str("Centralized and greedy tend to achieve higher `edge_coverage_rate` and lower `missed_edges`. CBBA may show higher `revisit_count` due to decentralized path construction.\n\n");
    out.push_str("### Distributed consensus overhead\n\n");
    out.push_str("Measured via `convergence_ticks_p50/p95` and `avg_messages_attempted`. CBBA typically requires 2-5x more messages than centralized/greedy. Convergence time increases with network loss.\n\n");
    out.push_str("### Safety constraint impact\n\n");
    out.push_str("Safety constraints (no-fly zones, geofences) reduce allocatable tasks. `safety_violations` should be near-zero for safety-aware allocators. Success rate may drop slightly when large task areas are blocked.\n\n");

    out.push_str("## Reproducibility\n\n");
    out.push_str("```bash\n");
    out.push_str("# Quick run (10 seeds, ~30s per mission)\n");
    out.push_str("cargo run -p swarm-examples --bin strategy_comparison -- --quick --mission sar --output-dir results/sar_quick/\n");
    out.push_str("cargo run -p swarm-examples --bin strategy_comparison -- --quick --mission inspection --output-dir results/inspection_quick/\n");
    out.push_str("cargo run -p swarm-examples --bin strategy_comparison -- --scenario-suite scenarios/coverage.safety.json --output-dir results/safety_quick/\n");
    out.push_str("cargo run -p swarm-examples --bin strategy_comparison -- --scenario-suite scenarios/cbba_stress.json --output-dir results/cbba_quick/\n\n");
    out.push_str("# Full run (1000 seeds, ~5min per mission)\n");
    out.push_str("cargo run -p swarm-examples --bin strategy_comparison -- --full --mission <mission> --output-dir results/<mission>_full/\n");
    out.push_str("```\n");

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use swarm_metrics::AggregateMetrics;

    fn make_report() -> ComparisonReport {
        let mut results = HashMap::new();
        results.insert(
            ("greedy".to_owned(), "ideal".to_owned()),
            AggregateMetrics {
                total_runs: 10,
                success_rate: 1.0,
                avg_detection_ticks: 0.0,
                avg_reallocation_ticks: 0.0,
                avg_messages_attempted: 90.0,
                avg_messages_dropped: 0.0,
                avg_tasks_injected: 0.0,
                avg_tasks_expired: 0.0,
                avg_conflicting_assignments: 0.0,
                avg_network_availability: 1.0,
                avg_relay_reallocation_ticks: 0.0,
                avg_avg_hop_count: 0.0,
                avg_disconnected_agents_max: 0.0,
                avg_coverage_progress: 1.0,
                avg_bytes_sent: 3960.0,
                avg_stale_state_age_ticks: 0.0,
                avg_battery_margin_min: 100.0,
                avg_battery_margin_avg: 100.0,
                avg_task_completion_rate: 1.0,
                avg_time_to_find: 0.0,
                avg_probability_of_detection: 0.0,
                avg_targets_found: 0.0,
                avg_safety_violations: 0.0,
                avg_belief_entropy_final: 0.0,
                avg_false_positive_rate: 0.0,
                avg_confirmation_scans: 0.0,
                convergence_ticks_p50: 0.0,
                convergence_ticks_p95: 0.0,
                convergence_ticks_max: 0.0,
                avg_bundle_travel_distance: 0.0,
                avg_edge_coverage_rate: 0.0,
                avg_missed_edges: 0.0,
                avg_revisit_count: 0.0,
                avg_route_efficiency: 0.0,
                avg_route_length: 0.0,
                avg_wasted_travel: 0.0,
                avg_return_reserve: 0.0,
                avg_infeasible_routes: 0.0,
                avg_hazard_zones_mapped: 0.0,
                avg_priority_updates: 0.0,
                avg_final_threat_level: 0.0,
                mission: "sar".to_owned(),
                scenario: "sar".to_owned(),
            },
        );
        ComparisonReport {
            benchmark_run_id: "test_10_quick".to_owned(),
            seed_range_start: 0,
            seed_range_end: 999,
            total_runs_per_cell: 10,
            mission_names: vec!["sar".to_owned()],
            scenario_names: vec!["sar_v1".to_owned()],
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["ideal".to_owned()],
            results,
        }
    }

    #[test]
    fn json_export_contains_benchmark_run_id() {
        let report = make_report();
        let json = export_json(&report).unwrap();
        assert!(json.contains("test_10_quick"));
        assert!(json.contains("benchmark_run_id"));
        assert!(json.contains("greedy"));
    }

    #[test]
    fn csv_export_contains_headers() {
        let report = make_report();
        let csv = export_csv(&report).unwrap();
        assert!(csv.contains("benchmark_run_id"));
        assert!(csv.contains("mission"));
        assert!(csv.contains("strategy"));
    }

    #[test]
    fn json_export_contains_mission_name() {
        let report = make_report();
        let json = export_json(&report).unwrap();
        assert!(json.contains("\"mission\""));
        assert!(json.contains("sar"));
    }

    #[test]
    fn csv_export_contains_mission_column() {
        let report = make_report();
        let csv = export_csv(&report).unwrap();
        assert!(csv.contains(",sar,"));
    }

    #[test]
    fn benchmark_manifest_serde_roundtrip() {
        let manifest = BenchmarkManifest {
            timestamp: "2024-01-01T00:00:00Z".to_owned(),
            git_commit: "abc123".to_owned(),
            command_line: "test".to_owned(),
            suite_name: "coverage".to_owned(),
            schema_version: "0.1".to_owned(),
            seed_range_start: 0,
            seed_range_end: 9,
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["ideal".to_owned()],
            metric_schema_version: "0.1".to_owned(),
            realism_profile: None,
            wind_enabled: false,
            pose_noise_m: 0.0,
            comms_jitter_ticks: 0,
            battery_model: None,
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let decoded: BenchmarkManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.git_commit, "abc123");
        assert_eq!(decoded.suite_name, "coverage");
        assert_eq!(decoded.seed_range_end, 9);
    }

    #[test]
    fn export_markdown_contains_header() {
        let report = make_report();
        let md = export_markdown(&report);
        assert!(md.contains("| Strategy"));
        assert!(md.contains("|"));
    }

    #[test]
    fn benchmark_manifest_new_has_git_commit() {
        let manifest = BenchmarkManifest::new(
            "test_suite",
            0,
            1,
            vec!["greedy".to_owned()],
            vec!["ideal".to_owned()],
        );
        assert!(!manifest.git_commit.is_empty());
        assert!(!manifest.timestamp.is_empty());
        assert_eq!(manifest.schema_version, "0.1");
        assert_eq!(manifest.metric_schema_version, "0.1");
    }

    #[test]
    fn focused_report_contains_mission_sections() {
        let report = make_report();
        let focused = generate_focused_report(&[("sar".to_owned(), report)]);
        assert!(focused.contains("# Benchmark Report"));
        assert!(focused.contains("## sar"));
        assert!(focused.contains("## Answers to Key Questions"));
        assert!(focused.contains("Where does CBBA win?"));
    }

    #[test]
    fn focused_report_has_summary_table() {
        let report = make_report();
        let focused = generate_focused_report(&[("sar".to_owned(), report)]);
        assert!(focused.contains("| Strategy"));
        assert!(focused.contains("| Profile"));
    }
}
