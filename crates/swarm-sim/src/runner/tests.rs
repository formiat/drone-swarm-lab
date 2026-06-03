use super::*;
use swarm_alloc::{AllocationAgent, AllocationTask, Allocator};
use swarm_types::{
    Aabb, Agent, Capability, CellState, EdgeId, Health, InspectionEdge, Pose, Role, SearchGrid,
    SensorModel, Task, TaskKind, TaskStatus, UrbanBus, UrbanBusId, UrbanDetectorConfig, UrbanEdge,
    UrbanEdgeId, UrbanNode, UrbanNodeId, UrbanObstacleId, UrbanPerimeterPatrol, UrbanRouteLoop,
    UrbanSearchState, UrbanStaticObstacle,
};

fn scenario(seed: u64, agent_count: usize, task_count: usize) -> Scenario {
    let agents = (0..agent_count)
        .map(|index| Agent {
            id: AgentId::from(format!("agent-{index}")),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            capabilities: Vec::new(),
            current_task: None,
            battery: 100.0,
            comms_range: f64::INFINITY,
            generation: 1,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
            battery_model: None,
        })
        .collect();
    let tasks = (0..task_count)
        .map(|index| Task {
            id: TaskId::from(format!("task-{index}")),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
            grid_cell: None,
            edge_id: None,
            kind: None,
        })
        .collect();
    Scenario {
        name: "test".to_owned(),
        seed,
        agents,
        tasks,
        ground_nodes: vec![],
        base_station: None,
        geo_origin: None,
    }
}

fn config(failures: Vec<FailureEvent>) -> RunConfig {
    RunConfig {
        max_ticks: 50,
        timeout_ticks: 3,
        max_unassigned_ticks: 5,
        packet_loss_rate: 0.0,
        latency_ticks: 0,
        latency_per_hop: 0,
        failures,
        dynamic_tasks: vec![],
        partition_events: vec![],
        gossip_interval_ticks: 999,
        base_id: None,
        enable_movement: false,
        tick_duration_ms: 100,
        grid_state: None,
        enable_cbba: false,
        ..Default::default()
    }
}

#[test]
fn runner_no_failure_assigns_all_tasks() {
    let scenario = scenario(0, 5, 8);
    let metrics = ScenarioRunner::run(&scenario, config(Vec::new()));

    assert!(metrics.success);
    assert!(metrics.all_tasks_assigned);
}

#[test]
fn runner_dynamic_task_appears_and_gets_assigned() {
    let s = scenario(0, 3, 0);
    let dynamic_task = Task {
        id: TaskId::from("dyn-0".to_owned()),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority: 1,
        required_capabilities: vec![],
        required_role: None,
        preferred_role: None,
        expires_at: None,
        pose: None,
        grid_cell: None,
        edge_id: None,
        kind: None,
    };
    let cfg = RunConfig {
        dynamic_tasks: vec![DynamicTaskEvent {
            at_tick: 2,
            task: dynamic_task,
        }],
        ..config(vec![])
    };
    let metrics = ScenarioRunner::run(&s, cfg);
    assert!(metrics.all_tasks_assigned);
    assert_eq!(metrics.tasks_injected, 1);
}

#[test]
fn runner_expired_task_counted_in_metrics() {
    let s = scenario(0, 3, 0);
    let expiring_task = Task {
        id: TaskId::from("exp-0".to_owned()),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority: 1,
        required_capabilities: vec![Capability::from("missing".to_owned())],
        required_role: None,
        preferred_role: None,
        expires_at: Some(3),
        pose: None,
        grid_cell: None,
        edge_id: None,
        kind: None,
    };
    let cfg = RunConfig {
        dynamic_tasks: vec![DynamicTaskEvent {
            at_tick: 1,
            task: expiring_task,
        }],
        ..config(vec![])
    };
    let metrics = ScenarioRunner::run(&s, cfg);
    assert_eq!(metrics.tasks_expired, 1);
}

#[test]
fn runner_greedy_deterministic_with_capabilities() {
    let mut s = scenario(5, 4, 2);
    s.agents[0].capabilities = vec![Capability::from("optical".to_owned())];
    s.tasks[0].required_capabilities = vec![Capability::from("optical".to_owned())];

    let cfg = config(vec![]);
    let a = ScenarioRunner::run(&s, cfg.clone());
    let b = ScenarioRunner::run(&s, cfg);

    assert_eq!(a, b);
}

#[test]
fn runner_run_applies_comms_penalty_weight_to_default_greedy() {
    let mut s = scenario(0, 2, 2);
    s.agents[0].pose = Pose {
        x: 0.0,
        y: 0.0,
        ..Default::default()
    };
    s.agents[0].comms_range = 1.0;
    s.agents[0].speed = 1.0;
    s.agents[0].battery_drain_rate = 200.0;
    s.agents[1].pose = Pose {
        x: 100.0,
        y: 0.0,
        ..Default::default()
    };
    s.agents[1].comms_range = 100.0;
    s.agents[1].speed = 1.0;
    s.agents[1].battery_drain_rate = 200.0;
    for task in &mut s.tasks {
        task.pose = Some(Pose {
            x: 100.0,
            y: 0.0,
            ..Default::default()
        });
    }

    let (_, baseline_log) = ScenarioRunner::run_with_default_greedy(
        &s,
        RunConfig {
            max_ticks: 2,
            ..config(vec![])
        },
        Some(swarm_replay::EventLogBuilder::new(
            "baseline".to_owned(),
            s.seed,
            &s.name,
        )),
    );
    let (_, weighted_log) = ScenarioRunner::run_with_default_greedy(
        &s,
        RunConfig {
            max_ticks: 2,
            comms_penalty_weight: 100.0,
            ..config(vec![])
        },
        Some(swarm_replay::EventLogBuilder::new(
            "weighted".to_owned(),
            s.seed,
            &s.name,
        )),
    );

    fn final_pose_x(log: &swarm_replay::EventLog, agent: &str) -> f64 {
        log.events
            .iter()
            .rev()
            .find_map(|event| match event {
                swarm_replay::Event::PoseUpdated { agent_id, pose, .. }
                    if agent_id.as_ref() == agent =>
                {
                    Some(pose.x)
                }
                _ => None,
            })
            .expect("final pose should be recorded")
    }

    let baseline_log = baseline_log.expect("baseline log should be captured");
    let weighted_log = weighted_log.expect("weighted log should be captured");

    assert_eq!(final_pose_x(&baseline_log, "agent-0"), 100.0);
    assert_eq!(final_pose_x(&baseline_log, "agent-1"), 100.0);
    assert_eq!(final_pose_x(&weighted_log, "agent-0"), 0.0);
    assert_eq!(final_pose_x(&weighted_log, "agent-1"), 100.0);
}

#[test]
fn runner_auction_deterministic() {
    use swarm_alloc::AuctionAllocator;
    let s = scenario(9, 5, 4);
    let cfg = config(vec![]);

    let a = ScenarioRunner::run_with(&s, cfg.clone(), AuctionAllocator::default());
    let b = ScenarioRunner::run_with(&s, cfg, AuctionAllocator::default());

    assert_eq!(a, b);
}

#[test]
fn runner_capability_gate_task_stays_unassigned() {
    let s = scenario(0, 3, 0);
    let impossible_task = Task {
        id: TaskId::from("imp-0".to_owned()),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority: 1,
        required_capabilities: vec![Capability::from("unobtainium".to_owned())],
        required_role: None,
        preferred_role: None,
        expires_at: None,
        pose: None,
        grid_cell: None,
        edge_id: None,
        kind: None,
    };
    let cfg = RunConfig {
        dynamic_tasks: vec![DynamicTaskEvent {
            at_tick: 1,
            task: impossible_task,
        }],
        ..config(vec![])
    };
    let metrics = ScenarioRunner::run(&s, cfg);
    assert!(!metrics.all_tasks_assigned);
}

#[test]
fn runner_no_duplicate_ownership_invariant() {
    let s = scenario(0, 5, 5);
    let cfg = config(vec![]);
    ScenarioRunner::run(&s, cfg);
}

struct DuplicateAllocator;

impl Allocator for DuplicateAllocator {
    fn allocate(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)> {
        if tasks.is_empty() || agents.is_empty() {
            return vec![];
        }
        let task_id = tasks[0].task.id.clone();
        let agent_id = agents[0].id.clone();
        vec![(task_id.clone(), agent_id.clone()), (task_id, agent_id)]
    }
}

#[test]
fn runner_conflict_counter_in_metrics() {
    let s = scenario(0, 2, 1);
    let cfg = config(vec![]);
    let metrics = ScenarioRunner::run_with(&s, cfg, DuplicateAllocator);
    assert!(metrics.conflicting_assignments > 0);
}

#[test]
fn allocate_unassigned_counts_duplicate_allocator_output() {
    let s = scenario(0, 2, 1);
    let cfg = config(vec![]);
    let metrics = ScenarioRunner::run_with(&s, cfg, DuplicateAllocator);
    assert!(metrics.conflicting_assignments > 0);
}

#[test]
fn runner_coverage_kind_exits_before_max_ticks() {
    // Tasks with kind: CoverageCell should trigger adapter-driven early exit once assigned,
    // so total_ticks must be less than max_ticks.
    use swarm_types::TaskKind;
    let scenario = {
        let agents = (0..3)
            .map(|i| Agent {
                id: AgentId::from(format!("agent-{i}")),
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
                comms_range: f64::INFINITY,
                generation: 1,
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
                battery_model: None,
            })
            .collect();
        let tasks = (0..3)
            .map(|i| Task {
                id: TaskId::from(format!("task-{i}")),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: 1,
                required_capabilities: vec![],
                required_role: None,
                preferred_role: None,
                expires_at: None,
                pose: None,
                grid_cell: None,
                edge_id: None,
                kind: Some(TaskKind::CoverageCell),
            })
            .collect();
        Scenario {
            name: "coverage_early_exit".to_owned(),
            seed: 0,
            agents,
            tasks,
            ground_nodes: vec![],
            base_station: None,
            geo_origin: None,
        }
    };
    let cfg = RunConfig {
        max_ticks: 200,
        ..config(vec![])
    };
    let metrics = ScenarioRunner::run(&scenario, cfg);
    assert!(
        metrics.total_ticks < 200,
        "coverage with kind-tagged tasks should exit early, got total_ticks={}",
        metrics.total_ticks
    );
    assert!(metrics.all_tasks_assigned);
}

#[test]
fn extension_fixture_runs_mapping_mission_with_replay_log() {
    let mut scenario = scenario(0, 1, 0);
    scenario.name = "extension_fixture".to_owned();
    scenario.tasks = vec![semantic_task("zone-0", TaskKind::MappingZone)];

    let cfg = RunConfig {
        max_ticks: 10,
        wildfire_state: Some(WildfireState {
            zones: vec![WildfireZone {
                id: "zone-0".to_owned(),
                threat_level: 1.0,
                priority: 1,
            }],
            mapped_zone_ids: std::collections::HashSet::new(),
            update_interval_ticks: 999,
            enable_dynamic_threat: false,
            enable_zone_expansion: false,
            enable_spatial_spread: false,
        }),
        ..config(vec![])
    };

    let (metrics, event_log) =
        ScenarioRunner::run_with_log(&scenario, cfg, swarm_alloc::GreedyAllocator::default());
    let event_log = event_log.expect("run_with_log should return an event log");

    assert!(metrics.success);
    assert_eq!(metrics.hazard_zones_mapped, 1);
    assert_eq!(event_log.schema_version, "0.2");
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::TaskCompleted { task_id, .. }
            if task_id.to_string() == "zone-0"
    )));
}

#[test]
fn wildfire_priority_trigger_reallocates_agent() {
    let mut scenario = scenario(0, 1, 0);
    scenario.name = "wildfire_priority_realloc".to_owned();
    let mut task = semantic_task("zone-0", TaskKind::MappingZone);
    task.priority = 7;
    task.pose = Some(Pose {
        x: 100.0,
        y: 0.0,
        ..Default::default()
    });
    scenario.tasks = vec![task];

    let cfg = RunConfig {
        max_ticks: 3,
        enable_movement: true,
        wildfire_priority_realloc_threshold: Some(8),
        wildfire_state: Some(WildfireState {
            zones: vec![WildfireZone {
                id: "zone-0".to_owned(),
                threat_level: 0.0,
                priority: 7,
            }],
            mapped_zone_ids: std::collections::HashSet::new(),
            update_interval_ticks: 1,
            enable_dynamic_threat: true,
            enable_zone_expansion: false,
            enable_spatial_spread: false,
        }),
        ..config(vec![])
    };

    let (_metrics, event_log) =
        ScenarioRunner::run_with_log(&scenario, cfg, swarm_alloc::GreedyAllocator::default());
    let event_log = event_log.expect("run_with_log should return an event log");

    assert!(event_log.events.iter().any(|event| {
        matches!(
            event,
            swarm_replay::Event::WildfirePriorityReallocationRequested {
                task_id,
                old_priority: 7,
                new_priority: 8,
                ..
            } if task_id.to_string() == "zone-0"
        )
    }));
}

#[test]
fn wildfire_priority_below_threshold_no_realloc() {
    let mut scenario = scenario(0, 1, 0);
    scenario.name = "wildfire_priority_no_realloc".to_owned();
    let mut task = semantic_task("zone-0", TaskKind::MappingZone);
    task.priority = 7;
    task.pose = Some(Pose {
        x: 100.0,
        y: 0.0,
        ..Default::default()
    });
    scenario.tasks = vec![task];

    let cfg = RunConfig {
        max_ticks: 1,
        enable_movement: true,
        wildfire_priority_realloc_threshold: Some(10),
        wildfire_state: Some(WildfireState {
            zones: vec![WildfireZone {
                id: "zone-0".to_owned(),
                threat_level: 0.0,
                priority: 7,
            }],
            mapped_zone_ids: std::collections::HashSet::new(),
            update_interval_ticks: 1,
            enable_dynamic_threat: true,
            enable_zone_expansion: false,
            enable_spatial_spread: false,
        }),
        ..config(vec![])
    };

    let (_metrics, event_log) =
        ScenarioRunner::run_with_log(&scenario, cfg, swarm_alloc::GreedyAllocator::default());
    let event_log = event_log.expect("run_with_log should return an event log");

    assert!(!event_log.events.iter().any(|event| {
        matches!(
            event,
            swarm_replay::Event::WildfirePriorityReallocationRequested { .. }
        )
    }));
}

#[test]
fn build_run_state_collects_mission_semantics() {
    let grid = SearchGrid::new(2, 2, 1.0);
    let mut grid_state =
        swarm_runtime::GridState::new(grid.clone(), vec![], SensorModel::new(1.0, 1.0, 1.0));
    grid_state.cells[grid.cell_index(1, 0)] = CellState::Visited {
        scanned_by: vec![AgentId::from("agent-0".to_owned())],
        scan_tick: 1,
    };
    grid_state.cells[grid.cell_index(0, 1)] = CellState::TargetFound {
        target_id: "target-0".to_owned(),
        found_by: AgentId::from("agent-1".to_owned()),
        found_at_tick: 2,
    };

    let edge_id = EdgeId::from("edge-0".to_owned());
    let mut inspection_state = InspectionState::new(swarm_types::InspectionGraph {
        edges: vec![InspectionEdge {
            id: edge_id.clone(),
            from: Pose::default(),
            to: Pose {
                x: 1.0,
                ..Default::default()
            },
            length_m: 1.0,
            priority: 1,
        }],
        depot: Pose::default(),
    });
    inspection_state.covered.insert(edge_id.clone());

    let mut wildfire_state = WildfireState::default();
    wildfire_state.mapped_zone_ids.insert("zone-0".to_owned());

    let mut assigned = semantic_task("assigned", TaskKind::CoverageCell);
    assigned.assigned_to = Some(AgentId::from("agent-0".to_owned()));
    let mut completed = semantic_task("completed", TaskKind::Waypoint);
    completed.status = TaskStatus::Completed;

    let state = ScenarioRunner::build_run_state(
        &Some(grid_state),
        &Some(inspection_state),
        &Some(wildfire_state),
        &[assigned.clone(), completed.clone()],
    );

    assert!(state.scanned_cells.contains(&(1, 0)));
    assert!(state.scanned_cells.contains(&(0, 1)));
    assert!(state.covered_edges.contains(&edge_id));
    assert!(state.mapped_zones.contains("zone-0"));
    assert!(state.completed_tasks.contains(&assigned.id));
    assert!(state.completed_tasks.contains(&completed.id));
}

#[test]
fn adapter_driven_complete_requires_exact_semantic_state() {
    let registry = AdapterRegistry::new();
    let mut sar = semantic_task("sar-0", TaskKind::SarScan);
    sar.grid_cell = Some((1, 1));
    let mut inspection = semantic_task("inspection-0", TaskKind::InspectionEdge);
    inspection.edge_id = Some(EdgeId::from("edge-0".to_owned()));
    let wildfire = semantic_task("zone-0", TaskKind::MappingZone);
    let waypoint = semantic_task("waypoint-0", TaskKind::Waypoint);
    let tasks = vec![
        sar.clone(),
        inspection.clone(),
        wildfire.clone(),
        waypoint.clone(),
    ];

    let mut state = RunState::default();
    state.scanned_cells.insert((1, 1));
    state
        .covered_edges
        .insert(EdgeId::from("edge-0".to_owned()));
    state.mapped_zones.insert("zone-0".to_owned());
    assert!(!ScenarioRunner::adapter_driven_complete(
        &tasks, &state, &registry
    ));

    state.completed_tasks.insert(waypoint.id.clone());
    assert!(ScenarioRunner::adapter_driven_complete(
        &tasks, &state, &registry
    ));
}

#[test]
fn cbba_distributed_path_succeeds() {
    use swarm_alloc::CbbaAllocator;
    let s = scenario(0, 3, 2);
    let mut cfg = config(vec![]);
    cfg.enable_cbba = true;
    cfg.gossip_interval_ticks = 1;
    cfg.max_ticks = 30;
    let metrics = ScenarioRunner::run_with(&s, cfg, CbbaAllocator::default());
    assert!(metrics.success, "CBBA did not complete the mission");
    assert!(metrics.cbba_messages > 0, "No CBBA messages were exchanged");
    assert!(
        metrics.cbba_rounds_to_convergence > 0,
        "CBBA did not converge"
    );
}

fn urban_test_run_config(
    max_ticks: u64,
    static_obstacles: Vec<UrbanStaticObstacle>,
) -> (Scenario, RunConfig) {
    let n0 = UrbanNodeId::from("n0".to_owned());
    let n1 = UrbanNodeId::from("n1".to_owned());
    let n2 = UrbanNodeId::from("n2".to_owned());
    let n3 = UrbanNodeId::from("n3".to_owned());
    let mut scenario = scenario(0, 1, 0);
    scenario.name = "urban-patrol".to_owned();
    scenario.agents[0].speed = 2.0;
    let run_config = RunConfig {
        max_ticks,
        tick_duration_ms: 1000,
        urban_state: Some(UrbanState {
            map: UrbanMap {
                nodes: vec![
                    UrbanNode {
                        id: n0.clone(),
                        pose: Pose {
                            x: 0.0,
                            y: 0.0,
                            ..Default::default()
                        },
                    },
                    UrbanNode {
                        id: n1.clone(),
                        pose: Pose {
                            x: 20.0,
                            y: 0.0,
                            ..Default::default()
                        },
                    },
                    UrbanNode {
                        id: n2.clone(),
                        pose: Pose {
                            x: 20.0,
                            y: 20.0,
                            ..Default::default()
                        },
                    },
                    UrbanNode {
                        id: n3.clone(),
                        pose: Pose {
                            x: 0.0,
                            y: 20.0,
                            ..Default::default()
                        },
                    },
                ],
                edges: vec![
                    UrbanEdge {
                        id: UrbanEdgeId::from("road-n0-n1".to_owned()),
                        from: n0.clone(),
                        to: n1.clone(),
                        cost: 20.0,
                        length_m: 20.0,
                        corridor_width_m: Some(4.0),
                        blocked: false,
                    },
                    UrbanEdge {
                        id: UrbanEdgeId::from("road-n1-n2".to_owned()),
                        from: n1.clone(),
                        to: n2.clone(),
                        cost: 20.0,
                        length_m: 20.0,
                        corridor_width_m: Some(4.0),
                        blocked: false,
                    },
                    UrbanEdge {
                        id: UrbanEdgeId::from("road-n2-n3".to_owned()),
                        from: n2.clone(),
                        to: n3.clone(),
                        cost: 20.0,
                        length_m: 20.0,
                        corridor_width_m: Some(4.0),
                        blocked: false,
                    },
                    UrbanEdge {
                        id: UrbanEdgeId::from("road-n3-n0".to_owned()),
                        from: n3.clone(),
                        to: n0.clone(),
                        cost: 20.0,
                        length_m: 20.0,
                        corridor_width_m: Some(4.0),
                        blocked: false,
                    },
                ],
                static_obstacles,
            },
            route_loop: UrbanRouteLoop {
                nodes: vec![n0.clone(), n1, n2, n3, n0],
            },
            start_node: Some(UrbanNodeId::from("n0".to_owned())),
            planner: "dijkstra".to_owned(),
            temporary_obstacles: vec![],
            blocked_route_policy: swarm_types::UrbanBlockedPolicy::default(),
            perimeter_patrol: None,
        }),
        ..config(vec![])
    };
    (scenario, run_config)
}

fn urban_search_test_run_config(
    bus_pose: Pose,
    detection_range_m: f64,
    detection_probability: f64,
    false_positive_rate: f64,
    max_ticks: u64,
) -> (Scenario, RunConfig) {
    let (mut scenario, mut run_config) = urban_test_run_config(max_ticks, vec![]);
    scenario.name = "urban-search".to_owned();
    run_config.urban_search_state = Some(UrbanSearchState {
        buses: vec![UrbanBus {
            id: UrbanBusId::from("bus-0".to_owned()),
            pose: bus_pose,
            active_from_tick: None,
            active_until_tick: None,
            route: None,
        }],
        detector: UrbanDetectorConfig {
            detection_range_m,
            detection_probability,
            false_positive_rate,
            seed: 11,
        },
    });
    (scenario, run_config)
}

#[test]
fn urban_patrol_completes_small_block_loop() {
    let (scenario, run_config) = urban_test_run_config(50, vec![]);

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(metrics.urban_route_planned);
    assert_eq!(metrics.urban_route_length_m, 80.0);
    assert_eq!(metrics.urban_violation_count, 0);
    assert!(metrics.urban_route_completed);
    assert!(metrics.urban_patrol_completed);
    assert_eq!(metrics.urban_time_to_complete_loop, Some(40));
    assert_eq!(metrics.urban_distance_travelled_m, 80.0);
    assert_eq!(metrics.urban_route_efficiency, 1.0);
    assert_eq!(metrics.urban_replan_count, 0);
    assert!(metrics.success);
    assert!(metrics.total_ticks < 50);
}

#[test]
fn urban_patrol_rejects_mismatched_start_node() {
    let (scenario, mut run_config) = urban_test_run_config(50, vec![]);
    run_config.urban_state.as_mut().unwrap().start_node = Some(UrbanNodeId::from("n1".to_owned()));

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(!metrics.success);
    assert!(!metrics.urban_patrol_completed);
    assert!(metrics
        .unsupported_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("start_node")));
}

#[test]
fn urban_patrol_rejects_agent_pose_away_from_start_node() {
    let (mut scenario, run_config) = urban_test_run_config(50, vec![]);
    scenario.agents[0].pose = Pose {
        x: 5.0,
        y: 0.0,
        ..Default::default()
    };

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(!metrics.success);
    assert!(!metrics.urban_patrol_completed);
    assert!(metrics
        .unsupported_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("starts")));
}

#[test]
fn urban_patrol_rejects_invalid_perimeter_config() {
    let (scenario, mut run_config) = urban_test_run_config(50, vec![]);
    run_config.urban_state.as_mut().unwrap().perimeter_patrol = Some(UrbanPerimeterPatrol {
        polygon: vec![
            Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            Pose {
                x: 10.0,
                y: 0.0,
                ..Default::default()
            },
            Pose {
                x: 10.0,
                y: 10.0,
                ..Default::default()
            },
        ],
        spacing_m: 0.0,
    });

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(!metrics.success);
    assert!(metrics
        .unsupported_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("urban_perimeter_invalid")));
}

#[test]
fn urban_patrol_timeout_does_not_complete() {
    let (scenario, run_config) = urban_test_run_config(3, vec![]);

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(metrics.urban_route_planned);
    assert_eq!(metrics.urban_route_length_m, 80.0);
    assert!(!metrics.urban_patrol_completed);
    assert!(!metrics.success);
    assert_eq!(metrics.urban_time_to_complete_loop, None);
    assert_eq!(metrics.urban_distance_travelled_m, 6.0);
}

#[test]
fn urban_patrol_violation_fails_before_completion() {
    let obstacle = UrbanStaticObstacle {
        id: UrbanObstacleId::from("road-block".to_owned()),
        bounds: Aabb {
            min_x: 8.0,
            min_y: -1.0,
            max_x: 12.0,
            max_y: 1.0,
        },
        label: Some("building".to_owned()),
    };
    let (scenario, run_config) = urban_test_run_config(50, vec![obstacle]);

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban run should produce replay log");

    assert!(metrics.urban_route_planned);
    assert!(metrics.urban_violation_count > 0);
    assert!(!metrics.urban_patrol_completed);
    assert!(!metrics.success);
    assert!(event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::UrbanViolation { .. })));
}

#[test]
fn urban_patrol_replay_records_ordered_route_events() {
    let (scenario, run_config) = urban_test_run_config(50, vec![]);

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban run should produce replay log");

    assert!(metrics.success);
    assert!(matches!(
        event_log.events.first(),
        Some(swarm_replay::Event::UrbanRoutePlanned { .. })
    ));
    assert_eq!(
        event_log
            .events
            .iter()
            .filter(|event| matches!(event, swarm_replay::Event::UrbanSegmentEntered { .. }))
            .count(),
        4
    );
    assert_eq!(
        event_log
            .events
            .iter()
            .filter(|event| matches!(event, swarm_replay::Event::UrbanSegmentCompleted { .. }))
            .count(),
        4
    );
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanPatrolCompleted { tick: 40, .. }
    )));
}

#[test]
fn urban_search_detects_bus_and_stops_before_timeout() {
    let (scenario, run_config) = urban_search_test_run_config(
        Pose {
            x: 4.0,
            y: 0.0,
            ..Default::default()
        },
        0.1,
        1.0,
        0.0,
        50,
    );

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(metrics.success);
    assert!(metrics.bus_detected);
    assert_eq!(metrics.time_to_detect_bus, Some(2));
    assert_eq!(metrics.false_positive_count, 0);
    assert_eq!(metrics.distance_before_detection, 4.0);
    assert!(metrics.search_success_without_violation);
    assert!(!metrics.urban_patrol_completed);
}

#[test]
fn urban_search_out_of_range_bus_times_out() {
    let (scenario, run_config) = urban_search_test_run_config(
        Pose {
            x: 100.0,
            y: 100.0,
            ..Default::default()
        },
        1.0,
        1.0,
        0.0,
        3,
    );

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(!metrics.success);
    assert!(!metrics.bus_detected);
    assert_eq!(metrics.time_to_detect_bus, None);
    assert_eq!(metrics.false_positive_count, 0);
    assert_eq!(metrics.urban_distance_travelled_m, 6.0);
    assert!(!metrics.search_success_without_violation);
}

#[test]
fn urban_search_false_positive_does_not_succeed() {
    let (scenario, run_config) = urban_search_test_run_config(
        Pose {
            x: 100.0,
            y: 100.0,
            ..Default::default()
        },
        1.0,
        0.0,
        1.0,
        3,
    );

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(!metrics.success);
    assert!(!metrics.bus_detected);
    assert!(metrics.false_positive_count > 0);
    assert!(!metrics.search_success_without_violation);
}

#[test]
fn urban_search_violation_prevents_success() {
    let obstacle = UrbanStaticObstacle {
        id: UrbanObstacleId::from("road-block".to_owned()),
        bounds: Aabb {
            min_x: 8.0,
            min_y: -1.0,
            max_x: 12.0,
            max_y: 1.0,
        },
        label: Some("building".to_owned()),
    };
    let (mut scenario, mut run_config) = urban_test_run_config(50, vec![obstacle]);
    scenario.name = "urban-search".to_owned();
    run_config.urban_search_state = Some(UrbanSearchState {
        buses: vec![UrbanBus {
            id: UrbanBusId::from("bus-0".to_owned()),
            pose: Pose::default(),
            active_from_tick: None,
            active_until_tick: None,
            route: None,
        }],
        detector: UrbanDetectorConfig {
            detection_range_m: 10.0,
            detection_probability: 1.0,
            false_positive_rate: 0.0,
            seed: 11,
        },
    });

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(!metrics.success);
    assert!(!metrics.bus_detected);
    assert!(metrics.urban_violation_count > 0);
    assert!(!metrics.search_success_without_violation);
}

#[test]
fn urban_search_replay_records_detection_events() {
    let (scenario, run_config) = urban_search_test_run_config(
        Pose {
            x: 4.0,
            y: 0.0,
            ..Default::default()
        },
        0.1,
        1.0,
        0.0,
        50,
    );

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban search run should produce replay log");

    assert!(metrics.success);
    assert!(event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::BusObserved { .. })));
    assert!(event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::BusDetected { .. })));
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanSearchCompleted { detected: true, .. }
    )));
}

#[test]
fn urban_search_invalid_start_fails_before_detection() {
    let (mut scenario, run_config) = urban_search_test_run_config(
        Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        },
        10.0,
        1.0,
        0.0,
        50,
    );
    scenario.agents[0].pose = Pose {
        x: 5.0,
        y: 0.0,
        ..Default::default()
    };

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(!metrics.success);
    assert!(!metrics.bus_detected);
    assert!(metrics
        .unsupported_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("starts")));
}

#[test]
fn urban_search_missing_urban_state_fails_without_panic() {
    let (scenario, mut run_config) = urban_search_test_run_config(
        Pose {
            x: 4.0,
            y: 0.0,
            ..Default::default()
        },
        0.1,
        1.0,
        0.0,
        50,
    );
    run_config.urban_state = None;

    let metrics = ScenarioRunner::run(&scenario, run_config);

    assert!(!metrics.success);
    assert!(!metrics.bus_detected);
    assert!(metrics
        .unsupported_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("missing_urban_state")));
}

fn semantic_task(id: &str, kind: TaskKind) -> Task {
    Task {
        id: TaskId::from(id.to_owned()),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority: 1,
        required_capabilities: vec![],
        required_role: None,
        preferred_role: None,
        expires_at: None,
        pose: Some(Pose::default()),
        grid_cell: None,
        edge_id: None,
        kind: Some(kind),
    }
}
