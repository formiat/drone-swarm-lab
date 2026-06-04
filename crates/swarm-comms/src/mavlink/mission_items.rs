#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;
#[cfg(feature = "mavlink-transport")]
use swarm_types::TaskStatus;

#[cfg(feature = "mavlink-transport")]
use crate::mavlink_coords::{local_to_mavlink_int, MavlinkCoordinateOrigin, MavlinkIntCoordinate};

#[cfg(feature = "mavlink-transport")]
use super::{CommonMessage, MavlinkMissionError, MissionItem, MissionUploadOptions, Waypoint};
/// Convert a local waypoint to a MAVLink global mission item.
#[cfg(feature = "mavlink-transport")]
pub fn waypoint_to_mission_item_int(
    waypoint: &Waypoint,
    options: &MissionUploadOptions,
) -> Result<CommonMessage, MavlinkMissionError> {
    let coord = waypoint_to_mavlink_coordinate(waypoint, options)?;

    Ok(CommonMessage::MISSION_ITEM_INT(
        common::MISSION_ITEM_INT_DATA {
            param1: 0.0,
            param2: 0.0,
            param3: 0.0,
            param4: f32::NAN,
            x: coord.lat_e7,
            y: coord.lon_e7,
            z: coord.relative_alt_m,
            seq: waypoint.seq,
            command: common::MavCmd::MAV_CMD_NAV_WAYPOINT,
            target_system: options.target_system,
            target_component: options.target_component,
            frame: options.frame.to_mav_frame()?,
            current: if waypoint.seq == 0 { 1 } else { 0 },
            autocontinue: 1,
        },
    ))
}

#[cfg(feature = "mavlink-transport")]
fn waypoint_to_mavlink_coordinate(
    waypoint: &Waypoint,
    options: &MissionUploadOptions,
) -> Result<MavlinkIntCoordinate, MavlinkMissionError> {
    local_to_mavlink_int(
        waypoint.x,
        waypoint.y,
        waypoint.z,
        MavlinkCoordinateOrigin {
            lat_deg: options.home_origin.lat_deg,
            lon_deg: options.home_origin.lon_deg,
            alt_m: options.home_origin.alt_m,
        },
    )
    .map_err(|error| MavlinkMissionError::Conversion(error.to_string()))
}

/// Convert a typed `MissionItem` to a MAVLink `MISSION_ITEM_INT` message.
///
/// `seq` in `item.position()` is overridden by the caller during upload.
#[cfg(feature = "mavlink-transport")]
pub fn mission_item_to_int(
    item: &MissionItem,
    options: &MissionUploadOptions,
) -> Result<CommonMessage, MavlinkMissionError> {
    let pos = item.position();
    let coord = waypoint_to_mavlink_coordinate(pos, options)?;
    let seq = pos.seq;
    let is_first = seq == 0;

    let data = match item {
        MissionItem::Goto { .. } => common::MISSION_ITEM_INT_DATA {
            param1: 0.0,
            param2: 0.0,
            param3: 0.0,
            param4: f32::NAN,
            x: coord.lat_e7,
            y: coord.lon_e7,
            z: coord.relative_alt_m,
            seq,
            command: common::MavCmd::MAV_CMD_NAV_WAYPOINT,
            target_system: options.target_system,
            target_component: options.target_component,
            frame: options.frame.to_mav_frame()?,
            current: if is_first { 1 } else { 0 },
            autocontinue: 1,
        },
        MissionItem::LoiterTime {
            hold_seconds,
            radius_m,
            ..
        } => common::MISSION_ITEM_INT_DATA {
            // param1: hold time in seconds
            param1: *hold_seconds,
            // param2: radius in metres (0 = autopilot default)
            param2: *radius_m,
            param3: 0.0,
            // param4: xtrack location (0 = center exit)
            param4: 0.0,
            x: coord.lat_e7,
            y: coord.lon_e7,
            z: coord.relative_alt_m,
            seq,
            command: common::MavCmd::MAV_CMD_NAV_LOITER_TIME,
            target_system: options.target_system,
            target_component: options.target_component,
            frame: options.frame.to_mav_frame()?,
            current: if is_first { 1 } else { 0 },
            autocontinue: 1,
        },
        MissionItem::LoiterTurns {
            turns, radius_m, ..
        } => common::MISSION_ITEM_INT_DATA {
            // param1: number of turns (positive = CCW)
            param1: *turns,
            // param2: heading required on exit (0 = no heading required)
            param2: 0.0,
            // param3: radius in metres (positive = CCW)
            param3: *radius_m,
            // param4: exit xtrack location
            param4: 0.0,
            x: coord.lat_e7,
            y: coord.lon_e7,
            z: coord.relative_alt_m,
            seq,
            command: common::MavCmd::MAV_CMD_NAV_LOITER_TURNS,
            target_system: options.target_system,
            target_component: options.target_component,
            frame: options.frame.to_mav_frame()?,
            current: if is_first { 1 } else { 0 },
            autocontinue: 1,
        },
        MissionItem::Land { .. } => common::MISSION_ITEM_INT_DATA {
            param1: 0.0,
            param2: 0.0,
            param3: 0.0,
            // param4: desired yaw (NAN = unchanged)
            param4: f32::NAN,
            // x/y = 0 means land at current position
            x: 0,
            y: 0,
            z: 0.0,
            seq,
            command: common::MavCmd::MAV_CMD_NAV_LAND,
            target_system: options.target_system,
            target_component: options.target_component,
            frame: options.frame.to_mav_frame()?,
            current: if is_first { 1 } else { 0 },
            autocontinue: 1,
        },
    };

    Ok(CommonMessage::MISSION_ITEM_INT(data))
}

/// Convert a Task to a MAVLink mission item int message (requires mavlink feature).
#[cfg(feature = "mavlink-transport")]
pub fn task_to_mavlink_waypoint(
    task: &swarm_types::Task,
    seq: u16,
    target_system: u8,
    target_component: u8,
) -> Option<CommonMessage> {
    let pose = task.pose?;
    let waypoint = Waypoint {
        x: pose.x,
        y: pose.y,
        z: pose.z,
        seq,
    };
    let options = MissionUploadOptions {
        target_system,
        target_component,
        ..MissionUploadOptions::default()
    };
    waypoint_to_mission_item_int(&waypoint, &options).ok()
}

/// Convert a MAVLink message to a TaskStatus (requires mavlink feature).
#[cfg(feature = "mavlink-transport")]
pub fn mavlink_status_to_task_status(msg: &CommonMessage) -> Option<TaskStatus> {
    match msg {
        CommonMessage::MISSION_ACK(ack) => {
            if ack.mavtype == common::MavMissionResult::MAV_MISSION_ACCEPTED {
                Some(TaskStatus::Completed)
            } else {
                Some(TaskStatus::Failed)
            }
        }
        CommonMessage::HEARTBEAT(_) => Some(TaskStatus::InProgress),
        _ => None,
    }
}
