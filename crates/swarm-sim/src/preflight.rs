use std::collections::HashSet;

use swarm_safety::preflight::{SafetyValidationReport, SafetyViolation, ViolationSeverity};
use swarm_safety::SafetyConfig;
use swarm_types::{Pose, TaskStatus, UrbanMap};

use crate::dsl::ScenarioSuiteEntry;

pub fn run_preflight(entry: &ScenarioSuiteEntry) -> SafetyValidationReport {
    let mut violations = Vec::new();
    violations.extend(check_mission_level(entry));
    violations.extend(check_ownership_invariants(entry));
    violations.extend(check_urban_safety(entry));
    violations.extend(check_mission_semantics(entry));
    SafetyValidationReport::from_violations(violations)
}

fn check_mission_level(entry: &ScenarioSuiteEntry) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();
    let config = entry.run_config.safety_config.as_ref();

    for task in &entry.scenario.tasks {
        let task_id = task.id.to_string();
        if task_id.trim().is_empty() {
            violations.push(error(
                "id.missing_task_id",
                Some(task_id.clone()),
                "task id must not be empty",
            ));
        }

        let Some(pose) = task.pose else {
            continue;
        };
        if !pose.x.is_finite() || !pose.y.is_finite() || !pose.z.is_finite() {
            violations.push(error(
                "pose.invalid_coordinate",
                Some(task_id.clone()),
                "task pose coordinates must be finite",
            ));
            continue;
        }

        if let Some(config) = config {
            check_pose_safety_config(config, &task_id, pose, &mut violations);
        }
    }

    if let Some(config) = config {
        check_route_limits(entry, config, &mut violations);
    }

    violations
}

fn check_pose_safety_config(
    config: &SafetyConfig,
    task_id: &str,
    pose: Pose,
    violations: &mut Vec<SafetyViolation>,
) {
    if let Some(geofence) = &config.geofence {
        if !geofence.bounds.contains(&pose) {
            violations.push(error(
                "geofence.waypoint_outside",
                Some(task_id.to_owned()),
                "task waypoint is outside configured geofence",
            ));
        }
    }
    for no_fly_zone in &config.no_fly_zones {
        if no_fly_zone.is_active_at(0) && no_fly_zone.bounds.contains(&pose) {
            violations.push(error(
                "nofly.waypoint_inside",
                Some(task_id.to_owned()),
                "task waypoint is inside an active no-fly zone",
            ));
        }
    }
    if let Some(max_altitude_m) = config.max_altitude_m {
        if pose.z > max_altitude_m {
            violations.push(error(
                "altitude.above_max",
                Some(task_id.to_owned()),
                format!(
                    "task altitude {}m exceeds max_altitude_m {max_altitude_m}m",
                    pose.z
                ),
            ));
        }
    }
    if let Some(min_altitude_m) = config.min_altitude_m {
        if pose.z < min_altitude_m {
            violations.push(warning(
                "altitude.below_min",
                Some(task_id.to_owned()),
                format!(
                    "task altitude {}m is below min_altitude_m {min_altitude_m}m",
                    pose.z
                ),
            ));
        }
    }
}

fn check_route_limits(
    entry: &ScenarioSuiteEntry,
    config: &SafetyConfig,
    violations: &mut Vec<SafetyViolation>,
) {
    if let (Some(max_route_length_m), Some(urban_state)) = (
        config.max_route_length_m,
        entry.run_config.urban_state.as_ref(),
    ) {
        if let Ok(route) = crate::urban::expand_route_loop_with_planner_name(
            &urban_state.map,
            &urban_state.route_loop,
            &urban_state.planner,
        ) {
            if route.total_length_m > max_route_length_m {
                violations.push(error(
                    "route.length_exceeds_max",
                    None,
                    format!(
                        "route length {}m exceeds max_route_length_m {max_route_length_m}m",
                        route.total_length_m
                    ),
                ));
            }
        }
    }

    if let Some(max_duration_ticks) = config.max_duration_ticks {
        let planned_duration_ms = entry
            .run_config
            .max_ticks
            .saturating_mul(entry.run_config.tick_duration_ms);
        let max_duration_ms = max_duration_ticks.saturating_mul(1000);
        if planned_duration_ms > max_duration_ms {
            violations.push(warning(
                "route.duration_exceeds_max",
                None,
                format!(
                    "planned duration {planned_duration_ms}ms exceeds max_duration_ticks {max_duration_ticks}s ({max_duration_ms}ms)"
                ),
            ));
        }
    }
}

fn check_ownership_invariants(entry: &ScenarioSuiteEntry) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();
    let mut task_ids = HashSet::new();
    for task in &entry.scenario.tasks {
        let task_id = task.id.to_string();
        if !task_ids.insert(task_id.clone()) {
            violations.push(error(
                "ownership.duplicate_task_id",
                Some(task_id.clone()),
                "task id must be unique within scenario",
            ));
        }
        if matches!(task.status, TaskStatus::Unassigned) && task.assigned_to.is_some() {
            violations.push(error(
                "ownership.task_assigned_and_unassigned",
                Some(task_id),
                "task cannot be Unassigned while assigned_to is set",
            ));
        }
    }
    violations
}

fn check_urban_safety(entry: &ScenarioSuiteEntry) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();
    let Some(urban_state) = entry.run_config.urban_state.as_ref() else {
        return violations;
    };
    for obstacle_error in urban_state
        .map
        .validate_temporary_obstacles(&urban_state.temporary_obstacles)
    {
        violations.push(error(
            "urban.invalid_temporary_obstacle",
            None,
            obstacle_error.to_string(),
        ));
    }
    violations.extend(check_route_loop_blocked_edges(
        &urban_state.map,
        &urban_state.route_loop,
    ));
    let Ok(route) = crate::urban::expand_route_loop_with_planner_name(
        &urban_state.map,
        &urban_state.route_loop,
        &urban_state.planner,
    ) else {
        return violations;
    };
    let nominal_bounds = nominal_map_bounds(&urban_state.map);

    for segment in &route.segments {
        let Some(edge) = urban_state.map.edge(&segment.edge_id) else {
            violations.push(error(
                "urban.unknown_edge",
                Some(segment.edge_id.to_string()),
                "planned route references an unknown edge",
            ));
            continue;
        };
        if edge.blocked {
            violations.push(error(
                "urban.blocked_edge",
                Some(edge.id.to_string()),
                "planned route uses a blocked urban edge",
            ));
        }
        for node_id in [&segment.from, &segment.to] {
            let Some(node) = urban_state.map.node(node_id) else {
                continue;
            };
            for obstacle in &urban_state.map.static_obstacles {
                if contains_pose(&obstacle.bounds, &node.pose) {
                    violations.push(error(
                        "urban.aabb_intersection",
                        Some(obstacle.id.to_string()),
                        format!(
                            "route waypoint '{}' intersects static obstacle '{}'",
                            node_id, obstacle.id
                        ),
                    ));
                }
            }
            if let Some(bounds) = nominal_bounds {
                if !contains_pose(&bounds, &node.pose) {
                    violations.push(warning(
                        "urban.waypoint_outside_assumptions",
                        Some(node_id.to_string()),
                        "route waypoint is outside nominal map bounds",
                    ));
                }
            }
        }
    }

    violations
}

fn check_route_loop_blocked_edges(
    map: &UrbanMap,
    route_loop: &swarm_types::UrbanRouteLoop,
) -> Vec<SafetyViolation> {
    let mut loop_nodes = route_loop.nodes.clone();
    if loop_nodes.first() != loop_nodes.last() {
        if let Some(first) = loop_nodes.first().cloned() {
            loop_nodes.push(first);
        }
    }
    let mut violations = Vec::new();
    for pair in loop_nodes.windows(2) {
        for edge in map
            .edges
            .iter()
            .filter(|edge| edge.from == pair[0] && edge.to == pair[1] && edge.blocked)
        {
            violations.push(error(
                "urban.blocked_edge",
                Some(edge.id.to_string()),
                "route loop directly references a blocked urban edge",
            ));
        }
    }
    violations
}

fn check_mission_semantics(entry: &ScenarioSuiteEntry) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();
    let strategy = entry.run_config.strategy_name.as_deref();
    let cbba_requested = entry.run_config.enable_cbba || strategy == Some("cbba");
    if cbba_requested && entry.mission != "cbba-stress" {
        violations.push(warning(
            "semantics.unsupported_strategy_pair",
            Some(entry.mission.clone()),
            format!(
                "CBBA strategy is only statically supported by preflight for cbba-stress, not '{}'",
                entry.mission
            ),
        ));
    }
    if entry.mission == "sar" && cbba_requested {
        violations.push(warning(
            "semantics.unsupported_strategy_pair",
            Some("sar+cbba".to_owned()),
            "SAR + CBBA remains unsupported by the current support matrix",
        ));
    }
    violations
}

fn nominal_map_bounds(map: &UrbanMap) -> Option<swarm_types::Aabb> {
    let mut nodes = map.nodes.iter();
    let first = nodes.next()?;
    let mut min_x = first.pose.x;
    let mut max_x = first.pose.x;
    let mut min_y = first.pose.y;
    let mut max_y = first.pose.y;
    for node in nodes {
        min_x = min_x.min(node.pose.x);
        max_x = max_x.max(node.pose.x);
        min_y = min_y.min(node.pose.y);
        max_y = max_y.max(node.pose.y);
    }
    Some(swarm_types::Aabb {
        min_x,
        max_x,
        min_y,
        max_y,
    })
}

fn contains_pose(bounds: &swarm_types::Aabb, pose: &Pose) -> bool {
    pose.x >= bounds.min_x
        && pose.x <= bounds.max_x
        && pose.y >= bounds.min_y
        && pose.y <= bounds.max_y
}

fn error(
    rule_id: impl Into<String>,
    affected_id: Option<String>,
    reason: impl Into<String>,
) -> SafetyViolation {
    SafetyViolation {
        rule_id: rule_id.into(),
        severity: ViolationSeverity::Error,
        affected_id,
        reason: reason.into(),
    }
}

fn warning(
    rule_id: impl Into<String>,
    affected_id: Option<String>,
    reason: impl Into<String>,
) -> SafetyViolation {
    SafetyViolation {
        rule_id: rule_id.into(),
        severity: ViolationSeverity::Warning,
        affected_id,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_safety::{Aabb, Geofence, NoFlyZone, SafetyConfig};
    use swarm_types::{
        Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskStatus, UrbanEdge, UrbanEdgeId,
        UrbanMap, UrbanNode, UrbanNodeId, UrbanObstacleId, UrbanRouteLoop, UrbanStaticObstacle,
    };

    fn agent() -> Agent {
        Agent {
            id: AgentId::from("agent-0".to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose::default(),
            capabilities: vec![],
            current_task: None,
            battery: 100.0,
            comms_range: 1000.0,
            generation: 1,
            speed: 1.0,
            max_range: 1000.0,
            battery_drain_rate: 0.0,
            battery_model: None,
        }
    }

    fn task(id: &str, pose: Pose) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(pose),
            grid_cell: None,
            edge_id: None,
            kind: None,
        }
    }

    fn entry(tasks: Vec<Task>) -> ScenarioSuiteEntry {
        ScenarioSuiteEntry {
            mission: "sitl".to_owned(),
            profile: "unit".to_owned(),
            scenario: crate::Scenario {
                name: "preflight".to_owned(),
                seed: 0,
                agents: vec![agent()],
                tasks,
                ground_nodes: vec![],
                base_station: None,
                geo_origin: None,
            },
            run_config: crate::RunConfig::default(),
        }
    }

    fn assert_rule(report: &SafetyValidationReport, rule_id: &str) {
        assert!(
            report
                .violations
                .iter()
                .any(|violation| violation.rule_id == rule_id),
            "missing rule {rule_id}: {:?}",
            report.violations
        );
    }

    #[test]
    fn geofence_violation_fails_preflight() {
        let mut entry = entry(vec![task(
            "wp-0",
            Pose {
                x: 20.0,
                y: 0.0,
                z: 5.0,
            },
        )]);
        entry.run_config.safety_config = Some(SafetyConfig {
            geofence: Some(Geofence {
                bounds: Aabb {
                    min_x: 0.0,
                    max_x: 10.0,
                    min_y: 0.0,
                    max_y: 10.0,
                },
            }),
            ..Default::default()
        });
        let report = run_preflight(&entry);
        assert!(!report.passed);
        assert_rule(&report, "geofence.waypoint_outside");
    }

    #[test]
    fn nofly_aabb_violation_fails_preflight() {
        let mut entry = entry(vec![task(
            "wp-0",
            Pose {
                x: 5.0,
                y: 5.0,
                z: 5.0,
            },
        )]);
        entry.run_config.safety_config = Some(SafetyConfig {
            no_fly_zones: vec![NoFlyZone {
                bounds: Aabb {
                    min_x: 0.0,
                    max_x: 10.0,
                    min_y: 0.0,
                    max_y: 10.0,
                },
                active_from_tick: None,
                active_until_tick: None,
            }],
            ..Default::default()
        });
        let report = run_preflight(&entry);
        assert!(!report.passed);
        assert_rule(&report, "nofly.waypoint_inside");
    }

    #[test]
    fn nonfinite_coordinate_rejected() {
        let report = run_preflight(&entry(vec![task(
            "wp-0",
            Pose {
                x: f64::NAN,
                y: 0.0,
                z: 5.0,
            },
        )]));
        assert!(!report.passed);
        assert_rule(&report, "pose.invalid_coordinate");
    }

    #[test]
    fn duplicate_task_id_rejected() {
        let report = run_preflight(&entry(vec![
            task("wp-0", Pose::default()),
            task("wp-0", Pose::default()),
        ]));
        assert!(!report.passed);
        assert_rule(&report, "ownership.duplicate_task_id");
    }

    #[test]
    fn unsupported_strategy_pair_returns_warning() {
        let mut entry = entry(vec![task("wp-0", Pose::default())]);
        entry.mission = "sar".to_owned();
        entry.run_config.enable_cbba = true;
        let report = run_preflight(&entry);
        assert!(report.passed);
        assert_rule(&report, "semantics.unsupported_strategy_pair");
    }

    #[test]
    fn duration_limit_uses_tick_duration_ms() {
        let mut entry = entry(vec![task("wp-0", Pose::default())]);
        entry.run_config.max_ticks = 5;
        entry.run_config.tick_duration_ms = 2000;
        entry.run_config.safety_config = Some(SafetyConfig {
            max_duration_ticks: Some(6),
            ..Default::default()
        });

        let report = run_preflight(&entry);

        assert!(report.passed);
        assert_rule(&report, "route.duration_exceeds_max");
    }

    #[test]
    fn urban_blocked_edge_fails_preflight() {
        let mut entry = urban_entry(false);
        entry.run_config.urban_state.as_mut().unwrap().map.edges[0].blocked = true;
        let report = run_preflight(&entry);
        assert!(!report.passed);
        assert_rule(&report, "urban.blocked_edge");
    }

    #[test]
    fn urban_aabb_intersection_fails_preflight() {
        let mut entry = urban_entry(false);
        entry
            .run_config
            .urban_state
            .as_mut()
            .unwrap()
            .map
            .static_obstacles
            .push(UrbanStaticObstacle {
                id: UrbanObstacleId::from("building".to_owned()),
                bounds: swarm_types::Aabb {
                    min_x: 9.0,
                    max_x: 11.0,
                    min_y: -1.0,
                    max_y: 1.0,
                },
                label: Some("building".to_owned()),
            });
        let report = run_preflight(&entry);
        assert!(!report.passed);
        assert_rule(&report, "urban.aabb_intersection");
    }

    #[test]
    fn valid_urban_route_passes_preflight() {
        let report = run_preflight(&urban_entry(false));
        assert!(report.passed, "{:?}", report.violations);
    }

    #[test]
    fn urban_invalid_temporary_obstacle_fails_preflight() {
        let mut entry = urban_entry(false);
        let urban = entry.run_config.urban_state.as_mut().unwrap();
        urban.temporary_obstacles = vec![swarm_types::UrbanTemporaryObstacle {
            edge_id: swarm_types::UrbanEdgeId::from("no-such-edge".to_owned()),
            appears_at_tick: 1,
            disappears_at_tick: None,
            reason: None,
            severity: None,
        }];
        let report = run_preflight(&entry);
        assert!(
            !report.passed,
            "should fail due to invalid temporary obstacle"
        );
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.rule_id == "urban.invalid_temporary_obstacle"),
            "should have urban.invalid_temporary_obstacle violation"
        );
    }

    fn urban_entry(blocked: bool) -> ScenarioSuiteEntry {
        let n0 = UrbanNodeId::from("n0".to_owned());
        let n1 = UrbanNodeId::from("n1".to_owned());
        ScenarioSuiteEntry {
            mission: "urban-patrol".to_owned(),
            profile: "unit".to_owned(),
            scenario: crate::Scenario {
                name: "urban".to_owned(),
                seed: 0,
                agents: vec![agent()],
                tasks: vec![task("wp-0", Pose::default())],
                ground_nodes: vec![],
                base_station: None,
                geo_origin: None,
            },
            run_config: crate::RunConfig {
                urban_state: Some(crate::UrbanState {
                    map: UrbanMap {
                        nodes: vec![
                            UrbanNode {
                                id: n0.clone(),
                                pose: Pose {
                                    x: 0.0,
                                    y: 0.0,
                                    z: 0.0,
                                },
                            },
                            UrbanNode {
                                id: n1.clone(),
                                pose: Pose {
                                    x: 10.0,
                                    y: 0.0,
                                    z: 0.0,
                                },
                            },
                        ],
                        edges: vec![
                            UrbanEdge {
                                id: UrbanEdgeId::from("e01".to_owned()),
                                from: n0.clone(),
                                to: n1.clone(),
                                cost: 10.0,
                                length_m: 10.0,
                                corridor_width_m: Some(5.0),
                                blocked,
                            },
                            UrbanEdge {
                                id: UrbanEdgeId::from("e10".to_owned()),
                                from: n1.clone(),
                                to: n0.clone(),
                                cost: 10.0,
                                length_m: 10.0,
                                corridor_width_m: Some(5.0),
                                blocked: false,
                            },
                        ],
                        static_obstacles: vec![],
                    },
                    route_loop: UrbanRouteLoop {
                        nodes: vec![n0, n1],
                    },
                    start_node: None,
                    planner: "dijkstra".to_owned(),
                    temporary_obstacles: vec![],
                    blocked_route_policy: swarm_types::UrbanBlockedPolicy::default(),
                    perimeter_patrol: None,
                }),
                ..Default::default()
            },
        }
    }
}
