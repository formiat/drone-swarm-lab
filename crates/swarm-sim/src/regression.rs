use std::{collections::HashMap, str::FromStr};

use serde::{ser::SerializeStruct, Deserialize, Serialize};
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
    #[serde(default)]
    pub group: SuiteGroup,
    pub mission: String,
    pub profile: String,
    pub strategy: String,
    pub thresholds: Vec<Threshold>,
    pub mode: SuiteMode,
    /// Whether to apply M31 realism preset to this suite's scenarios.
    #[serde(default)]
    pub realism: bool,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuiteGroup {
    #[default]
    Smoke,
    Quick,
    Experimental,
    Validation,
}

impl SuiteGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Smoke => "smoke",
            Self::Quick => "quick",
            Self::Experimental => "experimental",
            Self::Validation => "validation",
        }
    }

    pub fn is_gating(self) -> bool {
        matches!(self, Self::Smoke | Self::Quick)
    }
}

impl FromStr for SuiteGroup {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "smoke" => Ok(Self::Smoke),
            "quick" => Ok(Self::Quick),
            "experimental" => Ok(Self::Experimental),
            "validation" => Ok(Self::Validation),
            _ => Err(format!("unknown regression suite group: {value}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuiteMode {
    Smoke, // 1 seed, < 5s
    Quick, // 10 seeds, < 30s
}

impl SuiteMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Smoke => "smoke",
            Self::Quick => "quick",
        }
    }

    /// value: `(first_seed, last_seed_inclusive)`
    pub fn seed_range(self) -> (u64, u64) {
        match self {
            Self::Smoke => (0, 0),
            Self::Quick => (0, 9),
        }
    }
}

/// Result of running one concrete strategy within a suite.
#[derive(Clone, Debug)]
pub struct SuiteResult {
    pub suite: RegressionSuite,
    pub actual_strategy: String,
    pub metrics: AggregateMetrics,
    pub violations: Vec<ThresholdViolation>,
    /// Seed range used: `(first_seed, last_seed_inclusive)`.
    pub seed_range: (u64, u64),
}

impl Serialize for SuiteResult {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("SuiteResult", 8)?;
        state.serialize_field("suite", &self.suite)?;
        state.serialize_field("actual_strategy", &self.actual_strategy)?;
        state.serialize_field("metrics", &self.metrics)?;
        state.serialize_field("violations", &self.violations)?;
        state.serialize_field("seed_range", &self.seed_range)?;
        state.serialize_field("regression_key", &self.regression_key())?;
        state.serialize_field("status", self.status_label())?;
        state.serialize_field("reproduction_command", &self.reproduction_command())?;
        state.end()
    }
}

impl SuiteResult {
    pub fn regression_key(&self) -> String {
        if self.suite.strategy == "all" {
            let suite_name = &self.suite.name;
            let strategy_name = &self.actual_strategy;
            format!("{suite_name}/{strategy_name}")
        } else {
            self.suite.name.clone()
        }
    }

    pub fn reproduction_command(&self) -> String {
        let group = self.suite.group.as_str();
        let suite_name = &self.suite.name;
        format!(
            "cargo run -p swarm-examples --bin regression_runner -- --suite {group} --suite-name {suite_name} --jobs 1"
        )
    }

    pub fn status_label(&self) -> &'static str {
        if self.violations.is_empty() {
            "PASS"
        } else if self.suite.group.is_gating() {
            "FAIL"
        } else {
            "NON-GATING-FAIL"
        }
    }
}

/// A single threshold violation with the amount by which the threshold was missed.
///
/// For a `min` bound: `delta = actual - min` (negative means violation).
/// For a `max` bound: `delta = max - actual` (negative means violation).
#[derive(Clone, Debug, Serialize)]
pub struct ThresholdViolation {
    pub threshold: Threshold,
    pub actual: f64,
    pub delta: f64,
}

impl std::fmt::Display for ThresholdViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bound = if let Some(min) = self.threshold.min {
            format!("min:{min:.3}")
        } else if let Some(max) = self.threshold.max {
            format!("max:{max:.3}")
        } else {
            "none".to_owned()
        };
        write!(
            f,
            "metric={} actual={:.3} threshold={} delta={:.3}",
            self.threshold.metric, self.actual, bound, self.delta
        )
    }
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
                        delta: actual - min,
                    });
                }
            }
            if let Some(max) = t.max {
                if actual > max {
                    violations.push(ThresholdViolation {
                        threshold: t.clone(),
                        actual,
                        delta: max - actual,
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
    /// Seed range covered by this baseline: `(first_seed, last_seed_inclusive)`.
    #[serde(default)]
    pub seed_range: Option<(u64, u64)>,
    #[serde(default)]
    pub seed_count: Option<u64>,
    #[serde(default)]
    pub suite_group: Option<String>,
    pub results: HashMap<String, AggregateMetrics>,
}

impl Baseline {
    pub fn from_suites(results: &[(String, AggregateMetrics)]) -> Self {
        Self {
            version: "1.0".to_owned(),
            created_at: chrono::Utc::now().to_rfc3339(),
            commit: String::new(),
            seed_range: None,
            seed_count: None,
            suite_group: None,
            results: results.iter().cloned().collect(),
        }
    }

    pub fn from_report(report: &RegressionReport, suite_group: Option<&str>) -> Self {
        let results = report
            .suite_results
            .iter()
            .map(|result| (result.regression_key(), result.metrics.clone()))
            .collect();
        let seed_range = seed_range_for_results(&report.suite_results);
        let seed_count = seed_range.map(|(start, end)| end.saturating_sub(start) + 1);

        Self {
            version: "1.0".to_owned(),
            created_at: chrono::Utc::now().to_rfc3339(),
            commit: String::new(),
            seed_range,
            seed_count,
            suite_group: suite_group.map(str::to_owned),
            results,
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
            deltas.extend(compare_field(
                suite_name,
                "avg_wasted_travel",
                baseline_metrics.avg_wasted_travel,
                current.avg_wasted_travel,
            ));
            deltas.extend(compare_field(
                suite_name,
                "avg_return_reserve",
                baseline_metrics.avg_return_reserve,
                current.avg_return_reserve,
            ));
            deltas.extend(compare_field(
                suite_name,
                "avg_infeasible_routes",
                baseline_metrics.avg_infeasible_routes,
                current.avg_infeasible_routes,
            ));
        }
        deltas
    }

    pub fn has_result(&self, suite_name: &str) -> bool {
        self.results.contains_key(suite_name)
    }
}

fn seed_range_for_results(results: &[SuiteResult]) -> Option<(u64, u64)> {
    let start = results.iter().map(|result| result.seed_range.0).min()?;
    let end = results.iter().map(|result| result.seed_range.1).max()?;
    Some((start, end))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BaselineDelta {
    pub suite_name: String,
    pub metric: String,
    pub baseline_value: f64,
    pub current_value: f64,
    pub change_pct: f64,
    pub status: DeltaStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeltaStatus {
    Improved,
    Degraded,
    Stable,
}

impl DeltaStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Improved => "improved",
            Self::Degraded => "degraded",
            Self::Stable => "stable",
        }
    }
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

#[derive(Clone, Debug, Serialize)]
pub struct RegressionReport {
    pub suite_results: Vec<SuiteResult>,
    pub deltas: Vec<BaselineDelta>,
    pub missing_baselines: Vec<String>,
    pub overall_pass: bool,
}

impl RegressionReport {
    pub fn has_threshold_violations(&self) -> bool {
        self.suite_results
            .iter()
            .any(|result| !result.violations.is_empty())
    }
}

impl std::fmt::Display for RegressionReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "# Regression Report")?;
        writeln!(f, "overall_pass: {}", self.overall_pass)?;
        writeln!(f)?;
        for result in &self.suite_results {
            let group = result.suite.group.as_str();
            let mode = result.suite.mode.as_str();
            let status = result.status_label();
            writeln!(
                f,
                "## {} (mission={} profile={} strategy={} group={} mode={} seeds={}..={}) -> {}",
                result.suite.name,
                result.suite.mission,
                result.suite.profile,
                result.actual_strategy,
                group,
                mode,
                result.seed_range.0,
                result.seed_range.1,
                status
            )?;
            for v in &result.violations {
                writeln!(f, "  VIOLATION: {v}")?;
            }
            if !result.violations.is_empty() {
                writeln!(f, "  reproduce: {}", result.reproduction_command())?;
            }
        }
        if !self.deltas.is_empty() {
            writeln!(f)?;
            writeln!(f, "## Baseline Deltas")?;
            for d in &self.deltas {
                writeln!(
                    f,
                    "  {} {}: {:.3} -> {:.3} ({:+.1}%, {})",
                    d.suite_name,
                    d.metric,
                    d.baseline_value,
                    d.current_value,
                    d.change_pct,
                    d.status.as_str()
                )?;
            }
        }
        if !self.missing_baselines.is_empty() {
            writeln!(f)?;
            writeln!(f, "## Missing Baseline Entries")?;
            for suite_name in &self.missing_baselines {
                writeln!(f, "  {suite_name}")?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 5. Default suites
// ---------------------------------------------------------------------------

pub fn all_suites() -> Vec<RegressionSuite> {
    vec![
        // SAR — M35 changed success semantics to targets-found; use task_completion_rate
        // for the primary threshold since success_rate on seed 0 is unreliable.
        RegressionSuite {
            name: "sar_ideal_greedy".to_owned(),
            group: SuiteGroup::Smoke,
            mission: "sar".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![
                Threshold {
                    metric: "task_completion_rate".to_owned(),
                    min: Some(0.80),
                    max: None,
                },
                Threshold {
                    metric: "belief_entropy_final".to_owned(),
                    min: None,
                    max: Some(0.5),
                },
            ],
            mode: SuiteMode::Smoke,
            realism: false,
        },
        RegressionSuite {
            name: "sar_standard_greedy".to_owned(),
            group: SuiteGroup::Smoke,
            mission: "sar".to_owned(),
            profile: "standard".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![
                Threshold {
                    metric: "task_completion_rate".to_owned(),
                    min: Some(0.70),
                    max: None,
                },
                Threshold {
                    metric: "belief_entropy_final".to_owned(),
                    min: None,
                    max: Some(0.6),
                },
            ],
            mode: SuiteMode::Smoke,
            realism: false,
        },
        // Inspection
        RegressionSuite {
            name: "inspection_linear_all".to_owned(),
            group: SuiteGroup::Smoke,
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
            realism: false,
        },
        // Perimeter inspection — physically constrained; centralized achieves 0.3–0.45 depending on seed.
        // Floor threshold guards against complete failure; greedy-only suite has a stricter check.
        RegressionSuite {
            name: "inspection_perimeter_all".to_owned(),
            group: SuiteGroup::Smoke,
            mission: "inspection".to_owned(),
            profile: "perimeter".to_owned(),
            strategy: "all".to_owned(),
            thresholds: vec![Threshold {
                metric: "edge_coverage_rate".to_owned(),
                min: Some(0.25),
                max: None,
            }],
            mode: SuiteMode::Smoke,
            realism: false,
        },
        // Experimental perimeter suite: greedy-only, softer threshold for tracking coverage floor.
        RegressionSuite {
            name: "inspection_perimeter_experimental".to_owned(),
            group: SuiteGroup::Experimental,
            mission: "inspection".to_owned(),
            profile: "perimeter".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "edge_coverage_rate".to_owned(),
                min: Some(0.30),
                max: None,
            }],
            mode: SuiteMode::Smoke,
            realism: false,
        },
        // CBBA coverage — renamed from cbba_stress_pl_0_0 / cbba_stress_pl_0_2.
        RegressionSuite {
            name: "cbba_coverage_ideal_no_failures".to_owned(),
            group: SuiteGroup::Quick,
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
                Threshold {
                    metric: "task_completion_rate".to_owned(),
                    min: Some(0.95),
                    max: None,
                },
            ],
            mode: SuiteMode::Quick,
            realism: false,
        },
        RegressionSuite {
            name: "cbba_coverage_light_loss_no_failures".to_owned(),
            group: SuiteGroup::Quick,
            mission: "coverage".to_owned(),
            profile: "light-loss-no-failures".to_owned(),
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
            realism: false,
        },
        // Safety
        RegressionSuite {
            name: "safety_coverage".to_owned(),
            group: SuiteGroup::Smoke,
            mission: "coverage".to_owned(),
            profile: "ideal-no-failures".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "safety_violations".to_owned(),
                min: None,
                max: Some(0.0),
            }],
            mode: SuiteMode::Smoke,
            realism: false,
        },
        // Emergency mesh — success semantics on seed 0 produce 0; use network_availability floor.
        RegressionSuite {
            name: "emergency_mesh_ideal".to_owned(),
            group: SuiteGroup::Smoke,
            mission: "emergency-mesh".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "network_availability".to_owned(),
                min: Some(0.001),
                max: None,
            }],
            mode: SuiteMode::Smoke,
            realism: false,
        },
        // Wildfire — M35 changed success to mapped-ratio; task_completion_rate is the reliable signal.
        RegressionSuite {
            name: "wildfire_small_static_greedy".to_owned(),
            group: SuiteGroup::Smoke,
            mission: "wildfire".to_owned(),
            profile: "small-static".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "task_completion_rate".to_owned(),
                min: Some(0.80),
                max: None,
            }],
            mode: SuiteMode::Smoke,
            realism: false,
        },
        // Experimental: dynamic semantics; task_completion_rate as floor.
        RegressionSuite {
            name: "wildfire_medium_dynamic_greedy".to_owned(),
            group: SuiteGroup::Experimental,
            mission: "wildfire".to_owned(),
            profile: "medium-dynamic".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "task_completion_rate".to_owned(),
                min: Some(0.60),
                max: None,
            }],
            mode: SuiteMode::Smoke,
            realism: false,
        },
        // Realism smoke: coverage under M31 realism preset; softer threshold due to noise overhead.
        RegressionSuite {
            name: "realism_coverage_smoke".to_owned(),
            group: SuiteGroup::Experimental,
            mission: "coverage".to_owned(),
            profile: "ideal-no-failures".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "success_rate".to_owned(),
                min: Some(0.75),
                max: None,
            }],
            mode: SuiteMode::Smoke,
            realism: true,
        },
    ]
}

pub fn default_suites() -> Vec<RegressionSuite> {
    all_suites()
        .into_iter()
        .filter(|suite| suite.group.is_gating())
        .collect()
}

pub fn suites_by_group(group: SuiteGroup) -> Vec<RegressionSuite> {
    all_suites()
        .into_iter()
        .filter(|suite| suite.group == group)
        .collect()
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
            avg_high_priority_zones_mapped: 0.0,
            avg_time_to_map_first_high_risk: 0.0,
            avg_zone_observations: 0.0,
            mission: String::new(),
            scenario: String::new(),
        }
    }

    fn make_suite(
        name: &str,
        group: SuiteGroup,
        mode: SuiteMode,
        min_success_rate: f64,
    ) -> RegressionSuite {
        RegressionSuite {
            name: name.to_owned(),
            group,
            mission: "coverage".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "success_rate".to_owned(),
                min: Some(min_success_rate),
                max: None,
            }],
            mode,
            realism: false,
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
            seed_range: Some((0, 9)),
            seed_count: Some(10),
            suite_group: Some("quick".to_owned()),
            results: {
                let mut map = HashMap::new();
                map.insert("suite1".to_owned(), make_metrics(0.8));
                map
            },
        };
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let path = tmp_dir.path().join("baseline.json");
        let path_str = path.to_str().unwrap();
        baseline.save(path_str).unwrap();
        let loaded = Baseline::load(path_str).unwrap();
        assert_eq!(baseline, loaded);
    }

    #[test]
    fn threshold_violation_delta_min() {
        let metrics = make_metrics(0.42);
        let thresholds = vec![Threshold {
            metric: "success_rate".to_owned(),
            min: Some(0.70),
            max: None,
        }];
        let violations = ThresholdChecker::check(&metrics, &thresholds);
        assert_eq!(violations.len(), 1);
        let v = &violations[0];
        assert!((v.actual - 0.42).abs() < 1e-6);
        // delta = actual - min = 0.42 - 0.70 = -0.28
        assert!((v.delta - (-0.28)).abs() < 1e-6, "delta was {}", v.delta);
    }

    #[test]
    fn threshold_violation_delta_max() {
        let mut metrics = make_metrics(0.5);
        metrics.avg_belief_entropy_final = 0.8;
        let thresholds = vec![Threshold {
            metric: "belief_entropy_final".to_owned(),
            min: None,
            max: Some(0.5),
        }];
        let violations = ThresholdChecker::check(&metrics, &thresholds);
        assert_eq!(violations.len(), 1);
        let v = &violations[0];
        // delta = max - actual = 0.5 - 0.8 = -0.3
        assert!((v.delta - (-0.3)).abs() < 1e-6, "delta was {}", v.delta);
    }

    #[test]
    fn threshold_violation_display() {
        let v = ThresholdViolation {
            threshold: Threshold {
                metric: "success_rate".to_owned(),
                min: Some(0.70),
                max: None,
            },
            actual: 0.42,
            delta: -0.28,
        };
        let s = v.to_string();
        assert!(s.contains("metric=success_rate"), "got: {s}");
        assert!(s.contains("actual=0.420"), "got: {s}");
        assert!(s.contains("threshold=min:0.700"), "got: {s}");
        assert!(s.contains("delta=-0.280"), "got: {s}");
    }

    #[test]
    fn no_zero_min_thresholds_in_default_suites() {
        for suite in default_suites() {
            for t in &suite.thresholds {
                if let Some(min) = t.min {
                    assert!(
                        (min - 0.0).abs() > 1e-9,
                        "suite '{}' metric '{}' has meaningless min=0.0 threshold",
                        suite.name,
                        t.metric
                    );
                }
            }
        }
    }

    #[test]
    fn default_suites_exclude_experimental_and_validation() {
        let all = all_suites();
        assert!(all
            .iter()
            .any(|suite| suite.group == SuiteGroup::Experimental));

        let default = default_suites();
        assert!(default.iter().all(|suite| suite.group.is_gating()));
        assert!(!default
            .iter()
            .any(|suite| suite.name == "inspection_perimeter_experimental"));
        assert!(!default
            .iter()
            .any(|suite| suite.name == "wildfire_medium_dynamic_greedy"));
        assert!(!default
            .iter()
            .any(|suite| suite.name == "realism_coverage_smoke"));
    }

    #[test]
    fn suites_by_group_returns_only_requested_group() {
        let experimental = suites_by_group(SuiteGroup::Experimental);
        assert!(!experimental.is_empty());
        assert!(experimental
            .iter()
            .all(|suite| suite.group == SuiteGroup::Experimental));

        let validation = suites_by_group(SuiteGroup::Validation);
        assert!(validation.is_empty());
    }

    #[test]
    fn experimental_threshold_violations_are_non_gating() {
        let suites = vec![make_suite(
            "experimental_failure",
            SuiteGroup::Experimental,
            SuiteMode::Smoke,
            1.0,
        )];
        let report = RegressionRunner::run(&suites, None, |_| {
            let mut map = HashMap::new();
            map.insert("greedy".to_owned(), make_metrics(0.5));
            map
        });

        assert!(report.overall_pass);
        assert_eq!(report.suite_results[0].status_label(), "NON-GATING-FAIL");
    }

    #[test]
    fn missing_baseline_entries_are_reported() {
        let suites = vec![make_suite(
            "missing_baseline",
            SuiteGroup::Smoke,
            SuiteMode::Smoke,
            0.1,
        )];
        let baseline = Baseline::from_suites(&[]);
        let report = RegressionRunner::run(&suites, Some(&baseline), |_| {
            let mut map = HashMap::new();
            map.insert("greedy".to_owned(), make_metrics(0.9));
            map
        });

        assert_eq!(report.missing_baselines, vec!["missing_baseline"]);
        let display = report.to_string();
        assert!(
            display.contains("## Missing Baseline Entries"),
            "got: {display}"
        );
        assert!(display.contains("missing_baseline"), "got: {display}");
    }

    #[test]
    fn failure_output_includes_reproduction_command_and_context() {
        let suites = vec![make_suite(
            "actionable_failure",
            SuiteGroup::Smoke,
            SuiteMode::Smoke,
            1.0,
        )];
        let report = RegressionRunner::run(&suites, None, |_| {
            let mut map = HashMap::new();
            map.insert("greedy".to_owned(), make_metrics(0.5));
            map
        });
        let display = report.to_string();

        assert!(display.contains("mission=coverage"), "got: {display}");
        assert!(display.contains("profile=ideal"), "got: {display}");
        assert!(display.contains("strategy=greedy"), "got: {display}");
        assert!(display.contains("metric=success_rate"), "got: {display}");
        assert!(display.contains("threshold=min:1.000"), "got: {display}");
        assert!(display.contains("delta=-0.500"), "got: {display}");
        assert!(
            display.contains(
                "cargo run -p swarm-examples --bin regression_runner -- --suite smoke --suite-name actionable_failure --jobs 1"
            ),
            "got: {display}"
        );
    }

    #[test]
    fn baseline_from_report_stores_metadata() {
        let suites = vec![make_suite(
            "metadata_suite",
            SuiteGroup::Quick,
            SuiteMode::Quick,
            0.1,
        )];
        let report = RegressionRunner::run(&suites, None, |_| {
            let mut map = HashMap::new();
            map.insert("greedy".to_owned(), make_metrics(0.9));
            map
        });
        let baseline = Baseline::from_report(&report, Some("quick"));

        assert_eq!(baseline.seed_range, Some((0, 9)));
        assert_eq!(baseline.seed_count, Some(10));
        assert_eq!(baseline.suite_group.as_deref(), Some("quick"));
        assert!(baseline.results.contains_key("metadata_suite"));
    }

    #[test]
    fn regression_report_serializes_to_json() {
        let suites = vec![make_suite(
            "json_suite",
            SuiteGroup::Smoke,
            SuiteMode::Smoke,
            0.1,
        )];
        let report = RegressionRunner::run(&suites, None, |_| {
            let mut map = HashMap::new();
            map.insert("greedy".to_owned(), make_metrics(0.9));
            map
        });
        let value = serde_json::to_value(&report).unwrap();

        assert_eq!(value["overall_pass"], true);
        assert_eq!(value["suite_results"][0]["suite"]["group"], "smoke");
        assert_eq!(value["suite_results"][0]["suite"]["mode"], "smoke");
        assert_eq!(value["suite_results"][0]["status"], "PASS");
        assert!(value["suite_results"][0]["reproduction_command"]
            .as_str()
            .unwrap()
            .contains("--suite smoke --suite-name json_suite --jobs 1"));
    }

    #[test]
    fn suite_result_has_correct_seed_range_for_smoke() {
        let suites = vec![RegressionSuite {
            name: "test_smoke".to_owned(),
            group: SuiteGroup::Smoke,
            mission: "coverage".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![],
            mode: SuiteMode::Smoke,
            realism: false,
        }];
        let report = RegressionRunner::run(&suites, None, |_| {
            let mut map = HashMap::new();
            map.insert("greedy".to_owned(), make_metrics(0.9));
            map
        });
        assert_eq!(report.suite_results[0].seed_range, (0, 0));
    }

    #[test]
    fn suite_result_has_correct_seed_range_for_quick() {
        let suites = vec![RegressionSuite {
            name: "test_quick".to_owned(),
            group: SuiteGroup::Quick,
            mission: "coverage".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![],
            mode: SuiteMode::Quick,
            realism: false,
        }];
        let report = RegressionRunner::run(&suites, None, |_| {
            let mut map = HashMap::new();
            map.insert("greedy".to_owned(), make_metrics(0.9));
            map
        });
        assert_eq!(report.suite_results[0].seed_range, (0, 9));
    }

    #[test]
    fn baseline_compare_improved() {
        let baseline = Baseline {
            version: "1.0".to_owned(),
            created_at: "2025-05-26T12:00:00Z".to_owned(),
            commit: "abc".to_owned(),
            seed_range: None,
            seed_count: None,
            suite_group: None,
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
            seed_range: None,
            seed_count: None,
            suite_group: None,
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
            seed_range: None,
            seed_count: None,
            suite_group: None,
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
            seed_range: None,
            seed_count: None,
            suite_group: None,
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
            group: SuiteGroup::Smoke,
            mission: "coverage".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "success_rate".to_owned(),
                min: Some(0.7),
                max: None,
            }],
            mode: SuiteMode::Smoke,
            realism: false,
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
            group: SuiteGroup::Smoke,
            mission: "coverage".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![Threshold {
                metric: "success_rate".to_owned(),
                min: Some(1.0),
                max: None,
            }],
            mode: SuiteMode::Smoke,
            realism: false,
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
            group: SuiteGroup::Smoke,
            mission: "inspection".to_owned(),
            profile: "linear".to_owned(),
            strategy: "all".to_owned(),
            thresholds: vec![Threshold {
                metric: "success_rate".to_owned(),
                min: Some(0.5),
                max: None,
            }],
            mode: SuiteMode::Smoke,
            realism: false,
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
