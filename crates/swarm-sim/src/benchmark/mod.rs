mod aggregation;
mod harness;
mod markdown;
mod report;

pub use aggregation::merged_benchmark_run_id;
pub use harness::BenchmarkHarness;
pub use report::{
    BenchmarkOptions, BenchmarkResult, ComparisonReport, ScenarioBuilder, StrategyFactory,
};

#[cfg(test)]
mod tests;
