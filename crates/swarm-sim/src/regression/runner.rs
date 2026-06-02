use std::collections::HashMap;

use swarm_metrics::AggregateMetrics;

use super::types::{Baseline, RegressionReport, RegressionSuite, SuiteResult, ThresholdChecker};

pub struct RegressionRunner;

impl RegressionRunner {
    pub fn run(
        suites: &[RegressionSuite],
        baseline: Option<&Baseline>,
        suite_runner: impl Fn(&RegressionSuite) -> HashMap<String, AggregateMetrics>,
    ) -> RegressionReport {
        let mut suite_results = Vec::new();
        let mut deltas = Vec::new();
        let mut missing_baselines = Vec::new();
        let mut overall_pass = true;

        for suite in suites {
            let metrics_map = suite_runner(suite);
            let seed_range = suite.mode.seed_range();

            if suite.strategy == "all" {
                // Run thresholds against every strategy returned.
                for (strategy_name, metrics) in &metrics_map {
                    let violations = ThresholdChecker::check(metrics, &suite.thresholds);
                    if !violations.is_empty() && suite.group.is_gating() {
                        overall_pass = false;
                    }
                    let result = SuiteResult {
                        suite: suite.clone(),
                        actual_strategy: strategy_name.clone(),
                        metrics: metrics.clone(),
                        violations,
                        seed_range,
                    };
                    if let Some(b) = baseline {
                        let suite_key = result.regression_key();
                        if b.has_result(&suite_key) {
                            deltas.extend(b.compare(metrics, &suite_key));
                        } else {
                            missing_baselines.push(suite_key);
                        }
                    }
                    suite_results.push(result);
                }
            } else {
                let metrics = metrics_map
                    .get(&suite.strategy)
                    .cloned()
                    .unwrap_or_else(|| AggregateMetrics::from_runs(&[]));
                let violations = ThresholdChecker::check(&metrics, &suite.thresholds);
                if !violations.is_empty() && suite.group.is_gating() {
                    overall_pass = false;
                }
                let result = SuiteResult {
                    suite: suite.clone(),
                    actual_strategy: suite.strategy.clone(),
                    metrics: metrics.clone(),
                    violations,
                    seed_range,
                };
                if let Some(b) = baseline {
                    let suite_key = result.regression_key();
                    if b.has_result(&suite_key) {
                        deltas.extend(b.compare(&metrics, &suite_key));
                    } else {
                        missing_baselines.push(suite_key);
                    }
                }
                suite_results.push(result);
            }
        }

        RegressionReport {
            suite_results,
            deltas,
            missing_baselines,
            overall_pass,
        }
    }
}
