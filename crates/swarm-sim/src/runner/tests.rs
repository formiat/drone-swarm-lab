use super::*;
use swarm_alloc::{AllocationAgent, AllocationTask, Allocator};
use swarm_comms::{
    AgentAbsenceKind, ConflictResolution, DeconflictionMode, DroneLinkConfig, SupervisorDecision,
};
use swarm_runtime::autonomy::{AgentAutonomyConfig, GcsLostPolicy};
use swarm_types::{
    Aabb, Agent, Capability, CellState, EdgeId, Health, HiddenTarget, InspectionEdge, Pose, Role,
    SearchGrid, SensorModel, Task, TaskKind, TaskStatus, UrbanBlockedPolicy, UrbanBus, UrbanBusId,
    UrbanDetectorConfig, UrbanEdge, UrbanEdgeId, UrbanMap, UrbanNode, UrbanNodeId, UrbanObstacleId,
    UrbanPerimeterPatrol, UrbanRightOfWayPolicy, UrbanRouteLoop, UrbanSearchState,
    UrbanStaticObstacle, UrbanTemporaryObstacle,
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

fn sar_grid_state(targets_found: u32, targets_total: u32) -> GridState {
    let targets = (0..targets_total)
        .map(|index| HiddenTarget {
            id: format!("target-{index}"),
            cell_x: index,
            cell_y: 0,
        })
        .collect();
    let mut grid_state = GridState::new(
        SearchGrid::new(targets_total.max(1), 1, 10.0),
        targets,
        SensorModel::new(1.0, 1.0, 1.0),
    );
    grid_state.targets_found = targets_found;
    grid_state
}

fn compute_sar_success_for_threshold(
    targets_found: u32,
    targets_total: u32,
    sar_success_threshold: Option<f64>,
) -> bool {
    let grid_state = Some(sar_grid_state(targets_found, targets_total));
    compute_mission_success(
        5,
        &None,
        0.8,
        0.8,
        sar_success_threshold,
        true,
        true,
        0,
        &grid_state,
        &None,
        &None,
        &None,
        false,
        0,
        false,
        false,
    )
    .0
}

#[test]
fn sar_success_without_threshold_requires_all_targets_found() {
    assert!(compute_sar_success_for_threshold(2, 2, None));
    assert!(!compute_sar_success_for_threshold(1, 2, None));
}

#[test]
fn sar_success_threshold_accepts_partial_detection_above_threshold() {
    assert!(compute_sar_success_for_threshold(1, 2, Some(0.5)));
}

#[test]
fn sar_success_threshold_rejects_partial_detection_below_threshold() {
    assert!(!compute_sar_success_for_threshold(1, 3, Some(0.5)));
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

    let request_tick = event_log
        .events
        .iter()
        .find_map(|event| match event {
            swarm_replay::Event::WildfirePriorityReallocationRequested {
                task_id,
                old_priority: 7,
                new_priority: 8,
                tick,
                ..
            } if task_id.to_string() == "zone-0" => Some(*tick),
            _ => None,
        })
        .expect("priority reallocation request should be recorded");

    let release_tick = event_log
        .events
        .iter()
        .find_map(|event| match event {
            swarm_replay::Event::WildfirePriorityTaskReleased {
                task_id,
                old_priority: 7,
                new_priority: 8,
                previous_agent_id: Some(agent_id),
                tick,
            } if task_id.to_string() == "zone-0" && agent_id.to_string() == "agent-0" => {
                Some(*tick)
            }
            _ => None,
        })
        .expect("priority-triggered task release should be recorded");

    assert_eq!(release_tick, request_tick);
    assert!(event_log.events.iter().any(|event| {
        matches!(
            event,
            swarm_replay::Event::TaskAssigned {
                task_id,
                agent_id,
                tick,
            } if task_id.to_string() == "zone-0"
                && agent_id.to_string() == "agent-0"
                && *tick > release_tick
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
                        geo: None,
                    },
                    UrbanNode {
                        id: n1.clone(),
                        pose: Pose {
                            x: 20.0,
                            y: 0.0,
                            ..Default::default()
                        },
                        geo: None,
                    },
                    UrbanNode {
                        id: n2.clone(),
                        pose: Pose {
                            x: 20.0,
                            y: 20.0,
                            ..Default::default()
                        },
                        geo: None,
                    },
                    UrbanNode {
                        id: n3.clone(),
                        pose: Pose {
                            x: 0.0,
                            y: 20.0,
                            ..Default::default()
                        },
                        geo: None,
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
            mission_template: None,
            start_node: Some(UrbanNodeId::from("n0".to_owned())),
            planner: "dijkstra".to_owned(),
            temporary_obstacles: vec![],
            blocked_route_policy: swarm_types::UrbanBlockedPolicy::default(),
            deconfliction: Default::default(),
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
fn urban_deconfliction_prevents_duplicate_segment_ownership() {
    let (scenario, run_config) = urban_deconfliction_test_run_config(
        UrbanRightOfWayPolicy::FirstCome,
        UrbanBlockedPolicy::Wait,
    );

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban deconfliction run should produce replay log");

    assert!(metrics.success);
    assert!(metrics.urban_deconflict_conflict_count > 0);
    assert!(metrics.urban_deconflict_wait_ticks > 0);
    assert_no_duplicate_segment_ownership(&event_log);
    assert!(event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::UrbanSegmentConflict { .. })));
    assert!(event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::UrbanDeconflictWait { .. })));
}

#[test]
fn urban_deconfliction_priority_uses_agent_priorities() {
    let (scenario, run_config) = urban_deconfliction_test_run_config(
        UrbanRightOfWayPolicy::Priority,
        UrbanBlockedPolicy::Wait,
    );

    let (_metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban deconfliction run should produce replay log");

    let first_lock = event_log.events.iter().find_map(|event| match event {
        swarm_replay::Event::UrbanSegmentLockAcquired {
            agent_id,
            edge_id,
            tick: 0,
            ..
        } if edge_id.to_string() == "road-n0-n1" => Some(agent_id.to_string()),
        _ => None,
    });
    assert_eq!(first_lock.as_deref(), Some("agent-1"));
}

#[test]
fn shared_memory_deconfliction_backward_compat() {
    let (scenario, run_config) = urban_deconfliction_test_run_config(
        UrbanRightOfWayPolicy::FirstCome,
        UrbanBlockedPolicy::Wait,
    );

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban deconfliction run should produce replay log");

    assert!(metrics.success);
    assert_no_duplicate_segment_ownership(&event_log);
    assert!(!event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanSegmentCoordinatorEvent { .. }
    )));
}

#[test]
fn perimeter_patrol_sector_ownership_is_disjoint() {
    let (mut scenario, mut run_config) = urban_deconfliction_test_run_config(
        UrbanRightOfWayPolicy::FirstCome,
        UrbanBlockedPolicy::Wait,
    );
    scenario.agents[1].pose = Pose {
        x: 20.0,
        y: 20.0,
        ..Default::default()
    };
    run_config.urban_state.as_mut().unwrap().deconfliction.mode =
        DeconflictionMode::NetworkProtocol {
            coordinator_id: AgentId::from("coordinator-0".to_owned()),
        };

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban network deconfliction run should produce replay log");

    assert!(metrics.success);
    assert_no_duplicate_segment_ownership(&event_log);
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::SwarmProtocolMessage { kind, .. } if kind == "segment_reserve"
    )));
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanSegmentCoordinatorEvent { event, .. } if event == "grant_sent"
    )));
}

#[test]
fn agent_failure_handoff_runs_through_urban_network_runner() {
    let (mut scenario, mut run_config) = urban_deconfliction_test_run_config(
        UrbanRightOfWayPolicy::FirstCome,
        UrbanBlockedPolicy::Wait,
    );
    scenario.name = "urban_perimeter_patrol_network_failure".to_owned();
    scenario.agents[1].pose = Pose {
        x: 20.0,
        y: 20.0,
        ..Default::default()
    };
    run_config.max_ticks = 120;
    run_config.failures = vec![FailureEvent {
        agent_id: AgentId::from("agent-0".to_owned()),
        at_tick: 2,
    }];
    run_config.urban_state.as_mut().unwrap().deconfliction.mode =
        DeconflictionMode::NetworkProtocol {
            coordinator_id: AgentId::from("coordinator-0".to_owned()),
        };

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log =
        event_log.expect("urban network deconfliction failure run should produce replay log");

    assert!(metrics.success);
    assert_no_duplicate_segment_ownership(&event_log);
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::AgentFailed { agent_id, .. } if agent_id.as_ref() == "agent-0"
    )));
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::SwarmOwnershipReleased {
            agent_id,
            resource_id,
            reason,
            ..
        } if agent_id.as_ref() == "agent-0"
            && resource_id == "road-n0-n1"
            && reason == "agent_failed"
    )));
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::SwarmOwnershipHandoff {
            from_agent_id,
            to_agent_id,
            resource_id,
            reason,
            ..
        } if from_agent_id.as_ref() == "agent-0"
            && to_agent_id.as_ref() == "agent-1"
            && resource_id == "road-n0-n1"
            && reason == "agent_failed"
    )));
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanPatrolCompleted { agent_id, .. }
            if agent_id.as_ref() == "agent-1"
    )));
}

#[test]
fn corridor_failure_handoff_builds_connected_replacement_route() {
    let (scenario, mut run_config) = urban_corridor_deconfliction_test_run_config();
    let map = run_config
        .urban_state
        .as_ref()
        .expect("urban state should exist")
        .map
        .clone();
    run_config.failures = vec![FailureEvent {
        agent_id: AgentId::from("agent-0".to_owned()),
        at_tick: 2,
    }];

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("corridor failure run should produce replay log");

    assert!(metrics.success);
    assert_no_duplicate_segment_ownership(&event_log);
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::SwarmOwnershipHandoff {
            from_agent_id,
            to_agent_id,
            reason,
            ..
        } if from_agent_id.as_ref() == "agent-0"
            && to_agent_id.as_ref() == "agent-1"
            && reason == "agent_failed"
    )));
    let replacement_edge_ids = event_log
        .events
        .iter()
        .rev()
        .find_map(|event| match event {
            swarm_replay::Event::UrbanRoutePlanned {
                agent_id,
                tick: 2,
                edge_ids,
                ..
            } if agent_id.as_ref() == "agent-1" => Some(edge_ids.clone()),
            _ => None,
        })
        .expect("failure handoff should replan agent-1 route");
    assert!(
        replacement_edge_ids
            .iter()
            .any(|edge_id| edge_id.as_ref() == "corridor-c2-c1"),
        "replacement route should include bridge from survivor end to failed slice"
    );
    assert_route_edge_ids_connected(&map, &replacement_edge_ids);
}

#[test]
fn failure_before_segment_lock_does_not_activate_segment_handoff() {
    let (mut scenario, mut run_config) = urban_deconfliction_test_run_config(
        UrbanRightOfWayPolicy::FirstCome,
        UrbanBlockedPolicy::Wait,
    );
    scenario.agents[1].pose = Pose {
        x: 20.0,
        y: 20.0,
        ..Default::default()
    };
    run_config.max_ticks = 40;
    run_config.failures = vec![FailureEvent {
        agent_id: AgentId::from("agent-0".to_owned()),
        at_tick: 0,
    }];
    run_config.urban_state.as_mut().unwrap().deconfliction.mode =
        DeconflictionMode::NetworkProtocol {
            coordinator_id: AgentId::from("coordinator-0".to_owned()),
        };

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("failure-before-lock run should produce replay log");

    assert!(!metrics.success);
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::AgentFailed { agent_id, tick } if agent_id.as_ref() == "agent-0" && *tick == 0
    )));
    assert!(!event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::SwarmOwnershipReleased {
            reason,
            ..
        } if reason == "agent_failed"
    )));
    assert!(!event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::SwarmOwnershipHandoff { .. })));
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanDeconflictAbort { reason, .. }
            if reason == "failure handoff requires agent_failed ownership release"
    )));
}

fn assert_no_duplicate_segment_ownership(log: &swarm_replay::EventLog) {
    let mut active = std::collections::HashMap::<UrbanEdgeId, AgentId>::new();
    for event in &log.events {
        match event {
            swarm_replay::Event::UrbanSegmentLockAcquired {
                agent_id, edge_id, ..
            } => {
                if let Some(holder) = active.get(edge_id) {
                    assert_eq!(holder, agent_id, "duplicate holder for edge {edge_id}");
                } else {
                    active.insert(edge_id.clone(), agent_id.clone());
                }
            }
            swarm_replay::Event::UrbanSegmentLockReleased {
                agent_id, edge_id, ..
            } => {
                assert_eq!(
                    active.remove(edge_id).as_ref(),
                    Some(agent_id),
                    "release must match active holder for edge {edge_id}"
                );
            }
            _ => {}
        }
    }
    assert!(active.is_empty(), "all segment locks must be released");
}

fn assert_route_edge_ids_connected(map: &UrbanMap, edge_ids: &[UrbanEdgeId]) {
    let edges = edge_ids
        .iter()
        .map(|edge_id| {
            map.edges
                .iter()
                .find(|edge| &edge.id == edge_id)
                .unwrap_or_else(|| panic!("missing edge {edge_id}"))
        })
        .collect::<Vec<_>>();
    for pair in edges.windows(2) {
        assert_eq!(
            pair[0].to, pair[1].from,
            "route jump between {} and {}",
            pair[0].id, pair[1].id
        );
    }
}

fn urban_deconfliction_test_run_config(
    right_of_way_policy: UrbanRightOfWayPolicy,
    locked_segment_policy: UrbanBlockedPolicy,
) -> (Scenario, RunConfig) {
    let (mut scenario, mut run_config) = urban_test_run_config(80, vec![]);
    scenario.name = "urban_deconfliction_unit".to_owned();
    scenario.agents.push(Agent {
        id: AgentId::from("agent-1".to_owned()),
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
        speed: 2.0,
        max_range: 1000.0,
        battery_drain_rate: 0.0,
        battery_model: None,
    });
    let urban_state = run_config.urban_state.as_mut().unwrap();
    urban_state.deconfliction.enabled = true;
    urban_state.deconfliction.right_of_way_policy = right_of_way_policy;
    urban_state.deconfliction.locked_segment_policy = locked_segment_policy;
    urban_state
        .deconfliction
        .agent_priorities
        .insert(AgentId::from("agent-0".to_owned()), 1);
    urban_state
        .deconfliction
        .agent_priorities
        .insert(AgentId::from("agent-1".to_owned()), 9);
    (scenario, run_config)
}

fn urban_corridor_deconfliction_test_run_config() -> (Scenario, RunConfig) {
    let c0 = UrbanNodeId::from("c0".to_owned());
    let c1 = UrbanNodeId::from("c1".to_owned());
    let c2 = UrbanNodeId::from("c2".to_owned());
    let c3 = UrbanNodeId::from("c3".to_owned());
    let mut scenario = scenario(0, 3, 0);
    scenario.name = "urban_corridor_inspection_network_failure".to_owned();
    scenario.agents[0].speed = 2.5;
    scenario.agents[1].speed = 2.5;
    scenario.agents[1].pose = Pose {
        x: 50.0,
        y: 0.0,
        ..Default::default()
    };
    scenario.agents[2].speed = 2.5;
    scenario.agents[2].pose = Pose {
        x: 50.0,
        y: 0.0,
        ..Default::default()
    };
    let mut run_config = config(vec![]);
    run_config.max_ticks = 220;
    run_config.tick_duration_ms = 1000;
    run_config.urban_state = Some(UrbanState {
        map: UrbanMap {
            nodes: vec![
                UrbanNode {
                    id: c0.clone(),
                    pose: Pose {
                        x: 0.0,
                        y: 0.0,
                        ..Default::default()
                    },
                    geo: None,
                },
                UrbanNode {
                    id: c1.clone(),
                    pose: Pose {
                        x: 25.0,
                        y: 0.0,
                        ..Default::default()
                    },
                    geo: None,
                },
                UrbanNode {
                    id: c2.clone(),
                    pose: Pose {
                        x: 50.0,
                        y: 0.0,
                        ..Default::default()
                    },
                    geo: None,
                },
                UrbanNode {
                    id: c3.clone(),
                    pose: Pose {
                        x: 75.0,
                        y: 0.0,
                        ..Default::default()
                    },
                    geo: None,
                },
            ],
            edges: vec![
                UrbanEdge {
                    id: UrbanEdgeId::from("corridor-c0-c1".to_owned()),
                    from: c0.clone(),
                    to: c1.clone(),
                    cost: 25.0,
                    length_m: 25.0,
                    corridor_width_m: Some(5.0),
                    blocked: false,
                },
                UrbanEdge {
                    id: UrbanEdgeId::from("corridor-c1-c2".to_owned()),
                    from: c1.clone(),
                    to: c2.clone(),
                    cost: 25.0,
                    length_m: 25.0,
                    corridor_width_m: Some(5.0),
                    blocked: false,
                },
                UrbanEdge {
                    id: UrbanEdgeId::from("corridor-c2-c3".to_owned()),
                    from: c2.clone(),
                    to: c3.clone(),
                    cost: 25.0,
                    length_m: 25.0,
                    corridor_width_m: Some(5.0),
                    blocked: false,
                },
                UrbanEdge {
                    id: UrbanEdgeId::from("corridor-c3-c2".to_owned()),
                    from: c3.clone(),
                    to: c2.clone(),
                    cost: 25.0,
                    length_m: 25.0,
                    corridor_width_m: Some(5.0),
                    blocked: false,
                },
                UrbanEdge {
                    id: UrbanEdgeId::from("corridor-c2-c1".to_owned()),
                    from: c2.clone(),
                    to: c1.clone(),
                    cost: 25.0,
                    length_m: 25.0,
                    corridor_width_m: Some(5.0),
                    blocked: false,
                },
                UrbanEdge {
                    id: UrbanEdgeId::from("corridor-c1-c0".to_owned()),
                    from: c1.clone(),
                    to: c0.clone(),
                    cost: 25.0,
                    length_m: 25.0,
                    corridor_width_m: Some(5.0),
                    blocked: false,
                },
            ],
            static_obstacles: vec![],
        },
        route_loop: UrbanRouteLoop {
            nodes: vec![c0.clone(), c1.clone(), c2.clone(), c3, c2, c1, c0],
        },
        mission_template: None,
        start_node: Some(UrbanNodeId::from("c0".to_owned())),
        planner: "dijkstra".to_owned(),
        temporary_obstacles: vec![],
        blocked_route_policy: UrbanBlockedPolicy::Wait,
        deconfliction: Default::default(),
        perimeter_patrol: None,
    });
    let urban_state = run_config.urban_state.as_mut().unwrap();
    urban_state.deconfliction.enabled = true;
    urban_state.deconfliction.mode = DeconflictionMode::NetworkProtocol {
        coordinator_id: AgentId::from("coordinator-0".to_owned()),
    };
    urban_state.deconfliction.right_of_way_policy = UrbanRightOfWayPolicy::Priority;
    urban_state.deconfliction.locked_segment_policy = UrbanBlockedPolicy::Wait;
    urban_state
        .deconfliction
        .agent_priorities
        .insert(AgentId::from("agent-0".to_owned()), 3);
    urban_state
        .deconfliction
        .agent_priorities
        .insert(AgentId::from("agent-1".to_owned()), 2);
    urban_state
        .deconfliction
        .agent_priorities
        .insert(AgentId::from("agent-2".to_owned()), 1);
    (scenario, run_config)
}

#[test]
fn urban_blocked_policy_wait_emits_wait_decision() {
    let (scenario, mut run_config) = urban_test_run_config(60, vec![]);
    let urban_state = run_config.urban_state.as_mut().unwrap();
    urban_state.blocked_route_policy = UrbanBlockedPolicy::Wait;
    urban_state.temporary_obstacles = vec![UrbanTemporaryObstacle {
        edge_id: UrbanEdgeId::from("road-n0-n1".to_owned()),
        appears_at_tick: 0,
        disappears_at_tick: Some(3),
        reason: Some("temporary road closure".to_owned()),
        severity: None,
    }];

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban run should produce replay log");

    assert!(metrics.success);
    assert!(metrics.urban_wait_time_ticks > 0);
    assert_eq!(metrics.urban_unresolved_blockage_count, 0);
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanObstacleDetected { edge_id, .. }
            if edge_id.as_ref() == "road-n0-n1"
    )));
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanPolicyDecision { policy, .. } if policy == "wait"
    )));
    assert!(event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::UrbanWaitStarted { .. })));
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanWaitCompleted { waited_ticks, .. } if *waited_ticks > 0
    )));
}

#[test]
fn urban_blocked_policy_replan_uses_alternate_route() {
    let (scenario, mut run_config) = urban_test_run_config(80, vec![]);
    let urban_state = run_config.urban_state.as_mut().unwrap();
    urban_state.blocked_route_policy = UrbanBlockedPolicy::Replan;
    urban_state.temporary_obstacles = vec![UrbanTemporaryObstacle {
        edge_id: UrbanEdgeId::from("road-n0-n1".to_owned()),
        appears_at_tick: 0,
        disappears_at_tick: None,
        reason: Some("primary road blocked".to_owned()),
        severity: None,
    }];
    urban_state.map.edges.extend([
        UrbanEdge {
            id: UrbanEdgeId::from("detour-n0-n3".to_owned()),
            from: UrbanNodeId::from("n0".to_owned()),
            to: UrbanNodeId::from("n3".to_owned()),
            cost: 20.0,
            length_m: 20.0,
            corridor_width_m: Some(4.0),
            blocked: false,
        },
        UrbanEdge {
            id: UrbanEdgeId::from("detour-n3-n2".to_owned()),
            from: UrbanNodeId::from("n3".to_owned()),
            to: UrbanNodeId::from("n2".to_owned()),
            cost: 20.0,
            length_m: 20.0,
            corridor_width_m: Some(4.0),
            blocked: false,
        },
        UrbanEdge {
            id: UrbanEdgeId::from("detour-n2-n1".to_owned()),
            from: UrbanNodeId::from("n2".to_owned()),
            to: UrbanNodeId::from("n1".to_owned()),
            cost: 20.0,
            length_m: 20.0,
            corridor_width_m: Some(4.0),
            blocked: false,
        },
    ]);

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban run should produce replay log");

    assert!(metrics.success);
    assert!(metrics.urban_replan_count > 0);
    assert_eq!(metrics.urban_unresolved_blockage_count, 0);
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanPolicyDecision { policy, .. } if policy == "replan"
    )));
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanRouteReplanned { edge_ids, .. }
            if edge_ids.iter().any(|edge_id| edge_id.as_ref() == "detour-n0-n3")
                && !edge_ids.iter().any(|edge_id| edge_id.as_ref() == "road-n0-n1")
    )));
}

#[test]
fn blocked_route_recovery_produces_replacement_mission() {
    let (scenario, mut run_config) = urban_test_run_config(80, vec![]);
    let urban_state = run_config.urban_state.as_mut().unwrap();
    urban_state.blocked_route_policy = UrbanBlockedPolicy::Replan;
    urban_state.temporary_obstacles = vec![UrbanTemporaryObstacle {
        edge_id: UrbanEdgeId::from("road-n0-n1".to_owned()),
        appears_at_tick: 0,
        disappears_at_tick: None,
        reason: Some("primary road blocked".to_owned()),
        severity: None,
    }];
    urban_state.map.edges.extend([
        UrbanEdge {
            id: UrbanEdgeId::from("replacement-n0-n3".to_owned()),
            from: UrbanNodeId::from("n0".to_owned()),
            to: UrbanNodeId::from("n3".to_owned()),
            cost: 20.0,
            length_m: 20.0,
            corridor_width_m: Some(4.0),
            blocked: false,
        },
        UrbanEdge {
            id: UrbanEdgeId::from("replacement-n3-n2".to_owned()),
            from: UrbanNodeId::from("n3".to_owned()),
            to: UrbanNodeId::from("n2".to_owned()),
            cost: 20.0,
            length_m: 20.0,
            corridor_width_m: Some(4.0),
            blocked: false,
        },
        UrbanEdge {
            id: UrbanEdgeId::from("replacement-n2-n1".to_owned()),
            from: UrbanNodeId::from("n2".to_owned()),
            to: UrbanNodeId::from("n1".to_owned()),
            cost: 20.0,
            length_m: 20.0,
            corridor_width_m: Some(4.0),
            blocked: false,
        },
    ]);

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban run should produce replay log");

    assert!(metrics.success);
    assert!(metrics.urban_replan_count > 0);
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanRouteReplanned { edge_ids, .. }
            if edge_ids.iter().any(|edge_id| edge_id.as_ref() == "replacement-n0-n3")
                && !edge_ids.iter().any(|edge_id| edge_id.as_ref() == "road-n0-n1")
    )));
}

#[test]
fn urban_blocked_policy_abort_stops_run() {
    let (scenario, mut run_config) = urban_test_run_config(50, vec![]);
    let urban_state = run_config.urban_state.as_mut().unwrap();
    urban_state.blocked_route_policy = UrbanBlockedPolicy::Abort;
    urban_state.temporary_obstacles = vec![UrbanTemporaryObstacle {
        edge_id: UrbanEdgeId::from("road-n0-n1".to_owned()),
        appears_at_tick: 0,
        disappears_at_tick: None,
        reason: Some("hard closure".to_owned()),
        severity: None,
    }];

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &scenario,
        run_config,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("urban run should produce replay log");

    assert!(!metrics.success);
    assert!(!metrics.urban_patrol_completed);
    assert_eq!(metrics.urban_replan_count, 0);
    assert!(metrics.urban_unresolved_blockage_count > 0);
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::UrbanPolicyDecision { policy, .. } if policy == "abort"
    )));
    assert!(event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::UrbanNoRouteAvailable { .. })));
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

#[test]
fn existing_scenarios_load_without_drone_link_field() {
    // Verify that JSON without a "drone_link" key deserialises successfully
    // and defaults to DroneLinkConfig::Simulated.
    let json = r#"{"max_ticks": 100}"#;
    let config: RunConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.max_ticks, 100);
    assert_eq!(
        config.drone_link,
        DroneLinkConfig::Simulated,
        "drone_link must default to Simulated for backward compatibility"
    );
}

#[test]
fn existing_scenarios_load_without_autonomy_field() {
    // RunConfig without an "autonomy" key must deserialise and apply sane defaults.
    let json = r#"{"max_ticks": 50}"#;
    let config: RunConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.max_ticks, 50);
    assert_eq!(config.autonomy.gcs_heartbeat_timeout_ticks, 10);
    assert_eq!(config.autonomy.peer_heartbeat_timeout_ticks, 15);
}

// ── M93 integration tests ─────────────────────────────────────────────────────

/// Builds a minimal one-agent scenario suitable for autonomy FSM integration tests.
fn autonomy_scenario() -> Scenario {
    scenario(0, 1, 0)
}

fn two_agent_partition_scenario() -> Scenario {
    scenario(0, 2, 0)
}

fn three_agent_partition_scenario() -> Scenario {
    scenario(0, 3, 0)
}

/// Builds a RunConfig that partitions the default GCS ("base") from "agent-0"
/// starting at tick 1 and running until the simulation ends.
fn partition_config(extra_ticks: u64, autonomy: AgentAutonomyConfig) -> RunConfig {
    RunConfig {
        max_ticks: extra_ticks,
        partition_events: vec![PartitionEvent {
            at_tick: 1,
            until_tick: None,
            heal_at_tick: None,
            agents: (
                AgentId::from("base".to_owned()),
                AgentId::from("agent-0".to_owned()),
            ),
        }],
        autonomy,
        ..config(vec![])
    }
}

fn lease_record(
    lease_id: &str,
    resource_id: &str,
    granted_tick: u64,
    expiry_tick: u64,
) -> InitialAgentLeaseRecord {
    InitialAgentLeaseRecord {
        lease_id: lease_id.to_owned(),
        resource_id: resource_id.to_owned(),
        resource_kind: "task".to_owned(),
        granted_tick,
        expiry_tick,
    }
}

fn full_partition_events(agent_ids: &[&str], heal_at_tick: u64) -> Vec<PartitionEvent> {
    let mut events = Vec::new();
    for (index, agent_a) in agent_ids.iter().enumerate() {
        for agent_b in &agent_ids[index + 1..] {
            events.push(PartitionEvent {
                at_tick: 1,
                until_tick: None,
                heal_at_tick: Some(heal_at_tick),
                agents: (
                    AgentId::from((*agent_a).to_owned()),
                    AgentId::from((*agent_b).to_owned()),
                ),
            });
        }
    }
    events
}

#[test]
fn replay_contains_agent_gcs_lost() {
    let autonomy = AgentAutonomyConfig {
        gcs_lost_policy: GcsLostPolicy::ReturnToLaunch { after_ticks: 3 },
        ..AgentAutonomyConfig::default()
    };
    let (_, event_log) = ScenarioRunner::run_with_log(
        &autonomy_scenario(),
        partition_config(10, autonomy),
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("run_with_log should return an event log");
    assert!(
        event_log.events.iter().any(|event| matches!(
            event,
            swarm_replay::Event::AgentGcsLost { agent_id, .. }
                if agent_id.as_ref() == "agent-0"
        )),
        "replay log must contain AgentGcsLost for agent-0"
    );
}

#[test]
fn gcs_lost_count_metric_is_nonzero_after_partition() {
    let autonomy = AgentAutonomyConfig {
        gcs_lost_policy: GcsLostPolicy::ReturnToLaunch { after_ticks: 3 },
        ..AgentAutonomyConfig::default()
    };
    let (metrics, _) = ScenarioRunner::run_with_log(
        &autonomy_scenario(),
        partition_config(10, autonomy),
        swarm_alloc::GreedyAllocator::default(),
    );
    assert!(
        metrics.gcs_lost_count > 0,
        "gcs_lost_count should be non-zero after partition isolates the GCS"
    );
}

#[test]
fn replay_contains_agent_continuing_under_lease() {
    let autonomy = AgentAutonomyConfig {
        gcs_lost_policy: GcsLostPolicy::ReturnToLaunch { after_ticks: 3 },
        ..AgentAutonomyConfig::default()
    };
    let mut initial_leases = std::collections::HashMap::new();
    // Lease expires at tick 100 — well after the GCS loss at tick 3.
    initial_leases.insert(
        AgentId::from("agent-0".to_owned()),
        vec![("auto-lease-1".to_owned(), 100u64)],
    );
    let cfg = RunConfig {
        initial_agent_leases: initial_leases,
        ..partition_config(10, autonomy)
    };
    let (_, event_log) = ScenarioRunner::run_with_log(
        &autonomy_scenario(),
        cfg,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("run_with_log should return an event log");
    assert!(
        event_log.events.iter().any(|event| matches!(
            event,
            swarm_replay::Event::AgentContinuingUnderLease { agent_id, lease_id, .. }
                if agent_id.as_ref() == "agent-0" && lease_id == "auto-lease-1"
        )),
        "replay log must contain AgentContinuingUnderLease for agent-0 with auto-lease-1"
    );
}

// ── M94 integration tests ─────────────────────────────────────────────────────

#[test]
fn link_loss_does_not_mark_agent_dead_before_lease_expiry() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-link", "resource-link", 0, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 8,
        initial_agent_lease_records: initial_lease_records,
        ..partition_config(8, AgentAutonomyConfig::default())
    };

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &autonomy_scenario(),
        cfg,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("event log should be present");

    assert!(
        !event_log.events.iter().any(|event| matches!(
            event,
            swarm_replay::Event::AgentFailed { agent_id, .. } if agent_id.as_ref() == "agent-0"
        )),
        "partition-induced link loss must not mark the agent as failed"
    );
    assert!(
        metrics
            .degraded_decision_log
            .iter()
            .any(|entry| matches!(entry.absence_kind, Some(AgentAbsenceKind::LinkLoss { .. }))),
        "degraded decision log must record link loss, not node death"
    );
}

#[test]
fn isolated_agent_continues_under_valid_lease() {
    let autonomy = AgentAutonomyConfig {
        gcs_lost_policy: GcsLostPolicy::ReturnToLaunch { after_ticks: 3 },
        ..AgentAutonomyConfig::default()
    };
    let mut initial_agent_leases = std::collections::HashMap::new();
    initial_agent_leases.insert(
        AgentId::from("agent-0".to_owned()),
        vec![("lease-isolated".to_owned(), 100u64)],
    );
    let cfg = RunConfig {
        max_ticks: 8,
        initial_agent_leases,
        ..partition_config(8, autonomy)
    };

    let (_metrics, event_log) = ScenarioRunner::run_with_log(
        &autonomy_scenario(),
        cfg,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("event log should be present");

    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::AgentContinuingUnderLease { agent_id, lease_id, .. }
            if agent_id.as_ref() == "agent-0" && lease_id == "lease-isolated"
    )));
}

#[test]
fn node_failure_releases_resources_immediately() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-fail", "resource-fail", 0, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 5,
        failures: vec![FailureEvent {
            agent_id: AgentId::from("agent-0".to_owned()),
            at_tick: 2,
        }],
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let metrics = ScenarioRunner::run(&autonomy_scenario(), cfg);
    assert!(metrics.degraded_decision_log.iter().any(|entry| {
        matches!(entry.absence_kind, Some(AgentAbsenceKind::NodeFailure))
            && matches!(
                entry.decision,
                SupervisorDecision::ReleaseAfterTimeout { ticks: 0 }
            )
            && entry.affected_resources == vec!["resource-fail".to_owned()]
    }));
}

#[test]
fn partition_both_groups_continue_under_leases() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-a", "resource-a", 0, 100)],
    );
    initial_lease_records.insert(
        AgentId::from("agent-1".to_owned()),
        vec![lease_record("lease-b", "resource-b", 0, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 6,
        partition_events: vec![PartitionEvent {
            at_tick: 1,
            until_tick: None,
            heal_at_tick: Some(5),
            agents: (
                AgentId::from("agent-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            ),
        }],
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &two_agent_partition_scenario(),
        cfg,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("event log should be present");

    let continue_count = metrics
        .degraded_decision_log
        .iter()
        .filter(|entry| matches!(entry.decision, SupervisorDecision::ContinueUnderLease))
        .count();
    assert_eq!(
        continue_count, 2,
        "both isolated groups must continue under lease"
    );
    assert!(event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::PartitionDetected { .. })));
}

#[test]
fn older_lease_wins_conflict_resolution() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-old", "resource-shared", 0, 100)],
    );
    initial_lease_records.insert(
        AgentId::from("agent-1".to_owned()),
        vec![lease_record("lease-new", "resource-shared", 10, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 6,
        partition_events: vec![PartitionEvent {
            at_tick: 1,
            until_tick: None,
            heal_at_tick: Some(5),
            agents: (
                AgentId::from("agent-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            ),
        }],
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let (metrics, event_log) = ScenarioRunner::run_with_log(
        &two_agent_partition_scenario(),
        cfg,
        swarm_alloc::GreedyAllocator::default(),
    );
    let report = metrics
        .reconciliation_reports
        .first()
        .expect("reconciliation report should exist");
    let conflict = report
        .result
        .conflicts
        .first()
        .expect("ownership conflict should be recorded");
    assert!(matches!(
        conflict.resolution,
        ConflictResolution::OlderLeaseWins { ref winner } if winner.as_ref() == "agent-0"
    ));
    let event_log = event_log.expect("event log should be present");
    assert!(event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::SupervisorReconciled { .. })));
}

#[test]
fn older_lease_wins_conflict_resolution_with_three_contenders() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-middle", "resource-shared", 10, 100)],
    );
    initial_lease_records.insert(
        AgentId::from("agent-1".to_owned()),
        vec![lease_record("lease-oldest", "resource-shared", 0, 100)],
    );
    initial_lease_records.insert(
        AgentId::from("agent-2".to_owned()),
        vec![lease_record("lease-newest", "resource-shared", 20, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 6,
        partition_events: full_partition_events(&["agent-0", "agent-1", "agent-2"], 5),
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let metrics = ScenarioRunner::run(&three_agent_partition_scenario(), cfg);
    let report = metrics
        .reconciliation_reports
        .first()
        .expect("reconciliation report should exist");
    assert!(report.result.conflicts.iter().all(|conflict| matches!(
        conflict.resolution,
        ConflictResolution::OlderLeaseWins { ref winner } if winner.as_ref() == "agent-1"
    )));
}

#[test]
fn reconciliation_rejects_stale_lease_after_heal() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-stale", "resource-shared", 0, 4)],
    );
    initial_lease_records.insert(
        AgentId::from("agent-1".to_owned()),
        vec![lease_record("lease-valid", "resource-shared", 1, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 6,
        partition_events: vec![PartitionEvent {
            at_tick: 1,
            until_tick: None,
            heal_at_tick: Some(5),
            agents: (
                AgentId::from("agent-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            ),
        }],
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let metrics = ScenarioRunner::run(&two_agent_partition_scenario(), cfg);
    let report = metrics
        .reconciliation_reports
        .first()
        .expect("reconciliation report should exist");
    assert_eq!(report.result.accepted, vec!["resource-shared".to_owned()]);
    assert_eq!(report.result.rejected, vec!["resource-shared".to_owned()]);
}

#[test]
fn reconciliation_accepts_valid_lease_after_heal() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-valid", "resource-valid", 0, 100)],
    );
    initial_lease_records.insert(
        AgentId::from("agent-1".to_owned()),
        vec![lease_record("lease-stale", "resource-valid", 1, 4)],
    );
    let cfg = RunConfig {
        max_ticks: 6,
        partition_events: vec![PartitionEvent {
            at_tick: 1,
            until_tick: None,
            heal_at_tick: Some(5),
            agents: (
                AgentId::from("agent-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            ),
        }],
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let metrics = ScenarioRunner::run(&two_agent_partition_scenario(), cfg);
    let report = metrics
        .reconciliation_reports
        .first()
        .expect("reconciliation report should exist");
    assert_eq!(report.result.accepted, vec!["resource-valid".to_owned()]);
}

#[test]
fn command_suppressed_on_ambiguous_authority() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-a", "resource-shared", 0, 100)],
    );
    initial_lease_records.insert(
        AgentId::from("agent-1".to_owned()),
        vec![lease_record("lease-b", "resource-shared", 0, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 6,
        partition_events: vec![PartitionEvent {
            at_tick: 1,
            until_tick: None,
            heal_at_tick: Some(5),
            agents: (
                AgentId::from("agent-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            ),
        }],
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let (_, event_log) = ScenarioRunner::run_with_log(
        &two_agent_partition_scenario(),
        cfg,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("event log should be present");
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::CommandSuppressed { resource_id, .. }
            if resource_id == "resource-shared"
    )));
}

#[test]
fn supervisor_reset_on_unresolvable_conflict() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-a", "resource-reset", 0, 100)],
    );
    initial_lease_records.insert(
        AgentId::from("agent-1".to_owned()),
        vec![lease_record("lease-b", "resource-reset", 0, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 6,
        partition_events: vec![PartitionEvent {
            at_tick: 1,
            until_tick: None,
            heal_at_tick: Some(5),
            agents: (
                AgentId::from("agent-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            ),
        }],
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let metrics = ScenarioRunner::run(&two_agent_partition_scenario(), cfg);
    let report = metrics
        .reconciliation_reports
        .first()
        .expect("reconciliation report should exist");
    assert!(report
        .result
        .conflicts
        .iter()
        .all(|conflict| matches!(conflict.resolution, ConflictResolution::SupervisorReset)));
}

#[test]
fn partition_report_in_replay() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-a", "resource-a", 0, 100)],
    );
    initial_lease_records.insert(
        AgentId::from("agent-1".to_owned()),
        vec![lease_record("lease-b", "resource-b", 0, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 6,
        partition_events: vec![PartitionEvent {
            at_tick: 1,
            until_tick: None,
            heal_at_tick: Some(5),
            agents: (
                AgentId::from("agent-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            ),
        }],
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let (_, event_log) = ScenarioRunner::run_with_log(
        &two_agent_partition_scenario(),
        cfg,
        swarm_alloc::GreedyAllocator::default(),
    );
    let event_log = event_log.expect("event log should be present");
    assert!(event_log.events.iter().any(|event| matches!(
        event,
        swarm_replay::Event::PartitionDetected { group_a, group_b, .. }
            if !group_a.is_empty() && !group_b.is_empty()
    )));
    assert!(event_log
        .events
        .iter()
        .any(|event| matches!(event, swarm_replay::Event::PartitionHealed { .. })));
}

#[test]
fn reconciliation_report_in_artifact() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-old", "resource-artifact", 0, 100)],
    );
    initial_lease_records.insert(
        AgentId::from("agent-1".to_owned()),
        vec![lease_record("lease-new", "resource-artifact", 5, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 6,
        partition_events: vec![PartitionEvent {
            at_tick: 1,
            until_tick: None,
            heal_at_tick: Some(5),
            agents: (
                AgentId::from("agent-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            ),
        }],
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let metrics = ScenarioRunner::run(&two_agent_partition_scenario(), cfg);
    assert_eq!(metrics.reconciliation_reports.len(), 1);
    assert_eq!(metrics.partition_reports.len(), 1);
}

#[test]
fn no_silent_task_disappearance_invariant() {
    let mut initial_lease_records = std::collections::HashMap::new();
    initial_lease_records.insert(
        AgentId::from("agent-0".to_owned()),
        vec![lease_record("lease-fail", "resource-visible", 0, 100)],
    );
    let cfg = RunConfig {
        max_ticks: 5,
        failures: vec![FailureEvent {
            agent_id: AgentId::from("agent-0".to_owned()),
            at_tick: 2,
        }],
        initial_agent_lease_records: initial_lease_records,
        ..config(vec![])
    };

    let metrics = ScenarioRunner::run(&autonomy_scenario(), cfg);
    let released_resources = metrics
        .degraded_decision_log
        .iter()
        .flat_map(|entry| entry.affected_resources.iter())
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        released_resources.contains(&"resource-visible".to_owned()),
        "every released resource must be named in degraded_decision_log"
    );
}
