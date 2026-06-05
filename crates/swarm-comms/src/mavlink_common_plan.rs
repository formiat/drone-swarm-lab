use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use swarm_mission_ir::{
    validate as validate_ir_plan, CoordinateFrame, MissionCommand, MissionCommandEntry,
    MissionCommandPlan, OrbitDirection, Position, TerminalState,
};

use crate::mavlink_capability_profile::{
    classify_mavlink_plan_compatibility, MavlinkCapabilityProfileId, MavlinkCompatibilityReport,
};
use crate::mavlink_coords::{
    local_to_mavlink_int, relative_altitude, scaled_coordinate, MavlinkCoordinateError,
    MavlinkCoordinateOrigin, MavlinkIntCoordinate,
};
use crate::mavlink_fc_contract::{validate_fc_contract, FcContract, FcContractValidationResult};
use crate::mavlink_geofence::{
    compile_fence_items, fence_artifact, FenceCompilerError, MavlinkFenceArtifact, MavlinkFencePlan,
};
use crate::mavlink_parameters::{FcParamRequirement, FcParamSnapshot};

/// Schema version emitted by the M81 MAVLink Common compiler.
pub const MAVLINK_COMMON_PLAN_SCHEMA_VERSION: &str = "mavlink_common_plan.v1";

const COMMAND_IR_HASH_DOMAIN: &[u8] = b"mavlink_common_plan.ir_hash.v1\0";
const MAVLINK_GLOBAL_FRAME: &str = "MAV_FRAME_GLOBAL_RELATIVE_ALT_INT";

/// Compile hardware-agnostic mission IR into a transport-free MAVLink Common plan.
pub fn compile_mavlink_common_plan(
    plan: &MissionCommandPlan,
    options: &MavlinkCommonPlanOptions,
) -> Result<MavlinkCommonPlan, MavlinkCommonCompilerError> {
    validate_ir_plan(plan).map_err(|error| MavlinkCommonCompilerError::IrValidation {
        message: error.to_string(),
    })?;
    options.validate()?;

    let mut compiler = Compiler::new(plan, options);
    for entry in &plan.commands {
        compiler.compile_entry(entry)?;
    }
    compiler.finish()
}

/// Options for the transport-free MAVLink Common compiler.
#[derive(Clone, Debug, PartialEq)]
pub struct MavlinkCommonPlanOptions {
    /// Legacy backend profile label retained for source compatibility.
    pub backend_profile: String,
    /// Selected compatibility profile used for M82 annotations and artifact labels.
    pub capability_profile: MavlinkCapabilityProfileId,
    /// WGS84 origin used when compiling local mission positions.
    pub home_origin: Option<MavlinkCoordinateOrigin>,
    /// Optional anchor used when `Hold` / `LoiterTime` has no previous waypoint.
    pub default_hold_position: Option<Position>,
    /// Orbit handling policy.
    pub orbit_strategy: MavlinkOrbitStrategy,
    /// Optional FC geofence plan compiled into `geofence_prelude`.
    pub fence_plan: Option<MavlinkFencePlan>,
    /// FC parameter requirements that should be checked when a snapshot is available.
    pub param_requirements: Vec<FcParamRequirement>,
    /// Optional dry-run/preflight parameter snapshot used for contract validation.
    pub param_snapshot: Option<FcParamSnapshot>,
}

impl Default for MavlinkCommonPlanOptions {
    fn default() -> Self {
        Self {
            backend_profile: "mavlink_common_generic".to_owned(),
            capability_profile: MavlinkCapabilityProfileId::default(),
            home_origin: None,
            default_hold_position: None,
            orbit_strategy: MavlinkOrbitStrategy::Unsupported,
            fence_plan: None,
            param_requirements: Vec::new(),
            param_snapshot: None,
        }
    }
}

impl MavlinkCommonPlanOptions {
    fn validate(&self) -> Result<(), MavlinkCommonCompilerError> {
        if let MavlinkOrbitStrategy::WaypointApproximation { segments_per_turn } =
            self.orbit_strategy
        {
            if segments_per_turn == 0 {
                return Err(MavlinkCommonCompilerError::InvalidOrbitSegments { segments_per_turn });
            }
        }
        Ok(())
    }
}

/// Orbit handling strategy for M81.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MavlinkOrbitStrategy {
    /// Record `Orbit` as unsupported.
    Unsupported,
    /// Approximate orbit with ordered `MAV_CMD_NAV_WAYPOINT` mission items.
    WaypointApproximation {
        /// Number of waypoint segments per full turn.
        segments_per_turn: u16,
    },
}

/// Transport-free MAVLink Common plan artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MavlinkCommonPlan {
    /// Schema version for this artifact.
    pub schema_version: String,
    /// Source M80 mission id.
    pub source_mission_id: String,
    /// SHA-256 digest of canonical source IR JSON.
    pub command_ir_hash: String,
    /// Backend profile label. M81 uses `mavlink_common_generic`.
    pub backend_profile: String,
    /// Commands that must be sent before mission upload/start.
    pub command_prelude: Vec<MavlinkCommonCommand>,
    /// FC geofence mission items that must be uploaded before the mission body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geofence_prelude: Option<Vec<MavlinkCommonMissionItem>>,
    /// Human-readable FC geofence summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fence_summary: Option<MavlinkFenceArtifact>,
    /// FC contract validation result for fence and parameter requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fc_contract_result: Option<FcContractValidationResult>,
    /// Ordered uploaded mission items.
    pub mission_items: Vec<MavlinkCommonMissionItem>,
    /// Optional mission start command when upload items exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mission_start: Option<MavlinkCommonCommand>,
    /// Commands that must be sent after uploaded mission execution completes.
    #[serde(default)]
    pub command_postlude: Vec<MavlinkCommonCommand>,
    /// Deterministic acknowledgement expectations.
    pub expected_acks: Vec<MavlinkExpectedAck>,
    /// Deterministic telemetry milestones expected from execution.
    pub telemetry_milestones: Vec<MavlinkTelemetryMilestone>,
    /// Features that M81 could not compile conservatively.
    pub unsupported_features: Vec<MavlinkUnsupportedFeature>,
    /// Validation summary for the compiled plan.
    pub validation_result: MavlinkPlanValidationResult,
    /// M82 capability profile compatibility report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<MavlinkCompatibilityReport>,
}

/// A MAVLink `COMMAND_LONG`-style command in transport-free form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MavlinkCommonCommand {
    /// Source IR command id.
    pub command_id: String,
    /// MAVLink Common command.
    pub command: MavlinkCommonCommandName,
    /// Compiler phase where this command is expected.
    pub phase: MavlinkPlanPhase,
    /// COMMAND_LONG param1..param7. `None` represents MAVLink NaN/unchanged semantics.
    pub params: [Option<f64>; 7],
}

/// A MAVLink mission item in transport-free `MISSION_ITEM_INT` shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MavlinkCommonMissionItem {
    /// Upload sequence number.
    pub seq: u16,
    /// Source IR command id.
    pub command_id: String,
    /// MAVLink Common navigation command.
    pub command: MavlinkCommonCommandName,
    /// MAVLink coordinate frame name.
    pub frame: String,
    /// Latitude scaled by 1e7.
    pub lat_e7: i32,
    /// Longitude scaled by 1e7.
    pub lon_e7: i32,
    /// Relative altitude in metres.
    pub relative_alt_m: f32,
    /// Mission item params 1..4.
    pub params: [Option<f64>; 4],
    /// Whether this is the first mission item.
    pub current: bool,
    /// MAVLink autocontinue flag.
    pub autocontinue: bool,
    /// Optional source task id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_task_id: Option<String>,
    /// Optional source route id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_route_id: Option<String>,
}

/// MAVLink Common command names supported by M81.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MavlinkCommonCommandName {
    /// `MAV_CMD_COMPONENT_ARM_DISARM`.
    #[serde(rename = "MAV_CMD_COMPONENT_ARM_DISARM")]
    ComponentArmDisarm,
    /// `MAV_CMD_NAV_TAKEOFF`.
    #[serde(rename = "MAV_CMD_NAV_TAKEOFF")]
    NavTakeoff,
    /// `MAV_CMD_NAV_LAND`.
    #[serde(rename = "MAV_CMD_NAV_LAND")]
    NavLand,
    /// `MAV_CMD_NAV_RETURN_TO_LAUNCH`.
    #[serde(rename = "MAV_CMD_NAV_RETURN_TO_LAUNCH")]
    NavReturnToLaunch,
    /// `MAV_CMD_NAV_WAYPOINT`.
    #[serde(rename = "MAV_CMD_NAV_WAYPOINT")]
    NavWaypoint,
    /// `MAV_CMD_NAV_LOITER_TIME`.
    #[serde(rename = "MAV_CMD_NAV_LOITER_TIME")]
    NavLoiterTime,
    /// `MAV_CMD_MISSION_START`.
    #[serde(rename = "MAV_CMD_MISSION_START")]
    MissionStart,
    /// `MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION`.
    #[serde(rename = "MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION")]
    FenceCircleInclusion,
    /// `MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION`.
    #[serde(rename = "MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION")]
    FenceCircleExclusion,
    /// `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION`.
    #[serde(rename = "MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION")]
    FencePolygonVertexInclusion,
    /// `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION`.
    #[serde(rename = "MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION")]
    FencePolygonVertexExclusion,
    /// `MAV_CMD_DO_FENCE_ENABLE`.
    #[serde(rename = "MAV_CMD_DO_FENCE_ENABLE")]
    DoFenceEnable,
}

impl MavlinkCommonCommandName {
    /// Stable MAVLink Common command string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ComponentArmDisarm => "MAV_CMD_COMPONENT_ARM_DISARM",
            Self::NavTakeoff => "MAV_CMD_NAV_TAKEOFF",
            Self::NavLand => "MAV_CMD_NAV_LAND",
            Self::NavReturnToLaunch => "MAV_CMD_NAV_RETURN_TO_LAUNCH",
            Self::NavWaypoint => "MAV_CMD_NAV_WAYPOINT",
            Self::NavLoiterTime => "MAV_CMD_NAV_LOITER_TIME",
            Self::MissionStart => "MAV_CMD_MISSION_START",
            Self::FenceCircleInclusion => "MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION",
            Self::FenceCircleExclusion => "MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION",
            Self::FencePolygonVertexInclusion => "MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION",
            Self::FencePolygonVertexExclusion => "MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION",
            Self::DoFenceEnable => "MAV_CMD_DO_FENCE_ENABLE",
        }
    }
}

/// Compiler phase for commands, acks and telemetry milestones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MavlinkPlanPhase {
    /// Pre-upload command phase.
    CommandPrelude,
    /// Mission upload phase.
    MissionUpload,
    /// Mission start phase.
    MissionStart,
    /// Post-mission command phase.
    CommandPostlude,
    /// Telemetry/monitoring phase.
    Telemetry,
}

/// Expected acknowledgement entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MavlinkExpectedAck {
    /// Ack phase.
    pub phase: MavlinkPlanPhase,
    /// Ack kind.
    pub kind: MavlinkExpectedAckKind,
    /// Source command id where applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    /// MAVLink command where applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<MavlinkCommonCommandName>,
    /// Mission item seq where applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u16>,
}

/// Ack categories expected from a MAVLink execution layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MavlinkExpectedAckKind {
    /// `COMMAND_ACK`.
    CommandAck,
    /// Final accepted `MISSION_ACK` after upload.
    MissionAck,
}

/// Expected telemetry milestone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MavlinkTelemetryMilestone {
    /// Telemetry phase.
    pub phase: MavlinkPlanPhase,
    /// Milestone kind.
    pub kind: MavlinkTelemetryMilestoneKind,
    /// Optional source command id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    /// Optional mission item seq.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u16>,
    /// Human-readable description for artifact readers.
    pub description: String,
}

/// Telemetry milestone categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MavlinkTelemetryMilestoneKind {
    /// Vehicle heartbeat is expected before command/upload execution.
    HeartbeatExpected,
    /// Uploaded mission item is expected to be reached.
    MissionItemReachedExpected,
    /// Terminal state from the source IR is expected.
    TerminalStateExpected,
}

/// Structured unsupported feature record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MavlinkUnsupportedFeature {
    /// Stable rule id.
    pub rule_id: String,
    /// Source command id.
    pub command_id: String,
    /// Source command kind.
    pub command_kind: String,
    /// Whether the feature is required for a valid execution plan.
    pub required: bool,
    /// Human-readable reason.
    pub reason: String,
}

/// Validation summary for the compiled M81 plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MavlinkPlanValidationResult {
    /// True only when IR validation passed and no required unsupported features remain.
    pub passed: bool,
    /// True when M80 IR validation passed before compilation.
    pub ir_validation_passed: bool,
    /// Number of unsupported required features.
    pub unsupported_required_count: usize,
    /// Stable validation notes.
    pub notes: Vec<String>,
}

/// Compiler failures for deterministic M81 plan generation.
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum MavlinkCommonCompilerError {
    /// Source M80 IR validation failed.
    #[error("mission IR validation failed: {message}")]
    IrValidation {
        /// Validation message.
        message: String,
    },
    /// Coordinate conversion failed.
    #[error("coordinate conversion failed: {message}")]
    Coordinate {
        /// Conversion message.
        message: String,
    },
    /// Local position cannot be converted without a home origin.
    #[error("local position requires MavlinkCommonPlanOptions.home_origin")]
    MissingHomeOrigin,
    /// Orbit approximation requires at least one segment per turn.
    #[error("orbit approximation requires segments_per_turn > 0, got {segments_per_turn}")]
    InvalidOrbitSegments {
        /// Requested segment count.
        segments_per_turn: u16,
    },
    /// Generated mission has more items than MAVLink seq can represent.
    #[error("compiled mission contains too many mission items: {count}")]
    TooManyMissionItems {
        /// Item count.
        count: usize,
    },
    /// Source IR could not be serialized for stable hashing.
    #[error("mission IR serialization failed: {message}")]
    IrSerialization {
        /// Serialization message.
        message: String,
    },
    /// FC geofence compilation failed.
    #[error("fence compilation failed: {source}")]
    FenceCompilation {
        #[from]
        source: FenceCompilerError,
    },
}

impl From<MavlinkCoordinateError> for MavlinkCommonCompilerError {
    fn from(error: MavlinkCoordinateError) -> Self {
        Self::Coordinate {
            message: error.to_string(),
        }
    }
}

struct Compiler<'a> {
    plan: &'a MissionCommandPlan,
    options: &'a MavlinkCommonPlanOptions,
    command_prelude: Vec<MavlinkCommonCommand>,
    command_postlude: Vec<MavlinkCommonCommand>,
    mission_items: Vec<MavlinkCommonMissionItem>,
    unsupported_features: Vec<MavlinkUnsupportedFeature>,
    last_anchor: Option<MavlinkIntCoordinate>,
}

impl<'a> Compiler<'a> {
    fn new(plan: &'a MissionCommandPlan, options: &'a MavlinkCommonPlanOptions) -> Self {
        Self {
            plan,
            options,
            command_prelude: Vec::new(),
            command_postlude: Vec::new(),
            mission_items: Vec::new(),
            unsupported_features: Vec::new(),
            last_anchor: None,
        }
    }

    fn compile_entry(
        &mut self,
        entry: &MissionCommandEntry,
    ) -> Result<(), MavlinkCommonCompilerError> {
        match &entry.command {
            MissionCommand::Arm => self.push_command(
                entry,
                MavlinkCommonCommandName::ComponentArmDisarm,
                [
                    Some(1.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                ],
                MavlinkPlanPhase::CommandPrelude,
            ),
            MissionCommand::Disarm => self.push_command(
                entry,
                MavlinkCommonCommandName::ComponentArmDisarm,
                [
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                ],
                MavlinkPlanPhase::CommandPrelude,
            ),
            MissionCommand::Takeoff { altitude_m } => self.push_command(
                entry,
                MavlinkCommonCommandName::NavTakeoff,
                [
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    None,
                    Some(0.0),
                    Some(0.0),
                    Some(*altitude_m),
                ],
                MavlinkPlanPhase::CommandPrelude,
            ),
            MissionCommand::Hold { duration_secs } => {
                self.compile_hold_or_unsupported(entry, *duration_secs, "hold_requires_position")
            }
            MissionCommand::Land => self.push_lifecycle_command(
                entry,
                MavlinkCommonCommandName::NavLand,
                [
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    None,
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                ],
            ),
            MissionCommand::ReturnToLaunch | MissionCommand::Abort => self.push_lifecycle_command(
                entry,
                MavlinkCommonCommandName::NavReturnToLaunch,
                [
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                    Some(0.0),
                ],
            ),
            MissionCommand::GoTo { position } => {
                let coordinate = self.convert_position(position)?;
                self.push_mission_item(
                    entry,
                    MavlinkCommonCommandName::NavWaypoint,
                    coordinate,
                    [Some(0.0), Some(0.0), Some(0.0), None],
                )
            }
            MissionCommand::FollowRoute { waypoints, .. } => {
                for waypoint in waypoints {
                    let coordinate = self.convert_position(&waypoint.position)?;
                    self.push_mission_item(
                        entry,
                        MavlinkCommonCommandName::NavWaypoint,
                        coordinate,
                        [Some(0.0), waypoint.acceptance_radius_m, Some(0.0), None],
                    )?;
                }
                Ok(())
            }
            MissionCommand::LoiterTime { duration_secs } => self.compile_hold_or_unsupported(
                entry,
                *duration_secs,
                "loiter_time_requires_position",
            ),
            MissionCommand::Orbit {
                center,
                radius_m,
                turns,
                direction,
            } => self.compile_orbit(entry, center, *radius_m, *turns, *direction),
            MissionCommand::Pause => {
                self.push_unsupported(
                    entry,
                    "pause_unsupported",
                    "Pause has no conservative MAVLink Common mapping in M81",
                );
                Ok(())
            }
            MissionCommand::Resume => {
                self.push_unsupported(
                    entry,
                    "resume_unsupported",
                    "Resume has no conservative MAVLink Common mapping in M81",
                );
                Ok(())
            }
        }
    }

    fn compile_hold_or_unsupported(
        &mut self,
        entry: &MissionCommandEntry,
        duration_secs: f64,
        rule_id: &'static str,
    ) -> Result<(), MavlinkCommonCompilerError> {
        let coordinate = match self.last_anchor {
            Some(coordinate) => coordinate,
            None => match self.options.default_hold_position.as_ref() {
                Some(position) => self.convert_position(position)?,
                None => {
                    self.push_unsupported(
                        entry,
                        rule_id,
                        "Hold/loiter duration requires a previous waypoint or default_hold_position",
                    );
                    return Ok(());
                }
            },
        };
        self.push_mission_item(
            entry,
            MavlinkCommonCommandName::NavLoiterTime,
            coordinate,
            [Some(duration_secs), Some(0.0), Some(0.0), Some(0.0)],
        )
    }

    fn compile_orbit(
        &mut self,
        entry: &MissionCommandEntry,
        center: &Position,
        radius_m: f64,
        turns: f64,
        direction: OrbitDirection,
    ) -> Result<(), MavlinkCommonCompilerError> {
        let MavlinkOrbitStrategy::WaypointApproximation { segments_per_turn } =
            self.options.orbit_strategy
        else {
            self.push_unsupported(
                entry,
                "orbit_unsupported",
                "Orbit requires backend-specific support or waypoint approximation",
            );
            return Ok(());
        };

        let count = (turns * f64::from(segments_per_turn)).ceil() as usize;
        if count > u16::MAX as usize {
            return Err(MavlinkCommonCompilerError::TooManyMissionItems { count });
        }
        let local_center = match center {
            Position::Local(local) => Some((local.x_m, local.y_m, local.z_m)),
            Position::Geo(_) => None,
        };
        for index in 0..count {
            let fraction = index as f64 / f64::from(segments_per_turn);
            let signed_turns = match direction {
                OrbitDirection::CounterClockwise => fraction,
                OrbitDirection::Clockwise => -fraction,
            };
            let angle = signed_turns * std::f64::consts::TAU;
            let east_offset = radius_m * angle.cos();
            let north_offset = radius_m * angle.sin();
            let coordinate = match (local_center, center) {
                (Some((x, y, z)), _) => {
                    let position = Position::Local(swarm_mission_ir::LocalPosition {
                        x_m: x + east_offset,
                        y_m: y + north_offset,
                        z_m: z,
                    });
                    self.convert_position(&position)?
                }
                (None, Position::Geo(geo)) => local_to_mavlink_int(
                    east_offset,
                    north_offset,
                    geo.alt_m,
                    MavlinkCoordinateOrigin {
                        lat_deg: geo.lat_deg,
                        lon_deg: geo.lon_deg,
                        alt_m: geo.alt_m,
                    },
                )?,
                _ => unreachable!("center pattern is exhaustive"),
            };
            self.push_mission_item(
                entry,
                MavlinkCommonCommandName::NavWaypoint,
                coordinate,
                [Some(0.0), Some(0.0), Some(0.0), None],
            )?;
        }
        Ok(())
    }

    fn push_lifecycle_command(
        &mut self,
        entry: &MissionCommandEntry,
        command: MavlinkCommonCommandName,
        params: [Option<f64>; 7],
    ) -> Result<(), MavlinkCommonCompilerError> {
        let phase = if self.mission_items.is_empty() {
            MavlinkPlanPhase::CommandPrelude
        } else {
            MavlinkPlanPhase::CommandPostlude
        };
        self.push_command(entry, command, params, phase)
    }

    fn push_command(
        &mut self,
        entry: &MissionCommandEntry,
        command: MavlinkCommonCommandName,
        params: [Option<f64>; 7],
        phase: MavlinkPlanPhase,
    ) -> Result<(), MavlinkCommonCompilerError> {
        let mavlink_command = MavlinkCommonCommand {
            command_id: command_id(entry),
            command,
            phase,
            params,
        };
        match phase {
            MavlinkPlanPhase::CommandPrelude => self.command_prelude.push(mavlink_command),
            MavlinkPlanPhase::CommandPostlude => self.command_postlude.push(mavlink_command),
            MavlinkPlanPhase::MissionUpload
            | MavlinkPlanPhase::MissionStart
            | MavlinkPlanPhase::Telemetry => {
                unreachable!("push_command only supports command phases")
            }
        }
        Ok(())
    }

    fn push_mission_item(
        &mut self,
        entry: &MissionCommandEntry,
        command: MavlinkCommonCommandName,
        coordinate: MavlinkIntCoordinate,
        params: [Option<f64>; 4],
    ) -> Result<(), MavlinkCommonCompilerError> {
        let seq = self.mission_items.len();
        if seq > u16::MAX as usize {
            return Err(MavlinkCommonCompilerError::TooManyMissionItems { count: seq + 1 });
        }
        let item = MavlinkCommonMissionItem {
            seq: seq as u16,
            command_id: command_id(entry),
            command,
            frame: MAVLINK_GLOBAL_FRAME.to_owned(),
            lat_e7: coordinate.lat_e7,
            lon_e7: coordinate.lon_e7,
            relative_alt_m: coordinate.relative_alt_m,
            params,
            current: seq == 0,
            autocontinue: true,
            source_task_id: entry.source_task_id.clone(),
            source_route_id: entry.source_route_id.clone(),
        };
        self.last_anchor = Some(coordinate);
        self.mission_items.push(item);
        Ok(())
    }

    fn push_unsupported(
        &mut self,
        entry: &MissionCommandEntry,
        rule_id: &'static str,
        reason: &'static str,
    ) {
        self.unsupported_features.push(MavlinkUnsupportedFeature {
            rule_id: rule_id.to_owned(),
            command_id: command_id(entry),
            command_kind: entry.command.kind_name().to_owned(),
            required: true,
            reason: reason.to_owned(),
        });
    }

    fn convert_position(
        &self,
        position: &Position,
    ) -> Result<MavlinkIntCoordinate, MavlinkCommonCompilerError> {
        match position {
            Position::Geo(geo) => Ok(MavlinkIntCoordinate {
                lat_e7: scaled_coordinate(geo.lat_deg, "latitude")?,
                lon_e7: scaled_coordinate(geo.lon_deg, "longitude")?,
                relative_alt_m: relative_altitude(geo.alt_m)?,
            }),
            Position::Local(local) => {
                let origin = self
                    .options
                    .home_origin
                    .ok_or(MavlinkCommonCompilerError::MissingHomeOrigin)?;
                let (east_m, north_m, relative_alt_m) = match self.plan.coordinate_frame {
                    CoordinateFrame::LocalEnu => (local.x_m, local.y_m, local.z_m),
                    CoordinateFrame::LocalNed => (local.y_m, local.x_m, -local.z_m),
                    CoordinateFrame::Wgs84 => (local.x_m, local.y_m, local.z_m),
                };
                local_to_mavlink_int(east_m, north_m, relative_alt_m, origin).map_err(Into::into)
            }
        }
    }

    fn finish(mut self) -> Result<MavlinkCommonPlan, MavlinkCommonCompilerError> {
        let mut geofence_prelude = None;
        let mut fence_summary = None;
        if let Some(fence_plan) = &self.options.fence_plan {
            let profile = self.options.capability_profile.profile();
            let (fence_items, enable_command) = compile_fence_items(fence_plan, profile)?;
            if let Some(command) = enable_command {
                self.command_prelude.insert(0, command);
            }
            geofence_prelude = Some(fence_items);
            fence_summary = Some(fence_artifact(fence_plan, profile));
        }
        let fc_contract_result =
            if self.options.fence_plan.is_some() || !self.options.param_requirements.is_empty() {
                let contract = FcContract {
                    profile: self.options.capability_profile,
                    fence_plan: self.options.fence_plan.clone(),
                    param_requirements: self.options.param_requirements.clone(),
                };
                Some(validate_fc_contract(
                    &contract,
                    self.options.param_snapshot.as_ref(),
                ))
            } else {
                None
            };
        let mission_start = (!self.mission_items.is_empty()).then_some(MavlinkCommonCommand {
            command_id: "mission-start-0".to_owned(),
            command: MavlinkCommonCommandName::MissionStart,
            phase: MavlinkPlanPhase::MissionStart,
            params: [
                Some(0.0),
                Some(0.0),
                Some(0.0),
                Some(0.0),
                Some(0.0),
                Some(0.0),
                Some(0.0),
            ],
        });
        let expected_acks = build_expected_acks(
            &self.command_prelude,
            &self.mission_items,
            mission_start.as_ref(),
            &self.command_postlude,
        );
        let telemetry_milestones =
            build_telemetry_milestones(self.plan.expected_terminal_state, &self.mission_items);
        let unsupported_required_count = self
            .unsupported_features
            .iter()
            .filter(|feature| feature.required)
            .count();
        let mut notes = vec![
            "M81 compiler output is transport-free; no hardware upload is implied".to_owned(),
            "PX4/ArduPilot semantics are not identical even for MAVLink Common commands".to_owned(),
        ];
        if unsupported_required_count > 0 {
            notes.push(format!(
                "{unsupported_required_count} required feature(s) are unsupported by M81"
            ));
        }
        let mut compiled = MavlinkCommonPlan {
            schema_version: MAVLINK_COMMON_PLAN_SCHEMA_VERSION.to_owned(),
            source_mission_id: self.plan.mission_id.as_ref().clone(),
            command_ir_hash: command_ir_hash(self.plan)?,
            backend_profile: self.options.capability_profile.as_str().to_owned(),
            command_prelude: self.command_prelude,
            geofence_prelude,
            fence_summary,
            fc_contract_result,
            mission_items: self.mission_items,
            mission_start,
            command_postlude: self.command_postlude,
            expected_acks,
            telemetry_milestones,
            unsupported_features: self.unsupported_features,
            validation_result: MavlinkPlanValidationResult {
                passed: unsupported_required_count == 0,
                ir_validation_passed: true,
                unsupported_required_count,
                notes,
            },
            compatibility: None,
        };
        let compatibility = classify_mavlink_plan_compatibility(
            &compiled,
            self.options.capability_profile.profile(),
        );
        let profile_unsupported_count = compatibility.unsupported_count();
        if profile_unsupported_count > 0 {
            compiled.validation_result.passed = false;
            compiled.validation_result.unsupported_required_count += profile_unsupported_count;
            compiled.validation_result.notes.push(format!(
                "{profile_unsupported_count} profile compatibility issue(s) are unsupported by {}",
                self.options.capability_profile.as_str()
            ));
        }
        if let Some(contract_result) = &compiled.fc_contract_result {
            if contract_result.blocks_mission_start {
                let violation_count = contract_result.violations.len();
                compiled.validation_result.passed = false;
                compiled.validation_result.unsupported_required_count += violation_count;
                compiled.validation_result.notes.push(format!(
                    "{violation_count} FC contract violation(s) block mission start"
                ));
            }
        }
        compiled.compatibility = Some(compatibility);
        Ok(compiled)
    }
}

fn command_id(entry: &MissionCommandEntry) -> String {
    entry.command_id.as_ref().clone()
}

fn command_ir_hash(plan: &MissionCommandPlan) -> Result<String, MavlinkCommonCompilerError> {
    let json =
        serde_json::to_vec(plan).map_err(|error| MavlinkCommonCompilerError::IrSerialization {
            message: error.to_string(),
        })?;
    let mut hasher = Sha256::new();
    hasher.update(COMMAND_IR_HASH_DOMAIN);
    hasher.update(json);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("write to String cannot fail");
    }
    Ok(hex)
}

fn build_expected_acks(
    command_prelude: &[MavlinkCommonCommand],
    mission_items: &[MavlinkCommonMissionItem],
    mission_start: Option<&MavlinkCommonCommand>,
    command_postlude: &[MavlinkCommonCommand],
) -> Vec<MavlinkExpectedAck> {
    let mut acks = Vec::new();
    for command in command_prelude {
        acks.push(MavlinkExpectedAck {
            phase: command.phase,
            kind: MavlinkExpectedAckKind::CommandAck,
            command_id: Some(command.command_id.clone()),
            command: Some(command.command),
            seq: None,
        });
    }
    if !mission_items.is_empty() {
        acks.push(MavlinkExpectedAck {
            phase: MavlinkPlanPhase::MissionUpload,
            kind: MavlinkExpectedAckKind::MissionAck,
            command_id: None,
            command: None,
            seq: mission_items.last().map(|item| item.seq),
        });
    }
    if let Some(command) = mission_start {
        acks.push(MavlinkExpectedAck {
            phase: MavlinkPlanPhase::MissionStart,
            kind: MavlinkExpectedAckKind::CommandAck,
            command_id: Some(command.command_id.clone()),
            command: Some(command.command),
            seq: None,
        });
    }
    for command in command_postlude {
        acks.push(MavlinkExpectedAck {
            phase: command.phase,
            kind: MavlinkExpectedAckKind::CommandAck,
            command_id: Some(command.command_id.clone()),
            command: Some(command.command),
            seq: None,
        });
    }
    acks
}

fn build_telemetry_milestones(
    terminal_state: TerminalState,
    mission_items: &[MavlinkCommonMissionItem],
) -> Vec<MavlinkTelemetryMilestone> {
    let mut milestones = vec![MavlinkTelemetryMilestone {
        phase: MavlinkPlanPhase::Telemetry,
        kind: MavlinkTelemetryMilestoneKind::HeartbeatExpected,
        command_id: None,
        seq: None,
        description: "MAVLink HEARTBEAT expected before command/upload execution".to_owned(),
    }];
    for item in mission_items {
        milestones.push(MavlinkTelemetryMilestone {
            phase: MavlinkPlanPhase::Telemetry,
            kind: MavlinkTelemetryMilestoneKind::MissionItemReachedExpected,
            command_id: Some(item.command_id.clone()),
            seq: Some(item.seq),
            description: format!("MISSION_ITEM_REACHED expected for seq={}", item.seq),
        });
    }
    milestones.push(MavlinkTelemetryMilestone {
        phase: MavlinkPlanPhase::Telemetry,
        kind: MavlinkTelemetryMilestoneKind::TerminalStateExpected,
        command_id: None,
        seq: None,
        description: format!("terminal state expected: {terminal_state:?}"),
    });
    milestones
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mavlink_geofence::{
        FcGeofenceItem, FcGeofenceItemKind, FcGeofenceShape, MavlinkFencePlan,
    };
    use crate::mavlink_parameters::{FcParamId, FcParamRange, FcParamRequirement};
    use swarm_mission_ir::{
        AltitudeReference, CommandId, CompletionTolerance, LocalPosition, MissionId,
        MissionWaypoint, RouteId, TimeoutAction, TimeoutPolicy,
    };

    fn origin() -> MavlinkCoordinateOrigin {
        MavlinkCoordinateOrigin {
            lat_deg: 47.397_742,
            lon_deg: 8.545_594,
            alt_m: 0.0,
        }
    }

    fn local(x_m: f64, y_m: f64, z_m: f64) -> Position {
        Position::Local(LocalPosition { x_m, y_m, z_m })
    }

    fn entry(id: &str, command: MissionCommand) -> MissionCommandEntry {
        MissionCommandEntry {
            command_id: CommandId::from(id.to_owned()),
            command,
            source_task_id: None,
            source_route_id: None,
            source_agent_id: Some("agent-0".to_owned()),
        }
    }

    fn plan(commands: Vec<MissionCommandEntry>) -> MissionCommandPlan {
        MissionCommandPlan {
            schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
            mission_id: MissionId::from("m81-test".to_owned()),
            coordinate_frame: CoordinateFrame::LocalEnu,
            altitude_reference: AltitudeReference::RelativeHome,
            timeout_policy: TimeoutPolicy {
                command_timeout_secs: 5.0,
                completion_timeout_secs: 120.0,
                on_timeout: TimeoutAction::Abort,
            },
            expected_terminal_state: TerminalState::Landed,
            completion_tolerance: CompletionTolerance {
                position_m: 1.0,
                altitude_m: 0.5,
            },
            commands,
        }
    }

    fn options() -> MavlinkCommonPlanOptions {
        MavlinkCommonPlanOptions {
            home_origin: Some(origin()),
            ..Default::default()
        }
    }

    fn polygon_fence_plan() -> MavlinkFencePlan {
        MavlinkFencePlan {
            items: vec![FcGeofenceItem {
                id: "test-fence".to_owned(),
                kind: FcGeofenceItemKind::PolygonInclusion,
                shape: FcGeofenceShape::Polygon {
                    vertices: vec![
                        (473_977_420, 85_455_940),
                        (473_977_430, 85_455_940),
                        (473_977_430, 85_455_950),
                    ],
                },
            }],
            enable_fence: true,
        }
    }

    #[test]
    fn takeoff_and_land_compile_to_common_commands() {
        let plan = plan(vec![
            entry("arm", MissionCommand::Arm),
            entry("takeoff", MissionCommand::Takeoff { altitude_m: 3.0 }),
            entry("land", MissionCommand::Land),
        ]);

        let compiled = compile_mavlink_common_plan(&plan, &options()).unwrap();

        assert_eq!(
            compiled
                .command_prelude
                .iter()
                .map(|command| command.command)
                .collect::<Vec<_>>(),
            vec![
                MavlinkCommonCommandName::ComponentArmDisarm,
                MavlinkCommonCommandName::NavTakeoff,
                MavlinkCommonCommandName::NavLand,
            ]
        );
        assert!(compiled.mission_items.is_empty());
        assert!(compiled.validation_result.passed);
    }

    #[test]
    fn fence_plan_populates_geofence_prelude_and_summary() {
        let plan = plan(vec![
            entry("arm", MissionCommand::Arm),
            entry("takeoff", MissionCommand::Takeoff { altitude_m: 3.0 }),
        ]);
        let mut options = options();
        options.fence_plan = Some(polygon_fence_plan());
        options.param_requirements = vec![FcParamRequirement {
            param_id: FcParamId::from("GF_ACTION".to_owned()),
            required_range: FcParamRange::IntBounds { min: 0, max: 5 },
            reason: "PX4 geofence action must be known before execution".to_owned(),
        }];

        let compiled = compile_mavlink_common_plan(&plan, &options).unwrap();

        assert_eq!(compiled.geofence_prelude.as_ref().unwrap().len(), 3);
        assert_eq!(
            compiled.geofence_prelude.as_ref().unwrap()[0].command,
            MavlinkCommonCommandName::FencePolygonVertexInclusion
        );
        assert_eq!(compiled.fence_summary.as_ref().unwrap().item_count, 1);
        assert!(compiled.fc_contract_result.is_some());
        let fence_enable_index = compiled
            .command_prelude
            .iter()
            .position(|command| command.command == MavlinkCommonCommandName::DoFenceEnable)
            .unwrap();
        let arm_index = compiled
            .command_prelude
            .iter()
            .position(|command| command.command == MavlinkCommonCommandName::ComponentArmDisarm)
            .unwrap();
        let takeoff_index = compiled
            .command_prelude
            .iter()
            .position(|command| command.command == MavlinkCommonCommandName::NavTakeoff)
            .unwrap();
        assert_eq!(fence_enable_index, 0);
        assert!(fence_enable_index < arm_index);
        assert!(fence_enable_index < takeoff_index);
        let fence_ack_index = compiled
            .expected_acks
            .iter()
            .position(|ack| ack.command == Some(MavlinkCommonCommandName::DoFenceEnable))
            .unwrap();
        let arm_ack_index = compiled
            .expected_acks
            .iter()
            .position(|ack| ack.command == Some(MavlinkCommonCommandName::ComponentArmDisarm))
            .unwrap();
        let takeoff_ack_index = compiled
            .expected_acks
            .iter()
            .position(|ack| ack.command == Some(MavlinkCommonCommandName::NavTakeoff))
            .unwrap();
        assert_eq!(fence_ack_index, 0);
        assert!(fence_ack_index < arm_ack_index);
        assert!(fence_ack_index < takeoff_ack_index);
        assert!(compiled.validation_result.passed);
    }

    #[test]
    fn unsupported_fence_plan_returns_compiler_error() {
        let plan = plan(vec![entry(
            "takeoff",
            MissionCommand::Takeoff { altitude_m: 3.0 },
        )]);
        let mut options = options();
        options.capability_profile = MavlinkCapabilityProfileId::Px4;
        options.fence_plan = Some(MavlinkFencePlan {
            items: vec![FcGeofenceItem {
                id: "circle".to_owned(),
                kind: FcGeofenceItemKind::CircleInclusion,
                shape: FcGeofenceShape::Circle {
                    center_lat_e7: 473_977_420,
                    center_lon_e7: 85_455_940,
                    radius_m: 25.0,
                },
            }],
            enable_fence: true,
        });

        let error = compile_mavlink_common_plan(&plan, &options).unwrap_err();

        assert!(matches!(
            error,
            MavlinkCommonCompilerError::FenceCompilation { .. }
        ));
    }

    #[test]
    fn post_route_lifecycle_commands_compile_to_postlude() {
        let plan = plan(vec![
            entry("takeoff", MissionCommand::Takeoff { altitude_m: 3.0 }),
            entry(
                "goto",
                MissionCommand::GoTo {
                    position: local(10.0, 0.0, 3.0),
                },
            ),
            entry(
                "hold",
                MissionCommand::Hold {
                    duration_secs: 10.0,
                },
            ),
            entry("land", MissionCommand::Land),
        ]);

        let compiled = compile_mavlink_common_plan(&plan, &options()).unwrap();

        assert!(!compiled
            .command_prelude
            .iter()
            .any(|command| command.command == MavlinkCommonCommandName::NavLand));
        assert_eq!(
            compiled
                .command_postlude
                .iter()
                .map(|command| (command.command, command.phase))
                .collect::<Vec<_>>(),
            vec![(
                MavlinkCommonCommandName::NavLand,
                MavlinkPlanPhase::CommandPostlude
            )]
        );
        assert_eq!(
            compiled
                .mission_items
                .iter()
                .map(|item| item.command)
                .collect::<Vec<_>>(),
            vec![
                MavlinkCommonCommandName::NavWaypoint,
                MavlinkCommonCommandName::NavLoiterTime,
            ]
        );
        assert!(compiled.mission_start.is_some());

        let mission_start_ack_index = compiled
            .expected_acks
            .iter()
            .position(|ack| ack.command == Some(MavlinkCommonCommandName::MissionStart))
            .unwrap();
        let land_ack_index = compiled
            .expected_acks
            .iter()
            .position(|ack| ack.command_id.as_deref() == Some("land"))
            .unwrap();
        assert!(land_ack_index > mission_start_ack_index);
        assert_eq!(
            compiled.expected_acks[land_ack_index].phase,
            MavlinkPlanPhase::CommandPostlude
        );
    }

    #[test]
    fn goto_and_follow_route_compile_to_contiguous_waypoints() {
        let plan = plan(vec![
            entry(
                "goto",
                MissionCommand::GoTo {
                    position: local(10.0, 0.0, 5.0),
                },
            ),
            entry(
                "route",
                MissionCommand::FollowRoute {
                    route_id: RouteId::from("route-a".to_owned()),
                    waypoints: vec![
                        MissionWaypoint {
                            position: local(20.0, 0.0, 5.0),
                            acceptance_radius_m: None,
                        },
                        MissionWaypoint {
                            position: local(30.0, 10.0, 5.0),
                            acceptance_radius_m: Some(2.0),
                        },
                    ],
                },
            ),
        ]);

        let compiled = compile_mavlink_common_plan(&plan, &options()).unwrap();

        assert_eq!(compiled.mission_items.len(), 3);
        assert_eq!(
            compiled
                .mission_items
                .iter()
                .map(|item| item.seq)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(compiled
            .mission_items
            .iter()
            .all(|item| item.command == MavlinkCommonCommandName::NavWaypoint));
        assert!(compiled.mission_start.is_some());
        assert!(compiled
            .expected_acks
            .iter()
            .any(|ack| { ack.kind == MavlinkExpectedAckKind::MissionAck && ack.seq == Some(2) }));
    }

    #[test]
    fn local_ned_positions_swap_axes_and_invert_down_altitude() {
        let mut plan = plan(vec![entry(
            "goto",
            MissionCommand::GoTo {
                position: local(20.0, 10.0, -5.0),
            },
        )]);
        plan.coordinate_frame = CoordinateFrame::LocalNed;

        let compiled = compile_mavlink_common_plan(&plan, &options()).unwrap();
        let expected =
            crate::mavlink_coords::local_to_mavlink_int(10.0, 20.0, 5.0, origin()).unwrap();

        assert_eq!(compiled.mission_items[0].lat_e7, expected.lat_e7);
        assert_eq!(compiled.mission_items[0].lon_e7, expected.lon_e7);
        assert_eq!(compiled.mission_items[0].relative_alt_m, 5.0);
    }

    #[test]
    fn anchored_hold_compiles_to_loiter_time() {
        let plan = plan(vec![
            entry(
                "goto",
                MissionCommand::GoTo {
                    position: local(10.0, 0.0, 5.0),
                },
            ),
            entry(
                "hold",
                MissionCommand::Hold {
                    duration_secs: 10.0,
                },
            ),
        ]);

        let compiled = compile_mavlink_common_plan(&plan, &options()).unwrap();

        assert_eq!(compiled.mission_items.len(), 2);
        assert_eq!(
            compiled.mission_items[1].command,
            MavlinkCommonCommandName::NavLoiterTime
        );
        assert_eq!(compiled.mission_items[1].params[0], Some(10.0));
        assert!(compiled.validation_result.passed);
    }

    #[test]
    fn unanchored_hold_records_required_unsupported_feature() {
        let plan = plan(vec![entry(
            "hold",
            MissionCommand::Hold {
                duration_secs: 10.0,
            },
        )]);

        let compiled = compile_mavlink_common_plan(&plan, &options()).unwrap();

        assert_eq!(compiled.mission_items.len(), 0);
        assert_eq!(
            compiled.unsupported_features[0].rule_id,
            "hold_requires_position"
        );
        assert!(!compiled.validation_result.passed);
        assert_eq!(compiled.validation_result.unsupported_required_count, 1);
    }

    #[test]
    fn default_hold_position_anchors_takeoff_hold_land_sequence() {
        let plan = plan(vec![
            entry("takeoff", MissionCommand::Takeoff { altitude_m: 3.0 }),
            entry(
                "hold",
                MissionCommand::Hold {
                    duration_secs: 10.0,
                },
            ),
            entry("land", MissionCommand::Land),
        ]);
        let mut options = options();
        options.default_hold_position = Some(local(0.0, 0.0, 3.0));

        let compiled = compile_mavlink_common_plan(&plan, &options).unwrap();

        assert_eq!(compiled.mission_items.len(), 1);
        assert_eq!(
            compiled.mission_items[0].command,
            MavlinkCommonCommandName::NavLoiterTime
        );
        assert!(compiled.validation_result.passed);
    }

    #[test]
    fn pause_and_resume_are_structured_unsupported_features() {
        let plan = plan(vec![
            entry("pause", MissionCommand::Pause),
            entry("resume", MissionCommand::Resume),
        ]);

        let compiled = compile_mavlink_common_plan(&plan, &options()).unwrap();

        assert_eq!(
            compiled
                .unsupported_features
                .iter()
                .map(|feature| feature.rule_id.as_str())
                .collect::<Vec<_>>(),
            vec!["pause_unsupported", "resume_unsupported"]
        );
        assert!(!compiled.validation_result.passed);
    }

    #[test]
    fn orbit_fallback_produces_stable_waypoint_order() {
        let plan = plan(vec![entry(
            "orbit",
            MissionCommand::Orbit {
                center: local(0.0, 0.0, 5.0),
                radius_m: 10.0,
                turns: 1.0,
                direction: OrbitDirection::CounterClockwise,
            },
        )]);
        let mut options = options();
        options.orbit_strategy = MavlinkOrbitStrategy::WaypointApproximation {
            segments_per_turn: 4,
        };

        let compiled = compile_mavlink_common_plan(&plan, &options).unwrap();

        assert_eq!(compiled.mission_items.len(), 4);
        assert_eq!(
            compiled
                .mission_items
                .iter()
                .map(|item| item.seq)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        assert!(compiled.validation_result.passed);
    }

    #[test]
    fn orbit_unsupported_is_recorded_when_fallback_disabled() {
        let plan = plan(vec![entry(
            "orbit",
            MissionCommand::Orbit {
                center: local(0.0, 0.0, 5.0),
                radius_m: 10.0,
                turns: 1.0,
                direction: OrbitDirection::CounterClockwise,
            },
        )]);

        let compiled = compile_mavlink_common_plan(&plan, &options()).unwrap();

        assert_eq!(
            compiled.unsupported_features[0].rule_id,
            "orbit_unsupported"
        );
        assert!(!compiled.validation_result.passed);
    }

    #[test]
    fn zero_orbit_segments_is_rejected() {
        let plan = plan(vec![]);
        let mut options = options();
        options.orbit_strategy = MavlinkOrbitStrategy::WaypointApproximation {
            segments_per_turn: 0,
        };

        let error = compile_mavlink_common_plan(&plan, &options).unwrap_err();

        assert!(matches!(
            error,
            MavlinkCommonCompilerError::InvalidOrbitSegments { .. }
        ));
    }

    #[test]
    fn command_ir_hash_is_stable_golden_sha256() {
        let plan = plan(vec![
            entry("arm", MissionCommand::Arm),
            entry("takeoff", MissionCommand::Takeoff { altitude_m: 3.0 }),
            entry("land", MissionCommand::Land),
        ]);

        let compiled = compile_mavlink_common_plan(&plan, &options()).unwrap();

        assert_eq!(
            compiled.command_ir_hash,
            "0387b1f88a22d137b8a972c7ed4af869473ea275f90f9553f410bb13768ac86e"
        );
    }
}
