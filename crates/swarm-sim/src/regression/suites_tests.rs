#[cfg(test)]
mod tests {
    use super::super::*;
    use std::collections::HashMap;
    use swarm_metrics::AggregateMetrics;

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
            ..AggregateMetrics::default()
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
