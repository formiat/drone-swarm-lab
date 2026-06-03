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
    // v0.30 Wildfire Mapping metrics
    #[serde(default)]
    pub hazard_zones_mapped: u64,
    #[serde(default)]
    pub priority_updates: u64,
    #[serde(default)]
    pub final_avg_threat_level: f64,
    // v0.38 Wildfire v2
    #[serde(default)]
    pub high_priority_zones_mapped: u64,
    #[serde(default)]
    pub time_to_map_first_high_risk: Option<u64>,
    #[serde(default)]
    pub threat_level_over_time: Vec<f64>,
    #[serde(default)]
    pub zone_observations: u64,
    // v0.35 Dynamic Mission Correctness
    #[serde(default)]
    pub unsupported_reason: Option<String>,
    // v0.37 Realism Scenario Pack
    #[serde(default)]
    pub realism_profile: Option<String>,
    #[serde(default)]
    pub wind: Option<(f64, f64, f64)>,
    // v0.64 Urban Foundations
    #[serde(default)]
    pub urban_route_length_m: f64,
    // v0.68 Urban Algorithm Depth
    #[serde(default)]
    pub urban_route_risk_score: f64,
    #[serde(default)]
    pub urban_route_planned: bool,
    #[serde(default)]
    pub urban_violation_count: u64,
    #[serde(default)]
    pub urban_route_completed: bool,
    // v0.65 Urban Patrol v0
    #[serde(default)]
    pub urban_patrol_completed: bool,
    #[serde(default)]
    pub urban_time_to_complete_loop: Option<u64>,
    #[serde(default)]
    pub urban_distance_travelled_m: f64,
    #[serde(default)]
    pub urban_route_efficiency: f64,
    #[serde(default)]
    pub urban_replan_count: u64,
    // v0.66 Urban Search v1
    #[serde(default)]
    pub bus_detected: bool,
    #[serde(default)]
    pub time_to_detect_bus: Option<u64>,
    #[serde(default)]
    pub false_positive_count: u64,
    #[serde(default)]
    pub distance_before_detection: f64,
    #[serde(default)]
    pub search_success_without_violation: bool,
    // v0.67 Urban Replay / Analysis
    #[serde(default)]
    pub urban_min_agent_separation_m: Option<f64>,
    #[serde(default)]
    pub urban_separation_violation_count: u64,
    #[serde(default)]
    pub urban_route_conflict_count: u64,
    // v0.74 Urban Blocked-Route Decision Logic
    #[serde(default)]
    pub urban_wait_time_ticks: u64,
    #[serde(default)]
    pub urban_blocked_edge_count: u64,
    #[serde(default)]
    pub urban_replan_success_rate: f64,
    #[serde(default)]
    pub urban_unresolved_blockage_count: u64,
    // v0.75 Urban Mission Realism Follow-up
    #[serde(default)]
    pub perimeter_completion_rate: f64,
    #[serde(default)]
    pub perimeter_length_m: f64,
    #[serde(default)]
    pub time_to_complete_perimeter: Option<u64>,
    #[serde(default)]
    pub perimeter_violations: u64,
}
