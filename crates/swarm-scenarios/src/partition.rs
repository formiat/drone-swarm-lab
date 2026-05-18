use swarm_sim::{PartitionEvent, RunConfig, Scenario};
use swarm_types::{Agent, AgentId, Task};

pub struct PartitionConfig {
    pub seed: u64,
    pub agents: Vec<Agent>,
    pub tasks: Vec<Task>,
    pub timeout_ticks: u64,
    pub max_ticks: u64,
    pub gossip_interval_ticks: u64,
    pub partition_start_tick: u64,
    pub partition_heal_tick: u64,
    pub group_a: Vec<AgentId>,
    pub group_b: Vec<AgentId>,
}

pub fn build_partition_scenario(config: &PartitionConfig) -> (Scenario, RunConfig) {
    let scenario = Scenario {
        name: "partition".to_owned(),
        seed: config.seed,
        agents: config.agents.clone(),
        tasks: config.tasks.clone(),
        ground_nodes: vec![],
        base_station: None,
    };

    let mut partition_events = Vec::new();
    for a in &config.group_a {
        for b in &config.group_b {
            partition_events.push(PartitionEvent {
                at_tick: config.partition_start_tick,
                until_tick: Some(config.partition_heal_tick),
                agents: (a.clone(), b.clone()),
            });
        }
    }

    let run_config = RunConfig {
        max_ticks: config.max_ticks,
        timeout_ticks: config.timeout_ticks,
        max_unassigned_ticks: 10,
        packet_loss_rate: 0.0,
        latency_ticks: 0,
        latency_per_hop: 0,
        failures: vec![],
        dynamic_tasks: vec![],
        partition_events,
        gossip_interval_ticks: config.gossip_interval_ticks,
        base_id: None,
        enable_movement: false,
        tick_duration_ms: 100,
        grid_state: None,
        enable_cbba: false,
    };

    (scenario, run_config)
}
