use serde::{Deserialize, Serialize};

use super::RunMetrics;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MetricStats {
    pub mean: f64,
    pub stddev: f64,
    pub stderr: f64,
    pub ci95_low: f64,
    pub ci95_high: f64,
    pub min: f64,
    pub max: f64,
}

impl MetricStats {
    pub fn from_values(values: &[f64]) -> Self {
        if values.is_empty() {
            return Self::default();
        }

        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let variance = if values.len() > 1 {
            values
                .iter()
                .map(|value| {
                    let delta = value - mean;
                    delta * delta
                })
                .sum::<f64>()
                / (n - 1.0)
        } else {
            0.0
        };
        let stddev = variance.sqrt();
        let stderr = stddev / n.sqrt();
        let ci95_half_width = 1.96 * stderr;

        Self {
            mean,
            stddev,
            stderr,
            ci95_low: mean - ci95_half_width,
            ci95_high: mean + ci95_half_width,
            min,
            max,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AggregateMetrics {
    pub total_runs: u64,
    pub success_rate: f64,
    #[serde(default)]
    pub success_stats: MetricStats,
    #[serde(default)]
    pub task_completion_stats: MetricStats,
    #[serde(default)]
    pub failure_rate: f64,
    pub avg_detection_ticks: f64,
    pub avg_reallocation_ticks: f64,
    pub avg_messages_attempted: f64,
    pub avg_messages_dropped: f64,
    pub avg_tasks_injected: f64,
    pub avg_tasks_expired: f64,
    pub avg_conflicting_assignments: f64,
    // v0.5
    pub avg_network_availability: f64,
    pub avg_relay_reallocation_ticks: f64,
    pub avg_avg_hop_count: f64,
    pub avg_disconnected_agents_max: f64,
    // v0.6
    pub avg_coverage_progress: f64,
    pub avg_bytes_sent: f64,
    pub avg_stale_state_age_ticks: f64,
    pub avg_battery_margin_min: f64,
    pub avg_battery_margin_avg: f64,
    pub avg_task_completion_rate: f64,
    // v0.11 SAR aggregation
    pub avg_time_to_find: f64,
    pub avg_probability_of_detection: f64,
    pub avg_targets_found: f64,
    // v0.13 Safety aggregation
    #[serde(default)]
    pub avg_safety_violations: f64,
    // v0.14 SAR v2 belief aggregation
    #[serde(default)]
    pub avg_belief_entropy_final: f64,
    #[serde(default)]
    pub avg_false_positive_rate: f64,
    #[serde(default)]
    pub avg_confirmation_scans: f64,
    // v0.15 CBBA robustness
    #[serde(default)]
    pub convergence_ticks_p50: f64,
    #[serde(default)]
    pub convergence_ticks_p95: f64,
    #[serde(default)]
    pub convergence_ticks_max: f64,
    #[serde(default)]
    pub avg_bundle_travel_distance: f64,
    // v0.16 Inspection metrics
    #[serde(default)]
    pub avg_edge_coverage_rate: f64,
    #[serde(default)]
    pub avg_missed_edges: f64,
    #[serde(default)]
    pub avg_revisit_count: f64,
    #[serde(default)]
    pub avg_route_efficiency: f64,
    // v0.28 Planner Quality metrics
    #[serde(default)]
    pub avg_route_length: f64,
    #[serde(default)]
    pub avg_wasted_travel: f64,
    #[serde(default)]
    pub avg_return_reserve: f64,
    #[serde(default)]
    pub avg_infeasible_routes: f64,
    // v0.30 Wildfire Mapping metrics
    #[serde(default)]
    pub avg_hazard_zones_mapped: f64,
    #[serde(default)]
    pub avg_priority_updates: f64,
    #[serde(default)]
    pub avg_final_threat_level: f64,
    // v0.38 Wildfire v2
    #[serde(default)]
    pub avg_high_priority_zones_mapped: f64,
    #[serde(default)]
    pub avg_time_to_map_first_high_risk: f64,
    #[serde(default)]
    pub avg_zone_observations: f64,
    // v0.31 Report identity: per-row mission and scenario
    #[serde(default)]
    pub mission: String,
    #[serde(default)]
    pub scenario: String,
    // v0.64 Urban Foundations
    #[serde(default)]
    pub avg_urban_route_length_m: f64,
    // v0.68 Urban Algorithm Depth
    #[serde(default)]
    pub avg_urban_route_risk_score: f64,
    #[serde(default)]
    pub urban_route_planned_rate: f64,
    #[serde(default)]
    pub avg_urban_violation_count: f64,
    #[serde(default)]
    pub urban_route_completed_rate: f64,
    // v0.65 Urban Patrol v0
    #[serde(default)]
    pub urban_patrol_completed_rate: f64,
    #[serde(default)]
    pub avg_urban_time_to_complete_loop: f64,
    #[serde(default)]
    pub avg_urban_distance_travelled_m: f64,
    #[serde(default)]
    pub avg_urban_route_efficiency: f64,
    #[serde(default)]
    pub avg_urban_replan_count: f64,
    // v0.66 Urban Search v1
    #[serde(default)]
    pub bus_detection_rate: f64,
    #[serde(default)]
    pub avg_time_to_detect_bus: f64,
    #[serde(default)]
    pub avg_false_positive_count: f64,
    #[serde(default)]
    pub avg_distance_before_detection: f64,
    #[serde(default)]
    pub search_success_without_violation_rate: f64,
    // v0.67 Urban Replay / Analysis
    #[serde(default)]
    pub avg_urban_min_agent_separation_m: f64,
    #[serde(default)]
    pub avg_urban_separation_violation_count: f64,
    #[serde(default)]
    pub avg_urban_route_conflict_count: f64,
    // v0.85 Urban Multi-Agent Deconfliction
    #[serde(default)]
    pub avg_urban_deconflict_conflict_count: f64,
    #[serde(default)]
    pub avg_urban_deconflict_wait_ticks: f64,
    #[serde(default)]
    pub avg_urban_deconflict_replan_count: f64,
    #[serde(default)]
    pub avg_urban_deconflict_abort_count: f64,
    #[serde(default)]
    pub avg_urban_segment_utilization: f64,
    #[serde(default)]
    pub avg_urban_delay_per_agent_ticks: f64,
    // v0.75 Urban Mission Realism Follow-up
    #[serde(default)]
    pub avg_perimeter_completion_rate: f64,
    #[serde(default)]
    pub avg_perimeter_length_m: f64,
    #[serde(default)]
    pub avg_time_to_complete_perimeter: f64,
    #[serde(default)]
    pub avg_perimeter_violations: f64,
}

pub(super) fn percentile_of_sorted(sorted: &[u64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)) as usize;
    sorted[idx.min(sorted.len() - 1)] as f64
}

impl AggregateMetrics {
    pub fn from_runs(runs: &[RunMetrics]) -> Self {
        if runs.is_empty() {
            return Self {
                total_runs: 0,
                success_rate: 0.0,
                success_stats: MetricStats::default(),
                task_completion_stats: MetricStats::default(),
                failure_rate: 0.0,
                avg_detection_ticks: 0.0,
                avg_reallocation_ticks: 0.0,
                avg_messages_attempted: 0.0,
                avg_messages_dropped: 0.0,
                avg_tasks_injected: 0.0,
                avg_tasks_expired: 0.0,
                avg_conflicting_assignments: 0.0,
                avg_network_availability: 0.0,
                avg_relay_reallocation_ticks: 0.0,
                avg_avg_hop_count: 0.0,
                avg_disconnected_agents_max: 0.0,
                avg_coverage_progress: 0.0,
                avg_bytes_sent: 0.0,
                avg_stale_state_age_ticks: 0.0,
                avg_battery_margin_min: 0.0,
                avg_battery_margin_avg: 0.0,
                avg_task_completion_rate: 0.0,
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
                // v0.16 Inspection metrics
                avg_edge_coverage_rate: 0.0,
                avg_missed_edges: 0.0,
                avg_revisit_count: 0.0,
                avg_route_efficiency: 0.0,
                // v0.28 Planner Quality metrics
                avg_route_length: 0.0,
                avg_wasted_travel: 0.0,
                avg_return_reserve: 0.0,
                avg_infeasible_routes: 0.0,
                // v0.30 Wildfire Mapping metrics
                avg_hazard_zones_mapped: 0.0,
                avg_priority_updates: 0.0,
                avg_final_threat_level: 0.0,
                // v0.38 Wildfire v2
                avg_high_priority_zones_mapped: 0.0,
                avg_time_to_map_first_high_risk: 0.0,
                avg_zone_observations: 0.0,
                // v0.31 Report identity
                mission: String::new(),
                scenario: String::new(),
                // v0.64 Urban Foundations
                avg_urban_route_length_m: 0.0,
                avg_urban_route_risk_score: 0.0,
                urban_route_planned_rate: 0.0,
                avg_urban_violation_count: 0.0,
                urban_route_completed_rate: 0.0,
                // v0.65 Urban Patrol v0
                urban_patrol_completed_rate: 0.0,
                avg_urban_time_to_complete_loop: 0.0,
                avg_urban_distance_travelled_m: 0.0,
                avg_urban_route_efficiency: 0.0,
                avg_urban_replan_count: 0.0,
                // v0.66 Urban Search v1
                bus_detection_rate: 0.0,
                avg_time_to_detect_bus: 0.0,
                avg_false_positive_count: 0.0,
                avg_distance_before_detection: 0.0,
                search_success_without_violation_rate: 0.0,
                // v0.67 Urban Replay / Analysis
                avg_urban_min_agent_separation_m: 0.0,
                avg_urban_separation_violation_count: 0.0,
                avg_urban_route_conflict_count: 0.0,
                avg_urban_deconflict_conflict_count: 0.0,
                avg_urban_deconflict_wait_ticks: 0.0,
                avg_urban_deconflict_replan_count: 0.0,
                avg_urban_deconflict_abort_count: 0.0,
                avg_urban_segment_utilization: 0.0,
                avg_urban_delay_per_agent_ticks: 0.0,
                avg_perimeter_completion_rate: 0.0,
                avg_perimeter_length_m: 0.0,
                avg_time_to_complete_perimeter: 0.0,
                avg_perimeter_violations: 0.0,
            };
        }

        let total_runs = runs.len() as u64;
        let success_count = runs.iter().filter(|run| run.success).count() as f64;
        let success_values: Vec<f64> = runs
            .iter()
            .map(|run| if run.success { 1.0 } else { 0.0 })
            .collect();
        let task_completion_values: Vec<f64> =
            runs.iter().map(|run| run.task_completion_rate).collect();
        let total_messages_attempted: u64 = runs.iter().map(|run| run.messages_attempted).sum();
        let total_messages_dropped: u64 = runs.iter().map(|run| run.messages_dropped).sum();
        let total_tasks_injected: u64 = runs.iter().map(|run| run.tasks_injected).sum();
        let total_tasks_expired: u64 = runs.iter().map(|run| run.tasks_expired).sum();
        let total_conflicting: u64 = runs.iter().map(|run| run.conflicting_assignments).sum();
        let total_network_availability: f64 = runs.iter().map(|run| run.network_availability).sum();
        let total_avg_hop_count: f64 = runs.iter().map(|run| run.avg_hop_count).sum();
        let total_disconnected_max: u64 = runs.iter().map(|run| run.disconnected_agents_max).sum();
        let total_coverage_progress: f64 = runs.iter().map(|run| run.coverage_progress).sum();
        let total_bytes_sent: u64 = runs.iter().map(|run| run.bytes_sent).sum();
        let total_stale_state_age: u64 = runs.iter().map(|run| run.stale_state_age_ticks).sum();
        let total_battery_margin_min: f64 = runs.iter().map(|run| run.battery_margin_min).sum();
        let total_battery_margin_avg: f64 = runs.iter().map(|run| run.battery_margin_avg).sum();
        let total_task_completion_rate: f64 = runs.iter().map(|run| run.task_completion_rate).sum();
        let total_time_to_find: u64 = runs.iter().filter_map(|run| run.time_to_find).sum();
        let time_to_find_count =
            runs.iter().filter(|run| run.time_to_find.is_some()).count() as f64;
        let total_probability_of_detection: f64 =
            runs.iter().map(|run| run.probability_of_detection).sum();
        let total_targets_found: u64 = runs.iter().map(|run| run.targets_found as u64).sum();
        let total_safety_violations: u64 = runs.iter().map(|run| run.safety_violations).sum();
        let total_belief_entropy: f64 = runs.iter().map(|run| run.belief_entropy_final).sum();
        let total_false_positives: u64 = runs.iter().map(|run| run.false_positives as u64).sum();
        let total_confirmation_scans: u64 =
            runs.iter().map(|run| run.confirmation_scans as u64).sum();
        let total_scan_count: u64 = runs.iter().map(|run| run.scan_count as u64).sum();
        // v0.15 CBBA robustness
        let total_bundle_travel_distance: f64 =
            runs.iter().map(|run| run.bundle_travel_distance).sum();
        // v0.16 Inspection metrics
        let total_edge_coverage_rate: f64 = runs.iter().map(|run| run.edge_coverage_rate).sum();
        let total_missed_edges: u64 = runs.iter().map(|run| run.missed_edges).sum();
        let total_revisit_count: u64 = runs.iter().map(|run| run.revisit_count).sum();
        let total_route_efficiency: f64 = runs.iter().map(|run| run.route_efficiency).sum();
        // v0.28 Planner Quality metrics
        let total_route_length: f64 = runs.iter().map(|run| run.avg_route_length).sum();
        let total_wasted_travel: f64 = runs.iter().map(|run| run.avg_wasted_travel).sum();
        let total_return_reserve: f64 = runs.iter().map(|run| run.avg_return_reserve).sum();
        let total_infeasible_routes: u64 = runs.iter().map(|run| run.infeasible_routes).sum();
        // v0.30 Wildfire Mapping metrics
        let total_hazard_zones_mapped: u64 = runs.iter().map(|run| run.hazard_zones_mapped).sum();
        let total_priority_updates: u64 = runs.iter().map(|run| run.priority_updates).sum();
        let total_final_threat_level: f64 = runs.iter().map(|run| run.final_avg_threat_level).sum();
        // v0.38 Wildfire v2
        let total_high_priority_zones_mapped: u64 =
            runs.iter().map(|run| run.high_priority_zones_mapped).sum();
        let total_time_to_map_first_high_risk: u64 = runs
            .iter()
            .filter_map(|run| run.time_to_map_first_high_risk)
            .sum();
        let time_to_map_first_high_risk_count = runs
            .iter()
            .filter(|run| run.time_to_map_first_high_risk.is_some())
            .count() as f64;
        let total_zone_observations: u64 = runs.iter().map(|run| run.zone_observations).sum();
        // v0.64 Urban Foundations
        let total_urban_route_length_m: f64 = runs.iter().map(|run| run.urban_route_length_m).sum();
        let total_urban_route_risk_score: f64 =
            runs.iter().map(|run| run.urban_route_risk_score).sum();
        let urban_route_planned_count =
            runs.iter().filter(|run| run.urban_route_planned).count() as f64;
        let total_urban_violation_count: u64 =
            runs.iter().map(|run| run.urban_violation_count).sum();
        let urban_route_completed_count =
            runs.iter().filter(|run| run.urban_route_completed).count() as f64;
        // v0.65 Urban Patrol v0
        let urban_patrol_completed_count =
            runs.iter().filter(|run| run.urban_patrol_completed).count() as f64;
        let total_urban_time_to_complete_loop: u64 = runs
            .iter()
            .filter_map(|run| run.urban_time_to_complete_loop)
            .sum();
        let urban_time_to_complete_loop_count = runs
            .iter()
            .filter(|run| run.urban_time_to_complete_loop.is_some())
            .count() as f64;
        let total_urban_distance_travelled_m: f64 =
            runs.iter().map(|run| run.urban_distance_travelled_m).sum();
        let total_urban_route_efficiency: f64 =
            runs.iter().map(|run| run.urban_route_efficiency).sum();
        let total_urban_replan_count: u64 = runs.iter().map(|run| run.urban_replan_count).sum();
        // v0.66 Urban Search v1
        let bus_detected_count = runs.iter().filter(|run| run.bus_detected).count() as f64;
        let total_time_to_detect_bus: u64 =
            runs.iter().filter_map(|run| run.time_to_detect_bus).sum();
        let time_to_detect_bus_count = runs
            .iter()
            .filter(|run| run.time_to_detect_bus.is_some())
            .count() as f64;
        let total_false_positive_count: u64 = runs.iter().map(|run| run.false_positive_count).sum();
        let total_distance_before_detection: f64 =
            runs.iter().map(|run| run.distance_before_detection).sum();
        let search_success_without_violation_count = runs
            .iter()
            .filter(|run| run.search_success_without_violation)
            .count() as f64;
        // v0.67 Urban Replay / Analysis
        let total_urban_min_agent_separation_m: f64 = runs
            .iter()
            .filter_map(|run| run.urban_min_agent_separation_m)
            .sum();
        let urban_min_agent_separation_count = runs
            .iter()
            .filter(|run| run.urban_min_agent_separation_m.is_some())
            .count() as f64;
        let total_urban_separation_violation_count: u64 = runs
            .iter()
            .map(|run| run.urban_separation_violation_count)
            .sum();
        let total_urban_route_conflict_count: u64 =
            runs.iter().map(|run| run.urban_route_conflict_count).sum();
        // v0.85 Urban Multi-Agent Deconfliction
        let total_urban_deconflict_conflict_count: u64 = runs
            .iter()
            .map(|run| run.urban_deconflict_conflict_count)
            .sum();
        let total_urban_deconflict_wait_ticks: u64 =
            runs.iter().map(|run| run.urban_deconflict_wait_ticks).sum();
        let total_urban_deconflict_replan_count: u64 = runs
            .iter()
            .map(|run| run.urban_deconflict_replan_count)
            .sum();
        let total_urban_deconflict_abort_count: u64 = runs
            .iter()
            .map(|run| run.urban_deconflict_abort_count)
            .sum();
        let total_urban_segment_utilization: f64 =
            runs.iter().map(|run| run.urban_segment_utilization).sum();
        let total_urban_avg_delay_per_agent_ticks: f64 = runs
            .iter()
            .map(|run| run.urban_avg_delay_per_agent_ticks)
            .sum();
        // v0.75 Urban Mission Realism Follow-up
        let total_perimeter_completion_rate: f64 =
            runs.iter().map(|run| run.perimeter_completion_rate).sum();
        let total_perimeter_length_m: f64 = runs.iter().map(|run| run.perimeter_length_m).sum();
        let total_time_to_complete_perimeter: u64 = runs
            .iter()
            .filter_map(|run| run.time_to_complete_perimeter)
            .sum();
        let time_to_complete_perimeter_count = runs
            .iter()
            .filter(|run| run.time_to_complete_perimeter.is_some())
            .count() as f64;
        let total_perimeter_violations: u64 = runs.iter().map(|run| run.perimeter_violations).sum();
        let mut convergence_ticks: Vec<u64> = runs
            .iter()
            .filter_map(|run| run.cbba_convergence_tick)
            .collect();
        convergence_ticks.sort_unstable();
        let p50 = percentile_of_sorted(&convergence_ticks, 50.0);
        let p95 = percentile_of_sorted(&convergence_ticks, 95.0);
        let cmax = convergence_ticks.last().copied().unwrap_or(0) as f64;
        let n = runs.len() as f64;

        Self {
            total_runs,
            success_rate: success_count / n,
            success_stats: MetricStats::from_values(&success_values),
            task_completion_stats: MetricStats::from_values(&task_completion_values),
            failure_rate: 1.0 - (success_count / n),
            avg_detection_ticks: average_optional(runs.iter().map(|run| run.detection_time_ticks)),
            avg_reallocation_ticks: average_optional(
                runs.iter().map(|run| run.reallocation_time_ticks),
            ),
            avg_messages_attempted: total_messages_attempted as f64 / n,
            avg_messages_dropped: total_messages_dropped as f64 / n,
            avg_tasks_injected: total_tasks_injected as f64 / n,
            avg_tasks_expired: total_tasks_expired as f64 / n,
            avg_conflicting_assignments: total_conflicting as f64 / n,
            avg_network_availability: total_network_availability / n,
            avg_relay_reallocation_ticks: average_optional(
                runs.iter().map(|run| run.relay_reallocation_ticks),
            ),
            avg_avg_hop_count: total_avg_hop_count / n,
            avg_disconnected_agents_max: total_disconnected_max as f64 / n,
            avg_coverage_progress: total_coverage_progress / n,
            avg_bytes_sent: total_bytes_sent as f64 / n,
            avg_stale_state_age_ticks: total_stale_state_age as f64 / n,
            avg_battery_margin_min: total_battery_margin_min / n,
            avg_battery_margin_avg: total_battery_margin_avg / n,
            avg_task_completion_rate: total_task_completion_rate / n,
            avg_time_to_find: if time_to_find_count > 0.0 {
                total_time_to_find as f64 / time_to_find_count
            } else {
                0.0
            },
            avg_probability_of_detection: total_probability_of_detection / n,
            avg_targets_found: total_targets_found as f64 / n,
            avg_safety_violations: total_safety_violations as f64 / n,
            avg_belief_entropy_final: total_belief_entropy / n,
            avg_false_positive_rate: if total_scan_count > 0 {
                total_false_positives as f64 / total_scan_count as f64
            } else {
                0.0
            },
            avg_confirmation_scans: total_confirmation_scans as f64 / n,
            convergence_ticks_p50: p50,
            convergence_ticks_p95: p95,
            convergence_ticks_max: cmax,
            avg_bundle_travel_distance: total_bundle_travel_distance / n,
            // v0.16 Inspection metrics
            avg_edge_coverage_rate: total_edge_coverage_rate / n,
            avg_missed_edges: total_missed_edges as f64 / n,
            avg_revisit_count: total_revisit_count as f64 / n,
            avg_route_efficiency: total_route_efficiency / n,
            // v0.28 Planner Quality metrics
            avg_route_length: total_route_length / n,
            avg_wasted_travel: total_wasted_travel / n,
            avg_return_reserve: total_return_reserve / n,
            avg_infeasible_routes: total_infeasible_routes as f64 / n,
            // v0.30 Wildfire Mapping metrics
            avg_hazard_zones_mapped: total_hazard_zones_mapped as f64 / n,
            avg_priority_updates: total_priority_updates as f64 / n,
            avg_final_threat_level: total_final_threat_level / n,
            // v0.38 Wildfire v2
            avg_high_priority_zones_mapped: total_high_priority_zones_mapped as f64 / n,
            avg_time_to_map_first_high_risk: if time_to_map_first_high_risk_count > 0.0 {
                total_time_to_map_first_high_risk as f64 / time_to_map_first_high_risk_count
            } else {
                0.0
            },
            avg_zone_observations: total_zone_observations as f64 / n,
            // v0.31 Report identity: populated by caller after aggregation
            mission: String::new(),
            scenario: String::new(),
            // v0.64 Urban Foundations
            avg_urban_route_length_m: total_urban_route_length_m / n,
            avg_urban_route_risk_score: total_urban_route_risk_score / n,
            urban_route_planned_rate: urban_route_planned_count / n,
            avg_urban_violation_count: total_urban_violation_count as f64 / n,
            urban_route_completed_rate: urban_route_completed_count / n,
            // v0.65 Urban Patrol v0
            urban_patrol_completed_rate: urban_patrol_completed_count / n,
            avg_urban_time_to_complete_loop: if urban_time_to_complete_loop_count > 0.0 {
                total_urban_time_to_complete_loop as f64 / urban_time_to_complete_loop_count
            } else {
                0.0
            },
            avg_urban_distance_travelled_m: total_urban_distance_travelled_m / n,
            avg_urban_route_efficiency: total_urban_route_efficiency / n,
            avg_urban_replan_count: total_urban_replan_count as f64 / n,
            // v0.66 Urban Search v1
            bus_detection_rate: bus_detected_count / n,
            avg_time_to_detect_bus: if time_to_detect_bus_count > 0.0 {
                total_time_to_detect_bus as f64 / time_to_detect_bus_count
            } else {
                0.0
            },
            avg_false_positive_count: total_false_positive_count as f64 / n,
            avg_distance_before_detection: total_distance_before_detection / n,
            search_success_without_violation_rate: search_success_without_violation_count / n,
            // v0.67 Urban Replay / Analysis
            avg_urban_min_agent_separation_m: if urban_min_agent_separation_count > 0.0 {
                total_urban_min_agent_separation_m / urban_min_agent_separation_count
            } else {
                0.0
            },
            avg_urban_separation_violation_count: total_urban_separation_violation_count as f64 / n,
            avg_urban_route_conflict_count: total_urban_route_conflict_count as f64 / n,
            avg_urban_deconflict_conflict_count: total_urban_deconflict_conflict_count as f64 / n,
            avg_urban_deconflict_wait_ticks: total_urban_deconflict_wait_ticks as f64 / n,
            avg_urban_deconflict_replan_count: total_urban_deconflict_replan_count as f64 / n,
            avg_urban_deconflict_abort_count: total_urban_deconflict_abort_count as f64 / n,
            avg_urban_segment_utilization: total_urban_segment_utilization / n,
            avg_urban_delay_per_agent_ticks: total_urban_avg_delay_per_agent_ticks / n,
            avg_perimeter_completion_rate: total_perimeter_completion_rate / n,
            avg_perimeter_length_m: total_perimeter_length_m / n,
            avg_time_to_complete_perimeter: if time_to_complete_perimeter_count > 0.0 {
                total_time_to_complete_perimeter as f64 / time_to_complete_perimeter_count
            } else {
                0.0
            },
            avg_perimeter_violations: total_perimeter_violations as f64 / n,
        }
    }
}

fn average_optional(values: impl Iterator<Item = Option<u64>>) -> f64 {
    let mut count = 0_u64;
    let mut sum = 0_u64;

    for value in values.flatten() {
        count += 1;
        sum += value;
    }

    if count == 0 {
        0.0
    } else {
        sum as f64 / count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_stats_empty_values_are_zeroed() {
        let stats = MetricStats::from_values(&[]);

        assert_eq!(stats, MetricStats::default());
    }

    #[test]
    fn metric_stats_single_value_has_zero_variance() {
        let stats = MetricStats::from_values(&[0.75]);

        assert_eq!(stats.mean, 0.75);
        assert_eq!(stats.stddev, 0.0);
        assert_eq!(stats.stderr, 0.0);
        assert_eq!(stats.ci95_low, 0.75);
        assert_eq!(stats.ci95_high, 0.75);
        assert_eq!(stats.min, 0.75);
        assert_eq!(stats.max, 0.75);
    }

    #[test]
    fn metric_stats_binary_success_values_include_stderr_and_ci() {
        let stats = MetricStats::from_values(&[1.0, 0.0, 1.0, 1.0]);

        assert!((stats.mean - 0.75).abs() < 1e-9);
        assert!((stats.stddev - 0.5).abs() < 1e-9);
        assert!((stats.stderr - 0.25).abs() < 1e-9);
        assert!((stats.ci95_low - 0.26).abs() < 1e-9);
        assert!((stats.ci95_high - 1.24).abs() < 1e-9);
        assert_eq!(stats.min, 0.0);
        assert_eq!(stats.max, 1.0);
    }
}
