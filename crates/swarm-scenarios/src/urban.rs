use swarm_sim::{GeoOrigin, RunConfig, Scenario, UrbanState};
use swarm_types::{
    Aabb, Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskKind, TaskStatus, UrbanBus,
    UrbanBusId, UrbanDetectorConfig, UrbanEdge, UrbanEdgeId, UrbanMap, UrbanNode, UrbanNodeId,
    UrbanObstacleId, UrbanRouteLoop, UrbanSearchState, UrbanStaticObstacle,
};

pub struct UrbanConfig {
    pub seed: u64,
    pub max_ticks: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UrbanProfile {
    PatrolSmallBlock,
    MultiAgentSmallBlock,
    SearchStaticBus,
    SearchOutOfRange,
    SearchFalsePositive,
}

impl UrbanProfile {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "patrol-small-block" | "patrolsmallblock" => Some(Self::PatrolSmallBlock),
            "multi-agent-small-block" | "multiagentsmallblock" => Some(Self::MultiAgentSmallBlock),
            "search-static-bus" | "searchstaticbus" => Some(Self::SearchStaticBus),
            "search-out-of-range" | "searchoutofrange" => Some(Self::SearchOutOfRange),
            "search-false-positive" | "searchfalsepositive" => Some(Self::SearchFalsePositive),
            _ => None,
        }
    }

    pub fn config(&self, seed: u64) -> UrbanConfig {
        match self {
            Self::PatrolSmallBlock | Self::MultiAgentSmallBlock => UrbanConfig {
                seed,
                max_ticks: 120,
            },
            Self::SearchStaticBus => UrbanConfig {
                seed,
                max_ticks: 60,
            },
            Self::SearchOutOfRange => UrbanConfig {
                seed,
                max_ticks: 12,
            },
            Self::SearchFalsePositive => UrbanConfig { seed, max_ticks: 6 },
        }
    }
}

pub struct UrbanStandardProfiles;

impl UrbanStandardProfiles {
    pub fn profile_names() -> Vec<&'static str> {
        Self::patrol_profile_names()
    }

    pub fn patrol_profile_names() -> Vec<&'static str> {
        vec!["patrol-small-block"]
    }

    pub fn multi_agent_profile_names() -> Vec<&'static str> {
        vec!["multi-agent-small-block"]
    }

    pub fn search_profile_names() -> Vec<&'static str> {
        vec![
            "search-static-bus",
            "search-out-of-range",
            "search-false-positive",
        ]
    }
}

pub fn build_urban_patrol_scenario(config: &UrbanConfig) -> (Scenario, RunConfig) {
    let n0 = UrbanNodeId::from("n0".to_owned());
    let n1 = UrbanNodeId::from("n1".to_owned());
    let n2 = UrbanNodeId::from("n2".to_owned());
    let n3 = UrbanNodeId::from("n3".to_owned());

    let map = UrbanMap {
        nodes: vec![
            node(&n0, 0.0, 0.0),
            node(&n1, 20.0, 0.0),
            node(&n2, 20.0, 20.0),
            node(&n3, 0.0, 20.0),
        ],
        edges: vec![
            edge("road-n0-n1", &n0, &n1, 20.0, false),
            edge("road-n1-n2", &n1, &n2, 20.0, false),
            edge("road-n2-n3", &n2, &n3, 20.0, false),
            edge("road-n3-n0", &n3, &n0, 20.0, false),
            edge("blocked-diagonal", &n0, &n2, 15.0, true),
        ],
        static_obstacles: vec![UrbanStaticObstacle {
            id: UrbanObstacleId::from("building-center".to_owned()),
            bounds: Aabb {
                min_x: 8.0,
                min_y: 8.0,
                max_x: 12.0,
                max_y: 12.0,
            },
            label: Some("building".to_owned()),
        }],
    };

    let route_loop = UrbanRouteLoop {
        nodes: vec![n0.clone(), n1.clone(), n2.clone(), n3.clone(), n0.clone()],
    };
    let agents = vec![Agent {
        id: AgentId::from("agent-0".to_owned()),
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
        comms_range: 1000.0,
        generation: 1,
        speed: 2.0,
        max_range: 1000.0,
        battery_drain_rate: 0.0,
        battery_model: None,
    }];

    let tasks = [
        ("urban-waypoint-n1", 20.0, 0.0),
        ("urban-waypoint-n2", 20.0, 20.0),
        ("urban-waypoint-n3", 0.0, 20.0),
        ("urban-waypoint-n0", 0.0, 0.0),
    ]
    .into_iter()
    .map(|(id, x, y)| Task {
        id: TaskId::from(id.to_owned()),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority: 1,
        required_capabilities: vec![],
        required_role: None,
        preferred_role: Some(Role::Scout),
        expires_at: None,
        pose: Some(Pose {
            x,
            y,
            ..Default::default()
        }),
        grid_cell: None,
        edge_id: None,
        kind: Some(TaskKind::Waypoint),
    })
    .collect();

    let scenario = Scenario {
        name: "urban_patrol_small_block".to_owned(),
        seed: config.seed,
        agents,
        tasks,
        ground_nodes: vec![],
        base_station: Some(Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        }),
        geo_origin: Some(GeoOrigin {
            lat_deg: 47.397_742,
            lon_deg: 8.545_594,
            alt_m: 0.0,
        }),
    };

    let run_config = RunConfig {
        max_ticks: config.max_ticks,
        timeout_ticks: 3,
        max_unassigned_ticks: config.max_ticks,
        enable_movement: true,
        tick_duration_ms: 1000,
        urban_state: Some(UrbanState {
            map,
            route_loop,
            start_node: Some(n0),
            planner: "dijkstra".to_owned(),
        }),
        ..Default::default()
    };

    (scenario, run_config)
}

pub fn build_urban_multi_agent_scenario(config: &UrbanConfig) -> (Scenario, RunConfig) {
    let (mut scenario, run_config) = build_urban_patrol_scenario(config);
    scenario.name = "urban_multi_agent_small_block".to_owned();
    scenario.agents.push(Agent {
        id: AgentId::from("agent-1".to_owned()),
        role: Role::Scout,
        health: Health::Alive,
        pose: Pose {
            x: 1.0,
            y: 0.0,
            ..Default::default()
        },
        capabilities: vec![],
        current_task: None,
        battery: 100.0,
        comms_range: 1000.0,
        generation: 1,
        speed: 2.0,
        max_range: 1000.0,
        battery_drain_rate: 0.0,
        battery_model: None,
    });
    (scenario, run_config)
}

pub fn build_urban_search_scenario(
    config: &UrbanConfig,
    profile: UrbanProfile,
) -> (Scenario, RunConfig) {
    let (mut scenario, mut run_config) = build_urban_patrol_scenario(config);
    let (scenario_name, bus_pose, detection_range_m, detection_probability, false_positive_rate) =
        match profile {
            UrbanProfile::SearchStaticBus => (
                "urban_search_static_bus",
                Pose {
                    x: 4.0,
                    y: 0.0,
                    ..Default::default()
                },
                0.1,
                1.0,
                0.0,
            ),
            UrbanProfile::SearchOutOfRange => (
                "urban_search_out_of_range",
                Pose {
                    x: 100.0,
                    y: 100.0,
                    ..Default::default()
                },
                1.0,
                1.0,
                0.0,
            ),
            UrbanProfile::SearchFalsePositive => (
                "urban_search_false_positive",
                Pose {
                    x: 100.0,
                    y: 100.0,
                    ..Default::default()
                },
                1.0,
                0.0,
                1.0,
            ),
            UrbanProfile::PatrolSmallBlock => (
                "urban_search_static_bus",
                Pose {
                    x: 4.0,
                    y: 0.0,
                    ..Default::default()
                },
                0.1,
                1.0,
                0.0,
            ),
            UrbanProfile::MultiAgentSmallBlock => (
                "urban_search_static_bus",
                Pose {
                    x: 4.0,
                    y: 0.0,
                    ..Default::default()
                },
                0.1,
                1.0,
                0.0,
            ),
        };
    scenario.name = scenario_name.to_owned();
    run_config.max_ticks = config.max_ticks;
    run_config.max_unassigned_ticks = config.max_ticks;
    run_config.urban_search_state = Some(UrbanSearchState {
        buses: vec![UrbanBus {
            id: UrbanBusId::from("bus-0".to_owned()),
            pose: bus_pose,
            active_from_tick: None,
            active_until_tick: None,
        }],
        detector: UrbanDetectorConfig {
            detection_range_m,
            detection_probability,
            false_positive_rate,
            seed: config.seed ^ 0x66,
        },
    });
    (scenario, run_config)
}

fn node(id: &UrbanNodeId, x: f64, y: f64) -> UrbanNode {
    UrbanNode {
        id: id.clone(),
        pose: Pose {
            x,
            y,
            ..Default::default()
        },
    }
}

fn edge(id: &str, from: &UrbanNodeId, to: &UrbanNodeId, length_m: f64, blocked: bool) -> UrbanEdge {
    UrbanEdge {
        id: UrbanEdgeId::from(id.to_owned()),
        from: from.clone(),
        to: to.clone(),
        cost: length_m,
        length_m,
        corridor_width_m: Some(6.0),
        blocked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urban_patrol_fixture_has_valid_map() {
        let (_, run_config) =
            build_urban_patrol_scenario(&UrbanProfile::PatrolSmallBlock.config(42));
        let urban_state = run_config.urban_state.unwrap();
        assert!(urban_state.map.validate().is_empty());
        assert!(urban_state
            .map
            .validate_route_loop(&urban_state.route_loop)
            .is_empty());
    }

    #[test]
    fn urban_patrol_fixture_route_is_plannable() {
        let (_, run_config) =
            build_urban_patrol_scenario(&UrbanProfile::PatrolSmallBlock.config(42));
        let urban_state = run_config.urban_state.unwrap();
        let route =
            swarm_sim::expand_route_loop(&urban_state.map, &urban_state.route_loop).unwrap();
        assert_eq!(route.segments.len(), 4);
        assert_eq!(route.total_length_m, 80.0);
        assert!(swarm_sim::judge_route(&urban_state.map, &route).is_empty());
    }

    #[test]
    fn urban_patrol_fixture_completes_under_m65_runner() {
        let (scenario, run_config) =
            build_urban_patrol_scenario(&UrbanProfile::PatrolSmallBlock.config(42));
        let metrics = swarm_sim::ScenarioRunner::run(&scenario, run_config);
        assert!(metrics.success);
        assert!(metrics.urban_patrol_completed);
        assert_eq!(metrics.urban_time_to_complete_loop, Some(40));
        assert_eq!(metrics.urban_violation_count, 0);
    }

    #[test]
    fn urban_multi_agent_fixture_is_valid_for_analysis() {
        let (scenario, run_config) =
            build_urban_multi_agent_scenario(&UrbanProfile::MultiAgentSmallBlock.config(42));
        assert_eq!(scenario.name, "urban_multi_agent_small_block");
        assert_eq!(scenario.agents.len(), 2);
        assert!(
            UrbanStandardProfiles::multi_agent_profile_names().contains(&"multi-agent-small-block")
        );
        let urban_state = run_config.urban_state.unwrap();
        let route =
            swarm_sim::expand_route_loop(&urban_state.map, &urban_state.route_loop).unwrap();
        assert!(swarm_sim::judge_route(&urban_state.map, &route).is_empty());
    }

    #[test]
    fn urban_search_fixture_has_valid_search_state() {
        let (_, run_config) = build_urban_search_scenario(
            &UrbanProfile::SearchStaticBus.config(42),
            UrbanProfile::SearchStaticBus,
        );
        let urban_search_state = run_config
            .urban_search_state
            .as_ref()
            .expect("urban_search_state exists");
        assert!(urban_search_state.validate().is_empty());
    }

    #[test]
    fn urban_search_static_bus_fixture_detects_target() {
        let (scenario, run_config) = build_urban_search_scenario(
            &UrbanProfile::SearchStaticBus.config(42),
            UrbanProfile::SearchStaticBus,
        );
        let metrics = swarm_sim::ScenarioRunner::run(&scenario, run_config);
        assert!(metrics.success);
        assert!(metrics.bus_detected);
        assert_eq!(metrics.time_to_detect_bus, Some(2));
        assert!(metrics.search_success_without_violation);
    }

    #[test]
    fn urban_search_out_of_range_fixture_times_out() {
        let (scenario, run_config) = build_urban_search_scenario(
            &UrbanProfile::SearchOutOfRange.config(42),
            UrbanProfile::SearchOutOfRange,
        );
        let metrics = swarm_sim::ScenarioRunner::run(&scenario, run_config);
        assert!(!metrics.success);
        assert!(!metrics.bus_detected);
        assert_eq!(metrics.time_to_detect_bus, None);
    }
}
