#[cfg(feature = "mavlink-transport")]
use std::io::ErrorKind;
#[cfg(feature = "mavlink-transport")]
use std::time::{Duration, Instant};

#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;

#[cfg(test)]
use super::NoopMavlinkMissionObserver;
#[cfg(feature = "mavlink-transport")]
use super::{
    lifecycle::execute_uploaded_mission_with_connection_observed,
    mission_items::{mission_item_to_int, waypoint_to_mission_item_int},
    CommonHeader, CommonMessage, MavlinkFlightError, MavlinkFlightReport, MavlinkMissionError,
    MavlinkMissionEvent, MavlinkMissionObserver, MissionItem, MissionLifecycleOptions,
    MissionUploadOptions, MissionUploadReport, Waypoint,
};
#[cfg(feature = "mavlink-transport")]
use crate::mavlink_common_plan::{MavlinkCommonCommandName, MavlinkCommonMissionItem};
#[cfg(feature = "mavlink-transport")]
pub(super) trait MavlinkVehicleConnection {
    fn send_message(&mut self, msg: CommonMessage) -> Result<(), MavlinkMissionError>;
    fn try_recv_message(
        &mut self,
    ) -> Result<Option<(CommonHeader, CommonMessage)>, MavlinkMissionError>;
}
impl MavlinkVehicleConnection for mavlink::Connection<CommonMessage> {
    fn send_message(&mut self, msg: CommonMessage) -> Result<(), MavlinkMissionError> {
        use mavlink::MavConnection;

        self.send_default(&msg)
            .map(|_bytes| ())
            .map_err(|error| MavlinkMissionError::WriteFailed(error.to_string()))
    }

    fn try_recv_message(
        &mut self,
    ) -> Result<Option<(CommonHeader, CommonMessage)>, MavlinkMissionError> {
        use mavlink::MavConnection;

        match self.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(mavlink::error::MessageReadError::Io(error))
                if error.kind() == ErrorKind::WouldBlock =>
            {
                Ok(None)
            }
            Err(error) => Err(MavlinkMissionError::ReadFailed(error.to_string())),
        }
    }
}

#[cfg(all(feature = "mavlink-transport", test))]
pub(super) fn upload_mission_with_connection<C: MavlinkVehicleConnection>(
    conn: &mut C,
    waypoints: &[Waypoint],
    options: &MissionUploadOptions,
) -> Result<MissionUploadReport, MavlinkMissionError> {
    let mut observer = NoopMavlinkMissionObserver;
    upload_mission_with_connection_observed(conn, waypoints, options, &mut observer)
}
pub(super) fn upload_mission_with_connection_observed<C, O>(
    conn: &mut C,
    waypoints: &[Waypoint],
    options: &MissionUploadOptions,
    observer: &mut O,
) -> Result<MissionUploadReport, MavlinkMissionError>
where
    C: MavlinkVehicleConnection,
    O: MavlinkMissionObserver,
{
    if waypoints.is_empty() {
        return Err(MavlinkMissionError::EmptyMission);
    }
    if waypoints.len() > u16::MAX as usize {
        return Err(MavlinkMissionError::TooManyWaypoints {
            count: waypoints.len(),
        });
    }

    let mut last_error = None;
    for _attempt in 0..=options.retry_count {
        match upload_mission_attempt(conn, waypoints, options, observer) {
            Ok(report) => return Ok(report),
            Err(error) if error.is_retryable() => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or(MavlinkMissionError::MissionAckTimeout))
}

/// Upload a typed `MissionItem` list to the vehicle.
///
/// Unlike `upload_mission_with_connection_observed`, each item may carry a
/// different `MAV_CMD_NAV_*` command (loiter, turns, land, …).
#[cfg(all(feature = "mavlink-transport", test))]
#[expect(
    dead_code,
    reason = "test helper kept for future typed-item upload tests"
)]
pub(super) fn upload_mission_items_with_connection<C: MavlinkVehicleConnection>(
    conn: &mut C,
    items: &[MissionItem],
    options: &MissionUploadOptions,
) -> Result<MissionUploadReport, MavlinkMissionError> {
    let mut observer = NoopMavlinkMissionObserver;
    upload_mission_items_with_connection_observed(conn, items, options, &mut observer)
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn upload_mission_items_with_connection_observed<C, O>(
    conn: &mut C,
    items: &[MissionItem],
    options: &MissionUploadOptions,
    observer: &mut O,
) -> Result<MissionUploadReport, MavlinkMissionError>
where
    C: MavlinkVehicleConnection,
    O: MavlinkMissionObserver,
{
    if items.is_empty() {
        return Err(MavlinkMissionError::EmptyMission);
    }
    if items.len() > u16::MAX as usize {
        return Err(MavlinkMissionError::TooManyWaypoints { count: items.len() });
    }

    let mut last_error = None;
    for _attempt in 0..=options.retry_count {
        match upload_mission_items_attempt(conn, items, options, observer) {
            Ok(report) => return Ok(report),
            Err(error) if error.is_retryable() => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or(MavlinkMissionError::MissionAckTimeout))
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn upload_precompiled_mission_items_with_connection_observed<C, O>(
    conn: &mut C,
    items: &[MavlinkCommonMissionItem],
    options: &MissionUploadOptions,
    observer: &mut O,
) -> Result<MissionUploadReport, MavlinkMissionError>
where
    C: MavlinkVehicleConnection,
    O: MavlinkMissionObserver,
{
    if items.is_empty() {
        return Err(MavlinkMissionError::EmptyMission);
    }
    if items.len() > u16::MAX as usize {
        return Err(MavlinkMissionError::TooManyWaypoints { count: items.len() });
    }

    let mut last_error = None;
    for _attempt in 0..=options.retry_count {
        match upload_precompiled_mission_items_attempt(conn, items, options, observer) {
            Ok(report) => return Ok(report),
            Err(error) if error.is_retryable() => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or(MavlinkMissionError::MissionAckTimeout))
}

#[cfg(feature = "mavlink-transport")]
fn upload_precompiled_mission_items_attempt<C: MavlinkVehicleConnection>(
    conn: &mut C,
    items: &[MavlinkCommonMissionItem],
    options: &MissionUploadOptions,
    observer: &mut impl MavlinkMissionObserver,
) -> Result<MissionUploadReport, MavlinkMissionError> {
    wait_for_heartbeat(conn, options.timeout)?;
    observer.on_event(MavlinkMissionEvent::HeartbeatSeen);

    if options.clear_existing {
        conn.send_message(CommonMessage::MISSION_CLEAR_ALL(
            common::MISSION_CLEAR_ALL_DATA {
                target_system: options.target_system,
                target_component: options.target_component,
            },
        ))?;
        observer.on_event(MavlinkMissionEvent::MissionClearSent);
    }

    conn.send_message(CommonMessage::MISSION_COUNT(common::MISSION_COUNT_DATA {
        count: items.len() as u16,
        target_system: options.target_system,
        target_component: options.target_component,
    }))?;
    observer.on_event(MavlinkMissionEvent::MissionCountSent { count: items.len() });

    for (expected_seq, item) in items.iter().enumerate() {
        let expected_seq = expected_seq as u16;
        wait_for_mission_request(conn, expected_seq, options.timeout)?;
        observer.on_event(MavlinkMissionEvent::MissionItemRequested { seq: expected_seq });
        conn.send_message(precompiled_mission_item_to_int(
            item,
            expected_seq,
            options,
        )?)?;
        observer.on_event(MavlinkMissionEvent::MissionItemSent { seq: expected_seq });
    }

    let ack = wait_for_mission_ack(conn, options.timeout)?;
    observer.on_event(MavlinkMissionEvent::MissionAckReceived {
        result: format!("{ack:?}"),
        accepted: ack == common::MavMissionResult::MAV_MISSION_ACCEPTED,
    });
    if ack != common::MavMissionResult::MAV_MISSION_ACCEPTED {
        return Err(MavlinkMissionError::MissionRejected(ack));
    }

    Ok(MissionUploadReport {
        uploaded_count: items.len(),
        target_system: options.target_system,
        target_component: options.target_component,
        ack,
        cleared_existing: options.clear_existing,
    })
}

#[cfg(feature = "mavlink-transport")]
#[allow(deprecated)]
fn precompiled_mission_item_to_int(
    item: &MavlinkCommonMissionItem,
    seq: u16,
    options: &MissionUploadOptions,
) -> Result<CommonMessage, MavlinkMissionError> {
    if item.frame != "MAV_FRAME_GLOBAL_RELATIVE_ALT_INT" {
        return Err(MavlinkMissionError::UnsupportedFrame);
    }
    Ok(CommonMessage::MISSION_ITEM_INT(
        common::MISSION_ITEM_INT_DATA {
            param1: item.params[0].unwrap_or(0.0) as f32,
            param2: item.params[1].unwrap_or(0.0) as f32,
            param3: item.params[2].unwrap_or(0.0) as f32,
            param4: item.params[3].unwrap_or(f64::NAN) as f32,
            x: item.lat_e7,
            y: item.lon_e7,
            z: item.relative_alt_m,
            seq,
            command: common_mission_command_to_mavcmd(item.command)?,
            target_system: options.target_system,
            target_component: options.target_component,
            frame: common::MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT,
            current: if seq == 0 { 1 } else { 0 },
            autocontinue: if item.autocontinue { 1 } else { 0 },
        },
    ))
}

#[cfg(feature = "mavlink-transport")]
fn common_mission_command_to_mavcmd(
    command: MavlinkCommonCommandName,
) -> Result<common::MavCmd, MavlinkMissionError> {
    match command {
        MavlinkCommonCommandName::NavWaypoint => Ok(common::MavCmd::MAV_CMD_NAV_WAYPOINT),
        MavlinkCommonCommandName::NavLoiterTime => Ok(common::MavCmd::MAV_CMD_NAV_LOITER_TIME),
        MavlinkCommonCommandName::NavLand => Ok(common::MavCmd::MAV_CMD_NAV_LAND),
        MavlinkCommonCommandName::FenceCircleInclusion => {
            Ok(common::MavCmd::MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION)
        }
        MavlinkCommonCommandName::FenceCircleExclusion => {
            Ok(common::MavCmd::MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION)
        }
        MavlinkCommonCommandName::FencePolygonVertexInclusion => {
            Ok(common::MavCmd::MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION)
        }
        MavlinkCommonCommandName::FencePolygonVertexExclusion => {
            Ok(common::MavCmd::MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION)
        }
        MavlinkCommonCommandName::ComponentArmDisarm
        | MavlinkCommonCommandName::NavTakeoff
        | MavlinkCommonCommandName::NavReturnToLaunch
        | MavlinkCommonCommandName::MissionStart
        | MavlinkCommonCommandName::DoFenceEnable => Err(MavlinkMissionError::UnsupportedFrame),
    }
}

#[cfg(feature = "mavlink-transport")]
fn upload_mission_items_attempt<C: MavlinkVehicleConnection>(
    conn: &mut C,
    items: &[MissionItem],
    options: &MissionUploadOptions,
    observer: &mut impl MavlinkMissionObserver,
) -> Result<MissionUploadReport, MavlinkMissionError> {
    wait_for_heartbeat(conn, options.timeout)?;
    observer.on_event(MavlinkMissionEvent::HeartbeatSeen);

    if options.clear_existing {
        conn.send_message(CommonMessage::MISSION_CLEAR_ALL(
            common::MISSION_CLEAR_ALL_DATA {
                target_system: options.target_system,
                target_component: options.target_component,
            },
        ))?;
        observer.on_event(MavlinkMissionEvent::MissionClearSent);
    }

    conn.send_message(CommonMessage::MISSION_COUNT(common::MISSION_COUNT_DATA {
        count: items.len() as u16,
        target_system: options.target_system,
        target_component: options.target_component,
    }))?;
    observer.on_event(MavlinkMissionEvent::MissionCountSent { count: items.len() });

    for (seq, item) in items.iter().enumerate() {
        let seq = seq as u16;
        wait_for_mission_request(conn, seq, options.timeout)?;
        observer.on_event(MavlinkMissionEvent::MissionItemRequested { seq });

        // Build a copy of the item with the correct sequence number.
        let mut pos = item.position().clone();
        pos.seq = seq;
        let sequenced = match item {
            MissionItem::Goto { .. } => MissionItem::Goto { position: pos },
            MissionItem::LoiterTime {
                hold_seconds,
                radius_m,
                ..
            } => MissionItem::LoiterTime {
                position: pos,
                hold_seconds: *hold_seconds,
                radius_m: *radius_m,
            },
            MissionItem::LoiterTurns {
                turns, radius_m, ..
            } => MissionItem::LoiterTurns {
                position: pos,
                turns: *turns,
                radius_m: *radius_m,
            },
            MissionItem::Land { .. } => MissionItem::Land { position: pos },
        };

        conn.send_message(mission_item_to_int(&sequenced, options)?)?;
        observer.on_event(MavlinkMissionEvent::MissionItemSent { seq });
    }

    let ack = wait_for_mission_ack(conn, options.timeout)?;
    observer.on_event(MavlinkMissionEvent::MissionAckReceived {
        result: format!("{ack:?}"),
        accepted: ack == common::MavMissionResult::MAV_MISSION_ACCEPTED,
    });
    if ack != common::MavMissionResult::MAV_MISSION_ACCEPTED {
        return Err(MavlinkMissionError::MissionRejected(ack));
    }

    Ok(MissionUploadReport {
        uploaded_count: items.len(),
        target_system: options.target_system,
        target_component: options.target_component,
        ack,
        cleared_existing: options.clear_existing,
    })
}

#[cfg(feature = "mavlink-transport")]
fn upload_mission_attempt<C: MavlinkVehicleConnection>(
    conn: &mut C,
    waypoints: &[Waypoint],
    options: &MissionUploadOptions,
    observer: &mut impl MavlinkMissionObserver,
) -> Result<MissionUploadReport, MavlinkMissionError> {
    wait_for_heartbeat(conn, options.timeout)?;
    observer.on_event(MavlinkMissionEvent::HeartbeatSeen);

    if options.clear_existing {
        conn.send_message(CommonMessage::MISSION_CLEAR_ALL(
            common::MISSION_CLEAR_ALL_DATA {
                target_system: options.target_system,
                target_component: options.target_component,
            },
        ))?;
        observer.on_event(MavlinkMissionEvent::MissionClearSent);
    }

    conn.send_message(CommonMessage::MISSION_COUNT(common::MISSION_COUNT_DATA {
        count: waypoints.len() as u16,
        target_system: options.target_system,
        target_component: options.target_component,
    }))?;
    observer.on_event(MavlinkMissionEvent::MissionCountSent {
        count: waypoints.len(),
    });

    for (expected_seq, waypoint) in waypoints.iter().enumerate() {
        let expected_seq = expected_seq as u16;
        wait_for_mission_request(conn, expected_seq, options.timeout)?;
        observer.on_event(MavlinkMissionEvent::MissionItemRequested { seq: expected_seq });
        let waypoint = Waypoint {
            seq: expected_seq,
            ..waypoint.clone()
        };
        conn.send_message(waypoint_to_mission_item_int(&waypoint, options)?)?;
        observer.on_event(MavlinkMissionEvent::MissionItemSent { seq: expected_seq });
    }

    let ack = wait_for_mission_ack(conn, options.timeout)?;
    observer.on_event(MavlinkMissionEvent::MissionAckReceived {
        result: format!("{ack:?}"),
        accepted: ack == common::MavMissionResult::MAV_MISSION_ACCEPTED,
    });
    if ack != common::MavMissionResult::MAV_MISSION_ACCEPTED {
        return Err(MavlinkMissionError::MissionRejected(ack));
    }

    Ok(MissionUploadReport {
        uploaded_count: waypoints.len(),
        target_system: options.target_system,
        target_component: options.target_component,
        ack,
        cleared_existing: options.clear_existing,
    })
}

#[cfg(feature = "mavlink-transport")]
impl MavlinkMissionError {
    fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::HeartbeatTimeout
                | Self::MissionRequestTimeout { .. }
                | Self::MissionAckTimeout
                | Self::WriteFailed(_)
                | Self::ReadFailed(_)
        )
    }
}

#[cfg(feature = "mavlink-transport")]
fn wait_for_heartbeat<C: MavlinkVehicleConnection>(
    conn: &mut C,
    timeout: Duration,
) -> Result<(), MavlinkMissionError> {
    recv_matching(
        conn,
        timeout,
        |_header, msg| matches!(msg, CommonMessage::HEARTBEAT(_)).then_some(()),
        || MavlinkMissionError::HeartbeatTimeout,
    )
}

#[cfg(feature = "mavlink-transport")]
#[allow(deprecated)]
fn wait_for_mission_request<C: MavlinkVehicleConnection>(
    conn: &mut C,
    expected_seq: u16,
    timeout: Duration,
) -> Result<(), MavlinkMissionError> {
    recv_matching(
        conn,
        timeout,
        |_header, msg| match msg {
            CommonMessage::MISSION_REQUEST_INT(request) => {
                validate_requested_seq(expected_seq, request.seq)
            }
            CommonMessage::MISSION_REQUEST(request) => {
                validate_requested_seq(expected_seq, request.seq)
            }
            _ => None,
        },
        || MavlinkMissionError::MissionRequestTimeout { expected_seq },
    )?
}

#[cfg(feature = "mavlink-transport")]
fn validate_requested_seq(expected: u16, actual: u16) -> Option<Result<(), MavlinkMissionError>> {
    if actual == expected {
        Some(Ok(()))
    } else {
        Some(Err(MavlinkMissionError::UnexpectedRequestSeq {
            expected,
            actual,
        }))
    }
}

#[cfg(feature = "mavlink-transport")]
fn wait_for_mission_ack<C: MavlinkVehicleConnection>(
    conn: &mut C,
    timeout: Duration,
) -> Result<common::MavMissionResult, MavlinkMissionError> {
    recv_matching(
        conn,
        timeout,
        |_header, msg| match msg {
            CommonMessage::MISSION_ACK(ack) => Some(ack.mavtype),
            _ => None,
        },
        || MavlinkMissionError::MissionAckTimeout,
    )
}

#[cfg(feature = "mavlink-transport")]
fn recv_matching<T, C, F, E>(
    conn: &mut C,
    timeout: Duration,
    mut predicate: F,
    on_timeout: E,
) -> Result<T, MavlinkMissionError>
where
    C: MavlinkVehicleConnection,
    F: FnMut(CommonHeader, CommonMessage) -> Option<T>,
    E: Fn() -> MavlinkMissionError,
{
    let deadline = Instant::now() + timeout;
    loop {
        if let Some((header, msg)) = conn.try_recv_message()? {
            if let Some(value) = predicate(header, msg) {
                return Ok(value);
            }
            continue;
        }
        if Instant::now() >= deadline {
            return Err(on_timeout());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(all(feature = "mavlink-transport", test))]
pub(super) fn upload_and_execute_mission_with_connection<C: MavlinkVehicleConnection>(
    conn: &mut C,
    waypoints: &[Waypoint],
    upload_options: &MissionUploadOptions,
    lifecycle_options: &MissionLifecycleOptions,
) -> Result<MavlinkFlightReport, MavlinkFlightError> {
    let mut observer = NoopMavlinkMissionObserver;
    upload_and_execute_mission_with_connection_observed(
        conn,
        waypoints,
        upload_options,
        lifecycle_options,
        &mut observer,
    )
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn upload_and_execute_mission_with_connection_observed<C, O>(
    conn: &mut C,
    waypoints: &[Waypoint],
    upload_options: &MissionUploadOptions,
    lifecycle_options: &MissionLifecycleOptions,
    observer: &mut O,
) -> Result<MavlinkFlightReport, MavlinkFlightError>
where
    C: MavlinkVehicleConnection,
    O: MavlinkMissionObserver,
{
    let upload =
        upload_mission_with_connection_observed(conn, waypoints, upload_options, observer)?;
    let lifecycle =
        execute_uploaded_mission_with_connection_observed(conn, lifecycle_options, observer)?;
    Ok(MavlinkFlightReport { upload, lifecycle })
}
