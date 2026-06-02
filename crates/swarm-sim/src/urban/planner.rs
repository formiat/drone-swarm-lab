use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::error::Error;
use std::fmt;

use swarm_types::{
    UrbanEdge, UrbanMap, UrbanNode, UrbanNodeId, UrbanPlannedRoute, UrbanRouteLoop,
    UrbanRouteSegment,
};

use super::risk::edge_risk_score;

pub const URBAN_START_POSE_TOLERANCE_M: f64 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrbanPlannerMode {
    Dijkstra,
    CorridorAware,
}

impl UrbanPlannerMode {
    pub fn parse(input: &str) -> Result<Self, UrbanRouteError> {
        match input.trim().to_ascii_lowercase().as_str() {
            "dijkstra" => Ok(Self::Dijkstra),
            "corridor-aware" | "corridor_aware" => Ok(Self::CorridorAware),
            other => Err(UrbanRouteError::InvalidInput {
                field: "planner".to_owned(),
                message: format!(
                    "Unknown urban planner '{other}'. Expected 'dijkstra' or 'corridor-aware'"
                ),
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dijkstra => "dijkstra",
            Self::CorridorAware => "corridor-aware",
        }
    }
}

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
    plan_route_with_mode(map, from, to, UrbanPlannerMode::Dijkstra)
}

/// Plan a deterministic path over unblocked Urban road graph edges.
pub fn plan_route_with_mode(
    map: &UrbanMap,
    from: &UrbanNodeId,
    to: &UrbanNodeId,
    planner: UrbanPlannerMode,
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
            let edge_cost = planner_edge_cost(map, edge, planner);
            let next_cost = state.cost + edge_cost;
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
                    cost: edge_cost,
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
    expand_route_loop_with_planner(map, route_loop, UrbanPlannerMode::Dijkstra)
}

/// Expand an Urban route loop with a named planner from scenario DSL.
pub fn expand_route_loop_with_planner_name(
    map: &UrbanMap,
    route_loop: &UrbanRouteLoop,
    planner: &str,
) -> Result<UrbanPlannedRoute, UrbanRouteError> {
    let planner = UrbanPlannerMode::parse(planner)?;
    expand_route_loop_with_planner(map, route_loop, planner)
}

/// Expand an Urban route loop with the selected planner.
pub fn expand_route_loop_with_planner(
    map: &UrbanMap,
    route_loop: &UrbanRouteLoop,
    planner: UrbanPlannerMode,
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
        let route = plan_route_with_mode(map, &pair[0], &pair[1], planner)?;
        segments.extend(route.segments);
    }
    Ok(planned_route(segments))
}

/// Resolve and validate the executable start node for an Urban patrol route.
pub fn route_start_node<'a>(
    map: &'a UrbanMap,
    route_loop: &UrbanRouteLoop,
    route: &UrbanPlannedRoute,
    start_node: Option<&UrbanNodeId>,
) -> Result<&'a UrbanNode, UrbanRouteError> {
    let route_start_id = route
        .segments
        .first()
        .map(|segment| &segment.from)
        .or_else(|| route_loop.nodes.first())
        .ok_or_else(|| UrbanRouteError::InvalidInput {
            field: "route_loop.nodes".to_owned(),
            message: "Urban route loop must define a start node".to_owned(),
        })?;

    if let Some(start_node) = start_node {
        if map.node(start_node).is_none() {
            return Err(UrbanRouteError::InvalidInput {
                field: "start_node".to_owned(),
                message: format!("Unknown urban start_node '{start_node}'"),
            });
        }
        if start_node != route_start_id {
            return Err(UrbanRouteError::InvalidInput {
                field: "start_node".to_owned(),
                message: format!(
                    "Urban start_node '{start_node}' must match route_loop.nodes[0] '{route_start_id}' in M65"
                ),
            });
        }
    }

    map.node(route_start_id)
        .ok_or_else(|| UrbanRouteError::InvalidInput {
            field: "route_loop.nodes[0]".to_owned(),
            message: format!("Unknown urban node id '{route_start_id}'"),
        })
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

fn planner_edge_cost(map: &UrbanMap, edge: &UrbanEdge, planner: UrbanPlannerMode) -> f64 {
    match planner {
        UrbanPlannerMode::Dijkstra => edge.cost,
        UrbanPlannerMode::CorridorAware => edge.cost + edge_risk_score(map, edge),
    }
}
