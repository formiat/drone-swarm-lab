use super::*;
use crate::runner::UrbanState;
use crate::scenario::Scenario;
use crate::RunConfig;
use swarm_types::{
    Agent, Health, Pose, Role, Task, TaskKind, TaskStatus, UrbanBus, UrbanBusId,
    UrbanDetectorConfig, UrbanEdge, UrbanEdgeId, UrbanMap, UrbanNode, UrbanNodeId, UrbanRouteLoop,
    UrbanSearchState,
};

fn make_minimal_entry() -> ScenarioSuiteEntry {
    ScenarioSuiteEntry {
        mission: "coverage".to_owned(),
        profile: "ideal".to_owned(),
        scenario: Scenario {
            name: "test".to_owned(),
            seed: 0,
            agents: vec![Agent {
                id: swarm_types::AgentId::from("agent-0".to_owned()),
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
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
                battery_model: None,
            }],
            tasks: vec![Task {
                id: swarm_types::TaskId::from("task-0".to_owned()),
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
            }],
            ground_nodes: vec![],
            base_station: None,
        },
        run_config: RunConfig {
            max_ticks: 50,
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
            enable_movement: false,
            tick_duration_ms: 100,
            grid_state: None,
            enable_cbba: false,
            ..Default::default()
        },
    }
}

fn make_urban_entry() -> ScenarioSuiteEntry {
    let mut entry = make_minimal_entry();
    entry.mission = "urban-patrol".to_owned();
    entry.profile = "patrol-small-block".to_owned();
    entry.scenario.name = "urban_patrol_small_block".to_owned();
    entry.scenario.tasks[0].kind = Some(TaskKind::Waypoint);
    entry.scenario.tasks[0].pose = Some(Pose {
        x: 10.0,
        y: 0.0,
        ..Default::default()
    });
    entry.run_config.urban_state = Some(UrbanState {
        map: UrbanMap {
            nodes: vec![
                UrbanNode {
                    id: UrbanNodeId::from("n0".to_owned()),
                    pose: Pose {
                        x: 0.0,
                        y: 0.0,
                        ..Default::default()
                    },
                },
                UrbanNode {
                    id: UrbanNodeId::from("n1".to_owned()),
                    pose: Pose {
                        x: 10.0,
                        y: 0.0,
                        ..Default::default()
                    },
                },
            ],
            edges: vec![
                UrbanEdge {
                    id: UrbanEdgeId::from("e01".to_owned()),
                    from: UrbanNodeId::from("n0".to_owned()),
                    to: UrbanNodeId::from("n1".to_owned()),
                    cost: 10.0,
                    length_m: 10.0,
                    corridor_width_m: Some(4.0),
                    blocked: false,
                },
                UrbanEdge {
                    id: UrbanEdgeId::from("e10".to_owned()),
                    from: UrbanNodeId::from("n1".to_owned()),
                    to: UrbanNodeId::from("n0".to_owned()),
                    cost: 10.0,
                    length_m: 10.0,
                    corridor_width_m: Some(4.0),
                    blocked: false,
                },
            ],
            static_obstacles: vec![],
        },
        route_loop: UrbanRouteLoop {
            nodes: vec![
                UrbanNodeId::from("n0".to_owned()),
                UrbanNodeId::from("n1".to_owned()),
                UrbanNodeId::from("n0".to_owned()),
            ],
        },
        start_node: Some(UrbanNodeId::from("n0".to_owned())),
        planner: "dijkstra".to_owned(),
    });
    entry
}

fn make_urban_search_entry() -> ScenarioSuiteEntry {
    let mut entry = make_urban_entry();
    entry.mission = "urban-search".to_owned();
    entry.profile = "search-static-bus".to_owned();
    entry.scenario.name = "urban_search_static_bus".to_owned();
    entry.run_config.urban_search_state = Some(UrbanSearchState {
        buses: vec![UrbanBus {
            id: UrbanBusId::from("bus-0".to_owned()),
            pose: Pose {
                x: 5.0,
                y: 0.0,
                ..Default::default()
            },
            active_from_tick: None,
            active_until_tick: None,
        }],
        detector: UrbanDetectorConfig {
            detection_range_m: 2.0,
            detection_probability: 1.0,
            false_positive_rate: 0.0,
            seed: 11,
        },
    });
    entry
}

#[test]
fn scenario_suite_entry_json_roundtrip() {
    let entry = make_minimal_entry();
    let json = serde_json::to_string_pretty(&entry).unwrap();
    let parsed: ScenarioSuiteEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.mission, "coverage");
    assert_eq!(parsed.profile, "ideal");
    assert_eq!(parsed.scenario.name, "test");
    assert_eq!(parsed.run_config.max_ticks, 50);
}

#[test]
fn scenario_suite_entry_json_contains_mission_and_profile() {
    let entry = make_minimal_entry();
    let json = serde_json::to_string_pretty(&entry).unwrap();
    assert!(json.contains("\"mission\""));
    assert!(json.contains("\"profile\""));
    assert!(json.contains("\"coverage\""));
    assert!(json.contains("\"ideal\""));
}

#[test]
fn scenario_suite_load_from_file() {
    let suite = ScenarioSuite {
        schema_version: "0.1".to_owned(),
        name: "Test Suite".to_owned(),
        description: "A test suite".to_owned(),
        scenarios: vec![make_minimal_entry()],
    };
    let json = serde_json::to_string_pretty(&suite).unwrap();
    let tmp_file = tempfile::NamedTempFile::new().unwrap();
    let tmp = tmp_file.path().to_str().unwrap();
    std::fs::write(tmp, &json).unwrap();
    let loaded = load_scenario_suite(tmp).unwrap();
    assert_eq!(loaded.name, "Test Suite");
    assert_eq!(loaded.scenarios.len(), 1);
    assert_eq!(loaded.scenarios[0].mission, "coverage");
}

#[test]
fn scenario_json_roundtrip() {
    let entry = make_minimal_entry();
    let json = serde_json::to_string_pretty(&entry.scenario).unwrap();
    let parsed: Scenario = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "test");
    assert_eq!(parsed.seed, 0);
    assert_eq!(parsed.agents.len(), 1);
    assert_eq!(parsed.tasks.len(), 1);
}

#[test]
fn run_config_json_roundtrip() {
    let entry = make_minimal_entry();
    let json = serde_json::to_string_pretty(&entry.run_config).unwrap();
    let parsed: RunConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.max_ticks, 50);
    assert_eq!(parsed.timeout_ticks, 3);
    assert_eq!(parsed.max_unassigned_ticks, 10);
    assert!(parsed.failures.is_empty());
}

#[test]
fn run_config_json_defaults_work() {
    let json = r#"{"max_ticks": 30}"#;
    let parsed: RunConfig = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.max_ticks, 30);
    assert_eq!(parsed.timeout_ticks, 0);
    assert_eq!(parsed.max_unassigned_ticks, 10);
    assert_eq!(parsed.gossip_interval_ticks, 999);
    assert_eq!(parsed.tick_duration_ms, 100);
    assert!(!parsed.enable_cbba);
}

#[test]
fn scenario_suite_entry_integration_export() {
    let entry = make_minimal_entry();
    let json = export_entry(&entry).unwrap();
    assert!(!json.is_empty());
    let suite = ScenarioSuite {
        schema_version: "0.1".to_owned(),
        name: "Export Suite".to_owned(),
        description: "Suite for export test".to_owned(),
        scenarios: vec![entry],
    };
    let suite_json = export_suite(&suite).unwrap();
    assert!(suite_json.contains("Export Suite"));
}

#[test]
fn load_coverage_example_scenario() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scenarios/coverage.ideal.json"
    );
    let suite = load_scenario_suite(path).unwrap();
    assert_eq!(suite.name, "Coverage Quick Bench");
    assert_eq!(suite.scenarios.len(), 1);
    let entry = &suite.scenarios[0];
    assert_eq!(entry.mission, "coverage");
    assert_eq!(entry.profile, "ideal-no-failures");
    assert_eq!(entry.scenario.agents.len(), 5);
    assert_eq!(entry.scenario.tasks.len(), 3);
}

#[test]
fn load_emergency_mesh_example_scenario() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scenarios/emergency-mesh.ideal.json"
    );
    let suite = load_scenario_suite(path).unwrap();
    assert_eq!(suite.name, "Emergency Mesh Quick Bench");
    let entry = &suite.scenarios[0];
    assert_eq!(entry.mission, "emergency-mesh");
    assert_eq!(entry.profile, "ideal");
    assert_eq!(entry.scenario.ground_nodes.len(), 1);
    assert_eq!(
        entry.run_config.base_id,
        Some(swarm_types::AgentId::from("base".to_owned()))
    );
}

#[test]
fn load_sar_example_scenario() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scenarios/sar.ideal.json"
    );
    let suite = load_scenario_suite(path).unwrap();
    assert_eq!(suite.name, "SAR Quick Bench");
    let entry = &suite.scenarios[0];
    assert_eq!(entry.mission, "sar");
    assert!(entry.run_config.enable_movement);
    assert!(entry.run_config.grid_state.is_some());
    let gs = entry.run_config.grid_state.as_ref().unwrap();
    assert_eq!(gs.targets.len(), 2);
    assert_eq!(gs.grid.width, 6);
    assert_eq!(gs.grid.height, 6);
}

#[test]
fn schema_version_defaults_to_0_1() {
    let json = r#"{"name":"Test","description":"D","scenarios":[]}"#;
    let suite: ScenarioSuite = serde_json::from_str(json).unwrap();
    assert_eq!(suite.schema_version, "0.1");
}

#[test]
fn validate_rejects_empty_mission() {
    let mut entry = make_minimal_entry();
    entry.mission = "".to_owned();
    let errors = validate_entry(&entry);
    assert!(errors.iter().any(|e| e.field == "mission"));
}

#[test]
fn validate_rejects_empty_profile() {
    let mut entry = make_minimal_entry();
    entry.profile = "".to_owned();
    let errors = validate_entry(&entry);
    assert!(errors.iter().any(|e| e.field == "profile"));
}

#[test]
fn validate_rejects_no_agents() {
    let mut entry = make_minimal_entry();
    entry.scenario.agents.clear();
    let errors = validate_entry(&entry);
    assert!(errors.iter().any(|e| e.field == "scenario.agents"));
}

#[test]
fn validate_rejects_zero_max_ticks() {
    let mut entry = make_minimal_entry();
    entry.run_config.max_ticks = 0;
    let errors = validate_entry(&entry);
    assert!(errors.iter().any(|e| e.field == "run_config.max_ticks"));
}

#[test]
fn validate_sar_rejects_no_grid_state() {
    let mut entry = make_minimal_entry();
    entry.mission = "sar".to_owned();
    entry.run_config.grid_state = None;
    let errors = validate_entry(&entry);
    assert!(errors.iter().any(|e| e.field == "run_config.grid_state"));
}

#[test]
fn validate_sar_rejects_non_sar_task_kind() {
    let mut entry = make_minimal_entry();
    entry.mission = "sar".to_owned();
    entry.run_config.grid_state = Some(swarm_runtime::GridState::new(
        swarm_types::SearchGrid::new(1, 1, 1.0),
        vec![],
        swarm_types::SensorModel::new(1.0, 1.0, 1.0),
    ));
    entry.scenario.tasks[0].kind = Some(swarm_types::TaskKind::Waypoint);
    entry.scenario.tasks[0].pose = Some(Pose::default());
    entry.scenario.tasks[0].grid_cell = Some((0, 0));

    let errors = validate_entry(&entry);
    assert!(
        errors.iter().any(|e| e.field == "scenario.tasks[0].kind"),
        "Expected SAR task-kind mismatch, got: {errors:?}"
    );
}

#[test]
fn validate_inspection_rejects_non_inspection_task_kind() {
    let mut entry = make_minimal_entry();
    entry.mission = "inspection".to_owned();
    entry.run_config.enable_movement = true;
    entry.scenario.tasks[0].kind = Some(swarm_types::TaskKind::CoverageCell);
    entry.scenario.tasks[0].pose = Some(Pose::default());
    entry.scenario.tasks[0].edge_id = Some(swarm_types::EdgeId::from("edge-0".to_owned()));

    let errors = validate_entry(&entry);
    assert!(
        errors.iter().any(|e| e.field == "scenario.tasks[0].kind"),
        "Expected inspection task-kind mismatch, got: {errors:?}"
    );
}

#[test]
fn validate_emergency_mesh_allows_coverage_and_relay_kinds() {
    let mut entry = make_minimal_entry();
    entry.mission = "emergency-mesh".to_owned();
    entry.scenario.tasks[0].kind = Some(swarm_types::TaskKind::CoverageCell);
    entry.scenario.tasks[0].pose = Some(Pose::default());

    let mut relay = entry.scenario.tasks[0].clone();
    relay.id = swarm_types::TaskId::from("relay-0".to_owned());
    relay.kind = Some(swarm_types::TaskKind::RelayPlacement);
    relay.required_role = Some(Role::Relay);
    entry.scenario.tasks.push(relay);

    let errors = validate_entry(&entry);
    assert!(
        errors
            .iter()
            .all(|e| e.field != "scenario.tasks[0].kind" && e.field != "scenario.tasks[1].kind"),
        "Emergency mesh should allow coverage + relay task kinds, got: {errors:?}"
    );
}

#[test]
fn validate_urban_patrol_accepts_valid_entry() {
    let errors = validate_entry(&make_urban_entry());
    assert!(
        errors.is_empty(),
        "Expected valid urban entry, got: {errors:?}"
    );
}

#[test]
fn validate_urban_patrol_rejects_missing_urban_state() {
    let mut entry = make_urban_entry();
    entry.run_config.urban_state = None;
    let errors = validate_entry(&entry);
    assert!(errors
        .iter()
        .any(|error| error.field == "run_config.urban_state"));
}

#[test]
fn validate_urban_patrol_rejects_unknown_route_node() {
    let mut entry = make_urban_entry();
    entry
        .run_config
        .urban_state
        .as_mut()
        .unwrap()
        .route_loop
        .nodes
        .push(UrbanNodeId::from("missing".to_owned()));
    let errors = validate_entry(&entry);
    assert!(errors
        .iter()
        .any(|error| error.field == "run_config.urban_state.route_loop.nodes[3]"));
}

#[test]
fn validate_urban_patrol_rejects_start_node_mismatch() {
    let mut entry = make_urban_entry();
    entry.run_config.urban_state.as_mut().unwrap().start_node =
        Some(UrbanNodeId::from("n1".to_owned()));

    let errors = validate_entry(&entry);

    assert!(errors
        .iter()
        .any(|error| error.field == "run_config.urban_state.start_node"));
}

#[test]
fn validate_urban_patrol_rejects_agent_pose_away_from_start_node() {
    let mut entry = make_urban_entry();
    entry.scenario.agents[0].pose = Pose {
        x: 5.0,
        y: 0.0,
        ..Default::default()
    };

    let errors = validate_entry(&entry);

    assert!(errors
        .iter()
        .any(|error| error.field == "scenario.agents[0].pose"));
}

#[test]
fn validate_urban_patrol_rejects_unknown_edge_endpoint() {
    let mut entry = make_urban_entry();
    entry.run_config.urban_state.as_mut().unwrap().map.edges[0].to =
        UrbanNodeId::from("missing".to_owned());
    let errors = validate_entry(&entry);
    assert!(errors
        .iter()
        .any(|error| error.field == "run_config.urban_state.map.edges[0].to"));
}

#[test]
fn validate_urban_search_accepts_valid_entry() {
    let errors = validate_entry(&make_urban_search_entry());
    assert!(
        errors.is_empty(),
        "Expected valid urban search entry, got: {errors:?}"
    );
}

#[test]
fn validate_urban_search_rejects_missing_urban_state() {
    let mut entry = make_urban_search_entry();
    entry.run_config.urban_state = None;

    let errors = validate_entry(&entry);

    assert!(errors
        .iter()
        .any(|error| error.field == "run_config.urban_state"));
}

#[test]
fn validate_urban_search_rejects_missing_search_state() {
    let mut entry = make_urban_search_entry();
    entry.run_config.urban_search_state = None;

    let errors = validate_entry(&entry);

    assert!(errors
        .iter()
        .any(|error| error.field == "run_config.urban_search_state"));
}

#[test]
fn validate_urban_search_rejects_invalid_detector() {
    let mut entry = make_urban_search_entry();
    entry
        .run_config
        .urban_search_state
        .as_mut()
        .unwrap()
        .detector
        .detection_probability = 2.0;

    let errors = validate_entry(&entry);

    assert!(errors.iter().any(|error| {
        error.field == "run_config.urban_search_state.detector.detection_probability"
    }));
}

#[test]
fn validate_urban_search_rejects_invalid_bus() {
    let mut entry = make_urban_search_entry();
    entry.run_config.urban_search_state.as_mut().unwrap().buses[0]
        .pose
        .x = f64::NAN;

    let errors = validate_entry(&entry);

    assert!(errors
        .iter()
        .any(|error| error.field == "run_config.urban_search_state.buses[0].pose"));
}

#[test]
fn validate_accepts_valid_entry() {
    let entry = make_minimal_entry();
    let errors = validate_entry(&entry);
    assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
}

#[test]
fn validate_suite_rejects_empty_name() {
    let suite = ScenarioSuite {
        schema_version: "0.1".to_owned(),
        name: "".to_owned(),
        description: "test".to_owned(),
        scenarios: vec![make_minimal_entry()],
    };
    let errors = validate_scenario_suite(&suite);
    assert!(errors.iter().any(|e| e.field == "name"));
}

#[test]
fn validate_suite_rejects_unsupported_version() {
    let suite = ScenarioSuite {
        schema_version: "0.9".to_owned(),
        name: "Test".to_owned(),
        description: "test".to_owned(),
        scenarios: vec![make_minimal_entry()],
    };
    let errors = validate_scenario_suite(&suite);
    assert!(errors.iter().any(|e| e.field == "schema_version"));
}

#[test]
fn validate_suite_accepts_valid() {
    let suite = ScenarioSuite {
        schema_version: "0.1".to_owned(),
        name: "Test".to_owned(),
        description: "test".to_owned(),
        scenarios: vec![make_minimal_entry()],
    };
    let errors = validate_scenario_suite(&suite);
    assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
}
