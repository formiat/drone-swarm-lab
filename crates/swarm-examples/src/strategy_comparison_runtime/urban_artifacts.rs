use std::collections::HashMap;

use crate::regression_lib::{build_mission_scenario_builder, with_realism_if_needed};
use serde::Serialize;
use swarm_sim::{
    default_suites, Baseline, BenchmarkHarness, BenchmarkOptions, RegressionRunner, SuiteMode,
};

use super::cli::CliArgs;
use super::runs::{baseline_from_green_report, ensure_parent_dir};
use super::strategies::make_factories;

#[cfg(test)]
use super::runs::write_benchmark_pack;
#[cfg(test)]
use swarm_sim::{ComparisonReport, RegressionReport};

#[derive(Serialize)]
struct UrbanAnalysisManifest {
    schema_version: String,
    separation_threshold_m: f64,
    artifacts: Vec<UrbanAnalysisManifestEntry>,
}

#[derive(Serialize)]
struct UrbanAnalysisManifestEntry {
    replay_log_index: usize,
    run_id: String,
    scenario_name: String,
    route_trace_json: String,
    route_trace_csv: String,
    judge_report_json: String,
    judge_report_csv: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    segment_ownership_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    segment_ownership_csv: Option<String>,
    event_counts: swarm_sim::UrbanEventCounts,
    separation_summary: swarm_sim::UrbanSeparationSummary,
}

pub(super) fn write_urban_analysis_artifacts(
    output_dir: &str,
    replay_logs: &[swarm_replay::EventLog],
) -> Result<(), Box<dyn std::error::Error>> {
    let analysis_dir = format!("{output_dir}/urban_analysis");
    let mut artifacts = Vec::new();
    for (index, log) in replay_logs.iter().enumerate() {
        let event_counts = swarm_sim::count_urban_events(log);
        if !has_non_pose_urban_events(&event_counts) {
            continue;
        }

        std::fs::create_dir_all(&analysis_dir)?;
        let safe_run_id = format!("{index:03}_{}", sanitize_artifact_id(&log.run_id));
        let route_trace = swarm_sim::build_urban_route_trace(log);
        let judge_report = swarm_sim::build_urban_judge_report(log);
        let segment_ownership = swarm_sim::build_urban_segment_ownership_report(log);
        let separation_summary = swarm_sim::measure_urban_separation(
            &route_trace,
            swarm_sim::URBAN_ANALYSIS_DEFAULT_SEPARATION_THRESHOLD_M,
        );

        let route_trace_json = format!("urban_analysis/{safe_run_id}.route-trace.json");
        let route_trace_csv = format!("urban_analysis/{safe_run_id}.route-trace.csv");
        let judge_report_json = format!("urban_analysis/{safe_run_id}.judge-report.json");
        let judge_report_csv = format!("urban_analysis/{safe_run_id}.judge-report.csv");
        let segment_ownership_json = (!segment_ownership.records.is_empty())
            .then(|| format!("urban_analysis/{safe_run_id}.segment-ownership.json"));
        let segment_ownership_csv = (!segment_ownership.records.is_empty())
            .then(|| format!("urban_analysis/{safe_run_id}.segment-ownership.csv"));

        swarm_sim::write_urban_route_trace_json(
            &route_trace,
            format!("{output_dir}/{route_trace_json}"),
        )?;
        swarm_sim::write_urban_route_trace_csv(
            &route_trace,
            format!("{output_dir}/{route_trace_csv}"),
        )?;
        swarm_sim::write_urban_judge_report_json(
            &judge_report,
            format!("{output_dir}/{judge_report_json}"),
        )?;
        swarm_sim::write_urban_judge_report_csv(
            &judge_report,
            format!("{output_dir}/{judge_report_csv}"),
        )?;
        if let Some(path) = &segment_ownership_json {
            swarm_sim::write_urban_segment_ownership_json(
                &segment_ownership,
                format!("{output_dir}/{path}"),
            )?;
        }
        if let Some(path) = &segment_ownership_csv {
            swarm_sim::write_urban_segment_ownership_csv(
                &segment_ownership,
                format!("{output_dir}/{path}"),
            )?;
        }

        artifacts.push(UrbanAnalysisManifestEntry {
            replay_log_index: index,
            run_id: log.run_id.clone(),
            scenario_name: log.scenario_name.clone(),
            route_trace_json,
            route_trace_csv,
            judge_report_json,
            judge_report_csv,
            segment_ownership_json,
            segment_ownership_csv,
            event_counts,
            separation_summary,
        });
    }

    if artifacts.is_empty() {
        return Ok(());
    }

    let manifest = UrbanAnalysisManifest {
        schema_version: "urban_analysis.v1".to_owned(),
        separation_threshold_m: swarm_sim::URBAN_ANALYSIS_DEFAULT_SEPARATION_THRESHOLD_M,
        artifacts,
    };
    std::fs::write(
        format!("{analysis_dir}/manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    println!("Urban analysis artifacts written to {analysis_dir}");
    Ok(())
}

pub(super) fn sanitize_artifact_id(value: &str) -> String {
    value.replace(['/', '\\', ':'], "_")
}

fn has_non_pose_urban_events(counts: &swarm_sim::UrbanEventCounts) -> bool {
    counts.route_planned
        + counts.segment_entered
        + counts.segment_completed
        + counts.violation
        + counts.patrol_completed
        + counts.bus_observed
        + counts.bus_detected
        + counts.bus_false_positive
        + counts.search_completed
        + counts.edge_blocked
        + counts.edge_unblocked
        + counts.route_replanned
        + counts.wait_started
        + counts.wait_completed
        + counts.no_route_available
        + counts.segment_lock_acquired
        + counts.segment_lock_released
        + counts.segment_conflict
        + counts.deconflict_wait
        + counts.deconflict_replan
        + counts.deconflict_abort
        > 0
}

pub(super) fn merge_reports(
    reports: &[swarm_sim::ComparisonReport],
) -> swarm_sim::ComparisonReport {
    use std::collections::HashMap;
    let first = &reports[0];
    let mut merged_results: HashMap<(String, String), swarm_metrics::AggregateMetrics> =
        HashMap::new();
    for report in reports {
        for strategy_name in &report.strategy_names {
            for profile_name in &report.profile_names {
                let key = (strategy_name.clone(), profile_name.clone());
                if let Some(metrics) = report.results.get(&key) {
                    let scoped_profile = format!("{}/{}", metrics.mission, profile_name);
                    let scoped_key = (strategy_name.clone(), scoped_profile);
                    merged_results.insert(scoped_key, metrics.clone());
                }
            }
        }
    }
    // Collect unique mission-scoped profile names in original order across reports
    let mut all_profile_names = Vec::new();
    for r in reports {
        for name in &r.profile_names {
            for strategy_name in &r.strategy_names {
                let key = (strategy_name.clone(), name.clone());
                if let Some(metrics) = r.results.get(&key) {
                    let scoped = format!("{}/{}", metrics.mission, name);
                    if !all_profile_names.contains(&scoped) {
                        all_profile_names.push(scoped);
                    }
                }
            }
        }
    }
    swarm_sim::ComparisonReport {
        benchmark_run_id: swarm_sim::merged_benchmark_run_id(reports),
        seed_range_start: reports
            .iter()
            .map(|r| r.seed_range_start)
            .min()
            .unwrap_or(0),
        seed_range_end: reports.iter().map(|r| r.seed_range_end).max().unwrap_or(0),
        total_runs_per_cell: first.total_runs_per_cell,
        mission_names: reports
            .iter()
            .flat_map(|r| r.mission_names.clone())
            .collect(),
        scenario_names: reports
            .iter()
            .flat_map(|r| r.scenario_names.clone())
            .collect(),
        strategy_names: first.strategy_names.clone(),
        profile_names: all_profile_names,
        results: merged_results,
    }
}

pub(super) fn run_regression(cli: &CliArgs) {
    let baseline = cli
        .compare_baseline
        .as_ref()
        .and_then(|path| Baseline::load(path).ok());

    let suites = default_suites();
    let factories = make_factories(&cli.planner);

    let report = RegressionRunner::run(&suites, baseline.as_ref(), |suite| {
        let mut builder = build_mission_scenario_builder(&suite.mission).unwrap_or_else(|| {
            eprintln!("Unknown mission: {}", suite.mission);
            std::process::exit(1);
        });
        builder = with_realism_if_needed(builder, suite);

        let profile_names = vec![suite.profile.clone()];
        let result = match suite.mode {
            SuiteMode::Smoke => BenchmarkHarness::run_smoke_with_options(
                &factories,
                &profile_names,
                &builder,
                BenchmarkOptions {
                    prefix: Some(&suite.name),
                    enable_replay_log: false,
                    mission_name: &suite.mission,
                    jobs: cli.jobs,
                },
            ),
            SuiteMode::Quick => BenchmarkHarness::run_quick_with_options(
                &factories,
                &profile_names,
                &builder,
                BenchmarkOptions {
                    prefix: Some(&suite.name),
                    enable_replay_log: false,
                    mission_name: &suite.mission,
                    jobs: cli.jobs,
                },
            ),
        };

        let mut metrics_map = HashMap::new();
        for (strategy_name, _profile_name) in result.report.results.keys() {
            let key = (strategy_name.clone(), suite.profile.clone());
            if let Some(metrics) = result.report.results.get(&key) {
                metrics_map.insert(strategy_name.clone(), metrics.clone());
            }
        }
        metrics_map
    });

    println!("{}", report);

    if let Some(path) = &cli.update_baseline {
        let baseline = baseline_from_green_report(&report, "default").unwrap_or_else(|reason| {
            eprintln!("Refusing to update baseline from a report with {reason}");
            std::process::exit(1);
        });
        if let Err(e) = ensure_parent_dir(path) {
            eprintln!("Failed to create baseline parent directory: {}", e);
            std::process::exit(1);
        }
        if let Err(e) = baseline.save(path) {
            eprintln!("Failed to save baseline: {}", e);
            std::process::exit(1);
        }
        println!("Baseline saved to {}", path);
    }

    std::process::exit(if report.overall_pass { 0 } else { 1 });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn benchmark_pack_report() -> ComparisonReport {
        let mut results = HashMap::new();
        results.insert(
            ("greedy".to_owned(), "ideal".to_owned()),
            swarm_metrics::AggregateMetrics {
                total_runs: 1,
                success_rate: 1.0,
                avg_task_completion_rate: 1.0,
                mission: "coverage".to_owned(),
                scenario: "coverage".to_owned(),
                ..swarm_metrics::AggregateMetrics::default()
            },
        );

        ComparisonReport {
            benchmark_run_id: "test_1_smoke".to_owned(),
            seed_range_start: 0,
            seed_range_end: 1,
            total_runs_per_cell: 1,
            mission_names: vec!["coverage".to_owned()],
            scenario_names: vec!["coverage".to_owned()],
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["ideal".to_owned()],
            results,
        }
    }

    fn regression_report(has_threshold_violations: bool) -> RegressionReport {
        let threshold = swarm_sim::Threshold {
            metric: "success_rate".to_owned(),
            min: Some(0.9),
            max: None,
        };
        let violations = if has_threshold_violations {
            vec![swarm_sim::ThresholdViolation {
                threshold: threshold.clone(),
                actual: 0.5,
                delta: -0.4,
            }]
        } else {
            Vec::new()
        };
        let metrics = swarm_metrics::AggregateMetrics {
            total_runs: 10,
            success_rate: 0.95,
            mission: "coverage".to_owned(),
            scenario: "ideal".to_owned(),
            ..swarm_metrics::AggregateMetrics::default()
        };

        RegressionReport {
            suite_results: vec![swarm_sim::SuiteResult {
                suite: swarm_sim::RegressionSuite {
                    name: "strategy_comparison_regression".to_owned(),
                    group: swarm_sim::SuiteGroup::Quick,
                    mission: "coverage".to_owned(),
                    profile: "ideal".to_owned(),
                    strategy: "greedy".to_owned(),
                    thresholds: vec![threshold],
                    mode: swarm_sim::SuiteMode::Quick,
                    realism: false,
                },
                actual_strategy: "greedy".to_owned(),
                metrics,
                violations,
                seed_range: (0, 9),
            }],
            deltas: Vec::new(),
            missing_baselines: Vec::new(),
            overall_pass: !has_threshold_violations,
        }
    }

    #[test]
    fn benchmark_pack_manifest_records_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let report = benchmark_pack_report();

        write_benchmark_pack(
            dir.path().to_str().unwrap(),
            &report,
            None,
            &[],
            None,
            Some(14),
            "benchmark",
        )
        .unwrap();

        let manifest_path = dir.path().join("manifest.json");
        let manifest: swarm_sim::BenchmarkManifest =
            serde_json::from_str(&std::fs::read_to_string(manifest_path).unwrap()).unwrap();
        assert_eq!(manifest.jobs, Some(14));
        assert_eq!(manifest.suite_name, "coverage");
        assert_eq!(manifest.artifact_kind, "benchmark");
        assert!(manifest.build_profile.is_some());

        assert!(dir.path().join("results.json").exists());
        assert!(dir.path().join("results.csv").exists());
        assert!(dir.path().join("table.md").exists());
    }

    #[test]
    fn regression_baseline_update_refuses_threshold_violations() {
        let report = regression_report(true);

        assert_eq!(
            baseline_from_green_report(&report, "default").unwrap_err(),
            "threshold violations"
        );
    }

    #[test]
    fn regression_baseline_update_stores_m42_metadata() {
        let report = regression_report(false);
        let baseline = baseline_from_green_report(&report, "default").unwrap();

        assert_eq!(baseline.seed_range, Some((0, 9)));
        assert_eq!(baseline.seed_count, Some(10));
        assert_eq!(baseline.suite_group.as_deref(), Some("default"));
        assert!(baseline
            .results
            .contains_key("strategy_comparison_regression"));
    }
}
