use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::error::Error;
use std::fmt;

use swarm_types::{
    Pose, UrbanEdge, UrbanMap, UrbanNodeId, UrbanPlannedRoute, UrbanRouteLoop, UrbanRouteSegment,
    UrbanViolation,
};

#[derive(Clone, Debug, PartialEq)]
pub enum UrbanRouteError {
    InvalidInput { field: String, message: String },
    NoRoute { from: UrbanNodeId, to: UrbanNodeId },
}

impl fmt::Display for UrbanRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { field, message } => write!(f, "[{field}] {message}"),
            Self::NoRoute { from, to } => write!(f, "no urban route from '{from}' to '{to}'"),
        }
    }
}

impl Error for UrbanRouteError {}

#[derive(Clone, Debug)]
struct QueueState {
    cost: f64,
    hops: usize,
    node: UrbanNodeId,
}

impl PartialEq for QueueState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost && self.hops == other.hops && self.node == other.node
    }
}

impl Eq for QueueState {}

impl PartialOrd for QueueState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueueState {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.hops.cmp(&self.hops))
            .then_with(|| other.node.as_ref().cmp(self.node.as_ref()))
    }
}

/// Plan a deterministic shortest path over unblocked Urban road graph edges.
pub fn plan_route(
    map: &UrbanMap,
    from: &UrbanNodeId,
    to: &UrbanNodeId,
) -> Result<UrbanPlannedRoute, UrbanRouteError> {
    ensure_valid_route_inputs(map, from, to)?;
    if from == to {
        return Ok(UrbanPlannedRoute::default());
    }

    let mut adjacency: HashMap<UrbanNodeId, Vec<&UrbanEdge>> = HashMap::new();
    for edge in map.edges.iter().filter(|edge| !edge.blocked) {
        adjacency.entry(edge.from.clone()).or_default().push(edge);
    }
    for edges in adjacency.values_mut() {
        edges.sort_by(|a, b| {
            a.cost
                .total_cmp(&b.cost)
                .then_with(|| a.id.as_ref().cmp(b.id.as_ref()))
                .then_with(|| a.to.as_ref().cmp(b.to.as_ref()))
        });
    }

    let mut queue = BinaryHeap::new();
    let mut dist: HashMap<UrbanNodeId, (f64, usize)> = HashMap::new();
    let mut prev: HashMap<UrbanNodeId, (UrbanNodeId, UrbanRouteSegment)> = HashMap::new();

    dist.insert(from.clone(), (0.0, 0));
    queue.push(QueueState {
        cost: 0.0,
        hops: 0,
        node: from.clone(),
    });

    while let Some(state) = queue.pop() {
        if &state.node == to {
            break;
        }
        if let Some((best_cost, best_hops)) = dist.get(&state.node) {
            if state.cost > *best_cost || (state.cost == *best_cost && state.hops > *best_hops) {
                continue;
            }
        }

        for edge in adjacency.get(&state.node).into_iter().flatten() {
            let next_cost = state.cost + edge.cost;
            let next_hops = state.hops + 1;
            let should_update = match dist.get(&edge.to) {
                None => true,
                Some((old_cost, old_hops)) => {
                    next_cost < *old_cost || (next_cost == *old_cost && next_hops < *old_hops)
                }
            };
            if should_update {
                let segment = UrbanRouteSegment {
                    edge_id: edge.id.clone(),
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                    length_m: edge.length_m,
                    cost: edge.cost,
                };
                dist.insert(edge.to.clone(), (next_cost, next_hops));
                prev.insert(edge.to.clone(), (state.node.clone(), segment));
                queue.push(QueueState {
                    cost: next_cost,
                    hops: next_hops,
                    node: edge.to.clone(),
                });
            }
        }
    }

    if !dist.contains_key(to) {
        return Err(UrbanRouteError::NoRoute {
            from: from.clone(),
            to: to.clone(),
        });
    }

    let mut segments = Vec::new();
    let mut current = to.clone();
    while &current != from {
        let Some((previous, segment)) = prev.remove(&current) else {
            return Err(UrbanRouteError::NoRoute {
                from: from.clone(),
                to: to.clone(),
            });
        };
        segments.push(segment);
        current = previous;
    }
    segments.reverse();
    Ok(planned_route(segments))
}

/// Expand an Urban route loop into shortest-path graph segments.
pub fn expand_route_loop(
    map: &UrbanMap,
    route_loop: &UrbanRouteLoop,
) -> Result<UrbanPlannedRoute, UrbanRouteError> {
    if let Some(error) = map.validate().into_iter().next() {
        return Err(UrbanRouteError::InvalidInput {
            field: format!("map.{}", error.field),
            message: error.message,
        });
    }
    if let Some(error) = map.validate_route_loop(route_loop).into_iter().next() {
        return Err(UrbanRouteError::InvalidInput {
            field: error.field,
            message: error.message,
        });
    }

    let mut loop_nodes = route_loop.nodes.clone();
    if loop_nodes.first() != loop_nodes.last() {
        if let Some(first) = loop_nodes.first().cloned() {
            loop_nodes.push(first);
        }
    }

    let mut segments = Vec::new();
    for pair in loop_nodes.windows(2) {
        let route = plan_route(map, &pair[0], &pair[1])?;
        segments.extend(route.segments);
    }
    Ok(planned_route(segments))
}

/// Judge static validity of a planned Urban route.
pub fn judge_route(map: &UrbanMap, route: &UrbanPlannedRoute) -> Vec<UrbanViolation> {
    let mut violations = Vec::new();
    for segment in &route.segments {
        let Some(edge) = map.edge(&segment.edge_id) else {
            violations.push(UrbanViolation::MissingEdge {
                edge_id: segment.edge_id.clone(),
            });
            continue;
        };
        if edge.blocked {
            violations.push(UrbanViolation::BlockedEdge {
                edge_id: edge.id.clone(),
            });
        }
        let Some(from) = map.node(&segment.from).map(|node| node.pose) else {
            continue;
        };
        let Some(to) = map.node(&segment.to).map(|node| node.pose) else {
            continue;
        };
        for obstacle in &map.static_obstacles {
            if obstacle.bounds.contains(&from)
                || obstacle.bounds.contains(&to)
                || segment_intersects_aabb(from, to, obstacle.bounds)
            {
                violations.push(UrbanViolation::ObstacleIntersection {
                    edge_id: edge.id.clone(),
                    obstacle_id: obstacle.id.clone(),
                    location: midpoint(from, to),
                });
            }
        }
    }
    violations
}

fn ensure_valid_route_inputs(
    map: &UrbanMap,
    from: &UrbanNodeId,
    to: &UrbanNodeId,
) -> Result<(), UrbanRouteError> {
    if let Some(error) = map.validate().into_iter().next() {
        return Err(UrbanRouteError::InvalidInput {
            field: format!("map.{}", error.field),
            message: error.message,
        });
    }
    if map.node(from).is_none() {
        return Err(UrbanRouteError::InvalidInput {
            field: "from".to_owned(),
            message: format!("Unknown urban node id '{from}'"),
        });
    }
    if map.node(to).is_none() {
        return Err(UrbanRouteError::InvalidInput {
            field: "to".to_owned(),
            message: format!("Unknown urban node id '{to}'"),
        });
    }
    Ok(())
}

fn planned_route(segments: Vec<UrbanRouteSegment>) -> UrbanPlannedRoute {
    UrbanPlannedRoute {
        total_length_m: segments.iter().map(|segment| segment.length_m).sum(),
        total_cost: segments.iter().map(|segment| segment.cost).sum(),
        segments,
    }
}

fn midpoint(from: Pose, to: Pose) -> Pose {
    Pose {
        x: (from.x + to.x) / 2.0,
        y: (from.y + to.y) / 2.0,
        z: (from.z + to.z) / 2.0,
    }
}

fn segment_intersects_aabb(from: Pose, to: Pose, bounds: swarm_types::Aabb) -> bool {
    let mut t_min = 0.0;
    let mut t_max = 1.0;
    let dx = to.x - from.x;
    let dy = to.y - from.y;

    clip_axis(-dx, from.x - bounds.min_x, &mut t_min, &mut t_max)
        && clip_axis(dx, bounds.max_x - from.x, &mut t_min, &mut t_max)
        && clip_axis(-dy, from.y - bounds.min_y, &mut t_min, &mut t_max)
        && clip_axis(dy, bounds.max_y - from.y, &mut t_min, &mut t_max)
}

fn clip_axis(p: f64, q: f64, t_min: &mut f64, t_max: &mut f64) -> bool {
    if p == 0.0 {
        return q >= 0.0;
    }
    let r = q / p;
    if p < 0.0 {
        if r > *t_max {
            return false;
        }
        if r > *t_min {
            *t_min = r;
        }
    } else {
        if r < *t_min {
            return false;
        }
        if r < *t_max {
            *t_max = r;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::{Aabb, UrbanEdge, UrbanEdgeId, UrbanMap, UrbanNode, UrbanStaticObstacle};

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
        UrbanEdge {
            id: UrbanEdgeId::from(id.to_owned()),
            from: UrbanNodeId::from(from.to_owned()),
            to: UrbanNodeId::from(to.to_owned()),
            cost,
            length_m: cost,
            corridor_width_m: Some(4.0),
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
}
