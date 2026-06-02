#[cfg(feature = "mavlink-transport")]
use std::time::Duration;

#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;

#[cfg(feature = "mavlink-transport")]
use super::AbortCommandResult;

#[derive(Debug, thiserror::Error)]
pub enum MavlinkError {
    #[error("mavlink connection error: {0}")]
    Connection(String),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("transport not connected")]
    NotConnected,
    #[error("no pose on task")]
    NoPose,
    #[error(
        "generic MAVLink Transport::send is unsupported; use mission upload/lifecycle APIs for PX4 SITL"
    )]
    UnsupportedRawTransportSend,
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MavlinkTelemetryError {
    #[error("timed out waiting for MAVLink telemetry event after {timeout:?}")]
    Timeout { timeout: Duration },
    #[error("mavlink telemetry read failed: {0}")]
    ReadFailed(String),
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum MavlinkLifecycleError {
    #[error("invalid takeoff altitude: {altitude_m}")]
    InvalidTakeoffAltitude { altitude_m: f32 },
    #[error("timed out waiting for command ack: {command:?}; abort_result={abort_result:?}")]
    CommandAckTimeout {
        command: common::MavCmd,
        abort_result: Option<AbortCommandResult>,
    },
    #[error(
        "command rejected by vehicle: {command:?} result={result:?}; abort_result={abort_result:?}"
    )]
    CommandRejected {
        command: common::MavCmd,
        result: common::MavResult,
        abort_result: Option<AbortCommandResult>,
    },
    #[error("post-start heartbeat timeout; abort_result={abort_result:?}")]
    PostStartHeartbeatTimeout { abort_result: AbortCommandResult },
    #[error("abort command failed: {abort_result:?}")]
    AbortFailed { abort_result: AbortCommandResult },
    #[error("mavlink write failed: {0}")]
    WriteFailed(String),
    #[error("mavlink read failed: {0}")]
    ReadFailed(String),
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum MavlinkFlightError {
    #[error("mission upload failed: {0}")]
    MissionUpload(#[from] MavlinkMissionError),
    #[error("mission lifecycle failed: {0}")]
    Lifecycle(#[from] MavlinkLifecycleError),
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum MavlinkMissionError {
    #[error("mission upload requires at least one waypoint")]
    EmptyMission,
    #[error("mission contains too many waypoints: {count}")]
    TooManyWaypoints { count: usize },
    #[error("timed out waiting for MAVLink HEARTBEAT")]
    HeartbeatTimeout,
    #[error("timed out waiting for mission request seq={expected_seq}")]
    MissionRequestTimeout { expected_seq: u16 },
    #[error("timed out waiting for mission ack")]
    MissionAckTimeout,
    #[error("unexpected mission request sequence: expected {expected}, got {actual}")]
    UnexpectedRequestSeq { expected: u16, actual: u16 },
    #[error("mission rejected by vehicle: {0:?}")]
    MissionRejected(common::MavMissionResult),
    #[error("mission frame is not supported by this uploader")]
    UnsupportedFrame,
    #[error("coordinate conversion failed: {0}")]
    Conversion(String),
    #[error("mavlink write failed: {0}")]
    WriteFailed(String),
    #[error("mavlink read failed: {0}")]
    ReadFailed(String),
}
