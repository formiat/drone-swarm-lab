#![allow(unused_imports)]
use super::*;
use swarm_types::{
    Aabb, Pose, UrbanBus, UrbanBusId, UrbanDetectorConfig, UrbanEdge, UrbanEdgeId, UrbanMap,
    UrbanNode, UrbanNodeId, UrbanPlannedRoute, UrbanRouteLoop, UrbanRouteSegment, UrbanSearchState,
    UrbanStaticObstacle, UrbanViolation,
};

fn node(id: &str, x: f64, y: f64) -> UrbanNode {
    UrbanNode {
        id: UrbanNodeId::from(id.to_owned()),
        pose: Pose {
            x,
            y,
            ..Default::default()
        },
    }
}

fn edge(id: &str, from: &str, to: &str, cost: f64) -> UrbanEdge {
    edge_with_width(id, from, to, cost, 4.0)
}

fn edge_with_width(id: &str, from: &str, to: &str, cost: f64, width: f64) -> UrbanEdge {
    UrbanEdge {
        id: UrbanEdgeId::from(id.to_owned()),
        from: UrbanNodeId::from(from.to_owned()),
        to: UrbanNodeId::from(to.to_owned()),
        cost,
        length_m: cost,
        corridor_width_m: Some(width),
        blocked: false,
    }
}

fn block_map() -> UrbanMap {
    UrbanMap {
        nodes: vec![
            node("n0", 0.0, 0.0),
            node("n1", 10.0, 0.0),
            node("n2", 10.0, 10.0),
            node("n3", 0.0, 10.0),
        ],
        edges: vec![
            edge("e01", "n0", "n1", 10.0),
            edge("e12", "n1", "n2", 10.0),
            edge("e23", "n2", "n3", 10.0),
            edge("e30", "n3", "n0", 10.0),
            edge("e02", "n0", "n2", 25.0),
        ],
        static_obstacles: vec![],
    }
}

fn corridor_delta_map() -> UrbanMap {
    UrbanMap {
        nodes: vec![
            node("start", 0.0, 0.0),
            node("goal", 20.0, 0.0),
            node("safe-a", 0.0, 10.0),
            node("safe-b", 20.0, 10.0),
        ],
        edges: vec![
            edge_with_width("narrow-shortcut", "start", "goal", 20.0, 1.5),
            edge_with_width("safe-north-a", "start", "safe-a", 10.0, 8.0),
            edge_with_width("safe-north-b", "safe-a", "safe-b", 20.0, 8.0),
            edge_with_width("safe-north-c", "safe-b", "goal", 10.0, 8.0),
        ],
        static_obstacles: vec![UrbanStaticObstacle {
            id: swarm_types::UrbanObstacleId::from("building-near-shortcut".to_owned()),
            bounds: Aabb {
                min_x: 9.0,
                min_y: 2.0,
                max_x: 11.0,
                max_y: 4.0,
            },
            label: Some("building".to_owned()),
        }],
    }
}

fn search_state(
    bus_pose: Pose,
    range: f64,
    probability: f64,
    false_positive: f64,
) -> UrbanSearchState {
    UrbanSearchState {
        buses: vec![UrbanBus {
            id: UrbanBusId::from("bus-0".to_owned()),
            pose: bus_pose,
            active_from_tick: None,
            active_until_tick: None,
        }],
        detector: UrbanDetectorConfig {
            detection_range_m: range,
            detection_probability: probability,
            false_positive_rate: false_positive,
            seed: 11,
        },
    }
}

#[test]
fn urban_dijkstra_returns_shortest_route() {
    let route = plan_route(
        &block_map(),
        &UrbanNodeId::from("n0".to_owned()),
        &UrbanNodeId::from("n2".to_owned()),
    )
    .unwrap();
    let ids: Vec<_> = route
        .segments
        .iter()
        .map(|segment| &segment.edge_id)
        .collect();
    assert_eq!(
        ids,
        vec![
            &UrbanEdgeId::from("e01".to_owned()),
            &UrbanEdgeId::from("e12".to_owned())
        ]
    );
    assert_eq!(route.total_length_m, 20.0);
}

#[test]
fn urban_dijkstra_tie_breaking_is_deterministic() {
    let mut map = block_map();
    map.edges.push(edge("e03", "n0", "n3", 10.0));
    let route = plan_route(
        &map,
        &UrbanNodeId::from("n0".to_owned()),
        &UrbanNodeId::from("n2".to_owned()),
    )
    .unwrap();
    let ids: Vec<_> = route
        .segments
        .iter()
        .map(|segment| &segment.edge_id)
        .collect();
    assert_eq!(
        ids,
        vec![
            &UrbanEdgeId::from("e01".to_owned()),
            &UrbanEdgeId::from("e12".to_owned())
        ]
    );
}

#[test]
fn urban_planner_mode_rejects_unknown_value() {
    let err = UrbanPlannerMode::parse("shortest-and-magic").unwrap_err();
    assert!(matches!(err, UrbanRouteError::InvalidInput { field, .. } if field == "planner"));
}

#[test]
fn corridor_aware_route_prefers_wider_lower_risk_detour() {
    let map = corridor_delta_map();
    let from = UrbanNodeId::from("start".to_owned());
    let to = UrbanNodeId::from("goal".to_owned());
    let dijkstra = plan_route_with_mode(&map, &from, &to, UrbanPlannerMode::Dijkstra).unwrap();
    let corridor = plan_route_with_mode(&map, &from, &to, UrbanPlannerMode::CorridorAware).unwrap();

    assert_eq!(dijkstra.segments.len(), 1);
    assert_eq!(
        dijkstra.segments[0].edge_id,
        UrbanEdgeId::from("narrow-shortcut".to_owned())
    );
    assert_eq!(
        corridor
            .segments
            .iter()
            .map(|segment| segment.edge_id.as_ref())
            .collect::<Vec<_>>(),
        vec!["safe-north-a", "safe-north-b", "safe-north-c"]
    );
    assert!(corridor.total_length_m > dijkstra.total_length_m);
    assert!(route_risk_score(&map, &corridor) < route_risk_score(&map, &dijkstra));
    assert!(judge_route(&map, &corridor).is_empty());
}

#[test]
fn corridor_aware_handles_missing_width_without_panic() {
    let mut map = corridor_delta_map();
    map.edges[0].corridor_width_m = None;
    let route = plan_route_with_mode(
        &map,
        &UrbanNodeId::from("start".to_owned()),
        &UrbanNodeId::from("goal".to_owned()),
        UrbanPlannerMode::CorridorAware,
    )
    .unwrap();
    assert!(!route.segments.is_empty());
    assert!(route_risk_score(&map, &route).is_finite());
}

#[test]
fn urban_route_loop_expands_segments() {
    let route = expand_route_loop(
        &block_map(),
        &UrbanRouteLoop {
            nodes: vec![
                UrbanNodeId::from("n0".to_owned()),
                UrbanNodeId::from("n1".to_owned()),
                UrbanNodeId::from("n2".to_owned()),
                UrbanNodeId::from("n3".to_owned()),
                UrbanNodeId::from("n0".to_owned()),
            ],
        },
    )
    .unwrap();
    assert_eq!(route.segments.len(), 4);
    assert_eq!(route.total_length_m, 40.0);
}

#[test]
fn urban_route_missing_node_is_error() {
    let err = plan_route(
        &block_map(),
        &UrbanNodeId::from("missing".to_owned()),
        &UrbanNodeId::from("n2".to_owned()),
    )
    .unwrap_err();
    assert!(matches!(err, UrbanRouteError::InvalidInput { .. }));
}

#[test]
fn urban_route_avoids_blocked_edge() {
    let mut map = block_map();
    map.edges
        .iter_mut()
        .find(|edge| edge.id == UrbanEdgeId::from("e01".to_owned()))
        .unwrap()
        .blocked = true;
    map.edges.push(edge("e03", "n0", "n3", 10.0));
    map.edges.push(edge("e32", "n3", "n2", 10.0));
    let route = plan_route(
        &map,
        &UrbanNodeId::from("n0".to_owned()),
        &UrbanNodeId::from("n2".to_owned()),
    )
    .unwrap();
    assert_eq!(
        route.segments[0].edge_id,
        UrbanEdgeId::from("e03".to_owned())
    );
}

#[test]
fn urban_route_reports_no_route() {
    let mut map = block_map();
    map.edges.clear();
    map.edges.push(edge("isolated", "n0", "n1", 1.0));
    let err = plan_route(
        &map,
        &UrbanNodeId::from("n2".to_owned()),
        &UrbanNodeId::from("n0".to_owned()),
    )
    .unwrap_err();
    assert!(matches!(err, UrbanRouteError::NoRoute { .. }));
}

#[test]
fn urban_judge_reports_blocked_edge_violation() {
    let mut map = block_map();
    map.edges[0].blocked = true;
    let route = UrbanPlannedRoute {
        segments: vec![UrbanRouteSegment {
            edge_id: UrbanEdgeId::from("e01".to_owned()),
            from: UrbanNodeId::from("n0".to_owned()),
            to: UrbanNodeId::from("n1".to_owned()),
            length_m: 10.0,
            cost: 10.0,
        }],
        total_length_m: 10.0,
        total_cost: 10.0,
    };
    assert!(matches!(
        judge_route(&map, &route).as_slice(),
        [UrbanViolation::BlockedEdge { .. }]
    ));
}

#[test]
fn urban_judge_reports_aabb_intersection() {
    let mut map = block_map();
    map.static_obstacles.push(UrbanStaticObstacle {
        id: swarm_types::UrbanObstacleId::from("building".to_owned()),
        bounds: Aabb {
            min_x: 4.0,
            min_y: -1.0,
            max_x: 6.0,
            max_y: 1.0,
        },
        label: Some("building".to_owned()),
    });
    let route = plan_route(
        &map,
        &UrbanNodeId::from("n0".to_owned()),
        &UrbanNodeId::from("n1".to_owned()),
    )
    .unwrap();
    assert!(matches!(
        judge_route(&map, &route).as_slice(),
        [UrbanViolation::ObstacleIntersection { .. }]
    ));
}

#[test]
fn urban_pose_along_segment_interpolates_and_clamps() {
    let map = block_map();
    let segment = UrbanRouteSegment {
        edge_id: UrbanEdgeId::from("e01".to_owned()),
        from: UrbanNodeId::from("n0".to_owned()),
        to: UrbanNodeId::from("n1".to_owned()),
        length_m: 10.0,
        cost: 10.0,
    };

    let halfway = pose_along_segment(&map, &segment, 5.0).unwrap();
    assert_eq!(halfway.x, 5.0);
    assert_eq!(halfway.y, 0.0);

    let clamped = pose_along_segment(&map, &segment, 50.0).unwrap();
    assert_eq!(clamped.x, 10.0);
    assert_eq!(clamped.y, 0.0);
}

#[test]
fn detector_detects_in_range_bus_with_probability_one() {
    let state = search_state(
        Pose {
            x: 1.0,
            y: 0.0,
            ..Default::default()
        },
        2.0,
        1.0,
        0.0,
    );

    let outcome = detect_buses(Pose::default(), 0, 42, &state);

    assert_eq!(outcome.observations.len(), 1);
    assert!(outcome.detection.is_some());
    assert!(!outcome.false_positive);
}

#[test]
fn detector_ignores_out_of_range_bus() {
    let state = search_state(
        Pose {
            x: 10.0,
            y: 0.0,
            ..Default::default()
        },
        2.0,
        1.0,
        0.0,
    );

    let outcome = detect_buses(Pose::default(), 0, 42, &state);

    assert!(outcome.observations.is_empty());
    assert!(outcome.detection.is_none());
    assert!(!outcome.false_positive);
}

#[test]
fn detector_probability_zero_never_detects_real_bus() {
    let state = search_state(
        Pose {
            x: 1.0,
            y: 0.0,
            ..Default::default()
        },
        2.0,
        0.0,
        0.0,
    );

    let outcome = detect_buses(Pose::default(), 0, 42, &state);

    assert_eq!(outcome.observations.len(), 1);
    assert!(outcome.detection.is_none());
    assert!(!outcome.false_positive);
}

#[test]
fn detector_false_positive_is_seed_controlled() {
    let state = search_state(
        Pose {
            x: 10.0,
            y: 0.0,
            ..Default::default()
        },
        2.0,
        0.0,
        1.0,
    );

    let outcome = detect_buses(Pose::default(), 0, 42, &state);

    assert!(outcome.observations.is_empty());
    assert!(outcome.detection.is_none());
    assert!(outcome.false_positive);
}

#[test]
fn detector_respects_bus_active_window() {
    let mut state = search_state(
        Pose {
            x: 1.0,
            y: 0.0,
            ..Default::default()
        },
        2.0,
        1.0,
        0.0,
    );
    state.buses[0].active_from_tick = Some(5);
    state.buses[0].active_until_tick = Some(10);

    assert!(detect_buses(Pose::default(), 4, 42, &state)
        .observations
        .is_empty());
    assert!(detect_buses(Pose::default(), 5, 42, &state)
        .detection
        .is_some());
    assert!(detect_buses(Pose::default(), 11, 42, &state)
        .observations
        .is_empty());
}
