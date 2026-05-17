use swarm_alloc::{
    AuctionAllocator, ConnectivityAwareAllocator, GreedyAllocator, StrategyRegistry,
};
use swarm_scenarios::{build_coverage_scenario, CoverageConfig, StandardProfiles};
use swarm_sim::{BenchmarkHarness, FailureEvent, RunConfig, Scenario};
use swarm_types::AgentId;

type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig)>;

fn main() {
    let mut registry = StrategyRegistry::new();
    registry.register(Box::new(GreedyAllocator));
    registry.register(Box::new(AuctionAllocator::default()));
    registry.register(Box::new(ConnectivityAwareAllocator {
        base_allocator: AuctionAllocator::default(),
    }));

    // For quick benchmark, use a reduced matrix of key profiles
    let profile_names: Vec<String> = vec![
        "ideal-no-failures".to_owned(),
        "ideal-single-failure".to_owned(),
        "medium-loss-no-failures".to_owned(),
        "medium-loss-single-failure".to_owned(),
    ];

    let scenario_builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
        // Parse profile name like "ideal-no-failures" or "heavy-loss-cascade-failure"
        let parts: Vec<&str> = profile_name.split('-').collect();
        // Find where network profile ends and failure profile begins
        // This is a simple heuristic: join from the end until we match a failure profile name
        let mut net_name_parts = Vec::new();
        let mut fail_name_parts = Vec::new();
        let fail_names: Vec<&str> = StandardProfiles::failure_profiles()
            .iter()
            .map(|f| f.name)
            .collect();
        let _net_names: Vec<&str> = StandardProfiles::network_profiles()
            .iter()
            .map(|n| n.name)
            .collect();

        // Try to find the split point
        let mut found_split = false;
        for i in 0..parts.len() {
            let candidate_fail: String = parts[i..].join("-");
            if fail_names.contains(&candidate_fail.as_str()) {
                net_name_parts = parts[..i].to_vec();
                fail_name_parts = parts[i..].to_vec();
                found_split = true;
                break;
            }
        }

        if !found_split {
            // Fallback: assume everything is network profile
            net_name_parts = parts.clone();
            fail_name_parts = vec!["no-failures"];
        }

        let net_name = net_name_parts.join("-");
        let fail_name = fail_name_parts.join("-");

        let net_profile = StandardProfiles::network_profiles()
            .into_iter()
            .find(|p| p.name == net_name)
            .unwrap_or_else(|| StandardProfiles::network_profiles()[0].clone());
        let fail_profile = StandardProfiles::failure_profiles()
            .into_iter()
            .find(|p| p.name == fail_name)
            .unwrap_or_else(|| StandardProfiles::failure_profiles()[0].clone());

        let failure_tick = if fail_profile.failure_count > 0 {
            5 + (seed % 10)
        } else {
            999
        };

        let (scenario, mut run_config) = build_coverage_scenario(&CoverageConfig {
            seed,
            agent_count: 10,
            task_count: 15,
            failure_tick,
            packet_loss_rate: net_profile.packet_loss_rate,
            latency_ticks: net_profile.latency_ticks,
            timeout_ticks: 3,
            max_unassigned_ticks: 10,
            max_ticks: 200,
        });

        if fail_profile.failure_count == 0 {
            run_config.failures.clear();
        } else if fail_profile.failure_count > 1 {
            for i in 1..fail_profile.failure_count {
                let agent_id = AgentId::from(format!("agent-{}", i % 10));
                let at_tick = failure_tick + (i as u64) * 5;
                if at_tick < run_config.max_ticks {
                    run_config.failures.push(FailureEvent { agent_id, at_tick });
                }
            }
        }

        (scenario, run_config)
    });

    let report =
        BenchmarkHarness::run_quick(registry.strategies(), &profile_names, &scenario_builder);

    println!("{}", report);

    // Invariant: centralized should match or outperform greedy on ideal network
    let ideal_key = ("centralized".to_owned(), "ideal-no-failures".to_owned());
    let greedy_key = ("greedy".to_owned(), "ideal-no-failures".to_owned());

    if let (Some(centralized), Some(greedy)) = (
        report.results.get(&ideal_key),
        report.results.get(&greedy_key),
    ) {
        assert!(
            centralized.success_rate >= greedy.success_rate,
            "Centralized planner should outperform or match greedy on ideal network"
        );
    }

    std::process::exit(0);
}
