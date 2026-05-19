use serde::Serialize;

use crate::ComparisonReport;

/// Export a ComparisonReport to JSON.
pub fn export_json(report: &ComparisonReport) -> Result<String, serde_json::Error> {
    let mut rows = Vec::new();
    for strategy_name in &report.strategy_names {
        for profile_name in &report.profile_names {
            let key = (strategy_name.clone(), profile_name.clone());
            if let Some(metrics) = report.results.get(&key) {
                let row_id = format!(
                    "{}_{}_{}",
                    report.benchmark_run_id, strategy_name, profile_name
                );
                rows.push(ReportRow {
                    benchmark_run_id: report.benchmark_run_id.clone(),
                    run_id: row_id,
                    mission: report.mission_names.first().cloned().unwrap_or_default(),
                    scenario: report.scenario_names.first().cloned().unwrap_or_default(),
                    seed_range_start: report.seed_range_start,
                    seed_range_end: report.seed_range_end,
                    strategy: strategy_name.clone(),
                    profile: profile_name.clone(),
                    total_runs: metrics.total_runs,
                    success_rate: metrics.success_rate,
                    avg_task_completion_rate: metrics.avg_task_completion_rate,
                    avg_detection_ticks: metrics.avg_detection_ticks,
                    avg_reallocation_ticks: metrics.avg_reallocation_ticks,
                    avg_messages_attempted: metrics.avg_messages_attempted,
                    avg_messages_dropped: metrics.avg_messages_dropped,
                    avg_tasks_injected: metrics.avg_tasks_injected,
                    avg_tasks_expired: metrics.avg_tasks_expired,
                    avg_conflicting_assignments: metrics.avg_conflicting_assignments,
                    avg_network_availability: metrics.avg_network_availability,
                    avg_relay_reallocation_ticks: metrics.avg_relay_reallocation_ticks,
                    avg_avg_hop_count: metrics.avg_avg_hop_count,
                    avg_disconnected_agents_max: metrics.avg_disconnected_agents_max,
                    avg_coverage_progress: metrics.avg_coverage_progress,
                    avg_bytes_sent: metrics.avg_bytes_sent,
                    avg_stale_state_age_ticks: metrics.avg_stale_state_age_ticks,
                    avg_battery_margin_min: metrics.avg_battery_margin_min,
                    avg_battery_margin_avg: metrics.avg_battery_margin_avg,
                    time_to_find: if metrics.avg_time_to_find > 0.0 {
                        Some(metrics.avg_time_to_find)
                    } else {
                        None
                    },
                    probability_of_detection: metrics.avg_probability_of_detection,
                    targets_found: metrics.avg_targets_found,
                });
            }
        }
    }

    serde_json::to_string_pretty(&JsonReport {
        benchmark_run_id: report.benchmark_run_id.clone(),
        strategy_names: report.strategy_names.clone(),
        profile_names: report.profile_names.clone(),
        rows,
    })
}

/// Export a ComparisonReport to CSV.
pub fn export_csv(report: &ComparisonReport) -> Result<String, csv::Error> {
    let mut wtr = csv::Writer::from_writer(Vec::new());

    wtr.write_record([
        "benchmark_run_id",
        "run_id",
        "mission",
        "scenario",
        "seed_range_start",
        "seed_range_end",
        "strategy",
        "profile",
        "total_runs",
        "success_rate",
        "avg_task_completion_rate",
        "avg_detection_ticks",
        "avg_reallocation_ticks",
        "avg_messages_attempted",
        "avg_messages_dropped",
        "avg_tasks_injected",
        "avg_tasks_expired",
        "avg_conflicting_assignments",
        "avg_network_availability",
        "avg_relay_reallocation_ticks",
        "avg_avg_hop_count",
        "avg_disconnected_agents_max",
        "avg_coverage_progress",
        "avg_bytes_sent",
        "avg_stale_state_age_ticks",
        "avg_battery_margin_min",
        "avg_battery_margin_avg",
        "time_to_find",
        "probability_of_detection",
        "targets_found",
    ])?;

    let mission = report.mission_names.first().cloned().unwrap_or_default();
    let scenario = report.scenario_names.first().cloned().unwrap_or_default();

    for strategy_name in &report.strategy_names {
        for profile_name in &report.profile_names {
            let key = (strategy_name.clone(), profile_name.clone());
            if let Some(m) = report.results.get(&key) {
                let row_id = format!(
                    "{}_{}_{}",
                    report.benchmark_run_id, strategy_name, profile_name
                );
                wtr.write_record([
                    report.benchmark_run_id.as_str(),
                    row_id.as_str(),
                    mission.as_str(),
                    scenario.as_str(),
                    format!("{}", report.seed_range_start).as_str(),
                    format!("{}", report.seed_range_end).as_str(),
                    strategy_name,
                    profile_name,
                    m.total_runs.to_string().as_str(),
                    format!("{:.3}", m.success_rate).as_str(),
                    format!("{:.3}", m.avg_task_completion_rate).as_str(),
                    format!("{:.3}", m.avg_detection_ticks).as_str(),
                    format!("{:.3}", m.avg_reallocation_ticks).as_str(),
                    format!("{:.3}", m.avg_messages_attempted).as_str(),
                    format!("{:.3}", m.avg_messages_dropped).as_str(),
                    format!("{:.3}", m.avg_tasks_injected).as_str(),
                    format!("{:.3}", m.avg_tasks_expired).as_str(),
                    format!("{:.3}", m.avg_conflicting_assignments).as_str(),
                    format!("{:.3}", m.avg_network_availability).as_str(),
                    format!("{:.3}", m.avg_relay_reallocation_ticks).as_str(),
                    format!("{:.3}", m.avg_avg_hop_count).as_str(),
                    format!("{:.3}", m.avg_disconnected_agents_max).as_str(),
                    format!("{:.3}", m.avg_coverage_progress).as_str(),
                    format!("{:.3}", m.avg_bytes_sent).as_str(),
                    format!("{:.3}", m.avg_stale_state_age_ticks).as_str(),
                    format!("{:.3}", m.avg_battery_margin_min).as_str(),
                    format!("{:.3}", m.avg_battery_margin_avg).as_str(),
                    format!("{:.3}", m.avg_time_to_find).as_str(),
                    format!("{:.3}", m.avg_probability_of_detection).as_str(),
                    format!("{:.3}", m.avg_targets_found).as_str(),
                ])?;
            }
        }
    }

    wtr.flush()?;
    let bytes = wtr
        .into_inner()
        .map_err(|e| csv::Error::from(e.into_error()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[derive(Serialize)]
struct JsonReport {
    benchmark_run_id: String,
    strategy_names: Vec<String>,
    profile_names: Vec<String>,
    rows: Vec<ReportRow>,
}

#[derive(Serialize)]
struct ReportRow {
    benchmark_run_id: String,
    run_id: String,
    mission: String,
    scenario: String,
    seed_range_start: u64,
    seed_range_end: u64,
    strategy: String,
    profile: String,
    total_runs: u64,
    success_rate: f64,
    avg_task_completion_rate: f64,
    avg_detection_ticks: f64,
    avg_reallocation_ticks: f64,
    avg_messages_attempted: f64,
    avg_messages_dropped: f64,
    avg_tasks_injected: f64,
    avg_tasks_expired: f64,
    avg_conflicting_assignments: f64,
    avg_network_availability: f64,
    avg_relay_reallocation_ticks: f64,
    avg_avg_hop_count: f64,
    avg_disconnected_agents_max: f64,
    avg_coverage_progress: f64,
    avg_bytes_sent: f64,
    avg_stale_state_age_ticks: f64,
    avg_battery_margin_min: f64,
    avg_battery_margin_avg: f64,
    time_to_find: Option<f64>,
    probability_of_detection: f64,
    targets_found: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use swarm_metrics::AggregateMetrics;

    fn make_report() -> ComparisonReport {
        let mut results = HashMap::new();
        results.insert(
            ("greedy".to_owned(), "ideal".to_owned()),
            AggregateMetrics {
                total_runs: 10,
                success_rate: 1.0,
                avg_detection_ticks: 0.0,
                avg_reallocation_ticks: 0.0,
                avg_messages_attempted: 90.0,
                avg_messages_dropped: 0.0,
                avg_tasks_injected: 0.0,
                avg_tasks_expired: 0.0,
                avg_conflicting_assignments: 0.0,
                avg_network_availability: 1.0,
                avg_relay_reallocation_ticks: 0.0,
                avg_avg_hop_count: 0.0,
                avg_disconnected_agents_max: 0.0,
                avg_coverage_progress: 1.0,
                avg_bytes_sent: 3960.0,
                avg_stale_state_age_ticks: 0.0,
                avg_battery_margin_min: 100.0,
                avg_battery_margin_avg: 100.0,
                avg_task_completion_rate: 1.0,
                avg_time_to_find: 0.0,
                avg_probability_of_detection: 0.0,
                avg_targets_found: 0.0,
            },
        );
        ComparisonReport {
            benchmark_run_id: "test_10_quick".to_owned(),
            seed_range_start: 0,
            seed_range_end: 999,
            total_runs_per_cell: 10,
            mission_names: vec!["sar".to_owned()],
            scenario_names: vec!["sar_v1".to_owned()],
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["ideal".to_owned()],
            results,
        }
    }

    #[test]
    fn json_export_contains_benchmark_run_id() {
        let report = make_report();
        let json = export_json(&report).unwrap();
        assert!(json.contains("test_10_quick"));
        assert!(json.contains("benchmark_run_id"));
        assert!(json.contains("greedy"));
    }

    #[test]
    fn csv_export_contains_headers() {
        let report = make_report();
        let csv = export_csv(&report).unwrap();
        assert!(csv.contains("benchmark_run_id"));
        assert!(csv.contains("mission"));
        assert!(csv.contains("strategy"));
    }

    #[test]
    fn json_export_contains_mission_name() {
        let report = make_report();
        let json = export_json(&report).unwrap();
        assert!(json.contains("\"mission\""));
        assert!(json.contains("sar"));
    }

    #[test]
    fn csv_export_contains_mission_column() {
        let report = make_report();
        let csv = export_csv(&report).unwrap();
        assert!(csv.contains(",sar,"));
    }
}
