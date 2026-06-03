use super::identity::row_identity;
use crate::ComparisonReport;

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
        // v0.75 Urban Mission Realism Follow-up
        "avg_perimeter_completion_rate",
        "avg_perimeter_length_m",
        "avg_time_to_complete_perimeter",
        "avg_perimeter_violations",
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
                    // v0.75 Urban Mission Realism Follow-up
                    format!("{:.3}", m.avg_perimeter_completion_rate).as_str(),
                    format!("{:.3}", m.avg_perimeter_length_m).as_str(),
                    format!("{:.3}", m.avg_time_to_complete_perimeter).as_str(),
                    format!("{:.3}", m.avg_perimeter_violations).as_str(),
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
