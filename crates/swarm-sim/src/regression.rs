use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use swarm_metrics::AggregateMetrics;

// ---------------------------------------------------------------------------
// 1. Threshold & RegressionSuite
// ---------------------------------------------------------------------------

/// A single threshold check against an aggregated metric.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Threshold {
    pub metric: String,   // e.g. "success_rate"
    pub min: Option<f64>, // e.g. Some(0.7)
    pub max: Option<f64>, // e.g. Some(0.5) for entropy
}

/// One suite = one mission + one profile + one strategy + thresholds.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RegressionSuite {
    pub name: String,
    pub mission: String,
    pub profile: String,
    pub strategy: String,
    pub thresholds: Vec<Threshold>,
    pub mode: SuiteMode,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SuiteMode {
    Smoke, // 1 seed, < 5s
    Quick, // 10 seeds, < 30s
}

/// Result of running one concrete strategy within a suite.
#[derive(Clone, Debug)]
pub struct SuiteResult {
    pub suite: RegressionSuite,
    pub actual_strategy: String,
    pub metrics: AggregateMetrics,
    pub violations: Vec<ThresholdViolation>,
}

#[derive(Clone, Debug)]
pub struct ThresholdViolation {
    pub threshold: Threshold,
    pub actual: f64,
}

// ---------------------------------------------------------------------------
// 2. ThresholdChecker
// ---------------------------------------------------------------------------

pub struct ThresholdChecker;

impl ThresholdChecker {
    pub fn check(metrics: &AggregateMetrics, thresholds: &[Threshold]) -> Vec<ThresholdViolation> {
        let mut violations = Vec::new();
        for t in thresholds {
            let actual = extract_metric(metrics, &t.metric);
            if let Some(min) = t.min {
                if actual < min {
                    violations.push(ThresholdViolation {
                        threshold: t.clone(),
                        actual,
                    });
                }
            }
            if let Some(max) = t.max {
                if actual > max {
                    violations.push(ThresholdViolation {
                        threshold: t.clone(),
                        actual,
                    });
                }
            }
        }
        violations
    }
}

fn extract_metric(metrics: &AggregateMetrics, metric: &str) -> f64 {
    match metric {
        "success_rate" => metrics.success_rate,
        "task_completion_rate" => metrics.avg_task_completion_rate,
        "edge_coverage_rate" => metrics.avg_edge_coverage_rate,
        "missed_edges" => metrics.avg_missed_edges,
        "probability_of_detection" => metrics.avg_probability_of_detection,
        "belief_entropy_final" => metrics.avg_belief_entropy_final,
        "convergence_ticks_p50" => metrics.convergence_ticks_p50,
        "convergence_ticks_p95" => metrics.convergence_ticks_p95,
        "convergence_ticks_max" => metrics.convergence_ticks_max,
        "safety_violations" => metrics.avg_safety_violations,
        "route_length" => metrics.avg_route_length,
        "bundle_travel_distance" => metrics.avg_bundle_travel_distance,
        "detection_ticks" => metrics.avg_detection_ticks,
        "reallocation_ticks" => metrics.avg_reallocation_ticks,
        "messages_attempted" => metrics.avg_messages_attempted,
        "messages_dropped" => metrics.avg_messages_dropped,
        "tasks_injected" => metrics.avg_tasks_injected,
        "tasks_expired" => metrics.avg_tasks_expired,
        "conflicting_assignments" => metrics.avg_conflicting_assignments,
        "network_availability" => metrics.avg_network_availability,
        "relay_reallocation_ticks" => metrics.avg_relay_reallocation_ticks,
        "avg_hop_count" => metrics.avg_avg_hop_count,
        "disconnected_agents_max" => metrics.avg_disconnected_agents_max,
        "coverage_progress" => metrics.avg_coverage_progress,
        "bytes_sent" => metrics.avg_bytes_sent,
        "stale_state_age_ticks" => metrics.avg_stale_state_age_ticks,
        "battery_margin_min" => metrics.avg_battery_margin_min,
        "battery_margin_avg" => metrics.avg_battery_margin_avg,
        "time_to_find" => metrics.avg_time_to_find,
        "targets_found" => metrics.avg_targets_found,
        "false_positive_rate" => metrics.avg_false_positive_rate,
        "confirmation_scans" => metrics.avg_confirmation_scans,
        "revisit_count" => metrics.avg_revisit_count,
        "route_efficiency" => metrics.avg_route_efficiency,
        "wasted_travel" => metrics.avg_wasted_travel,
        "return_reserve" => metrics.avg_return_reserve,
        "infeasible_routes" => metrics.avg_infeasible_routes,
        _ => 0.0,
    }
}

// ---------------------------------------------------------------------------
// 3. Baseline
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Baseline {
    pub version: String,
    pub created_at: String, // ISO 8601
    pub commit: String,
    pub results: HashMap<String, AggregateMetrics>,
}

impl Baseline {
    pub fn from_suites(results: &[(String, AggregateMetrics)]) -> Self {
        Self {
            version: "1.0".to_owned(),
            created_at: chrono::Utc::now().to_rfc3339(),
            commit: String::new(),
            results: results.iter().cloned().collect(),
        }
    }

    pub fn load(path: &str) -> Result<Self, std::io::Error> {
        let contents = std::fs::read_to_string(path)?;
        let baseline: Baseline = serde_json::from_str(&contents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(baseline)
    }

    pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
        let contents = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, contents)
    }

    pub fn compare(&self, current: &AggregateMetrics, suite_name: &str) -> Vec<BaselineDelta> {
        let mut deltas = Vec::new();
        if let Some(baseline_metrics) = self.results.get(suite_name) {
            deltas.extend(compare_field(
                suite_name,
                "success_rate",
                baseline_metrics.success_rate,
                current.success_rate,
            ));
            deltas.extend(compare_field(
                suite_name,
                "avg_task_completion_rate",
                baseline_metrics.avg_task_completion_rate,
                current.avg_task_completion_rate,
            ));
            deltas.extend(compare_field(
                suite_name,
                "avg_edge_coverage_rate",
                baseline_metrics.avg_edge_coverage_rate,
                current.avg_edge_coverage_rate,
            ));
            deltas.extend(compare_field(
                suite_name,
                "avg_probability_of_detection",
                baseline_metrics.avg_probability_of_detection,
                current.avg_probability_of_detection,
            ));
            deltas.extend(compare_field(
                suite_name,
                "avg_belief_entropy_final",
                baseline_metrics.avg_belief_entropy_final,
                current.avg_belief_entropy_final,
            ));
            deltas.extend(compare_field(
                suite_name,
                "convergence_ticks_p95",
                baseline_metrics.convergence_ticks_p95,
                current.convergence_ticks_p95,
            ));
            deltas.extend(compare_field(
                suite_name,
                "avg_safety_violations",
                baseline_metrics.avg_safety_violations,
                current.avg_safety_violations,
            ));
            deltas.extend(compare_field(
                suite_name,
                "avg_route_length",
                baseline_metrics.avg_route_length,
                current.avg_route_length,
            ));
            deltas.extend(compare_field(
                suite_name,
                "avg_bundle_travel_distance",
                baseline_metrics.avg_bundle_travel_distance,
                current.avg_bundle_travel_distance,
            ));
        }
        deltas
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BaselineDelta {
    pub suite_name: String,
    pub metric: String,
    pub baseline_value: f64,
    pub current_value: f64,
    pub change_pct: f64,
    pub status: DeltaStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DeltaStatus {
    Improved,
    Degraded,
    Stable,
}

fn compare_field(
    suite_name: &str,
    metric: &str,
    baseline: f64,
    current: f64,
) -> Option<BaselineDelta> {
    if baseline == 0.0 {
        return None;
    }
    let change_pct = ((current - baseline) / baseline) * 100.0;
    // For most metrics higher is better, except entropy/violations/time.
    // Use a simple heuristic: if metric name contains "entropy", "violations",
    // "ticks", "missed", "dropped" then lower is better.
    let lower_is_better = metric.contains("entropy")
        || metric.contains("violations")
        || metric.contains("ticks")
        || metric.contains("missed")
        || metric.contains("dropped")
        || metric.contains("wasted");

    let status = if change_pct.abs() < 1.0 {
        DeltaStatus::Stable
    } else if change_pct > 0.0 {
        if lower_is_better {
            DeltaStatus::Degraded
        } else {
            DeltaStatus::Improved
        }
    } else {
        if lower_is_better {
            DeltaStatus::Improved
        } else {
            DeltaStatus::Degraded
        }
    };

    Some(BaselineDelta {
        suite_name: suite_name.to_owned(),
        metric: metric.to_owned(),
        baseline_value: baseline,
        current_value: current,
        change_pct,
        status,
    })
}

// ---------------------------------------------------------------------------
// 4. RegressionRunner
// ---------------------------------------------------------------------------

pub struct RegressionRunner;

impl RegressionRunner {
    pub fn run(
        suites: &[RegressionSuite],
        baseline: Option<&Baseline>,
        suite_runner: impl Fn(&RegressionSuite) -> HashMap<String, AggregateMetrics>,
    ) -> RegressionReport {
        let mut suite_results = Vec::new();
        let mut deltas = Vec::new();
        let mut overall_pass = true;

        for suite in suites {
            let metrics_map = suite_runner(suite);

            if suite.strategy == "all" {
                // Run thresholds against every strategy returned.
                for (strategy_name, metrics) in &metrics_map {
                    let violations = ThresholdChecker::check(metrics, &suite.thresholds);
                    let pass = violations.is_empty();
                    if !pass {
                        overall_pass = false;
                    }
                    suite_results.push(SuiteResult {
                        suite: suite.clone(),
                        actual_strategy: strategy_name.clone(),
                        metrics: metrics.clone(),
                        violations,
                    });
                    if let Some(b) = baseline {
                        let suite_key = format!("{}/{}", suite.name, strategy_name);
                        deltas.extend(b.compare(metrics, &suite_key));
                    }
                }
            } else {
                let metrics = metrics_map
                    .get(&suite.strategy)
                    .cloned()
                    .unwrap_or_else(|| AggregateMetrics::from_runs(&[]));
                let violations = ThresholdChecker::check(&metrics, &suite.thresholds);
                let pass = violations.is_empty();
                if !pass {
                    overall_pass = false;
                }
                suite_results.push(SuiteResult {
                    suite: suite.clone(),
                    actual_strategy: suite.strategy.clone(),
                    metrics: metrics.clone(),
                    violations: violations.clone(),
                });
                if let Some(b) = baseline {
                    deltas.extend(b.compare(&metrics, &suite.name));
                }
            }
        }

        RegressionReport {
            suite_results,
            deltas,
            overall_pass,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RegressionReport {
    pub suite_results: Vec<SuiteResult>,
    pub deltas: Vec<BaselineDelta>,
    pub overall_pass: bool,
}

impl std::fmt::Display for RegressionReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "# Regression Report")?;
        writeln!(f, "overall_pass: {}", self.overall_pass)?;
        writeln!(f)?;
        for result in &self.suite_results {
            let status = if result.violations.is_empty() {
                "PASS"
            } else {
                "FAIL"
            };
            writeln!(
                f,
                "## {} (strategy={}) -> {}",
                result.suite.name, result.actual_strategy, status
            )?;
            for v in &result.violations {
                writeln!(
                    f,
                    "  VIOLATION: {} = {:.3} (expected: {:?})",
                    v.threshold.metric,
                    v.actual,
                    v.threshold.min.or(v.threshold.max)
                )?;
            }
        }
        if !self.deltas.is_empty() {
            writeln!(f)?;
            writeln!(f, "## Baseline Deltas")?;
            for d in &self.deltas {
                writeln!(
                    f,
                    "  {} {}: {:.3} -> {:.3} ({:+.1}%)",
                    d.suite_name, d.metric, d.baseline_value, d.current_value, d.change_pct
                )?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Default suites
// ---------------------------------------------------------------------------

pub fn default_suites() -> Vec<RegressionSuite> {
    vec![
        // SAR
        RegressionSuite {
            name: "sar_ideal_greedy".to_owned(),
            mission: "sar".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![
                Threshold {
                    metric: "success_rate".to_owned(),
                    min: Some(0.0),
                    max: None,
                },
                Threshold {
                    metric: "belief_entropy_final".to_owned(),
                    min: None,
                    max: Some(0.5),
                },
            ],
            mode: SuiteMode::Smoke,
        },
        RegressionSuite {
            name: "sar_standard_greedy".to_owned(),
            mission: "sar".to_owned(),
            profile: "standard".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "success_rate".to_owned(),
                min: Some(0.0),
                max: None,
            }],
            mode: SuiteMode::Smoke,
        },
        // Inspection
        RegressionSuite {
            name: "inspection_linear_all".to_owned(),
            mission: "inspection".to_owned(),
            profile: "linear".to_owned(),
            strategy: "all".to_owned(),
            thresholds: vec![
                Threshold {
                    metric: "edge_coverage_rate".to_owned(),
                    min: Some(0.85),
                    max: None,
                },
                Threshold {
                    metric: "success_rate".to_owned(),
                    min: Some(0.9),
                    max: None,
                },
            ],
            mode: SuiteMode::Smoke,
        },
        RegressionSuite {
            name: "inspection_perimeter_all".to_owned(),
            mission: "inspection".to_owned(),
            profile: "perimeter".to_owned(),
            strategy: "all".to_owned(),
            thresholds: vec![Threshold {
                metric: "edge_coverage_rate".to_owned(),
                min: Some(0.3),
                max: None,
            }],
            mode: SuiteMode::Smoke,
        },
        // CBBA stress
        RegressionSuite {
            name: "cbba_stress_pl_0_0".to_owned(),
            mission: "coverage".to_owned(),
            profile: "ideal-no-failures".to_owned(),
            strategy: "cbba".to_owned(),
            thresholds: vec![
                Threshold {
                    metric: "success_rate".to_owned(),
                    min: Some(0.9),
                    max: None,
                },
                Threshold {
                    metric: "convergence_ticks_p95".to_owned(),
                    min: None,
                    max: Some(15.0),
                },
            ],
            mode: SuiteMode::Quick,
        },
        RegressionSuite {
            name: "cbba_stress_pl_0_2".to_owned(),
            mission: "coverage".to_owned(),
            profile: "ideal-no-failures".to_owned(),
            strategy: "cbba".to_owned(),
            thresholds: vec![
                Threshold {
                    metric: "success_rate".to_owned(),
                    min: Some(0.8),
                    max: None,
                },
                Threshold {
                    metric: "convergence_ticks_p95".to_owned(),
                    min: None,
                    max: Some(20.0),
                },
            ],
            mode: SuiteMode::Quick,
        },
        // Safety
        RegressionSuite {
            name: "safety_coverage".to_owned(),
            mission: "coverage".to_owned(),
            profile: "ideal-no-failures".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "safety_violations".to_owned(),
                min: None,
                max: Some(0.0),
            }],
            mode: SuiteMode::Smoke,
        },
        // Emergency mesh
        RegressionSuite {
            name: "emergency_mesh_ideal".to_owned(),
            mission: "emergency-mesh".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "success_rate".to_owned(),
                min: Some(0.0),
                max: None,
            }],
            mode: SuiteMode::Smoke,
        },
    ]
}

// ---------------------------------------------------------------------------
// 6. Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metrics(success_rate: f64) -> AggregateMetrics {
        AggregateMetrics {
            total_runs: 10,
            success_rate,
            avg_detection_ticks: 0.0,
            avg_reallocation_ticks: 0.0,
            avg_messages_attempted: 0.0,
            avg_messages_dropped: 0.0,
            avg_tasks_injected: 0.0,
            avg_tasks_expired: 0.0,
            avg_conflicting_assignments: 0.0,
            avg_network_availability: 0.0,
            avg_relay_reallocation_ticks: 0.0,
            avg_avg_hop_count: 0.0,
            avg_disconnected_agents_max: 0.0,
            avg_coverage_progress: 0.0,
            avg_bytes_sent: 0.0,
            avg_stale_state_age_ticks: 0.0,
            avg_battery_margin_min: 0.0,
            avg_battery_margin_avg: 0.0,
            avg_task_completion_rate: 0.0,
            avg_time_to_find: 0.0,
            avg_probability_of_detection: 0.0,
            avg_targets_found: 0.0,
            avg_safety_violations: 0.0,
            avg_belief_entropy_final: 0.0,
            avg_false_positive_rate: 0.0,
            avg_confirmation_scans: 0.0,
            convergence_ticks_p50: 0.0,
            convergence_ticks_p95: 0.0,
            convergence_ticks_max: 0.0,
            avg_bundle_travel_distance: 0.0,
            avg_edge_coverage_rate: 0.0,
            avg_missed_edges: 0.0,
            avg_revisit_count: 0.0,
            avg_route_efficiency: 0.0,
            avg_route_length: 0.0,
            avg_wasted_travel: 0.0,
            avg_return_reserve: 0.0,
            avg_infeasible_routes: 0.0,
            avg_hazard_zones_mapped: 0.0,
            avg_priority_updates: 0.0,
            avg_final_threat_level: 0.0,
            mission: String::new(),
            scenario: String::new(),
        }
    }

    #[test]
    fn threshold_min_violation() {
        let metrics = make_metrics(0.5);
        let thresholds = vec![Threshold {
            metric: "success_rate".to_owned(),
            min: Some(0.7),
            max: None,
        }];
        let violations = ThresholdChecker::check(&metrics, &thresholds);
        assert_eq!(violations.len(), 1);
        assert!((violations[0].actual - 0.5).abs() < 1e-6);
    }

    #[test]
    fn threshold_max_violation() {
        let metrics = make_metrics(0.9);
        let thresholds = vec![Threshold {
            metric: "success_rate".to_owned(),
            min: None,
            max: Some(0.7),
        }];
        let violations = ThresholdChecker::check(&metrics, &thresholds);
        assert_eq!(violations.len(), 1);
        assert!((violations[0].actual - 0.9).abs() < 1e-6);
    }

    #[test]
    fn threshold_min_and_max_both_checked() {
        let mut metrics = make_metrics(0.9);
        metrics.avg_belief_entropy_final = 0.6;
        let thresholds = vec![
            Threshold {
                metric: "success_rate".to_owned(),
                min: Some(0.7),
                max: None,
            },
            Threshold {
                metric: "belief_entropy_final".to_owned(),
                min: None,
                max: Some(0.5),
            },
        ];
        let violations = ThresholdChecker::check(&metrics, &thresholds);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].threshold.metric, "belief_entropy_final");
    }

    #[test]
    fn threshold_no_violation_when_in_range() {
        let metrics = make_metrics(0.8);
        let thresholds = vec![Threshold {
            metric: "success_rate".to_owned(),
            min: Some(0.7),
            max: Some(0.9),
        }];
        let violations = ThresholdChecker::check(&metrics, &thresholds);
        assert!(violations.is_empty());
    }

    #[test]
    fn extract_metric_all_fields() {
        let mut m = make_metrics(0.5);
        m.avg_task_completion_rate = 0.1;
        m.avg_edge_coverage_rate = 0.2;
        m.avg_probability_of_detection = 0.3;
        m.avg_belief_entropy_final = 0.4;
        m.convergence_ticks_p50 = 5.0;
        m.convergence_ticks_p95 = 10.0;
        m.avg_safety_violations = 2.0;
        m.avg_route_length = 15.0;
        m.avg_bundle_travel_distance = 20.0;

        assert!((extract_metric(&m, "success_rate") - 0.5).abs() < 1e-6);
        assert!((extract_metric(&m, "task_completion_rate") - 0.1).abs() < 1e-6);
        assert!((extract_metric(&m, "edge_coverage_rate") - 0.2).abs() < 1e-6);
        assert!((extract_metric(&m, "probability_of_detection") - 0.3).abs() < 1e-6);
        assert!((extract_metric(&m, "belief_entropy_final") - 0.4).abs() < 1e-6);
        assert!((extract_metric(&m, "convergence_ticks_p50") - 5.0).abs() < 1e-6);
        assert!((extract_metric(&m, "convergence_ticks_p95") - 10.0).abs() < 1e-6);
        assert!((extract_metric(&m, "safety_violations") - 2.0).abs() < 1e-6);
        assert!((extract_metric(&m, "route_length") - 15.0).abs() < 1e-6);
        assert!((extract_metric(&m, "bundle_travel_distance") - 20.0).abs() < 1e-6);
    }

    #[test]
    fn baseline_roundtrip() {
        let baseline = Baseline {
            version: "1.0".to_owned(),
            created_at: "2025-05-26T12:00:00Z".to_owned(),
            commit: "abc123".to_owned(),
            results: {
                let mut map = HashMap::new();
                map.insert("suite1".to_owned(), make_metrics(0.8));
                map
            },
        };
        let path = "/tmp/test_baseline.json";
        baseline.save(path).unwrap();
        let loaded = Baseline::load(path).unwrap();
        assert_eq!(baseline, loaded);
    }

    #[test]
    fn baseline_compare_improved() {
        let baseline = Baseline {
            version: "1.0".to_owned(),
            created_at: "2025-05-26T12:00:00Z".to_owned(),
            commit: "abc".to_owned(),
            results: {
                let mut map = HashMap::new();
                map.insert("suite1".to_owned(), make_metrics(0.8));
                map
            },
        };
        let current = make_metrics(0.88);
        let deltas = baseline.compare(&current, "suite1");
        let sr_delta = deltas.iter().find(|d| d.metric == "success_rate").unwrap();
        assert!((sr_delta.change_pct - 10.0).abs() < 0.1);
        assert_eq!(sr_delta.status, DeltaStatus::Improved);
    }

    #[test]
    fn baseline_compare_degraded() {
        let baseline = Baseline {
            version: "1.0".to_owned(),
            created_at: "2025-05-26T12:00:00Z".to_owned(),
            commit: "abc".to_owned(),
            results: {
                let mut map = HashMap::new();
                map.insert("suite1".to_owned(), make_metrics(0.8));
                map
            },
        };
        let current = make_metrics(0.72);
        let deltas = baseline.compare(&current, "suite1");
        let sr_delta = deltas.iter().find(|d| d.metric == "success_rate").unwrap();
        assert!((sr_delta.change_pct - (-10.0)).abs() < 0.1);
        assert_eq!(sr_delta.status, DeltaStatus::Degraded);
    }

    #[test]
    fn baseline_compare_stable() {
        let baseline = Baseline {
            version: "1.0".to_owned(),
            created_at: "2025-05-26T12:00:00Z".to_owned(),
            commit: "abc".to_owned(),
            results: {
                let mut map = HashMap::new();
                map.insert("suite1".to_owned(), make_metrics(0.8));
                map
            },
        };
        let current = make_metrics(0.805);
        let deltas = baseline.compare(&current, "suite1");
        let sr_delta = deltas.iter().find(|d| d.metric == "success_rate").unwrap();
        assert!(sr_delta.change_pct.abs() < 1.0);
        assert_eq!(sr_delta.status, DeltaStatus::Stable);
    }

    #[test]
    fn baseline_compare_lower_is_better() {
        let mut baseline_metrics = make_metrics(0.8);
        baseline_metrics.avg_belief_entropy_final = 0.5;
        let baseline = Baseline {
            version: "1.0".to_owned(),
            created_at: "2025-05-26T12:00:00Z".to_owned(),
            commit: "abc".to_owned(),
            results: {
                let mut map = HashMap::new();
                map.insert("suite1".to_owned(), baseline_metrics);
                map
            },
        };
        let mut current = make_metrics(0.8);
        current.avg_belief_entropy_final = 0.4;
        let deltas = baseline.compare(&current, "suite1");
        let entropy_delta = deltas
            .iter()
            .find(|d| d.metric == "avg_belief_entropy_final")
            .unwrap();
        assert_eq!(entropy_delta.status, DeltaStatus::Improved);
    }

    #[test]
    fn regression_runner_single_suite() {
        let suites = vec![RegressionSuite {
            name: "test_suite".to_owned(),
            mission: "coverage".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "success_rate".to_owned(),
                min: Some(0.7),
                max: None,
            }],
            mode: SuiteMode::Smoke,
        }];
        let report = RegressionRunner::run(&suites, None, |_suite| {
            let mut map = HashMap::new();
            map.insert("greedy".to_owned(), make_metrics(0.8));
            map
        });
        assert!(report.overall_pass);
        assert_eq!(report.suite_results.len(), 1);
        assert!(report.suite_results[0].violations.is_empty());
    }

    #[test]
    fn regression_runner_forced_failure() {
        let suites = vec![RegressionSuite {
            name: "test_suite".to_owned(),
            mission: "coverage".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "success_rate".to_owned(),
                min: Some(1.0),
                max: None,
            }],
            mode: SuiteMode::Smoke,
        }];
        let report = RegressionRunner::run(&suites, None, |_suite| {
            let mut map = HashMap::new();
            map.insert("greedy".to_owned(), make_metrics(0.8));
            map
        });
        assert!(!report.overall_pass);
        assert_eq!(report.suite_results.len(), 1);
        assert_eq!(report.suite_results[0].violations.len(), 1);
        assert_eq!(
            report.suite_results[0].violations[0].threshold.metric,
            "success_rate"
        );
    }

    #[test]
    fn regression_runner_all_strategy_mode() {
        let suites = vec![RegressionSuite {
            name: "inspection_all".to_owned(),
            mission: "inspection".to_owned(),
            profile: "linear".to_owned(),
            strategy: "all".to_owned(),
            thresholds: vec![Threshold {
                metric: "success_rate".to_owned(),
                min: Some(0.5),
                max: None,
            }],
            mode: SuiteMode::Smoke,
        }];
        let report = RegressionRunner::run(&suites, None, |_suite| {
            let mut map = HashMap::new();
            map.insert("greedy".to_owned(), make_metrics(0.8));
            map.insert("auction".to_owned(), make_metrics(0.6));
            map
        });
        assert!(report.overall_pass);
        assert_eq!(report.suite_results.len(), 2);
    }
}
