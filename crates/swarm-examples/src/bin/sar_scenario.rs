use swarm_scenarios::{build_sar_scenario, SarScenarioConfig};
use swarm_sim::ScenarioRunner;
use swarm_types::{SearchGrid, SensorModel};

fn main() {
    let config = SarScenarioConfig {
        grid: SearchGrid::new(6, 6, 10.0),
        target_count: 2,
        scout_count: 3,
        thermal_count: 1,
        relay_count: 1,
        sensor: SensorModel::new(0.6, 0.95, 0.2),
        enable_movement: true,
        tick_duration_ms: 1000,
        max_ticks: 300,
        seed: 42,
        prior: 0.05,
    };

    let (scenario, run_config) = build_sar_scenario(&config);

    let metrics = ScenarioRunner::run(&scenario, run_config);

    println!(
        "Targets found: {}/{}",
        metrics.targets_found, metrics.targets_total
    );
    println!("Time to first find: {:?}", metrics.time_to_find);
    println!(
        "Final coverage: {:.2}",
        metrics.coverage_over_time.last().unwrap_or(&0.0)
    );
    println!("PoD: {:.2}", metrics.probability_of_detection);
    println!("Total ticks: {}", metrics.total_ticks);
    println!("Success: {}", metrics.success);

    assert!(
        metrics.targets_found > 0,
        "At least one target should be found"
    );
}
