use std::collections::HashMap;
use std::path::Path;

use swarm_alloc::{
    AllocationAgent, AllocationTask, AuctionAllocator, CbbaAllocator, CentralizedPlanner,
    ConnectivityAwareAllocator, GreedyAllocator, Strategy,
};
use swarm_scenarios::{
    build_coverage_scenario, build_emergency_mesh_scenario, build_inspection_scenario,
    build_sar_scenario, build_wildfire_scenario, CoverageConfig, EmergencyMeshProfile,
    EmergencyMeshStandardProfiles, InspectionProfile, InspectionStandardProfiles, SarProfile,
    StandardProfiles, WildfireProfile,
};
use swarm_sim::{
    default_suites, export_csv, export_json, Baseline, BenchmarkHarness, BenchmarkOptions,
    ComparisonReport, FailureEvent, PartitionEvent, RegressionRunner, RunConfig, Scenario,
    SuiteMode,
};
use swarm_types::{AgentId, BatteryModel};

type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig) + Send + Sync>;
type StrategyFactory =
    Box<dyn Fn(&Scenario, &RunConfig) -> Box<dyn swarm_alloc::Strategy> + Send + Sync>;

#[derive(Clone, Copy)]
enum RunMode {
    Smoke, // 1 seed
    Quick, // 10 seeds (default)
    Full,  // 1000 seeds
}

#[derive(Clone)]
enum Mission {
    Coverage,
    EmergencyMesh,
    Sar,
    Inspection,
    Wildfire,
}

fn parse_mission(arg: &str) -> Vec<Mission> {
    match arg {
        "coverage" => vec![Mission::Coverage],
        "emergency-mesh" => vec![Mission::EmergencyMesh],
        "sar" => vec![Mission::Sar],
        "inspection" => vec![Mission::Inspection],
        "wildfire" => vec![Mission::Wildfire],
        "all" => vec![
            Mission::Coverage,
            Mission::EmergencyMesh,
            Mission::Sar,
            Mission::Inspection,
            Mission::Wildfire,
        ],
        _ => {
            panic!("unknown mission: {arg}. Valid: coverage, emergency-mesh, sar, inspection, wildfire, all")
        }
    }
}

fn mission_name(mission: &Mission) -> &'static str {
    match mission {
        Mission::Coverage => "coverage",
        Mission::EmergencyMesh => "emergency-mesh",
        Mission::Sar => "sar",
        Mission::Inspection => "inspection",
        Mission::Wildfire => "wildfire",
    }
}

#[derive(Clone)]
enum PlannerChoice {
    NearestNeighbour,
    TwoOpt,
    BatteryAware,
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
    report_path: Option<String>,
    /// Limit rayon parallelism; `None` uses all available CPUs.
    jobs: Option<usize>,
    /// Route planner for bundle ordering (CBBA only).
    planner: PlannerChoice,
    /// Run regression suites instead of normal benchmark.
    regression: bool,
    /// Compare current run against a baseline file.
    compare_baseline: Option<String>,
    /// Write current run as new baseline.
    update_baseline: Option<String>,
    /// Enable M31 realism preset (wind, pose noise, comms jitter, battery model v2).
    realism: bool,
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
        report_path: None,
        jobs: None,
        planner: PlannerChoice::NearestNeighbour,
        regression: false,
        compare_baseline: None,
        update_baseline: None,
        realism: false,
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
            "--report" => {
                i += 1;
                if i < args.len() {
                    cli.report_path = Some(args[i].clone());
                }
            }
            "--jobs" => {
                i += 1;
                if i < args.len() {
                    cli.jobs = args[i].parse::<usize>().ok();
                }
            }
            "--planner" => {
                i += 1;
                if i < args.len() {
                    cli.planner = match args[i].as_str() {
                        "nn" | "nearest-neighbour" => PlannerChoice::NearestNeighbour,
                        "two-opt" | "2opt" => PlannerChoice::TwoOpt,
                        "battery-aware" | "battery" => PlannerChoice::BatteryAware,
                        _ => {
                            eprintln!(
                                "Unknown planner '{}'. Valid: nn, two-opt, battery-aware",
                                args[i]
                            );
                            std::process::exit(1);
                        }
                    };
                }
            }
            "--regression" => cli.regression = true,
            "--realism" => cli.realism = true,
            "--compare-baseline" => {
                i += 1;
                if i < args.len() {
                    cli.compare_baseline = Some(args[i].clone());
                }
            }
            "--update-baseline" => {
                i += 1;
                if i < args.len() {
                    cli.update_baseline = Some(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    cli
}

fn make_cbba_allocator(planner: &PlannerChoice) -> CbbaAllocator {
    use swarm_alloc::route_planner::{BatteryAwarePlanner, NearestNeighbourPlanner, TwoOptPlanner};
    let mut cbba = CbbaAllocator::default();
    cbba.route_planner = match planner {
        PlannerChoice::NearestNeighbour => Box::new(NearestNeighbourPlanner),
        PlannerChoice::TwoOpt => Box::new(TwoOptPlanner::default()),
        PlannerChoice::BatteryAware => Box::new(BatteryAwarePlanner::default()),
    };
    cbba
}

fn make_factories(planner: &PlannerChoice) -> Vec<StrategyFactory> {
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
        Box::new({
            let planner = planner.clone();
            move |_scenario: &Scenario, _run_config: &RunConfig| {
                Box::new(make_cbba_allocator(&planner))
            }
        }),
    ]
}

/// Applies M31 realism preset to a (Scenario, RunConfig) pair.
fn apply_realism_preset(
    mut scenario: Scenario,
    mut run_config: RunConfig,
) -> (Scenario, RunConfig) {
    run_config.pose_noise_m = 0.5;
    run_config.wind = Some((0.1, 0.1, 0.0));
    run_config.comms_jitter_ticks = 1;
    let battery = BatteryModel {
        hover_drain_per_tick: 0.01,
        climb_drain_per_meter: 0.05,
        cruise_drain_per_meter: 0.02,
        reserve_fraction: 0.1,
    };
    for agent in &mut scenario.agents {
        if agent.battery_model.is_none() {
            agent.battery_model = Some(battery.clone());
        }
    }
    (scenario, run_config)
}

/// Wraps a ScenarioBuilder so that every produced pair passes through the realism preset.
fn with_realism(builder: ScenarioBuilder) -> ScenarioBuilder {
    Box::new(move |seed, profile| {
        let (scenario, run_config) = builder(seed, profile);
        apply_realism_preset(scenario, run_config)
    })
}

fn ensure_parent_dir(path: impl AsRef<Path>) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn write_file_creating_parent(
    path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    let path = path.as_ref();
    ensure_parent_dir(path)?;
    std::fs::write(path, contents)
}

fn main() {
    let cli = parse_args();

    if cli.regression {
        run_regression(&cli);
        return;
    }

    if let Some(suite_path) = &cli.scenario_suite_path {
        run_from_suite(suite_path, &cli);
        return;
    }

    let factories = make_factories(&cli.planner);

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
            Mission::Wildfire => {
                let profiles: Vec<String> =
                    vec!["small-static".to_owned(), "medium-dynamic".to_owned()];
                let builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
                    let profile = WildfireProfile::from_str(profile_name)
                        .unwrap_or(WildfireProfile::SmallStatic);
                    build_wildfire_scenario(&profile.config(seed))
                });
                (profiles, builder)
            }
        };

        let builder = if cli.realism {
            with_realism(builder)
        } else {
            builder
        };

        let mission_options = BenchmarkOptions {
            prefix: cli.run_id_prefix.as_deref(),
            enable_replay_log: enable_replay,
            mission_name: mname,
            jobs: cli.jobs,
        };
        let result = match cli.mode {
            RunMode::Smoke => BenchmarkHarness::run_smoke_with_options(
                &factories,
                &profile_names,
                &builder,
                mission_options,
            ),
            RunMode::Quick => BenchmarkHarness::run_quick_with_options(
                &factories,
                &profile_names,
                &builder,
                mission_options,
            ),
            RunMode::Full => BenchmarkHarness::run_full_with_options(
                &factories,
                &profile_names,
                &builder,
                mission_options,
            ),
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
        write_file_creating_parent(path, json).expect("Failed to write JSON file");
        println!("JSON report written to {}", path);
    }

    // Export CSV
    if let Some(path) = &cli.csv_path {
        let csv = export_csv(&merged).expect("CSV export failed");
        write_file_creating_parent(path, csv).expect("Failed to write CSV file");
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

    if let Some(dir) = &cli.output_dir {
        if let Err(e) = write_benchmark_pack(dir, &merged, None, &all_replay_logs, cli.realism) {
            eprintln!("Failed to write benchmark pack: {}", e);
            std::process::exit(1);
        }
    }

    if let Some(path) = &cli.report_path {
        let named_reports: Vec<(String, ComparisonReport)> = all_reports
            .iter()
            .map(|r| {
                let name = r
                    .mission_names
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "mission".to_owned());
                (name, r.clone())
            })
            .collect();
        let report_md = swarm_sim::generate_focused_report(&named_reports);
        write_file_creating_parent(path, report_md).expect("Failed to write report file");
        println!("Focused report written to {}", path);
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

    let factories = make_factories(&cli.planner);

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

        let (scenario, run_config) = if cli.realism {
            apply_realism_preset(entry.scenario.clone(), entry.run_config.clone())
        } else {
            (entry.scenario.clone(), entry.run_config.clone())
        };

        for factory in &factories {
            let mut strategy = factory(&scenario, &run_config);
            let strategy_name = strategy.name().to_owned();
            if !all_strategy_names.contains(&strategy_name) {
                all_strategy_names.push(strategy_name.clone());
            }
            let key = (strategy_name, profile_key.clone());
            let (metrics, _log) = swarm_sim::ScenarioRunner::run_with_log(
                &scenario,
                run_config.clone(),
                SuiteStrategyWrapper(&mut *strategy),
            );
            results.entry(key).or_default().push(metrics);
            total_runs += 1;
        }
    }

    let mut report_results: HashMap<(String, String), swarm_metrics::AggregateMetrics> =
        HashMap::new();
    for ((strategy, profile), runs) in &results {
        let mut metrics = swarm_metrics::AggregateMetrics::from_runs(runs);
        // Extract mission from "{mission}/{profile}" profile key used in suite mode
        let parts: Vec<&str> = profile.splitn(2, '/').collect();
        metrics.mission = parts.first().unwrap_or(&"").to_string();
        metrics.scenario = parts.get(1).unwrap_or(&"").to_string();
        report_results.insert((strategy.clone(), profile.clone()), metrics);
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
        write_file_creating_parent(path, json).expect("Failed to write JSON file");
        println!("JSON report written to {}", path);
    }

    if let Some(path) = &cli.csv_path {
        let csv = export_csv(&report).expect("CSV export failed");
        write_file_creating_parent(path, csv).expect("Failed to write CSV file");
        println!("CSV report written to {}", path);
    }

    if let Some(dir) = &cli.output_dir {
        if let Err(e) = write_benchmark_pack(dir, &report, Some(&suite), &[], cli.realism) {
            eprintln!("Failed to write benchmark pack: {}", e);
            std::process::exit(1);
        }
    }

    if let Some(path) = &cli.report_path {
        let mission_name = report
            .mission_names
            .first()
            .cloned()
            .unwrap_or_else(|| "suite".to_owned());
        let report_md = swarm_sim::generate_focused_report(&[(mission_name, report)]);
        write_file_creating_parent(path, report_md).expect("Failed to write report file");
        println!("Focused report written to {}", path);
    }
}

fn write_benchmark_pack(
    output_dir: &str,
    report: &ComparisonReport,
    suite: Option<&swarm_sim::ScenarioSuite>,
    replay_logs: &[swarm_replay::EventLog],
    realism: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(output_dir)?;

    let json = swarm_sim::export_json(report)?;
    std::fs::write(format!("{}/results.json", output_dir), json)?;

    let csv = swarm_sim::export_csv(report)?;
    std::fs::write(format!("{}/results.csv", output_dir), csv)?;

    let md = swarm_sim::export_markdown(report);
    std::fs::write(format!("{}/table.md", output_dir), md)?;

    let mut manifest = swarm_sim::BenchmarkManifest::new(
        report.mission_names.join(","),
        report.seed_range_start,
        report.seed_range_end,
        report.strategy_names.clone(),
        report.profile_names.clone(),
    );
    if realism {
        manifest.realism_profile = Some("default".to_owned());
        manifest.wind_enabled = true;
        manifest.pose_noise_m = 0.5;
        manifest.comms_jitter_ticks = 1;
    }
    std::fs::write(
        format!("{}/manifest.json", output_dir),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    if let Some(suite) = suite {
        let snapshot = swarm_sim::export_suite(suite)?;
        std::fs::write(format!("{}/scenario_snapshot.json", output_dir), snapshot)?;
    }

    if !replay_logs.is_empty() {
        let replay_dir = format!("{}/replay_logs", output_dir);
        std::fs::create_dir_all(&replay_dir)?;
        for (i, log) in replay_logs.iter().enumerate() {
            let path = format!("{}/replay_{}.json", replay_dir, i);
            let json = serde_json::to_string_pretty(log)?;
            std::fs::write(path, json)?;
        }
    }

    println!("Benchmark pack written to {}", output_dir);
    Ok(())
}

fn merge_reports(reports: &[swarm_sim::ComparisonReport]) -> swarm_sim::ComparisonReport {
    use std::collections::HashMap;
    let first = &reports[0];
    let mut merged_results: HashMap<(String, String), swarm_metrics::AggregateMetrics> =
        HashMap::new();
    for report in reports {
        for strategy_name in &report.strategy_names {
            for profile_name in &report.profile_names {
                let key = (strategy_name.clone(), profile_name.clone());
                if let Some(metrics) = report.results.get(&key) {
                    let scoped_profile = format!("{}/{}", metrics.mission, profile_name);
                    let scoped_key = (strategy_name.clone(), scoped_profile);
                    merged_results.insert(scoped_key, metrics.clone());
                }
            }
        }
    }
    // Collect unique mission-scoped profile names in original order across reports
    let mut all_profile_names = Vec::new();
    for r in reports {
        for name in &r.profile_names {
            for strategy_name in &r.strategy_names {
                let key = (strategy_name.clone(), name.clone());
                if let Some(metrics) = r.results.get(&key) {
                    let scoped = format!("{}/{}", metrics.mission, name);
                    if !all_profile_names.contains(&scoped) {
                        all_profile_names.push(scoped);
                    }
                }
            }
        }
    }
    swarm_sim::ComparisonReport {
        benchmark_run_id: swarm_sim::merged_benchmark_run_id(reports),
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
        profile_names: all_profile_names,
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

fn run_regression(cli: &CliArgs) {
    let baseline = cli
        .compare_baseline
        .as_ref()
        .and_then(|path| Baseline::load(path).ok());

    let suites = default_suites();
    let factories = make_factories(&cli.planner);

    let report = RegressionRunner::run(&suites, baseline.as_ref(), |suite| {
        let profile_names = vec![suite.profile.clone()];
        let builder = match suite.mission.as_str() {
            "coverage" => {
                Box::new(|seed: u64, profile_name: &str| build_coverage_profile(seed, profile_name))
                    as ScenarioBuilder
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
                    jobs: cli.jobs,
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
                    jobs: cli.jobs,
                },
            ),
        };

        let mut metrics_map = HashMap::new();
        for (strategy_name, profile_name) in result.report.results.keys() {
            let key = (strategy_name.clone(), profile_name.clone());
            if let Some(metrics) = result.report.results.get(&key) {
                metrics_map.insert(strategy_name.clone(), metrics.clone());
            }
        }
        metrics_map
    });

    println!("{}", report);

    if let Some(path) = &cli.update_baseline {
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
        if let Err(e) = ensure_parent_dir(path) {
            eprintln!("Failed to create baseline parent directory: {}", e);
            std::process::exit(1);
        }
        if let Err(e) = baseline.save(path) {
            eprintln!("Failed to save baseline: {}", e);
            std::process::exit(1);
        }
        println!("Baseline saved to {}", path);
    }

    std::process::exit(if report.overall_pass { 0 } else { 1 });
}
