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
        task_completion_rate: if success { 1.0 } else { 0.0 },
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
        // v0.30 Wildfire Mapping metrics
        hazard_zones_mapped: 0,
        priority_updates: 0,
        final_avg_threat_level: 0.0,
        // v0.38 Wildfire v2
        high_priority_zones_mapped: 0,
        time_to_map_first_high_risk: None,
        threat_level_over_time: vec![],
        zone_observations: 0,
        // v0.35 Dynamic Mission Correctness
        unsupported_reason: None,
        // v0.37 Realism Scenario Pack
        realism_profile: None,
        wind: None,
        // v0.64 Urban Foundations
        urban_route_length_m: 0.0,
        urban_route_risk_score: 0.0,
        urban_route_planned: false,
        urban_violation_count: 0,
        urban_route_completed: false,
        // v0.65 Urban Patrol v0
        urban_patrol_completed: false,
        urban_time_to_complete_loop: None,
        urban_distance_travelled_m: 0.0,
        urban_route_efficiency: 0.0,
        urban_replan_count: 0,
        // v0.66 Urban Search v1
        bus_detected: false,
        time_to_detect_bus: None,
        false_positive_count: 0,
        distance_before_detection: 0.0,
        search_success_without_violation: false,
        urban_min_agent_separation_m: None,
        urban_separation_violation_count: 0,
        urban_route_conflict_count: 0,
        // v0.74 Urban Blocked-Route Decision Logic
        urban_wait_time_ticks: 0,
        urban_blocked_edge_count: 0,
        urban_replan_success_rate: 0.0,
        urban_unresolved_blockage_count: 0,
        urban_deconflict_conflict_count: 0,
        urban_deconflict_wait_ticks: 0,
        urban_deconflict_replan_count: 0,
        urban_deconflict_abort_count: 0,
        urban_segment_utilization: 0.0,
        urban_avg_delay_per_agent_ticks: 0.0,
        perimeter_completion_rate: 0.0,
        perimeter_length_m: 0.0,
        time_to_complete_perimeter: None,
        perimeter_violations: 0,
        gcs_lost_count: 0,
        gcs_lost_total_ticks: 0,
        neighbor_lost_count: 0,
        failsafe_rtl_count: 0,
        lease_expired_during_gcs_loss_count: 0,
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
fn aggregate_urban_search_fields() {
    let mut runs = vec![run(true, None), run(false, None)];
    runs[0].bus_detected = true;
    runs[0].time_to_detect_bus = Some(12);
    runs[0].false_positive_count = 1;
    runs[0].distance_before_detection = 24.0;
    runs[0].search_success_without_violation = true;
    runs[1].false_positive_count = 3;
    runs[1].distance_before_detection = 40.0;

    let metrics = AggregateMetrics::from_runs(&runs);

    assert_eq!(metrics.bus_detection_rate, 0.5);
    assert_eq!(metrics.avg_time_to_detect_bus, 12.0);
    assert_eq!(metrics.avg_false_positive_count, 2.0);
    assert_eq!(metrics.avg_distance_before_detection, 32.0);
    assert_eq!(metrics.search_success_without_violation_rate, 0.5);
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
    let metrics = AggregateMetrics::from_runs(&runs);

    assert!(
        (metrics.avg_task_completion_rate - 0.666_666_7).abs() < 1e-6,
        "Expected ~0.666667 for 2/3 completed runs, got {}",
        metrics.avg_task_completion_rate
    );
}

#[test]
fn aggregate_task_completion_stats_use_fractional_run_rates() {
    let mut runs = vec![run(true, None), run(true, None), run(false, None)];
    runs[0].all_tasks_assigned = true;
    runs[0].task_completion_rate = 1.0;
    runs[1].all_tasks_assigned = false;
    runs[1].task_completion_rate = 0.5;
    runs[2].all_tasks_assigned = false;
    runs[2].task_completion_rate = 0.0;

    let metrics = AggregateMetrics::from_runs(&runs);

    assert!((metrics.avg_task_completion_rate - 0.5).abs() < 1e-9);
    assert!((metrics.task_completion_stats.mean - 0.5).abs() < 1e-9);
    assert!((metrics.task_completion_stats.stddev - 0.5).abs() < 1e-9);
    assert!((metrics.task_completion_stats.stderr - 0.288_675_134_594_812_9).abs() < 1e-9);
    assert!((metrics.task_completion_stats.ci95_low - -0.065_803_263_805_833_3).abs() < 1e-9);
    assert!((metrics.task_completion_stats.ci95_high - 1.065_803_263_805_833_3).abs() < 1e-9);
    assert_eq!(metrics.task_completion_stats.min, 0.0);
    assert_eq!(metrics.task_completion_stats.max, 1.0);
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
fn aggregate_urban_fields() {
    let mut runs = vec![run(false, None), run(false, None)];
    runs[0].urban_route_planned = true;
    runs[0].urban_route_length_m = 40.0;
    runs[0].urban_route_risk_score = 12.0;
    runs[0].urban_violation_count = 0;
    runs[0].urban_route_completed = false;
    runs[0].urban_patrol_completed = false;
    runs[0].urban_distance_travelled_m = 40.0;
    runs[0].urban_route_efficiency = 1.0;
    runs[0].urban_replan_count = 0;
    runs[0].urban_min_agent_separation_m = Some(3.0);
    runs[0].urban_separation_violation_count = 1;
    runs[0].urban_route_conflict_count = 2;
    runs[1].urban_route_planned = true;
    runs[1].urban_route_length_m = 20.0;
    runs[1].urban_route_risk_score = 4.0;
    runs[1].urban_violation_count = 2;
    runs[1].urban_route_completed = true;
    runs[1].urban_patrol_completed = true;
    runs[1].urban_time_to_complete_loop = Some(12);
    runs[1].urban_distance_travelled_m = 20.0;
    runs[1].urban_route_efficiency = 1.0;
    runs[1].urban_replan_count = 1;
    runs[1].urban_min_agent_separation_m = Some(5.0);
    runs[1].urban_separation_violation_count = 3;
    runs[1].urban_route_conflict_count = 4;
    runs[0].perimeter_completion_rate = 1.0;
    runs[0].perimeter_length_m = 40.0;
    runs[0].time_to_complete_perimeter = Some(10);
    runs[0].perimeter_violations = 0;
    runs[1].perimeter_completion_rate = 0.5;
    runs[1].perimeter_length_m = 60.0;
    runs[1].time_to_complete_perimeter = Some(20);
    runs[1].perimeter_violations = 2;

    let metrics = AggregateMetrics::from_runs(&runs);

    assert_eq!(metrics.avg_urban_route_length_m, 30.0);
    assert_eq!(metrics.avg_urban_route_risk_score, 8.0);
    assert_eq!(metrics.urban_route_planned_rate, 1.0);
    assert_eq!(metrics.avg_urban_violation_count, 1.0);
    assert_eq!(metrics.urban_route_completed_rate, 0.5);
    assert_eq!(metrics.urban_patrol_completed_rate, 0.5);
    assert_eq!(metrics.avg_urban_time_to_complete_loop, 12.0);
    assert_eq!(metrics.avg_urban_distance_travelled_m, 30.0);
    assert_eq!(metrics.avg_urban_route_efficiency, 1.0);
    assert_eq!(metrics.avg_urban_replan_count, 0.5);
    assert_eq!(metrics.avg_urban_min_agent_separation_m, 4.0);
    assert_eq!(metrics.avg_urban_separation_violation_count, 2.0);
    assert_eq!(metrics.avg_urban_route_conflict_count, 3.0);
    assert_eq!(metrics.avg_perimeter_completion_rate, 0.75);
    assert_eq!(metrics.avg_perimeter_length_m, 50.0);
    assert_eq!(metrics.avg_time_to_complete_perimeter, 15.0);
    assert_eq!(metrics.avg_perimeter_violations, 1.0);
}

#[test]
fn urban_analysis_metric_fields_default_from_legacy_json() {
    let mut run_json = serde_json::to_value(run(true, None)).unwrap();
    let run_object = run_json.as_object_mut().unwrap();
    run_object.remove("urban_min_agent_separation_m");
    run_object.remove("urban_separation_violation_count");
    run_object.remove("urban_route_conflict_count");
    run_object.remove("perimeter_completion_rate");
    run_object.remove("perimeter_length_m");
    run_object.remove("time_to_complete_perimeter");
    run_object.remove("perimeter_violations");
    run_object.remove("urban_route_risk_score");

    let run_metrics: RunMetrics = serde_json::from_value(run_json).unwrap();
    assert_eq!(run_metrics.urban_route_risk_score, 0.0);
    assert_eq!(run_metrics.urban_min_agent_separation_m, None);
    assert_eq!(run_metrics.urban_separation_violation_count, 0);
    assert_eq!(run_metrics.urban_route_conflict_count, 0);
    assert_eq!(run_metrics.perimeter_completion_rate, 0.0);
    assert_eq!(run_metrics.perimeter_length_m, 0.0);
    assert_eq!(run_metrics.time_to_complete_perimeter, None);
    assert_eq!(run_metrics.perimeter_violations, 0);

    let mut aggregate_json =
        serde_json::to_value(AggregateMetrics::from_runs(&[run(true, None)])).unwrap();
    let aggregate_object = aggregate_json.as_object_mut().unwrap();
    aggregate_object.remove("avg_urban_min_agent_separation_m");
    aggregate_object.remove("avg_urban_separation_violation_count");
    aggregate_object.remove("avg_urban_route_conflict_count");
    aggregate_object.remove("avg_perimeter_completion_rate");
    aggregate_object.remove("avg_perimeter_length_m");
    aggregate_object.remove("avg_time_to_complete_perimeter");
    aggregate_object.remove("avg_perimeter_violations");
    aggregate_object.remove("avg_urban_route_risk_score");

    let aggregate_metrics: AggregateMetrics = serde_json::from_value(aggregate_json).unwrap();
    assert_eq!(aggregate_metrics.avg_urban_route_risk_score, 0.0);
    assert_eq!(aggregate_metrics.avg_urban_min_agent_separation_m, 0.0);
    assert_eq!(aggregate_metrics.avg_urban_separation_violation_count, 0.0);
    assert_eq!(aggregate_metrics.avg_urban_route_conflict_count, 0.0);
    assert_eq!(aggregate_metrics.avg_perimeter_completion_rate, 0.0);
    assert_eq!(aggregate_metrics.avg_perimeter_length_m, 0.0);
    assert_eq!(aggregate_metrics.avg_time_to_complete_perimeter, 0.0);
    assert_eq!(aggregate_metrics.avg_perimeter_violations, 0.0);
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
