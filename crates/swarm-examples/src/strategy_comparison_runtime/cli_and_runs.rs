use std::collections::HashMap;
use std::path::Path;

use swarm_alloc::{
    AllocationAgent, AllocationTask, AuctionAllocator, CbbaAllocator, CentralizedPlanner,
    ConnectivityAwareAllocator, GreedyAllocator, Strategy,
};
use swarm_scenarios::{
    build_emergency_mesh_scenario, build_inspection_scenario, build_sar_scenario,
    build_urban_patrol_scenario, build_urban_search_scenario, build_wildfire_scenario,
    EmergencyMeshProfile, EmergencyMeshStandardProfiles, InspectionProfile,
    InspectionStandardProfiles, SarProfile, StandardProfiles, UrbanProfile, UrbanStandardProfiles,
    WildfireProfile,
};
use swarm_sim::{
    export_csv, export_json, Baseline, BenchmarkHarness, BenchmarkOptions, ComparisonReport,
    RegressionReport, RunConfig, Scenario,
};

use crate::regression_lib::build_coverage_profile;

use crate::realism::{apply_realism_preset, RealismProfile};

use super::urban_artifacts_and_tests::{
    merge_reports, run_regression, sanitize_artifact_id, write_urban_analysis_artifacts,
};

type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig) + Send + Sync>;
pub(super) type StrategyFactory =
    Box<dyn Fn(&Scenario, &RunConfig) -> Box<dyn swarm_alloc::Strategy> + Send + Sync>;

#[derive(Clone, Copy)]
pub(super) enum RunMode {
    Smoke,       // 1 seed
    Quick,       // 10 seeds (default)
    Full,        // 1000 seeds
    Custom(u64), // User-specified seed count
}

impl RunMode {
    fn seed_count(self) -> u64 {
        match self {
            Self::Smoke => 1,
            Self::Quick => 10,
            Self::Full => 1000,
            Self::Custom(seed_count) => seed_count,
        }
    }

    fn uses_full_profile_matrix(self) -> bool {
        matches!(self, Self::Full) || self.seed_count() > 10
    }
}

#[derive(Clone)]
pub(super) enum Mission {
    Coverage,
    EmergencyMesh,
    Sar,
    Inspection,
    Wildfire,
    UrbanPatrol,
    UrbanSearch,
}

fn parse_mission(arg: &str) -> Vec<Mission> {
    match arg {
        "coverage" => vec![Mission::Coverage],
        "emergency-mesh" => vec![Mission::EmergencyMesh],
        "sar" => vec![Mission::Sar],
        "inspection" => vec![Mission::Inspection],
        "wildfire" => vec![Mission::Wildfire],
        "urban-patrol" => vec![Mission::UrbanPatrol],
        "urban-search" => vec![Mission::UrbanSearch],
        "all" => vec![
            Mission::Coverage,
            Mission::EmergencyMesh,
            Mission::Sar,
            Mission::Inspection,
            Mission::Wildfire,
        ],
        _ => {
            panic!("unknown mission: {arg}. Valid: coverage, emergency-mesh, sar, inspection, wildfire, urban-patrol, urban-search, all")
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
        Mission::UrbanPatrol => "urban-patrol",
        Mission::UrbanSearch => "urban-search",
    }
}

#[derive(Clone)]
pub(super) enum PlannerChoice {
    NearestNeighbour,
    TwoOpt,
    BatteryAware,
}

pub(super) struct CliArgs {
    pub(super) mode: RunMode,
    pub(super) missions: Vec<Mission>,
    pub(super) json_path: Option<String>,
    pub(super) csv_path: Option<String>,
    pub(super) replay_log_dir: Option<String>,
    pub(super) run_id_prefix: Option<String>,
    pub(super) scenario_suite_path: Option<String>,
    pub(super) output_dir: Option<String>,
    pub(super) report_path: Option<String>,
    /// Limit rayon parallelism; `None` uses all available CPUs.
    pub(super) jobs: Option<usize>,
    /// Route planner for bundle ordering (CBBA only).
    pub(super) planner: PlannerChoice,
    /// Run regression suites instead of normal benchmark.
    pub(super) regression: bool,
    /// Compare current run against a baseline file.
    pub(super) compare_baseline: Option<String>,
    /// Write current run as new baseline.
    pub(super) update_baseline: Option<String>,
    /// Enable M31 realism preset (wind, pose noise, comms jitter, battery model v2).
    pub(super) realism: bool,
    /// Realism profile: light, medium, or heavy (default: medium).
    pub(super) realism_profile: Option<String>,
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
        realism_profile: None,
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--smoke" => cli.mode = RunMode::Smoke,
            "--quick" => cli.mode = RunMode::Quick,
            "--full" => cli.mode = RunMode::Full,
            "--seeds" => {
                i += 1;
                if i < args.len() {
                    let seed_count = args[i].parse::<u64>().unwrap_or_else(|_| {
                        eprintln!(
                            "Invalid --seeds value '{}'. Expected positive integer.",
                            args[i]
                        );
                        std::process::exit(1);
                    });
                    if seed_count == 0 {
                        eprintln!("Invalid --seeds value '0'. Expected positive integer.");
                        std::process::exit(1);
                    }
                    cli.mode = RunMode::Custom(seed_count);
                }
            }
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
            "--realism-profile" => {
                i += 1;
                if i < args.len() {
                    cli.realism_profile = Some(args[i].clone());
                }
            }
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

pub(super) fn make_factories(planner: &PlannerChoice) -> Vec<StrategyFactory> {
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

/// Wraps a ScenarioBuilder so that every produced pair passes through the realism preset.
fn with_realism(builder: ScenarioBuilder, profile: RealismProfile) -> ScenarioBuilder {
    Box::new(move |seed, profile_name| {
        let (scenario, run_config) = builder(seed, profile_name);
        apply_realism_preset(scenario, run_config, profile.clone())
    })
}

pub(super) fn ensure_parent_dir(path: impl AsRef<Path>) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn current_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|commit| commit.trim().to_owned())
        .unwrap_or_default()
}

pub(super) fn baseline_from_green_report(
    report: &RegressionReport,
    suite_group: &str,
) -> Result<Baseline, &'static str> {
    if report.has_threshold_violations() {
        return Err("threshold violations");
    }

    let mut baseline = Baseline::from_report(report, Some(suite_group));
    baseline.commit = current_commit();
    Ok(baseline)
}

fn write_file_creating_parent(
    path: impl AsRef<Path>,
    contents: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    let path = path.as_ref();
    ensure_parent_dir(path)?;
    std::fs::write(path, contents)
}

pub(crate) fn main() {
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
                let profiles = if cli.mode.uses_full_profile_matrix() {
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
                let profiles: Vec<String> = vec![
                    "small-static".to_owned(),
                    "medium-dynamic".to_owned(),
                    "large-static".to_owned(),
                    "high-threat-dynamic".to_owned(),
                ];
                let builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
                    let profile = WildfireProfile::from_str(profile_name)
                        .unwrap_or(WildfireProfile::SmallStatic);
                    build_wildfire_scenario(&profile.config(seed))
                });
                (profiles, builder)
            }
            Mission::UrbanPatrol => {
                let profiles: Vec<String> = UrbanStandardProfiles::patrol_profile_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                let builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
                    let profile = UrbanProfile::from_str(profile_name)
                        .unwrap_or(UrbanProfile::PatrolSmallBlock);
                    build_urban_patrol_scenario(&profile.config(seed))
                });
                (profiles, builder)
            }
            Mission::UrbanSearch => {
                let profiles: Vec<String> = UrbanStandardProfiles::search_profile_names()
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                let builder: ScenarioBuilder = Box::new(|seed: u64, profile_name: &str| {
                    let profile = UrbanProfile::from_str(profile_name)
                        .unwrap_or(UrbanProfile::SearchStaticBus);
                    build_urban_search_scenario(&profile.config(seed), profile)
                });
                (profiles, builder)
            }
        };

        let builder = if cli.realism {
            let profile = cli
                .realism_profile
                .as_deref()
                .and_then(RealismProfile::parse)
                .unwrap_or(RealismProfile::Medium);
            with_realism(builder, profile)
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
            RunMode::Custom(seed_count) => BenchmarkHarness::run_with_seed_count_with_options(
                &factories,
                &profile_names,
                &builder,
                seed_count,
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
        write_replay_logs_to_dir(dir, &all_replay_logs);
    }

    if let Some(dir) = &cli.output_dir {
        let profile_name = if cli.realism {
            cli.realism_profile
                .clone()
                .or_else(|| Some("medium".to_owned()))
        } else {
            None
        };
        if let Err(e) = write_benchmark_pack(
            dir,
            &merged,
            None,
            &all_replay_logs,
            profile_name.as_deref(),
            cli.jobs,
        ) {
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
    let mut all_replay_logs = Vec::new();
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
            let profile = cli
                .realism_profile
                .as_deref()
                .and_then(RealismProfile::parse)
                .unwrap_or(RealismProfile::Medium);
            apply_realism_preset(entry.scenario.clone(), entry.run_config.clone(), profile)
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
            let (metrics, log) = swarm_sim::ScenarioRunner::run_with_log(
                &scenario,
                run_config.clone(),
                SuiteStrategyWrapper(&mut *strategy),
            );
            if let Some(log) = log {
                all_replay_logs.push(log);
            }
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
        let profile_name = if cli.realism {
            cli.realism_profile
                .clone()
                .or_else(|| Some("medium".to_owned()))
        } else {
            None
        };
        if let Err(e) = write_benchmark_pack(
            dir,
            &report,
            Some(&suite),
            &all_replay_logs,
            profile_name.as_deref(),
            cli.jobs,
        ) {
            eprintln!("Failed to write benchmark pack: {}", e);
            std::process::exit(1);
        }
    }

    if let Some(dir) = &cli.replay_log_dir {
        write_replay_logs_to_dir(dir, &all_replay_logs);
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

pub(super) fn write_benchmark_pack(
    output_dir: &str,
    report: &ComparisonReport,
    suite: Option<&swarm_sim::ScenarioSuite>,
    replay_logs: &[swarm_replay::EventLog],
    realism_profile: Option<&str>,
    jobs: Option<usize>,
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
    manifest.jobs = jobs;
    if let Some(profile) = realism_profile {
        let params = RealismProfile::parse(profile)
            .unwrap_or(RealismProfile::Medium)
            .params();
        manifest.realism_profile = Some(profile.to_owned());
        manifest.wind_enabled = params.wind.is_some();
        manifest.pose_noise_m = params.pose_noise_m;
        manifest.comms_jitter_ticks = params.comms_jitter_ticks;
        manifest.battery_model = Some(params.battery);
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
        write_urban_analysis_artifacts(output_dir, replay_logs)?;
    }

    println!("Benchmark pack written to {}", output_dir);
    Ok(())
}

fn write_replay_logs_to_dir(dir: &str, replay_logs: &[swarm_replay::EventLog]) {
    let path = Path::new(dir);
    if !path.exists() {
        std::fs::create_dir_all(path).expect("Failed to create replay log directory");
    }
    for (index, log) in replay_logs.iter().enumerate() {
        let file_name = format!(
            "{index:03}_{}.replay.json",
            sanitize_artifact_id(&log.run_id)
        );
        let file_path = path.join(&file_name);
        swarm_replay::write_to_file(log, &file_path).expect("Failed to write replay log");
    }
    println!("Replay logs saved to {} ({} files)", dir, replay_logs.len());
}
