use super::*;
use swarm_metrics::AggregateMetrics;

pub(super) fn compare_aggregate_metrics(
    key: &(String, String),
    a: &AggregateMetrics,
    b: &AggregateMetrics,
    errors: &mut Vec<String>,
) {
    let errors_before = errors.len();
    macro_rules! compare_field {
        ($field:ident) => {
            compare_metric_field(errors, key, stringify!($field), &a.$field, &b.$field);
        };
    }

    compare_field!(total_runs);
    compare_field!(success_rate);
    compare_field!(avg_detection_ticks);
    compare_field!(avg_reallocation_ticks);
    compare_field!(avg_messages_attempted);
    compare_field!(avg_messages_dropped);
    compare_field!(avg_tasks_injected);
    compare_field!(avg_tasks_expired);
    compare_field!(avg_conflicting_assignments);
    compare_field!(avg_network_availability);
    compare_field!(avg_relay_reallocation_ticks);
    compare_field!(avg_avg_hop_count);
    compare_field!(avg_disconnected_agents_max);
    compare_field!(avg_coverage_progress);
    compare_field!(avg_bytes_sent);
    compare_field!(avg_stale_state_age_ticks);
    compare_field!(avg_battery_margin_min);
    compare_field!(avg_battery_margin_avg);
    compare_field!(avg_task_completion_rate);
    compare_field!(avg_time_to_find);
    compare_field!(avg_probability_of_detection);
    compare_field!(avg_targets_found);
    compare_field!(avg_safety_violations);
    compare_field!(avg_belief_entropy_final);
    compare_field!(avg_false_positive_rate);
    compare_field!(avg_confirmation_scans);
    compare_field!(convergence_ticks_p50);
    compare_field!(convergence_ticks_p95);
    compare_field!(convergence_ticks_max);
    compare_field!(avg_bundle_travel_distance);
    compare_field!(avg_edge_coverage_rate);
    compare_field!(avg_missed_edges);
    compare_field!(avg_revisit_count);
    compare_field!(avg_route_efficiency);
    compare_field!(avg_route_length);
    compare_field!(avg_wasted_travel);
    compare_field!(avg_return_reserve);
    compare_field!(avg_infeasible_routes);
    compare_field!(avg_hazard_zones_mapped);
    compare_field!(avg_priority_updates);
    compare_field!(avg_final_threat_level);
    compare_field!(avg_high_priority_zones_mapped);
    compare_field!(avg_time_to_map_first_high_risk);
    compare_field!(avg_zone_observations);
    compare_field!(avg_urban_route_length_m);
    compare_field!(avg_urban_route_risk_score);
    compare_field!(urban_route_planned_rate);
    compare_field!(avg_urban_violation_count);
    compare_field!(urban_route_completed_rate);
    compare_field!(urban_patrol_completed_rate);
    compare_field!(avg_urban_time_to_complete_loop);
    compare_field!(avg_urban_distance_travelled_m);
    compare_field!(avg_urban_route_efficiency);
    compare_field!(avg_urban_replan_count);
    compare_field!(bus_detection_rate);
    compare_field!(avg_time_to_detect_bus);
    compare_field!(avg_false_positive_count);
    compare_field!(avg_distance_before_detection);
    compare_field!(search_success_without_violation_rate);
    compare_field!(avg_urban_min_agent_separation_m);
    compare_field!(avg_urban_separation_violation_count);
    compare_field!(avg_urban_route_conflict_count);
    compare_field!(mission);
    compare_field!(scenario);

    if a != b && errors.len() == errors_before {
        errors.push(format!(
            "key {key:?}: aggregate metrics differ in an unlisted field"
        ));
    }
}
