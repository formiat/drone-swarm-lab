use std::error::Error;
use std::fmt;

use serde_json::Value;
use swarm_types::{Pose, UrbanEdge, UrbanEdgeId, UrbanGeoPoint, UrbanMap, UrbanNode, UrbanNodeId};

#[derive(Clone, Debug, PartialEq)]
pub struct UrbanGeoJsonImportOptions {
    pub default_altitude_m: f64,
    pub bidirectional_edges: bool,
}

impl Default for UrbanGeoJsonImportOptions {
    fn default() -> Self {
        Self {
            default_altitude_m: 5.0,
            bidirectional_edges: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UrbanGeoJsonImportError {
    Parse(String),
    Invalid { field: String, message: String },
}

impl fmt::Display for UrbanGeoJsonImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(message) => write!(f, "GeoJSON parse failed: {message}"),
            Self::Invalid { field, message } => write!(f, "[{field}] {message}"),
        }
    }
}

impl Error for UrbanGeoJsonImportError {}

pub fn import_urban_map_from_geojson_str(
    input: &str,
    options: &UrbanGeoJsonImportOptions,
) -> Result<UrbanMap, UrbanGeoJsonImportError> {
    if !options.default_altitude_m.is_finite() {
        return invalid("default_altitude_m", "must be finite");
    }
    let root: Value = serde_json::from_str(input)
        .map_err(|error| UrbanGeoJsonImportError::Parse(error.to_string()))?;
    let features = root
        .get("features")
        .and_then(Value::as_array)
        .ok_or_else(|| UrbanGeoJsonImportError::Invalid {
            field: "features".to_owned(),
            message: "FeatureCollection.features array is required".to_owned(),
        })?;

    let mut point_specs = Vec::new();
    let mut edge_specs = Vec::new();
    for (index, feature) in features.iter().enumerate() {
        let geometry = feature
            .get("geometry")
            .ok_or_else(|| UrbanGeoJsonImportError::Invalid {
                field: format!("features[{index}].geometry"),
                message: "geometry is required".to_owned(),
            })?;
        let geometry_type = geometry
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| UrbanGeoJsonImportError::Invalid {
                field: format!("features[{index}].geometry.type"),
                message: "geometry type is required".to_owned(),
            })?;
        match geometry_type {
            "Point" => point_specs.push(parse_point_feature(index, feature, options)?),
            "LineString" => edge_specs.push(parse_edge_feature(index, feature)?),
            other => {
                return invalid(
                    format!("features[{index}].geometry.type"),
                    format!("unsupported geometry type '{other}'"),
                );
            }
        }
    }

    let origin = point_specs
        .first()
        .ok_or_else(|| UrbanGeoJsonImportError::Invalid {
            field: "features".to_owned(),
            message: "at least one Point feature is required".to_owned(),
        })?
        .geo;

    let nodes: Vec<UrbanNode> = point_specs
        .into_iter()
        .map(|point| UrbanNode {
            id: UrbanNodeId::from(point.id),
            pose: local_pose(origin, point.geo),
            geo: Some(point.geo),
        })
        .collect();

    let mut edges = Vec::new();
    for edge in edge_specs {
        let length_m = edge.length_m.unwrap_or_else(|| {
            node_pose_distance(&nodes, &edge.from, &edge.to).unwrap_or(edge.cost.unwrap_or(0.0))
        });
        let cost = edge.cost.unwrap_or(length_m);
        edges.push(UrbanEdge {
            id: UrbanEdgeId::from(edge.id.clone()),
            from: UrbanNodeId::from(edge.from.clone()),
            to: UrbanNodeId::from(edge.to.clone()),
            cost,
            length_m,
            corridor_width_m: edge.corridor_width_m,
            blocked: edge.blocked,
        });
        if options.bidirectional_edges {
            edges.push(UrbanEdge {
                id: UrbanEdgeId::from(format!("{}-reverse", edge.id)),
                from: UrbanNodeId::from(edge.to),
                to: UrbanNodeId::from(edge.from),
                cost,
                length_m,
                corridor_width_m: edge.corridor_width_m,
                blocked: edge.blocked,
            });
        }
    }

    let map = UrbanMap {
        nodes,
        edges,
        static_obstacles: Vec::new(),
    };
    let validation_errors = map.validate();
    if let Some(error) = validation_errors.first() {
        return invalid(error.field.clone(), error.message.clone());
    }
    Ok(map)
}

#[derive(Clone, Debug)]
struct PointSpec {
    id: String,
    geo: UrbanGeoPoint,
}

#[derive(Clone, Debug)]
struct EdgeSpec {
    id: String,
    from: String,
    to: String,
    cost: Option<f64>,
    length_m: Option<f64>,
    corridor_width_m: Option<f64>,
    blocked: bool,
}

fn parse_point_feature(
    index: usize,
    feature: &Value,
    options: &UrbanGeoJsonImportOptions,
) -> Result<PointSpec, UrbanGeoJsonImportError> {
    let id = required_prop_str(feature, index, "id")?;
    let coords = feature
        .pointer("/geometry/coordinates")
        .and_then(Value::as_array)
        .ok_or_else(|| UrbanGeoJsonImportError::Invalid {
            field: format!("features[{index}].geometry.coordinates"),
            message: "Point coordinates array is required".to_owned(),
        })?;
    if coords.len() < 2 {
        return invalid(
            format!("features[{index}].geometry.coordinates"),
            "Point coordinates must contain [lon, lat] or [lon, lat, alt]",
        );
    }
    let lon_deg = number(
        coords.first(),
        format!("features[{index}].geometry.coordinates[0]"),
    )?;
    let lat_deg = number(
        coords.get(1),
        format!("features[{index}].geometry.coordinates[1]"),
    )?;
    let alt_m = coords
        .get(2)
        .map(|value| {
            number(
                Some(value),
                format!("features[{index}].geometry.coordinates[2]"),
            )
        })
        .transpose()?
        .unwrap_or(options.default_altitude_m);
    Ok(PointSpec {
        id,
        geo: UrbanGeoPoint {
            lat_deg,
            lon_deg,
            alt_m,
        },
    })
}

fn parse_edge_feature(index: usize, feature: &Value) -> Result<EdgeSpec, UrbanGeoJsonImportError> {
    Ok(EdgeSpec {
        id: required_prop_str(feature, index, "id")?,
        from: required_prop_str(feature, index, "from")?,
        to: required_prop_str(feature, index, "to")?,
        cost: optional_prop_f64(feature, index, "cost")?,
        length_m: optional_prop_f64(feature, index, "length_m")?,
        corridor_width_m: optional_prop_f64(feature, index, "corridor_width_m")?,
        blocked: feature
            .pointer("/properties/blocked")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn required_prop_str(
    feature: &Value,
    index: usize,
    key: &str,
) -> Result<String, UrbanGeoJsonImportError> {
    feature
        .pointer(&format!("/properties/{key}"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| UrbanGeoJsonImportError::Invalid {
            field: format!("features[{index}].properties.{key}"),
            message: "non-empty string is required".to_owned(),
        })
}

fn optional_prop_f64(
    feature: &Value,
    index: usize,
    key: &str,
) -> Result<Option<f64>, UrbanGeoJsonImportError> {
    feature
        .pointer(&format!("/properties/{key}"))
        .map(|value| number(Some(value), format!("features[{index}].properties.{key}")))
        .transpose()
}

fn number(value: Option<&Value>, field: String) -> Result<f64, UrbanGeoJsonImportError> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| UrbanGeoJsonImportError::Invalid {
            field,
            message: "finite number is required".to_owned(),
        })
}

fn local_pose(origin: UrbanGeoPoint, point: UrbanGeoPoint) -> Pose {
    let meters_per_lon_degree = 111_320.0 * origin.lat_deg.to_radians().cos();
    Pose {
        x: (point.lon_deg - origin.lon_deg) * meters_per_lon_degree,
        y: (point.lat_deg - origin.lat_deg) * 111_320.0,
        z: point.alt_m,
    }
}

fn node_pose_distance(nodes: &[UrbanNode], from: &str, to: &str) -> Option<f64> {
    let from = nodes.iter().find(|node| node.id.as_ref() == from)?.pose;
    let to = nodes.iter().find(|node| node.id.as_ref() == to)?.pose;
    Some(((to.x - from.x).powi(2) + (to.y - from.y).powi(2) + (to.z - from.z).powi(2)).sqrt())
}

fn invalid<T>(
    field: impl Into<String>,
    message: impl Into<String>,
) -> Result<T, UrbanGeoJsonImportError> {
    Err(UrbanGeoJsonImportError::Invalid {
        field: field.into(),
        message: message.into(),
    })
}
