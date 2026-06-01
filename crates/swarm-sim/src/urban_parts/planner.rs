#![allow(unused_imports)]
use super::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::error::Error;
use std::fmt;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use swarm_types::{
    Pose, UrbanBus, UrbanBusId, UrbanDetectorConfig, UrbanEdge, UrbanMap, UrbanNode, UrbanNodeId,
    UrbanPlannedRoute, UrbanRouteLoop, UrbanRouteSegment, UrbanSearchState, UrbanViolation,
};

pub const URBAN_START_POSE_TOLERANCE_M: f64 = 0.01;
const CORRIDOR_NEUTRAL_WIDTH_M: f64 = 6.0;
const CLEARANCE_NEUTRAL_M: f64 = 8.0;

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

/// Compute an additive route risk proxy from corridor width and obstacle clearance.
pub fn route_risk_score(map: &UrbanMap, route: &UrbanPlannedRoute) -> f64 {
    route
        .segments
        .iter()
        .filter_map(|segment| map.edge(&segment.edge_id))
        .map(|edge| edge_risk_score(map, edge))
        .sum()
}

/// Interpolate a pose along a planned Urban route segment.
///
/// Distance is clamped to the segment length. A zero-length segment returns the
/// destination node pose.
pub fn pose_along_segment(
    map: &UrbanMap,
    segment: &UrbanRouteSegment,
    distance_m: f64,
) -> Result<Pose, UrbanRouteError> {
    let from = map
        .node(&segment.from)
        .map(|node| node.pose)
        .ok_or_else(|| UrbanRouteError::InvalidInput {
            field: "segment.from".to_owned(),
            message: format!("Unknown urban node id '{}'", segment.from),
        })?;
    let to = map.node(&segment.to).map(|node| node.pose).ok_or_else(|| {
        UrbanRouteError::InvalidInput {
            field: "segment.to".to_owned(),
            message: format!("Unknown urban node id '{}'", segment.to),
        }
    })?;
    if segment.length_m <= 0.0 {
        return Ok(to);
    }
    let ratio = (distance_m / segment.length_m).clamp(0.0, 1.0);
    Ok(Pose {
        x: from.x + (to.x - from.x) * ratio,
        y: from.y + (to.y - from.y) * ratio,
        z: from.z + (to.z - from.z) * ratio,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct UrbanBusObservation {
    pub bus_id: UrbanBusId,
    pub pose: Pose,
    pub distance_m: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UrbanDetectionOutcome {
    pub observations: Vec<UrbanBusObservation>,
    pub detection: Option<UrbanBusObservation>,
    pub false_positive: bool,
}

/// Evaluate the mocked distance-based Urban Search detector for one tick.
pub fn detect_buses(
    agent_pose: Pose,
    tick: u64,
    scenario_seed: u64,
    search_state: &UrbanSearchState,
) -> UrbanDetectionOutcome {
    let mut observations: Vec<UrbanBusObservation> = search_state
        .buses
        .iter()
        .filter(|bus| bus_is_active(bus, tick))
        .filter_map(|bus| {
            let distance_m = agent_pose.distance_to(&bus.pose);
            (distance_m <= search_state.detector.detection_range_m).then(|| UrbanBusObservation {
                bus_id: bus.id.clone(),
                pose: bus.pose,
                distance_m,
            })
        })
        .collect();
    observations.sort_by(|a, b| a.bus_id.as_ref().cmp(b.bus_id.as_ref()));

    let detection = observations
        .iter()
        .enumerate()
        .find(|(index, _)| {
            deterministic_probability_draw(
                &search_state.detector,
                scenario_seed,
                tick,
                *index as u64,
                0xD37E_C710_0000_0001,
            ) < search_state.detector.detection_probability
        })
        .map(|(_, observation)| observation.clone());

    let false_positive = detection.is_none()
        && deterministic_probability_draw(
            &search_state.detector,
            scenario_seed,
            tick,
            observations.len() as u64,
            0xFA15_EF05_1717_0001,
        ) < search_state.detector.false_positive_rate;

    UrbanDetectionOutcome {
        observations,
        detection,
        false_positive,
    }
}

fn bus_is_active(bus: &UrbanBus, tick: u64) -> bool {
    bus.active_from_tick.is_none_or(|from| tick >= from)
        && bus.active_until_tick.is_none_or(|until| tick <= until)
}

fn deterministic_probability_draw(
    detector: &UrbanDetectorConfig,
    scenario_seed: u64,
    tick: u64,
    draw_index: u64,
    salt: u64,
) -> f64 {
    let seed = detector.seed
        ^ scenario_seed.rotate_left(13)
        ^ tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ draw_index.wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ salt;
    let mut rng = StdRng::seed_from_u64(seed);
    rng.gen()
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

fn edge_risk_score(map: &UrbanMap, edge: &UrbanEdge) -> f64 {
    let width_penalty = match edge.corridor_width_m {
        Some(width) if width > 0.0 => CORRIDOR_NEUTRAL_WIDTH_M / width,
        Some(_) => CORRIDOR_NEUTRAL_WIDTH_M,
        None => 1.0,
    };
    let clearance_penalty = edge_clearance_m(map, edge)
        .map(|clearance| ((CLEARANCE_NEUTRAL_M - clearance).max(0.0)) / CLEARANCE_NEUTRAL_M)
        .unwrap_or(0.0);
    edge.length_m * (width_penalty + clearance_penalty)
}

fn edge_clearance_m(map: &UrbanMap, edge: &UrbanEdge) -> Option<f64> {
    let from = map.node(&edge.from).map(|node| node.pose)?;
    let to = map.node(&edge.to).map(|node| node.pose)?;
    map.static_obstacles
        .iter()
        .map(|obstacle| segment_aabb_clearance(from, to, obstacle.bounds))
        .min_by(|a, b| a.total_cmp(b))
}

fn midpoint(from: Pose, to: Pose) -> Pose {
    Pose {
        x: (from.x + to.x) / 2.0,
        y: (from.y + to.y) / 2.0,
        z: (from.z + to.z) / 2.0,
    }
}

fn segment_aabb_clearance(from: Pose, to: Pose, bounds: swarm_types::Aabb) -> f64 {
    if bounds.contains(&from) || bounds.contains(&to) || segment_intersects_aabb(from, to, bounds) {
        return 0.0;
    }

    let mut clearance = point_aabb_distance(from, bounds).min(point_aabb_distance(to, bounds));
    for corner in aabb_corners(bounds) {
        clearance = clearance.min(point_segment_distance(corner, from, to));
    }
    clearance
}

fn point_aabb_distance(point: Pose, bounds: swarm_types::Aabb) -> f64 {
    let dx = if point.x < bounds.min_x {
        bounds.min_x - point.x
    } else if point.x > bounds.max_x {
        point.x - bounds.max_x
    } else {
        0.0
    };
    let dy = if point.y < bounds.min_y {
        bounds.min_y - point.y
    } else if point.y > bounds.max_y {
        point.y - bounds.max_y
    } else {
        0.0
    };
    (dx * dx + dy * dy).sqrt()
}

fn aabb_corners(bounds: swarm_types::Aabb) -> [Pose; 4] {
    [
        Pose {
            x: bounds.min_x,
            y: bounds.min_y,
            ..Default::default()
        },
        Pose {
            x: bounds.min_x,
            y: bounds.max_y,
            ..Default::default()
        },
        Pose {
            x: bounds.max_x,
            y: bounds.min_y,
            ..Default::default()
        },
        Pose {
            x: bounds.max_x,
            y: bounds.max_y,
            ..Default::default()
        },
    ]
}

fn point_segment_distance(point: Pose, from: Pose, to: Pose) -> f64 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f64::EPSILON {
        return point.distance_to(&from);
    }
    let t = (((point.x - from.x) * dx + (point.y - from.y) * dy) / len_sq).clamp(0.0, 1.0);
    let projected = Pose {
        x: from.x + t * dx,
        y: from.y + t * dy,
        z: from.z + t * (to.z - from.z),
    };
    point.distance_to(&projected)
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
