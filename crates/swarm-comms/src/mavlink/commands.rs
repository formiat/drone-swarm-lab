#[cfg(feature = "mavlink-transport")]
use std::time::{Duration, Instant};

#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;

#[cfg(feature = "mavlink-transport")]
use crate::mavlink_common_plan::{MavlinkCommonCommand, MavlinkCommonCommandName};

#[cfg(feature = "mavlink-transport")]
use super::{
    mission_upload::MavlinkVehicleConnection, AbortCommandResult, CommonHeader, CommonMessage,
    MavlinkLifecycleError, MavlinkMissionError, MavlinkMissionEvent, MavlinkMissionObserver,
    MavlinkTelemetryError, MissionLifecycleOptions, NoopMavlinkMissionObserver,
};
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
pub fn common_command_to_message(
    command: &MavlinkCommonCommand,
    target_system: u8,
    target_component: u8,
) -> CommonMessage {
    command_long(
        common_command_name_to_mavcmd(command.command),
        target_system,
        target_component,
        command.params.map(|param| param.unwrap_or(f64::NAN) as f32),
    )
}

#[cfg(feature = "mavlink-transport")]
fn common_command_name_to_mavcmd(command: MavlinkCommonCommandName) -> common::MavCmd {
    match command {
        MavlinkCommonCommandName::ComponentArmDisarm => {
            common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM
        }
        MavlinkCommonCommandName::NavTakeoff => common::MavCmd::MAV_CMD_NAV_TAKEOFF,
        MavlinkCommonCommandName::NavLand => common::MavCmd::MAV_CMD_NAV_LAND,
        MavlinkCommonCommandName::NavReturnToLaunch => common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
        MavlinkCommonCommandName::NavWaypoint => common::MavCmd::MAV_CMD_NAV_WAYPOINT,
        MavlinkCommonCommandName::NavLoiterTime => common::MavCmd::MAV_CMD_NAV_LOITER_TIME,
        MavlinkCommonCommandName::MissionStart => common::MavCmd::MAV_CMD_MISSION_START,
        MavlinkCommonCommandName::FenceCircleInclusion => {
            common::MavCmd::MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION
        }
        MavlinkCommonCommandName::FenceCircleExclusion => {
            common::MavCmd::MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION
        }
        MavlinkCommonCommandName::FencePolygonVertexInclusion => {
            common::MavCmd::MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION
        }
        MavlinkCommonCommandName::FencePolygonVertexExclusion => {
            common::MavCmd::MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION
        }
        MavlinkCommonCommandName::DoFenceEnable => common::MavCmd::MAV_CMD_DO_FENCE_ENABLE,
    }
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
pub(super) fn send_command_and_wait_observed<C, O>(
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
pub(super) fn wait_command_ack<C: MavlinkVehicleConnection>(
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
pub(super) fn wait_for_post_start_heartbeat<C: MavlinkVehicleConnection>(
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
pub(super) fn send_abort_command<C: MavlinkVehicleConnection>(
    conn: &mut C,
    options: &MissionLifecycleOptions,
) -> AbortCommandResult {
    let mut observer = NoopMavlinkMissionObserver;
    send_abort_command_observed(conn, options, &mut observer)
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn send_abort_command_observed<C, O>(
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
pub(super) fn attach_abort_result(
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
pub(super) fn mission_error_to_telemetry_error(
    error: MavlinkMissionError,
) -> MavlinkTelemetryError {
    match error {
        MavlinkMissionError::ReadFailed(message) => MavlinkTelemetryError::ReadFailed(message),
        other => MavlinkTelemetryError::ReadFailed(other.to_string()),
    }
}
