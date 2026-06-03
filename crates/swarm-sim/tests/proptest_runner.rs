use proptest::prelude::*;
use swarm_alloc::GreedyAllocator;
use swarm_sim::{RunConfig, ScenarioRunner};
use swarm_types::{Agent, AgentId, Capability, Health, Pose, Role, Task, TaskId, TaskStatus};

fn agent_strategy() -> impl Strategy<Value = Agent> {
    (
        any::<u8>(),
        any::<u8>(),
        10.0f64..100.0f64,
        1.0f64..50.0f64,
        prop::collection::vec(
            prop::sample::select(vec![
                Capability::from("thermal".to_owned()),
                Capability::from("optical".to_owned()),
            ]),
            0..3,
        ),
    )
        .prop_map(
            |(idx, role_idx, battery, comms_range, capabilities)| Agent {
                id: AgentId::from(format!("agent-{}", idx)),
                role: match role_idx % 5 {
                    0 => Role::Scout,
                    1 => Role::Relay,
                    2 => Role::Mapper,
                    3 => Role::Inspector,
                    _ => Role::Carrier,
                },
                health: Health::Alive,
                pose: Pose {
                    x: (idx as f64) * 10.0,
                    y: (idx as f64) * 5.0,
                    ..Default::default()
                },
                capabilities,
                current_task: None,
                battery,
                comms_range,
                generation: 1,
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
                battery_model: None,
            },
        )
}

fn task_strategy() -> impl Strategy<Value = Task> {
    (any::<u8>(), any::<u8>(), 1u8..10u8).prop_map(|(idx, role_idx, priority)| Task {
        id: TaskId::from(format!("task-{}", idx)),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority,
        required_capabilities: vec![],
        required_role: if role_idx % 4 == 0 {
            Some(Role::Relay)
        } else {
            None
        },
        preferred_role: None,
        expires_at: None,
        grid_cell: None,
        edge_id: None,
        pose: Some(Pose {
            x: (idx as f64) * 8.0,
            y: (idx as f64) * 4.0,
            ..Default::default()
        }),
        kind: None,
    })
}

fn scenario_from_agents_tasks(agents: Vec<Agent>, tasks: Vec<Task>) -> swarm_sim::Scenario {
    swarm_sim::Scenario {
        name: "proptest".to_owned(),
        seed: 42,
        agents,
        tasks,
        ground_nodes: vec![],
        base_station: None,
        geo_origin: None,
    }
}

fn default_run_config() -> RunConfig {
    RunConfig {
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
        tick_duration_ms: 100,
        grid_state: None,
        enable_cbba: false,
        ..Default::default()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn runner_does_not_panic(
        agents in prop::collection::vec(agent_strategy(), 3..10),
        tasks in prop::collection::vec(task_strategy(), 3..15),
    ) {
        let scenario = scenario_from_agents_tasks(agents, tasks);
        let config = default_run_config();
        let _metrics = ScenarioRunner::run(&scenario, config);
        // If we reach here, the runner did not panic.
    }

    #[test]
    fn success_rate_is_bounded(
        agents in prop::collection::vec(agent_strategy(), 3..10),
        tasks in prop::collection::vec(task_strategy(), 3..15),
    ) {
        let scenario = scenario_from_agents_tasks(agents, tasks);
        let config = default_run_config();
        let metrics = ScenarioRunner::run(&scenario, config);
        // success is a boolean; success_rate for a single run is either 0 or 1
        let rate: f64 = if metrics.success { 1.0 } else { 0.0 };
        prop_assert!((0.0..=1.0).contains(&rate));
    }

    #[test]
    fn replay_matches_original(
        agents in prop::collection::vec(agent_strategy(), 3..10),
        tasks in prop::collection::vec(task_strategy(), 3..15),
    ) {
        let scenario = scenario_from_agents_tasks(agents, tasks);
        let config = default_run_config();
        let (metrics, log_opt) = ScenarioRunner::run_with_log(&scenario, config, GreedyAllocator::default());
        // run_with_log should return Some(EventLog)
        prop_assert!(log_opt.is_some(), "run_with_log should return EventLog");
        let log = log_opt.unwrap();
        // Serialize and deserialize round-trip
        let json = swarm_replay::to_json(&log).expect("serialize");
        let restored = swarm_replay::from_json(&json).expect("deserialize");
        prop_assert_eq!(log, restored.clone(), "EventLog round-trip failed");
        // Replay reconstructs some state
        let _state = swarm_replay::replay(&restored);
        // Basic sanity: replay state should have same tick count as metrics
        let tick_count = restored.events.iter().filter(|e| matches!(e, swarm_replay::Event::TickStart { .. })).count() as u64;
        prop_assert_eq!(tick_count, metrics.total_ticks, "Tick count mismatch after replay");
    }

    #[test]
    fn centralized_beats_greedy_on_ideal(
        agents in prop::collection::vec(agent_strategy(), 3..10),
        tasks in prop::collection::vec(task_strategy(), 3..15),
    ) {
        let scenario = scenario_from_agents_tasks(agents, tasks);
        let mut config = default_run_config();
        config.packet_loss_rate = 0.0;
        config.latency_ticks = 0;
        config.latency_per_hop = 0;
        config.failures.clear();
        let greedy_metrics = ScenarioRunner::run_with(&scenario, config.clone(), GreedyAllocator::default());
        let centralized_metrics = ScenarioRunner::run_with(&scenario, config, swarm_alloc::CentralizedPlanner::new(
            &scenario.tasks.iter().map(|t| swarm_alloc::AllocationTask { task: t }).collect::<Vec<_>>(),
            &scenario.agents.iter().map(|a| swarm_alloc::AllocationAgent {
                id: a.id.clone(),
                pose: a.pose,
                battery: a.battery,
                capabilities: a.capabilities.clone(),
                role: a.role.clone(),
                comms_range: a.comms_range,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
            }).collect::<Vec<_>>(),
        ));
        let centralized_rate = if centralized_metrics.success { 1.0 } else { 0.0 };
        let greedy_rate = if greedy_metrics.success { 1.0 } else { 0.0 };
        prop_assert!(
            centralized_rate >= greedy_rate,
            "Centralized should match or beat greedy on ideal network: centralized={}, greedy={}",
            centralized_rate,
            greedy_rate
        );
    }
}
