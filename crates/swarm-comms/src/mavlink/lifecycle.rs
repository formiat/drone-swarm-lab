#[cfg(test)]
use super::NoopMavlinkMissionObserver;
#[cfg(feature = "mavlink-transport")]
use super::{
    commands::{
        arm_command, attach_abort_result, send_abort_command_observed,
        send_command_and_wait_observed, start_mission_command, takeoff_command,
        wait_for_post_start_heartbeat,
    },
    mission_upload::MavlinkVehicleConnection,
    AbortCommandResult, MavlinkLifecycleError, MavlinkMissionEvent, MavlinkMissionObserver,
    MissionLifecycleOptions, MissionLifecycleReport,
};

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
