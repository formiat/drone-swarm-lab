use serde::Serialize;

use super::identity::row_identity;
use crate::ComparisonReport;

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
