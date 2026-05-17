use swarm_sim::{FailureEvent, RunConfig, Scenario};
use swarm_types::{Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskStatus};

pub struct CoverageConfig {
    pub seed: u64,
    pub agent_count: usize,
    pub task_count: usize,
    pub failure_tick: u64,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub timeout_ticks: u64,
    pub max_unassigned_ticks: u64,
    pub max_ticks: u64,
}

pub fn build_coverage_scenario(config: &CoverageConfig) -> (Scenario, RunConfig) {
    assert!(
        (5..=20).contains(&config.agent_count),
        "agent_count must be in 5..=20"
    );
    assert!(
        config.task_count >= config.agent_count,
        "task_count must be at least agent_count"
    );

    let agents: Vec<_> = (0..config.agent_count)
        .map(|index| Agent {
            id: AgentId::from(format!("agent-{index}")),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose { x: 0.0, y: 0.0 },
            capabilities: Vec::new(),
            current_task: None,
            battery: 100.0,
            comms_range: f64::INFINITY,
            generation: 1,
        })
        .collect();

    let tasks = (0..config.task_count)
        .map(|index| {
            let assigned_to = if index < config.agent_count {
                Some(AgentId::from(format!("agent-{index}")))
            } else {
                None
            };
            Task {
                id: TaskId::from(format!("task-{index}")),
                status: if assigned_to.is_some() {
                    TaskStatus::Assigned
                } else {
                    TaskStatus::Unassigned
                },
                assigned_to,
                priority: 1,
                required_capabilities: vec![],
                required_role: None,
                preferred_role: None,
                expires_at: None,
                pose: None,
            }
        })
        .collect();

    let scenario = Scenario {
        name: "coverage_with_failure".to_owned(),
        seed: config.seed,
        agents,
        tasks,
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
        dynamic_tasks: vec![],
        partition_events: vec![],
        gossip_interval_ticks: 999,
        base_id: None,
    };

    (scenario, run_config)
}
