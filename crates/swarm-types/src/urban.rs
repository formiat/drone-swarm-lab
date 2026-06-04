use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use derive_more::{AsRef, Deref, DerefMut, Display, From, Into};
use serde::{Deserialize, Serialize};

use crate::pose::{Aabb, Pose};

/// Unique identifier for an urban road graph node.
#[derive(
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct UrbanNodeId(String);

/// Unique identifier for an urban road graph edge.
#[derive(
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct UrbanEdgeId(String);

/// Unique identifier for an urban static obstacle.
#[derive(
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct UrbanObstacleId(String);

/// Unique identifier for a mocked Urban bus target.
#[derive(
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct UrbanBusId(String);

/// A scheduled stop for a mocked Urban bus target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanBusStop {
    pub node_id: UrbanNodeId,
    pub arrival_tick: u64,
}

/// A deterministic graph-based route for a mocked Urban bus target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanBusRoute {
    pub stops: Vec<UrbanBusStop>,
    pub speed_m_per_tick: f64,
}

/// An intersection or waypoint node in a road graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanNode {
    pub id: UrbanNodeId,
    pub pose: Pose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo: Option<UrbanGeoPoint>,
}

/// WGS84 position attached to an Urban road-graph node.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanGeoPoint {
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// Altitude in metres relative to the current mission altitude reference.
    pub alt_m: f64,
}

/// A directed road/corridor segment in a road graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanEdge {
    pub id: UrbanEdgeId,
    pub from: UrbanNodeId,
    pub to: UrbanNodeId,
    pub cost: f64,
    pub length_m: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corridor_width_m: Option<f64>,
    #[serde(default)]
    pub blocked: bool,
}

/// A static AABB obstacle such as a building or no-fly rectangle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanStaticObstacle {
    pub id: UrbanObstacleId,
    pub bounds: Aabb,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// A minimal road-graph map for Urban missions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanMap {
    pub nodes: Vec<UrbanNode>,
    pub edges: Vec<UrbanEdge>,
    #[serde(default)]
    pub static_obstacles: Vec<UrbanStaticObstacle>,
}

/// A patrol loop represented as an ordered sequence of road graph node ids.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanRouteLoop {
    pub nodes: Vec<UrbanNodeId>,
}

/// One planned edge traversal in an Urban route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanRouteSegment {
    pub edge_id: UrbanEdgeId,
    pub from: UrbanNodeId,
    pub to: UrbanNodeId,
    pub length_m: f64,
    pub cost: f64,
}

/// A route planned over an `UrbanMap`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UrbanPlannedRoute {
    pub segments: Vec<UrbanRouteSegment>,
    pub total_length_m: f64,
    pub total_cost: f64,
}

/// A judge violation for a planned Urban route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum UrbanViolation {
    MissingEdge {
        edge_id: UrbanEdgeId,
    },
    BlockedEdge {
        edge_id: UrbanEdgeId,
    },
    ObstacleIntersection {
        edge_id: UrbanEdgeId,
        obstacle_id: UrbanObstacleId,
        location: Pose,
    },
}

/// A mocked bus target for Urban Search.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanBus {
    pub id: UrbanBusId,
    pub pose: Pose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_from_tick: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_until_tick: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<UrbanBusRoute>,
}

/// Optional perimeter-patrol declaration for Urban waypoint mission realism.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanPerimeterPatrol {
    pub polygon: Vec<Pose>,
    pub spacing_m: f64,
}

impl UrbanBus {
    /// Return the mocked bus pose at `tick`.
    ///
    /// Static buses use `pose` and the legacy active window. Moving buses use
    /// scheduled graph stops. The schedule, not `speed_m_per_tick`, is
    /// authoritative for interpolation.
    pub fn pose_at_tick(&self, map: &UrbanMap, tick: u64) -> Option<Pose> {
        let Some(route) = &self.route else {
            return bus_static_window_active(self, tick).then_some(self.pose);
        };
        let first = route.stops.first()?;
        let last = route.stops.last()?;
        if tick < first.arrival_tick || tick > last.arrival_tick {
            return None;
        }
        if route.stops.len() == 1 {
            return (tick == first.arrival_tick)
                .then(|| map.node(&first.node_id).map(|node| node.pose))
                .flatten();
        }
        for pair in route.stops.windows(2) {
            let from = &pair[0];
            let to = &pair[1];
            if tick < from.arrival_tick || tick > to.arrival_tick {
                continue;
            }
            let from_pose = map.node(&from.node_id)?.pose;
            let to_pose = map.node(&to.node_id)?.pose;
            let duration = to.arrival_tick.saturating_sub(from.arrival_tick);
            if duration == 0 {
                return None;
            }
            let ratio = (tick - from.arrival_tick) as f64 / duration as f64;
            return Some(Pose {
                x: from_pose.x + (to_pose.x - from_pose.x) * ratio,
                y: from_pose.y + (to_pose.y - from_pose.y) * ratio,
                z: from_pose.z + (to_pose.z - from_pose.z) * ratio,
            });
        }
        None
    }
}

/// Distance-based mocked detector config for Urban Search.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanDetectorConfig {
    pub detection_range_m: f64,
    pub detection_probability: f64,
    pub false_positive_rate: f64,
    pub seed: u64,
}

/// M66 Urban Search state layered on top of `UrbanState`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanSearchState {
    pub buses: Vec<UrbanBus>,
    pub detector: UrbanDetectorConfig,
}

/// Whether a temporary obstacle causes a hard routing block or is advisory only.
///
/// `Hard` obstacles (and obstacles with no severity set) are included in the
/// effective blocked-edge set: the route planner avoids them and traversal is a
/// judge violation. `Soft` obstacles are advisory — they appear in lookahead
/// events but do not block routing and are not counted as violations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObstacleSeverity {
    Hard,
    Soft,
}

/// A time-gated edge blockage injected into an Urban scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanTemporaryObstacle {
    pub edge_id: UrbanEdgeId,
    pub appears_at_tick: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disappears_at_tick: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Defaults to `Hard` when absent for backward compatibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<ObstacleSeverity>,
}

impl UrbanTemporaryObstacle {
    /// Returns `true` if this obstacle blocks routing (severity is `Hard` or absent).
    pub fn is_hard_block(&self) -> bool {
        !matches!(self.severity, Some(ObstacleSeverity::Soft))
    }

    /// Returns true if the obstacle is active at `tick`.
    pub fn is_active(&self, tick: u64) -> bool {
        tick >= self.appears_at_tick && self.disappears_at_tick.is_none_or(|d| tick < d)
    }
}

/// Policy applied when the next segment on the route is blocked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UrbanBlockedPolicy {
    #[default]
    Wait,
    Replan,
    Abort,
}

/// Typed validation error for Urban map and route-loop inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrbanMapValidationError {
    pub field: String,
    pub message: String,
}

impl UrbanMapValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for UrbanMapValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.field, self.message)
    }
}

impl Error for UrbanMapValidationError {}

impl UrbanMap {
    /// Validate node, edge and obstacle references.
    pub fn validate(&self) -> Vec<UrbanMapValidationError> {
        let mut errors = Vec::new();

        if self.nodes.is_empty() {
            errors.push(UrbanMapValidationError::new(
                "nodes",
                "Urban map must contain at least one node",
            ));
        }
        if self.edges.is_empty() {
            errors.push(UrbanMapValidationError::new(
                "edges",
                "Urban map must contain at least one edge",
            ));
        }

        let mut node_ids = HashSet::new();
        let mut nodes_with_geo = 0usize;
        for (index, node) in self.nodes.iter().enumerate() {
            if !node_ids.insert(node.id.clone()) {
                errors.push(UrbanMapValidationError::new(
                    format!("nodes[{index}].id"),
                    format!("Duplicate urban node id '{}'", node.id),
                ));
            }
            validate_pose(index, node.pose, &mut errors);
            if let Some(geo) = node.geo {
                nodes_with_geo += 1;
                validate_geo(index, geo, &mut errors);
            }
        }
        if nodes_with_geo > 0 && nodes_with_geo != self.nodes.len() {
            errors.push(UrbanMapValidationError::new(
                "nodes[].geo",
                "Urban map nodes must either all carry geo coordinates or all omit them",
            ));
        }

        let mut edge_ids = HashSet::new();
        for (index, edge) in self.edges.iter().enumerate() {
            if !edge_ids.insert(edge.id.clone()) {
                errors.push(UrbanMapValidationError::new(
                    format!("edges[{index}].id"),
                    format!("Duplicate urban edge id '{}'", edge.id),
                ));
            }
            if !node_ids.contains(&edge.from) {
                errors.push(UrbanMapValidationError::new(
                    format!("edges[{index}].from"),
                    format!("Unknown urban node id '{}'", edge.from),
                ));
            }
            if !node_ids.contains(&edge.to) {
                errors.push(UrbanMapValidationError::new(
                    format!("edges[{index}].to"),
                    format!("Unknown urban node id '{}'", edge.to),
                ));
            }
            if !edge.cost.is_finite() || edge.cost < 0.0 {
                errors.push(UrbanMapValidationError::new(
                    format!("edges[{index}].cost"),
                    "Edge cost must be finite and >= 0",
                ));
            }
            if !edge.length_m.is_finite() || edge.length_m < 0.0 {
                errors.push(UrbanMapValidationError::new(
                    format!("edges[{index}].length_m"),
                    "Edge length_m must be finite and >= 0",
                ));
            }
            if edge
                .corridor_width_m
                .is_some_and(|width| !width.is_finite() || width < 0.0)
            {
                errors.push(UrbanMapValidationError::new(
                    format!("edges[{index}].corridor_width_m"),
                    "corridor_width_m must be finite and >= 0",
                ));
            }
        }

        let mut obstacle_ids = HashSet::new();
        for (index, obstacle) in self.static_obstacles.iter().enumerate() {
            if !obstacle_ids.insert(obstacle.id.clone()) {
                errors.push(UrbanMapValidationError::new(
                    format!("static_obstacles[{index}].id"),
                    format!("Duplicate urban obstacle id '{}'", obstacle.id),
                ));
            }
            let bounds = obstacle.bounds;
            if !bounds.min_x.is_finite()
                || !bounds.min_y.is_finite()
                || !bounds.max_x.is_finite()
                || !bounds.max_y.is_finite()
                || bounds.min_x > bounds.max_x
                || bounds.min_y > bounds.max_y
            {
                errors.push(UrbanMapValidationError::new(
                    format!("static_obstacles[{index}].bounds"),
                    "AABB bounds must be finite and min <= max",
                ));
            }
        }

        errors
    }

    /// Validate that all route-loop nodes exist and the loop is usable.
    pub fn validate_route_loop(&self, route_loop: &UrbanRouteLoop) -> Vec<UrbanMapValidationError> {
        let mut errors = Vec::new();
        if route_loop.nodes.len() < 2 {
            errors.push(UrbanMapValidationError::new(
                "route_loop.nodes",
                "Urban route loop must contain at least two nodes",
            ));
        }

        let node_ids: HashSet<_> = self.nodes.iter().map(|node| node.id.clone()).collect();
        for (index, node_id) in route_loop.nodes.iter().enumerate() {
            if !node_ids.contains(node_id) {
                errors.push(UrbanMapValidationError::new(
                    format!("route_loop.nodes[{index}]"),
                    format!("Unknown urban node id '{node_id}'"),
                ));
            }
        }
        errors
    }

    /// Validate temporary obstacles: edge_id must exist; appears_at_tick must be ≤ disappears_at_tick.
    pub fn validate_temporary_obstacles(
        &self,
        obstacles: &[UrbanTemporaryObstacle],
    ) -> Vec<UrbanMapValidationError> {
        let edge_ids: HashSet<_> = self.edges.iter().map(|e| e.id.clone()).collect();
        let mut errors = Vec::new();
        for (index, obstacle) in obstacles.iter().enumerate() {
            if !edge_ids.contains(&obstacle.edge_id) {
                errors.push(UrbanMapValidationError::new(
                    format!("temporary_obstacles[{index}].edge_id"),
                    format!("Unknown urban edge id '{}'", obstacle.edge_id),
                ));
            }
            if let (Some(appears), Some(disappears)) =
                (Some(obstacle.appears_at_tick), obstacle.disappears_at_tick)
            {
                if appears >= disappears {
                    errors.push(UrbanMapValidationError::new(
                        format!("temporary_obstacles[{index}].disappears_at_tick"),
                        "disappears_at_tick must be greater than appears_at_tick",
                    ));
                }
            }
        }
        errors
    }

    pub fn node(&self, id: &UrbanNodeId) -> Option<&UrbanNode> {
        self.nodes.iter().find(|node| &node.id == id)
    }

    pub fn edge(&self, id: &UrbanEdgeId) -> Option<&UrbanEdge> {
        self.edges.iter().find(|edge| &edge.id == id)
    }
}

impl UrbanSearchState {
    /// Validate bus target and mocked detector inputs.
    pub fn validate(&self) -> Vec<UrbanMapValidationError> {
        self.validate_inner(None)
    }

    /// Validate bus target, route and mocked detector inputs against a map.
    pub fn validate_with_map(&self, map: &UrbanMap) -> Vec<UrbanMapValidationError> {
        self.validate_inner(Some(map))
    }

    fn validate_inner(&self, map: Option<&UrbanMap>) -> Vec<UrbanMapValidationError> {
        let mut errors = Vec::new();
        if self.buses.is_empty() {
            errors.push(UrbanMapValidationError::new(
                "buses",
                "Urban Search requires at least one bus",
            ));
        }

        let mut bus_ids = HashSet::new();
        for (index, bus) in self.buses.iter().enumerate() {
            if !bus_ids.insert(bus.id.clone()) {
                errors.push(UrbanMapValidationError::new(
                    format!("buses[{index}].id"),
                    format!("Duplicate urban bus id '{}'", bus.id),
                ));
            }
            if !bus.pose.x.is_finite() || !bus.pose.y.is_finite() || !bus.pose.z.is_finite() {
                errors.push(UrbanMapValidationError::new(
                    format!("buses[{index}].pose"),
                    "Bus pose coordinates must be finite",
                ));
            }
            if let (Some(from), Some(until)) = (bus.active_from_tick, bus.active_until_tick) {
                if from > until {
                    errors.push(UrbanMapValidationError::new(
                        format!("buses[{index}].active_until_tick"),
                        "active_until_tick must be >= active_from_tick",
                    ));
                }
            }
            if let Some(route) = &bus.route {
                validate_bus_route(index, route, map, &mut errors);
            }
        }

        if !self.detector.detection_range_m.is_finite() || self.detector.detection_range_m < 0.0 {
            errors.push(UrbanMapValidationError::new(
                "detector.detection_range_m",
                "detection_range_m must be finite and >= 0",
            ));
        }
        validate_probability(
            "detector.detection_probability",
            self.detector.detection_probability,
            &mut errors,
        );
        validate_probability(
            "detector.false_positive_rate",
            self.detector.false_positive_rate,
            &mut errors,
        );

        errors
    }
}

fn bus_static_window_active(bus: &UrbanBus, tick: u64) -> bool {
    bus.active_from_tick.is_none_or(|from| tick >= from)
        && bus.active_until_tick.is_none_or(|until| tick <= until)
}

fn validate_bus_route(
    bus_index: usize,
    route: &UrbanBusRoute,
    map: Option<&UrbanMap>,
    errors: &mut Vec<UrbanMapValidationError>,
) {
    if route.stops.is_empty() {
        errors.push(UrbanMapValidationError::new(
            format!("buses[{bus_index}].route.stops"),
            "Bus route requires at least one stop",
        ));
    }
    if !route.speed_m_per_tick.is_finite() || route.speed_m_per_tick <= 0.0 {
        errors.push(UrbanMapValidationError::new(
            format!("buses[{bus_index}].route.speed_m_per_tick"),
            "speed_m_per_tick must be finite and > 0",
        ));
    }
    let mut previous_tick = None;
    for (stop_index, stop) in route.stops.iter().enumerate() {
        if previous_tick.is_some_and(|previous| stop.arrival_tick <= previous) {
            errors.push(UrbanMapValidationError::new(
                format!("buses[{bus_index}].route.stops[{stop_index}].arrival_tick"),
                "arrival_tick values must be strictly increasing",
            ));
        }
        previous_tick = Some(stop.arrival_tick);
        if let Some(map) = map {
            if map.node(&stop.node_id).is_none() {
                errors.push(UrbanMapValidationError::new(
                    format!("buses[{bus_index}].route.stops[{stop_index}].node_id"),
                    format!("Unknown urban bus route stop node id '{}'", stop.node_id),
                ));
            }
        }
    }
}

fn validate_pose(index: usize, pose: Pose, errors: &mut Vec<UrbanMapValidationError>) {
    if !pose.x.is_finite() || !pose.y.is_finite() || !pose.z.is_finite() {
        errors.push(UrbanMapValidationError::new(
            format!("nodes[{index}].pose"),
            "Node pose coordinates must be finite",
        ));
    }
}

fn validate_geo(index: usize, geo: UrbanGeoPoint, errors: &mut Vec<UrbanMapValidationError>) {
    if !geo.lat_deg.is_finite() || geo.lat_deg < -90.0 || geo.lat_deg > 90.0 {
        errors.push(UrbanMapValidationError::new(
            format!("nodes[{index}].geo.lat_deg"),
            "lat_deg must be finite and within [-90, 90]",
        ));
    }
    if !geo.lon_deg.is_finite() || geo.lon_deg < -180.0 || geo.lon_deg > 180.0 {
        errors.push(UrbanMapValidationError::new(
            format!("nodes[{index}].geo.lon_deg"),
            "lon_deg must be finite and within [-180, 180]",
        ));
    }
    if !geo.alt_m.is_finite() {
        errors.push(UrbanMapValidationError::new(
            format!("nodes[{index}].geo.alt_m"),
            "alt_m must be finite",
        ));
    }
}

fn validate_probability(field: &str, value: f64, errors: &mut Vec<UrbanMapValidationError>) {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        errors.push(UrbanMapValidationError::new(
            field,
            "probability must be finite and in [0, 1]",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, x: f64, y: f64) -> UrbanNode {
        UrbanNode {
            id: UrbanNodeId::from(id.to_owned()),
            pose: Pose {
                x,
                y,
                ..Default::default()
            },
            geo: None,
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

    fn map() -> UrbanMap {
        UrbanMap {
            nodes: vec![node("n0", 0.0, 0.0), node("n1", 10.0, 0.0)],
            edges: vec![edge("e0", "n0", "n1", 10.0)],
            static_obstacles: vec![],
        }
    }

    fn search_state() -> UrbanSearchState {
        UrbanSearchState {
            buses: vec![UrbanBus {
                id: UrbanBusId::from("bus-0".to_owned()),
                pose: Pose {
                    x: 5.0,
                    y: 0.0,
                    ..Default::default()
                },
                active_from_tick: Some(1),
                active_until_tick: Some(10),
                route: None,
            }],
            detector: UrbanDetectorConfig {
                detection_range_m: 4.0,
                detection_probability: 1.0,
                false_positive_rate: 0.0,
                seed: 7,
            },
        }
    }

    #[test]
    fn urban_map_validation_accepts_valid_map() {
        assert!(map().validate().is_empty());
    }

    #[test]
    fn urban_map_validation_rejects_duplicate_nodes() {
        let map = UrbanMap {
            nodes: vec![node("n0", 0.0, 0.0), node("n0", 1.0, 1.0)],
            edges: vec![edge("e0", "n0", "n0", 1.0)],
            static_obstacles: vec![],
        };
        let errors = map.validate();
        assert!(errors.iter().any(|error| error.field == "nodes[1].id"));
    }

    #[test]
    fn urban_map_validation_rejects_duplicate_edges() {
        let map = UrbanMap {
            nodes: vec![node("n0", 0.0, 0.0), node("n1", 1.0, 0.0)],
            edges: vec![edge("e0", "n0", "n1", 1.0), edge("e0", "n1", "n0", 1.0)],
            static_obstacles: vec![],
        };
        let errors = map.validate();
        assert!(errors.iter().any(|error| error.field == "edges[1].id"));
    }

    #[test]
    fn urban_map_validation_rejects_unknown_edge_endpoint() {
        let map = UrbanMap {
            nodes: vec![node("n0", 0.0, 0.0)],
            edges: vec![edge("e0", "n0", "missing", 1.0)],
            static_obstacles: vec![],
        };
        let errors = map.validate();
        assert!(errors.iter().any(|error| error.field == "edges[0].to"));
    }

    #[test]
    fn urban_map_validation_rejects_negative_edge_cost() {
        let map = UrbanMap {
            nodes: vec![node("n0", 0.0, 0.0), node("n1", 1.0, 0.0)],
            edges: vec![edge("e0", "n0", "n1", -1.0)],
            static_obstacles: vec![],
        };
        let errors = map.validate();
        assert!(errors.iter().any(|error| error.field == "edges[0].cost"));
    }

    #[test]
    fn urban_map_validation_rejects_invalid_aabb() {
        let map = UrbanMap {
            nodes: vec![node("n0", 0.0, 0.0), node("n1", 1.0, 0.0)],
            edges: vec![edge("e0", "n0", "n1", 1.0)],
            static_obstacles: vec![UrbanStaticObstacle {
                id: UrbanObstacleId::from("building".to_owned()),
                bounds: Aabb {
                    min_x: 2.0,
                    min_y: 0.0,
                    max_x: 1.0,
                    max_y: 1.0,
                },
                label: None,
            }],
        };
        let errors = map.validate();
        assert!(errors
            .iter()
            .any(|error| error.field == "static_obstacles[0].bounds"));
    }

    #[test]
    fn urban_route_loop_validation_rejects_unknown_node() {
        let errors = map().validate_route_loop(&UrbanRouteLoop {
            nodes: vec![
                UrbanNodeId::from("n0".to_owned()),
                UrbanNodeId::from("missing".to_owned()),
            ],
        });
        assert!(errors
            .iter()
            .any(|error| error.field == "route_loop.nodes[1]"));
    }

    #[test]
    fn urban_search_state_validation_accepts_valid_state() {
        assert!(search_state().validate().is_empty());
    }

    #[test]
    fn urban_search_state_serde_roundtrip() {
        let json = serde_json::to_string(&search_state()).unwrap();
        let parsed: UrbanSearchState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, search_state());
    }

    #[test]
    fn urban_search_state_validation_rejects_duplicate_bus_id() {
        let mut state = search_state();
        state.buses.push(state.buses[0].clone());

        let errors = state.validate();

        assert!(errors.iter().any(|error| error.field == "buses[1].id"));
    }

    #[test]
    fn urban_search_state_validation_rejects_invalid_bus_pose() {
        let mut state = search_state();
        state.buses[0].pose.x = f64::NAN;

        let errors = state.validate();

        assert!(errors.iter().any(|error| error.field == "buses[0].pose"));
    }

    #[test]
    fn urban_search_state_validation_rejects_invalid_active_window() {
        let mut state = search_state();
        state.buses[0].active_from_tick = Some(10);
        state.buses[0].active_until_tick = Some(1);

        let errors = state.validate();

        assert!(errors
            .iter()
            .any(|error| error.field == "buses[0].active_until_tick"));
    }

    #[test]
    fn bus_pose_at_tick_static_returns_fixed_pose() {
        let bus = &search_state().buses[0];

        let pose = bus.pose_at_tick(&map(), 5).unwrap();

        assert_eq!(pose.x, 5.0);
        assert_eq!(pose.y, 0.0);
    }

    #[test]
    fn bus_pose_at_tick_static_returns_none_outside_window() {
        let bus = &search_state().buses[0];

        assert!(bus.pose_at_tick(&map(), 0).is_none());
        assert!(bus.pose_at_tick(&map(), 11).is_none());
    }

    #[test]
    fn bus_pose_at_tick_interpolates_between_stops() {
        let mut bus = search_state().buses[0].clone();
        bus.route = Some(UrbanBusRoute {
            stops: vec![
                UrbanBusStop {
                    node_id: UrbanNodeId::from("n0".to_owned()),
                    arrival_tick: 10,
                },
                UrbanBusStop {
                    node_id: UrbanNodeId::from("n1".to_owned()),
                    arrival_tick: 20,
                },
            ],
            speed_m_per_tick: 1.0,
        });

        let pose = bus.pose_at_tick(&map(), 15).unwrap();

        assert_eq!(pose.x, 5.0);
        assert_eq!(pose.y, 0.0);
    }

    #[test]
    fn bus_pose_at_tick_returns_none_outside_route_window() {
        let mut bus = search_state().buses[0].clone();
        bus.route = Some(UrbanBusRoute {
            stops: vec![
                UrbanBusStop {
                    node_id: UrbanNodeId::from("n0".to_owned()),
                    arrival_tick: 10,
                },
                UrbanBusStop {
                    node_id: UrbanNodeId::from("n1".to_owned()),
                    arrival_tick: 20,
                },
            ],
            speed_m_per_tick: 1.0,
        });

        assert!(bus.pose_at_tick(&map(), 9).is_none());
        assert!(bus.pose_at_tick(&map(), 21).is_none());
    }

    #[test]
    fn urban_search_state_validation_rejects_unknown_bus_route_stop() {
        let mut state = search_state();
        state.buses[0].route = Some(UrbanBusRoute {
            stops: vec![UrbanBusStop {
                node_id: UrbanNodeId::from("missing".to_owned()),
                arrival_tick: 1,
            }],
            speed_m_per_tick: 1.0,
        });

        let errors = state.validate_with_map(&map());

        assert!(errors
            .iter()
            .any(|error| error.field == "buses[0].route.stops[0].node_id"));
    }

    #[test]
    fn urban_search_state_validation_rejects_non_monotonic_bus_route() {
        let mut state = search_state();
        state.buses[0].route = Some(UrbanBusRoute {
            stops: vec![
                UrbanBusStop {
                    node_id: UrbanNodeId::from("n0".to_owned()),
                    arrival_tick: 5,
                },
                UrbanBusStop {
                    node_id: UrbanNodeId::from("n1".to_owned()),
                    arrival_tick: 5,
                },
            ],
            speed_m_per_tick: 1.0,
        });

        let errors = state.validate_with_map(&map());

        assert!(errors
            .iter()
            .any(|error| error.field == "buses[0].route.stops[1].arrival_tick"));
    }

    #[test]
    fn temporary_obstacle_is_active_within_window() {
        let obstacle = UrbanTemporaryObstacle {
            edge_id: UrbanEdgeId::from("e0".to_owned()),
            appears_at_tick: 5,
            disappears_at_tick: Some(10),
            reason: None,
            severity: None,
        };
        assert!(!obstacle.is_active(4));
        assert!(obstacle.is_active(5));
        assert!(obstacle.is_active(9));
        assert!(!obstacle.is_active(10));
    }

    #[test]
    fn temporary_obstacle_no_disappears_stays_forever() {
        let obstacle = UrbanTemporaryObstacle {
            edge_id: UrbanEdgeId::from("e0".to_owned()),
            appears_at_tick: 3,
            disappears_at_tick: None,
            reason: None,
            severity: None,
        };
        assert!(!obstacle.is_active(2));
        assert!(obstacle.is_active(3));
        assert!(obstacle.is_active(u64::MAX));
    }

    #[test]
    fn temporary_obstacle_inactive_before_appears() {
        let obstacle = UrbanTemporaryObstacle {
            edge_id: UrbanEdgeId::from("e0".to_owned()),
            appears_at_tick: 100,
            disappears_at_tick: Some(200),
            reason: None,
            severity: None,
        };
        assert!(!obstacle.is_active(0));
        assert!(!obstacle.is_active(99));
        assert!(obstacle.is_active(100));
    }

    #[test]
    fn validate_temporary_obstacles_accepts_valid_obstacle() {
        let map = map();
        let obstacles = vec![UrbanTemporaryObstacle {
            edge_id: UrbanEdgeId::from("e0".to_owned()),
            appears_at_tick: 5,
            disappears_at_tick: Some(10),
            reason: None,
            severity: None,
        }];
        assert!(map.validate_temporary_obstacles(&obstacles).is_empty());
    }

    #[test]
    fn validate_temporary_obstacles_rejects_unknown_edge() {
        let map = map();
        let obstacles = vec![UrbanTemporaryObstacle {
            edge_id: UrbanEdgeId::from("no-such-edge".to_owned()),
            appears_at_tick: 1,
            disappears_at_tick: None,
            reason: None,
            severity: None,
        }];
        let errors = map.validate_temporary_obstacles(&obstacles);
        assert!(errors
            .iter()
            .any(|e| e.field == "temporary_obstacles[0].edge_id"));
    }

    #[test]
    fn validate_temporary_obstacles_rejects_inverted_window() {
        let map = map();
        let obstacles = vec![UrbanTemporaryObstacle {
            edge_id: UrbanEdgeId::from("e0".to_owned()),
            appears_at_tick: 10,
            disappears_at_tick: Some(5),
            reason: None,
            severity: None,
        }];
        let errors = map.validate_temporary_obstacles(&obstacles);
        assert!(errors
            .iter()
            .any(|e| e.field == "temporary_obstacles[0].disappears_at_tick"));
    }

    #[test]
    fn urban_blocked_policy_default_is_wait() {
        assert_eq!(UrbanBlockedPolicy::default(), UrbanBlockedPolicy::Wait);
    }

    #[test]
    fn urban_blocked_policy_serde_roundtrip() {
        let policy = UrbanBlockedPolicy::Replan;
        let json = serde_json::to_string(&policy).unwrap();
        let parsed: UrbanBlockedPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, policy);
    }

    #[test]
    fn urban_map_accepts_complete_geo_nodes() {
        let mut map = map();
        map.nodes[0].geo = Some(UrbanGeoPoint {
            lat_deg: 47.0,
            lon_deg: 8.0,
            alt_m: 5.0,
        });
        map.nodes[1].geo = Some(UrbanGeoPoint {
            lat_deg: 47.0001,
            lon_deg: 8.0001,
            alt_m: 5.0,
        });

        assert!(map.validate().is_empty());
    }

    #[test]
    fn urban_map_rejects_mixed_or_invalid_geo_nodes() {
        let mut map = map();
        map.nodes[0].geo = Some(UrbanGeoPoint {
            lat_deg: 91.0,
            lon_deg: 8.0,
            alt_m: 5.0,
        });

        let errors = map.validate();

        assert!(errors.iter().any(|error| error.field == "nodes[].geo"));
        assert!(errors
            .iter()
            .any(|error| error.field == "nodes[0].geo.lat_deg"));
    }

    #[test]
    fn urban_search_state_validation_rejects_invalid_detector_config() {
        let mut state = search_state();
        state.detector.detection_range_m = -1.0;
        state.detector.detection_probability = 1.1;
        state.detector.false_positive_rate = f64::INFINITY;

        let errors = state.validate();

        assert!(errors
            .iter()
            .any(|error| error.field == "detector.detection_range_m"));
        assert!(errors
            .iter()
            .any(|error| error.field == "detector.detection_probability"));
        assert!(errors
            .iter()
            .any(|error| error.field == "detector.false_positive_rate"));
    }
}
