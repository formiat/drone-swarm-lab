use std::collections::HashMap;

use swarm_alloc::Strategy;
use swarm_metrics::AggregateMetrics;

use crate::{RunConfig, Scenario, ScenarioRunner};

/// Report produced by a benchmark run comparing strategies across profiles.
pub struct ComparisonReport {
    pub strategy_names: Vec<String>,
    pub profile_names: Vec<String>,
    pub results: HashMap<(String, String), AggregateMetrics>,
}

impl std::fmt::Display for ComparisonReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "| Стратегия | Профиль | Успех | Обнаружение | Перераспределение | Покрытие | Сообщения | Доступность |"
        )?;
        writeln!(
            f,
            "|-----------|---------|-------|-------------|-------------------|----------|-----------|-------------|"
        )?;
        for strategy_name in &self.strategy_names {
            for profile_name in &self.profile_names {
                let key = (strategy_name.clone(), profile_name.clone());
                if let Some(metrics) = self.results.get(&key) {
                    writeln!(
                        f,
                        "| {:9} | {:7} | {:5.3} | {:11.3} | {:17.3} | {:8.3} | {:9.3} | {:11.3} |",
                        strategy_name,
                        profile_name,
                        metrics.success_rate,
                        metrics.avg_detection_ticks,
                        metrics.avg_reallocation_ticks,
                        metrics.avg_coverage_progress,
                        metrics.avg_messages_attempted,
                        metrics.avg_network_availability,
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// A function that builds a (Scenario, RunConfig) pair from a seed and profile name.
pub type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig)>;

/// Harness that runs strategies across seeds and profiles.
pub struct BenchmarkHarness;

impl BenchmarkHarness {
    /// Run a small benchmark for CI/testing (10 seeds).
    pub fn run_quick(
        strategies: &[Box<dyn Strategy>],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
    ) -> ComparisonReport {
        Self::run_with_seeds(strategies, profile_names, scenario_builder, 0..10)
    }

    /// Run a full benchmark (1000 seeds).
    pub fn run_full(
        strategies: &[Box<dyn Strategy>],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
    ) -> ComparisonReport {
        Self::run_with_seeds(strategies, profile_names, scenario_builder, 0..1000)
    }

    fn run_with_seeds(
        strategies: &[Box<dyn Strategy>],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
        seeds: std::ops::Range<u64>,
    ) -> ComparisonReport {
        let mut results: HashMap<(String, String), Vec<swarm_metrics::RunMetrics>> = HashMap::new();

        for seed in seeds {
            for strategy in strategies {
                for profile_name in profile_names {
                    let (scenario, run_config) = scenario_builder(seed, profile_name);
                    // Use a reference to the boxed strategy; ScenarioRunner takes impl Allocator
                    // We need to deref the box to get the inner value
                    let metrics = run_with_strategy(&scenario, run_config, strategy.as_ref());
                    results
                        .entry((strategy.name().to_owned(), profile_name.clone()))
                        .or_default()
                        .push(metrics);
                }
            }
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
            report_results.insert(
                (strategy_name, profile_name),
                AggregateMetrics::from_runs(&runs),
            );
        }

        ComparisonReport {
            strategy_names,
            profile_names: report_profile_names,
            results: report_results,
        }
    }
}

fn run_with_strategy(
    scenario: &Scenario,
    run_config: RunConfig,
    strategy: &dyn Strategy,
) -> swarm_metrics::RunMetrics {
    // Since Strategy: Allocator, we can pass the strategy reference directly
    // but ScenarioRunner::run_with expects a generic A: Allocator.
    // We need a workaround since &dyn Strategy doesn't automatically impl Allocator.
    // The simplest approach is to use a wrapper that delegates.
    struct StrategyWrapper<'a>(&'a dyn Strategy);
    impl<'a> swarm_alloc::Allocator for StrategyWrapper<'a> {
        fn allocate(
            &self,
            tasks: &[swarm_alloc::AllocationTask<'_>],
            agents: &[swarm_alloc::AllocationAgent],
        ) -> Vec<(swarm_types::TaskId, swarm_types::AgentId)> {
            self.0.allocate(tasks, agents)
        }

        fn allocate_with_connectivity(
            &self,
            tasks: &[swarm_alloc::AllocationTask<'_>],
            agents: &[swarm_alloc::AllocationAgent],
            connectivity: &swarm_alloc::ConnectivityContext,
        ) -> Vec<(swarm_types::TaskId, swarm_types::AgentId)> {
            self.0
                .allocate_with_connectivity(tasks, agents, connectivity)
        }
    }

    ScenarioRunner::run_with(scenario, run_config, StrategyWrapper(strategy))
}
