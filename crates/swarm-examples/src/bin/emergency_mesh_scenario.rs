use swarm_alloc::ConnectivityAwareAllocator;
use swarm_scenarios::{build_emergency_mesh_scenario, EmergencyMeshConfig};
use swarm_sim::ScenarioRunner;
use swarm_types::Pose;

const SEED_COUNT: u64 = 1000;

fn main() {
    let mut all_metrics = Vec::new();

    for seed in 0..SEED_COUNT {
        let config = EmergencyMeshConfig {
            seed,
            scout_count: 4,
            relay_count: 2,
            ground_node_count: 2,
            base_pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            area_size: 20.0,
            comms_range: 15.0,
            failure_tick: 15,
            max_ticks: 80,
            timeout_ticks: 5,
            gossip_interval_ticks: 3,
            packet_loss_rate: 0.0,
        };

        let (scenario, run_config) = build_emergency_mesh_scenario(&config);
        let allocator = ConnectivityAwareAllocator {
            base_allocator: swarm_alloc::AuctionAllocator::default(),
        };
        let metrics = ScenarioRunner::run_with(&scenario, run_config, allocator);

        all_metrics.push(metrics);
    }

    let aggregate = swarm_metrics::AggregateMetrics::from_runs(&all_metrics);

    println!("=== Emergency Mesh Scenario ({} seeds) ===", SEED_COUNT);
    println!("{aggregate}");

    // Verify invariants
    let mut violations = Vec::new();

    let min_availability = all_metrics
        .iter()
        .map(|m| m.network_availability)
        .fold(f64::INFINITY, f64::min);
    if min_availability < 0.8 {
        violations.push(format!(
            "network_availability min {:.3} below threshold 0.8",
            min_availability
        ));
    }

    let relay_realloc_missing = all_metrics
        .iter()
        .filter(|m| m.relay_reallocation_ticks.is_none())
        .count();
    if relay_realloc_missing > (SEED_COUNT as usize / 10) {
        violations.push(format!(
            "relay_reallocation_ticks missing in {} / {} runs",
            relay_realloc_missing, SEED_COUNT
        ));
    }

    let failed_runs = all_metrics.iter().filter(|m| !m.success).count();
    if failed_runs > 0 {
        violations.push(format!("{} runs failed out of {}", failed_runs, SEED_COUNT));
    }

    if violations.is_empty() {
        println!("PASS: all invariants satisfied");
        std::process::exit(0);
    } else {
        for v in &violations {
            eprintln!("FAIL: {v}");
        }
        std::process::exit(1);
    }
}
