#[cfg(feature = "mavlink-transport")]
use std::io::ErrorKind;
#[cfg(feature = "mavlink-transport")]
use std::time::{Duration, Instant};

#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;

#[cfg(feature = "mavlink-transport")]
use super::{
    commands::{
        arm_command, attach_abort_result, mission_error_to_telemetry_error,
        send_abort_command_observed, send_command_and_wait_observed, start_mission_command,
        takeoff_command, wait_for_post_start_heartbeat,
    },
    mission_items::waypoint_to_mission_item_int,
    AbortCommandResult, CommonHeader, CommonMessage, MavlinkFlightError, MavlinkFlightReport,
    MavlinkLifecycleError, MavlinkMissionError, MavlinkMissionEvent, MavlinkMissionObserver,
    MavlinkTelemetryError, MavlinkTelemetryEvent, MissionLifecycleOptions, MissionLifecycleReport,
    MissionUploadOptions, MissionUploadReport, NoopMavlinkMissionObserver, Waypoint,
};
#[cfg(feature = "mavlink-transport")]
pub(super) trait MavlinkVehicleConnection {
    fn send_message(&mut self, msg: CommonMessage) -> Result<(), MavlinkMissionError>;
    fn try_recv_message(
        &mut self,
    ) -> Result<Option<(CommonHeader, CommonMessage)>, MavlinkMissionError>;
}

#[cfg(feature = "mavlink-transport")]
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

#[cfg(feature = "mavlink-transport")]
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

#[cfg(all(feature = "mavlink-transport", test))]
pub(super) fn execute_uploaded_mission_with_connection<C: MavlinkVehicleConnection>(
    conn: &mut C,
    options: &MissionLifecycleOptions,
) -> Result<MissionLifecycleReport, MavlinkLifecycleError> {
    let mut observer = NoopMavlinkMissionObserver;
    execute_uploaded_mission_with_connection_observed(conn, options, &mut observer)
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn execute_uploaded_mission_with_connection_observed<C, O>(
    conn: &mut C,
    options: &MissionLifecycleOptions,
    observer: &mut O,
) -> Result<MissionLifecycleReport, MavlinkLifecycleError>
where
    C: MavlinkVehicleConnection,
    O: MavlinkMissionObserver,
{
    if !options.takeoff_altitude_m.is_finite() || options.takeoff_altitude_m < 0.0 {
        return Err(MavlinkLifecycleError::InvalidTakeoffAltitude {
            altitude_m: options.takeoff_altitude_m,
        });
    }

    let mut report = MissionLifecycleReport::default();

    if !options.no_arm {
        send_command_and_wait_observed(
            conn,
            arm_command(options.target_system, options.target_component),
            options.timeout,
            observer,
        )?;
        report.armed = true;
    }

    if let Err(error) = send_command_and_wait_observed(
        conn,
        takeoff_command(
            options.target_system,
            options.target_component,
            options.takeoff_altitude_m,
        ),
        options.timeout,
        observer,
    ) {
        let abort_result = send_abort_command_observed(conn, options, observer);
        return Err(attach_abort_result(error, abort_result));
    }
    report.took_off = true;

    if let Err(error) = send_command_and_wait_observed(
        conn,
        start_mission_command(options.target_system, options.target_component),
        options.timeout,
        observer,
    ) {
        let abort_result = send_abort_command_observed(conn, options, observer);
        return Err(attach_abort_result(error, abort_result));
    }
    report.started = true;

    match wait_for_post_start_heartbeat(conn, options.timeout) {
        Ok(()) => {
            report.post_start_heartbeat = true;
            observer.on_event(MavlinkMissionEvent::HeartbeatSeen);
        }
        Err(MavlinkLifecycleError::PostStartHeartbeatTimeout { .. }) => {
            let abort_result = send_abort_command_observed(conn, options, observer);
            return Err(MavlinkLifecycleError::PostStartHeartbeatTimeout { abort_result });
        }
        Err(error) => return Err(error),
    }

    if let Some(abort_after) = options.abort_after {
        std::thread::sleep(abort_after);
        let abort_result = send_abort_command_observed(conn, options, observer);
        if abort_result != AbortCommandResult::Accepted {
            return Err(MavlinkLifecycleError::AbortFailed { abort_result });
        }
        report.abort_result = Some(AbortCommandResult::Accepted);
    }

    Ok(report)
}
