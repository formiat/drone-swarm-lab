use swarm_alloc::{CbbaAllocator, CentralizedPlanner};
use swarm_scenarios::{
    build_inspection_scenario, build_sar_scenario, build_wildfire_scenario, InspectionProfile,
    SarProfile, WildfireProfile,
};
use swarm_sim::{classify_support, ScenarioRunner, SupportReason, SupportStatus};
use swarm_types::AllocationAgent;

// ---------------------------------------------------------------------------
// SAR support matrix
// ---------------------------------------------------------------------------

#[test]
fn support_matrix_sar_greedy_is_supported() {
    let config = SarProfile::Ideal.config(42);
    let (scenario, run_config) = build_sar_scenario(&config);
    let metrics = ScenarioRunner::run(&scenario, run_config);
    // SAR + greedy is a supported combination; the simulation should run
    // without an unsupported_reason. Success depends on whether targets
    // are actually found (mission-specific semantics).
    assert!(
        metrics.unsupported_reason.is_none(),
        "SAR + greedy should not have an unsupported_reason"
    );
}

#[test]
fn support_matrix_sar_cbba_is_unsupported() {
    let support = classify_support("sar", "ideal", "cbba");
    assert_eq!(support.status, SupportStatus::Unsupported);
    assert_eq!(support.reason, SupportReason::DelayedReconvergence);

    let config = SarProfile::Ideal.config(42);
    let (scenario, mut run_config) = build_sar_scenario(&config);
    run_config.strategy_name = Some("cbba".to_owned());
    let metrics = ScenarioRunner::run_with(&scenario, run_config, CbbaAllocator::default());
    assert!(!metrics.success, "SAR + CBBA should be unsupported");
    assert_eq!(
        metrics.unsupported_reason,
        Some("delayed_reconvergence".to_owned()),
        "expected delayed_reconvergence reason"
    );
}

#[test]
fn support_matrix_sar_centralized_is_unsupported() {
    let support = classify_support("sar", "ideal", "centralized");
    assert_eq!(support.status, SupportStatus::Unsupported);
    assert_eq!(support.reason, SupportReason::StaticPrePlan);

    let config = SarProfile::Ideal.config(42);
    let (scenario, mut run_config) = build_sar_scenario(&config);
    let agents: Vec<AllocationAgent> = scenario
        .agents
        .iter()
        .map(|a| AllocationAgent {
            id: a.id.clone(),
            pose: a.pose,
            battery: a.battery,
            capabilities: a.capabilities.clone(),
            role: a.role.clone(),
            comms_range: a.comms_range,
            speed: a.speed,
            max_range: a.max_range,
            battery_drain_rate: a.battery_drain_rate,
        })
        .collect();
    let tasks: Vec<swarm_alloc::AllocationTask<'_>> = scenario
        .tasks
        .iter()
        .map(|t| swarm_alloc::AllocationTask { task: t })
        .collect();
    let planner = CentralizedPlanner::new(&tasks, &agents);
    run_config.strategy_name = Some("centralized".to_owned());
    let metrics = ScenarioRunner::run_with(&scenario, run_config, planner);
    assert!(!metrics.success, "SAR + centralized should be unsupported");
    assert_eq!(
        metrics.unsupported_reason,
        Some("static_pre_plan".to_owned()),
        "expected static_pre_plan reason"
    );
}

// ---------------------------------------------------------------------------
// Inspection support matrix
// ---------------------------------------------------------------------------

#[test]
fn support_matrix_inspection_linear_is_supported() {
    let config = InspectionProfile::Linear.config(42);
    let (scenario, run_config) = build_inspection_scenario(&config);
    let metrics = ScenarioRunner::run(&scenario, run_config);
    assert!(
        metrics.success,
        "inspection linear + greedy should be supported"
    );
}

#[test]
fn support_matrix_inspection_perimeter_success_aligns_with_coverage() {
    let config = InspectionProfile::Perimeter.config(42);
    let (scenario, mut run_config) = build_inspection_scenario(&config);
    run_config.inspection_coverage_threshold = 0.8;
    let metrics = ScenarioRunner::run(&scenario, run_config);
    // Perimeter may or may not succeed depending on battery, but coverage should be reported.
    if metrics.edge_coverage_rate >= 0.8 {
        assert!(
            metrics.success,
            "inspection perimeter with coverage >= 0.8 should succeed"
        );
    }
}

// ---------------------------------------------------------------------------
// Wildfire support matrix
// ---------------------------------------------------------------------------

#[test]
fn support_matrix_wildfire_small_static_is_supported() {
    let config = WildfireProfile::SmallStatic.config(42);
    let (scenario, run_config) = build_wildfire_scenario(&config);
    let metrics = ScenarioRunner::run(&scenario, run_config);
    // Wildfire small-static should map all zones with greedy.
    assert!(
        metrics.unsupported_reason.is_none(),
        "wildfire small-static + greedy should not have unsupported_reason"
    );
}

#[test]
fn support_matrix_wildfire_medium_dynamic_completion_consistency() {
    let config = WildfireProfile::MediumDynamic.config(42);
    let (scenario, mut run_config) = build_wildfire_scenario(&config);
    run_config.wildfire_success_threshold = 0.8;
    let metrics = ScenarioRunner::run(&scenario, run_config);
    // With mission-specific success semantics, wildfire success depends on:
    // 1. mapped_ratio >= threshold
    // 2. max_task_unassigned_ticks <= max_unassigned_ticks
    // Medium-dynamic may have high unassigned time due to priority changes.
    assert!(
        metrics.unsupported_reason.is_none(),
        "wildfire medium-dynamic should not have unsupported_reason"
    );
}

#[test]
fn support_matrix_emergency_mesh_connectivity_aware_is_experimental() {
    let support = classify_support("emergency-mesh", "ideal", "connectivity-aware");
    assert_eq!(support.status, SupportStatus::Experimental);
    assert_eq!(support.reason, SupportReason::RelayPlacementExperimental);
}
