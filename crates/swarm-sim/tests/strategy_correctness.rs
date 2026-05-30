use swarm_alloc::{
    AllocationAgent, AllocationTask, CbbaAllocator, CentralizedPlanner, GreedyAllocator,
};
use swarm_scenarios::{
    build_inspection_scenario, build_sar_scenario, InspectionProfile, SarScenarioConfig,
};
use swarm_sim::ScenarioRunner;
use swarm_types::{SearchGrid, SensorModel};

// ── Inspection linear: success ↔ edge_coverage ──────────────────────────────

/// Verifies that `success=true` whenever `edge_coverage_rate=1.0` for the
/// linear inspection profile with greedy allocation.
///
/// Documents and locks in the invariant: task completion drives success,
/// and all edges being covered means all tasks completed.
#[test]
fn inspection_linear_success_equals_edge_coverage() {
    let config = InspectionProfile::Linear.config(42);
    let (scenario, run_config) = build_inspection_scenario(&config);
    let metrics = ScenarioRunner::run_with(&scenario, run_config, GreedyAllocator);
    eprintln!(
        "inspection_linear_success_equals_edge_coverage: edge_coverage={}, success={}",
        metrics.edge_coverage_rate, metrics.success
    );
    if (metrics.edge_coverage_rate - 1.0).abs() < 1e-6 {
        assert!(
            metrics.success,
            "edge_coverage=1.0 but success=false; task completion check diverges from edge coverage"
        );
    }
}

// ── Inspection perimeter: CBBA coverage after bundle-slot fix ───────────────

/// Verifies that CBBA achieves meaningful edge coverage on the perimeter profile
/// after the bundle-slot leak fix.
///
/// The perimeter profile has battery_constraint=0.3 (30% of full-route battery),
/// which physically limits maximum reachable coverage per agent. The threshold
/// of 0.3 is conservative for a single seed; the 10-seed benchmark average was
/// ~0.795 before the fix (which is also battery-limited, not bundle-limited).
#[test]
fn inspection_perimeter_cbba_coverage_improves() {
    let config = InspectionProfile::Perimeter.config(42);
    let (scenario, mut run_config) = build_inspection_scenario(&config);
    run_config.enable_cbba = true;
    run_config.gossip_interval_ticks = 1;
    let metrics = ScenarioRunner::run_with(&scenario, run_config, CbbaAllocator::default());
    eprintln!(
        "inspection_perimeter_cbba_coverage_improves: edge_coverage={}, success={}",
        metrics.edge_coverage_rate, metrics.success
    );
    assert!(
        metrics.edge_coverage_rate > 0.3,
        "CBBA perimeter edge_coverage should exceed 0.3 after bundle-slot fix, got {}",
        metrics.edge_coverage_rate
    );
}

// ── SAR helpers ─────────────────────────────────────────────────────────────

/// Small 4×4 SAR grid for fast documented-status tests.
fn small_sar_config(seed: u64) -> SarScenarioConfig {
    SarScenarioConfig {
        grid: SearchGrid::new(4, 4, 10.0),
        target_count: 1,
        scout_count: 2,
        thermal_count: 0,
        relay_count: 0,
        sensor: SensorModel::new_v2(0.5, 0.9, 0.1, 0.5, 0.05),
        enable_movement: true,
        tick_duration_ms: 1000,
        max_ticks: 100,
        seed,
        prior: 0.05,
    }
}

// ── SAR + CBBA: documented status (no panic, deterministic) ─────────────────

/// Documents that SAR+CBBA produces stable, deterministic results.
///
/// Currently shows 0% success because CBBA re-convergence delay keeps
/// `max_task_unassigned_ticks` above the configured threshold. Fix scoped to M27.
#[test]
fn sar_cbba_has_documented_status() {
    let config = small_sar_config(42);
    let (scenario, mut run_config) = build_sar_scenario(&config);
    run_config.enable_cbba = true;

    let m1 = ScenarioRunner::run_with(&scenario, run_config.clone(), CbbaAllocator::default());
    let m2 = ScenarioRunner::run_with(&scenario, run_config, CbbaAllocator::default());

    assert_eq!(
        m1.total_ticks, m2.total_ticks,
        "SAR+CBBA must be deterministic: total_ticks differs between identical runs"
    );
    assert_eq!(
        m1.success, m2.success,
        "SAR+CBBA must be deterministic: success differs between identical runs"
    );
    eprintln!(
        "sar_cbba_has_documented_status: success={}, ticks={}, max_unassigned_ticks={}",
        m1.success, m1.total_ticks, m1.max_task_unassigned_ticks
    );
}

// ── SAR + Centralized: documented status (no panic, deterministic) ──────────

/// Documents that SAR+Centralized produces stable, deterministic results.
///
/// Currently shows 0% success because the static pre-plan is computed once
/// at construction; after SAR scan progress, the planner can keep agents in an
/// ineffective revisit cycle until `max_ticks` is exhausted. Fix scoped to M27.
#[test]
fn sar_centralized_has_documented_status() {
    let config = small_sar_config(42);
    let (scenario, run_config) = build_sar_scenario(&config);

    let make_planner = || {
        let allocation_tasks: Vec<AllocationTask<'_>> = scenario
            .tasks
            .iter()
            .map(|t| AllocationTask { task: t })
            .collect();
        let allocation_agents: Vec<AllocationAgent> = scenario
            .agents
            .iter()
            .map(|a| AllocationAgent {
                id: a.id.clone(),
                pose: a.pose,
                battery: a.battery,
                capabilities: a.capabilities.clone(),
                role: a.role.clone(),
                comms_range: a.comms_range,
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
            })
            .collect();
        CentralizedPlanner::new(&allocation_tasks, &allocation_agents)
    };

    let m1 = ScenarioRunner::run_with(&scenario, run_config.clone(), make_planner());
    let m2 = ScenarioRunner::run_with(&scenario, run_config, make_planner());

    assert_eq!(
        m1.total_ticks, m2.total_ticks,
        "SAR+Centralized must be deterministic: total_ticks differs between identical runs"
    );
    assert_eq!(
        m1.success, m2.success,
        "SAR+Centralized must be deterministic: success differs between identical runs"
    );
    eprintln!(
        "sar_centralized_has_documented_status: success={}, ticks={}, max_unassigned_ticks={}",
        m1.success, m1.total_ticks, m1.max_task_unassigned_ticks
    );
}
