use serde::{Deserialize, Serialize};

/// Coordinate reference frame used by positions in this mission plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateFrame {
    /// WGS84 geodetic coordinates (latitude / longitude / altitude).
    Wgs84,
    /// Local North-East-Down frame relative to a reference origin.
    LocalNed,
    /// Local East-North-Up frame relative to a reference origin.
    LocalEnu,
}

/// Reference datum for altitude values in this mission plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AltitudeReference {
    /// Above mean sea level (AMSL).
    Amsl,
    /// Above ground level (AGL).
    Agl,
    /// Relative to the takeoff / home position.
    RelativeHome,
    /// WGS84 ellipsoid height.
    Ellipsoid,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_frame_roundtrip() {
        for frame in [
            CoordinateFrame::Wgs84,
            CoordinateFrame::LocalNed,
            CoordinateFrame::LocalEnu,
        ] {
            let json = serde_json::to_string(&frame).unwrap();
            let back: CoordinateFrame = serde_json::from_str(&json).unwrap();
            assert_eq!(frame, back);
        }
    }

    #[test]
    fn altitude_reference_roundtrip() {
        for alt_ref in [
            AltitudeReference::Amsl,
            AltitudeReference::Agl,
            AltitudeReference::RelativeHome,
            AltitudeReference::Ellipsoid,
        ] {
            let json = serde_json::to_string(&alt_ref).unwrap();
            let back: AltitudeReference = serde_json::from_str(&json).unwrap();
            assert_eq!(alt_ref, back);
        }
    }

    #[test]
    fn coordinate_frame_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&CoordinateFrame::LocalNed).unwrap(),
            "\"local_ned\""
        );
        assert_eq!(
            serde_json::to_string(&CoordinateFrame::LocalEnu).unwrap(),
            "\"local_enu\""
        );
        assert_eq!(
            serde_json::to_string(&CoordinateFrame::Wgs84).unwrap(),
            "\"wgs84\""
        );
    }
}
