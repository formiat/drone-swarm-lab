fn compare_aggregate_metrics(
    key: &(String, String),
    a: &AggregateMetrics,
    b: &AggregateMetrics,
    errors: &mut Vec<String>,
) {
    let errors_before = errors.len();
    macro_rules! compare_field {
        ($field:ident) => {
            compare_metric_field(errors, key, stringify!($field), &a.$field, &b.$field);
        };
    }

    compare_field!(total_runs);
    compare_field!(success_rate);
    compare_field!(avg_detection_ticks);
    compare_field!(avg_reallocation_ticks);
    compare_field!(avg_messages_attempted);
    compare_field!(avg_messages_dropped);
    compare_field!(avg_tasks_injected);
    compare_field!(avg_tasks_expired);
    compare_field!(avg_conflicting_assignments);
    compare_field!(avg_network_availability);
    compare_field!(avg_relay_reallocation_ticks);
    compare_field!(avg_avg_hop_count);
    compare_field!(avg_disconnected_agents_max);
    compare_field!(avg_coverage_progress);
    compare_field!(avg_bytes_sent);
    compare_field!(avg_stale_state_age_ticks);
    compare_field!(avg_battery_margin_min);
    compare_field!(avg_battery_margin_avg);
    compare_field!(avg_task_completion_rate);
    compare_field!(avg_time_to_find);
    compare_field!(avg_probability_of_detection);
    compare_field!(avg_targets_found);
    compare_field!(avg_safety_violations);
    compare_field!(avg_belief_entropy_final);
    compare_field!(avg_false_positive_rate);
    compare_field!(avg_confirmation_scans);
    compare_field!(convergence_ticks_p50);
    compare_field!(convergence_ticks_p95);
    compare_field!(convergence_ticks_max);
    compare_field!(avg_bundle_travel_distance);
    compare_field!(avg_edge_coverage_rate);
    compare_field!(avg_missed_edges);
    compare_field!(avg_revisit_count);
    compare_field!(avg_route_efficiency);
    compare_field!(avg_route_length);
    compare_field!(avg_wasted_travel);
    compare_field!(avg_return_reserve);
    compare_field!(avg_infeasible_routes);
    compare_field!(avg_hazard_zones_mapped);
    compare_field!(avg_priority_updates);
    compare_field!(avg_final_threat_level);
    compare_field!(avg_high_priority_zones_mapped);
    compare_field!(avg_time_to_map_first_high_risk);
    compare_field!(avg_zone_observations);
    compare_field!(avg_urban_route_length_m);
    compare_field!(avg_urban_route_risk_score);
    compare_field!(urban_route_planned_rate);
    compare_field!(avg_urban_violation_count);
    compare_field!(urban_route_completed_rate);
    compare_field!(urban_patrol_completed_rate);
    compare_field!(avg_urban_time_to_complete_loop);
    compare_field!(avg_urban_distance_travelled_m);
    compare_field!(avg_urban_route_efficiency);
    compare_field!(avg_urban_replan_count);
    compare_field!(bus_detection_rate);
    compare_field!(avg_time_to_detect_bus);
    compare_field!(avg_false_positive_count);
    compare_field!(avg_distance_before_detection);
    compare_field!(search_success_without_violation_rate);
    compare_field!(avg_urban_min_agent_separation_m);
    compare_field!(avg_urban_separation_violation_count);
    compare_field!(avg_urban_route_conflict_count);
    compare_field!(mission);
    compare_field!(scenario);

    if a != b && errors.len() == errors_before {
        errors.push(format!(
            "key {key:?}: aggregate metrics differ in an unlisted field"
        ));
    }
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
                mission: "sar".to_owned(),
                scenario: "sar".to_owned(),
                ..AggregateMetrics::default()
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

    fn make_urban_report() -> ComparisonReport {
        let mut report = make_report();
        report.mission_names = vec!["urban-patrol".to_owned()];
        report.scenario_names = vec!["urban_patrol_small_block".to_owned()];
        let metrics = report
            .results
            .get_mut(&("greedy".to_owned(), "ideal".to_owned()))
            .expect("test report should contain greedy/ideal metrics");
        metrics.mission = "urban-patrol".to_owned();
        metrics.scenario = "urban_patrol_small_block".to_owned();
        metrics.avg_urban_route_length_m = 80.0;
        metrics.avg_urban_route_risk_score = 22.0;
        metrics.urban_route_planned_rate = 1.0;
        metrics.avg_urban_violation_count = 0.0;
        metrics.urban_route_completed_rate = 1.0;
        metrics.urban_patrol_completed_rate = 1.0;
        metrics.avg_urban_time_to_complete_loop = 40.0;
        metrics.avg_urban_distance_travelled_m = 80.0;
        metrics.avg_urban_route_efficiency = 1.0;
        metrics.avg_urban_replan_count = 0.0;
        report
    }

    fn make_urban_search_report() -> ComparisonReport {
        let mut report = make_report();
        report.mission_names = vec!["urban-search".to_owned()];
        report.scenario_names = vec!["urban_search_static_bus".to_owned()];
        let metrics = report
            .results
            .get_mut(&("greedy".to_owned(), "ideal".to_owned()))
            .expect("test report should contain greedy/ideal metrics");
        metrics.mission = "urban-search".to_owned();
        metrics.scenario = "urban_search_static_bus".to_owned();
        metrics.bus_detection_rate = 1.0;
        metrics.avg_time_to_detect_bus = 2.0;
        metrics.avg_false_positive_count = 0.0;
        metrics.avg_distance_before_detection = 4.0;
        metrics.search_success_without_violation_rate = 1.0;
        metrics.avg_urban_violation_count = 0.0;
        metrics.avg_urban_route_efficiency = 1.0;
        report
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
        assert!(csv.contains("avg_urban_route_length_m"));
        assert!(csv.contains("avg_urban_route_risk_score"));
        assert!(csv.contains("urban_route_planned_rate"));
        assert!(csv.contains("avg_urban_violation_count"));
        assert!(csv.contains("urban_route_completed_rate"));
        assert!(csv.contains("urban_patrol_completed_rate"));
        assert!(csv.contains("avg_urban_time_to_complete_loop"));
        assert!(csv.contains("avg_urban_distance_travelled_m"));
        assert!(csv.contains("avg_urban_route_efficiency"));
        assert!(csv.contains("avg_urban_replan_count"));
        assert!(csv.contains("bus_detection_rate"));
        assert!(csv.contains("avg_time_to_detect_bus"));
        assert!(csv.contains("avg_false_positive_count"));
        assert!(csv.contains("avg_distance_before_detection"));
        assert!(csv.contains("search_success_without_violation_rate"));
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

    #[test]
    fn benchmark_manifest_serde_roundtrip() {
        let manifest = BenchmarkManifest {
            timestamp: "2024-01-01T00:00:00Z".to_owned(),
            git_commit: "abc123".to_owned(),
            command_line: "test".to_owned(),
            suite_name: "coverage".to_owned(),
            schema_version: "0.1".to_owned(),
            seed_range_start: 0,
            seed_range_end: 9,
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["ideal".to_owned()],
            metric_schema_version: "0.1".to_owned(),
            realism_profile: None,
            wind_enabled: false,
            pose_noise_m: 0.0,
            comms_jitter_ticks: 0,
            battery_model: None,
            jobs: None,
            build_profile: None,
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let decoded: BenchmarkManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.git_commit, "abc123");
        assert_eq!(decoded.suite_name, "coverage");
        assert_eq!(decoded.seed_range_end, 9);
    }

    #[test]
    fn export_markdown_contains_header() {
        let report = make_report();
        let md = export_markdown(&report);
        assert!(md.contains("| Strategy"));
        assert!(md.contains("|"));
    }

    #[test]
    fn export_markdown_contains_urban_metric_columns() {
        let report = make_urban_report();
        let md = export_markdown(&report);
        assert!(md.contains("UrbanRouteLength"));
        assert!(md.contains("UrbanRisk"));
        assert!(md.contains("UrbanPlanned"));
        assert!(md.contains("UrbanViolations"));
        assert!(md.contains("UrbanCompleted"));
        assert!(md.contains("PatrolCompleted"));
        assert!(md.contains("TimeToLoop"));
        assert!(md.contains("RouteEfficiency"));
        assert!(md.contains("80.000"));
    }

    #[test]
    fn benchmark_manifest_new_has_git_commit() {
        let manifest = BenchmarkManifest::new(
            "test_suite",
            0,
            1,
            vec!["greedy".to_owned()],
            vec!["ideal".to_owned()],
        );
        assert!(!manifest.git_commit.is_empty());
        assert!(!manifest.timestamp.is_empty());
        assert_eq!(manifest.schema_version, "0.1");
        assert_eq!(manifest.metric_schema_version, "0.1");
        assert!(manifest.build_profile.is_some());
    }

    #[test]
    fn focused_report_contains_mission_sections() {
        let report = make_report();
        let focused = generate_focused_report(&[("sar".to_owned(), report)]);
        assert!(focused.contains("# Benchmark Report"));
        assert!(focused.contains("## sar"));
        assert!(focused.contains("## Answers to Key Questions"));
        assert!(focused.contains("Where does CBBA win?"));
    }

    #[test]
    fn focused_report_has_summary_table() {
        let report = make_report();
        let focused = generate_focused_report(&[("sar".to_owned(), report)]);
        assert!(focused.contains("| Strategy"));
        assert!(focused.contains("| Profile"));
    }

    #[test]
    fn focused_report_has_urban_patrol_metrics() {
        let report = make_urban_report();
        let focused = generate_focused_report(&[("urban-patrol".to_owned(), report)]);
        assert!(focused.contains("## urban-patrol"));
        assert!(focused.contains("UrbanRouteLength"));
        assert!(focused.contains("UrbanRisk"));
        assert!(focused.contains("UrbanPlanned"));
        assert!(focused.contains("UrbanViolations"));
        assert!(focused.contains("UrbanCompleted"));
        assert!(focused.contains("PatrolCompleted"));
        assert!(focused.contains("TimeToLoop"));
        assert!(focused.contains("RouteEfficiency"));
        assert!(focused.contains("80.000"));
    }

    #[test]
    fn focused_report_has_urban_search_metrics() {
        let report = make_urban_search_report();
        let focused = generate_focused_report(&[("urban-search".to_owned(), report)]);
        assert!(focused.contains("## urban-search"));
        assert!(focused.contains("BusDetected"));
        assert!(focused.contains("TimeToBus"));
        assert!(focused.contains("FalsePositives"));
        assert!(focused.contains("DistanceBeforeBus"));
        assert!(focused.contains("SearchSuccessNoViolation"));
        assert!(focused.contains("4.000"));
    }

    #[test]
    fn benchmark_manifest_jobs_field_roundtrips() {
        let mut manifest = BenchmarkManifest::new(
            "test",
            0,
            1,
            vec!["greedy".to_owned()],
            vec!["ideal".to_owned()],
        );
        manifest.jobs = Some(4);
        let json = serde_json::to_string(&manifest).unwrap();
        let decoded: BenchmarkManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.jobs, Some(4));
    }

    #[test]
    fn benchmark_manifest_jobs_default_is_none() {
        let manifest = BenchmarkManifest::new(
            "test",
            0,
            1,
            vec!["greedy".to_owned()],
            vec!["ideal".to_owned()],
        );
        assert!(manifest.jobs.is_none());
        // Old manifests without the field deserialize to None.
        let json_without_jobs = r#"{"timestamp":"t","git_commit":"abc","command_line":"c","suite_name":"s","schema_version":"0.1","seed_range_start":0,"seed_range_end":1,"strategy_names":[],"profile_names":[],"metric_schema_version":"0.1"}"#;
        let decoded: BenchmarkManifest = serde_json::from_str(json_without_jobs).unwrap();
        assert!(decoded.jobs.is_none());
        assert!(decoded.build_profile.is_none());
    }

    fn make_aggregate(mission: &str, scenario: &str, success_rate: f64) -> AggregateMetrics {
        AggregateMetrics {
            total_runs: 1,
            success_rate,
            mission: mission.to_owned(),
            scenario: scenario.to_owned(),
            avg_network_availability: 1.0,
            avg_task_completion_rate: 1.0,
            ..AggregateMetrics::default()
        }
    }

    fn make_report_for_comparison(mission: &str, success: f64) -> crate::ComparisonReport {
        let mut results = HashMap::new();
        results.insert(
            ("greedy".to_owned(), "ideal".to_owned()),
            make_aggregate(mission, mission, success),
        );
        crate::ComparisonReport {
            benchmark_run_id: "ignored".to_owned(),
            seed_range_start: 0,
            seed_range_end: 1,
            total_runs_per_cell: 1,
            mission_names: vec![mission.to_owned()],
            scenario_names: vec![],
            strategy_names: vec!["greedy".to_owned()],
            profile_names: vec!["ideal".to_owned()],
            results,
        }
    }

    #[test]
    fn compare_reports_identical_ok() {
        let r = make_report_for_comparison("sar", 0.9);
        assert!(compare_reports(&r, &r).is_ok());
    }

    #[test]
    fn compare_reports_detects_success_rate_mismatch() {
        let r1 = make_report_for_comparison("sar", 1.0);
        let r2 = make_report_for_comparison("sar", 0.0);
        let err = compare_reports(&r1, &r2).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("success_rate")),
            "should report success_rate mismatch, got: {err:?}"
        );
    }

    #[test]
    fn compare_reports_detects_strategy_set_mismatch() {
        let r1 = make_report_for_comparison("sar", 1.0);
        let mut r2 = make_report_for_comparison("sar", 1.0);
        r2.strategy_names.push("cbba".to_owned());
        let err = compare_reports(&r1, &r2).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("strategy_names")),
            "should report strategy_names mismatch, got: {err:?}"
        );
    }

    #[test]
    fn compare_reports_detects_row_count_mismatch() {
        let r1 = make_report_for_comparison("sar", 1.0);
        let mut r2 = make_report_for_comparison("sar", 1.0);
        r2.results.insert(
            ("cbba".to_owned(), "ideal".to_owned()),
            make_aggregate("sar", "sar", 1.0),
        );
        let err = compare_reports(&r1, &r2).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("row count")),
            "should report row count mismatch, got: {err:?}"
        );
    }

    #[test]
    fn compare_reports_detects_scenario_mismatch() {
        let r1 = make_report_for_comparison("sar", 1.0);
        let mut r2 = make_report_for_comparison("sar", 1.0);
        r2.results
            .get_mut(&("greedy".to_owned(), "ideal".to_owned()))
            .unwrap()
            .scenario = "sar-v2".to_owned();
        let err = compare_reports(&r1, &r2).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("scenario")),
            "should report scenario mismatch, got: {err:?}"
        );
    }

    #[test]
    fn compare_reports_detects_empty_identity() {
        let r1 = make_report_for_comparison("sar", 1.0);
        let mut r2 = make_report_for_comparison("sar", 1.0);
        r2.results
            .get_mut(&("greedy".to_owned(), "ideal".to_owned()))
            .unwrap()
            .mission
            .clear();
        let err = compare_reports(&r1, &r2).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("empty mission")),
            "should report empty mission, got: {err:?}"
        );
    }

    #[test]
    fn compare_reports_detects_unlisted_metric_mismatch() {
        let r1 = make_report_for_comparison("sar", 1.0);
        let mut r2 = make_report_for_comparison("sar", 1.0);
        r2.results
            .get_mut(&("greedy".to_owned(), "ideal".to_owned()))
            .unwrap()
            .avg_bytes_sent = 42.0;
        let err = compare_reports(&r1, &r2).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("avg_bytes_sent")),
            "should report avg_bytes_sent mismatch, got: {err:?}"
        );
    }

    #[test]
    fn compare_reports_detects_duplicate_visible_identity() {
        let mut r = make_report_for_comparison("sar", 1.0);
        r.strategy_names.push("greedy".to_owned());
        let err = compare_reports(&r, &r).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("duplicate")),
            "should report duplicate identity, got: {err:?}"
        );
    }

    #[test]
    fn report_identity_matches_json_csv_markdown() {
        let report = make_report();
        let json = export_json(&report).unwrap();
        let json_value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let json_row = &json_value["rows"][0];
        let json_identity = (
            json_row["mission"].as_str().unwrap().to_owned(),
            json_row["scenario"].as_str().unwrap().to_owned(),
            json_row["strategy"].as_str().unwrap().to_owned(),
            json_row["profile"].as_str().unwrap().to_owned(),
        );

        let csv = export_csv(&report).unwrap();
        let mut reader = csv::Reader::from_reader(csv.as_bytes());
        let csv_row = reader.records().next().unwrap().unwrap();
        let csv_identity = (
            csv_row.get(2).unwrap().to_owned(),
            csv_row.get(3).unwrap().to_owned(),
            csv_row.get(6).unwrap().to_owned(),
            csv_row.get(7).unwrap().to_owned(),
        );

        let markdown = export_markdown(&report);
        let markdown_row = markdown
            .lines()
            .find(|line| line.contains("| sar") && line.contains("| greedy"))
            .unwrap();
        let cells: Vec<String> = markdown_row
            .split('|')
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .map(str::to_owned)
            .collect();
        let markdown_identity = (
            cells[0].clone(),
            cells[1].clone(),
            cells[2].clone(),
            cells[3].clone(),
        );

        assert_eq!(json_identity, csv_identity);
        assert_eq!(json_identity, markdown_identity);
    }
}
