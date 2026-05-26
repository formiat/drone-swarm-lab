use std::collections::HashMap;

use swarm_alloc::{
    AllocationAgent, AllocationTask, AuctionAllocator, CbbaAllocator, CentralizedPlanner,
    ConnectivityAwareAllocator, GreedyAllocator,
};
use swarm_scenarios::{
    build_coverage_scenario, build_emergency_mesh_scenario, build_inspection_scenario,
    build_sar_scenario, CoverageConfig, EmergencyMeshProfile, InspectionProfile, SarProfile,
    StandardProfiles,
};
use swarm_sim::{
    default_suites, Baseline, BenchmarkHarness, BenchmarkOptions, RegressionRunner, RunConfig,
    Scenario, SuiteMode,
};
use swarm_types::AgentId;

type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig) + Send + Sync>;
type StrategyFactory =
    Box<dyn Fn(&Scenario, &RunConfig) -> Box<dyn swarm_alloc::Strategy> + Send + Sync>;

fn make_cbba_allocator() -> CbbaAllocator {
    use swarm_alloc::route_planner::NearestNeighbourPlanner;
    let mut cbba = CbbaAllocator::default();
    cbba.route_planner = Box::new(NearestNeighbourPlanner);
    cbba
}

fn make_factories() -> Vec<StrategyFactory> {
    vec![
        Box::new(|_scenario: &Scenario, _run_config: &RunConfig| Box::new(GreedyAllocator)),
        Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
            Box::new(AuctionAllocator::default())
        }),
        Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
            Box::new(ConnectivityAwareAllocator {
                base_allocator: AuctionAllocator::default(),
            })
        }),
        Box::new(|scenario: &Scenario, _run_config: &RunConfig| {
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
            Box::new(CentralizedPlanner::new(
                &allocation_tasks,
                &allocation_agents,
            ))
        }),
        Box::new(|_scenario: &Scenario, _run_config: &RunConfig| Box::new(make_cbba_allocator())),
    ]
}

fn build_coverage_profile(seed: u64, profile_name: &str) -> (Scenario, RunConfig) {
    let parts: Vec<&str> = profile_name.split('-').collect();
    let mut net_name_parts = Vec::new();
    let mut fail_name_parts = Vec::new();
    let fail_names: Vec<&str> = StandardProfiles::failure_profiles()
        .iter()
        .map(|f| f.name)
        .collect();

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
        let range = fail_profile.failure_tick_range;
        range.0 + (seed % (range.1.saturating_sub(range.0) + 1).max(1))
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

    run_config.latency_per_hop = net_profile.latency_per_hop;

    if fail_profile.failure_count == 0 {
        run_config.failures.clear();
    } else if fail_profile.failure_count > 1 {
        for i in 1..fail_profile.failure_count {
            let agent_id = AgentId::from(format!("agent-{}", i % 10));
            let range = fail_profile.failure_tick_range;
            let at_tick =
                range.0 + ((seed + i as u64) % (range.1.saturating_sub(range.0) + 1).max(1));
            if at_tick < run_config.max_ticks {
                run_config
                    .failures
                    .push(swarm_sim::FailureEvent { agent_id, at_tick });
            }
        }
    }

    if net_name == "partition-prone" {
        let group_a: Vec<AgentId> = (0..5)
            .map(|i| AgentId::from(format!("agent-{i}")))
            .collect();
        let group_b: Vec<AgentId> = (5..10)
            .map(|i| AgentId::from(format!("agent-{i}")))
            .collect();
        let partition_start = 10u64;
        let partition_end = 30u64;
        for a in &group_a {
            for b in &group_b {
                run_config.partition_events.push(swarm_sim::PartitionEvent {
                    at_tick: partition_start,
                    until_tick: Some(partition_end),
                    heal_at_tick: None,
                    agents: (a.clone(), b.clone()),
                });
            }
        }
    }

    (scenario, run_config)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut compare_baseline: Option<String> = None;
    let mut update_baseline: Option<String> = None;
    let mut jobs: Option<usize> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--compare-baseline" => {
                i += 1;
                if i < args.len() {
                    compare_baseline = Some(args[i].clone());
                }
            }
            "--update-baseline" => {
                i += 1;
                if i < args.len() {
                    update_baseline = Some(args[i].clone());
                }
            }
            "--jobs" => {
                i += 1;
                if i < args.len() {
                    jobs = args[i].parse::<usize>().ok();
                }
            }
            _ => {}
        }
        i += 1;
    }

    let baseline = compare_baseline
        .as_ref()
        .and_then(|path| Baseline::load(path).ok());

    let suites = default_suites();
    let factories = make_factories();

    let report = RegressionRunner::run(&suites, baseline.as_ref(), |suite| {
        let profile_names = vec![suite.profile.clone()];
        let builder: ScenarioBuilder = match suite.mission.as_str() {
            "coverage" => {
                Box::new(|seed: u64, profile_name: &str| build_coverage_profile(seed, profile_name))
            }
            "emergency-mesh" => Box::new(|seed: u64, profile_name: &str| {
                let profile = EmergencyMeshProfile::from_str(profile_name)
                    .unwrap_or(EmergencyMeshProfile::Ideal);
                build_emergency_mesh_scenario(&profile.config(seed))
            }),
            "sar" => Box::new(|seed: u64, profile_name: &str| {
                let profile = SarProfile::from_str(profile_name).unwrap_or(SarProfile::Ideal);
                build_sar_scenario(&profile.config(seed))
            }),
            "inspection" => Box::new(|seed: u64, profile_name: &str| {
                let profile =
                    InspectionProfile::from_str(profile_name).unwrap_or(InspectionProfile::Linear);
                build_inspection_scenario(&profile.config(seed))
            }),
            _ => Box::new(|_seed: u64, _profile: &str| {
                let scenario = Scenario {
                    name: "empty".to_owned(),
                    seed: 0,
                    agents: vec![],
                    tasks: vec![],
                    ground_nodes: vec![],
                    base_station: None,
                };
                let run_config = RunConfig {
                    max_ticks: 10,
                    ..Default::default()
                };
                (scenario, run_config)
            }),
        };

        let result = match suite.mode {
            SuiteMode::Smoke => BenchmarkHarness::run_smoke_with_options(
                &factories,
                &profile_names,
                &builder,
                BenchmarkOptions {
                    prefix: Some(&suite.name),
                    enable_replay_log: false,
                    mission_name: &suite.mission,
                    jobs,
                },
            ),
            SuiteMode::Quick => BenchmarkHarness::run_quick_with_options(
                &factories,
                &profile_names,
                &builder,
                BenchmarkOptions {
                    prefix: Some(&suite.name),
                    enable_replay_log: false,
                    mission_name: &suite.mission,
                    jobs,
                },
            ),
        };

        let mut metrics_map = HashMap::new();
        for (strategy_name, _profile_name) in result.report.results.keys() {
            let key = (strategy_name.clone(), suite.profile.clone());
            if let Some(metrics) = result.report.results.get(&key) {
                metrics_map.insert(strategy_name.clone(), metrics.clone());
            }
        }
        metrics_map
    });

    println!("{}", report);

    if let Some(path) = &update_baseline {
        let results: Vec<(String, swarm_metrics::AggregateMetrics)> = report
            .suite_results
            .iter()
            .map(|sr| {
                let key = if sr.suite.strategy == "all" {
                    format!("{}/{}", sr.suite.name, sr.actual_strategy)
                } else {
                    sr.suite.name.clone()
                };
                (key, sr.metrics.clone())
            })
            .collect();
        let mut baseline = Baseline::from_suites(&results);
        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
        {
            baseline.commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        }
        if let Err(e) = baseline.save(path) {
            eprintln!("Failed to save baseline: {}", e);
            std::process::exit(1);
        }
        println!("Baseline saved to {}", path);
    }

    std::process::exit(if report.overall_pass { 0 } else { 1 });
}
