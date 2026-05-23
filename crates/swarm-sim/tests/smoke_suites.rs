use swarm_alloc::GreedyAllocator;
use swarm_sim::{load_scenario_suite, ScenarioRunner};

fn smoke_run(path: &str) {
    let abs = concat!(env!("CARGO_MANIFEST_DIR"), "/../../");
    let full_path = format!("{}{}", abs, path);
    let suite = load_scenario_suite(&full_path).unwrap();
    let entry = &suite.scenarios[0];
    let metrics =
        ScenarioRunner::run_with(&entry.scenario, entry.run_config.clone(), GreedyAllocator);
    // Smoke test: must not panic, must complete within max_ticks
    assert!(
        metrics.total_ticks > 0,
        "smoke run produced 0 ticks for: {}",
        path
    );
}

#[test]
fn smoke_coverage_safety() {
    smoke_run("scenarios/coverage.safety.json");
}

#[test]
fn smoke_sar_uncertain() {
    smoke_run("scenarios/sar.uncertain.json");
}

#[test]
fn smoke_sar_noisy() {
    smoke_run("scenarios/sar.noisy.json");
}

#[test]
fn smoke_cbba_stress() {
    smoke_run("scenarios/cbba_stress.json");
}

#[test]
fn smoke_inspection_linear() {
    smoke_run("scenarios/inspection.linear.json");
}
