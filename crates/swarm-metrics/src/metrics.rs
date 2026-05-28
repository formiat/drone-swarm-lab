use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunMetrics {
    pub seed: u64,
    pub total_ticks: u64,
    pub messages_attempted: u64,
    pub messages_dropped: u64,
    pub detection_time_ticks: Option<u64>,
    pub reallocation_time_ticks: Option<u64>,
    pub max_task_unassigned_ticks: u64,
    pub all_tasks_assigned: bool,
    pub success: bool,
    pub tasks_injected: u64,
    pub tasks_expired: u64,
    pub conflicting_assignments: u64,
    pub partition_events: u64,
    pub partitions_active: bool,
    pub stale_messages_discarded: u64,
    pub convergence_ticks: Option<u64>,
    pub max_view_divergence: u64,
    // v0.5 network availability metrics
    pub network_availability: f64,
    pub relay_reallocation_ticks: Option<u64>,
    pub avg_hop_count: f64,
    pub disconnected_agents_max: u64,
    // v0.6 strategy comparison metrics
    #[serde(default)]
    pub coverage_progress: f64,
    #[serde(default)]
    pub bytes_sent: u64,
    #[serde(default)]
    pub stale_state_age_ticks: u64,
    #[serde(default)]
    pub battery_margin_min: f64,
    #[serde(default)]
    pub battery_margin_avg: f64,
    // v0.8 kinematic metrics
    #[serde(default)]
    pub final_battery_min: f64,
    #[serde(default)]
    pub avg_distance_travelled: f64,
    #[serde(default)]
    pub agents_exhausted: u64,
    #[serde(default)]
    pub total_distance_travelled: f64,
    #[serde(default)]
    pub mission_completion_ticks: u64,
    #[serde(default)]
    pub time_to_first_exhaustion: Option<u64>,
    // v0.9 SAR metrics
    #[serde(default)]
    pub time_to_find: Option<u64>,
    #[serde(default)]
    pub coverage_over_time: Vec<f64>,
    #[serde(default)]
    pub probability_of_detection: f64,
    #[serde(default)]
    pub targets_found: u32,
    #[serde(default)]
    pub targets_total: u32,
    #[serde(default)]
    pub scan_count: u32,
    // v0.10 CBBA metrics
    #[serde(default)]
    pub cbba_rounds_to_convergence: u64,
    #[serde(default)]
    pub cbba_converged: bool,
    #[serde(default)]
    pub cbba_messages: u64,
    // v0.13 Safety metrics
    #[serde(default)]
    pub safety_violations: u64,
    // v0.14 SAR v2 belief metrics
    #[serde(default)]
    pub belief_entropy_final: f64,
    #[serde(default)]
    pub false_positives: u32,
    #[serde(default)]
    pub confirmation_scans: u32,
    // v0.15 CBBA robustness
    #[serde(default)]
    pub cbba_convergence_tick: Option<u64>,
    #[serde(default)]
    pub bundle_travel_distance: f64,
    // v0.16 Inspection metrics
    #[serde(default)]
    pub edge_coverage_rate: f64,
    #[serde(default)]
    pub missed_edges: u64,
    #[serde(default)]
    pub revisit_count: u64,
    #[serde(default)]
    pub route_efficiency: f64,
    // v0.28 Planner Quality metrics
    #[serde(default)]
    pub avg_route_length: f64,
    #[serde(default)]
    pub avg_wasted_travel: f64,
    #[serde(default)]
    pub avg_return_reserve: f64,
    #[serde(default)]
    pub infeasible_routes: u64,
    // v0.30 Wildfire / Flood Mapping metrics
    #[serde(default)]
    pub hazard_zones_mapped: u64,
    #[serde(default)]
    pub priority_updates: u64,
    #[serde(default)]
    pub final_avg_threat_level: f64,
    // v0.35 Dynamic Mission Correctness
    #[serde(default)]
    pub unsupported_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregateMetrics {
    pub total_runs: u64,
    pub success_rate: f64,
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
    // v0.30 Wildfire / Flood Mapping metrics
    #[serde(default)]
    pub avg_hazard_zones_mapped: f64,
    #[serde(default)]
    pub avg_priority_updates: f64,
    #[serde(default)]
    pub avg_final_threat_level: f64,
    // v0.31 Report identity: per-row mission and scenario
    #[serde(default)]
    pub mission: String,
    #[serde(default)]
    pub scenario: String,
}

fn percentile_of_sorted(sorted: &[u64], p: f64) -> f64 {
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
                // v0.30 Wildfire / Flood Mapping metrics
                avg_hazard_zones_mapped: 0.0,
                avg_priority_updates: 0.0,
                avg_final_threat_level: 0.0,
                // v0.31 Report identity
                mission: String::new(),
                scenario: String::new(),
            };
        }

        let total_runs = runs.len() as u64;
        let success_count = runs.iter().filter(|run| run.success).count() as f64;
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
        let task_completion_count = runs.iter().filter(|run| run.all_tasks_assigned).count() as f64;
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
        // v0.30 Wildfire / Flood Mapping metrics
        let total_hazard_zones_mapped: u64 = runs.iter().map(|run| run.hazard_zones_mapped).sum();
        let total_priority_updates: u64 = runs.iter().map(|run| run.priority_updates).sum();
        let total_final_threat_level: f64 = runs.iter().map(|run| run.final_avg_threat_level).sum();
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
            avg_task_completion_rate: task_completion_count / n,
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
            // v0.30 Wildfire / Flood Mapping metrics
            avg_hazard_zones_mapped: total_hazard_zones_mapped as f64 / n,
            avg_priority_updates: total_priority_updates as f64 / n,
            avg_final_threat_level: total_final_threat_level / n,
            // v0.31 Report identity: populated by caller after aggregation
            mission: String::new(),
            scenario: String::new(),
        }
    }
}

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
        // v0.28 Planner Quality metrics
        writeln!(f, "avg_route_length: {:.3}", self.avg_route_length)?;
        writeln!(f, "avg_wasted_travel: {:.3}", self.avg_wasted_travel)?;
        writeln!(f, "avg_return_reserve: {:.3}", self.avg_return_reserve)?;
        writeln!(
            f,
            "avg_infeasible_routes: {:.3}",
            self.avg_infeasible_routes
        )?;
        // v0.30 Wildfire / Flood Mapping metrics
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
        // v0.31 Report identity
        writeln!(f, "mission: {}", self.mission)?;
        write!(f, "scenario: {}", self.scenario)
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

    fn run(success: bool, detection_time_ticks: Option<u64>) -> RunMetrics {
        RunMetrics {
            seed: 0,
            total_ticks: 10,
            messages_attempted: 10,
            messages_dropped: 2,
            detection_time_ticks,
            reallocation_time_ticks: Some(1),
            max_task_unassigned_ticks: 1,
            all_tasks_assigned: success,
            success,
            tasks_injected: 0,
            tasks_expired: 0,
            conflicting_assignments: 0,
            partition_events: 0,
            partitions_active: false,
            stale_messages_discarded: 0,
            convergence_ticks: None,
            max_view_divergence: 0,
            network_availability: 1.0,
            relay_reallocation_ticks: None,
            avg_hop_count: 0.0,
            disconnected_agents_max: 0,
            coverage_progress: 0.0,
            bytes_sent: 0,
            stale_state_age_ticks: 0,
            battery_margin_min: 0.0,
            battery_margin_avg: 0.0,
            final_battery_min: 0.0,
            avg_distance_travelled: 0.0,
            agents_exhausted: 0,
            total_distance_travelled: 0.0,
            mission_completion_ticks: 0,
            time_to_first_exhaustion: None,
            time_to_find: None,
            coverage_over_time: vec![],
            probability_of_detection: 0.0,
            targets_found: 0,
            targets_total: 0,
            scan_count: 0,
            cbba_rounds_to_convergence: 0,
            cbba_converged: false,
            cbba_messages: 0,
            safety_violations: 0,
            belief_entropy_final: 0.0,
            false_positives: 0,
            confirmation_scans: 0,
            cbba_convergence_tick: None,
            bundle_travel_distance: 0.0,
            // v0.16 Inspection metrics
            edge_coverage_rate: 0.0,
            missed_edges: 0,
            revisit_count: 0,
            route_efficiency: 0.0,
            // v0.28 Planner Quality metrics
            avg_route_length: 0.0,
            avg_wasted_travel: 0.0,
            avg_return_reserve: 0.0,
            infeasible_routes: 0,
            // v0.30 Wildfire / Flood Mapping metrics
            hazard_zones_mapped: 0,
            priority_updates: 0,
            final_avg_threat_level: 0.0,
            // v0.35 Dynamic Mission Correctness
            unsupported_reason: None,
        }
    }

    #[test]
    fn aggregate_success_rate() {
        let mut runs = Vec::new();
        for _ in 0..8 {
            runs.push(run(true, Some(2)));
        }
        for _ in 0..2 {
            runs.push(run(false, Some(4)));
        }

        let metrics = AggregateMetrics::from_runs(&runs);

        assert_eq!(metrics.success_rate, 0.8);
    }

    #[test]
    fn aggregate_avg_detection() {
        let runs = vec![run(true, Some(2)), run(true, Some(4)), run(true, None)];

        let metrics = AggregateMetrics::from_runs(&runs);

        assert_eq!(metrics.avg_detection_ticks, 3.0);
    }

    #[test]
    fn aggregate_avg_tasks_injected() {
        let mut runs = vec![run(true, None), run(true, None), run(true, None)];
        runs[0].tasks_injected = 3;
        runs[1].tasks_injected = 6;
        runs[2].tasks_injected = 0;

        let metrics = AggregateMetrics::from_runs(&runs);

        assert_eq!(metrics.avg_tasks_injected, 3.0);
    }

    #[test]
    fn aggregate_avg_tasks_expired() {
        let mut runs = vec![run(true, None), run(true, None)];
        runs[0].tasks_expired = 2;
        runs[1].tasks_expired = 4;

        let metrics = AggregateMetrics::from_runs(&runs);

        assert_eq!(metrics.avg_tasks_expired, 3.0);
    }

    #[test]
    fn aggregate_avg_task_completion_rate() {
        let runs = vec![run(true, None), run(true, None), run(false, None)];
        // 2 out of 3 runs have all_tasks_assigned=true (set by run(success, ...))
        // The third run has success=false, so all_tasks_assigned=false
        let metrics = AggregateMetrics::from_runs(&runs);

        assert!(
            (metrics.avg_task_completion_rate - 0.666_666_7).abs() < 1e-6,
            "Expected ~0.666667 for 2/3 completed runs, got {}",
            metrics.avg_task_completion_rate
        );
    }

    #[test]
    fn aggregate_sar_fields() {
        let mut runs = Vec::new();
        for i in 0..10 {
            let mut r = run(true, None);
            r.time_to_find = if i < 5 { Some(100) } else { None };
            r.probability_of_detection = 0.8;
            r.targets_total = 5;
            r.targets_found = 3;
            runs.push(r);
        }

        let metrics = AggregateMetrics::from_runs(&runs);
        assert!((metrics.avg_probability_of_detection - 0.8).abs() < 0.01);
        assert!((metrics.avg_targets_found - 3.0).abs() < 0.01);
    }

    #[test]
    fn aggregate_sar_fields_empty() {
        let metrics = AggregateMetrics::from_runs(&[]);
        assert_eq!(metrics.avg_time_to_find, 0.0);
        assert_eq!(metrics.avg_probability_of_detection, 0.0);
        assert_eq!(metrics.avg_targets_found, 0.0);
    }

    #[test]
    fn percentile_calculation_p50_p95() {
        let sorted = vec![10u64, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let p50 = percentile_of_sorted(&sorted, 50.0);
        let p95 = percentile_of_sorted(&sorted, 95.0);
        // p50 of 10 elements ≈ sorted[4] = 50
        assert!((p50 - 50.0).abs() < 10.0, "p50={}", p50);
        // p95 of 10 elements ≈ sorted[8] = 90
        assert!((p95 - 90.0).abs() < 10.0, "p95={}", p95);
    }

    #[test]
    fn percentile_empty_returns_zero() {
        let result = percentile_of_sorted(&[], 50.0);
        assert_eq!(result, 0.0);
    }
}
