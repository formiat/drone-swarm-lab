/// Typed errors produced by mission command IR validation.
#[derive(Debug, thiserror::Error)]
pub enum MissionIrError {
    #[error("duplicate command id '{0}'")]
    DuplicateCommandId(String),

    #[error("takeoff altitude must be positive, got {altitude_m}")]
    InvalidTakeoffAltitude { altitude_m: f64 },

    #[error("hold/loiter duration must be positive, got {duration_secs}s")]
    InvalidDuration { duration_secs: f64 },

    #[error("orbit radius must be positive, got {radius_m}m")]
    InvalidOrbitRadius { radius_m: f64 },

    #[error("orbit turns must be positive, got {turns}")]
    InvalidOrbitTurns { turns: f64 },

    #[error("follow_route command has no waypoints (route_id = '{route_id}')")]
    EmptyRoute { route_id: String },

    #[error("non-finite coordinate in {context}: ({x}, {y}, {z})")]
    NonFiniteCoordinate {
        context: &'static str,
        x: f64,
        y: f64,
        z: f64,
    },

    #[error("position kind '{kind}' is ambiguous for coordinate frame '{frame}'")]
    AmbiguousCoordinateFrame { kind: String, frame: String },
}
