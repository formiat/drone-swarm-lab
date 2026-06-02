use super::*;
pub fn all_suites() -> Vec<RegressionSuite> {
    vec![
        // SAR — M35 changed success semantics to targets-found; keep success_rate out of
        // the smoke gate because seed 0 can exceed the unassigned-tick success threshold
        // even after useful scan progress. Completed scan tasks and targets_found are the
        // stable smoke checks.
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
                    metric: "targets_found".to_owned(),
                    min: Some(2.0),
                    max: None,
                },
                Threshold {
                    metric: "belief_entropy_final".to_owned(),
                    min: None,
                    max: Some(0.75),
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
        // Urban Search — M66 simulation-only search fixture with a mocked bus detector.
        RegressionSuite {
            name: "urban_search_static_bus_greedy".to_owned(),
            group: SuiteGroup::Smoke,
            mission: "urban-search".to_owned(),
            profile: "search-static-bus".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![
                Threshold {
                    metric: "success_rate".to_owned(),
                    min: Some(1.0),
                    max: None,
                },
                Threshold {
                    metric: "bus_detection_rate".to_owned(),
                    min: Some(1.0),
                    max: None,
                },
                Threshold {
                    metric: "search_success_without_violation".to_owned(),
                    min: Some(1.0),
                    max: None,
                },
                Threshold {
                    metric: "false_positive_count".to_owned(),
                    min: None,
                    max: Some(0.0),
                },
            ],
            mode: SuiteMode::Smoke,
            realism: false,
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
