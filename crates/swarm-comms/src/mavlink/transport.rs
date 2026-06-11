use std::collections::VecDeque;

use crate::{RawMessage, Transport};

use super::{MavlinkError, Waypoint};

#[cfg(feature = "mavlink-transport")]
use std::{borrow::Cow, time::Duration};

#[cfg(feature = "mavlink-transport")]
use mavlink::{dialects::common, types::CharArray};

#[cfg(feature = "mavlink-transport")]
use super::commands::{
    common_command_to_message, send_abort_command, send_abort_command_observed,
    send_command_and_wait_observed, wait_for_post_start_heartbeat,
};
#[cfg(feature = "mavlink-transport")]
use super::{
    lifecycle::execute_uploaded_mission_with_connection_observed,
    mission_upload::{
        upload_and_execute_mission_with_connection_observed,
        upload_mission_items_with_connection_observed, upload_mission_with_connection_observed,
        upload_precompiled_mission_items_with_connection_observed,
    },
    telemetry::{poll_telemetry_event_with_connection, wait_next_telemetry_event_with_connection},
    AbortCommandResult, CommonMessage, MavlinkFlightError, MavlinkFlightReport,
    MavlinkLifecycleError, MavlinkMissionError, MavlinkMissionEvent, MavlinkMissionObserver,
    MavlinkTelemetryError, MavlinkTelemetryEvent, MissionItem, MissionLifecycleOptions,
    MissionLifecycleReport, MissionUploadOptions, MissionUploadReport, NoopMavlinkMissionObserver,
};
#[cfg(feature = "mavlink-transport")]
use crate::mavlink_common_plan::{
    MavlinkCommonCommand, MavlinkCommonCommandName, MavlinkCommonMissionItem, MavlinkCommonPlan,
};
#[cfg(feature = "mavlink-transport")]
use crate::mavlink_executor::{
    AckProvider, FcConfigError, FcConfigProvider, FcParamWriteOk, FcParamWriteResult,
    GeofenceUploadOk, GeofenceUploadResult, MavlinkExecutionStepResult,
};
#[cfg(feature = "mavlink-transport")]
use crate::mavlink_geofence::MavlinkFencePlan;
#[cfg(feature = "mavlink-transport")]
use crate::mavlink_parameters::{
    FcParamId, FcParamRequirement, FcParamSnapshot, FcParamValue, FcParamWritePlan,
};

/// Mock MAVLink transport for unit tests and --mock mode.
pub struct MockMavlinkTransport {
    sent: Vec<RawMessage>,
    inbox: VecDeque<RawMessage>,
    waypoints: Vec<Waypoint>,
}

impl MockMavlinkTransport {
    pub fn new() -> Self {
        Self {
            sent: Vec::new(),
            inbox: VecDeque::new(),
            waypoints: Vec::new(),
        }
    }

    pub fn sent_messages(&self) -> &[RawMessage] {
        &self.sent
    }

    pub fn push_incoming(&mut self, msg: RawMessage) {
        self.inbox.push_back(msg);
    }

    pub fn waypoints(&self) -> &[Waypoint] {
        &self.waypoints
    }

    pub fn send_waypoint(&mut self, wp: Waypoint) {
        self.waypoints.push(wp);
    }
}

impl Default for MockMavlinkTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for MockMavlinkTransport {
    type Error = MavlinkError;

    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        self.sent.push(msg);
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
        Ok(self.inbox.pop_front())
    }
}

/// Wraps a MAVLink connection for use with the swarm Transport trait.
/// Only available with feature "mavlink-transport".
#[cfg(feature = "mavlink-transport")]
pub struct MavlinkTransport {
    pub(super) conn: mavlink::Connection<CommonMessage>,
    pub(super) agent_id: swarm_types::AgentId,
    pub(super) recv_buf: VecDeque<RawMessage>,
}

#[cfg(feature = "mavlink-transport")]
impl MavlinkTransport {
    pub fn new(
        connection_string: &str,
        agent_id: swarm_types::AgentId,
    ) -> Result<Self, MavlinkError> {
        let connection_string = normalize_mavlink_connection_string(connection_string);
        let conn: mavlink::Connection<CommonMessage> = mavlink::connect(connection_string.as_ref())
            .map_err(|e: std::io::Error| MavlinkError::Connection(e.to_string()))?;
        Ok(Self {
            conn,
            agent_id,
            recv_buf: VecDeque::new(),
        })
    }

    pub fn upload_mission(
        &mut self,
        waypoints: &[Waypoint],
        options: MissionUploadOptions,
    ) -> Result<MissionUploadReport, MavlinkMissionError> {
        let mut observer = NoopMavlinkMissionObserver;
        upload_mission_with_connection_observed(&mut self.conn, waypoints, &options, &mut observer)
    }

    pub fn upload_mission_observed<O: MavlinkMissionObserver>(
        &mut self,
        waypoints: &[Waypoint],
        options: MissionUploadOptions,
        observer: &mut O,
    ) -> Result<MissionUploadReport, MavlinkMissionError> {
        upload_mission_with_connection_observed(&mut self.conn, waypoints, &options, observer)
    }

    /// Upload a typed `MissionItem` list (loiter, orbit, land, …) to the vehicle.
    pub fn upload_mission_items(
        &mut self,
        items: &[MissionItem],
        options: MissionUploadOptions,
    ) -> Result<MissionUploadReport, MavlinkMissionError> {
        let mut observer = NoopMavlinkMissionObserver;
        upload_mission_items_with_connection_observed(
            &mut self.conn,
            items,
            &options,
            &mut observer,
        )
    }

    /// Upload a typed `MissionItem` list with event observation.
    pub fn upload_mission_items_observed<O: MavlinkMissionObserver>(
        &mut self,
        items: &[MissionItem],
        options: MissionUploadOptions,
        observer: &mut O,
    ) -> Result<MissionUploadReport, MavlinkMissionError> {
        upload_mission_items_with_connection_observed(&mut self.conn, items, &options, observer)
    }

    pub fn execute_uploaded_mission(
        &mut self,
        options: MissionLifecycleOptions,
    ) -> Result<MissionLifecycleReport, MavlinkLifecycleError> {
        let mut observer = NoopMavlinkMissionObserver;
        execute_uploaded_mission_with_connection_observed(&mut self.conn, &options, &mut observer)
    }

    pub fn execute_uploaded_mission_observed<O: MavlinkMissionObserver>(
        &mut self,
        options: MissionLifecycleOptions,
        observer: &mut O,
    ) -> Result<MissionLifecycleReport, MavlinkLifecycleError> {
        execute_uploaded_mission_with_connection_observed(&mut self.conn, &options, observer)
    }

    pub fn upload_and_execute_mission(
        &mut self,
        waypoints: &[Waypoint],
        upload_options: MissionUploadOptions,
        lifecycle_options: MissionLifecycleOptions,
    ) -> Result<MavlinkFlightReport, MavlinkFlightError> {
        let mut observer = NoopMavlinkMissionObserver;
        upload_and_execute_mission_with_connection_observed(
            &mut self.conn,
            waypoints,
            &upload_options,
            &lifecycle_options,
            &mut observer,
        )
    }

    pub fn upload_and_execute_mission_observed<O: MavlinkMissionObserver>(
        &mut self,
        waypoints: &[Waypoint],
        upload_options: MissionUploadOptions,
        lifecycle_options: MissionLifecycleOptions,
        observer: &mut O,
    ) -> Result<MavlinkFlightReport, MavlinkFlightError> {
        upload_and_execute_mission_with_connection_observed(
            &mut self.conn,
            waypoints,
            &upload_options,
            &lifecycle_options,
            observer,
        )
    }

    pub fn abort_mission(&mut self, options: &MissionLifecycleOptions) -> AbortCommandResult {
        send_abort_command(&mut self.conn, options)
    }

    pub fn poll_telemetry_event(
        &mut self,
    ) -> Result<Option<MavlinkTelemetryEvent>, MavlinkTelemetryError> {
        poll_telemetry_event_with_connection(&mut self.conn)
    }

    pub fn wait_next_telemetry_event(
        &mut self,
        timeout: Duration,
    ) -> Result<MavlinkTelemetryEvent, MavlinkTelemetryError> {
        wait_next_telemetry_event_with_connection(&mut self.conn, timeout)
    }
}

/// MAVLink transport-backed ACK provider for `MavlinkPlanExecutor`.
#[cfg(feature = "mavlink-transport")]
pub struct MavlinkTransportAckProvider<'a, O: MavlinkMissionObserver> {
    inner: MavlinkConnectionAckProvider<'a, mavlink::Connection<CommonMessage>, O>,
}

#[cfg(feature = "mavlink-transport")]
pub(super) struct MavlinkConnectionAckProvider<
    'a,
    C: super::mission_upload::MavlinkVehicleConnection,
    O,
> {
    conn: &'a mut C,
    mission_items: Vec<MavlinkCommonMissionItem>,
    mission_start: Option<MavlinkCommonCommand>,
    upload_options: MissionUploadOptions,
    lifecycle_options: MissionLifecycleOptions,
    observer: &'a mut O,
}

#[cfg(feature = "mavlink-transport")]
impl<'a, O: MavlinkMissionObserver> MavlinkTransportAckProvider<'a, O> {
    pub fn new(
        transport: &'a mut MavlinkTransport,
        plan: &MavlinkCommonPlan,
        upload_options: MissionUploadOptions,
        lifecycle_options: MissionLifecycleOptions,
        observer: &'a mut O,
    ) -> Self {
        Self {
            inner: MavlinkConnectionAckProvider {
                conn: &mut transport.conn,
                mission_items: plan.mission_items.clone(),
                mission_start: plan.mission_start.clone(),
                upload_options,
                lifecycle_options,
                observer,
            },
        }
    }
}

#[cfg(feature = "mavlink-transport")]
impl<C, O> MavlinkConnectionAckProvider<'_, C, O>
where
    C: super::mission_upload::MavlinkVehicleConnection,
    O: MavlinkMissionObserver,
{
    #[cfg(test)]
    pub(super) fn new_for_connection<'a>(
        conn: &'a mut C,
        plan: &MavlinkCommonPlan,
        upload_options: MissionUploadOptions,
        lifecycle_options: MissionLifecycleOptions,
        observer: &'a mut O,
    ) -> MavlinkConnectionAckProvider<'a, C, O> {
        MavlinkConnectionAckProvider {
            conn,
            mission_items: plan.mission_items.clone(),
            mission_start: plan.mission_start.clone(),
            upload_options,
            lifecycle_options,
            observer,
        }
    }

    fn ack_command(&mut self, command: &MavlinkCommonCommand) -> MavlinkExecutionStepResult {
        if should_skip_arm_command(command, &self.lifecycle_options) {
            return MavlinkExecutionStepResult::Skipped {
                reason: "no_arm lifecycle option skips MAV_CMD_COMPONENT_ARM_DISARM arm command"
                    .to_owned(),
            };
        }
        let msg = common_command_to_message(
            command,
            self.lifecycle_options.target_system,
            self.lifecycle_options.target_component,
        );
        lifecycle_result_to_step(
            send_command_and_wait_observed(
                self.conn,
                msg,
                self.lifecycle_options.timeout,
                self.observer,
            ),
            self.lifecycle_options.timeout,
        )
    }

    fn ack_mission_start_with_heartbeat(&mut self) -> MavlinkExecutionStepResult {
        let command = self
            .mission_start
            .clone()
            .unwrap_or_else(default_mission_start_command);
        let result = self.ack_command(&command);
        if result != MavlinkExecutionStepResult::Accepted {
            return result;
        }
        match wait_for_post_start_heartbeat(self.conn, self.lifecycle_options.timeout) {
            Ok(()) => {
                self.observer.on_event(MavlinkMissionEvent::HeartbeatSeen);
                MavlinkExecutionStepResult::Accepted
            }
            Err(error) => {
                let abort_result =
                    send_abort_command_observed(self.conn, &self.lifecycle_options, self.observer);
                lifecycle_result_to_step(
                    Err(attach_abort_to_lifecycle_error(error, abort_result)),
                    self.lifecycle_options.timeout,
                )
            }
        }
    }
}

#[cfg(feature = "mavlink-transport")]
impl<O: MavlinkMissionObserver> AckProvider for MavlinkTransportAckProvider<'_, O> {
    fn ack_prelude_command(
        &mut self,
        command: &MavlinkCommonCommand,
    ) -> MavlinkExecutionStepResult {
        self.inner.ack_command(command)
    }

    fn ack_mission_upload(&mut self) -> MavlinkExecutionStepResult {
        self.inner.ack_mission_upload()
    }

    fn ack_mission_start(&mut self) -> MavlinkExecutionStepResult {
        self.inner.ack_mission_start_with_heartbeat()
    }

    fn ack_postlude_command(
        &mut self,
        command: &MavlinkCommonCommand,
    ) -> MavlinkExecutionStepResult {
        self.inner.ack_command(command)
    }
}

#[cfg(feature = "mavlink-transport")]
impl<C, O> AckProvider for MavlinkConnectionAckProvider<'_, C, O>
where
    C: super::mission_upload::MavlinkVehicleConnection,
    O: MavlinkMissionObserver,
{
    fn ack_prelude_command(
        &mut self,
        command: &MavlinkCommonCommand,
    ) -> MavlinkExecutionStepResult {
        self.ack_command(command)
    }

    fn ack_mission_upload(&mut self) -> MavlinkExecutionStepResult {
        if self.mission_items.is_empty() {
            return MavlinkExecutionStepResult::Rejected {
                reason: "compiled plan has no supported transport mission items".to_owned(),
            };
        }
        mission_result_to_step(
            upload_precompiled_mission_items_with_connection_observed(
                self.conn,
                &self.mission_items,
                &self.upload_options,
                self.observer,
            ),
            self.upload_options.timeout,
        )
    }

    fn ack_mission_start(&mut self) -> MavlinkExecutionStepResult {
        self.ack_mission_start_with_heartbeat()
    }

    fn ack_postlude_command(
        &mut self,
        command: &MavlinkCommonCommand,
    ) -> MavlinkExecutionStepResult {
        self.ack_command(command)
    }
}

#[cfg(feature = "mavlink-transport")]
fn should_skip_arm_command(
    command: &MavlinkCommonCommand,
    options: &MissionLifecycleOptions,
) -> bool {
    options.no_arm
        && command.command == MavlinkCommonCommandName::ComponentArmDisarm
        && command.params[0].is_some_and(|param| (param - 1.0).abs() < f64::EPSILON)
}

#[cfg(feature = "mavlink-transport")]
fn default_mission_start_command() -> MavlinkCommonCommand {
    MavlinkCommonCommand {
        command_id: "mission-start-0".to_owned(),
        command: MavlinkCommonCommandName::MissionStart,
        phase: crate::mavlink_common_plan::MavlinkPlanPhase::MissionStart,
        params: [
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
        ],
    }
}

#[cfg(feature = "mavlink-transport")]
fn attach_abort_to_lifecycle_error(
    error: MavlinkLifecycleError,
    abort_result: AbortCommandResult,
) -> MavlinkLifecycleError {
    match error {
        MavlinkLifecycleError::PostStartHeartbeatTimeout { .. } => {
            MavlinkLifecycleError::PostStartHeartbeatTimeout { abort_result }
        }
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
fn mission_result_to_step(
    result: Result<MissionUploadReport, MavlinkMissionError>,
    timeout: Duration,
) -> MavlinkExecutionStepResult {
    match result {
        Ok(_) => MavlinkExecutionStepResult::Accepted,
        Err(MavlinkMissionError::MissionAckTimeout)
        | Err(MavlinkMissionError::MissionRequestTimeout { .. })
        | Err(MavlinkMissionError::HeartbeatTimeout) => MavlinkExecutionStepResult::Timeout {
            after_ms: timeout.as_millis() as u64,
        },
        Err(MavlinkMissionError::MissionRejected(result)) => MavlinkExecutionStepResult::Rejected {
            reason: format!("{result:?}"),
        },
        Err(MavlinkMissionError::WriteFailed(message))
        | Err(MavlinkMissionError::ReadFailed(message)) => {
            MavlinkExecutionStepResult::TransportFailure { reason: message }
        }
        Err(error) => MavlinkExecutionStepResult::Rejected {
            reason: error.to_string(),
        },
    }
}

/// MAVLink transport-backed provider for FC configuration operations.
#[cfg(feature = "mavlink-transport")]
pub struct MavlinkTransportFcConfigProvider<'a> {
    inner: MavlinkConnectionFcConfigProvider<'a, mavlink::Connection<CommonMessage>>,
}

#[cfg(feature = "mavlink-transport")]
impl<'a> MavlinkTransportFcConfigProvider<'a> {
    pub fn new(
        transport: &'a mut MavlinkTransport,
        profile: crate::mavlink_capability_profile::MavlinkCapabilityProfileId,
        _upload_options: MissionUploadOptions,
        lifecycle_options: MissionLifecycleOptions,
    ) -> Self {
        Self {
            inner: MavlinkConnectionFcConfigProvider {
                conn: &mut transport.conn,
                profile,
                lifecycle_options,
            },
        }
    }
}

#[cfg(feature = "mavlink-transport")]
impl FcConfigProvider for MavlinkTransportFcConfigProvider<'_> {
    fn upload_fence(&mut self, plan: &MavlinkFencePlan) -> GeofenceUploadResult {
        self.inner.upload_fence(plan)
    }

    fn read_params(
        &mut self,
        requirements: &[FcParamRequirement],
    ) -> Result<FcParamSnapshot, FcConfigError> {
        self.inner.read_params(requirements)
    }

    fn write_params(&mut self, plan: &FcParamWritePlan) -> FcParamWriteResult {
        self.inner.write_params(plan)
    }
}

#[cfg(feature = "mavlink-transport")]
pub(super) struct MavlinkConnectionFcConfigProvider<
    'a,
    C: super::mission_upload::MavlinkVehicleConnection,
> {
    conn: &'a mut C,
    profile: crate::mavlink_capability_profile::MavlinkCapabilityProfileId,
    lifecycle_options: MissionLifecycleOptions,
}

#[cfg(feature = "mavlink-transport")]
impl<C> FcConfigProvider for MavlinkConnectionFcConfigProvider<'_, C>
where
    C: super::mission_upload::MavlinkVehicleConnection,
{
    fn upload_fence(&mut self, plan: &MavlinkFencePlan) -> GeofenceUploadResult {
        if plan.items.is_empty() && !plan.enable_fence {
            return Ok(GeofenceUploadOk {
                items_uploaded: 0,
                fence_enable_sent: false,
            });
        }

        Err(FcConfigError::Unsupported {
            operation: "geofence_upload".to_owned(),
            reason: format!(
                "profile {:?}: current mavlink Common generated MISSION_COUNT/MISSION_ITEM_INT messages do not expose mission_type=MAV_MISSION_TYPE_FENCE; refusing ordinary mission upload fallback",
                self.profile
            ),
        })
    }

    fn read_params(
        &mut self,
        requirements: &[FcParamRequirement],
    ) -> Result<FcParamSnapshot, FcConfigError> {
        let mut params = std::collections::HashMap::new();
        for requirement in requirements {
            let value = request_param_value(
                self.conn,
                &requirement.param_id,
                self.lifecycle_options.target_system,
                self.lifecycle_options.target_component,
                self.lifecycle_options.timeout,
            )?;
            params.insert(requirement.param_id.clone(), value);
        }
        Ok(FcParamSnapshot {
            params,
            description: "transport-backed MAVLink PARAM_VALUE snapshot".to_owned(),
        })
    }

    fn write_params(&mut self, plan: &FcParamWritePlan) -> FcParamWriteResult {
        for (param_id, value) in &plan.writes {
            write_param_value(
                self.conn,
                param_id,
                *value,
                self.lifecycle_options.target_system,
                self.lifecycle_options.target_component,
                self.lifecycle_options.timeout,
            )?;
        }
        Ok(FcParamWriteOk {
            written_count: plan.writes.len(),
        })
    }
}

#[cfg(all(feature = "mavlink-transport", test))]
impl<'a, C> MavlinkConnectionFcConfigProvider<'a, C>
where
    C: super::mission_upload::MavlinkVehicleConnection,
{
    pub(super) fn new_for_connection(
        conn: &'a mut C,
        profile: crate::mavlink_capability_profile::MavlinkCapabilityProfileId,
        _upload_options: MissionUploadOptions,
        lifecycle_options: MissionLifecycleOptions,
    ) -> Self {
        Self {
            conn,
            profile,
            lifecycle_options,
        }
    }
}

#[cfg(feature = "mavlink-transport")]
fn request_param_value<C>(
    conn: &mut C,
    param_id: &FcParamId,
    target_system: u8,
    target_component: u8,
    timeout: Duration,
) -> Result<FcParamValue, FcConfigError>
where
    C: super::mission_upload::MavlinkVehicleConnection,
{
    conn.send_message(CommonMessage::PARAM_REQUEST_READ(
        common::PARAM_REQUEST_READ_DATA {
            param_id: param_id_to_char_array(param_id),
            target_system,
            target_component,
            param_index: -1,
        },
    ))
    .map_err(|error| FcConfigError::TransportFailure {
        operation: format!("param_read:{}", param_id.as_ref().as_str()),
        message: error.to_string(),
    })?;
    wait_param_value(conn, param_id, timeout, "param_read")
}

#[cfg(feature = "mavlink-transport")]
fn write_param_value<C>(
    conn: &mut C,
    param_id: &FcParamId,
    value: FcParamValue,
    target_system: u8,
    target_component: u8,
    timeout: Duration,
) -> FcParamWriteResult
where
    C: super::mission_upload::MavlinkVehicleConnection,
{
    conn.send_message(CommonMessage::PARAM_SET(common::PARAM_SET_DATA {
        param_value: param_value_to_wire(value),
        target_system,
        target_component,
        param_id: param_id_to_char_array(param_id),
        param_type: param_value_type(value),
    }))
    .map_err(|error| FcConfigError::TransportFailure {
        operation: format!("param_write:{}", param_id.as_ref().as_str()),
        message: error.to_string(),
    })?;
    let confirmed = wait_param_value(conn, param_id, timeout, "param_write")?;
    if confirmed == value {
        Ok(FcParamWriteOk { written_count: 1 })
    } else {
        Err(FcConfigError::Mismatch {
            operation: format!("param_write:{}", param_id.as_ref().as_str()),
            expected: format!("{value:?}"),
            actual: format!("{confirmed:?}"),
        })
    }
}

#[cfg(feature = "mavlink-transport")]
fn wait_param_value<C>(
    conn: &mut C,
    param_id: &FcParamId,
    timeout: Duration,
    operation: &str,
) -> Result<FcParamValue, FcConfigError>
where
    C: super::mission_upload::MavlinkVehicleConnection,
{
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some((_header, CommonMessage::PARAM_VALUE(value))) = conn
            .try_recv_message()
            .map_err(|error| FcConfigError::TransportFailure {
                operation: format!("{operation}:{}", param_id.as_ref().as_str()),
                message: error.to_string(),
            })?
        {
            if char_array_matches_param_id(&value.param_id, param_id) {
                return param_value_from_wire(value.param_value, value.param_type).map_err(
                    |reason| FcConfigError::Unsupported {
                        operation: format!("{operation}:{}", param_id.as_ref().as_str()),
                        reason,
                    },
                );
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(FcConfigError::Timeout {
                operation: format!("{operation}:{}", param_id.as_ref().as_str()),
                expected: format!("PARAM_VALUE {}", param_id.as_ref().as_str()),
            });
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(feature = "mavlink-transport")]
fn param_id_to_char_array(param_id: &FcParamId) -> CharArray<16> {
    param_id.as_ref().as_str().into()
}

#[cfg(feature = "mavlink-transport")]
fn char_array_matches_param_id(actual: &CharArray<16>, expected: &FcParamId) -> bool {
    actual
        .to_str()
        .is_ok_and(|actual| actual == expected.as_ref().as_str())
}

#[cfg(feature = "mavlink-transport")]
fn param_value_to_wire(value: FcParamValue) -> f32 {
    match value {
        FcParamValue::Int32(value) => value as f32,
        FcParamValue::Float(value) => value,
    }
}

#[cfg(feature = "mavlink-transport")]
fn param_value_type(value: FcParamValue) -> common::MavParamType {
    match value {
        FcParamValue::Int32(_) => common::MavParamType::MAV_PARAM_TYPE_INT32,
        FcParamValue::Float(_) => common::MavParamType::MAV_PARAM_TYPE_REAL32,
    }
}

#[cfg(feature = "mavlink-transport")]
fn param_value_from_wire(
    value: f32,
    param_type: common::MavParamType,
) -> Result<FcParamValue, String> {
    match param_type {
        common::MavParamType::MAV_PARAM_TYPE_INT8
        | common::MavParamType::MAV_PARAM_TYPE_UINT8
        | common::MavParamType::MAV_PARAM_TYPE_INT16
        | common::MavParamType::MAV_PARAM_TYPE_UINT16
        | common::MavParamType::MAV_PARAM_TYPE_INT32
        | common::MavParamType::MAV_PARAM_TYPE_UINT32 => Ok(FcParamValue::Int32(value as i32)),
        common::MavParamType::MAV_PARAM_TYPE_REAL32 => Ok(FcParamValue::Float(value)),
        other => Err(format!("unsupported PARAM_VALUE type {other:?}")),
    }
}

#[cfg(feature = "mavlink-transport")]
fn lifecycle_result_to_step(
    result: Result<(), MavlinkLifecycleError>,
    timeout: Duration,
) -> MavlinkExecutionStepResult {
    match result {
        Ok(()) => MavlinkExecutionStepResult::Accepted,
        Err(MavlinkLifecycleError::CommandAckTimeout { .. })
        | Err(MavlinkLifecycleError::PostStartHeartbeatTimeout { .. }) => {
            MavlinkExecutionStepResult::Timeout {
                after_ms: timeout.as_millis() as u64,
            }
        }
        Err(MavlinkLifecycleError::CommandRejected { result, .. }) => {
            MavlinkExecutionStepResult::Rejected {
                reason: format!("{result:?}"),
            }
        }
        Err(MavlinkLifecycleError::WriteFailed(message))
        | Err(MavlinkLifecycleError::ReadFailed(message)) => {
            MavlinkExecutionStepResult::TransportFailure { reason: message }
        }
        Err(error) => MavlinkExecutionStepResult::Rejected {
            reason: error.to_string(),
        },
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
pub(super) fn normalize_mavlink_connection_string(connection_string: &str) -> Cow<'_, str> {
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
pub(super) fn reject_raw_transport_send(_msg: RawMessage) -> Result<(), MavlinkError> {
    Err(MavlinkError::UnsupportedRawTransportSend)
}
