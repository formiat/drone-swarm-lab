#[cfg(feature = "mavlink-transport")]
use std::time::Duration;

#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;
use swarm_types::TaskStatus;

#[cfg(feature = "mavlink-transport")]
use super::MavlinkMissionError;

/// A waypoint in local coordinate space (no MAVLink dependency).
#[derive(Debug, Clone, PartialEq)]
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

/// A typed MAVLink mission item that maps to a concrete `MAV_CMD_NAV_*`.
///
/// Used by `upload_mission_items` to upload primitive missions without the
/// allocation or simulation layers.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq)]
pub enum MissionItem {
    /// Fly to position (`MAV_CMD_NAV_WAYPOINT`).
    Goto { position: Waypoint },
    /// Loiter at position for `hold_seconds` (`MAV_CMD_NAV_LOITER_TIME`).
    ///
    /// `radius_m` sets the loiter radius in metres; 0.0 uses the autopilot
    /// default. Positive = counter-clockwise, negative = clockwise.
    LoiterTime {
        position: Waypoint,
        hold_seconds: f32,
        radius_m: f32,
    },
    /// Complete `turns` full circles of `radius_m` (`MAV_CMD_NAV_LOITER_TURNS`).
    ///
    /// Positive turns = counter-clockwise, negative = clockwise.
    LoiterTurns {
        position: Waypoint,
        turns: f32,
        radius_m: f32,
    },
    /// Land at position (`MAV_CMD_NAV_LAND`).
    Land { position: Waypoint },
}

#[cfg(feature = "mavlink-transport")]
impl MissionItem {
    /// The x/y/z position of this item.
    pub fn position(&self) -> &Waypoint {
        match self {
            Self::Goto { position }
            | Self::LoiterTime { position, .. }
            | Self::LoiterTurns { position, .. }
            | Self::Land { position } => position,
        }
    }

    /// Short label for dry-run display.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Goto { .. } => "waypoint",
            Self::LoiterTime { .. } => "loiter_time",
            Self::LoiterTurns { .. } => "loiter_turns",
            Self::Land { .. } => "land",
        }
    }
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
