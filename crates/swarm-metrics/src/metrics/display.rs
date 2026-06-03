use std::fmt;

use super::AggregateMetrics;

impl fmt::Display for AggregateMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "runs: {}", self.total_runs)?;
        writeln!(f, "success_rate: {:.3}", self.success_rate)?;
        writeln!(f, "avg_detection_ticks: {:.3}", self.avg_detection_ticks)?;
        writeln!(
            f,
            "avg_reallocation_ticks: {:.3}",
            self.avg_reallocation_ticks
        )?;
        writeln!(
            f,
            "avg_messages_attempted: {:.3}",
            self.avg_messages_attempted
        )?;
        writeln!(f, "avg_messages_dropped: {:.3}", self.avg_messages_dropped)?;
        writeln!(f, "avg_tasks_injected: {:.3}", self.avg_tasks_injected)?;
        writeln!(f, "avg_tasks_expired: {:.3}", self.avg_tasks_expired)?;
        writeln!(
            f,
            "avg_conflicting_assignments: {:.3}",
            self.avg_conflicting_assignments
        )?;
        writeln!(
            f,
            "avg_network_availability: {:.3}",
            self.avg_network_availability
        )?;
        writeln!(
            f,
            "avg_relay_reallocation_ticks: {:.3}",
            self.avg_relay_reallocation_ticks
        )?;
        writeln!(f, "avg_avg_hop_count: {:.3}", self.avg_avg_hop_count)?;
        writeln!(
            f,
            "avg_disconnected_agents_max: {:.3}",
            self.avg_disconnected_agents_max
        )?;
        writeln!(
            f,
            "avg_coverage_progress: {:.3}",
            self.avg_coverage_progress
        )?;
        writeln!(f, "avg_bytes_sent: {:.3}", self.avg_bytes_sent)?;
        writeln!(
            f,
            "avg_stale_state_age_ticks: {:.3}",
            self.avg_stale_state_age_ticks
        )?;
        writeln!(
            f,
            "avg_battery_margin_min: {:.3}",
            self.avg_battery_margin_min
        )?;
        writeln!(
            f,
            "avg_battery_margin_avg: {:.3}",
            self.avg_battery_margin_avg
        )?;
        writeln!(
            f,
            "avg_task_completion_rate: {:.3}",
            self.avg_task_completion_rate
        )?;
        writeln!(f, "avg_time_to_find: {:.3}", self.avg_time_to_find)?;
        writeln!(
            f,
            "avg_probability_of_detection: {:.3}",
            self.avg_probability_of_detection
        )?;
        writeln!(f, "avg_targets_found: {:.3}", self.avg_targets_found)?;
        writeln!(
            f,
            "avg_belief_entropy_final: {:.3}",
            self.avg_belief_entropy_final
        )?;
        writeln!(
            f,
            "avg_false_positive_rate: {:.3}",
            self.avg_false_positive_rate
        )?;
        write!(
            f,
            "avg_confirmation_scans: {:.3}",
            self.avg_confirmation_scans
        )?;
        writeln!(f)?;
        writeln!(
            f,
            "convergence_ticks_p50: {:.3}",
            self.convergence_ticks_p50
        )?;
        writeln!(
            f,
            "convergence_ticks_p95: {:.3}",
            self.convergence_ticks_p95
        )?;
        writeln!(
            f,
            "convergence_ticks_max: {:.3}",
            self.convergence_ticks_max
        )?;
        writeln!(
            f,
            "avg_bundle_travel_distance: {:.3}",
            self.avg_bundle_travel_distance
        )?;
        writeln!(
            f,
            "avg_edge_coverage_rate: {:.3}",
            self.avg_edge_coverage_rate
        )?;
        writeln!(f, "avg_missed_edges: {:.3}", self.avg_missed_edges)?;
        writeln!(f, "avg_revisit_count: {:.3}", self.avg_revisit_count)?;
        writeln!(f, "avg_route_efficiency: {:.3}", self.avg_route_efficiency)?;
        writeln!(f, "avg_route_length: {:.3}", self.avg_route_length)?;
        writeln!(f, "avg_wasted_travel: {:.3}", self.avg_wasted_travel)?;
        writeln!(f, "avg_return_reserve: {:.3}", self.avg_return_reserve)?;
        writeln!(
            f,
            "avg_infeasible_routes: {:.3}",
            self.avg_infeasible_routes
        )?;
        writeln!(
            f,
            "avg_hazard_zones_mapped: {:.3}",
            self.avg_hazard_zones_mapped
        )?;
        writeln!(f, "avg_priority_updates: {:.3}", self.avg_priority_updates)?;
        writeln!(
            f,
            "avg_final_threat_level: {:.3}",
            self.avg_final_threat_level
        )?;
        writeln!(
            f,
            "avg_high_priority_zones_mapped: {:.3}",
            self.avg_high_priority_zones_mapped
        )?;
        writeln!(
            f,
            "avg_time_to_map_first_high_risk: {:.3}",
            self.avg_time_to_map_first_high_risk
        )?;
        writeln!(
            f,
            "avg_zone_observations: {:.3}",
            self.avg_zone_observations
        )?;
        writeln!(
            f,
            "avg_urban_route_length_m: {:.3}",
            self.avg_urban_route_length_m
        )?;
        writeln!(
            f,
            "avg_urban_route_risk_score: {:.3}",
            self.avg_urban_route_risk_score
        )?;
        writeln!(
            f,
            "urban_route_planned_rate: {:.3}",
            self.urban_route_planned_rate
        )?;
        writeln!(
            f,
            "avg_urban_violation_count: {:.3}",
            self.avg_urban_violation_count
        )?;
        writeln!(
            f,
            "urban_route_completed_rate: {:.3}",
            self.urban_route_completed_rate
        )?;
        writeln!(
            f,
            "urban_patrol_completed_rate: {:.3}",
            self.urban_patrol_completed_rate
        )?;
        writeln!(
            f,
            "avg_urban_time_to_complete_loop: {:.3}",
            self.avg_urban_time_to_complete_loop
        )?;
        writeln!(
            f,
            "avg_urban_distance_travelled_m: {:.3}",
            self.avg_urban_distance_travelled_m
        )?;
        writeln!(
            f,
            "avg_urban_route_efficiency: {:.3}",
            self.avg_urban_route_efficiency
        )?;
        writeln!(
            f,
            "avg_urban_replan_count: {:.3}",
            self.avg_urban_replan_count
        )?;
        writeln!(f, "bus_detection_rate: {:.3}", self.bus_detection_rate)?;
        writeln!(
            f,
            "avg_time_to_detect_bus: {:.3}",
            self.avg_time_to_detect_bus
        )?;
        writeln!(
            f,
            "avg_false_positive_count: {:.3}",
            self.avg_false_positive_count
        )?;
        writeln!(
            f,
            "avg_distance_before_detection: {:.3}",
            self.avg_distance_before_detection
        )?;
        writeln!(
            f,
            "search_success_without_violation_rate: {:.3}",
            self.search_success_without_violation_rate
        )?;
        writeln!(
            f,
            "avg_urban_min_agent_separation_m: {:.3}",
            self.avg_urban_min_agent_separation_m
        )?;
        writeln!(
            f,
            "avg_urban_separation_violation_count: {:.3}",
            self.avg_urban_separation_violation_count
        )?;
        writeln!(
            f,
            "avg_urban_route_conflict_count: {:.3}",
            self.avg_urban_route_conflict_count
        )?;
        writeln!(
            f,
            "avg_perimeter_completion_rate: {:.3}",
            self.avg_perimeter_completion_rate
        )?;
        writeln!(
            f,
            "avg_perimeter_length_m: {:.3}",
            self.avg_perimeter_length_m
        )?;
        writeln!(
            f,
            "avg_time_to_complete_perimeter: {:.3}",
            self.avg_time_to_complete_perimeter
        )?;
        writeln!(
            f,
            "avg_perimeter_violations: {:.3}",
            self.avg_perimeter_violations
        )?;
        writeln!(f, "mission: {}", self.mission)?;
        write!(f, "scenario: {}", self.scenario)
    }
}
