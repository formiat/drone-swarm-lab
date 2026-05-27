use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use swarm_sim::{FailureEvent, RunConfig, Scenario};
use swarm_types::{
    Agent, AgentId, GroundNode, Health, Pose, Role, Task, TaskId, TaskKind, TaskStatus,
};

#[derive(Clone, Debug, PartialEq)]
pub enum EmergencyMeshProfile {
    Ideal,
    LowLoss,
    MediumLoss,
    SingleFailure,
    PacketLoss10,
}

impl EmergencyMeshProfile {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ideal" => Some(Self::Ideal),
            "low-loss" | "lowloss" => Some(Self::LowLoss),
            "medium-loss" | "mediumloss" => Some(Self::MediumLoss),
            "single-failure" | "singlefailure" => Some(Self::SingleFailure),
            "packet-loss-10" | "packetloss10" => Some(Self::PacketLoss10),
            _ => None,
        }
    }

    pub fn config(&self, seed: u64) -> EmergencyMeshConfig {
        match self {
            Self::Ideal => EmergencyMeshConfig {
                seed,
                scout_count: 3,
                relay_count: 3,
                ground_node_count: 2,
                base_pose: Pose {
                    x: 0.0,
                    y: 0.0,
                    ..Default::default()
                },
                area_size: 100.0,
                comms_range: 30.0,
                failure_tick: 999,
                max_ticks: 200,
                timeout_ticks: 3,
                gossip_interval_ticks: 3,
                packet_loss_rate: 0.0,
            },
            Self::LowLoss => EmergencyMeshConfig {
                seed,
                scout_count: 3,
                relay_count: 3,
                ground_node_count: 2,
                base_pose: Pose {
                    x: 0.0,
                    y: 0.0,
                    ..Default::default()
                },
                area_size: 100.0,
                comms_range: 30.0,
                failure_tick: 999,
                max_ticks: 200,
                timeout_ticks: 3,
                gossip_interval_ticks: 3,
                packet_loss_rate: 0.05,
            },
            Self::MediumLoss => EmergencyMeshConfig {
                seed,
                scout_count: 3,
                relay_count: 3,
                ground_node_count: 2,
                base_pose: Pose {
                    x: 0.0,
                    y: 0.0,
                    ..Default::default()
                },
                area_size: 100.0,
                comms_range: 25.0,
                failure_tick: 999,
                max_ticks: 250,
                timeout_ticks: 5,
                gossip_interval_ticks: 3,
                packet_loss_rate: 0.10,
            },
            Self::SingleFailure => EmergencyMeshConfig {
                seed,
                scout_count: 3,
                relay_count: 3,
                ground_node_count: 2,
                base_pose: Pose {
                    x: 0.0,
                    y: 0.0,
                    ..Default::default()
                },
                area_size: 100.0,
                comms_range: 30.0,
                failure_tick: 10,
                max_ticks: 200,
                timeout_ticks: 3,
                gossip_interval_ticks: 3,
                packet_loss_rate: 0.0,
            },
            Self::PacketLoss10 => EmergencyMeshConfig {
                seed,
                scout_count: 3,
                relay_count: 3,
                ground_node_count: 2,
                base_pose: Pose {
                    x: 0.0,
                    y: 0.0,
                    ..Default::default()
                },
                area_size: 100.0,
                comms_range: 30.0,
                failure_tick: 999,
                max_ticks: 200,
                timeout_ticks: 3,
                gossip_interval_ticks: 3,
                packet_loss_rate: 0.10,
            },
        }
    }
}

pub struct EmergencyMeshStandardProfiles;

impl EmergencyMeshStandardProfiles {
    pub fn profile_names() -> Vec<&'static str> {
        vec![
            "ideal",
            "low-loss",
            "medium-loss",
            "single-failure",
            "packet-loss-10",
        ]
    }
}

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
    pub packet_loss_rate: f64,
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
                pose: Pose {
                    x,
                    y,
                    ..Default::default()
                },
                capabilities: vec![],
                current_task: None,
                battery: 100.0,
                comms_range: config.comms_range,
                generation: 1,
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
                battery_model: None,
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
                pose: Pose {
                    x,
                    y,
                    ..Default::default()
                },
                capabilities: vec![],
                current_task: None,
                battery: 100.0,
                comms_range: config.comms_range,
                generation: 1,
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
                battery_model: None,
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
                pose: Pose {
                    x,
                    y,
                    ..Default::default()
                },
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
            grid_cell: None,
            edge_id: None,
            kind: Some(TaskKind::CoverageCell),
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
            grid_cell: None,
            edge_id: None,
            kind: Some(TaskKind::RelayPlacement),
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
        packet_loss_rate: config.packet_loss_rate,
        latency_ticks: 0,
        latency_per_hop: 0,
        failures: failure,
        dynamic_tasks: vec![],
        partition_events: vec![],
        gossip_interval_ticks: config.gossip_interval_ticks,
        base_id: Some(base_id),
        enable_movement: false,
        tick_duration_ms: 100,
        grid_state: None,
        enable_cbba: false,
        ..Default::default()
    };

    (scenario, run_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emergency_mesh_ideal_profile_params() {
        let config = EmergencyMeshProfile::Ideal.config(42);
        assert_eq!(config.scout_count, 3);
        assert_eq!(config.relay_count, 3);
        assert_eq!(config.packet_loss_rate, 0.0);
        assert_eq!(config.failure_tick, 999);
    }

    #[test]
    fn emergency_mesh_low_loss_profile_params() {
        let config = EmergencyMeshProfile::LowLoss.config(42);
        assert_eq!(config.packet_loss_rate, 0.05);
    }

    #[test]
    fn emergency_mesh_medium_loss_profile_params() {
        let config = EmergencyMeshProfile::MediumLoss.config(42);
        assert_eq!(config.packet_loss_rate, 0.10);
        assert_eq!(config.comms_range, 25.0);
    }

    #[test]
    fn emergency_mesh_single_failure_profile_params() {
        let config = EmergencyMeshProfile::SingleFailure.config(42);
        assert_eq!(config.failure_tick, 10);
        assert_eq!(config.packet_loss_rate, 0.0);
    }

    #[test]
    fn emergency_mesh_packet_loss_10_profile_params() {
        let config = EmergencyMeshProfile::PacketLoss10.config(42);
        assert_eq!(config.packet_loss_rate, 0.10);
    }

    #[test]
    fn emergency_mesh_profile_from_str_roundtrip() {
        assert_eq!(
            EmergencyMeshProfile::from_str("ideal"),
            Some(EmergencyMeshProfile::Ideal)
        );
        assert_eq!(
            EmergencyMeshProfile::from_str("low-loss"),
            Some(EmergencyMeshProfile::LowLoss)
        );
        assert_eq!(
            EmergencyMeshProfile::from_str("lowloss"),
            Some(EmergencyMeshProfile::LowLoss)
        );
        assert_eq!(
            EmergencyMeshProfile::from_str("medium-loss"),
            Some(EmergencyMeshProfile::MediumLoss)
        );
        assert_eq!(
            EmergencyMeshProfile::from_str("single-failure"),
            Some(EmergencyMeshProfile::SingleFailure)
        );
        assert_eq!(
            EmergencyMeshProfile::from_str("packet-loss-10"),
            Some(EmergencyMeshProfile::PacketLoss10)
        );
        assert_eq!(EmergencyMeshProfile::from_str("unknown"), None);
    }

    #[test]
    fn emergency_mesh_standard_profiles_names() {
        let names = EmergencyMeshStandardProfiles::profile_names();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"ideal"));
        assert!(names.contains(&"low-loss"));
        assert!(names.contains(&"medium-loss"));
        assert!(names.contains(&"single-failure"));
        assert!(names.contains(&"packet-loss-10"));
    }
}
