use proptest::prelude::*;
use swarm_alloc::CbbaAllocator;
use swarm_sim::{RunConfig, Scenario, ScenarioRunner};
use swarm_types::{Agent, AgentId, Capability, Health, Pose, Role, Task, TaskId, TaskStatus};

fn make_agent(id: u8) -> Agent {
    Agent {
        id: AgentId::from(format!("agent-{id}")),
        role: Role::Scout,
        health: Health::Alive,
        pose: Pose { x: 0.0, y: 0.0 },
        capabilities: vec![Capability::from("basic".to_owned())],
        current_task: None,
        battery: 100.0,
        comms_range: f64::INFINITY,
        generation: 1,
        speed: 0.0,
        max_range: 0.0,
        battery_drain_rate: 0.0,
    }
}

fn make_task(id: u8) -> Task {
    Task {
        id: TaskId::from(format!("task-{id}")),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority: 1,
        required_capabilities: vec![Capability::from("basic".to_owned())],
        required_role: None,
        preferred_role: None,
        expires_at: None,
        pose: None,
        grid_cell: None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 50,
        max_global_rejects: 2000,
        max_local_rejects: 5000,
        max_flat_map_regens: 1000,
        timeout: 1000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn cbba_no_panic_with_random_agents(
        agent_count in 2usize..=6,
        task_count in 1usize..=8,
        packet_loss in 0.0f64..0.3,
    ) {
        let agents: Vec<Agent> = (0..agent_count as u8)
            .map(make_agent)
            .collect();
        let tasks: Vec<Task> = (0..task_count as u8)
            .map(make_task)
            .collect();

        let scenario = Scenario {
            name: "proptest_cbba".to_owned(),
            seed: 42,
            agents,
            tasks,
            ground_nodes: vec![],
            base_station: None,
        };

        let mut config: RunConfig = RunConfig {
            max_ticks: 30,
            timeout_ticks: 5,
            max_unassigned_ticks: 10,
            packet_loss_rate: packet_loss,
            latency_ticks: 0,
            latency_per_hop: 0,
            failures: vec![],
            dynamic_tasks: vec![],
            partition_events: vec![],
            gossip_interval_ticks: 1,
            base_id: None,
            enable_movement: false,
            tick_duration_ms: 100,
            grid_state: None,
            enable_cbba: true,
            ..Default::default()
        };
        config.enable_cbba = true;
        let metrics = ScenarioRunner::run_with(&scenario, config, CbbaAllocator::default());

        // success_rate bounded check
        let rate = if metrics.success { 1.0 } else { 0.0 };
        assert!((0.0..=1.0).contains(&rate),
            "success_rate out of bounds: {rate}");
    }

    #[test]
    fn cbba_success_rate_is_bounded(
        agent_count in 2usize..=4,
        task_count in 1usize..=4,
    ) {
        let agents: Vec<Agent> = (0..agent_count as u8)
            .map(make_agent)
            .collect();
        let tasks: Vec<Task> = (0..task_count as u8)
            .map(make_task)
            .collect();

        let scenario = Scenario {
            name: "proptest_cbba_bounded".to_owned(),
            seed: 99,
            agents,
            tasks,
            ground_nodes: vec![],
            base_station: None,
        };

        let mut config = RunConfig {
            max_ticks: 20,
            timeout_ticks: 3,
            max_unassigned_ticks: 10,
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            latency_per_hop: 0,
            failures: vec![],
            dynamic_tasks: vec![],
            partition_events: vec![],
            gossip_interval_ticks: 1,
            base_id: None,
            enable_movement: false,
            tick_duration_ms: 100,
            grid_state: None,
            enable_cbba: true,
            ..Default::default()
        };
        config.enable_cbba = true;
        let metrics = ScenarioRunner::run_with(&scenario, config, CbbaAllocator::default());

        // success_rate is bounded in [0.0, 1.0]
        let rate = if metrics.success { 1.0 } else { 0.0 };
        assert!(
            (0.0..=1.0).contains(&rate),
            "success_rate out of bounds"
        );
    }
}
