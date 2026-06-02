#[cfg(feature = "mavlink-transport")]
use std::time::{Duration, Instant};

#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;

#[cfg(feature = "mavlink-transport")]
use super::{
    commands::mission_error_to_telemetry_error, mission_upload::MavlinkVehicleConnection,
    CommonMessage, MavlinkTelemetryError,
};

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
pub fn mavlink_message_to_telemetry_event(msg: &CommonMessage) -> Option<MavlinkTelemetryEvent> {
    match msg {
        CommonMessage::HEARTBEAT(_) => Some(MavlinkTelemetryEvent::Heartbeat),
        CommonMessage::MISSION_CURRENT(current) => {
            Some(MavlinkTelemetryEvent::MissionCurrent { seq: current.seq })
        }
        CommonMessage::MISSION_ITEM_REACHED(reached) => {
            Some(MavlinkTelemetryEvent::WaypointReached { seq: reached.seq })
        }
        CommonMessage::MISSION_ACK(ack)
            if ack.mavtype == common::MavMissionResult::MAV_MISSION_ACCEPTED =>
        {
            Some(MavlinkTelemetryEvent::MissionComplete)
        }
        CommonMessage::MISSION_ACK(ack) => Some(MavlinkTelemetryEvent::MissionRejected {
            reason: format!("{:?}", ack.mavtype),
        }),
        _ => None,
    }
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn poll_telemetry_event_with_connection<C: MavlinkVehicleConnection>(
    conn: &mut C,
) -> Result<Option<MavlinkTelemetryEvent>, MavlinkTelemetryError> {
    while let Some((_header, msg)) = conn
        .try_recv_message()
        .map_err(mission_error_to_telemetry_error)?
    {
        if let Some(event) = mavlink_message_to_telemetry_event(&msg) {
            return Ok(Some(event));
        }
    }
    Ok(None)
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn wait_next_telemetry_event_with_connection<C: MavlinkVehicleConnection>(
    conn: &mut C,
    timeout: Duration,
) -> Result<MavlinkTelemetryEvent, MavlinkTelemetryError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(event) = poll_telemetry_event_with_connection(conn)? {
            return Ok(event);
        }
        if Instant::now() >= deadline {
            return Err(MavlinkTelemetryError::Timeout { timeout });
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
