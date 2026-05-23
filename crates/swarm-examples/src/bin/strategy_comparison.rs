use std::collections::HashMap;
use std::path::Path;

use swarm_alloc::{
    AllocationAgent, AllocationTask, AuctionAllocator, CbbaAllocator, CentralizedPlanner,
    ConnectivityAwareAllocator, GreedyAllocator, Strategy,
};
use swarm_scenarios::{
    build_coverage_scenario, build_emergency_mesh_scenario, build_inspection_scenario,
    build_sar_scenario, CoverageConfig, EmergencyMeshProfile, EmergencyMeshStandardProfiles,
    InspectionProfile, InspectionStandardProfiles, SarProfile, StandardProfiles,
};
use swarm_sim::{
    export_csv, export_json, BenchmarkHarness, BenchmarkOptions, ComparisonReport, FailureEvent,
    PartitionEvent, RunConfig, Scenario,
};
use swarm_types::AgentId;

type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig)>;
type StrategyFactory = Box<dyn Fn(&Scenario, &RunConfig) -> Box<dyn swarm_alloc::Strategy>>;

#[derive(Clone, Copy)]
enum RunMode {
    Smoke,  // 1 seed
    Quick,  // 10 seeds (default)
    Full,   // 1000 seeds
}

#[derive(Clone)]
enum Mission {
    Coverage,
    EmergencyMesh,
    Sar,
    Inspection,
}

fn parse_mission(arg: &str) -> Vec<Mission> {
    match arg {
        "coverage" => vec![Mission::Coverage],
        "emergency-mesh" => vec![Mission::EmergencyMesh],
        "sar" => vec![Mission::Sar],
        "inspection" => vec![Mission::Inspection],
        "all" => vec![
            Mission::Coverage,
            Mission::EmergencyMesh,
            Mission::Sar,
            Mission::Inspection,
        ],
        _ => {
            panic!("unknown mission: {arg}. Valid: coverage, emergency-mesh, sar, inspection, all")
        }
    }
}

fn mission_name(mission: &Mission) -> &'static str {
    match mission {
        Mission::Coverage => "coverage",
        Mission::EmergencyMesh => "emergency-mesh",
        Mission::Sar => "sar",
        Mission::Inspection => "inspection",
    }
}

struct CliArgs {
    mode: RunMode,
    missions: Vec<Mission>,
    json_path: Option<String>,
    csv_path: Option<String>,
    replay_log_dir: Option<String>,
    run_id_prefix: Option<String>,
    scenario_suite_path: Option<String>,
    output_dir: Option<String>,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut cli = CliArgs {
        mode: RunMode::Quick,
        missions: vec![Mission::Coverage],
        json_path: None,
        csv_path: None,
        replay_log_dir: None,
        run_id_prefix: None,
        scenario_suite_path: None,
        output_dir: None,
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--smoke" => cli.mode = RunMode::Smoke,
            "--quick" => cli.mode = RunMode::Quick,
            "--full" => cli.mode = RunMode::Full,
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
            "--scenario-suite" => {
                i += 1;
                if i < args.len() {
                    cli.scenario_suite_path = Some(args[i].clone());
                }
            }
            "--output-dir" => {
                i += 1;
                if i < args.len() {
                    cli.output_dir = Some(args[i].clone());
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

    if let Some(suite_path) = &cli.scenario_suite_path {
        run_from_suite(suite_path, &cli);
        return;
    }

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
                let profiles = if matches!(cli.mode, RunMode::Full) {
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
            Mission::Inspection => {
                let profiles: Vec<String> = InspectionStandardProfiles::profile_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                let builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
                    let profile = InspectionProfile::from_str(profile_name)
                        .unwrap_or(InspectionProfile::Linear);
                    build_inspection_scenario(&profile.config(seed))
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

fn run_from_suite(suite_path: &str, cli: &CliArgs) {
    use swarm_alloc::Allocator;

    struct SuiteStrategyWrapper<'a>(&'a mut dyn Strategy);
    impl<'a> Allocator for SuiteStrategyWrapper<'a> {
        fn allocate(
            &mut self,
            tasks: &[swarm_alloc::AllocationTask<'_>],
            agents: &[swarm_alloc::AllocationAgent],
        ) -> Vec<(swarm_types::TaskId, swarm_types::AgentId)> {
            self.0.allocate(tasks, agents)
        }

        fn allocate_with_connectivity(
            &mut self,
            tasks: &[swarm_alloc::AllocationTask<'_>],
            agents: &[swarm_alloc::AllocationAgent],
            connectivity: &swarm_alloc::ConnectivityContext,
        ) -> Vec<(swarm_types::TaskId, swarm_types::AgentId)> {
            self.0
                .allocate_with_connectivity(tasks, agents, connectivity)
        }
    }

    let suite = match swarm_sim::load_scenario_suite(suite_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error loading {}: {}", suite_path, e);
            std::process::exit(1);
        }
    };

    let errors = swarm_sim::validate_scenario_suite(&suite);
    if !errors.is_empty() {
        eprintln!("Validation failed for {}:", suite_path);
        for err in &errors {
            eprintln!("  [{}] {}", err.field, err.message);
        }
        std::process::exit(1);
    }

    println!(
        "Loaded suite: {} ({} entries)",
        suite.name,
        suite.scenarios.len()
    );

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

    let mut results: HashMap<(String, String), Vec<swarm_metrics::RunMetrics>> = HashMap::new();
    let mut all_mission_names: Vec<String> = Vec::new();
    let mut all_profile_names: Vec<String> = Vec::new();
    let mut all_strategy_names: Vec<String> = Vec::new();
    let mut seed_min = u64::MAX;
    let mut seed_max = 0u64;
    let mut total_runs: u64 = 0;

    for entry in &suite.scenarios {
        let seed = entry.scenario.seed;
        seed_min = seed_min.min(seed);
        seed_max = seed_max.max(seed);
        if !all_mission_names.contains(&entry.mission) {
            all_mission_names.push(entry.mission.clone());
        }
        let profile_key = format!("{}/{}", entry.mission, entry.profile);
        if !all_profile_names.contains(&profile_key) {
            all_profile_names.push(profile_key.clone());
        }

        for factory in &factories {
            let mut strategy = factory(&entry.scenario, &entry.run_config);
            let strategy_name = strategy.name().to_owned();
            if !all_strategy_names.contains(&strategy_name) {
                all_strategy_names.push(strategy_name.clone());
            }
            let key = (strategy_name, profile_key.clone());
            let (metrics, _log) = swarm_sim::ScenarioRunner::run_with_log(
                &entry.scenario,
                entry.run_config.clone(),
                SuiteStrategyWrapper(&mut *strategy),
            );
            results.entry(key).or_default().push(metrics);
            total_runs += 1;
        }
    }

    let mut report_results: HashMap<(String, String), swarm_metrics::AggregateMetrics> =
        HashMap::new();
    for ((strategy, profile), runs) in &results {
        report_results.insert(
            (strategy.clone(), profile.clone()),
            swarm_metrics::AggregateMetrics::from_runs(runs),
        );
    }

    let report = ComparisonReport {
        benchmark_run_id: format!("suite_{}", suite.name),
        seed_range_start: seed_min,
        seed_range_end: seed_max,
        total_runs_per_cell: total_runs,
        mission_names: all_mission_names,
        scenario_names: suite
            .scenarios
            .iter()
            .map(|e| e.scenario.name.clone())
            .collect(),
        strategy_names: all_strategy_names,
        profile_names: all_profile_names,
        results: report_results,
    };

    println!("{}", report);

    if let Some(path) = &cli.json_path {
        let json = export_json(&report).expect("JSON export failed");
        std::fs::write(path, json).expect("Failed to write JSON file");
        println!("JSON report written to {}", path);
    }

    if let Some(path) = &cli.csv_path {
        let csv = export_csv(&report).expect("CSV export failed");
        std::fs::write(path, csv).expect("Failed to write CSV file");
        println!("CSV report written to {}", path);
    }
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
                    heal_at_tick: None,
                    agents: (a.clone(), b.clone()),
                });
            }
        }
    }

    (scenario, run_config)
}
