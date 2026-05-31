use std::collections::HashMap;

use rayon::prelude::*;
use swarm_alloc::Strategy;
use swarm_metrics::AggregateMetrics;

use crate::{RunConfig, Scenario, ScenarioRunner};

/// Report produced by a benchmark run comparing strategies across profiles.
#[derive(Clone)]
pub struct ComparisonReport {
    pub benchmark_run_id: String,
    pub seed_range_start: u64,
    pub seed_range_end: u64,
    pub total_runs_per_cell: u64,
    pub mission_names: Vec<String>,
    pub scenario_names: Vec<String>,
    pub strategy_names: Vec<String>,
    pub profile_names: Vec<String>,
    pub results: HashMap<(String, String), AggregateMetrics>,
}

impl std::fmt::Display for ComparisonReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let seeds = format!("{}-{}", self.seed_range_start, self.seed_range_end);
        writeln!(
            f,
            "| Mission | Scenario | Strategy | Profile | Seeds | Success | Completion | Detection | Realloc | Coverage | Messages | Bytes | Conflicts | Stale | BatteryMin | BatteryAvg | Availability | TimeToFind | PoD | Targets | BeliefEntropy | FalsePosRate | ConfirmationScans | ConvP50 | ConvP95 | BundleDist | EdgeCoverage | MissedEdges | Revisits | RouteEfficiency | UrbanRouteLength | UrbanRisk | UrbanPlanned | UrbanViolations | UrbanCompleted | PatrolCompleted | TimeToLoop | UrbanDistance | UrbanEfficiency | UrbanReplans | BusDetected | TimeToBus | BusFalsePos | DistanceBeforeBus | SearchSuccessNoViolation |"
        )?;
        writeln!(
            f,
            "|---------|----------|----------|---------|-------|---------|------------|-----------|---------|----------|----------|-------|-----------|-------|------------|------------|--------------|-----------|-----|---------|---------------|--------------|-------------------|---------|---------|------------|--------------|-------------|----------|-----------------|------------------|-----------|--------------|-----------------|----------------|-----------------|------------|---------------|-----------------|--------------|-------------|-----------|-------------|-------------------|--------------------------|"
        )?;
        for strategy_name in &self.strategy_names {
            for profile_name in &self.profile_names {
                let key = (strategy_name.clone(), profile_name.clone());
                if let Some(metrics) = self.results.get(&key) {
                    let ttf = if metrics.avg_time_to_find > 0.0 {
                        format!("{:.1}", metrics.avg_time_to_find)
                    } else {
                        "-".to_owned()
                    };
                    writeln!(
                        f,
                        "| {:7} | {:8} | {:8} | {:7} | {:5} | {:7.3} | {:10.3} | {:9.3} | {:7.3} | {:8.3} | {:8.3} | {:5.0} | {:9.3} | {:5.0} | {:10.3} | {:10.3} | {:12.3} | {:>10} | {:3.3} | {:7.1} | {:13.3} | {:12.3} | {:17.3} | {:7.3} | {:7.3} | {:10.3} | {:12.3} | {:11.3} | {:8.3} | {:15.3} | {:16.3} | {:9.3} | {:12.3} | {:15.3} | {:14.3} | {:15.3} | {:10.3} | {:13.3} | {:15.3} | {:12.3} | {:11.3} | {:9.3} | {:11.3} | {:17.3} | {:24.3} |",
                        metrics.mission.as_str(),
                        metrics.scenario.as_str(),
                        strategy_name,
                        profile_name,
                        seeds,
                        metrics.success_rate,
                        metrics.avg_task_completion_rate,
                        metrics.avg_detection_ticks,
                        metrics.avg_reallocation_ticks,
                        metrics.avg_coverage_progress,
                        metrics.avg_messages_attempted,
                        metrics.avg_bytes_sent,
                        metrics.avg_conflicting_assignments,
                        metrics.avg_stale_state_age_ticks,
                        metrics.avg_battery_margin_min,
                        metrics.avg_battery_margin_avg,
                        metrics.avg_network_availability,
                        ttf,
                        metrics.avg_probability_of_detection,
                        metrics.avg_targets_found,
                        metrics.avg_belief_entropy_final,
                        metrics.avg_false_positive_rate,
                        metrics.avg_confirmation_scans,
                        metrics.convergence_ticks_p50,
                        metrics.convergence_ticks_p95,
                        metrics.avg_bundle_travel_distance,
                        // v0.16 Inspection metrics
                        metrics.avg_edge_coverage_rate,
                        metrics.avg_missed_edges,
                        metrics.avg_revisit_count,
                        metrics.avg_route_efficiency,
                        // v0.64 Urban Foundations metrics
                        metrics.avg_urban_route_length_m,
                        metrics.avg_urban_route_risk_score,
                        metrics.urban_route_planned_rate,
                        metrics.avg_urban_violation_count,
                        metrics.urban_route_completed_rate,
                        // v0.65 Urban Patrol v0 metrics
                        metrics.urban_patrol_completed_rate,
                        metrics.avg_urban_time_to_complete_loop,
                        metrics.avg_urban_distance_travelled_m,
                        metrics.avg_urban_route_efficiency,
                        metrics.avg_urban_replan_count,
                        // v0.66 Urban Search v1 metrics
                        metrics.bus_detection_rate,
                        metrics.avg_time_to_detect_bus,
                        metrics.avg_false_positive_count,
                        metrics.avg_distance_before_detection,
                        metrics.search_success_without_violation_rate,
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// A function that builds a (Scenario, RunConfig) pair from a seed and profile name.
pub type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig) + Send + Sync>;

/// A function that creates a strategy for a given scenario.
pub type StrategyFactory = Box<dyn Fn(&Scenario, &RunConfig) -> Box<dyn Strategy> + Send + Sync>;

/// Options for running a benchmark.
pub struct BenchmarkOptions<'a> {
    pub prefix: Option<&'a str>,
    pub enable_replay_log: bool,
    pub mission_name: &'a str,
    /// Number of rayon threads; `None` or `Some(0)` uses all available CPUs.
    pub jobs: Option<usize>,
}

impl Default for BenchmarkOptions<'_> {
    fn default() -> Self {
        Self {
            prefix: None,
            enable_replay_log: false,
            mission_name: "coverage",
            jobs: None,
        }
    }
}

/// Result of a benchmark run, optionally including replay logs.
pub struct BenchmarkResult {
    pub report: ComparisonReport,
    pub replay_logs: Vec<swarm_replay::EventLog>,
}

/// Harness that runs strategies across seeds and profiles.
pub struct BenchmarkHarness;

impl BenchmarkHarness {
    /// Run a minimal smoke benchmark (1 seed).
    pub fn run_smoke(
        strategies: &[StrategyFactory],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
    ) -> ComparisonReport {
        Self::run_with_seeds(strategies, profile_names, scenario_builder, 0..1, None).report
    }

    /// Run a smoke benchmark with options.
    pub fn run_smoke_with_options(
        strategies: &[StrategyFactory],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
        options: BenchmarkOptions,
    ) -> BenchmarkResult {
        Self::run_with_seeds(
            strategies,
            profile_names,
            scenario_builder,
            0..1,
            Some(options),
        )
    }

    /// Run a small benchmark for CI/testing (10 seeds).
    pub fn run_quick(
        strategies: &[StrategyFactory],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
    ) -> ComparisonReport {
        Self::run_with_seeds(strategies, profile_names, scenario_builder, 0..10, None).report
    }

    /// Run a small benchmark with options.
    pub fn run_quick_with_options(
        strategies: &[StrategyFactory],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
        options: BenchmarkOptions,
    ) -> BenchmarkResult {
        Self::run_with_seeds(
            strategies,
            profile_names,
            scenario_builder,
            0..10,
            Some(options),
        )
    }

    /// Run a full benchmark (1000 seeds).
    pub fn run_full(
        strategies: &[StrategyFactory],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
    ) -> ComparisonReport {
        Self::run_with_seeds(strategies, profile_names, scenario_builder, 0..1000, None).report
    }

    /// Run a full benchmark with options.
    pub fn run_full_with_options(
        strategies: &[StrategyFactory],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
        options: BenchmarkOptions,
    ) -> BenchmarkResult {
        Self::run_with_seeds(
            strategies,
            profile_names,
            scenario_builder,
            0..1000,
            Some(options),
        )
    }

    /// Run a benchmark with a custom number of seeds starting from 0.
    pub fn run_with_seed_count_with_options(
        strategies: &[StrategyFactory],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
        seed_count: u64,
        options: BenchmarkOptions,
    ) -> BenchmarkResult {
        Self::run_with_seeds(
            strategies,
            profile_names,
            scenario_builder,
            0..seed_count,
            Some(options),
        )
    }

    fn run_with_seeds(
        strategies: &[StrategyFactory],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
        seeds: std::ops::Range<u64>,
        options: Option<BenchmarkOptions>,
    ) -> BenchmarkResult {
        let opts = options.unwrap_or_default();
        let benchmark_run_id =
            generate_benchmark_run_id(seeds.start, seeds.end, opts.mission_name, opts.prefix);
        let enable_replay_log = opts.enable_replay_log;
        let num_threads = opts.jobs.unwrap_or(0);

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .expect("failed to build rayon thread pool");

        /// value: `(seed, per-run (strategy+profile key, metrics) pairs, replay logs)`
        type SeedRow = (
            u64,
            Vec<((String, String), swarm_metrics::RunMetrics)>,
            Vec<swarm_replay::EventLog>,
        );

        // Run seeds in parallel; each element is (seed, per-run metrics, replay logs).
        let mut seed_results: Vec<SeedRow> = pool.install(|| {
            seeds
                .clone()
                .into_par_iter()
                .map(|seed| {
                    let mut local_metrics = Vec::new();
                    let mut local_logs = Vec::new();
                    for factory in strategies {
                        for profile_name in profile_names {
                            let (scenario, run_config) = scenario_builder(seed, profile_name);
                            let mut strategy = factory(&scenario, &run_config);
                            let strategy_name = strategy.name().to_owned();
                            let (metrics, log) = run_with_strategy(
                                &scenario,
                                run_config,
                                &mut *strategy,
                                enable_replay_log,
                            );
                            local_metrics.push(((strategy_name, profile_name.clone()), metrics));
                            if let Some(event_log) = log {
                                local_logs.push(event_log);
                            }
                        }
                    }
                    (seed, local_metrics, local_logs)
                })
                .collect()
        });

        // Sort by seed so aggregation order is identical regardless of thread count.
        seed_results.sort_unstable_by_key(|(seed, _, _)| *seed);

        let mut results: HashMap<(String, String), Vec<swarm_metrics::RunMetrics>> = HashMap::new();
        let mut replay_logs: Vec<swarm_replay::EventLog> = Vec::new();

        for (_seed, local_metrics, local_logs) in seed_results {
            for (key, metrics) in local_metrics {
                results.entry(key).or_default().push(metrics);
            }
            replay_logs.extend(local_logs);
        }

        let mut report_results = HashMap::new();
        let mut strategy_names = Vec::new();
        let mut report_profile_names = Vec::new();

        for ((strategy_name, profile_name), runs) in results {
            if !strategy_names.contains(&strategy_name) {
                strategy_names.push(strategy_name.clone());
            }
            if !report_profile_names.contains(&profile_name) {
                report_profile_names.push(profile_name.clone());
            }
            let mut metrics = AggregateMetrics::from_runs(&runs);
            metrics.mission = opts.mission_name.to_owned();
            metrics.scenario = opts.mission_name.to_owned();
            report_results.insert((strategy_name, profile_name), metrics);
        }

        // Sort for deterministic display and export ordering regardless of HashMap iteration order.
        strategy_names.sort();
        report_profile_names.sort();

        BenchmarkResult {
            report: ComparisonReport {
                benchmark_run_id,
                seed_range_start: seeds.start,
                seed_range_end: seeds.end,
                total_runs_per_cell: seeds.end.saturating_sub(seeds.start),
                mission_names: vec![],
                scenario_names: vec![],
                strategy_names,
                profile_names: report_profile_names,
                results: report_results,
            },
            replay_logs,
        }
    }
}

fn generate_benchmark_run_id(
    start_seed: u64,
    end_seed: u64,
    scenario_name: &str,
    prefix: Option<&str>,
) -> String {
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H%M%SZ");
    let seed_count = end_seed - start_seed;
    let mode = if seed_count <= 1 {
        "smoke"
    } else if seed_count <= 10 {
        "quick"
    } else if seed_count >= 1000 {
        "full"
    } else {
        "custom"
    };
    if let Some(p) = prefix {
        format!(
            "{}_{}_{}_{}_{}",
            p,
            timestamp,
            scenario_name,
            end_seed - start_seed,
            mode
        )
    } else {
        format!(
            "{}_{}_{}_{}",
            timestamp,
            scenario_name,
            end_seed - start_seed,
            mode
        )
    }
}

/// Generate a merged benchmark run id for `--mission all` mode.
/// Preserves prefix and timestamp from the first report, replaces mission with "all".
/// For a single report, returns the original id unchanged.
pub fn merged_benchmark_run_id(reports: &[ComparisonReport]) -> String {
    if reports.len() == 1 {
        return reports[0].benchmark_run_id.clone();
    }
    let first_id = &reports[0].benchmark_run_id;
    let parts: Vec<&str> = first_id.split('_').collect();

    // Detect prefix by checking if first part looks like a timestamp (contains 'T')
    let (prefix, timestamp) = if parts.len() >= 5 && !parts[0].contains('T') {
        // Has prefix: prefix_timestamp_mission_count_mode
        (Some(parts[0]), parts[1])
    } else if parts.len() >= 4 && parts[0].contains('T') {
        // No prefix: timestamp_mission_count_mode
        (None, parts[0])
    } else {
        // Unrecognized format: fallback to appending _all
        return format!("{}_all", first_id);
    };

    let seed_count = reports[0].total_runs_per_cell;
    let mode = if seed_count <= 1 {
        "smoke"
    } else if seed_count <= 10 {
        "quick"
    } else if seed_count >= 1000 {
        "full"
    } else {
        "custom"
    };

    if let Some(p) = prefix {
        format!("{}_{}_all_{}_{}", p, timestamp, seed_count, mode)
    } else {
        format!("{}_all_{}_{}", timestamp, seed_count, mode)
    }
}

fn run_with_strategy(
    scenario: &Scenario,
    mut run_config: RunConfig,
    strategy: &mut dyn Strategy,
    enable_log: bool,
) -> (swarm_metrics::RunMetrics, Option<swarm_replay::EventLog>) {
    struct StrategyWrapper<'a>(&'a mut dyn Strategy);
    impl<'a> swarm_alloc::Allocator for StrategyWrapper<'a> {
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

        fn allocation_metrics(&self) -> (u64, bool, u64) {
            self.0.allocation_metrics()
        }
    }

    // v0.35: Pass strategy name for support matrix detection.
    run_config.strategy_name = Some(strategy.name().to_owned());

    if enable_log {
        ScenarioRunner::run_with_log(scenario, run_config, StrategyWrapper(strategy))
    } else {
        (
            ScenarioRunner::run_with(scenario, run_config, StrategyWrapper(strategy)),
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_alloc::{AllocationAgent, AllocationTask, CentralizedPlanner, GreedyAllocator};
    use swarm_types::{Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskStatus};

    fn make_scenario_builder() -> ScenarioBuilder {
        Box::new(|seed: u64, _profile: &str| {
            let agents: Vec<Agent> = (0..5)
                .map(|i| Agent {
                    id: AgentId::from(format!("agent-{i}")),
                    role: Role::Scout,
                    health: Health::Alive,
                    pose: Pose {
                        x: 0.0,
                        y: 0.0,
                        ..Default::default()
                    },
                    capabilities: vec![],
                    current_task: None,
                    battery: 100.0,
                    comms_range: f64::INFINITY,
                    generation: 1,
                    speed: 0.0,
                    max_range: 0.0,
                    battery_drain_rate: 0.0,
                    battery_model: None,
                })
                .collect();
            let tasks: Vec<Task> = (0..5)
                .map(|i| Task {
                    id: TaskId::from(format!("task-{i}")),
                    status: TaskStatus::Unassigned,
                    assigned_to: None,
                    priority: 1,
                    required_capabilities: vec![],
                    required_role: None,
                    preferred_role: None,
                    expires_at: None,
                    grid_cell: None,
                    edge_id: None,
                    pose: None,
                    kind: None,
                })
                .collect();
            let scenario = Scenario {
                name: "test".to_owned(),
                seed,
                agents,
                tasks,
                ground_nodes: vec![],
                base_station: None,
            };
            let run_config = RunConfig {
                max_ticks: 50,
                timeout_ticks: 3,
                max_unassigned_ticks: 10,
                packet_loss_rate: 0.0,
                latency_ticks: 0,
                latency_per_hop: 0,
                failures: vec![],
                dynamic_tasks: vec![],
                partition_events: vec![],
                gossip_interval_ticks: 999,
                base_id: None,
                enable_movement: false,
                grid_state: None,
                tick_duration_ms: 100,
                enable_cbba: false,
                ..Default::default()
            };
            (scenario, run_config)
        })
    }

    #[test]
    fn harness_runs_and_produces_report() {
        let factories: Vec<StrategyFactory> =
            vec![Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
                Box::new(GreedyAllocator) as Box<dyn Strategy>
            })];
        let profiles = vec!["ideal".to_owned()];
        let builder = make_scenario_builder();
        let report = BenchmarkHarness::run_quick(&factories, &profiles, &builder);
        assert!(report
            .results
            .contains_key(&("greedy".to_owned(), "ideal".to_owned())));
    }

    #[test]
    fn centralized_present_in_report() {
        let factories: Vec<StrategyFactory> = vec![
            Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
                Box::new(GreedyAllocator) as Box<dyn Strategy>
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
                )) as Box<dyn Strategy>
            }),
        ];
        let profiles = vec!["ideal".to_owned()];
        let builder = make_scenario_builder();
        let report = BenchmarkHarness::run_quick(&factories, &profiles, &builder);
        assert!(report
            .results
            .contains_key(&("centralized".to_owned(), "ideal".to_owned())));
    }

    #[test]
    fn centralized_matches_or_beats_greedy_on_ideal() {
        let factories: Vec<StrategyFactory> = vec![
            Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
                Box::new(GreedyAllocator) as Box<dyn Strategy>
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
                )) as Box<dyn Strategy>
            }),
        ];
        let profiles = vec!["ideal".to_owned()];
        let builder = make_scenario_builder();
        let report = BenchmarkHarness::run_quick(&factories, &profiles, &builder);

        let greedy_key = ("greedy".to_owned(), "ideal".to_owned());
        let centralized_key = ("centralized".to_owned(), "ideal".to_owned());
        let greedy = report.results.get(&greedy_key).unwrap();
        let centralized = report.results.get(&centralized_key).unwrap();
        assert!(
            centralized.success_rate >= greedy.success_rate,
            "centralized ({}) should match or beat greedy ({}) on ideal network",
            centralized.success_rate,
            greedy.success_rate
        );
    }

    #[test]
    fn determinism_jobs_1_vs_4() {
        let factories: Vec<StrategyFactory> =
            vec![Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
                Box::new(GreedyAllocator) as Box<dyn Strategy>
            })];
        let profiles = vec!["ideal".to_owned()];
        let builder = make_scenario_builder();

        let run = |jobs: usize| {
            BenchmarkHarness::run_with_seeds(
                &factories,
                &profiles,
                &builder,
                0..10,
                Some(BenchmarkOptions {
                    jobs: Some(jobs),
                    ..Default::default()
                }),
            )
            .report
        };

        let r1 = run(1);
        let r4 = run(4);

        let key = ("greedy".to_owned(), "ideal".to_owned());
        let m1 = r1.results.get(&key).unwrap();
        let m4 = r4.results.get(&key).unwrap();
        assert_eq!(
            m1.success_rate, m4.success_rate,
            "success_rate must be identical for jobs=1 and jobs=4"
        );
        assert_eq!(
            m1.avg_task_completion_rate, m4.avg_task_completion_rate,
            "avg_task_completion_rate must be identical for jobs=1 and jobs=4"
        );
    }

    #[test]
    fn report_row_order_stable_across_jobs() {
        // Verifies that strategy_names and profile_names — and therefore the Display output —
        // are identical regardless of rayon thread count.
        let factories: Vec<StrategyFactory> = vec![
            Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
                Box::new(GreedyAllocator) as Box<dyn Strategy>
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
                )) as Box<dyn Strategy>
            }),
        ];
        let profiles = vec!["profile-a".to_owned(), "profile-b".to_owned()];
        let builder = make_scenario_builder();

        let run = |jobs: usize| {
            BenchmarkHarness::run_with_seeds(
                &factories,
                &profiles,
                &builder,
                0..4,
                Some(BenchmarkOptions {
                    jobs: Some(jobs),
                    ..Default::default()
                }),
            )
            .report
        };

        let r1 = run(1);
        let r2 = run(2);

        assert_eq!(
            r1.strategy_names, r2.strategy_names,
            "strategy_names order must be stable across jobs"
        );
        assert_eq!(
            r1.profile_names, r2.profile_names,
            "profile_names order must be stable across jobs"
        );
        // Display output must be bit-identical (same row order, same values).
        assert_eq!(
            format!("{r1}"),
            format!("{r2}"),
            "Display output must be identical for jobs=1 vs jobs=2"
        );
    }

    #[test]
    fn report_completion_is_not_tasks_injected() {
        // Regression test: "Завершение" must come from task_completion_rate,
        // not avg_tasks_injected. With all_tasks_assigned=true and no dynamic
        // tasks, completion should be 1.000, not 0.000.
        let factories: Vec<StrategyFactory> =
            vec![Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
                Box::new(GreedyAllocator) as Box<dyn Strategy>
            })];
        let profiles = vec!["ideal".to_owned()];
        let builder = make_scenario_builder();
        let report = BenchmarkHarness::run_quick(&factories, &profiles, &builder);
        let report_text = format!("{}", report);

        // Parse the markdown table and check the "Completion" column specifically.
        // Column layout: | Mission | Scenario | Strategy | Profile | Seeds | Success | Completion | ...
        // After splitting by '|', index 7 is the completion column.
        let rows: Vec<&str> = report_text.lines().skip(2).collect();
        for row in &rows {
            if row.contains("greedy") {
                let cols: Vec<&str> = row.split('|').collect();
                let completion_col = cols.get(7).map(|s| s.trim());
                assert_eq!(
                    completion_col,
                    Some("1.000"),
                    "Completion column (index 7) should be 1.000 when all_tasks_assigned=true, got cols: {:?}",
                    cols
                );
            }
        }
    }

    #[test]
    fn custom_seed_count_produces_custom_report_id() {
        let factories: Vec<StrategyFactory> =
            vec![Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
                Box::new(GreedyAllocator) as Box<dyn Strategy>
            })];
        let profiles = vec!["ideal".to_owned()];
        let builder = make_scenario_builder();
        let result = BenchmarkHarness::run_with_seed_count_with_options(
            &factories,
            &profiles,
            &builder,
            12,
            BenchmarkOptions {
                mission_name: "coverage",
                jobs: Some(2),
                ..BenchmarkOptions::default()
            },
        );

        assert_eq!(result.report.seed_range_start, 0);
        assert_eq!(result.report.seed_range_end, 12);
        assert_eq!(result.report.total_runs_per_cell, 12);
        assert!(
            result
                .report
                .benchmark_run_id
                .ends_with("_coverage_12_custom"),
            "custom seed count should be marked custom, got: {}",
            result.report.benchmark_run_id
        );
    }

    #[test]
    fn merged_benchmark_run_id_single_report_unchanged() {
        let report = ComparisonReport {
            benchmark_run_id: "2026-01-01T000000Z_coverage_10_quick".to_owned(),
            seed_range_start: 0,
            seed_range_end: 10,
            total_runs_per_cell: 10,
            mission_names: vec!["coverage".to_owned()],
            scenario_names: vec!["coverage".to_owned()],
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["ideal".to_owned()],
            results: std::collections::HashMap::new(),
        };
        let id = merged_benchmark_run_id(&[report]);
        assert_eq!(id, "2026-01-01T000000Z_coverage_10_quick");
    }

    #[test]
    fn merged_benchmark_run_id_multiple_reports_contains_all() {
        let r1 = ComparisonReport {
            benchmark_run_id: "2026-01-01T000000Z_coverage_10_quick".to_owned(),
            seed_range_start: 0,
            seed_range_end: 10,
            total_runs_per_cell: 10,
            mission_names: vec!["coverage".to_owned()],
            scenario_names: vec!["coverage".to_owned()],
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["ideal".to_owned()],
            results: std::collections::HashMap::new(),
        };
        let r2 = ComparisonReport {
            benchmark_run_id: "2026-01-01T000000Z_sar_10_quick".to_owned(),
            seed_range_start: 0,
            seed_range_end: 10,
            total_runs_per_cell: 10,
            mission_names: vec!["sar".to_owned()],
            scenario_names: vec!["sar".to_owned()],
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["standard".to_owned()],
            results: std::collections::HashMap::new(),
        };
        let id = merged_benchmark_run_id(&[r1, r2]);
        assert!(
            id.contains("_all_"),
            "merged id should contain '_all_', got: {}",
            id
        );
        assert!(
            !id.contains("coverage"),
            "merged id should not contain a mission name, got: {}",
            id
        );
        assert!(
            id.ends_with("_10_quick"),
            "mode should be preserved, got: {}",
            id
        );
    }

    #[test]
    fn merged_benchmark_run_id_preserves_prefix() {
        let r1 = ComparisonReport {
            benchmark_run_id: "myrun_2026-01-01T000000Z_coverage_1_smoke".to_owned(),
            seed_range_start: 0,
            seed_range_end: 1,
            total_runs_per_cell: 1,
            mission_names: vec!["coverage".to_owned()],
            scenario_names: vec!["coverage".to_owned()],
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["ideal".to_owned()],
            results: std::collections::HashMap::new(),
        };
        let r2 = ComparisonReport {
            benchmark_run_id: "myrun_2026-01-01T000000Z_sar_1_smoke".to_owned(),
            seed_range_start: 0,
            seed_range_end: 1,
            total_runs_per_cell: 1,
            mission_names: vec!["sar".to_owned()],
            scenario_names: vec!["sar".to_owned()],
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["standard".to_owned()],
            results: std::collections::HashMap::new(),
        };
        let id = merged_benchmark_run_id(&[r1, r2]);
        assert!(
            id.starts_with("myrun_"),
            "prefix should be preserved, got: {}",
            id
        );
        assert!(
            id.contains("_all_"),
            "merged id should contain '_all_', got: {}",
            id
        );
    }
}
