use proptest::prelude::*;
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
                },
                capabilities,
                current_task: None,
                battery,
                comms_range,
                generation: 1,
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
        pose: Some(Pose {
            x: (idx as f64) * 8.0,
            y: (idx as f64) * 4.0,
        }),
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
}
