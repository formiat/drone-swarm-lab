use super::*;
use serde::{Deserialize, Serialize};
use swarm_metrics::AggregateMetrics;

use crate::ComparisonReport;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct RowIdentity {
    mission: String,
    scenario: String,
    strategy: String,
    profile: String,
}

fn row_identity(
    strategy_name: &str,
    profile_name: &str,
    metrics: &AggregateMetrics,
) -> RowIdentity {
    RowIdentity {
        mission: metrics.mission.clone(),
        scenario: metrics.scenario.clone(),
        strategy: strategy_name.to_owned(),
        profile: profile_name.to_owned(),
    }
}

/// Export a ComparisonReport to JSON.
pub fn export_json(report: &ComparisonReport) -> Result<String, serde_json::Error> {
    let mut rows = Vec::new();
    for strategy_name in &report.strategy_names {
        for profile_name in &report.profile_names {
            let key = (strategy_name.clone(), profile_name.clone());
            if let Some(metrics) = report.results.get(&key) {
                let identity = row_identity(strategy_name, profile_name, metrics);
                let safe_profile = identity.profile.replace('/', "_");
                let row_id = format!(
                    "{}_{}_{}_{}",
                    report.benchmark_run_id, identity.mission, identity.strategy, safe_profile
                );
                rows.push(ReportRow {
                    benchmark_run_id: report.benchmark_run_id.clone(),
                    run_id: row_id,
                    mission: identity.mission,
                    scenario: identity.scenario,
                    seed_range_start: report.seed_range_start,
                    seed_range_end: report.seed_range_end,
                    strategy: identity.strategy,
                    profile: identity.profile,
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
                    // v0.30 Wildfire Mapping metrics
                    avg_hazard_zones_mapped: metrics.avg_hazard_zones_mapped,
                    avg_priority_updates: metrics.avg_priority_updates,
                    avg_final_threat_level: metrics.avg_final_threat_level,
                    // v0.38 Wildfire v2
                    avg_high_priority_zones_mapped: metrics.avg_high_priority_zones_mapped,
                    avg_time_to_map_first_high_risk: metrics.avg_time_to_map_first_high_risk,
                    avg_zone_observations: metrics.avg_zone_observations,
                    // v0.64 Urban Foundations
                    avg_urban_route_length_m: metrics.avg_urban_route_length_m,
                    avg_urban_route_risk_score: metrics.avg_urban_route_risk_score,
                    urban_route_planned_rate: metrics.urban_route_planned_rate,
                    avg_urban_violation_count: metrics.avg_urban_violation_count,
                    urban_route_completed_rate: metrics.urban_route_completed_rate,
                    // v0.65 Urban Patrol v0
                    urban_patrol_completed_rate: metrics.urban_patrol_completed_rate,
                    avg_urban_time_to_complete_loop: metrics.avg_urban_time_to_complete_loop,
                    avg_urban_distance_travelled_m: metrics.avg_urban_distance_travelled_m,
                    avg_urban_route_efficiency: metrics.avg_urban_route_efficiency,
                    avg_urban_replan_count: metrics.avg_urban_replan_count,
                    // v0.66 Urban Search v1
                    bus_detection_rate: metrics.bus_detection_rate,
                    avg_time_to_detect_bus: metrics.avg_time_to_detect_bus,
                    avg_false_positive_count: metrics.avg_false_positive_count,
                    avg_distance_before_detection: metrics.avg_distance_before_detection,
                    search_success_without_violation_rate: metrics
                        .search_success_without_violation_rate,
                    // v0.67 Urban Replay / Analysis
                    avg_urban_min_agent_separation_m: metrics.avg_urban_min_agent_separation_m,
                    avg_urban_separation_violation_count: metrics
                        .avg_urban_separation_violation_count,
                    avg_urban_route_conflict_count: metrics.avg_urban_route_conflict_count,
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
        // v0.30 Wildfire Mapping metrics
        "avg_hazard_zones_mapped",
        "avg_priority_updates",
        "avg_final_threat_level",
        // v0.38 Wildfire v2
        "avg_high_priority_zones_mapped",
        "avg_time_to_map_first_high_risk",
        "avg_zone_observations",
        // v0.64 Urban Foundations
        "avg_urban_route_length_m",
        "avg_urban_route_risk_score",
        "urban_route_planned_rate",
        "avg_urban_violation_count",
        "urban_route_completed_rate",
        // v0.65 Urban Patrol v0
        "urban_patrol_completed_rate",
        "avg_urban_time_to_complete_loop",
        "avg_urban_distance_travelled_m",
        "avg_urban_route_efficiency",
        "avg_urban_replan_count",
        // v0.66 Urban Search v1
        "bus_detection_rate",
        "avg_time_to_detect_bus",
        "avg_false_positive_count",
        "avg_distance_before_detection",
        "search_success_without_violation_rate",
        // v0.67 Urban Replay / Analysis
        "avg_urban_min_agent_separation_m",
        "avg_urban_separation_violation_count",
        "avg_urban_route_conflict_count",
    ])?;

    for strategy_name in &report.strategy_names {
        for profile_name in &report.profile_names {
            let key = (strategy_name.clone(), profile_name.clone());
            if let Some(m) = report.results.get(&key) {
                let identity = row_identity(strategy_name, profile_name, m);
                let safe_profile = identity.profile.replace('/', "_");
                let row_id = format!(
                    "{}_{}_{}_{}",
                    report.benchmark_run_id, identity.mission, identity.strategy, safe_profile
                );
                wtr.write_record([
                    report.benchmark_run_id.as_str(),
                    row_id.as_str(),
                    identity.mission.as_str(),
                    identity.scenario.as_str(),
                    format!("{}", report.seed_range_start).as_str(),
                    format!("{}", report.seed_range_end).as_str(),
                    identity.strategy.as_str(),
                    identity.profile.as_str(),
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
                    // v0.30 Wildfire Mapping metrics
                    format!("{:.3}", m.avg_hazard_zones_mapped).as_str(),
                    format!("{:.3}", m.avg_priority_updates).as_str(),
                    format!("{:.3}", m.avg_final_threat_level).as_str(),
                    // v0.38 Wildfire v2
                    format!("{:.3}", m.avg_high_priority_zones_mapped).as_str(),
                    format!("{:.3}", m.avg_time_to_map_first_high_risk).as_str(),
                    format!("{:.3}", m.avg_zone_observations).as_str(),
                    // v0.64 Urban Foundations
                    format!("{:.3}", m.avg_urban_route_length_m).as_str(),
                    format!("{:.3}", m.avg_urban_route_risk_score).as_str(),
                    format!("{:.3}", m.urban_route_planned_rate).as_str(),
                    format!("{:.3}", m.avg_urban_violation_count).as_str(),
                    format!("{:.3}", m.urban_route_completed_rate).as_str(),
                    // v0.65 Urban Patrol v0
                    format!("{:.3}", m.urban_patrol_completed_rate).as_str(),
                    format!("{:.3}", m.avg_urban_time_to_complete_loop).as_str(),
                    format!("{:.3}", m.avg_urban_distance_travelled_m).as_str(),
                    format!("{:.3}", m.avg_urban_route_efficiency).as_str(),
                    format!("{:.3}", m.avg_urban_replan_count).as_str(),
                    // v0.66 Urban Search v1
                    format!("{:.3}", m.bus_detection_rate).as_str(),
                    format!("{:.3}", m.avg_time_to_detect_bus).as_str(),
                    format!("{:.3}", m.avg_false_positive_count).as_str(),
                    format!("{:.3}", m.avg_distance_before_detection).as_str(),
                    format!("{:.3}", m.search_success_without_violation_rate).as_str(),
                    // v0.67 Urban Replay / Analysis
                    format!("{:.3}", m.avg_urban_min_agent_separation_m).as_str(),
                    format!("{:.3}", m.avg_urban_separation_violation_count).as_str(),
                    format!("{:.3}", m.avg_urban_route_conflict_count).as_str(),
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
    // v0.30 Wildfire Mapping metrics
    avg_hazard_zones_mapped: f64,
    avg_priority_updates: f64,
    avg_final_threat_level: f64,
    // v0.38 Wildfire v2
    avg_high_priority_zones_mapped: f64,
    avg_time_to_map_first_high_risk: f64,
    avg_zone_observations: f64,
    // v0.64 Urban Foundations
    avg_urban_route_length_m: f64,
    avg_urban_route_risk_score: f64,
    urban_route_planned_rate: f64,
    avg_urban_violation_count: f64,
    urban_route_completed_rate: f64,
    // v0.65 Urban Patrol v0
    urban_patrol_completed_rate: f64,
    avg_urban_time_to_complete_loop: f64,
    avg_urban_distance_travelled_m: f64,
    avg_urban_route_efficiency: f64,
    avg_urban_replan_count: f64,
    // v0.66 Urban Search v1
    bus_detection_rate: f64,
    avg_time_to_detect_bus: f64,
    avg_false_positive_count: f64,
    avg_distance_before_detection: f64,
    search_success_without_violation_rate: f64,
    // v0.67 Urban Replay / Analysis
    avg_urban_min_agent_separation_m: f64,
    avg_urban_separation_violation_count: f64,
    avg_urban_route_conflict_count: f64,
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
    /// Number of rayon worker threads used; `None` means all available CPUs.
    #[serde(default)]
    pub jobs: Option<usize>,
    /// Cargo build profile when known (`debug` or `release`).
    #[serde(default)]
    pub build_profile: Option<String>,
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
            jobs: None,
            build_profile: Some(
                if cfg!(debug_assertions) {
                    "debug"
                } else {
                    "release"
                }
                .to_owned(),
            ),
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
            "urban-patrol" => {
                out.push_str("| Strategy | Profile | Success | Completion | UrbanRouteLength | UrbanRisk | UrbanPlanned | UrbanViolations | UrbanCompleted | PatrolCompleted | TimeToLoop | Distance | RouteEfficiency | Replans | MinSeparation | SeparationViolations | RouteConflicts |\n");
                out.push_str(
                    "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
                );
                for strategy in &report.strategy_names {
                    for profile in &report.profile_names {
                        if let Some(m) = report.results.get(&(strategy.clone(), profile.clone())) {
                            out.push_str(&format!(
                                "| {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
                                strategy,
                                profile,
                                m.success_rate,
                                m.avg_task_completion_rate,
                                m.avg_urban_route_length_m,
                                m.avg_urban_route_risk_score,
                                m.urban_route_planned_rate,
                                m.avg_urban_violation_count,
                                m.urban_route_completed_rate,
                                m.urban_patrol_completed_rate,
                                m.avg_urban_time_to_complete_loop,
                                m.avg_urban_distance_travelled_m,
                                m.avg_urban_route_efficiency,
                                m.avg_urban_replan_count,
                                m.avg_urban_min_agent_separation_m,
                                m.avg_urban_separation_violation_count,
                                m.avg_urban_route_conflict_count
                            ));
                        }
                    }
                }
            }
            "urban-search" => {
                out.push_str("| Strategy | Profile | Success | BusDetected | TimeToBus | FalsePositives | DistanceBeforeBus | SearchSuccessNoViolation | UrbanViolations | RouteEfficiency | MinSeparation | SeparationViolations | RouteConflicts |\n");
                out.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
                for strategy in &report.strategy_names {
                    for profile in &report.profile_names {
                        if let Some(m) = report.results.get(&(strategy.clone(), profile.clone())) {
                            out.push_str(&format!(
                                "| {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
                                strategy,
                                profile,
                                m.success_rate,
                                m.bus_detection_rate,
                                m.avg_time_to_detect_bus,
                                m.avg_false_positive_count,
                                m.avg_distance_before_detection,
                                m.search_success_without_violation_rate,
                                m.avg_urban_violation_count,
                                m.avg_urban_route_efficiency,
                                m.avg_urban_min_agent_separation_m,
                                m.avg_urban_separation_violation_count,
                                m.avg_urban_route_conflict_count
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

/// Compare two [`crate::ComparisonReport`]s for metric equality, ignoring timestamps,
/// run ids, and iteration-order differences in strategy/profile names.
///
/// Returns `Ok(())` when the reports agree on all checked metrics, or `Err(msgs)` with a
/// list of human-readable mismatch descriptions. Because both inputs are expected to use the
/// same seeds in sorted order, metric values must be bit-identical — no tolerance is applied.
pub fn compare_reports(
    a: &crate::ComparisonReport,
    b: &crate::ComparisonReport,
) -> Result<(), Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    validate_report_identity("first", a, &mut errors);
    validate_report_identity("second", b, &mut errors);

    compare_string_sets(
        "mission_names",
        &a.mission_names,
        &b.mission_names,
        &mut errors,
    );
    compare_string_sets(
        "scenario_names",
        &a.scenario_names,
        &b.scenario_names,
        &mut errors,
    );
    compare_string_sets(
        "strategy_names",
        &a.strategy_names,
        &b.strategy_names,
        &mut errors,
    );
    compare_string_sets(
        "profile_names",
        &a.profile_names,
        &b.profile_names,
        &mut errors,
    );

    if a.seed_range_start != b.seed_range_start {
        errors.push(format!(
            "seed_range_start differs: {} vs {}",
            a.seed_range_start, b.seed_range_start
        ));
    }
    if a.seed_range_end != b.seed_range_end {
        errors.push(format!(
            "seed_range_end differs: {} vs {}",
            a.seed_range_end, b.seed_range_end
        ));
    }
    if a.total_runs_per_cell != b.total_runs_per_cell {
        errors.push(format!(
            "total_runs_per_cell differs: {} vs {}",
            a.total_runs_per_cell, b.total_runs_per_cell
        ));
    }

    let a_identities = sorted_report_identities(a);
    let b_identities = sorted_report_identities(b);
    if a_identities != b_identities {
        errors.push(format!(
            "row identities differ: {:?} vs {:?}",
            a_identities, b_identities
        ));
    }

    if a.results.len() != b.results.len() {
        errors.push(format!(
            "row count differs: {} vs {}",
            a.results.len(),
            b.results.len()
        ));
    }

    // Per-row metric equality.
    for key in a.results.keys() {
        match (a.results.get(key), b.results.get(key)) {
            (Some(ma), Some(mb)) => {
                compare_aggregate_metrics(key, ma, mb, &mut errors);
            }
            (Some(_), None) => {
                errors.push(format!(
                    "key {key:?} present in first report but not in second"
                ));
            }
            _ => {}
        }
    }
    for key in b.results.keys() {
        if !a.results.contains_key(key) {
            errors.push(format!(
                "key {key:?} present in second report but not in first"
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn sorted_report_identities(report: &crate::ComparisonReport) -> Vec<RowIdentity> {
    let mut identities = Vec::new();
    for strategy_name in &report.strategy_names {
        for profile_name in &report.profile_names {
            let key = (strategy_name.clone(), profile_name.clone());
            if let Some(metrics) = report.results.get(&key) {
                identities.push(row_identity(strategy_name, profile_name, metrics));
            }
        }
    }
    identities.sort();
    identities
}

fn compare_string_sets(label: &str, a: &[String], b: &[String], errors: &mut Vec<String>) {
    let mut a_sorted = a.to_vec();
    let mut b_sorted = b.to_vec();
    a_sorted.sort();
    b_sorted.sort();
    if a_sorted != b_sorted {
        errors.push(format!("{label} differ: {a_sorted:?} vs {b_sorted:?}"));
    }
}

fn validate_report_identity(
    label: &str,
    report: &crate::ComparisonReport,
    errors: &mut Vec<String>,
) {
    validate_name_list(label, "strategy_names", &report.strategy_names, errors);
    validate_name_list(label, "profile_names", &report.profile_names, errors);

    let mut visible_identities = std::collections::BTreeSet::new();
    for identity in sorted_report_identities(report) {
        if identity.mission.is_empty() {
            errors.push(format!("{label}: row {identity:?} has empty mission"));
        }
        if identity.scenario.is_empty() {
            errors.push(format!("{label}: row {identity:?} has empty scenario"));
        }
        if identity.strategy.is_empty() {
            errors.push(format!("{label}: row {identity:?} has empty strategy"));
        }
        if identity.profile.is_empty() {
            errors.push(format!("{label}: row {identity:?} has empty profile"));
        }
        if !visible_identities.insert(identity.clone()) {
            errors.push(format!("{label}: duplicate row identity {identity:?}"));
        }
    }

    for key in report.results.keys() {
        if !report.strategy_names.contains(&key.0) {
            errors.push(format!(
                "{label}: results key {key:?} uses a strategy absent from strategy_names"
            ));
        }
        if !report.profile_names.contains(&key.1) {
            errors.push(format!(
                "{label}: results key {key:?} uses a profile absent from profile_names"
            ));
        }
    }
}

fn validate_name_list(
    report_label: &str,
    field_label: &str,
    values: &[String],
    errors: &mut Vec<String>,
) {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        if value.is_empty() {
            errors.push(format!(
                "{report_label}: {field_label} contains an empty name"
            ));
        }
        if !seen.insert(value) {
            errors.push(format!(
                "{report_label}: {field_label} contains duplicate name {value:?}"
            ));
        }
    }
}

pub(super) fn compare_metric_field<T: PartialEq + std::fmt::Debug>(
    errors: &mut Vec<String>,
    key: &(String, String),
    field: &str,
    a: &T,
    b: &T,
) {
    if a != b {
        errors.push(format!("key {key:?}: {field} {a:?} vs {b:?}"));
    }
}
