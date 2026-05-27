use swarm_sim::{DynamicTaskEvent, FailureEvent, RunConfig, Scenario};
use swarm_types::{
    Agent, AgentId, Capability, Health, Pose, Role, Task, TaskId, TaskKind, TaskStatus,
};

#[derive(Clone, Copy)]
pub struct DynamicAuctionConfig {
    pub seed: u64,
    pub agent_count: usize,
    pub initial_task_count: usize,
    /// Tasks injected dynamically during the mission.
    pub dynamic_task_count: usize,
    pub dynamic_task_start_tick: u64,
    pub dynamic_task_interval_ticks: u64,
    /// Ticks after injection before a dynamic task expires.
    pub task_expiry_ticks: u64,
    pub failure_tick: u64,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub timeout_ticks: u64,
    pub max_unassigned_ticks: u64,
    pub max_ticks: u64,
}

/// value: `(scenario, run_config)`
pub fn build_dynamic_auction_scenario(config: &DynamicAuctionConfig) -> (Scenario, RunConfig) {
    let roles = [Role::Scout, Role::Mapper, Role::Inspector];
    let cap_names = ["optical", "thermal", "lidar"];

    let agents: Vec<Agent> = (0..config.agent_count)
        .map(|i| {
            let role = roles[i % roles.len()].clone();
            let cap = Capability::from(cap_names[i % cap_names.len()].to_owned());
            // Deterministic pose derived from seed and index.
            let x = ((config
                .seed
                .wrapping_add(i as u64)
                .wrapping_mul(7)
                .wrapping_add(3))
                % 50) as f64;
            let y = ((config
                .seed
                .wrapping_add(i as u64)
                .wrapping_mul(13)
                .wrapping_add(7))
                % 50) as f64;
            Agent {
                id: AgentId::from(format!("agent-{i}")),
                role,
                health: Health::Alive,
                pose: Pose {
                    x,
                    y,
                    ..Default::default()
                },
                capabilities: vec![cap],
                current_task: None,
                battery: 100.0,
                comms_range: f64::INFINITY,
                generation: 1,
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
                battery_model: None,
            }
        })
        .collect();

    let initial_tasks: Vec<Task> = (0..config.initial_task_count)
        .map(|i| {
            let cap = Capability::from(cap_names[i % cap_names.len()].to_owned());
            let tx = ((config
                .seed
                .wrapping_add(i as u64 + 100)
                .wrapping_mul(11)
                .wrapping_add(5))
                % 50) as f64;
            let ty = ((config
                .seed
                .wrapping_add(i as u64 + 100)
                .wrapping_mul(17)
                .wrapping_add(9))
                % 50) as f64;
            Task {
                id: TaskId::from(format!("task-{i}")),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: ((i % 5) + 1) as u8,
                required_capabilities: vec![cap],
                required_role: None,
                preferred_role: None,
                expires_at: None,
                pose: Some(Pose {
                    x: tx,
                    y: ty,
                    ..Default::default()
                }),
                grid_cell: None,
                edge_id: None,
                kind: Some(TaskKind::CoverageCell),
            }
        })
        .collect();

    let dynamic_tasks: Vec<DynamicTaskEvent> = (0..config.dynamic_task_count)
        .map(|i| {
            let injection_tick =
                config.dynamic_task_start_tick + (i as u64 * config.dynamic_task_interval_ticks);
            let cap = Capability::from(cap_names[i % cap_names.len()].to_owned());
            let tx = ((config
                .seed
                .wrapping_add(i as u64 + 200)
                .wrapping_mul(19)
                .wrapping_add(3))
                % 50) as f64;
            let ty = ((config
                .seed
                .wrapping_add(i as u64 + 200)
                .wrapping_mul(23)
                .wrapping_add(11))
                % 50) as f64;
            DynamicTaskEvent {
                at_tick: injection_tick,
                task: Task {
                    id: TaskId::from(format!("dynamic-{i}")),
                    status: TaskStatus::Unassigned,
                    assigned_to: None,
                    priority: ((i % 5) + 1) as u8,
                    required_capabilities: vec![cap],
                    required_role: None,
                    preferred_role: None,
                    expires_at: Some(injection_tick + config.task_expiry_ticks),
                    pose: Some(Pose {
                        x: tx,
                        y: ty,
                        ..Default::default()
                    }),
                    grid_cell: None,
                    edge_id: None,
                    kind: Some(TaskKind::CoverageCell),
                },
            }
        })
        .collect();

    let scenario = Scenario {
        name: "dynamic_auction".to_owned(),
        seed: config.seed,
        agents,
        tasks: initial_tasks,
        ground_nodes: vec![],
        base_station: None,
    };

    let run_config = RunConfig {
        max_ticks: config.max_ticks,
        timeout_ticks: config.timeout_ticks,
        max_unassigned_ticks: config.max_unassigned_ticks,
        packet_loss_rate: config.packet_loss_rate,
        latency_ticks: config.latency_ticks,
        latency_per_hop: 0,
        failures: vec![FailureEvent {
            agent_id: AgentId::from("agent-0".to_owned()),
            at_tick: config.failure_tick,
        }],
        dynamic_tasks,
        partition_events: vec![],
        gossip_interval_ticks: 999,
        base_id: None,
        enable_movement: false,
        tick_duration_ms: 100,
        grid_state: None,
        enable_cbba: false,
        ..Default::default()
    };

    (scenario, run_config)
}
