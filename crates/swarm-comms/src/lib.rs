pub mod connectivity;
pub mod drone_link;
pub mod mavlink;
pub mod mavlink_capability_profile;
pub mod mavlink_common_plan;
pub mod mavlink_coords;
pub mod mavlink_executor;
pub mod mavlink_fc_contract;
pub mod mavlink_geofence;
pub mod mavlink_parameters;
pub mod network;
pub mod swarm_protocol;
pub mod transport;

pub use connectivity::{ConnectivityModel, ConnectivitySnapshot};
pub use drone_link::{
    DroneLinkConfig, InternetLikeMock, InternetLikeMockProfile, NullDroneLink, SerialDroneLink,
    SerialDroneLinkError, UdpDroneLink, UdpDroneLinkError, UDP_MAX_PAYLOAD,
};
pub use mavlink::{
    task_to_waypoint, waypoint_status_to_task_status, MavlinkError, MockMavlinkTransport, Waypoint,
};
pub use mavlink_capability_profile::{
    classify_mavlink_plan_compatibility, compatibility_matrix_rows, fence_item_support_rule,
    FenceItemSupportRule, MavlinkCapabilityProfile, MavlinkCapabilityProfileId,
    MavlinkCapabilityProfileParseError, MavlinkCommandCapabilityRule, MavlinkCommandCompatibility,
    MavlinkCompatibilityClass, MavlinkCompatibilityMatrixRow, MavlinkCompatibilityReport,
    MavlinkExecutionMode, MavlinkModeRequirement, MavlinkModeTransitionRule,
    MavlinkRequiredModeTransition,
};
pub use mavlink_common_plan::{
    compile_mavlink_common_plan, MavlinkCommonCommand, MavlinkCommonCommandName,
    MavlinkCommonCompilerError, MavlinkCommonMissionItem, MavlinkCommonPlan,
    MavlinkCommonPlanOptions, MavlinkExpectedAck, MavlinkExpectedAckKind, MavlinkOrbitStrategy,
    MavlinkPlanPhase, MavlinkPlanValidationResult, MavlinkTelemetryMilestone,
    MavlinkTelemetryMilestoneKind, MavlinkUnsupportedFeature, MAVLINK_COMMON_PLAN_SCHEMA_VERSION,
};
pub use mavlink_coords::{
    local_to_mavlink_int, relative_altitude, scaled_coordinate, MavlinkCoordinateError,
    MavlinkCoordinateOrigin, MavlinkIntCoordinate,
};
pub use mavlink_executor::{
    execute_geofence_upload, execute_param_snapshot, execute_param_write, AckProvider,
    FcConfigError, FcConfigProvider, FcParamWriteOk, FcParamWriteResult, GeofenceUploadOk,
    GeofenceUploadResult, MavlinkExecutionOutcome, MavlinkExecutionStepResult,
    MavlinkPlanExecutionReport, MavlinkPlanExecutor, MissionExecuteLifecycleState, MockAckProvider,
    MockFcConfigProvider, ScriptedAckProvider,
};
pub use mavlink_fc_contract::{
    validate_fc_contract, FcContract, FcContractValidationResult, FcContractViolation,
};
pub use mavlink_geofence::{
    compile_fence_items, fence_artifact, FcGeofenceItem, FcGeofenceItemKind, FcGeofenceShape,
    FenceCompilerError, MavlinkFenceArtifact, MavlinkFencePlan,
};
pub use mavlink_parameters::{
    check_param_requirement, read_plan_from_requirements, validate_param_requirements,
    FcKnownParam, FcParamId, FcParamRange, FcParamReadPlan, FcParamRequirement, FcParamSnapshot,
    FcParamValidationResult, FcParamValue, FcParamViolation, FcParamWritePlan,
    FC_KNOWN_PARAMS_ARDUPILOT, FC_KNOWN_PARAMS_PX4,
};
pub use network::{InMemAgentTransport, InMemNetwork, NetworkConfig};
pub use swarm_protocol::{
    AbortAction, AgentMissionState, CommandPosition, DegradedReason, DuplicateSuppressor, Lease,
    LeaseId, MissionRejectReason, ProtocolRole, ReleaseReason, ReplanReason, SegmentDenyReason,
    SwarmMessage, SwarmMessageEnvelope, SWARM_PROTOCOL_SCHEMA_VERSION,
};
pub use transport::{RawMessage, Transport};

#[cfg(feature = "mavlink-transport")]
pub use mavlink::{
    abort_command, arm_command, disarm_command, mavlink_message_to_telemetry_event,
    mavlink_status_to_task_status, mission_item_to_int, start_mission_command, takeoff_command,
    task_to_mavlink_waypoint, waypoint_to_mission_item_int, AbortCommandResult, MavlinkFlightError,
    MavlinkFlightReport, MavlinkLifecycleError, MavlinkMissionError, MavlinkMissionEvent,
    MavlinkMissionObserver, MavlinkTelemetryError, MavlinkTelemetryEvent, MavlinkTransport,
    MissionFrame, MissionHomeOrigin, MissionItem, MissionLifecycleOptions, MissionLifecycleReport,
    MissionUploadOptions, MissionUploadReport,
};
