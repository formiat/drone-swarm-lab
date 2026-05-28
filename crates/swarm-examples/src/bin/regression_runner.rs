use swarm_alloc::{
    AllocationAgent, AllocationTask, AuctionAllocator, CbbaAllocator, CentralizedPlanner,
    ConnectivityAwareAllocator, GreedyAllocator,
};
use swarm_sim::{
    all_suites, default_suites, suites_by_group, Baseline, RegressionReport, RegressionRunner,
    RegressionSuite, RunConfig, Scenario, SuiteGroup,
};

use swarm_examples::regression_lib::{run_regression_suite, StrategyFactory};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug)]
struct CliOptions {
    compare_baseline: Option<String>,
    update_baseline: Option<String>,
    jobs: Option<usize>,
    list_suites: bool,
    suite_group: Option<SuiteGroup>,
    suite_name: Option<String>,
    format: OutputFormat,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            compare_baseline: None,
            update_baseline: None,
            jobs: None,
            list_suites: false,
            suite_group: None,
            suite_name: None,
            format: OutputFormat::Human,
        }
    }
}

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

fn usage() -> &'static str {
    "Usage: regression_runner [--list-suites] [--suite smoke|quick|experimental|validation] [--suite-name NAME] [--format human|json] [--compare-baseline PATH] [--update-baseline PATH] [--jobs N]"
}

fn next_arg(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_args(args: &[String]) -> Result<CliOptions, String> {
    let mut options = CliOptions::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--compare-baseline" => {
                options.compare_baseline = Some(next_arg(args, &mut i, "--compare-baseline")?);
            }
            "--update-baseline" => {
                options.update_baseline = Some(next_arg(args, &mut i, "--update-baseline")?);
            }
            "--jobs" => {
                let value = next_arg(args, &mut i, "--jobs")?;
                options.jobs = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("invalid --jobs value: {value}"))?,
                );
            }
            "--list-suites" => {
                options.list_suites = true;
            }
            "--suite" => {
                let value = next_arg(args, &mut i, "--suite")?;
                options.suite_group = Some(value.parse::<SuiteGroup>()?);
            }
            "--suite-name" => {
                options.suite_name = Some(next_arg(args, &mut i, "--suite-name")?);
            }
            "--format" => {
                let value = next_arg(args, &mut i, "--format")?;
                options.format = match value.as_str() {
                    "human" => OutputFormat::Human,
                    "json" => OutputFormat::Json,
                    _ => return Err(format!("invalid --format value: {value}")),
                };
            }
            "--json" => {
                options.format = OutputFormat::Json;
            }
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            unknown => {
                return Err(format!("unknown argument: {unknown}"));
            }
        }
        i += 1;
    }

    Ok(options)
}

fn selected_suites(options: &CliOptions) -> Vec<RegressionSuite> {
    let mut suites = if let Some(group) = options.suite_group {
        suites_by_group(group)
    } else {
        default_suites()
    };

    if let Some(name) = &options.suite_name {
        suites.retain(|suite| suite.name == *name);
    }

    suites
}

fn suites_for_listing(options: &CliOptions) -> Vec<RegressionSuite> {
    if options.suite_group.is_some() || options.suite_name.is_some() {
        selected_suites(options)
    } else {
        all_suites()
    }
}

fn print_suites(suites: &[RegressionSuite]) {
    for suite in suites {
        println!(
            "{} group={} mode={} mission={} profile={} strategy={} gating={}",
            suite.name,
            suite.group.as_str(),
            suite.mode.as_str(),
            suite.mission,
            suite.profile,
            suite.strategy,
            suite.group.is_gating()
        );
    }
}

fn emit_report(report: &RegressionReport, format: OutputFormat) -> Result<(), serde_json::Error> {
    match format {
        OutputFormat::Human => {
            println!("{report}");
            Ok(())
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(report)?);
            Ok(())
        }
    }
}

fn has_threshold_violations(report: &RegressionReport) -> bool {
    report
        .suite_results
        .iter()
        .any(|result| !result.violations.is_empty())
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    };

    if options.list_suites {
        let suites = suites_for_listing(&options);
        print_suites(&suites);
        std::process::exit(0);
    }

    let baseline = match &options.compare_baseline {
        Some(path) => match Baseline::load(path) {
            Ok(baseline) => Some(baseline),
            Err(error) => {
                eprintln!("Failed to load baseline {path}: {error}");
                std::process::exit(2);
            }
        },
        None => None,
    };

    let suites = selected_suites(&options);
    if suites.is_empty() && options.suite_name.is_some() {
        eprintln!("No regression suite matched the requested --suite-name");
        std::process::exit(2);
    }

    let factories = make_factories();
    let report = RegressionRunner::run(&suites, baseline.as_ref(), |suite| {
        run_regression_suite(suite, &factories, options.jobs).unwrap_or_else(|error| {
            eprintln!("Failed to run suite {}: {}", suite.name, error);
            std::process::exit(2);
        })
    });

    if let Err(error) = emit_report(&report, options.format) {
        eprintln!("Failed to serialize regression report: {error}");
        std::process::exit(1);
    }

    if let Some(path) = &options.update_baseline {
        if has_threshold_violations(&report) {
            eprintln!("Refusing to update baseline from a report with threshold violations");
            std::process::exit(1);
        }

        let suite_group = options
            .suite_group
            .map(SuiteGroup::as_str)
            .unwrap_or("default");
        let mut baseline = Baseline::from_report(&report, Some(suite_group));
        baseline.commit = current_commit();
        if let Err(e) = baseline.save(path) {
            eprintln!("Failed to save baseline: {e}");
            std::process::exit(1);
        }
        eprintln!("Baseline saved to {path}");
    }

    std::process::exit(if report.overall_pass { 0 } else { 1 });
}
