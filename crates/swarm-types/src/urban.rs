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

/// An intersection or waypoint node in a road graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanNode {
    pub id: UrbanNodeId,
    pub pose: Pose,
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
        for (index, node) in self.nodes.iter().enumerate() {
            if !node_ids.insert(node.id.clone()) {
                errors.push(UrbanMapValidationError::new(
                    format!("nodes[{index}].id"),
                    format!("Duplicate urban node id '{}'", node.id),
                ));
            }
            validate_pose(index, node.pose, &mut errors);
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

fn validate_pose(index: usize, pose: Pose, errors: &mut Vec<UrbanMapValidationError>) {
    if !pose.x.is_finite() || !pose.y.is_finite() || !pose.z.is_finite() {
        errors.push(UrbanMapValidationError::new(
            format!("nodes[{index}].pose"),
            "Node pose coordinates must be finite",
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
