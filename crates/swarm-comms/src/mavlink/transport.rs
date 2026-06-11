use std::collections::VecDeque;

use crate::{RawMessage, Transport};

use super::{MavlinkError, Waypoint};

#[cfg(feature = "mavlink-transport")]
use std::{borrow::Cow, time::Duration};

#[cfg(feature = "mavlink-transport")]
use super::commands::{
    common_command_to_message, send_abort_command, send_command_and_wait_observed,
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
    MavlinkLifecycleError, MavlinkMissionError, MavlinkMissionObserver, MavlinkTelemetryError,
    MavlinkTelemetryEvent, MissionItem, MissionLifecycleOptions, MissionLifecycleReport,
    MissionUploadOptions, MissionUploadReport, NoopMavlinkMissionObserver,
};
#[cfg(feature = "mavlink-transport")]
use crate::mavlink_common_plan::{
    MavlinkCommonCommand, MavlinkCommonCommandName, MavlinkCommonMissionItem, MavlinkCommonPlan,
};
#[cfg(feature = "mavlink-transport")]
use crate::mavlink_executor::{AckProvider, MavlinkExecutionStepResult};

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
    transport: &'a mut MavlinkTransport,
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
            transport,
            mission_items: plan.mission_items.clone(),
            mission_start: plan.mission_start.clone(),
            upload_options,
            lifecycle_options,
            observer,
        }
    }

    fn ack_command(&mut self, command: &MavlinkCommonCommand) -> MavlinkExecutionStepResult {
        let msg = common_command_to_message(
            command,
            self.lifecycle_options.target_system,
            self.lifecycle_options.target_component,
        );
        lifecycle_result_to_step(
            send_command_and_wait_observed(
                &mut self.transport.conn,
                msg,
                self.lifecycle_options.timeout,
                self.observer,
            ),
            self.lifecycle_options.timeout,
        )
    }
}

#[cfg(feature = "mavlink-transport")]
impl<O: MavlinkMissionObserver> AckProvider for MavlinkTransportAckProvider<'_, O> {
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
                &mut self.transport.conn,
                &self.mission_items,
                &self.upload_options,
                self.observer,
            ),
            self.upload_options.timeout,
        )
    }

    fn ack_mission_start(&mut self) -> MavlinkExecutionStepResult {
        let command = self
            .mission_start
            .clone()
            .unwrap_or_else(default_mission_start_command);
        self.ack_command(&command)
    }

    fn ack_postlude_command(
        &mut self,
        command: &MavlinkCommonCommand,
    ) -> MavlinkExecutionStepResult {
        self.ack_command(command)
    }
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
