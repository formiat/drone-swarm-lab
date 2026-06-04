use serde::{Deserialize, Serialize};

use crate::position::Position;

/// A single waypoint within a `follow_route` or similar command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissionWaypoint {
    /// Target position for this waypoint.
    pub position: Position,
    /// Optional per-waypoint acceptance radius override (metres).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_radius_m: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::LocalPosition;

    #[test]
    fn mission_waypoint_roundtrip() {
        let wp = MissionWaypoint {
            position: Position::Local(LocalPosition {
                x_m: 10.0,
                y_m: 20.0,
                z_m: 5.0,
            }),
            acceptance_radius_m: Some(1.0),
        };
        let json = serde_json::to_string(&wp).unwrap();
        let back: MissionWaypoint = serde_json::from_str(&json).unwrap();
        assert_eq!(wp, back);
    }

    #[test]
    fn mission_waypoint_no_radius_omitted() {
        let wp = MissionWaypoint {
            position: Position::Local(LocalPosition {
                x_m: 0.0,
                y_m: 0.0,
                z_m: 0.0,
            }),
            acceptance_radius_m: None,
        };
        let json = serde_json::to_string(&wp).unwrap();
        assert!(!json.contains("acceptance_radius_m"), "json={json}");
    }
}
