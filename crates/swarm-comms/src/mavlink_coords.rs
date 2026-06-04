/// Geographic origin used to convert local metres into MAVLink global-int fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MavlinkCoordinateOrigin {
    /// WGS84 latitude in decimal degrees.
    pub lat_deg: f64,
    /// WGS84 longitude in decimal degrees.
    pub lon_deg: f64,
    /// Origin altitude in metres. M81 currently emits relative-altitude items.
    pub alt_m: f64,
}

/// MAVLink global-int coordinate fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MavlinkIntCoordinate {
    /// Latitude scaled by 1e7.
    pub lat_e7: i32,
    /// Longitude scaled by 1e7.
    pub lon_e7: i32,
    /// Relative altitude in metres.
    pub relative_alt_m: f32,
}

/// Coordinate conversion failures shared by transport-free and transport layers.
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum MavlinkCoordinateError {
    /// A required numeric field was NaN or infinite.
    #[error("{label} must be finite")]
    NonFinite {
        /// Field label.
        label: &'static str,
    },
    /// Latitude conversion produced a value outside WGS84 bounds.
    #[error("latitude out of range after local conversion: {lat_deg}")]
    LatitudeOutOfRange {
        /// Converted latitude.
        lat_deg: f64,
    },
    /// Longitude conversion is unstable because the origin is too close to a pole.
    #[error("longitude conversion is unstable near the poles")]
    LongitudeUnstableNearPole,
    /// Longitude conversion produced a value outside WGS84 bounds.
    #[error("longitude out of range after local conversion: {lon_deg}")]
    LongitudeOutOfRange {
        /// Converted longitude.
        lon_deg: f64,
    },
    /// Altitude cannot fit in a MAVLink f32 altitude field.
    #[error("altitude out of f32 range: {altitude_m}")]
    AltitudeOutOfRange {
        /// Altitude in metres.
        altitude_m: f64,
    },
    /// Scaled latitude/longitude cannot fit in MAVLink int32 fields.
    #[error("{label} out of MAVLink int32 range after scaling: {scaled}")]
    ScaledCoordinateOutOfRange {
        /// Field label.
        label: &'static str,
        /// Scaled value before int32 conversion.
        scaled: f64,
    },
}

/// Convert a local east/north/relative-altitude offset into MAVLink global-int coordinates.
pub fn local_to_mavlink_int(
    east_m: f64,
    north_m: f64,
    z_relative_m: f64,
    origin: MavlinkCoordinateOrigin,
) -> Result<MavlinkIntCoordinate, MavlinkCoordinateError> {
    let lat = local_to_lat_deg(north_m, origin.lat_deg)?;
    let lon = local_to_lon_deg(east_m, origin.lat_deg, origin.lon_deg)?;
    Ok(MavlinkIntCoordinate {
        lat_e7: scaled_coordinate(lat, "latitude")?,
        lon_e7: scaled_coordinate(lon, "longitude")?,
        relative_alt_m: relative_altitude(z_relative_m)?,
    })
}

/// Convert a north offset to WGS84 latitude.
pub fn local_to_lat_deg(north_m: f64, origin_lat_deg: f64) -> Result<f64, MavlinkCoordinateError> {
    ensure_finite("north_m", north_m)?;
    ensure_finite("origin_lat_deg", origin_lat_deg)?;
    let lat_deg = origin_lat_deg + north_m / 111_320.0;
    if (-90.0..=90.0).contains(&lat_deg) {
        Ok(lat_deg)
    } else {
        Err(MavlinkCoordinateError::LatitudeOutOfRange { lat_deg })
    }
}

/// Convert an east offset to WGS84 longitude.
pub fn local_to_lon_deg(
    east_m: f64,
    origin_lat_deg: f64,
    origin_lon_deg: f64,
) -> Result<f64, MavlinkCoordinateError> {
    ensure_finite("east_m", east_m)?;
    ensure_finite("origin_lat_deg", origin_lat_deg)?;
    ensure_finite("origin_lon_deg", origin_lon_deg)?;
    let meters_per_degree = 111_320.0 * origin_lat_deg.to_radians().cos();
    if meters_per_degree.abs() < 1.0 {
        return Err(MavlinkCoordinateError::LongitudeUnstableNearPole);
    }
    let lon_deg = origin_lon_deg + east_m / meters_per_degree;
    if (-180.0..=180.0).contains(&lon_deg) {
        Ok(lon_deg)
    } else {
        Err(MavlinkCoordinateError::LongitudeOutOfRange { lon_deg })
    }
}

/// Convert a relative altitude into the f32 field used by MAVLink mission items.
pub fn relative_altitude(z_m: f64) -> Result<f32, MavlinkCoordinateError> {
    ensure_finite("z_m", z_m)?;
    if z_m < f32::MIN as f64 || z_m > f32::MAX as f64 {
        return Err(MavlinkCoordinateError::AltitudeOutOfRange { altitude_m: z_m });
    }
    Ok(z_m as f32)
}

/// Scale a WGS84 coordinate to MAVLink `*_int` representation.
pub fn scaled_coordinate(value: f64, label: &'static str) -> Result<i32, MavlinkCoordinateError> {
    ensure_finite(label, value)?;
    let scaled = (value * 10_000_000.0).round();
    if scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
        return Err(MavlinkCoordinateError::ScaledCoordinateOutOfRange { label, scaled });
    }
    Ok(scaled as i32)
}

fn ensure_finite(label: &'static str, value: f64) -> Result<(), MavlinkCoordinateError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(MavlinkCoordinateError::NonFinite { label })
    }
}
