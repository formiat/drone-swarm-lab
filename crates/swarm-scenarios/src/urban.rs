use std::collections::HashMap;

use swarm_sim::{GeoOrigin, RunConfig, Scenario, UrbanDeconflictionConfig, UrbanState};
use swarm_types::{
    Aabb, Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskKind, TaskStatus,
    UrbanBlockedPolicy, UrbanBus, UrbanBusId, UrbanBusRoute, UrbanBusStop, UrbanDetectorConfig,
    UrbanEdge, UrbanEdgeId, UrbanMap, UrbanNode, UrbanNodeId, UrbanObstacleId,
    UrbanPerimeterPatrol, UrbanRightOfWayPolicy, UrbanRouteLoop, UrbanSearchState,
    UrbanStaticObstacle, UrbanTemporaryObstacle,
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
    SearchMovingBus,
    SearchOutOfRange,
    SearchFalsePositive,
    PerimeterSquare,
    BlockedRouteWaitAndContinue,
    BlockedRouteReplan,
    BlockedRouteNoAlternative,
    DeconflictFirstCome,
    DeconflictPriority,
    DeconflictRoundRobin,
    DeconflictReplan,
    DeconflictAbort,
}

impl UrbanProfile {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "patrol-small-block" | "patrolsmallblock" => Some(Self::PatrolSmallBlock),
            "multi-agent-small-block" | "multiagentsmallblock" => Some(Self::MultiAgentSmallBlock),
            "search-static-bus" | "searchstaticbus" => Some(Self::SearchStaticBus),
            "search-moving-bus" | "searchmovingbus" => Some(Self::SearchMovingBus),
            "search-out-of-range" | "searchoutofrange" => Some(Self::SearchOutOfRange),
            "search-false-positive" | "searchfalsepositive" => Some(Self::SearchFalsePositive),
            "perimeter-square" | "perimetersquare" => Some(Self::PerimeterSquare),
            "blocked-route-wait" | "blockedroutewait" => Some(Self::BlockedRouteWaitAndContinue),
            "blocked-route-replan" | "blockedroutereplan" => Some(Self::BlockedRouteReplan),
            "blocked-route-no-alt" | "blockedroutenoalt" => Some(Self::BlockedRouteNoAlternative),
            "deconflict-first-come" | "deconflictfirstcome" => Some(Self::DeconflictFirstCome),
            "deconflict-priority" | "deconflictpriority" => Some(Self::DeconflictPriority),
            "deconflict-round-robin" | "deconflictroundrobin" => Some(Self::DeconflictRoundRobin),
            "deconflict-replan" | "deconflictreplan" => Some(Self::DeconflictReplan),
            "deconflict-abort" | "deconflictabort" => Some(Self::DeconflictAbort),
            _ => None,
        }
    }

    pub fn config(&self, seed: u64) -> UrbanConfig {
        match self {
            Self::PatrolSmallBlock | Self::MultiAgentSmallBlock | Self::PerimeterSquare => {
                UrbanConfig {
                    seed,
                    max_ticks: 120,
                }
            }
            Self::SearchStaticBus | Self::SearchMovingBus => UrbanConfig {
                seed,
                max_ticks: 60,
            },
            Self::SearchOutOfRange => UrbanConfig {
                seed,
                max_ticks: 12,
            },
            Self::SearchFalsePositive => UrbanConfig { seed, max_ticks: 6 },
            Self::BlockedRouteWaitAndContinue => UrbanConfig {
                seed,
                max_ticks: 60,
            },
            Self::BlockedRouteReplan => UrbanConfig {
                seed,
                max_ticks: 60,
            },
            Self::BlockedRouteNoAlternative => UrbanConfig {
                seed,
                max_ticks: 30,
            },
            Self::DeconflictFirstCome
            | Self::DeconflictPriority
            | Self::DeconflictRoundRobin
            | Self::DeconflictReplan
            | Self::DeconflictAbort => UrbanConfig {
                seed,
                max_ticks: 80,
            },
        }
    }
}

pub struct UrbanStandardProfiles;

impl UrbanStandardProfiles {
    pub fn profile_names() -> Vec<&'static str> {
        Self::patrol_profile_names()
    }

    pub fn patrol_profile_names() -> Vec<&'static str> {
        vec!["patrol-small-block", "perimeter-square"]
    }

    pub fn multi_agent_profile_names() -> Vec<&'static str> {
        vec![
            "multi-agent-small-block",
            "deconflict-first-come",
            "deconflict-priority",
            "deconflict-round-robin",
            "deconflict-replan",
            "deconflict-abort",
        ]
    }

    pub fn search_profile_names() -> Vec<&'static str> {
        vec![
            "search-static-bus",
            "search-moving-bus",
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
            mission_template: None,
            start_node: Some(n0),
            planner: "dijkstra".to_owned(),
            temporary_obstacles: vec![],
            blocked_route_policy: UrbanBlockedPolicy::default(),
            deconfliction: Default::default(),
            perimeter_patrol: None,
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

pub fn build_urban_deconfliction_scenario(
    config: &UrbanConfig,
    profile: UrbanProfile,
) -> (Scenario, RunConfig) {
    let (mut scenario, mut run_config) = build_urban_patrol_scenario(config);
    let agent_1_id = AgentId::from("agent-1".to_owned());
    scenario.name = match profile {
        UrbanProfile::DeconflictPriority => "urban_deconflict_priority",
        UrbanProfile::DeconflictRoundRobin => "urban_deconflict_round_robin",
        UrbanProfile::DeconflictReplan => "urban_deconflict_replan",
        UrbanProfile::DeconflictAbort => "urban_deconflict_abort",
        UrbanProfile::DeconflictFirstCome => "urban_deconflict_first_come",
        _ => "urban_deconflict_first_come",
    }
    .to_owned();
    scenario.agents.push(patrol_agent("agent-1", 0.0, 0.0, 2.0));

    if let Some(urban_state) = run_config.urban_state.as_mut() {
        let (right_of_way_policy, locked_segment_policy) = match profile {
            UrbanProfile::DeconflictPriority => {
                (UrbanRightOfWayPolicy::Priority, UrbanBlockedPolicy::Wait)
            }
            UrbanProfile::DeconflictRoundRobin => {
                (UrbanRightOfWayPolicy::RoundRobin, UrbanBlockedPolicy::Wait)
            }
            UrbanProfile::DeconflictReplan => {
                (UrbanRightOfWayPolicy::FirstCome, UrbanBlockedPolicy::Replan)
            }
            UrbanProfile::DeconflictAbort => {
                (UrbanRightOfWayPolicy::FirstCome, UrbanBlockedPolicy::Abort)
            }
            UrbanProfile::DeconflictFirstCome => {
                (UrbanRightOfWayPolicy::FirstCome, UrbanBlockedPolicy::Wait)
            }
            _ => (UrbanRightOfWayPolicy::FirstCome, UrbanBlockedPolicy::Wait),
        };
        let mut agent_priorities = HashMap::new();
        agent_priorities.insert(AgentId::from("agent-0".to_owned()), 1);
        agent_priorities.insert(agent_1_id, 9);
        urban_state.deconfliction = UrbanDeconflictionConfig {
            enabled: true,
            mode: Default::default(),
            right_of_way_policy,
            locked_segment_policy,
            agent_priorities,
        };
    }

    (scenario, run_config)
}

pub fn build_urban_perimeter_scenario(config: &UrbanConfig) -> (Scenario, RunConfig) {
    let (mut scenario, mut run_config) = build_urban_patrol_scenario(config);
    scenario.name = "urban_perimeter_square".to_owned();
    if let Some(urban_state) = run_config.urban_state.as_mut() {
        urban_state.perimeter_patrol = Some(UrbanPerimeterPatrol {
            polygon: square_block_polygon(),
            spacing_m: 10.0,
        });
    }
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
            UrbanProfile::SearchMovingBus => (
                "urban_search_moving_bus",
                Pose {
                    x: 100.0,
                    y: 100.0,
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
            UrbanProfile::PerimeterSquare => (
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
            UrbanProfile::BlockedRouteWaitAndContinue
            | UrbanProfile::BlockedRouteReplan
            | UrbanProfile::BlockedRouteNoAlternative
            | UrbanProfile::DeconflictFirstCome
            | UrbanProfile::DeconflictPriority
            | UrbanProfile::DeconflictRoundRobin
            | UrbanProfile::DeconflictReplan
            | UrbanProfile::DeconflictAbort => {
                ("urban_search_static_bus", Pose::default(), 1.0, 1.0, 0.0)
            }
        };
    scenario.name = scenario_name.to_owned();
    run_config.max_ticks = config.max_ticks;
    run_config.max_unassigned_ticks = config.max_ticks;
    let route = matches!(profile, UrbanProfile::SearchMovingBus).then(|| UrbanBusRoute {
        stops: vec![
            UrbanBusStop {
                node_id: UrbanNodeId::from("n1".to_owned()),
                arrival_tick: 0,
            },
            UrbanBusStop {
                node_id: UrbanNodeId::from("n0".to_owned()),
                arrival_tick: 10,
            },
        ],
        speed_m_per_tick: 2.0,
    });
    run_config.urban_search_state = Some(UrbanSearchState {
        buses: vec![UrbanBus {
            id: UrbanBusId::from("bus-0".to_owned()),
            pose: bus_pose,
            active_from_tick: None,
            active_until_tick: None,
            route,
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

fn square_block_polygon() -> Vec<Pose> {
    vec![
        Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        },
        Pose {
            x: 20.0,
            y: 0.0,
            ..Default::default()
        },
        Pose {
            x: 20.0,
            y: 20.0,
            ..Default::default()
        },
        Pose {
            x: 0.0,
            y: 20.0,
            ..Default::default()
        },
    ]
}

/// Build a single-agent scenario for `BlockedRouteWaitAndContinue`:
/// linear map A→B→C→D, obstacle on B→C from tick 5 to tick 15, policy=Wait.
pub fn build_blocked_route_wait_scenario(config: &UrbanConfig) -> (Scenario, RunConfig) {
    let n0 = UrbanNodeId::from("n0".to_owned());
    let n1 = UrbanNodeId::from("n1".to_owned());
    let n2 = UrbanNodeId::from("n2".to_owned());
    let n3 = UrbanNodeId::from("n3".to_owned());

    let map = UrbanMap {
        nodes: vec![
            node(&n0, 0.0, 0.0),
            node(&n1, 10.0, 0.0),
            node(&n2, 20.0, 0.0),
            node(&n3, 30.0, 0.0),
        ],
        edges: vec![
            edge("e-n0-n1", &n0, &n1, 10.0, false),
            edge("e-n1-n2", &n1, &n2, 10.0, false),
            edge("e-n2-n3", &n2, &n3, 10.0, false),
            edge("e-n3-n0", &n3, &n0, 30.0, false),
        ],
        static_obstacles: vec![],
    };
    let route_loop = UrbanRouteLoop {
        nodes: vec![n0.clone(), n1.clone(), n2.clone(), n3.clone(), n0.clone()],
    };
    let agent = patrol_agent("agent-0", 0.0, 0.0, 2.0);
    let scenario = Scenario {
        name: "blocked_route_wait_and_continue".to_owned(),
        seed: config.seed,
        agents: vec![agent],
        tasks: vec![],
        ground_nodes: vec![],
        base_station: None,
        geo_origin: None,
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
            mission_template: None,
            start_node: Some(n0),
            planner: "dijkstra".to_owned(),
            temporary_obstacles: vec![UrbanTemporaryObstacle {
                edge_id: UrbanEdgeId::from("e-n1-n2".to_owned()),
                appears_at_tick: 5,
                disappears_at_tick: Some(15),
                reason: Some("construction".to_owned()),
                severity: None,
            }],
            blocked_route_policy: UrbanBlockedPolicy::Wait,
            deconfliction: Default::default(),
            perimeter_patrol: None,
        }),
        ..Default::default()
    };
    (scenario, run_config)
}

/// Build a single-agent scenario for `BlockedRouteReplan`:
/// map with two paths A→C (via B) and A→D→C; the direct edge A→B is blocked from tick 0.
/// Policy=Replan → agent uses the alternate path A→D→C.
pub fn build_blocked_route_replan_scenario(config: &UrbanConfig) -> (Scenario, RunConfig) {
    let na = UrbanNodeId::from("nA".to_owned());
    let nb = UrbanNodeId::from("nB".to_owned());
    let nc = UrbanNodeId::from("nC".to_owned());
    let nd = UrbanNodeId::from("nD".to_owned());

    // A --(e-AB)--> B --(e-BC)--> C
    // A --(e-AD)--> D --(e-DC)--> C
    // C --(e-CA)--> A  (close loop)
    let map = UrbanMap {
        nodes: vec![
            node(&na, 0.0, 0.0),
            node(&nb, 10.0, 0.0),
            node(&nc, 20.0, 0.0),
            node(&nd, 10.0, 10.0),
        ],
        edges: vec![
            edge("e-AB", &na, &nb, 10.0, false),
            edge("e-BC", &nb, &nc, 10.0, false),
            edge("e-AD", &na, &nd, 12.0, false),
            edge("e-DC", &nd, &nc, 12.0, false),
            edge("e-CA", &nc, &na, 20.0, false),
        ],
        static_obstacles: vec![],
    };
    let route_loop = UrbanRouteLoop {
        nodes: vec![na.clone(), nc.clone(), na.clone()],
    };
    let agent = patrol_agent("agent-0", 0.0, 0.0, 2.0);
    let scenario = Scenario {
        name: "blocked_route_replan".to_owned(),
        seed: config.seed,
        agents: vec![agent],
        tasks: vec![],
        ground_nodes: vec![],
        base_station: None,
        geo_origin: None,
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
            mission_template: None,
            start_node: Some(na),
            planner: "dijkstra".to_owned(),
            temporary_obstacles: vec![UrbanTemporaryObstacle {
                edge_id: UrbanEdgeId::from("e-AB".to_owned()),
                appears_at_tick: 0,
                disappears_at_tick: None,
                reason: Some("road closed".to_owned()),
                severity: None,
            }],
            blocked_route_policy: UrbanBlockedPolicy::Replan,
            deconfliction: Default::default(),
            perimeter_patrol: None,
        }),
        ..Default::default()
    };
    (scenario, run_config)
}

/// Build a single-agent scenario for `BlockedRouteNoAlternative`:
/// linear map A→B→C, both edges blocked from tick 0.
/// Policy=Replan → no alternate route → abort with explicit reason.
pub fn build_blocked_route_no_alternative_scenario(config: &UrbanConfig) -> (Scenario, RunConfig) {
    let na = UrbanNodeId::from("nA".to_owned());
    let nb = UrbanNodeId::from("nB".to_owned());
    let nc = UrbanNodeId::from("nC".to_owned());

    let map = UrbanMap {
        nodes: vec![
            node(&na, 0.0, 0.0),
            node(&nb, 10.0, 0.0),
            node(&nc, 20.0, 0.0),
        ],
        edges: vec![
            edge("e-AB", &na, &nb, 10.0, false),
            edge("e-BC", &nb, &nc, 10.0, false),
            edge("e-CA", &nc, &na, 20.0, false),
        ],
        static_obstacles: vec![],
    };
    let route_loop = UrbanRouteLoop {
        nodes: vec![na.clone(), nb.clone(), nc.clone(), na.clone()],
    };
    let agent = patrol_agent("agent-0", 0.0, 0.0, 2.0);
    let scenario = Scenario {
        name: "blocked_route_no_alternative".to_owned(),
        seed: config.seed,
        agents: vec![agent],
        tasks: vec![],
        ground_nodes: vec![],
        base_station: None,
        geo_origin: None,
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
            mission_template: None,
            start_node: Some(na),
            planner: "dijkstra".to_owned(),
            temporary_obstacles: vec![
                UrbanTemporaryObstacle {
                    edge_id: UrbanEdgeId::from("e-AB".to_owned()),
                    appears_at_tick: 0,
                    disappears_at_tick: None,
                    reason: None,
                    severity: None,
                },
                UrbanTemporaryObstacle {
                    edge_id: UrbanEdgeId::from("e-BC".to_owned()),
                    appears_at_tick: 0,
                    disappears_at_tick: None,
                    reason: None,
                    severity: None,
                },
            ],
            blocked_route_policy: UrbanBlockedPolicy::Replan,
            deconfliction: Default::default(),
            perimeter_patrol: None,
        }),
        ..Default::default()
    };
    (scenario, run_config)
}

fn patrol_agent(id: &str, x: f64, y: f64, speed: f64) -> Agent {
    Agent {
        id: AgentId::from(id.to_owned()),
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
        comms_range: 1000.0,
        generation: 1,
        speed,
        max_range: 1000.0,
        battery_drain_rate: 0.0,
        battery_model: None,
    }
}

fn node(id: &UrbanNodeId, x: f64, y: f64) -> UrbanNode {
    UrbanNode {
        id: id.clone(),
        pose: Pose {
            x,
            y,
            ..Default::default()
        },
        geo: None,
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
    fn urban_search_moving_bus_fixture_detects_target() {
        let (scenario, run_config) = build_urban_search_scenario(
            &UrbanProfile::SearchMovingBus.config(42),
            UrbanProfile::SearchMovingBus,
        );
        let metrics = swarm_sim::ScenarioRunner::run(&scenario, run_config);
        assert!(metrics.success);
        assert!(metrics.bus_detected);
        assert_eq!(metrics.time_to_detect_bus, Some(5));
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

    #[test]
    fn urban_perimeter_fixture_completes_with_perimeter_metrics() {
        let (scenario, run_config) =
            build_urban_perimeter_scenario(&UrbanProfile::PerimeterSquare.config(42));
        let metrics = swarm_sim::ScenarioRunner::run(&scenario, run_config);
        assert!(metrics.success);
        assert!(metrics.urban_patrol_completed);
        assert_eq!(metrics.perimeter_length_m, 80.0);
        assert_eq!(metrics.perimeter_completion_rate, 1.0);
        assert_eq!(metrics.time_to_complete_perimeter, Some(40));
        assert_eq!(metrics.perimeter_violations, 0);
    }

    #[test]
    fn urban_perimeter_fixture_waypoints_are_deterministic() {
        let (_, run_config) =
            build_urban_perimeter_scenario(&UrbanProfile::PerimeterSquare.config(42));
        let perimeter = run_config
            .urban_state
            .as_ref()
            .and_then(|urban_state| urban_state.perimeter_patrol.as_ref())
            .expect("perimeter patrol exists");
        let waypoints =
            swarm_sim::urban::perimeter_waypoints(&perimeter.polygon, perimeter.spacing_m)
                .expect("perimeter waypoints are valid");
        let waypoints_again =
            swarm_sim::urban::perimeter_waypoints(&perimeter.polygon, perimeter.spacing_m)
                .expect("perimeter waypoints are valid");
        assert_eq!(waypoints, waypoints_again);
        assert_eq!(waypoints.len(), 9);
        assert_eq!(waypoints.first(), waypoints.last());
    }
}

#[test]
fn blocked_route_wait_scenario_completes_after_unblock() {
    let (scenario, run_config) =
        build_blocked_route_wait_scenario(&UrbanProfile::BlockedRouteWaitAndContinue.config(42));
    let metrics = swarm_sim::ScenarioRunner::run(&scenario, run_config);
    // Agent should complete the patrol after waiting for the obstacle to clear.
    assert!(
        metrics.urban_patrol_completed,
        "patrol should complete after wait"
    );
    assert!(metrics.success, "run should succeed");
    assert!(
        metrics.urban_wait_time_ticks > 0,
        "agent should have waited"
    );
    assert_eq!(metrics.urban_violation_count, 0, "no violations expected");
}

#[test]
fn blocked_route_replan_scenario_uses_alternate_route() {
    let (scenario, run_config) =
        build_blocked_route_replan_scenario(&UrbanProfile::BlockedRouteReplan.config(42));
    let metrics = swarm_sim::ScenarioRunner::run(&scenario, run_config);
    assert!(
        metrics.urban_patrol_completed,
        "patrol should complete via alternate route"
    );
    assert!(metrics.success, "run should succeed");
    assert!(
        metrics.urban_replan_count > 0,
        "route should have been replanned"
    );
    assert_eq!(metrics.urban_violation_count, 0, "no violations expected");
}

#[test]
fn blocked_route_no_alternative_fails_safely() {
    let (scenario, run_config) = build_blocked_route_no_alternative_scenario(
        &UrbanProfile::BlockedRouteNoAlternative.config(42),
    );
    let metrics = swarm_sim::ScenarioRunner::run(&scenario, run_config);
    assert!(
        !metrics.success,
        "run should fail when no route is available"
    );
    assert!(
        metrics.urban_unresolved_blockage_count > 0,
        "unresolved blockage should be recorded"
    );
}

#[test]
fn blocked_route_wait_replay_contains_expected_events() {
    use swarm_replay::Event;
    let (scenario, run_config) =
        build_blocked_route_wait_scenario(&UrbanProfile::BlockedRouteWaitAndContinue.config(42));
    let (metrics, log) = swarm_sim::ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    assert!(metrics.urban_patrol_completed);
    let log = log.expect("replay log should be present");
    assert!(
        log.events
            .iter()
            .any(|e| matches!(e, Event::UrbanWaitStarted { .. })),
        "replay should contain UrbanWaitStarted"
    );
    assert!(
        log.events
            .iter()
            .any(|e| matches!(e, Event::UrbanWaitCompleted { .. })),
        "replay should contain UrbanWaitCompleted"
    );
    assert!(
        log.events
            .iter()
            .any(|e| matches!(e, Event::UrbanPolicyDecision { policy, .. } if policy == "wait")),
        "replay should contain UrbanPolicyDecision(wait)"
    );
}

#[test]
fn blocked_route_replan_replay_contains_replanned_event() {
    use swarm_replay::Event;
    let (scenario, run_config) =
        build_blocked_route_replan_scenario(&UrbanProfile::BlockedRouteReplan.config(42));
    let (_, log) = swarm_sim::ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let log = log.expect("replay log should be present");
    assert!(
        log.events
            .iter()
            .any(|e| matches!(e, Event::UrbanRouteReplanned { .. })),
        "replay should contain UrbanRouteReplanned"
    );
}

#[test]
fn blocked_route_no_alt_replay_contains_no_route_event() {
    use swarm_replay::Event;
    let (scenario, run_config) = build_blocked_route_no_alternative_scenario(
        &UrbanProfile::BlockedRouteNoAlternative.config(42),
    );
    let (_, log) = swarm_sim::ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let log = log.expect("replay log should be present");
    assert!(
        log.events
            .iter()
            .any(|e| matches!(e, Event::UrbanNoRouteAvailable { .. })),
        "replay should contain UrbanNoRouteAvailable"
    );
}
