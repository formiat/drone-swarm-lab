use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use swarm_sim::{RunConfig, Scenario};
use swarm_types::{
    Aabb, Agent, AgentId, Capability, Health, Pose, Role, Task, TaskId, TaskKind, TaskStatus,
};

/// A single hazard zone to be mapped.
#[derive(Clone, Debug, PartialEq)]
pub struct HazardZone {
    pub id: String,
    pub bounds: Aabb,
    pub threat_level: f64,
    pub priority: u8,
}

/// Configuration for a wildfire / flood mapping scenario.
pub struct WildfireConfig {
    pub seed: u64,
    pub agent_count: u32,
    pub zones: Vec<HazardZone>,
    pub update_interval_ticks: u64,
    pub max_ticks: u64,
    pub enable_dynamic_threat: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WildfireProfile {
    SmallStatic,
    MediumDynamic,
}

impl WildfireProfile {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "small-static" | "smallstatic" => Some(Self::SmallStatic),
            "medium-dynamic" | "mediumdynamic" => Some(Self::MediumDynamic),
            _ => None,
        }
    }

    pub fn config(&self, seed: u64) -> WildfireConfig {
        match self {
            Self::SmallStatic => WildfireConfig {
                seed,
                agent_count: 2,
                zones: vec![
                    HazardZone {
                        id: "zone-a".to_owned(),
                        bounds: Aabb {
                            min_x: 0.0,
                            min_y: 0.0,
                            max_x: 20.0,
                            max_y: 20.0,
                        },
                        threat_level: 0.7,
                        priority: 5,
                    },
                    HazardZone {
                        id: "zone-b".to_owned(),
                        bounds: Aabb {
                            min_x: 20.0,
                            min_y: 20.0,
                            max_x: 40.0,
                            max_y: 40.0,
                        },
                        threat_level: 0.3,
                        priority: 3,
                    },
                ],
                update_interval_ticks: 999,
                max_ticks: 200,
                enable_dynamic_threat: false,
            },
            Self::MediumDynamic => WildfireConfig {
                seed,
                agent_count: 4,
                zones: vec![
                    HazardZone {
                        id: "zone-a".to_owned(),
                        bounds: Aabb {
                            min_x: 0.0,
                            min_y: 0.0,
                            max_x: 20.0,
                            max_y: 20.0,
                        },
                        threat_level: 0.5,
                        priority: 4,
                    },
                    HazardZone {
                        id: "zone-b".to_owned(),
                        bounds: Aabb {
                            min_x: 20.0,
                            min_y: 0.0,
                            max_x: 40.0,
                            max_y: 20.0,
                        },
                        threat_level: 0.2,
                        priority: 2,
                    },
                    HazardZone {
                        id: "zone-c".to_owned(),
                        bounds: Aabb {
                            min_x: 0.0,
                            min_y: 20.0,
                            max_x: 20.0,
                            max_y: 40.0,
                        },
                        threat_level: 0.4,
                        priority: 3,
                    },
                    HazardZone {
                        id: "zone-d".to_owned(),
                        bounds: Aabb {
                            min_x: 20.0,
                            min_y: 20.0,
                            max_x: 40.0,
                            max_y: 40.0,
                        },
                        threat_level: 0.1,
                        priority: 1,
                    },
                ],
                update_interval_ticks: 50,
                max_ticks: 400,
                enable_dynamic_threat: true,
            },
        }
    }
}

pub struct WildfireStandardProfiles;

impl WildfireStandardProfiles {
    pub fn profile_names() -> Vec<&'static str> {
        vec!["small-static", "medium-dynamic"]
    }
}

pub fn build_wildfire_scenario(config: &WildfireConfig) -> (Scenario, RunConfig) {
    let mut rng = StdRng::seed_from_u64(config.seed);

    let agents: Vec<Agent> = (0..config.agent_count)
        .map(|i| {
            let x = rng.gen::<f64>() * 40.0;
            let y = rng.gen::<f64>() * 40.0;
            Agent {
                id: AgentId::from(format!("agent-{i}")),
                role: Role::Scout,
                health: Health::Alive,
                pose: Pose {
                    x,
                    y,
                    ..Default::default()
                },
                capabilities: vec![Capability::from("thermal".to_owned())],
                current_task: None,
                battery: 100.0,
                comms_range: f64::INFINITY,
                generation: 1,
                speed: 1.0,
                max_range: 0.0,
                battery_drain_rate: 0.1,
                battery_model: None,
            }
        })
        .collect();

    let tasks: Vec<Task> = config
        .zones
        .iter()
        .map(|zone| Task {
            id: TaskId::from(zone.id.clone()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: zone.priority,
            required_capabilities: vec![Capability::from("thermal".to_owned())],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(zone.bounds.center()),
            grid_cell: None,
            edge_id: None,
            kind: Some(TaskKind::MappingZone),
        })
        .collect();

    let scenario = Scenario {
        name: "wildfire".to_owned(),
        seed: config.seed,
        agents,
        tasks,
        ground_nodes: vec![],
        base_station: None,
    };

    let wildfire_state = swarm_sim::WildfireState {
        zones: config
            .zones
            .iter()
            .map(|z| swarm_sim::WildfireZone {
                id: z.id.clone(),
                threat_level: z.threat_level,
                priority: z.priority,
            })
            .collect(),
        mapped_zone_ids: std::collections::HashSet::new(),
        update_interval_ticks: config.update_interval_ticks,
        enable_dynamic_threat: config.enable_dynamic_threat,
    };

    let run_config = RunConfig {
        max_ticks: config.max_ticks,
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
        enable_movement: true,
        grid_state: None,
        tick_duration_ms: 100,
        enable_cbba: false,
        wildfire_state: Some(wildfire_state),
        ..Default::default()
    };

    (scenario, run_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_static_builds_scenario() {
        let config = WildfireProfile::SmallStatic.config(42);
        let (scenario, run_config) = build_wildfire_scenario(&config);
        assert_eq!(scenario.tasks.len(), 2);
        assert_eq!(scenario.agents.len(), 2);
        assert_eq!(run_config.max_ticks, 200);
    }

    #[test]
    fn medium_dynamic_builds_scenario() {
        let config = WildfireProfile::MediumDynamic.config(42);
        let (scenario, run_config) = build_wildfire_scenario(&config);
        assert_eq!(scenario.tasks.len(), 4);
        assert_eq!(scenario.agents.len(), 4);
        assert_eq!(run_config.max_ticks, 400);
    }

    #[test]
    fn tasks_have_mapping_zone_kind() {
        let config = WildfireProfile::SmallStatic.config(0);
        let (scenario, _) = build_wildfire_scenario(&config);
        for task in &scenario.tasks {
            assert_eq!(task.kind, Some(TaskKind::MappingZone));
            assert!(task.pose.is_some());
        }
    }

    #[test]
    fn task_poses_inside_zone_bounds() {
        let config = WildfireProfile::SmallStatic.config(0);
        let (scenario, _) = build_wildfire_scenario(&config);
        for (task, zone) in scenario.tasks.iter().zip(&config.zones) {
            let pose = task.pose.unwrap();
            assert!(
                zone.bounds.contains(&pose),
                "Task pose {:?} not inside zone {:?}",
                pose,
                zone.bounds
            );
        }
    }

    #[test]
    fn aabb_contains_center() {
        let aabb = Aabb {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 10.0,
        };
        assert!(aabb.contains(&aabb.center()));
    }

    #[test]
    fn aabb_area() {
        let aabb = Aabb {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 5.0,
            max_y: 3.0,
        };
        assert!((aabb.area() - 15.0).abs() < 1e-9);
    }
}
