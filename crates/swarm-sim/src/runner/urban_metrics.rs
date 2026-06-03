use super::{RunMetrics, Scenario};

pub(super) fn finish_urban_run_metrics(
    mut metrics: RunMetrics,
    log_builder: Option<swarm_replay::EventLogBuilder>,
) -> (RunMetrics, Option<swarm_replay::EventLog>) {
    let event_log = log_builder.map(|builder| builder.build());
    if let Some(log) = &event_log {
        let trace = crate::urban_analysis::build_urban_route_trace(log);
        let separation = crate::urban_analysis::measure_urban_separation(
            &trace,
            crate::urban_analysis::URBAN_ANALYSIS_DEFAULT_SEPARATION_THRESHOLD_M,
        );
        metrics.urban_min_agent_separation_m = separation.min_separation_m;
        metrics.urban_separation_violation_count = separation.separation_violation_count;
        metrics.urban_route_conflict_count = separation.route_conflict_count;
    }
    (metrics, event_log)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn urban_patrol_metrics(
    scenario: &Scenario,
    total_ticks: u64,
    success: bool,
    urban_route_planned: bool,
    urban_route_length_m: f64,
    urban_route_risk_score: f64,
    urban_violation_count: u64,
    urban_patrol_completed: bool,
    urban_time_to_complete_loop: Option<u64>,
    urban_distance_travelled_m: f64,
    urban_route_efficiency: f64,
    unsupported_reason: Option<String>,
    urban_replan_count: u64,
    urban_wait_time_ticks: u64,
    urban_blocked_edge_count: u64,
    urban_replan_success_rate: f64,
    urban_unresolved_blockage_count: u64,
) -> RunMetrics {
    let agent_count = scenario.agents.len() as f64;
    let battery_min = scenario
        .agents
        .iter()
        .map(|agent| agent.battery)
        .fold(f64::INFINITY, f64::min);
    let battery_min = if battery_min.is_finite() {
        battery_min
    } else {
        0.0
    };
    let battery_avg = if agent_count > 0.0 {
        scenario
            .agents
            .iter()
            .map(|agent| agent.battery)
            .sum::<f64>()
            / agent_count
    } else {
        0.0
    };
    let coverage_progress = if urban_route_length_m > 0.0 {
        (urban_distance_travelled_m / urban_route_length_m).clamp(0.0, 1.0)
    } else if urban_patrol_completed {
        1.0
    } else {
        0.0
    };

    RunMetrics {
        seed: scenario.seed,
        total_ticks,
        messages_attempted: 0,
        messages_dropped: 0,
        detection_time_ticks: None,
        reallocation_time_ticks: None,
        max_task_unassigned_ticks: 0,
        all_tasks_assigned: urban_patrol_completed,
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
        coverage_progress,
        bytes_sent: 0,
        stale_state_age_ticks: 0,
        battery_margin_min: battery_min,
        battery_margin_avg: battery_avg,
        final_battery_min: battery_min,
        avg_distance_travelled: urban_distance_travelled_m,
        agents_exhausted: 0,
        total_distance_travelled: urban_distance_travelled_m,
        mission_completion_ticks: urban_time_to_complete_loop.unwrap_or(total_ticks),
        time_to_first_exhaustion: None,
        time_to_find: None,
        coverage_over_time: vec![coverage_progress],
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
        bundle_travel_distance: urban_distance_travelled_m,
        edge_coverage_rate: 0.0,
        missed_edges: 0,
        revisit_count: 0,
        route_efficiency: urban_route_efficiency,
        avg_route_length: urban_route_length_m,
        avg_wasted_travel: 0.0,
        avg_return_reserve: 0.0,
        infeasible_routes: 0,
        hazard_zones_mapped: 0,
        priority_updates: 0,
        final_avg_threat_level: 0.0,
        high_priority_zones_mapped: 0,
        time_to_map_first_high_risk: None,
        threat_level_over_time: vec![],
        zone_observations: 0,
        unsupported_reason,
        realism_profile: None,
        wind: None,
        urban_route_length_m,
        urban_route_risk_score,
        urban_route_planned,
        urban_violation_count,
        urban_route_completed: urban_patrol_completed,
        urban_patrol_completed,
        urban_time_to_complete_loop,
        urban_distance_travelled_m,
        urban_route_efficiency,
        urban_replan_count,
        bus_detected: false,
        time_to_detect_bus: None,
        false_positive_count: 0,
        distance_before_detection: 0.0,
        search_success_without_violation: false,
        urban_min_agent_separation_m: None,
        urban_separation_violation_count: 0,
        urban_route_conflict_count: 0,
        urban_wait_time_ticks,
        urban_blocked_edge_count,
        urban_replan_success_rate,
        urban_unresolved_blockage_count,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn urban_search_metrics(
    scenario: &Scenario,
    total_ticks: u64,
    bus_detected: bool,
    urban_route_planned: bool,
    urban_route_length_m: f64,
    urban_route_risk_score: f64,
    urban_violation_count: u64,
    time_to_detect_bus: Option<u64>,
    false_positive_count: u64,
    urban_distance_travelled_m: f64,
    urban_route_efficiency: f64,
    unsupported_reason: Option<String>,
) -> RunMetrics {
    let search_success_without_violation =
        bus_detected && urban_violation_count == 0 && unsupported_reason.is_none();
    let mut metrics = urban_patrol_metrics(
        scenario,
        total_ticks,
        search_success_without_violation,
        urban_route_planned,
        urban_route_length_m,
        urban_route_risk_score,
        urban_violation_count,
        false,
        None,
        urban_distance_travelled_m,
        urban_route_efficiency,
        unsupported_reason,
        0,
        0,
        0,
        0.0,
        0,
    );
    metrics.all_tasks_assigned = bus_detected;
    metrics.bus_detected = bus_detected;
    metrics.time_to_detect_bus = time_to_detect_bus;
    metrics.false_positive_count = false_positive_count;
    metrics.distance_before_detection = if bus_detected {
        urban_distance_travelled_m
    } else {
        0.0
    };
    metrics.search_success_without_violation = search_success_without_violation;
    metrics
}
