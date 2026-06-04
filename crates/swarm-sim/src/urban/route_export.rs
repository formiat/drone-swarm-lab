use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};
use swarm_mission_ir::{
    GeoPosition, LocalPosition, MissionCommand, MissionWaypoint, Position, RouteId,
};
use swarm_types::{
    Pose, UrbanEdgeId, UrbanGeoPoint, UrbanMap, UrbanNodeId, UrbanPlannedRoute, UrbanRouteLoop,
};

use super::{expand_route_loop_with_planner_name, UrbanRouteError};

pub const DEFAULT_URBAN_ROUTE_ALTITUDE_M: f64 = 5.0;
pub const DEFAULT_URBAN_ROUTE_MAX_SPACING_M: f64 = 25.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrbanCoordinateMode {
    LocalWithOrigin,
    Wgs84NodeGeo,
}

impl UrbanCoordinateMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalWithOrigin => "local_with_origin",
            Self::Wgs84NodeGeo => "wgs84_node_geo",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UrbanRouteExportOptions {
    pub default_altitude_m: f64,
    pub max_spacing_m: f64,
    pub planner: String,
}

impl Default for UrbanRouteExportOptions {
    fn default() -> Self {
        Self {
            default_altitude_m: DEFAULT_URBAN_ROUTE_ALTITUDE_M,
            max_spacing_m: DEFAULT_URBAN_ROUTE_MAX_SPACING_M,
            planner: "dijkstra".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UrbanRouteWaypoint {
    pub seq: u16,
    pub task_id: String,
    pub pose: Pose,
    pub geo: Option<UrbanGeoPoint>,
    pub edge_id: UrbanEdgeId,
    pub from_node_id: UrbanNodeId,
    pub to_node_id: UrbanNodeId,
    pub segment_index: usize,
    pub point_index_on_segment: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UrbanRouteExportMetadata {
    pub route_length_m: f64,
    pub segment_count: usize,
    pub waypoint_count: usize,
    pub altitude_m: f64,
    pub altitude_source: String,
    pub spacing_m: f64,
    pub planner: String,
    pub coordinate_mode: UrbanCoordinateMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UrbanRouteExport {
    pub route: UrbanPlannedRoute,
    pub waypoints: Vec<UrbanRouteWaypoint>,
    pub metadata: UrbanRouteExportMetadata,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UrbanRouteExportError {
    InvalidOption { field: String, message: String },
    Route(UrbanRouteError),
    MissingNode { node_id: UrbanNodeId },
    MixedGeoNodes,
    TooManyWaypoints { count: usize },
}

impl fmt::Display for UrbanRouteExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOption { field, message } => write!(f, "[{field}] {message}"),
            Self::Route(error) => write!(f, "{error}"),
            Self::MissingNode { node_id } => {
                write!(f, "urban route references missing node '{node_id}'")
            }
            Self::MixedGeoNodes => write!(
                f,
                "urban route export requires either geo coordinates on every node or none"
            ),
            Self::TooManyWaypoints { count } => {
                write!(f, "urban route export produced too many waypoints: {count}")
            }
        }
    }
}

impl Error for UrbanRouteExportError {}

pub fn export_route_loop_to_waypoints(
    map: &UrbanMap,
    route_loop: &UrbanRouteLoop,
    options: &UrbanRouteExportOptions,
) -> Result<UrbanRouteExport, UrbanRouteExportError> {
    validate_options(options)?;
    let route = expand_route_loop_with_planner_name(map, route_loop, &options.planner)
        .map_err(UrbanRouteExportError::Route)?;
    export_planned_route_to_waypoints(map, route, options)
}

pub fn export_planned_route_to_waypoints(
    map: &UrbanMap,
    route: UrbanPlannedRoute,
    options: &UrbanRouteExportOptions,
) -> Result<UrbanRouteExport, UrbanRouteExportError> {
    validate_options(options)?;
    let coordinate_mode = coordinate_mode(map)?;
    let mut waypoints = Vec::new();

    for (segment_index, segment) in route.segments.iter().enumerate() {
        let from = node_pose(map, &segment.from)?;
        let to = node_pose(map, &segment.to)?;
        let interval_count = ((segment.length_m / options.max_spacing_m).ceil() as usize).max(1);

        let point_count = match coordinate_mode {
            UrbanCoordinateMode::LocalWithOrigin => interval_count,
            UrbanCoordinateMode::Wgs84NodeGeo => 1,
        };

        for point_index in 1..=point_count {
            let fraction = point_index as f64 / interval_count as f64;
            let (pose, geo) = match coordinate_mode {
                UrbanCoordinateMode::LocalWithOrigin => (
                    Pose {
                        x: interpolate(from.x, to.x, fraction),
                        y: interpolate(from.y, to.y, fraction),
                        z: options.default_altitude_m,
                    },
                    None,
                ),
                UrbanCoordinateMode::Wgs84NodeGeo => {
                    let mut geo = node_geo(map, &segment.to)?;
                    geo.alt_m = options.default_altitude_m;
                    (
                        Pose {
                            x: to.x,
                            y: to.y,
                            z: options.default_altitude_m,
                        },
                        Some(geo),
                    )
                }
            };
            let seq = u16::try_from(waypoints.len()).map_err(|_| {
                UrbanRouteExportError::TooManyWaypoints {
                    count: waypoints.len() + 1,
                }
            })?;
            waypoints.push(UrbanRouteWaypoint {
                seq,
                task_id: stable_task_id(segment_index, &segment.edge_id, point_index),
                pose,
                geo,
                edge_id: segment.edge_id.clone(),
                from_node_id: segment.from.clone(),
                to_node_id: segment.to.clone(),
                segment_index,
                point_index_on_segment: point_index,
            });
        }
    }

    let waypoint_count = waypoints.len();
    Ok(UrbanRouteExport {
        metadata: UrbanRouteExportMetadata {
            route_length_m: route.total_length_m,
            segment_count: route.segments.len(),
            waypoint_count,
            altitude_m: options.default_altitude_m,
            altitude_source: "urban_route_export.default_altitude_m".to_owned(),
            spacing_m: options.max_spacing_m,
            planner: options.planner.clone(),
            coordinate_mode,
        },
        route,
        waypoints,
    })
}

fn coordinate_mode(map: &UrbanMap) -> Result<UrbanCoordinateMode, UrbanRouteExportError> {
    let nodes_with_geo = map.nodes.iter().filter(|node| node.geo.is_some()).count();
    if nodes_with_geo == 0 {
        Ok(UrbanCoordinateMode::LocalWithOrigin)
    } else if nodes_with_geo == map.nodes.len() {
        Ok(UrbanCoordinateMode::Wgs84NodeGeo)
    } else {
        Err(UrbanRouteExportError::MixedGeoNodes)
    }
}

fn validate_options(options: &UrbanRouteExportOptions) -> Result<(), UrbanRouteExportError> {
    if !options.default_altitude_m.is_finite() {
        return Err(UrbanRouteExportError::InvalidOption {
            field: "default_altitude_m".to_owned(),
            message: "must be finite".to_owned(),
        });
    }
    if !options.max_spacing_m.is_finite() || options.max_spacing_m <= 0.0 {
        return Err(UrbanRouteExportError::InvalidOption {
            field: "max_spacing_m".to_owned(),
            message: "must be finite and greater than 0".to_owned(),
        });
    }
    if options.planner.trim().is_empty() {
        return Err(UrbanRouteExportError::InvalidOption {
            field: "planner".to_owned(),
            message: "must not be empty".to_owned(),
        });
    }
    Ok(())
}

fn node_pose(map: &UrbanMap, node_id: &UrbanNodeId) -> Result<Pose, UrbanRouteExportError> {
    map.nodes
        .iter()
        .find(|node| &node.id == node_id)
        .map(|node| node.pose)
        .ok_or_else(|| UrbanRouteExportError::MissingNode {
            node_id: node_id.clone(),
        })
}

fn node_geo(map: &UrbanMap, node_id: &UrbanNodeId) -> Result<UrbanGeoPoint, UrbanRouteExportError> {
    map.nodes
        .iter()
        .find(|node| &node.id == node_id)
        .and_then(|node| node.geo)
        .ok_or_else(|| UrbanRouteExportError::MissingNode {
            node_id: node_id.clone(),
        })
}

fn interpolate(from: f64, to: f64, fraction: f64) -> f64 {
    from + (to - from) * fraction
}

fn stable_task_id(segment_index: usize, edge_id: &UrbanEdgeId, point_index: usize) -> String {
    let edge_id = edge_id.as_ref();
    format!("urban-route-{segment_index}-{edge_id}-{point_index}")
}

/// Converts a planned Urban route into a hardware-agnostic `MissionCommand::FollowRoute`.
///
/// Each segment's destination node becomes a `MissionWaypoint`. Maps where all
/// nodes carry `UrbanGeoPoint` values produce WGS84 waypoints; local maps keep
/// the simulation pose frame. Segments whose destination node is absent from the
/// map are silently skipped.
///
/// Returns `None` when the route has no segments, or when no destination nodes
/// could be resolved (resulting in an empty waypoint list).
pub fn urban_route_to_follow_route(
    map: &UrbanMap,
    route: &UrbanPlannedRoute,
    route_id: RouteId,
    altitude_m: f64,
) -> Option<MissionCommand> {
    if route.segments.is_empty() {
        return None;
    }
    let waypoints: Vec<MissionWaypoint> = route
        .segments
        .iter()
        .filter_map(|seg| {
            map.nodes
                .iter()
                .find(|n| n.id == seg.to)
                .map(|node| MissionWaypoint {
                    position: node.geo.map_or_else(
                        || {
                            Position::Local(LocalPosition {
                                x_m: node.pose.x,
                                y_m: node.pose.y,
                                z_m: altitude_m,
                            })
                        },
                        |geo| {
                            Position::Geo(GeoPosition {
                                lat_deg: geo.lat_deg,
                                lon_deg: geo.lon_deg,
                                alt_m: altitude_m,
                            })
                        },
                    ),
                    acceptance_radius_m: None,
                })
        })
        .collect();

    if waypoints.is_empty() {
        return None;
    }

    Some(MissionCommand::FollowRoute {
        route_id,
        waypoints,
    })
}
