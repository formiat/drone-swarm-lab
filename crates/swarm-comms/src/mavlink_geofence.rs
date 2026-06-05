use serde::{Deserialize, Serialize};

use crate::mavlink_capability_profile::{
    fence_item_support_rule, MavlinkCapabilityProfile, MavlinkCapabilityProfileId,
    MavlinkCompatibilityClass,
};
use crate::mavlink_common_plan::{
    MavlinkCommonCommand, MavlinkCommonCommandName, MavlinkCommonMissionItem, MavlinkPlanPhase,
};

const MAVLINK_FENCE_FRAME: &str = "MAV_FRAME_GLOBAL";
const MAVLINK_POLYGON_VERTEX_LIMIT: usize = 70;

/// Kind of a single FC geofence item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FcGeofenceItemKind {
    CircleInclusion,
    CircleExclusion,
    PolygonInclusion,
    PolygonExclusion,
}

/// FC geofence item shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FcGeofenceShape {
    Circle {
        center_lat_e7: i32,
        center_lon_e7: i32,
        radius_m: f64,
    },
    /// value: `(lat_e7, lon_e7)` per vertex
    Polygon { vertices: Vec<(i32, i32)> },
}

/// One geofence item for FC upload planning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcGeofenceItem {
    pub id: String,
    pub kind: FcGeofenceItemKind,
    pub shape: FcGeofenceShape,
}

/// Compiled fence plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MavlinkFencePlan {
    pub items: Vec<FcGeofenceItem>,
    pub enable_fence: bool,
}

/// Human-readable fence summary in a dry-run artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MavlinkFenceArtifact {
    pub item_count: usize,
    pub inclusion_count: usize,
    pub exclusion_count: usize,
    pub has_polygon: bool,
    pub has_circle: bool,
    pub profile_classification: MavlinkCompatibilityClass,
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum FenceCompilerError {
    #[error("fence item kind '{kind:?}' is not supported by profile '{profile}'")]
    UnsupportedByProfile {
        kind: FcGeofenceItemKind,
        profile: MavlinkCapabilityProfileId,
    },
    #[error("polygon fence requires at least 3 vertices, got {count}")]
    PolygonTooFewVertices { count: usize },
    #[error("polygon fence exceeds MAVLink limit of {limit} vertices, got {count}")]
    PolygonTooManyVertices { count: usize, limit: usize },
    #[error("fence item '{id}' contains non-finite coordinate")]
    NonFiniteCoordinate { id: String },
}

/// Compile FcGeofenceItems to MavlinkCommonMissionItems.
pub fn compile_fence_items(
    plan: &MavlinkFencePlan,
    profile: &MavlinkCapabilityProfile,
) -> Result<(Vec<MavlinkCommonMissionItem>, Option<MavlinkCommonCommand>), FenceCompilerError> {
    let mut items = Vec::new();
    for item in &plan.items {
        ensure_supported(item.kind, profile)?;
        match &item.shape {
            FcGeofenceShape::Circle {
                center_lat_e7,
                center_lon_e7,
                radius_m,
            } => {
                if !radius_m.is_finite() {
                    return Err(FenceCompilerError::NonFiniteCoordinate {
                        id: item.id.clone(),
                    });
                }
                let seq = items.len() as u16;
                items.push(MavlinkCommonMissionItem {
                    seq,
                    command_id: format!("fence:{}:{seq}", item.id),
                    command: fence_command(item.kind),
                    frame: MAVLINK_FENCE_FRAME.to_owned(),
                    lat_e7: *center_lat_e7,
                    lon_e7: *center_lon_e7,
                    relative_alt_m: 0.0,
                    params: [Some(*radius_m), None, None, None],
                    current: false,
                    autocontinue: false,
                    source_task_id: None,
                    source_route_id: None,
                });
            }
            FcGeofenceShape::Polygon { vertices } => {
                if vertices.len() < 3 {
                    return Err(FenceCompilerError::PolygonTooFewVertices {
                        count: vertices.len(),
                    });
                }
                if vertices.len() > MAVLINK_POLYGON_VERTEX_LIMIT {
                    return Err(FenceCompilerError::PolygonTooManyVertices {
                        count: vertices.len(),
                        limit: MAVLINK_POLYGON_VERTEX_LIMIT,
                    });
                }
                for (lat_e7, lon_e7) in vertices {
                    let seq = items.len() as u16;
                    items.push(MavlinkCommonMissionItem {
                        seq,
                        command_id: format!("fence:{}:{seq}", item.id),
                        command: fence_command(item.kind),
                        frame: MAVLINK_FENCE_FRAME.to_owned(),
                        lat_e7: *lat_e7,
                        lon_e7: *lon_e7,
                        relative_alt_m: 0.0,
                        params: [Some(vertices.len() as f64), None, None, None],
                        current: false,
                        autocontinue: false,
                        source_task_id: None,
                        source_route_id: None,
                    });
                }
            }
        }
    }
    let enable = plan.enable_fence.then_some(MavlinkCommonCommand {
        command_id: "fence-enable-0".to_owned(),
        command: MavlinkCommonCommandName::DoFenceEnable,
        phase: MavlinkPlanPhase::CommandPrelude,
        params: [
            Some(1.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
        ],
    });
    Ok((items, enable))
}

/// Build a fence artifact summary from a compiled fence plan.
pub fn fence_artifact(
    plan: &MavlinkFencePlan,
    profile: &MavlinkCapabilityProfile,
) -> MavlinkFenceArtifact {
    let mut profile_classification = profile.geofence_support;
    let mut caveats = Vec::new();
    for item in &plan.items {
        if let Some(rule) = fence_item_support_rule(profile, item.kind) {
            profile_classification = profile_classification.max(rule.classification);
            for caveat in rule.caveats {
                if !caveats.iter().any(|existing| existing == caveat) {
                    caveats.push((*caveat).to_owned());
                }
            }
        }
    }
    MavlinkFenceArtifact {
        item_count: plan.items.len(),
        inclusion_count: plan
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item.kind,
                    FcGeofenceItemKind::CircleInclusion | FcGeofenceItemKind::PolygonInclusion
                )
            })
            .count(),
        exclusion_count: plan
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item.kind,
                    FcGeofenceItemKind::CircleExclusion | FcGeofenceItemKind::PolygonExclusion
                )
            })
            .count(),
        has_polygon: plan
            .items
            .iter()
            .any(|item| matches!(item.shape, FcGeofenceShape::Polygon { .. })),
        has_circle: plan
            .items
            .iter()
            .any(|item| matches!(item.shape, FcGeofenceShape::Circle { .. })),
        profile_classification,
        caveats,
    }
}

fn ensure_supported(
    kind: FcGeofenceItemKind,
    profile: &MavlinkCapabilityProfile,
) -> Result<(), FenceCompilerError> {
    let Some(rule) = fence_item_support_rule(profile, kind) else {
        return Err(FenceCompilerError::UnsupportedByProfile {
            kind,
            profile: profile.id,
        });
    };
    if rule.classification.blocks_hardware_facing_success() {
        Err(FenceCompilerError::UnsupportedByProfile {
            kind,
            profile: profile.id,
        })
    } else {
        Ok(())
    }
}

fn fence_command(kind: FcGeofenceItemKind) -> MavlinkCommonCommandName {
    match kind {
        FcGeofenceItemKind::CircleInclusion => MavlinkCommonCommandName::FenceCircleInclusion,
        FcGeofenceItemKind::CircleExclusion => MavlinkCommonCommandName::FenceCircleExclusion,
        FcGeofenceItemKind::PolygonInclusion => {
            MavlinkCommonCommandName::FencePolygonVertexInclusion
        }
        FcGeofenceItemKind::PolygonExclusion => {
            MavlinkCommonCommandName::FencePolygonVertexExclusion
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mavlink_capability_profile::MavlinkCapabilityProfileId;

    fn polygon_item(vertices: Vec<(i32, i32)>) -> FcGeofenceItem {
        FcGeofenceItem {
            id: "poly".to_owned(),
            kind: FcGeofenceItemKind::PolygonInclusion,
            shape: FcGeofenceShape::Polygon { vertices },
        }
    }

    fn generic_profile() -> &'static MavlinkCapabilityProfile {
        MavlinkCapabilityProfileId::MavlinkCommonGeneric.profile()
    }

    #[test]
    fn circular_fence_compiles_to_expected_item() {
        let plan = MavlinkFencePlan {
            items: vec![FcGeofenceItem {
                id: "circle".to_owned(),
                kind: FcGeofenceItemKind::CircleInclusion,
                shape: FcGeofenceShape::Circle {
                    center_lat_e7: 473_977_420,
                    center_lon_e7: 85_455_940,
                    radius_m: 25.0,
                },
            }],
            enable_fence: false,
        };

        let (items, enable) = compile_fence_items(&plan, generic_profile()).unwrap();

        assert!(enable.is_none());
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].command,
            MavlinkCommonCommandName::FenceCircleInclusion
        );
        assert_eq!(items[0].params[0], Some(25.0));
        assert_eq!(items[0].lat_e7, 473_977_420);
    }

    #[test]
    fn polygon_fence_compiles_to_n_vertex_items() {
        let plan = MavlinkFencePlan {
            items: vec![polygon_item(vec![(1, 2), (3, 4), (5, 6), (7, 8)])],
            enable_fence: false,
        };

        let (items, _) = compile_fence_items(&plan, generic_profile()).unwrap();

        assert_eq!(items.len(), 4);
        assert!(items.iter().all(|item| {
            item.command == MavlinkCommonCommandName::FencePolygonVertexInclusion
                && item.params[0] == Some(4.0)
        }));
    }

    #[test]
    fn fence_enable_command_present() {
        let plan = MavlinkFencePlan {
            items: vec![polygon_item(vec![(1, 2), (3, 4), (5, 6)])],
            enable_fence: true,
        };

        let (_, enable) = compile_fence_items(&plan, generic_profile()).unwrap();

        assert_eq!(
            enable.map(|command| (command.command, command.params[0])),
            Some((MavlinkCommonCommandName::DoFenceEnable, Some(1.0)))
        );
    }

    #[test]
    fn fence_enable_absent_when_disabled() {
        let plan = MavlinkFencePlan {
            items: vec![polygon_item(vec![(1, 2), (3, 4), (5, 6)])],
            enable_fence: false,
        };

        assert!(compile_fence_items(&plan, generic_profile())
            .unwrap()
            .1
            .is_none());
    }

    #[test]
    fn polygon_too_few_vertices_returns_error() {
        let plan = MavlinkFencePlan {
            items: vec![polygon_item(vec![(1, 2), (3, 4)])],
            enable_fence: false,
        };

        assert!(matches!(
            compile_fence_items(&plan, generic_profile()),
            Err(FenceCompilerError::PolygonTooFewVertices { count: 2 })
        ));
    }

    #[test]
    fn polygon_too_many_vertices_returns_error() {
        let plan = MavlinkFencePlan {
            items: vec![polygon_item((0..=70).map(|value| (value, value)).collect())],
            enable_fence: false,
        };

        assert!(matches!(
            compile_fence_items(&plan, generic_profile()),
            Err(FenceCompilerError::PolygonTooManyVertices { count: 71, .. })
        ));
    }

    #[test]
    fn unsupported_profile_returns_structured_error() {
        let plan = MavlinkFencePlan {
            items: vec![FcGeofenceItem {
                id: "circle".to_owned(),
                kind: FcGeofenceItemKind::CircleInclusion,
                shape: FcGeofenceShape::Circle {
                    center_lat_e7: 1,
                    center_lon_e7: 2,
                    radius_m: 3.0,
                },
            }],
            enable_fence: false,
        };

        assert!(matches!(
            compile_fence_items(&plan, MavlinkCapabilityProfileId::Px4.profile()),
            Err(FenceCompilerError::UnsupportedByProfile {
                kind: FcGeofenceItemKind::CircleInclusion,
                profile: MavlinkCapabilityProfileId::Px4
            })
        ));
    }
}
