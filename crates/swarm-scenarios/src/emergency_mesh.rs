use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use swarm_sim::{FailureEvent, RunConfig, Scenario};
use swarm_types::{Agent, AgentId, GroundNode, Health, Pose, Role, Task, TaskId, TaskStatus};

pub struct EmergencyMeshConfig {
    pub seed: u64,
    pub scout_count: usize,
    pub relay_count: usize,
    pub ground_node_count: usize,
    pub base_pose: Pose,
    pub area_size: f64,
    pub comms_range: f64,
    pub failure_tick: u64,
    pub max_ticks: u64,
    pub timeout_ticks: u64,
    pub gossip_interval_ticks: u64,
}

pub fn build_emergency_mesh_scenario(config: &EmergencyMeshConfig) -> (Scenario, RunConfig) {
    let mut rng = StdRng::seed_from_u64(config.seed);

    let base_id = AgentId::from("base".to_owned());

    // Generate scouts at random positions in the area
    let scouts: Vec<Agent> = (0..config.scout_count)
        .map(|i| {
            let x = rng.gen::<f64>() * config.area_size;
            let y = rng.gen::<f64>() * config.area_size;
            Agent {
                id: AgentId::from(format!("scout-{i}")),
                role: Role::Scout,
                health: Health::Alive,
                pose: Pose { x, y },
                capabilities: vec![],
                current_task: None,
                battery: 100.0,
                comms_range: config.comms_range,
                generation: 1,
            }
        })
        .collect();

    // Generate relays positioned between base and scouts to form a mesh
    let relays: Vec<Agent> = (0..config.relay_count)
        .map(|i| {
            // Position relays along a line from base toward the far corner
            let fraction = (i + 1) as f64 / (config.relay_count + 1) as f64;
            let x = config.base_pose.x + fraction * (config.area_size * 0.8);
            let y = config.base_pose.y + fraction * (config.area_size * 0.5);
            Agent {
                id: AgentId::from(format!("relay-{i}")),
                role: Role::Relay,
                health: Health::Alive,
                pose: Pose { x, y },
                capabilities: vec![],
                current_task: None,
                battery: 100.0,
                comms_range: config.comms_range,
                generation: 1,
            }
        })
        .collect();

    let mut agents = vec![];
    agents.extend(scouts.clone());
    agents.extend(relays.clone());

    // Ground nodes at fixed positions
    let ground_nodes: Vec<GroundNode> = (0..config.ground_node_count)
        .map(|i| {
            let x = rng.gen::<f64>() * config.area_size;
            let y = rng.gen::<f64>() * config.area_size;
            GroundNode {
                id: format!("gn-{i}"),
                pose: Pose { x, y },
                comms_range: config.comms_range,
            }
        })
        .collect();

    // Scout tasks: coverage of sub-zones
    let scout_tasks: Vec<Task> = scouts
        .iter()
        .enumerate()
        .map(|(i, scout)| Task {
            id: TaskId::from(format!("scout-task-{i}")),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: Some(Role::Scout),
            preferred_role: Some(Role::Scout),
            expires_at: None,
            pose: Some(scout.pose),
        })
        .collect();

    // Relay tasks: position at key mesh points
    let relay_tasks: Vec<Task> = relays
        .iter()
        .enumerate()
        .map(|(i, relay)| Task {
            id: TaskId::from(format!("relay-task-{i}")),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 2, // Higher priority than scout tasks
            required_capabilities: vec![],
            required_role: Some(Role::Relay),
            preferred_role: Some(Role::Relay),
            expires_at: None,
            pose: Some(relay.pose),
        })
        .collect();

    let mut tasks = vec![];
    tasks.extend(scout_tasks);
    tasks.extend(relay_tasks);

    let scenario = Scenario {
        name: "emergency_mesh".to_owned(),
        seed: config.seed,
        agents,
        tasks,
        ground_nodes,
        base_station: Some(config.base_pose),
    };

    let failure = if config.relay_count > 0 {
        vec![FailureEvent {
            agent_id: AgentId::from("relay-0".to_owned()),
            at_tick: config.failure_tick,
        }]
    } else {
        vec![]
    };

    let run_config = RunConfig {
        max_ticks: config.max_ticks,
        timeout_ticks: config.timeout_ticks,
        max_unassigned_ticks: 10,
        packet_loss_rate: 0.0,
        latency_ticks: 0,
        failures: failure,
        dynamic_tasks: vec![],
        partition_events: vec![],
        gossip_interval_ticks: config.gossip_interval_ticks,
        base_id: Some(base_id),
    };

    (scenario, run_config)
}
