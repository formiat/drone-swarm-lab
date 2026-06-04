use serde::{Deserialize, Serialize};

/// Geographic position in WGS84 geodetic coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeoPosition {
    /// Latitude in decimal degrees.
    pub lat_deg: f64,
    /// Longitude in decimal degrees.
    pub lon_deg: f64,
    /// Altitude in metres (reference defined by `AltitudeReference` in the plan).
    pub alt_m: f64,
}

/// Local position in metric units relative to a reference origin.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalPosition {
    /// X offset in metres (East in ENU, North in NED).
    pub x_m: f64,
    /// Y offset in metres (North in ENU, East in NED).
    pub y_m: f64,
    /// Z offset in metres (Up in ENU, negative-Down in NED).
    pub z_m: f64,
}

/// A position that is either geodetic (WGS84) or local (simulation frame).
///
/// Use the `kind` discriminant to distinguish at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Position {
    Geo(GeoPosition),
    Local(LocalPosition),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geo_position_roundtrip() {
        let pos = Position::Geo(GeoPosition {
            lat_deg: 47.397_742,
            lon_deg: 8.545_594,
            alt_m: 10.0,
        });
        let json = serde_json::to_string(&pos).unwrap();
        let back: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(pos, back);
    }

    #[test]
    fn local_position_roundtrip() {
        let pos = Position::Local(LocalPosition {
            x_m: 1.0,
            y_m: 2.0,
            z_m: 3.0,
        });
        let json = serde_json::to_string(&pos).unwrap();
        let back: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(pos, back);
    }

    #[test]
    fn position_tag_is_deterministic() {
        let json = serde_json::to_string(&Position::Local(LocalPosition {
            x_m: 0.0,
            y_m: 0.0,
            z_m: 0.0,
        }))
        .unwrap();
        assert!(json.contains("\"kind\":\"local\""), "json={json}");
    }
}
