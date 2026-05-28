use std::collections::HashMap;

use swarm_alloc::{
    AllocationAgent, AllocationTask, AuctionAllocator, CbbaAllocator, CentralizedPlanner,
    ConnectivityAwareAllocator, GreedyAllocator,
};
use swarm_sim::{
    default_suites, Baseline, BenchmarkHarness, BenchmarkOptions, RegressionRunner, RunConfig,
    Scenario, SuiteMode,
};

use swarm_examples::regression_lib::{build_mission_scenario_builder, with_realism_if_needed};

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
        let mut builder = build_mission_scenario_builder(&suite.mission).unwrap_or_else(|| {
            eprintln!("Unknown mission: {}", suite.mission);
            std::process::exit(1);
        });
        builder = with_realism_if_needed(builder, suite);

        let profile_names = vec![suite.profile.clone()];
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
