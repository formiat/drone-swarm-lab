#[cfg(feature = "mavlink-transport")]
use std::time::Duration;

#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;
use swarm_types::TaskStatus;

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

/// A waypoint in local coordinate space (no MAVLink dependency).
#[derive(Debug, Clone)]
pub struct Waypoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub seq: u16,
}

/// Origin used to convert local simulation coordinates into WGS84 mission items.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissionHomeOrigin {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub alt_m: f64,
}

#[cfg(feature = "mavlink-transport")]
impl Default for MissionHomeOrigin {
    fn default() -> Self {
        Self {
            lat_deg: 47.397_742,
            lon_deg: 8.545_594,
            alt_m: 0.0,
        }
    }
}

/// MAVLink frame used for uploaded waypoint missions.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionFrame {
    GlobalRelativeAlt,
    LocalNed,
}

#[cfg(feature = "mavlink-transport")]
impl MissionFrame {
    #[allow(deprecated)]
    pub(super) fn to_mav_frame(self) -> Result<common::MavFrame, MavlinkMissionError> {
        match self {
            Self::GlobalRelativeAlt => Ok(common::MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT),
            Self::LocalNed => Err(MavlinkMissionError::UnsupportedFrame),
        }
    }
}

/// Options for the minimal MAVLink mission upload transaction.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone)]
pub struct MissionUploadOptions {
    pub target_system: u8,
    pub target_component: u8,
    pub timeout: Duration,
    pub retry_count: u8,
    pub clear_existing: bool,
    pub home_origin: MissionHomeOrigin,
    pub frame: MissionFrame,
}

#[cfg(feature = "mavlink-transport")]
impl Default for MissionUploadOptions {
    fn default() -> Self {
        Self {
            target_system: 1,
            target_component: 1,
            timeout: Duration::from_secs(2),
            retry_count: 0,
            clear_existing: true,
            home_origin: MissionHomeOrigin::default(),
            frame: MissionFrame::GlobalRelativeAlt,
        }
    }
}

/// Summary returned after PX4 accepts a waypoint mission upload.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq)]
pub struct MissionUploadReport {
    pub uploaded_count: usize,
    pub target_system: u8,
    pub target_component: u8,
    pub ack: common::MavMissionResult,
    pub cleared_existing: bool,
}

/// Options for the controlled post-upload flight lifecycle.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone)]
pub struct MissionLifecycleOptions {
    pub target_system: u8,
    pub target_component: u8,
    pub timeout: Duration,
    pub no_arm: bool,
    pub abort_after: Option<Duration>,
    pub takeoff_altitude_m: f32,
}

#[cfg(feature = "mavlink-transport")]
impl Default for MissionLifecycleOptions {
    fn default() -> Self {
        Self {
            target_system: 1,
            target_component: 1,
            timeout: Duration::from_secs(2),
            no_arm: false,
            abort_after: None,
            takeoff_altitude_m: 2.5,
        }
    }
}

/// Result of an attempted abort command during lifecycle execution.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq)]
pub enum AbortCommandResult {
    NotAttempted,
    Accepted,
    Rejected(common::MavResult),
    AckTimeout,
    Failed(String),
}

/// Summary returned after PX4 accepts lifecycle commands.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MissionLifecycleReport {
    pub armed: bool,
    pub took_off: bool,
    pub started: bool,
    pub post_start_heartbeat: bool,
    pub abort_result: Option<AbortCommandResult>,
}

/// Summary returned by the combined upload + execute workflow.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq)]
pub struct MavlinkFlightReport {
    pub upload: MissionUploadReport,
    pub lifecycle: MissionLifecycleReport,
}

/// Progress-oriented MAVLink telemetry event consumed by SITL workflows.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MavlinkTelemetryEvent {
    Heartbeat,
    MissionCurrent { seq: u16 },
    WaypointReached { seq: u16 },
    MissionComplete,
    MissionRejected { reason: String },
    Disconnected,
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MavlinkMissionEvent {
    HeartbeatSeen,
    MissionClearSent,
    MissionCountSent {
        count: usize,
    },
    MissionItemRequested {
        seq: u16,
    },
    MissionItemSent {
        seq: u16,
    },
    MissionAckReceived {
        result: String,
        accepted: bool,
    },
    CommandSent {
        command: String,
    },
    CommandAckReceived {
        command: String,
        result: String,
        accepted: bool,
    },
    AbortRequested {
        result: String,
    },
}

#[cfg(feature = "mavlink-transport")]
pub trait MavlinkMissionObserver {
    fn on_event(&mut self, event: MavlinkMissionEvent);
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Default)]
pub struct NoopMavlinkMissionObserver;

#[cfg(feature = "mavlink-transport")]
impl MavlinkMissionObserver for NoopMavlinkMissionObserver {
    fn on_event(&mut self, _event: MavlinkMissionEvent) {}
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

/// Convert a Task to a Waypoint.
pub fn task_to_waypoint(task: &swarm_types::Task) -> Option<Waypoint> {
    task.pose.map(|pose| Waypoint {
        x: pose.x,
        y: pose.y,
        z: pose.z,
        seq: 0,
    })
}

/// Derive TaskStatus from a boolean acknowledgement flag (mock path).
pub fn waypoint_status_to_task_status(ack: bool) -> TaskStatus {
    if ack {
        TaskStatus::Completed
    } else {
        TaskStatus::InProgress
    }
}
