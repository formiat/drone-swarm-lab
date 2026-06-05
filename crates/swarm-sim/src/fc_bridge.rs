use std::fmt;

use swarm_comms::{
    local_to_mavlink_int, FcGeofenceItem, FcGeofenceItemKind, FcGeofenceShape,
    MavlinkCoordinateError, MavlinkCoordinateOrigin, MavlinkFencePlan,
};
use swarm_safety::{Aabb, SafetyConfig};

/// Error raised while converting simulation safety constraints into FC-facing plans.
#[derive(Debug, Clone, PartialEq)]
pub enum FcBridgeError {
    /// Local metres could not be converted to MAVLink global-int coordinates.
    CoordinateConversion { source: MavlinkCoordinateError },
}

impl fmt::Display for FcBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinateConversion { source } => {
                write!(f, "coordinate conversion failed: {source}")
            }
        }
    }
}

impl std::error::Error for FcBridgeError {}

impl From<MavlinkCoordinateError> for FcBridgeError {
    fn from(source: MavlinkCoordinateError) -> Self {
        Self::CoordinateConversion { source }
    }
}

/// Convert simulation `SafetyConfig` AABB fences into an FC geofence plan.
///
/// This bridge is intentionally narrow: it preserves the existing preflight
/// AABB model and emits MAVLink fence polygons without introducing a
/// `swarm-comms -> swarm-safety` dependency.
pub fn safety_config_to_fence_plan(
    config: &SafetyConfig,
    origin: MavlinkCoordinateOrigin,
    enable_fence: bool,
) -> Result<MavlinkFencePlan, FcBridgeError> {
    let mut items = Vec::new();
    if let Some(geofence) = &config.geofence {
        items.push(aabb_to_fence_item(
            "safety-geofence",
            FcGeofenceItemKind::PolygonInclusion,
            &geofence.bounds,
            origin,
        )?);
    }
    for (index, no_fly_zone) in config.no_fly_zones.iter().enumerate() {
        items.push(aabb_to_fence_item(
            format!("safety-nofly-{index}"),
            FcGeofenceItemKind::PolygonExclusion,
            &no_fly_zone.bounds,
            origin,
        )?);
    }
    Ok(MavlinkFencePlan {
        items,
        enable_fence,
    })
}

fn aabb_to_fence_item(
    id: impl Into<String>,
    kind: FcGeofenceItemKind,
    bounds: &Aabb,
    origin: MavlinkCoordinateOrigin,
) -> Result<FcGeofenceItem, FcBridgeError> {
    let corners = [
        (bounds.min_x, bounds.min_y),
        (bounds.max_x, bounds.min_y),
        (bounds.max_x, bounds.max_y),
        (bounds.min_x, bounds.max_y),
    ];
    let mut vertices = Vec::with_capacity(corners.len());
    for (east_m, north_m) in corners {
        let coordinate = local_to_mavlink_int(east_m, north_m, 0.0, origin)?;
        vertices.push((coordinate.lat_e7, coordinate.lon_e7));
    }
    Ok(FcGeofenceItem {
        id: id.into(),
        kind,
        shape: FcGeofenceShape::Polygon { vertices },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_safety::{Geofence, NoFlyZone};

    fn origin() -> MavlinkCoordinateOrigin {
        MavlinkCoordinateOrigin {
            lat_deg: 47.397_742,
            lon_deg: 8.545_594,
            alt_m: 0.0,
        }
    }

    fn bounds(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Aabb {
        Aabb {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    #[test]
    fn safety_config_converts_geofence_and_no_fly_zones() {
        let config = SafetyConfig {
            geofence: Some(Geofence {
                bounds: bounds(0.0, 0.0, 10.0, 10.0),
            }),
            no_fly_zones: vec![NoFlyZone {
                bounds: bounds(2.0, 2.0, 4.0, 4.0),
                active_from_tick: None,
                active_until_tick: None,
            }],
            separation: None,
            max_altitude_m: None,
            min_altitude_m: None,
            max_route_length_m: None,
            max_duration_ticks: None,
        };

        let plan = safety_config_to_fence_plan(&config, origin(), true).unwrap();

        assert!(plan.enable_fence);
        assert_eq!(plan.items.len(), 2);
        assert_eq!(plan.items[0].id, "safety-geofence");
        assert_eq!(plan.items[0].kind, FcGeofenceItemKind::PolygonInclusion);
        assert_eq!(plan.items[1].id, "safety-nofly-0");
        assert_eq!(plan.items[1].kind, FcGeofenceItemKind::PolygonExclusion);
        let FcGeofenceShape::Polygon { vertices } = &plan.items[0].shape else {
            panic!("expected polygon");
        };
        assert_eq!(vertices.len(), 4);
    }

    #[test]
    fn empty_safety_config_converts_to_empty_fence_plan() {
        let plan = safety_config_to_fence_plan(&SafetyConfig::default(), origin(), false).unwrap();

        assert!(!plan.enable_fence);
        assert!(plan.items.is_empty());
    }
}
