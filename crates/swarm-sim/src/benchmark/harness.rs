use std::collections::HashMap;

use rayon::prelude::*;
use swarm_alloc::Strategy;
use swarm_metrics::AggregateMetrics;

use super::aggregation::generate_benchmark_run_id;
use super::{
    BenchmarkOptions, BenchmarkResult, ComparisonReport, ScenarioBuilder, StrategyFactory,
};
use crate::{RunConfig, Scenario, ScenarioRunner};

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

    pub(super) fn run_with_seeds(
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
