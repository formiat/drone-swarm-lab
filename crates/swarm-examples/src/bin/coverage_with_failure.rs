use swarm_metrics::AggregateMetrics;
use swarm_scenarios::{build_coverage_scenario, CoverageConfig};
use swarm_sim::ScenarioRunner;

fn main() {
    let runs = (0..1000)
        .map(|seed| {
            let config = CoverageConfig {
                seed,
                agent_count: 10,
                task_count: 15,
                failure_tick: 5,
                packet_loss_rate: 0.1,
                latency_ticks: 1,
                timeout_ticks: 3,
                max_unassigned_ticks: 5,
                max_ticks: 200,
            };
            let (scenario, run_config) = build_coverage_scenario(&config);
            ScenarioRunner::run(&scenario, run_config)
        })
        .collect::<Vec<_>>();

    let metrics = AggregateMetrics::from_runs(&runs);
    println!("{metrics}");

    if metrics.success_rate < 0.99 {
        std::process::exit(1);
    }
}
