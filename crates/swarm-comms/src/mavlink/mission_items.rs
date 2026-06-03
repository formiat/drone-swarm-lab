#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;
#[cfg(feature = "mavlink-transport")]
use swarm_types::TaskStatus;

#[cfg(feature = "mavlink-transport")]
use super::{CommonMessage, MavlinkMissionError, MissionItem, MissionUploadOptions, Waypoint};
/// Convert a local waypoint to a MAVLink global mission item.
#[cfg(feature = "mavlink-transport")]
pub fn waypoint_to_mission_item_int(
    waypoint: &Waypoint,
    options: &MissionUploadOptions,
) -> Result<CommonMessage, MavlinkMissionError> {
    let lat = local_to_lat_deg(waypoint.y, options.home_origin.lat_deg)?;
    let lon = local_to_lon_deg(
        waypoint.x,
        options.home_origin.lat_deg,
        options.home_origin.lon_deg,
    )?;
    let lat = scaled_coordinate(lat, "latitude")?;
    let lon = scaled_coordinate(lon, "longitude")?;
    let z = relative_altitude(waypoint.z)?;

    Ok(CommonMessage::MISSION_ITEM_INT(
        common::MISSION_ITEM_INT_DATA {
            param1: 0.0,
            param2: 0.0,
            param3: 0.0,
            param4: f32::NAN,
            x: lat,
            y: lon,
            z,
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
fn local_to_lat_deg(north_m: f64, origin_lat_deg: f64) -> Result<f64, MavlinkMissionError> {
    ensure_finite("north_m", north_m)?;
    ensure_finite("origin_lat_deg", origin_lat_deg)?;
    let lat = origin_lat_deg + north_m / 111_320.0;
    if (-90.0..=90.0).contains(&lat) {
        Ok(lat)
    } else {
        Err(MavlinkMissionError::Conversion(format!(
            "latitude out of range after local conversion: {lat}"
        )))
    }
}

#[cfg(feature = "mavlink-transport")]
fn local_to_lon_deg(
    east_m: f64,
    origin_lat_deg: f64,
    origin_lon_deg: f64,
) -> Result<f64, MavlinkMissionError> {
    ensure_finite("east_m", east_m)?;
    ensure_finite("origin_lat_deg", origin_lat_deg)?;
    ensure_finite("origin_lon_deg", origin_lon_deg)?;
    let meters_per_degree = 111_320.0 * origin_lat_deg.to_radians().cos();
    if meters_per_degree.abs() < 1.0 {
        return Err(MavlinkMissionError::Conversion(
            "longitude conversion is unstable near the poles".to_owned(),
        ));
    }
    let lon = origin_lon_deg + east_m / meters_per_degree;
    if (-180.0..=180.0).contains(&lon) {
        Ok(lon)
    } else {
        Err(MavlinkMissionError::Conversion(format!(
            "longitude out of range after local conversion: {lon}"
        )))
    }
}

#[cfg(feature = "mavlink-transport")]
fn relative_altitude(z_m: f64) -> Result<f32, MavlinkMissionError> {
    ensure_finite("z_m", z_m)?;
    let altitude = z_m;
    if altitude < f32::MIN as f64 || altitude > f32::MAX as f64 {
        return Err(MavlinkMissionError::Conversion(format!(
            "altitude out of f32 range: {altitude}"
        )));
    }
    Ok(altitude as f32)
}

#[cfg(feature = "mavlink-transport")]
fn scaled_coordinate(value: f64, label: &str) -> Result<i32, MavlinkMissionError> {
    ensure_finite(label, value)?;
    let scaled = (value * 10_000_000.0).round();
    if scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
        return Err(MavlinkMissionError::Conversion(format!(
            "{label} out of MAVLink int32 range after scaling: {scaled}"
        )));
    }
    Ok(scaled as i32)
}

#[cfg(feature = "mavlink-transport")]
fn ensure_finite(label: &str, value: f64) -> Result<(), MavlinkMissionError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(MavlinkMissionError::Conversion(format!(
            "{label} must be finite"
        )))
    }
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
    let lat = local_to_lat_deg(pos.y, options.home_origin.lat_deg)?;
    let lon = local_to_lon_deg(
        pos.x,
        options.home_origin.lat_deg,
        options.home_origin.lon_deg,
    )?;
    let lat = scaled_coordinate(lat, "latitude")?;
    let lon = scaled_coordinate(lon, "longitude")?;
    let z = relative_altitude(pos.z)?;
    let seq = pos.seq;
    let is_first = seq == 0;

    let data = match item {
        MissionItem::Goto { .. } => common::MISSION_ITEM_INT_DATA {
            param1: 0.0,
            param2: 0.0,
            param3: 0.0,
            param4: f32::NAN,
            x: lat,
            y: lon,
            z,
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
            x: lat,
            y: lon,
            z,
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
            x: lat,
            y: lon,
            z,
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
