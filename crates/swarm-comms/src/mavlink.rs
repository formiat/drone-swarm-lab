#[cfg(feature = "mavlink-transport")]
use std::borrow::Cow;
use std::collections::VecDeque;
#[cfg(feature = "mavlink-transport")]
use std::io::ErrorKind;
#[cfg(feature = "mavlink-transport")]
use std::time::{Duration, Instant};

use swarm_types::TaskStatus;

use crate::{RawMessage, Transport};

#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;

#[cfg(feature = "mavlink-transport")]
type CommonHeader = mavlink::MavHeader;
#[cfg(feature = "mavlink-transport")]
type CommonMessage = common::MavMessage;

#[derive(Debug, thiserror::Error)]
pub enum MavlinkError {
    #[error("mavlink connection error: {0}")]
    Connection(String),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("transport not connected")]
    NotConnected,
    #[error("no pose on task")]
    NoPose,
    #[error(
        "generic MAVLink Transport::send is unsupported; use mission upload/lifecycle APIs for PX4 SITL"
    )]
    UnsupportedRawTransportSend,
}

/// A waypoint in local coordinate space (no MAVLink dependency).
#[derive(Debug, Clone)]
pub struct Waypoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub seq: u16,
}

/// Origin used to convert local simulation coordinates into WGS84 mission items.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissionHomeOrigin {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub alt_m: f64,
}

#[cfg(feature = "mavlink-transport")]
impl Default for MissionHomeOrigin {
    fn default() -> Self {
        Self {
            lat_deg: 47.397_742,
            lon_deg: 8.545_594,
            alt_m: 0.0,
        }
    }
}

/// MAVLink frame used for uploaded waypoint missions.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionFrame {
    GlobalRelativeAlt,
    LocalNed,
}

#[cfg(feature = "mavlink-transport")]
impl MissionFrame {
    #[allow(deprecated)]
    fn to_mav_frame(self) -> Result<common::MavFrame, MavlinkMissionError> {
        match self {
            Self::GlobalRelativeAlt => Ok(common::MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT),
            Self::LocalNed => Err(MavlinkMissionError::UnsupportedFrame),
        }
    }
}

/// Options for the minimal MAVLink mission upload transaction.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone)]
pub struct MissionUploadOptions {
    pub target_system: u8,
    pub target_component: u8,
    pub timeout: Duration,
    pub retry_count: u8,
    pub clear_existing: bool,
    pub home_origin: MissionHomeOrigin,
    pub frame: MissionFrame,
}

#[cfg(feature = "mavlink-transport")]
impl Default for MissionUploadOptions {
    fn default() -> Self {
        Self {
            target_system: 1,
            target_component: 1,
            timeout: Duration::from_secs(2),
            retry_count: 0,
            clear_existing: true,
            home_origin: MissionHomeOrigin::default(),
            frame: MissionFrame::GlobalRelativeAlt,
        }
    }
}

/// Summary returned after PX4 accepts a waypoint mission upload.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq)]
pub struct MissionUploadReport {
    pub uploaded_count: usize,
    pub target_system: u8,
    pub target_component: u8,
    pub ack: common::MavMissionResult,
    pub cleared_existing: bool,
}

/// Options for the controlled post-upload flight lifecycle.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone)]
pub struct MissionLifecycleOptions {
    pub target_system: u8,
    pub target_component: u8,
    pub timeout: Duration,
    pub no_arm: bool,
    pub abort_after: Option<Duration>,
    pub takeoff_altitude_m: f32,
}

#[cfg(feature = "mavlink-transport")]
impl Default for MissionLifecycleOptions {
    fn default() -> Self {
        Self {
            target_system: 1,
            target_component: 1,
            timeout: Duration::from_secs(2),
            no_arm: false,
            abort_after: None,
            takeoff_altitude_m: 2.5,
        }
    }
}

/// Result of an attempted abort command during lifecycle execution.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq)]
pub enum AbortCommandResult {
    NotAttempted,
    Accepted,
    Rejected(common::MavResult),
    AckTimeout,
    Failed(String),
}

/// Summary returned after PX4 accepts lifecycle commands.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MissionLifecycleReport {
    pub armed: bool,
    pub took_off: bool,
    pub started: bool,
    pub post_start_heartbeat: bool,
    pub abort_result: Option<AbortCommandResult>,
}

/// Summary returned by the combined upload + execute workflow.
#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq)]
pub struct MavlinkFlightReport {
    pub upload: MissionUploadReport,
    pub lifecycle: MissionLifecycleReport,
}

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MavlinkMissionEvent {
    HeartbeatSeen,
    MissionClearSent,
    MissionCountSent {
        count: usize,
    },
    MissionItemRequested {
        seq: u16,
    },
    MissionItemSent {
        seq: u16,
    },
    MissionAckReceived {
        result: String,
        accepted: bool,
    },
    CommandSent {
        command: String,
    },
    CommandAckReceived {
        command: String,
        result: String,
        accepted: bool,
    },
    AbortRequested {
        result: String,
    },
}

#[cfg(feature = "mavlink-transport")]
pub trait MavlinkMissionObserver {
    fn on_event(&mut self, event: MavlinkMissionEvent);
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Default)]
pub struct NoopMavlinkMissionObserver;

#[cfg(feature = "mavlink-transport")]
impl MavlinkMissionObserver for NoopMavlinkMissionObserver {
    fn on_event(&mut self, _event: MavlinkMissionEvent) {}
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MavlinkTelemetryError {
    #[error("timed out waiting for MAVLink telemetry event after {timeout:?}")]
    Timeout { timeout: Duration },
    #[error("mavlink telemetry read failed: {0}")]
    ReadFailed(String),
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum MavlinkLifecycleError {
    #[error("invalid takeoff altitude: {altitude_m}")]
    InvalidTakeoffAltitude { altitude_m: f32 },
    #[error("timed out waiting for command ack: {command:?}; abort_result={abort_result:?}")]
    CommandAckTimeout {
        command: common::MavCmd,
        abort_result: Option<AbortCommandResult>,
    },
    #[error(
        "command rejected by vehicle: {command:?} result={result:?}; abort_result={abort_result:?}"
    )]
    CommandRejected {
        command: common::MavCmd,
        result: common::MavResult,
        abort_result: Option<AbortCommandResult>,
    },
    #[error("post-start heartbeat timeout; abort_result={abort_result:?}")]
    PostStartHeartbeatTimeout { abort_result: AbortCommandResult },
    #[error("abort command failed: {abort_result:?}")]
    AbortFailed { abort_result: AbortCommandResult },
    #[error("mavlink write failed: {0}")]
    WriteFailed(String),
    #[error("mavlink read failed: {0}")]
    ReadFailed(String),
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum MavlinkFlightError {
    #[error("mission upload failed: {0}")]
    MissionUpload(#[from] MavlinkMissionError),
    #[error("mission lifecycle failed: {0}")]
    Lifecycle(#[from] MavlinkLifecycleError),
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum MavlinkMissionError {
    #[error("mission upload requires at least one waypoint")]
    EmptyMission,
    #[error("mission contains too many waypoints: {count}")]
    TooManyWaypoints { count: usize },
    #[error("timed out waiting for MAVLink HEARTBEAT")]
    HeartbeatTimeout,
    #[error("timed out waiting for mission request seq={expected_seq}")]
    MissionRequestTimeout { expected_seq: u16 },
    #[error("timed out waiting for mission ack")]
    MissionAckTimeout,
    #[error("unexpected mission request sequence: expected {expected}, got {actual}")]
    UnexpectedRequestSeq { expected: u16, actual: u16 },
    #[error("mission rejected by vehicle: {0:?}")]
    MissionRejected(common::MavMissionResult),
    #[error("mission frame is not supported by this uploader")]
    UnsupportedFrame,
    #[error("coordinate conversion failed: {0}")]
    Conversion(String),
    #[error("mavlink write failed: {0}")]
    WriteFailed(String),
    #[error("mavlink read failed: {0}")]
    ReadFailed(String),
}

/// Convert a Task to a Waypoint.
pub fn task_to_waypoint(task: &swarm_types::Task) -> Option<Waypoint> {
    task.pose.map(|pose| Waypoint {
        x: pose.x,
        y: pose.y,
        z: pose.z,
        seq: 0,
    })
}

/// Derive TaskStatus from a boolean acknowledgement flag (mock path).
pub fn waypoint_status_to_task_status(ack: bool) -> TaskStatus {
    if ack {
        TaskStatus::Completed
    } else {
        TaskStatus::InProgress
    }
}

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
    conn: mavlink::Connection<CommonMessage>,
    agent_id: swarm_types::AgentId,
    recv_buf: VecDeque<RawMessage>,
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

#[cfg(feature = "mavlink-transport")]
trait MavlinkVehicleConnection {
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
fn upload_mission_with_connection<C: MavlinkVehicleConnection>(
    conn: &mut C,
    waypoints: &[Waypoint],
    options: &MissionUploadOptions,
) -> Result<MissionUploadReport, MavlinkMissionError> {
    let mut observer = NoopMavlinkMissionObserver;
    upload_mission_with_connection_observed(conn, waypoints, options, &mut observer)
}

#[cfg(feature = "mavlink-transport")]
fn upload_mission_with_connection_observed<C, O>(
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
fn upload_and_execute_mission_with_connection<C: MavlinkVehicleConnection>(
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
fn upload_and_execute_mission_with_connection_observed<C, O>(
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
fn poll_telemetry_event_with_connection<C: MavlinkVehicleConnection>(
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
fn wait_next_telemetry_event_with_connection<C: MavlinkVehicleConnection>(
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
fn execute_uploaded_mission_with_connection<C: MavlinkVehicleConnection>(
    conn: &mut C,
    options: &MissionLifecycleOptions,
) -> Result<MissionLifecycleReport, MavlinkLifecycleError> {
    let mut observer = NoopMavlinkMissionObserver;
    execute_uploaded_mission_with_connection_observed(conn, options, &mut observer)
}

#[cfg(feature = "mavlink-transport")]
fn execute_uploaded_mission_with_connection_observed<C, O>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::{AgentId, Task, TaskId, TaskStatus};

    #[test]
    fn mock_mavlink_send_poll_roundtrip() {
        let mut transport = MockMavlinkTransport::new();
        let msg = RawMessage {
            from: AgentId::from("agent-0".to_owned()),
            to: AgentId::from("sitl".to_owned()),
            payload: b"hello".to_vec(),
        };
        transport.send(msg.clone()).unwrap();
        assert_eq!(transport.sent_messages().len(), 1);
        assert_eq!(transport.sent_messages()[0].payload, b"hello");
    }

    #[test]
    fn mock_mavlink_poll_returns_pushed() {
        let mut transport = MockMavlinkTransport::new();
        let msg = RawMessage {
            from: AgentId::from("sitl".to_owned()),
            to: AgentId::from("agent-0".to_owned()),
            payload: b"ack".to_vec(),
        };
        transport.push_incoming(msg.clone());
        let polled = transport.poll().unwrap().unwrap();
        assert_eq!(polled.payload, b"ack");
    }

    #[test]
    fn mock_mavlink_poll_empty_returns_none() {
        let mut transport = MockMavlinkTransport::new();
        assert!(transport.poll().unwrap().is_none());
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn raw_mavlink_transport_send_is_explicitly_unsupported() {
        let msg = RawMessage {
            from: AgentId::from("agent-0".to_owned()),
            to: AgentId::from("sitl".to_owned()),
            payload: b"not-a-mission-upload".to_vec(),
        };

        let error = reject_raw_transport_send(msg).unwrap_err();

        assert!(matches!(error, MavlinkError::UnsupportedRawTransportSend));
        assert!(error.to_string().contains("mission upload/lifecycle APIs"));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn mavlink_connection_string_legacy_aliases_are_normalized() {
        assert_eq!(
            normalize_mavlink_connection_string("udp:127.0.0.1:14550").as_ref(),
            "udpin:127.0.0.1:14550"
        );
        assert_eq!(
            normalize_mavlink_connection_string("tcp:127.0.0.1:5760").as_ref(),
            "tcpout:127.0.0.1:5760"
        );
        assert_eq!(
            normalize_mavlink_connection_string("udpin:0.0.0.0:14550").as_ref(),
            "udpin:0.0.0.0:14550"
        );
    }

    #[test]
    fn task_to_waypoint_with_pose() {
        let task = Task {
            id: TaskId::from("t1".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(swarm_types::Pose {
                x: 10.0,
                y: 20.0,
                z: 3.0,
            }),
            grid_cell: None,
            edge_id: None,
            kind: None,
        };
        let wp = task_to_waypoint(&task).unwrap();
        assert!((wp.x - 10.0).abs() < 1e-6);
        assert!((wp.y - 20.0).abs() < 1e-6);
        assert!((wp.z - 3.0).abs() < 1e-6);
    }

    #[test]
    fn task_to_waypoint_no_pose() {
        let task = Task {
            id: TaskId::from("t1".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
            grid_cell: None,
            edge_id: None,
            kind: None,
        };
        assert!(task_to_waypoint(&task).is_none());
    }

    #[test]
    fn waypoint_status_to_task_status_completed() {
        assert_eq!(waypoint_status_to_task_status(true), TaskStatus::Completed);
    }

    #[test]
    fn waypoint_status_to_task_status_in_progress() {
        assert_eq!(
            waypoint_status_to_task_status(false),
            TaskStatus::InProgress
        );
    }
}

#[cfg(all(test, feature = "mavlink-transport"))]
mod mission_upload_tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct FakeMissionConnection {
        incoming: VecDeque<(CommonHeader, CommonMessage)>,
        sent: Vec<CommonMessage>,
    }

    impl FakeMissionConnection {
        fn with_incoming(messages: impl IntoIterator<Item = CommonMessage>) -> Self {
            Self {
                incoming: messages
                    .into_iter()
                    .map(|message| {
                        (
                            CommonHeader {
                                system_id: 1,
                                component_id: 1,
                                sequence: 0,
                            },
                            message,
                        )
                    })
                    .collect(),
                sent: Vec::new(),
            }
        }

        fn sent(&self) -> &[CommonMessage] {
            &self.sent
        }
    }

    impl MavlinkVehicleConnection for FakeMissionConnection {
        fn send_message(&mut self, msg: CommonMessage) -> Result<(), MavlinkMissionError> {
            self.sent.push(msg);
            Ok(())
        }

        fn try_recv_message(
            &mut self,
        ) -> Result<Option<(CommonHeader, CommonMessage)>, MavlinkMissionError> {
            Ok(self.incoming.pop_front())
        }
    }

    fn options() -> MissionUploadOptions {
        MissionUploadOptions {
            timeout: Duration::from_millis(1),
            ..MissionUploadOptions::default()
        }
    }

    fn waypoint(seq: u16) -> Waypoint {
        Waypoint {
            x: 10.0 + f64::from(seq),
            y: 20.0 + f64::from(seq),
            z: 30.0,
            seq,
        }
    }

    fn heartbeat() -> CommonMessage {
        CommonMessage::HEARTBEAT(common::HEARTBEAT_DATA::default())
    }

    fn request_int(seq: u16) -> CommonMessage {
        CommonMessage::MISSION_REQUEST_INT(common::MISSION_REQUEST_INT_DATA {
            seq,
            target_system: 255,
            target_component: 0,
        })
    }

    #[allow(deprecated)]
    fn request(seq: u16) -> CommonMessage {
        CommonMessage::MISSION_REQUEST(common::MISSION_REQUEST_DATA {
            seq,
            target_system: 255,
            target_component: 0,
        })
    }

    fn ack(result: common::MavMissionResult) -> CommonMessage {
        CommonMessage::MISSION_ACK(common::MISSION_ACK_DATA {
            target_system: 255,
            target_component: 0,
            mavtype: result,
        })
    }

    fn command_ack(command: common::MavCmd, result: common::MavResult) -> CommonMessage {
        CommonMessage::COMMAND_ACK(common::COMMAND_ACK_DATA { command, result })
    }

    fn lifecycle_options() -> MissionLifecycleOptions {
        MissionLifecycleOptions {
            timeout: Duration::from_millis(1),
            ..MissionLifecycleOptions::default()
        }
    }

    fn assert_command(
        message: &CommonMessage,
        command: common::MavCmd,
    ) -> &common::COMMAND_LONG_DATA {
        let CommonMessage::COMMAND_LONG(data) = message else {
            panic!("expected COMMAND_LONG");
        };
        assert_eq!(data.command, command);
        data
    }

    fn command_long_count(conn: &FakeMissionConnection) -> usize {
        conn.sent()
            .iter()
            .filter(|message| matches!(message, CommonMessage::COMMAND_LONG(_)))
            .count()
    }

    #[derive(Default)]
    struct RecordingObserver {
        events: Vec<MavlinkMissionEvent>,
    }

    impl MavlinkMissionObserver for RecordingObserver {
        fn on_event(&mut self, event: MavlinkMissionEvent) {
            self.events.push(event);
        }
    }

    fn mission_current(seq: u16) -> CommonMessage {
        CommonMessage::MISSION_CURRENT(common::MISSION_CURRENT_DATA { seq })
    }

    fn waypoint_reached(seq: u16) -> CommonMessage {
        CommonMessage::MISSION_ITEM_REACHED(common::MISSION_ITEM_REACHED_DATA { seq })
    }

    fn unrelated_message() -> CommonMessage {
        CommonMessage::RAW_RPM(common::RAW_RPM_DATA::default())
    }

    #[test]
    fn telemetry_parser_maps_heartbeat() {
        assert_eq!(
            mavlink_message_to_telemetry_event(&heartbeat()),
            Some(MavlinkTelemetryEvent::Heartbeat)
        );
    }

    #[test]
    fn telemetry_parser_maps_mission_current() {
        assert_eq!(
            mavlink_message_to_telemetry_event(&mission_current(7)),
            Some(MavlinkTelemetryEvent::MissionCurrent { seq: 7 })
        );
    }

    #[test]
    fn telemetry_parser_maps_waypoint_reached() {
        assert_eq!(
            mavlink_message_to_telemetry_event(&waypoint_reached(3)),
            Some(MavlinkTelemetryEvent::WaypointReached { seq: 3 })
        );
    }

    #[test]
    fn telemetry_parser_maps_mission_ack_results() {
        assert_eq!(
            mavlink_message_to_telemetry_event(&ack(
                common::MavMissionResult::MAV_MISSION_ACCEPTED
            )),
            Some(MavlinkTelemetryEvent::MissionComplete)
        );
        assert_eq!(
            mavlink_message_to_telemetry_event(&ack(
                common::MavMissionResult::MAV_MISSION_INVALID_SEQUENCE
            )),
            Some(MavlinkTelemetryEvent::MissionRejected {
                reason: "MAV_MISSION_INVALID_SEQUENCE".to_owned(),
            })
        );
    }

    #[test]
    fn telemetry_poll_ignores_unrelated_messages() {
        let mut conn =
            FakeMissionConnection::with_incoming([unrelated_message(), mission_current(2)]);

        let event = poll_telemetry_event_with_connection(&mut conn).unwrap();

        assert_eq!(
            event,
            Some(MavlinkTelemetryEvent::MissionCurrent { seq: 2 })
        );
    }

    #[test]
    fn telemetry_poll_returns_none_without_event() {
        let mut conn = FakeMissionConnection::with_incoming([unrelated_message()]);

        let event = poll_telemetry_event_with_connection(&mut conn).unwrap();

        assert_eq!(event, None);
    }

    #[test]
    fn telemetry_wait_times_out_without_event() {
        let mut conn = FakeMissionConnection::default();

        let error = wait_next_telemetry_event_with_connection(&mut conn, Duration::from_millis(1))
            .unwrap_err();

        assert_eq!(
            error,
            MavlinkTelemetryError::Timeout {
                timeout: Duration::from_millis(1),
            }
        );
    }

    #[test]
    fn mission_upload_happy_path_uses_request_int() {
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            request_int(0),
            request_int(1),
            ack(common::MavMissionResult::MAV_MISSION_ACCEPTED),
        ]);

        let report =
            upload_mission_with_connection(&mut conn, &[waypoint(0), waypoint(1)], &options())
                .unwrap();

        assert_eq!(report.uploaded_count, 2);
        assert_eq!(conn.sent().len(), 4);
        assert!(matches!(
            &conn.sent()[0],
            CommonMessage::MISSION_CLEAR_ALL(_)
        ));
        assert!(matches!(
            &conn.sent()[1],
            CommonMessage::MISSION_COUNT(count) if count.count == 2
        ));
        assert!(matches!(
            &conn.sent()[2],
            CommonMessage::MISSION_ITEM_INT(item) if item.seq == 0
        ));
        assert!(matches!(
            &conn.sent()[3],
            CommonMessage::MISSION_ITEM_INT(item) if item.seq == 1
        ));
    }

    #[test]
    fn mission_upload_observer_records_handshake_events() {
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            request_int(0),
            request_int(1),
            ack(common::MavMissionResult::MAV_MISSION_ACCEPTED),
        ]);
        let mut observer = RecordingObserver::default();

        upload_mission_with_connection_observed(
            &mut conn,
            &[waypoint(0), waypoint(1)],
            &options(),
            &mut observer,
        )
        .unwrap();

        assert_eq!(
            observer.events,
            vec![
                MavlinkMissionEvent::HeartbeatSeen,
                MavlinkMissionEvent::MissionClearSent,
                MavlinkMissionEvent::MissionCountSent { count: 2 },
                MavlinkMissionEvent::MissionItemRequested { seq: 0 },
                MavlinkMissionEvent::MissionItemSent { seq: 0 },
                MavlinkMissionEvent::MissionItemRequested { seq: 1 },
                MavlinkMissionEvent::MissionItemSent { seq: 1 },
                MavlinkMissionEvent::MissionAckReceived {
                    result: "MAV_MISSION_ACCEPTED".to_owned(),
                    accepted: true,
                },
            ]
        );
    }

    #[test]
    fn mission_upload_accepts_legacy_request_fallback() {
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            request(0),
            ack(common::MavMissionResult::MAV_MISSION_ACCEPTED),
        ]);

        upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap();

        assert!(matches!(
            &conn.sent()[1],
            CommonMessage::MISSION_COUNT(count) if count.count == 1
        ));
        assert!(matches!(
            &conn.sent()[2],
            CommonMessage::MISSION_ITEM_INT(item) if item.seq == 0
        ));
    }

    #[test]
    fn mission_upload_rejects_wrong_request_sequence() {
        let mut conn = FakeMissionConnection::with_incoming([heartbeat(), request_int(7)]);

        let error =
            upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap_err();

        assert_eq!(
            error,
            MavlinkMissionError::UnexpectedRequestSeq {
                expected: 0,
                actual: 7,
            }
        );
    }

    #[test]
    fn mission_upload_reports_rejected_ack() {
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            request_int(0),
            ack(common::MavMissionResult::MAV_MISSION_INVALID_SEQUENCE),
        ]);

        let error =
            upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap_err();

        assert_eq!(
            error,
            MavlinkMissionError::MissionRejected(
                common::MavMissionResult::MAV_MISSION_INVALID_SEQUENCE
            )
        );
    }

    #[test]
    fn mission_upload_times_out_without_heartbeat() {
        let mut conn = FakeMissionConnection::default();

        let error =
            upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap_err();

        assert_eq!(error, MavlinkMissionError::HeartbeatTimeout);
    }

    #[test]
    fn mission_upload_times_out_without_request() {
        let mut conn = FakeMissionConnection::with_incoming([heartbeat()]);

        let error =
            upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap_err();

        assert_eq!(
            error,
            MavlinkMissionError::MissionRequestTimeout { expected_seq: 0 }
        );
    }

    #[test]
    fn mission_upload_times_out_without_final_ack() {
        let mut conn = FakeMissionConnection::with_incoming([heartbeat(), request_int(0)]);

        let error =
            upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap_err();

        assert_eq!(error, MavlinkMissionError::MissionAckTimeout);
    }

    #[test]
    fn mission_upload_can_skip_clear_existing() {
        let options = MissionUploadOptions {
            clear_existing: false,
            ..options()
        };
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            request_int(0),
            ack(common::MavMissionResult::MAV_MISSION_ACCEPTED),
        ]);

        let report = upload_mission_with_connection(&mut conn, &[waypoint(0)], &options).unwrap();

        assert!(!report.cleared_existing);
        assert!(matches!(
            &conn.sent()[0],
            CommonMessage::MISSION_COUNT(count) if count.count == 1
        ));
        assert!(matches!(
            &conn.sent()[1],
            CommonMessage::MISSION_ITEM_INT(item) if item.seq == 0
        ));
    }

    #[test]
    fn mission_upload_rejects_empty_mission() {
        let mut conn = FakeMissionConnection::default();

        let error = upload_mission_with_connection(&mut conn, &[], &options()).unwrap_err();

        assert_eq!(error, MavlinkMissionError::EmptyMission);
    }

    #[test]
    #[allow(deprecated)]
    fn waypoint_conversion_uses_home_origin_and_relative_altitude() {
        let options = MissionUploadOptions {
            home_origin: MissionHomeOrigin {
                lat_deg: 47.0,
                lon_deg: 8.0,
                alt_m: 5.0,
            },
            ..options()
        };
        let message = waypoint_to_mission_item_int(
            &Waypoint {
                x: 100.0,
                y: 111.32,
                z: 25.0,
                seq: 3,
            },
            &options,
        )
        .unwrap();

        let CommonMessage::MISSION_ITEM_INT(item) = message else {
            panic!("expected MISSION_ITEM_INT");
        };

        assert_eq!(item.seq, 3);
        assert_eq!(item.target_system, 1);
        assert_eq!(item.target_component, 1);
        assert_eq!(
            item.frame,
            common::MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT
        );
        assert!((f64::from(item.x) / 10_000_000.0 - 47.001).abs() < 0.000_001);
        assert!(f64::from(item.y) / 10_000_000.0 > 8.001);
        assert!((item.z - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn waypoint_conversion_rejects_unsupported_frame() {
        let options = MissionUploadOptions {
            frame: MissionFrame::LocalNed,
            ..options()
        };

        let error = waypoint_to_mission_item_int(&waypoint(0), &options).unwrap_err();

        assert_eq!(error, MavlinkMissionError::UnsupportedFrame);
    }

    #[test]
    fn command_helpers_build_expected_command_long_messages() {
        let arm = arm_command(1, 2);
        let arm = assert_command(&arm, common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM);
        assert_eq!(arm.target_system, 1);
        assert_eq!(arm.target_component, 2);
        assert_eq!(arm.param1, 1.0);

        let disarm = disarm_command(1, 2);
        let disarm = assert_command(&disarm, common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM);
        assert_eq!(disarm.param1, 0.0);

        let takeoff = takeoff_command(1, 2, 12.5);
        let takeoff = assert_command(&takeoff, common::MavCmd::MAV_CMD_NAV_TAKEOFF);
        assert!((takeoff.param7 - 12.5).abs() < f32::EPSILON);

        let start = start_mission_command(1, 2);
        assert_command(&start, common::MavCmd::MAV_CMD_MISSION_START);

        let abort = abort_command(1, 2);
        assert_command(&abort, common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH);
    }

    #[test]
    fn wait_command_ack_accepts_matching_ack_and_ignores_unrelated_messages() {
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
        ]);

        wait_command_ack(
            &mut conn,
            common::MavCmd::MAV_CMD_NAV_TAKEOFF,
            Duration::from_millis(1),
        )
        .unwrap();
    }

    #[test]
    fn wait_command_ack_reports_rejected_result() {
        let mut conn = FakeMissionConnection::with_incoming([command_ack(
            common::MavCmd::MAV_CMD_NAV_TAKEOFF,
            common::MavResult::MAV_RESULT_DENIED,
        )]);

        let error = wait_command_ack(
            &mut conn,
            common::MavCmd::MAV_CMD_NAV_TAKEOFF,
            Duration::from_millis(1),
        )
        .unwrap_err();

        assert_eq!(
            error,
            MavlinkLifecycleError::CommandRejected {
                command: common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                result: common::MavResult::MAV_RESULT_DENIED,
                abort_result: None,
            }
        );
    }

    #[test]
    fn wait_command_ack_times_out_without_matching_ack() {
        let mut conn = FakeMissionConnection::default();

        let error = wait_command_ack(
            &mut conn,
            common::MavCmd::MAV_CMD_NAV_TAKEOFF,
            Duration::from_millis(1),
        )
        .unwrap_err();

        assert_eq!(
            error,
            MavlinkLifecycleError::CommandAckTimeout {
                command: common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                abort_result: None,
            }
        );
    }

    #[test]
    fn lifecycle_happy_path_sends_arm_takeoff_start() {
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_MISSION_START,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            heartbeat(),
        ]);

        let report =
            execute_uploaded_mission_with_connection(&mut conn, &lifecycle_options()).unwrap();

        assert!(report.armed);
        assert!(report.took_off);
        assert!(report.started);
        assert!(report.post_start_heartbeat);
        assert_eq!(command_long_count(&conn), 3);
        assert_command(
            &conn.sent()[0],
            common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
        );
        assert_command(&conn.sent()[1], common::MavCmd::MAV_CMD_NAV_TAKEOFF);
        assert_command(&conn.sent()[2], common::MavCmd::MAV_CMD_MISSION_START);
    }

    #[test]
    fn lifecycle_no_arm_skips_arm_command() {
        let mut options = lifecycle_options();
        options.no_arm = true;
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_MISSION_START,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            heartbeat(),
        ]);

        let report = execute_uploaded_mission_with_connection(&mut conn, &options).unwrap();

        assert!(!report.armed);
        assert!(report.took_off);
        assert_eq!(command_long_count(&conn), 2);
        assert_command(&conn.sent()[0], common::MavCmd::MAV_CMD_NAV_TAKEOFF);
        assert_command(&conn.sent()[1], common::MavCmd::MAV_CMD_MISSION_START);
    }

    #[test]
    fn lifecycle_arm_failure_sends_no_takeoff_or_abort() {
        let mut conn = FakeMissionConnection::with_incoming([command_ack(
            common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            common::MavResult::MAV_RESULT_DENIED,
        )]);

        let error =
            execute_uploaded_mission_with_connection(&mut conn, &lifecycle_options()).unwrap_err();

        assert!(matches!(
            error,
            MavlinkLifecycleError::CommandRejected {
                command: common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                result: common::MavResult::MAV_RESULT_DENIED,
                abort_result: None,
            }
        ));
        assert_eq!(command_long_count(&conn), 1);
    }

    #[test]
    fn lifecycle_takeoff_failure_sends_abort() {
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_DENIED,
            ),
        ]);

        let error =
            execute_uploaded_mission_with_connection(&mut conn, &lifecycle_options()).unwrap_err();

        assert!(matches!(
            error,
            MavlinkLifecycleError::CommandRejected {
                command: common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                result: common::MavResult::MAV_RESULT_DENIED,
                abort_result: Some(AbortCommandResult::AckTimeout),
            }
        ));
        assert_eq!(command_long_count(&conn), 3);
        assert_command(
            &conn.sent()[2],
            common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
        );
    }

    #[test]
    fn lifecycle_start_failure_sends_abort() {
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_MISSION_START,
                common::MavResult::MAV_RESULT_FAILED,
            ),
        ]);

        let error =
            execute_uploaded_mission_with_connection(&mut conn, &lifecycle_options()).unwrap_err();

        assert!(matches!(
            error,
            MavlinkLifecycleError::CommandRejected {
                command: common::MavCmd::MAV_CMD_MISSION_START,
                result: common::MavResult::MAV_RESULT_FAILED,
                abort_result: Some(AbortCommandResult::AckTimeout),
            }
        ));
        assert_eq!(command_long_count(&conn), 4);
        assert_command(
            &conn.sent()[3],
            common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
        );
    }

    #[test]
    fn lifecycle_abort_after_sends_abort_after_successful_start() {
        let mut options = lifecycle_options();
        options.abort_after = Some(Duration::ZERO);
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_MISSION_START,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            heartbeat(),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
        ]);

        let report = execute_uploaded_mission_with_connection(&mut conn, &options).unwrap();

        assert_eq!(report.abort_result, Some(AbortCommandResult::Accepted));
        assert_eq!(command_long_count(&conn), 4);
        assert_command(
            &conn.sent()[3],
            common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
        );
    }

    #[test]
    fn upload_failure_in_execute_workflow_sends_no_lifecycle_commands() {
        let mut conn = FakeMissionConnection::with_incoming([heartbeat(), request_int(7)]);

        let error = upload_and_execute_mission_with_connection(
            &mut conn,
            &[waypoint(0)],
            &options(),
            &lifecycle_options(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            MavlinkFlightError::MissionUpload(MavlinkMissionError::UnexpectedRequestSeq {
                expected: 0,
                actual: 7,
            })
        ));
        assert_eq!(command_long_count(&conn), 0);
    }

    #[test]
    fn lifecycle_post_start_heartbeat_timeout_sends_abort() {
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_MISSION_START,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
        ]);

        let error =
            execute_uploaded_mission_with_connection(&mut conn, &lifecycle_options()).unwrap_err();

        assert_eq!(
            error,
            MavlinkLifecycleError::PostStartHeartbeatTimeout {
                abort_result: AbortCommandResult::AckTimeout,
            }
        );
        assert_eq!(command_long_count(&conn), 4);
        assert_command(
            &conn.sent()[3],
            common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
        );
    }
}
