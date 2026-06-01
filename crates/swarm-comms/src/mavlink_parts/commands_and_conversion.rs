#[cfg(feature = "mavlink-transport")]
pub fn arm_command(target_system: u8, target_component: u8) -> CommonMessage {
    command_long(
        common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
        target_system,
        target_component,
        [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    )
}

#[cfg(feature = "mavlink-transport")]
pub fn disarm_command(target_system: u8, target_component: u8) -> CommonMessage {
    command_long(
        common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
        target_system,
        target_component,
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    )
}

#[cfg(feature = "mavlink-transport")]
pub fn takeoff_command(target_system: u8, target_component: u8, altitude_m: f32) -> CommonMessage {
    command_long(
        common::MavCmd::MAV_CMD_NAV_TAKEOFF,
        target_system,
        target_component,
        [0.0, 0.0, 0.0, f32::NAN, 0.0, 0.0, altitude_m],
    )
}

#[cfg(feature = "mavlink-transport")]
pub fn start_mission_command(target_system: u8, target_component: u8) -> CommonMessage {
    command_long(
        common::MavCmd::MAV_CMD_MISSION_START,
        target_system,
        target_component,
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    )
}

#[cfg(feature = "mavlink-transport")]
pub fn abort_command(target_system: u8, target_component: u8) -> CommonMessage {
    command_long(
        common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
        target_system,
        target_component,
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    )
}

#[cfg(feature = "mavlink-transport")]
fn command_long(
    command: common::MavCmd,
    target_system: u8,
    target_component: u8,
    params: [f32; 7],
) -> CommonMessage {
    CommonMessage::COMMAND_LONG(common::COMMAND_LONG_DATA {
        param1: params[0],
        param2: params[1],
        param3: params[2],
        param4: params[3],
        param5: params[4],
        param6: params[5],
        param7: params[6],
        command,
        target_system,
        target_component,
        confirmation: 0,
    })
}

#[cfg(feature = "mavlink-transport")]
fn send_command_and_wait_observed<C, O>(
    conn: &mut C,
    msg: CommonMessage,
    timeout: Duration,
    observer: &mut O,
) -> Result<(), MavlinkLifecycleError>
where
    C: MavlinkVehicleConnection,
    O: MavlinkMissionObserver,
{
    let command = command_id(&msg).expect("command helper must build COMMAND_LONG");
    observer.on_event(MavlinkMissionEvent::CommandSent {
        command: format!("{command:?}"),
    });
    conn.send_message(msg)
        .map_err(mission_error_to_lifecycle_error)?;
    match wait_command_ack(conn, command, timeout) {
        Ok(()) => {
            observer.on_event(MavlinkMissionEvent::CommandAckReceived {
                command: format!("{command:?}"),
                result: "MAV_RESULT_ACCEPTED".to_owned(),
                accepted: true,
            });
            Ok(())
        }
        Err(MavlinkLifecycleError::CommandRejected {
            command,
            result,
            abort_result,
        }) => {
            observer.on_event(MavlinkMissionEvent::CommandAckReceived {
                command: format!("{command:?}"),
                result: format!("{result:?}"),
                accepted: false,
            });
            Err(MavlinkLifecycleError::CommandRejected {
                command,
                result,
                abort_result,
            })
        }
        Err(MavlinkLifecycleError::CommandAckTimeout {
            command,
            abort_result,
        }) => {
            observer.on_event(MavlinkMissionEvent::CommandAckReceived {
                command: format!("{command:?}"),
                result: "timeout".to_owned(),
                accepted: false,
            });
            Err(MavlinkLifecycleError::CommandAckTimeout {
                command,
                abort_result,
            })
        }
        Err(error) => Err(error),
    }
}

#[cfg(feature = "mavlink-transport")]
fn command_id(msg: &CommonMessage) -> Option<common::MavCmd> {
    match msg {
        CommonMessage::COMMAND_LONG(command) => Some(command.command),
        CommonMessage::COMMAND_INT(command) => Some(command.command),
        _ => None,
    }
}

#[cfg(feature = "mavlink-transport")]
fn wait_command_ack<C: MavlinkVehicleConnection>(
    conn: &mut C,
    command: common::MavCmd,
    timeout: Duration,
) -> Result<(), MavlinkLifecycleError> {
    recv_matching_lifecycle(
        conn,
        timeout,
        |_header, msg| match msg {
            CommonMessage::COMMAND_ACK(ack) if ack.command == command => {
                if ack.result == common::MavResult::MAV_RESULT_ACCEPTED {
                    Some(Ok(()))
                } else {
                    Some(Err(MavlinkLifecycleError::CommandRejected {
                        command,
                        result: ack.result,
                        abort_result: None,
                    }))
                }
            }
            _ => None,
        },
        || MavlinkLifecycleError::CommandAckTimeout {
            command,
            abort_result: None,
        },
    )?
}

#[cfg(feature = "mavlink-transport")]
fn wait_for_post_start_heartbeat<C: MavlinkVehicleConnection>(
    conn: &mut C,
    timeout: Duration,
) -> Result<(), MavlinkLifecycleError> {
    recv_matching_lifecycle(
        conn,
        timeout,
        |_header, msg| matches!(msg, CommonMessage::HEARTBEAT(_)).then_some(Ok(())),
        || MavlinkLifecycleError::PostStartHeartbeatTimeout {
            abort_result: AbortCommandResult::NotAttempted,
        },
    )?
}

#[cfg(feature = "mavlink-transport")]
fn recv_matching_lifecycle<T, C, F, E>(
    conn: &mut C,
    timeout: Duration,
    mut predicate: F,
    on_timeout: E,
) -> Result<T, MavlinkLifecycleError>
where
    C: MavlinkVehicleConnection,
    F: FnMut(CommonHeader, CommonMessage) -> Option<T>,
    E: Fn() -> MavlinkLifecycleError,
{
    let deadline = Instant::now() + timeout;
    loop {
        if let Some((header, msg)) = conn
            .try_recv_message()
            .map_err(mission_error_to_lifecycle_error)?
        {
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

#[cfg(feature = "mavlink-transport")]
fn send_abort_command<C: MavlinkVehicleConnection>(
    conn: &mut C,
    options: &MissionLifecycleOptions,
) -> AbortCommandResult {
    let mut observer = NoopMavlinkMissionObserver;
    send_abort_command_observed(conn, options, &mut observer)
}

#[cfg(feature = "mavlink-transport")]
fn send_abort_command_observed<C, O>(
    conn: &mut C,
    options: &MissionLifecycleOptions,
    observer: &mut O,
) -> AbortCommandResult
where
    C: MavlinkVehicleConnection,
    O: MavlinkMissionObserver,
{
    let command = common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH;
    observer.on_event(MavlinkMissionEvent::CommandSent {
        command: format!("{command:?}"),
    });
    if let Err(error) = conn.send_message(abort_command(
        options.target_system,
        options.target_component,
    )) {
        let result = AbortCommandResult::Failed(error.to_string());
        observer.on_event(MavlinkMissionEvent::AbortRequested {
            result: format!("{result:?}"),
        });
        return result;
    }

    let result = match wait_command_ack(conn, command, options.timeout) {
        Ok(()) => AbortCommandResult::Accepted,
        Err(MavlinkLifecycleError::CommandRejected { result, .. }) => {
            AbortCommandResult::Rejected(result)
        }
        Err(MavlinkLifecycleError::CommandAckTimeout { .. }) => AbortCommandResult::AckTimeout,
        Err(error) => AbortCommandResult::Failed(error.to_string()),
    };
    observer.on_event(MavlinkMissionEvent::AbortRequested {
        result: format!("{result:?}"),
    });
    result
}

#[cfg(feature = "mavlink-transport")]
fn attach_abort_result(
    error: MavlinkLifecycleError,
    abort_result: AbortCommandResult,
) -> MavlinkLifecycleError {
    match error {
        MavlinkLifecycleError::CommandAckTimeout { command, .. } => {
            MavlinkLifecycleError::CommandAckTimeout {
                command,
                abort_result: Some(abort_result),
            }
        }
        MavlinkLifecycleError::CommandRejected {
            command, result, ..
        } => MavlinkLifecycleError::CommandRejected {
            command,
            result,
            abort_result: Some(abort_result),
        },
        other => other,
    }
}

#[cfg(feature = "mavlink-transport")]
fn mission_error_to_lifecycle_error(error: MavlinkMissionError) -> MavlinkLifecycleError {
    match error {
        MavlinkMissionError::WriteFailed(message) => MavlinkLifecycleError::WriteFailed(message),
        MavlinkMissionError::ReadFailed(message) => MavlinkLifecycleError::ReadFailed(message),
        other => MavlinkLifecycleError::ReadFailed(other.to_string()),
    }
}

#[cfg(feature = "mavlink-transport")]
fn mission_error_to_telemetry_error(error: MavlinkMissionError) -> MavlinkTelemetryError {
    match error {
        MavlinkMissionError::ReadFailed(message) => MavlinkTelemetryError::ReadFailed(message),
        other => MavlinkTelemetryError::ReadFailed(other.to_string()),
    }
}

#[cfg(feature = "mavlink-transport")]
impl Transport for MavlinkTransport {
    type Error = MavlinkError;

    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        reject_raw_transport_send(msg)
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
        use mavlink::MavConnection;
        if let Some(msg) = self.recv_buf.pop_front() {
            return Ok(Some(msg));
        }
        match self.conn.try_recv() {
            Ok((_header, mav_msg)) => {
                let result = RawMessage {
                    from: self.agent_id.clone(),
                    to: self.agent_id.clone(),
                    payload: serde_json::to_vec(&format!("{mav_msg:?}"))?,
                };
                self.recv_buf.push_back(result);
                Ok(self.recv_buf.pop_front())
            }
            Err(e) => Err(MavlinkError::Connection(e.to_string())),
        }
    }
}

#[cfg(feature = "mavlink-transport")]
fn normalize_mavlink_connection_string(connection_string: &str) -> Cow<'_, str> {
    let connection_string = connection_string.trim();
    if let Some(rest) = connection_string.strip_prefix("udp:") {
        return Cow::Owned(format!("udpin:{rest}"));
    }
    if let Some(rest) = connection_string.strip_prefix("tcp:") {
        return Cow::Owned(format!("tcpout:{rest}"));
    }
    Cow::Borrowed(connection_string)
}

#[cfg(feature = "mavlink-transport")]
fn reject_raw_transport_send(_msg: RawMessage) -> Result<(), MavlinkError> {
    Err(MavlinkError::UnsupportedRawTransportSend)
}

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
