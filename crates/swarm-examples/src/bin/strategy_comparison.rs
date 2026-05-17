use std::path::Path;

use swarm_alloc::{
    AllocationAgent, AllocationTask, AuctionAllocator, CentralizedPlanner,
    ConnectivityAwareAllocator, GreedyAllocator,
};
use swarm_scenarios::{build_coverage_scenario, CoverageConfig, StandardProfiles};
use swarm_sim::{
    export_csv, export_json, BenchmarkHarness, BenchmarkOptions, FailureEvent, PartitionEvent,
    RunConfig, Scenario,
};
use swarm_types::AgentId;

type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig)>;
type StrategyFactory = Box<dyn Fn(&Scenario, &RunConfig) -> Box<dyn swarm_alloc::Strategy>>;

struct CliArgs {
    full_mode: bool,
    json_path: Option<String>,
    csv_path: Option<String>,
    replay_log_dir: Option<String>,
    run_id_prefix: Option<String>,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut cli = CliArgs {
        full_mode: false,
        json_path: None,
        csv_path: None,
        replay_log_dir: None,
        run_id_prefix: None,
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--full" => cli.full_mode = true,
            "--json" => {
                i += 1;
                if i < args.len() {
                    cli.json_path = Some(args[i].clone());
                }
            }
            "--csv" => {
                i += 1;
                if i < args.len() {
                    cli.csv_path = Some(args[i].clone());
                }
            }
            "--replay-log" => {
                i += 1;
                if i < args.len() {
                    cli.replay_log_dir = Some(args[i].clone());
                }
            }
            "--run-id-prefix" => {
                i += 1;
                if i < args.len() {
                    cli.run_id_prefix = Some(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    cli
}

fn main() {
    let cli = parse_args();

    // Register all 4 strategies using factories for per-run construction
    let factories: Vec<StrategyFactory> = vec![
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
                })
                .collect();
            Box::new(CentralizedPlanner::new(
                &allocation_tasks,
                &allocation_agents,
            ))
        }),
    ];

    // Build profile names from StandardProfiles combinations
    let profile_names: Vec<String> = if cli.full_mode {
        let nets = StandardProfiles::network_profiles();
        let fails = StandardProfiles::failure_profiles();
        let mut combos = Vec::new();
        for net in &nets {
            for fail in &fails {
                combos.push(format!("{}-{}", net.name, fail.name));
            }
        }
        combos
    } else {
        // Quick mode: reduced matrix for fast CI/testing
        vec![
            "ideal-no-failures".to_owned(),
            "ideal-single-failure".to_owned(),
            "medium-loss-no-failures".to_owned(),
            "medium-loss-single-failure".to_owned(),
        ]
    };

    let scenario_builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
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
                    run_config.failures.push(FailureEvent { agent_id, at_tick });
                }
            }
        }

        // partition-prone profile: inject partition events
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
                    run_config.partition_events.push(PartitionEvent {
                        at_tick: partition_start,
                        until_tick: Some(partition_end),
                        agents: (a.clone(), b.clone()),
                    });
                }
            }
        }

        (scenario, run_config)
    });

    let enable_replay = cli.replay_log_dir.is_some();
    let options = BenchmarkOptions {
        prefix: cli.run_id_prefix.as_deref(),
        enable_replay_log: enable_replay,
    };

    let result = if cli.full_mode {
        BenchmarkHarness::run_full_with_options(
            &factories,
            &profile_names,
            &scenario_builder,
            options,
        )
    } else {
        BenchmarkHarness::run_quick_with_options(
            &factories,
            &profile_names,
            &scenario_builder,
            options,
        )
    };

    let report = result.report;
    println!("{}", report);

    // Export JSON
    if let Some(path) = &cli.json_path {
        let json = export_json(&report).expect("JSON export failed");
        std::fs::write(path, json).expect("Failed to write JSON file");
        println!("JSON report written to {}", path);
    }

    // Export CSV
    if let Some(path) = &cli.csv_path {
        let csv = export_csv(&report).expect("CSV export failed");
        std::fs::write(path, csv).expect("Failed to write CSV file");
        println!("CSV report written to {}", path);
    }

    // Replay log directory
    if let Some(dir) = &cli.replay_log_dir {
        let path = Path::new(dir);
        if !path.exists() {
            std::fs::create_dir_all(path).expect("Failed to create replay log directory");
        }
        for log in &result.replay_logs {
            let file_name = format!("{}.replay.json", log.run_id.replace('/', "_"));
            let file_path = path.join(&file_name);
            swarm_replay::write_to_file(log, &file_path).expect("Failed to write replay log");
        }
        println!(
            "Replay logs saved to {} ({} files)",
            dir,
            result.replay_logs.len()
        );
    }

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
