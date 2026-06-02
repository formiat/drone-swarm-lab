use super::*;
use crate::{RunConfig, Scenario};
use swarm_alloc::Strategy;
use swarm_alloc::{AllocationAgent, AllocationTask, CentralizedPlanner, GreedyAllocator};
use swarm_types::{Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskStatus};

fn make_scenario_builder() -> ScenarioBuilder {
    Box::new(|seed: u64, _profile: &str| {
        let agents: Vec<Agent> = (0..5)
            .map(|i| Agent {
                id: AgentId::from(format!("agent-{i}")),
                role: Role::Scout,
                health: Health::Alive,
                pose: Pose {
                    x: 0.0,
                    y: 0.0,
                    ..Default::default()
                },
                capabilities: vec![],
                current_task: None,
                battery: 100.0,
                comms_range: f64::INFINITY,
                generation: 1,
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
                battery_model: None,
            })
            .collect();
        let tasks: Vec<Task> = (0..5)
            .map(|i| Task {
                id: TaskId::from(format!("task-{i}")),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: 1,
                required_capabilities: vec![],
                required_role: None,
                preferred_role: None,
                expires_at: None,
                grid_cell: None,
                edge_id: None,
                pose: None,
                kind: None,
            })
            .collect();
        let scenario = Scenario {
            name: "test".to_owned(),
            seed,
            agents,
            tasks,
            ground_nodes: vec![],
            base_station: None,
            geo_origin: None,
        };
        let run_config = RunConfig {
            max_ticks: 50,
            timeout_ticks: 3,
            max_unassigned_ticks: 10,
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            latency_per_hop: 0,
            failures: vec![],
            dynamic_tasks: vec![],
            partition_events: vec![],
            gossip_interval_ticks: 999,
            base_id: None,
            enable_movement: false,
            grid_state: None,
            tick_duration_ms: 100,
            enable_cbba: false,
            ..Default::default()
        };
        (scenario, run_config)
    })
}

#[test]
fn harness_runs_and_produces_report() {
    let factories: Vec<StrategyFactory> =
        vec![Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
            Box::new(GreedyAllocator) as Box<dyn Strategy>
        })];
    let profiles = vec!["ideal".to_owned()];
    let builder = make_scenario_builder();
    let report = BenchmarkHarness::run_quick(&factories, &profiles, &builder);
    assert!(report
        .results
        .contains_key(&("greedy".to_owned(), "ideal".to_owned())));
}

#[test]
fn centralized_present_in_report() {
    let factories: Vec<StrategyFactory> = vec![
        Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
            Box::new(GreedyAllocator) as Box<dyn Strategy>
        }),
        Box::new(|scenario: &Scenario, _run_config: &RunConfig| {
            let allocation_tasks: Vec<AllocationTask<'_>> = scenario
                .tasks
                .iter()
                .map(|t| AllocationTask { task: t })
                .collect();
            let allocation_agents: Vec<AllocationAgent> = scenario
                .agents
                .iter()
                .map(|a| AllocationAgent {
                    id: a.id.clone(),
                    pose: a.pose,
                    battery: a.battery,
                    capabilities: a.capabilities.clone(),
                    role: a.role.clone(),
                    comms_range: a.comms_range,
                    speed: 0.0,
                    max_range: 0.0,
                    battery_drain_rate: 0.0,
                })
                .collect();
            Box::new(CentralizedPlanner::new(
                &allocation_tasks,
                &allocation_agents,
            )) as Box<dyn Strategy>
        }),
    ];
    let profiles = vec!["ideal".to_owned()];
    let builder = make_scenario_builder();
    let report = BenchmarkHarness::run_quick(&factories, &profiles, &builder);
    assert!(report
        .results
        .contains_key(&("centralized".to_owned(), "ideal".to_owned())));
}

#[test]
fn centralized_matches_or_beats_greedy_on_ideal() {
    let factories: Vec<StrategyFactory> = vec![
        Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
            Box::new(GreedyAllocator) as Box<dyn Strategy>
        }),
        Box::new(|scenario: &Scenario, _run_config: &RunConfig| {
            let allocation_tasks: Vec<AllocationTask<'_>> = scenario
                .tasks
                .iter()
                .map(|t| AllocationTask { task: t })
                .collect();
            let allocation_agents: Vec<AllocationAgent> = scenario
                .agents
                .iter()
                .map(|a| AllocationAgent {
                    id: a.id.clone(),
                    pose: a.pose,
                    battery: a.battery,
                    capabilities: a.capabilities.clone(),
                    role: a.role.clone(),
                    comms_range: a.comms_range,
                    speed: 0.0,
                    max_range: 0.0,
                    battery_drain_rate: 0.0,
                })
                .collect();
            Box::new(CentralizedPlanner::new(
                &allocation_tasks,
                &allocation_agents,
            )) as Box<dyn Strategy>
        }),
    ];
    let profiles = vec!["ideal".to_owned()];
    let builder = make_scenario_builder();
    let report = BenchmarkHarness::run_quick(&factories, &profiles, &builder);

    let greedy_key = ("greedy".to_owned(), "ideal".to_owned());
    let centralized_key = ("centralized".to_owned(), "ideal".to_owned());
    let greedy = report.results.get(&greedy_key).unwrap();
    let centralized = report.results.get(&centralized_key).unwrap();
    assert!(
        centralized.success_rate >= greedy.success_rate,
        "centralized ({}) should match or beat greedy ({}) on ideal network",
        centralized.success_rate,
        greedy.success_rate
    );
}

#[test]
fn determinism_jobs_1_vs_4() {
    let factories: Vec<StrategyFactory> =
        vec![Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
            Box::new(GreedyAllocator) as Box<dyn Strategy>
        })];
    let profiles = vec!["ideal".to_owned()];
    let builder = make_scenario_builder();

    let run = |jobs: usize| {
        BenchmarkHarness::run_with_seeds(
            &factories,
            &profiles,
            &builder,
            0..10,
            Some(BenchmarkOptions {
                jobs: Some(jobs),
                ..Default::default()
            }),
        )
        .report
    };

    let r1 = run(1);
    let r4 = run(4);

    let key = ("greedy".to_owned(), "ideal".to_owned());
    let m1 = r1.results.get(&key).unwrap();
    let m4 = r4.results.get(&key).unwrap();
    assert_eq!(
        m1.success_rate, m4.success_rate,
        "success_rate must be identical for jobs=1 and jobs=4"
    );
    assert_eq!(
        m1.avg_task_completion_rate, m4.avg_task_completion_rate,
        "avg_task_completion_rate must be identical for jobs=1 and jobs=4"
    );
}

#[test]
fn report_row_order_stable_across_jobs() {
    // Verifies that strategy_names and profile_names — and therefore the Display output —
    // are identical regardless of rayon thread count.
    let factories: Vec<StrategyFactory> = vec![
        Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
            Box::new(GreedyAllocator) as Box<dyn Strategy>
        }),
        Box::new(|scenario: &Scenario, _run_config: &RunConfig| {
            let allocation_tasks: Vec<AllocationTask<'_>> = scenario
                .tasks
                .iter()
                .map(|t| AllocationTask { task: t })
                .collect();
            let allocation_agents: Vec<AllocationAgent> = scenario
                .agents
                .iter()
                .map(|a| AllocationAgent {
                    id: a.id.clone(),
                    pose: a.pose,
                    battery: a.battery,
                    capabilities: a.capabilities.clone(),
                    role: a.role.clone(),
                    comms_range: a.comms_range,
                    speed: 0.0,
                    max_range: 0.0,
                    battery_drain_rate: 0.0,
                })
                .collect();
            Box::new(CentralizedPlanner::new(
                &allocation_tasks,
                &allocation_agents,
            )) as Box<dyn Strategy>
        }),
    ];
    let profiles = vec!["profile-a".to_owned(), "profile-b".to_owned()];
    let builder = make_scenario_builder();

    let run = |jobs: usize| {
        BenchmarkHarness::run_with_seeds(
            &factories,
            &profiles,
            &builder,
            0..4,
            Some(BenchmarkOptions {
                jobs: Some(jobs),
                ..Default::default()
            }),
        )
        .report
    };

    let r1 = run(1);
    let r2 = run(2);

    assert_eq!(
        r1.strategy_names, r2.strategy_names,
        "strategy_names order must be stable across jobs"
    );
    assert_eq!(
        r1.profile_names, r2.profile_names,
        "profile_names order must be stable across jobs"
    );
    // Display output must be bit-identical (same row order, same values).
    assert_eq!(
        format!("{r1}"),
        format!("{r2}"),
        "Display output must be identical for jobs=1 vs jobs=2"
    );
}

#[test]
fn report_completion_is_not_tasks_injected() {
    // Regression test: "Завершение" must come from task_completion_rate,
    // not avg_tasks_injected. With all_tasks_assigned=true and no dynamic
    // tasks, completion should be 1.000, not 0.000.
    let factories: Vec<StrategyFactory> =
        vec![Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
            Box::new(GreedyAllocator) as Box<dyn Strategy>
        })];
    let profiles = vec!["ideal".to_owned()];
    let builder = make_scenario_builder();
    let report = BenchmarkHarness::run_quick(&factories, &profiles, &builder);
    let report_text = format!("{}", report);

    // Parse the markdown table and check the "Completion" column specifically.
    // Column layout: | Mission | Scenario | Strategy | Profile | Seeds | Success | Completion | ...
    // After splitting by '|', index 7 is the completion column.
    let rows: Vec<&str> = report_text.lines().skip(2).collect();
    for row in &rows {
        if row.contains("greedy") {
            let cols: Vec<&str> = row.split('|').collect();
            let completion_col = cols.get(7).map(|s| s.trim());
            assert_eq!(
                    completion_col,
                    Some("1.000"),
                    "Completion column (index 7) should be 1.000 when all_tasks_assigned=true, got cols: {:?}",
                    cols
                );
        }
    }
}

#[test]
fn custom_seed_count_produces_custom_report_id() {
    let factories: Vec<StrategyFactory> =
        vec![Box::new(|_scenario: &Scenario, _run_config: &RunConfig| {
            Box::new(GreedyAllocator) as Box<dyn Strategy>
        })];
    let profiles = vec!["ideal".to_owned()];
    let builder = make_scenario_builder();
    let result = BenchmarkHarness::run_with_seed_count_with_options(
        &factories,
        &profiles,
        &builder,
        12,
        BenchmarkOptions {
            mission_name: "coverage",
            jobs: Some(2),
            ..BenchmarkOptions::default()
        },
    );

    assert_eq!(result.report.seed_range_start, 0);
    assert_eq!(result.report.seed_range_end, 12);
    assert_eq!(result.report.total_runs_per_cell, 12);
    assert!(
        result
            .report
            .benchmark_run_id
            .ends_with("_coverage_12_custom"),
        "custom seed count should be marked custom, got: {}",
        result.report.benchmark_run_id
    );
}

#[test]
fn merged_benchmark_run_id_single_report_unchanged() {
    let report = ComparisonReport {
        benchmark_run_id: "2026-01-01T000000Z_coverage_10_quick".to_owned(),
        seed_range_start: 0,
        seed_range_end: 10,
        total_runs_per_cell: 10,
        mission_names: vec!["coverage".to_owned()],
        scenario_names: vec!["coverage".to_owned()],
        strategy_names: vec!["greedy".to_owned()],
        profile_names: vec!["ideal".to_owned()],
        results: std::collections::HashMap::new(),
    };
    let id = merged_benchmark_run_id(&[report]);
    assert_eq!(id, "2026-01-01T000000Z_coverage_10_quick");
}

#[test]
fn merged_benchmark_run_id_multiple_reports_contains_all() {
    let r1 = ComparisonReport {
        benchmark_run_id: "2026-01-01T000000Z_coverage_10_quick".to_owned(),
        seed_range_start: 0,
        seed_range_end: 10,
        total_runs_per_cell: 10,
        mission_names: vec!["coverage".to_owned()],
        scenario_names: vec!["coverage".to_owned()],
        strategy_names: vec!["greedy".to_owned()],
        profile_names: vec!["ideal".to_owned()],
        results: std::collections::HashMap::new(),
    };
    let r2 = ComparisonReport {
        benchmark_run_id: "2026-01-01T000000Z_sar_10_quick".to_owned(),
        seed_range_start: 0,
        seed_range_end: 10,
        total_runs_per_cell: 10,
        mission_names: vec!["sar".to_owned()],
        scenario_names: vec!["sar".to_owned()],
        strategy_names: vec!["greedy".to_owned()],
        profile_names: vec!["standard".to_owned()],
        results: std::collections::HashMap::new(),
    };
    let id = merged_benchmark_run_id(&[r1, r2]);
    assert!(
        id.contains("_all_"),
        "merged id should contain '_all_', got: {}",
        id
    );
    assert!(
        !id.contains("coverage"),
        "merged id should not contain a mission name, got: {}",
        id
    );
    assert!(
        id.ends_with("_10_quick"),
        "mode should be preserved, got: {}",
        id
    );
}

#[test]
fn merged_benchmark_run_id_preserves_prefix() {
    let r1 = ComparisonReport {
        benchmark_run_id: "myrun_2026-01-01T000000Z_coverage_1_smoke".to_owned(),
        seed_range_start: 0,
        seed_range_end: 1,
        total_runs_per_cell: 1,
        mission_names: vec!["coverage".to_owned()],
        scenario_names: vec!["coverage".to_owned()],
        strategy_names: vec!["greedy".to_owned()],
        profile_names: vec!["ideal".to_owned()],
        results: std::collections::HashMap::new(),
    };
    let r2 = ComparisonReport {
        benchmark_run_id: "myrun_2026-01-01T000000Z_sar_1_smoke".to_owned(),
        seed_range_start: 0,
        seed_range_end: 1,
        total_runs_per_cell: 1,
        mission_names: vec!["sar".to_owned()],
        scenario_names: vec!["sar".to_owned()],
        strategy_names: vec!["greedy".to_owned()],
        profile_names: vec!["standard".to_owned()],
        results: std::collections::HashMap::new(),
    };
    let id = merged_benchmark_run_id(&[r1, r2]);
    assert!(
        id.starts_with("myrun_"),
        "prefix should be preserved, got: {}",
        id
    );
    assert!(
        id.contains("_all_"),
        "merged id should contain '_all_', got: {}",
        id
    );
}
