use std::collections::HashMap;

use swarm_alloc::Strategy;
use swarm_metrics::AggregateMetrics;

use crate::{RunConfig, Scenario};

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
