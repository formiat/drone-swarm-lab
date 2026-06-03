use swarm_alloc::{AuctionAllocator, GreedyAllocator};
use swarm_metrics::AggregateMetrics;
use swarm_scenarios::{build_dynamic_auction_scenario, DynamicAuctionConfig};
use swarm_sim::ScenarioRunner;

fn make_config(seed: u64) -> DynamicAuctionConfig {
    DynamicAuctionConfig {
        seed,
        agent_count: 10,
        initial_task_count: 8,
        dynamic_task_count: 10,
        dynamic_task_start_tick: 5,
        dynamic_task_interval_ticks: 3,
        task_expiry_ticks: 15,
        failure_tick: 5,
        packet_loss_rate: 0.1,
        latency_ticks: 1,
        timeout_ticks: 3,
        max_unassigned_ticks: 8,
        max_ticks: 200,
    }
}

fn main() {
    let greedy_runs: Vec<_> = (0..1000)
        .map(|seed| {
            let (scenario, run_config) = build_dynamic_auction_scenario(&make_config(seed));
            ScenarioRunner::run_with(&scenario, run_config, GreedyAllocator::default())
        })
        .collect();

    let auction_runs: Vec<_> = (0..1000)
        .map(|seed| {
            let (scenario, run_config) = build_dynamic_auction_scenario(&make_config(seed));
            ScenarioRunner::run_with(&scenario, run_config, AuctionAllocator::default())
        })
        .collect();

    let greedy_metrics = AggregateMetrics::from_runs(&greedy_runs);
    let auction_metrics = AggregateMetrics::from_runs(&auction_runs);

    println!("=== greedy ===");
    println!("{greedy_metrics}");
    println!("=== auction ===");
    println!("{auction_metrics}");

    if greedy_metrics.success_rate < 0.95 || auction_metrics.success_rate < 0.95 {
        std::process::exit(1);
    }
}
