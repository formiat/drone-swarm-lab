pub mod connectivity;
pub mod mavlink;
pub mod mavlink_capability_profile;
pub mod mavlink_common_plan;
pub mod mavlink_coords;
pub mod network;
pub mod transport;

pub use connectivity::{ConnectivityModel, ConnectivitySnapshot};
pub use mavlink::{
    task_to_waypoint, waypoint_status_to_task_status, MavlinkError, MockMavlinkTransport, Waypoint,
};
pub use mavlink_capability_profile::{
    classify_mavlink_plan_compatibility, compatibility_matrix_rows, MavlinkCapabilityProfile,
    MavlinkCapabilityProfileId, MavlinkCapabilityProfileParseError, MavlinkCommandCapabilityRule,
    MavlinkCommandCompatibility, MavlinkCompatibilityClass, MavlinkCompatibilityMatrixRow,
    MavlinkCompatibilityReport, MavlinkExecutionMode, MavlinkModeRequirement,
    MavlinkModeTransitionRule, MavlinkRequiredModeTransition,
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
pub use network::{InMemAgentTransport, InMemNetwork, NetworkConfig};
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
