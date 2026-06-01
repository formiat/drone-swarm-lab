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
        "bus_detection_rate" => metrics.bus_detection_rate,
        "time_to_detect_bus" => metrics.avg_time_to_detect_bus,
        "false_positive_count" => metrics.avg_false_positive_count,
        "distance_before_detection" => metrics.avg_distance_before_detection,
        "search_success_without_violation" => metrics.search_success_without_violation_rate,
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
