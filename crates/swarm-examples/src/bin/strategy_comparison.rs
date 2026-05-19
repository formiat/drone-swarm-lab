use std::path::Path;

use swarm_alloc::{
    AllocationAgent, AllocationTask, AuctionAllocator, CbbaAllocator, CentralizedPlanner,
    ConnectivityAwareAllocator, GreedyAllocator,
};
use swarm_scenarios::{
    build_coverage_scenario, build_emergency_mesh_scenario, build_sar_scenario, CoverageConfig,
    EmergencyMeshProfile, EmergencyMeshStandardProfiles, SarProfile, StandardProfiles,
};
use swarm_sim::{
    export_csv, export_json, BenchmarkHarness, BenchmarkOptions, FailureEvent, PartitionEvent,
    RunConfig, Scenario,
};
use swarm_types::AgentId;

type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig)>;
type StrategyFactory = Box<dyn Fn(&Scenario, &RunConfig) -> Box<dyn swarm_alloc::Strategy>>;

#[derive(Clone)]
enum Mission {
    Coverage,
    EmergencyMesh,
    Sar,
}

fn parse_mission(arg: &str) -> Vec<Mission> {
    match arg {
        "coverage" => vec![Mission::Coverage],
        "emergency-mesh" => vec![Mission::EmergencyMesh],
        "sar" => vec![Mission::Sar],
        "all" => vec![Mission::Coverage, Mission::EmergencyMesh, Mission::Sar],
        _ => panic!("unknown mission: {arg}. Valid: coverage, emergency-mesh, sar, all"),
    }
}

fn mission_name(mission: &Mission) -> &'static str {
    match mission {
        Mission::Coverage => "coverage",
        Mission::EmergencyMesh => "emergency-mesh",
        Mission::Sar => "sar",
    }
}

struct CliArgs {
    full_mode: bool,
    missions: Vec<Mission>,
    json_path: Option<String>,
    csv_path: Option<String>,
    replay_log_dir: Option<String>,
    run_id_prefix: Option<String>,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut cli = CliArgs {
        full_mode: false,
        missions: vec![Mission::Coverage],
        json_path: None,
        csv_path: None,
        replay_log_dir: None,
        run_id_prefix: None,
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--full" => cli.full_mode = true,
            "--mission" => {
                i += 1;
                if i < args.len() {
                    cli.missions = parse_mission(&args[i]);
                }
            }
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
        Box::new(
            |_scenario: &Scenario, _run_config: &RunConfig| Box::new(CbbaAllocator::default()),
        ),
    ];

    let enable_replay = cli.replay_log_dir.is_some();

    let mut all_reports = Vec::new();
    let mut all_replay_logs = Vec::new();

    for mission in &cli.missions {
        let mname = mission_name(mission);

        let (profile_names, builder): (Vec<String>, ScenarioBuilder) = match mission {
            Mission::Coverage => {
                let profiles = if cli.full_mode {
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
                    vec![
                        "ideal-no-failures".to_owned(),
                        "ideal-single-failure".to_owned(),
                        "medium-loss-no-failures".to_owned(),
                        "medium-loss-single-failure".to_owned(),
                    ]
                };
                let builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
                    build_coverage_profile(seed, profile_name)
                });
                (profiles, builder)
            }
            Mission::EmergencyMesh => {
                let profiles: Vec<String> = EmergencyMeshStandardProfiles::profile_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                let builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
                    let profile = EmergencyMeshProfile::from_str(profile_name)
                        .unwrap_or(EmergencyMeshProfile::Ideal);
                    build_emergency_mesh_scenario(&profile.config(seed))
                });
                (profiles, builder)
            }
            Mission::Sar => {
                let profiles: Vec<String> = vec!["ideal".to_owned(), "standard".to_owned()];
                let builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
                    let profile = SarProfile::from_str(profile_name).unwrap_or(SarProfile::Ideal);
                    build_sar_scenario(&profile.config(seed))
                });
                (profiles, builder)
            }
        };

        let mission_options = BenchmarkOptions {
            prefix: cli.run_id_prefix.as_deref(),
            enable_replay_log: enable_replay,
            mission_name: mname,
        };
        let result = if cli.full_mode {
            BenchmarkHarness::run_full_with_options(
                &factories,
                &profile_names,
                &builder,
                mission_options,
            )
        } else {
            BenchmarkHarness::run_quick_with_options(
                &factories,
                &profile_names,
                &builder,
                mission_options,
            )
        };

        let mut report = result.report;
        report.mission_names = vec![mname.to_owned()];
        report.scenario_names = vec![mname.to_owned()];
        all_reports.push(report);
        all_replay_logs.extend(result.replay_logs);
    }

    // Merge all mission reports into one with 3-part key
    let merged = merge_reports(&all_reports);

    // Print and export
    println!("{}", merged);

    // Export JSON
    if let Some(path) = &cli.json_path {
        let json = export_json(&merged).expect("JSON export failed");
        std::fs::write(path, json).expect("Failed to write JSON file");
        println!("JSON report written to {}", path);
    }

    // Export CSV
    if let Some(path) = &cli.csv_path {
        let csv = export_csv(&merged).expect("CSV export failed");
        std::fs::write(path, csv).expect("Failed to write CSV file");
        println!("CSV report written to {}", path);
    }

    // Invariant: centralized should match or outperform greedy on ideal network (coverage mission)
    let ideal_key = ("centralized".to_owned(), "ideal-no-failures".to_owned());
    let greedy_key = ("greedy".to_owned(), "ideal-no-failures".to_owned());
    for report in &all_reports {
        if let (Some(centralized), Some(greedy)) = (
            report.results.get(&ideal_key),
            report.results.get(&greedy_key),
        ) {
            assert!(
                centralized.success_rate >= greedy.success_rate,
                "Centralized planner should outperform or match greedy on ideal network"
            );
            break;
        }
    }

    // Replay logs
    if let Some(dir) = &cli.replay_log_dir {
        let path = Path::new(dir);
        if !path.exists() {
            std::fs::create_dir_all(path).expect("Failed to create replay log directory");
        }
        for log in &all_replay_logs {
            let file_name = format!("{}.replay.json", log.run_id.replace('/', "_"));
            let file_path = path.join(&file_name);
            swarm_replay::write_to_file(log, &file_path).expect("Failed to write replay log");
        }
        println!(
            "Replay logs saved to {} ({} files)",
            dir,
            all_replay_logs.len()
        );
    }

    std::process::exit(0);
}

fn merge_reports(reports: &[swarm_sim::ComparisonReport]) -> swarm_sim::ComparisonReport {
    use std::collections::HashMap;
    let first = &reports[0];
    let mut merged_results: HashMap<(String, String), swarm_metrics::AggregateMetrics> =
        HashMap::new();
    for report in reports {
        let mission = report.mission_names.first().cloned().unwrap_or_default();
        for strategy_name in &report.strategy_names {
            for profile_name in &report.profile_names {
                let old_key = (strategy_name.clone(), profile_name.clone());
                let new_key = (
                    strategy_name.clone(),
                    format!("{}/{}", mission, profile_name),
                );
                if let Some(metrics) = report.results.get(&old_key) {
                    merged_results.insert(new_key, metrics.clone());
                }
            }
        }
    }
    swarm_sim::ComparisonReport {
        benchmark_run_id: first.benchmark_run_id.clone(),
        seed_range_start: reports
            .iter()
            .map(|r| r.seed_range_start)
            .min()
            .unwrap_or(0),
        seed_range_end: reports.iter().map(|r| r.seed_range_end).max().unwrap_or(0),
        total_runs_per_cell: first.total_runs_per_cell,
        mission_names: reports
            .iter()
            .flat_map(|r| r.mission_names.clone())
            .collect(),
        scenario_names: reports
            .iter()
            .flat_map(|r| r.scenario_names.clone())
            .collect(),
        strategy_names: first.strategy_names.clone(),
        profile_names: {
            let mut p = Vec::new();
            for r in reports {
                let mission = r.mission_names.first().cloned().unwrap_or_default();
                for name in &r.profile_names {
                    p.push(format!("{}/{}", mission, name));
                }
            }
            p
        },
        results: merged_results,
    }
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
                run_config.failures.push(FailureEvent { agent_id, at_tick });
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
                run_config.partition_events.push(PartitionEvent {
                    at_tick: partition_start,
                    until_tick: Some(partition_end),
                    agents: (a.clone(), b.clone()),
                });
            }
        }
    }

    (scenario, run_config)
}
