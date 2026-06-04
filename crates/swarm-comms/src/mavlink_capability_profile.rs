use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::mavlink_common_plan::{
    MavlinkCommonCommand, MavlinkCommonCommandName, MavlinkCommonMissionItem, MavlinkCommonPlan,
    MavlinkPlanPhase,
};

const MAVLINK_GLOBAL_RELATIVE_ALT_INT: &str = "MAV_FRAME_GLOBAL_RELATIVE_ALT_INT";

const GENERIC_CAVEAT: &str =
    "MAVLink Common syntax support does not prove autopilot acceptance or flight safety";
const PX4_SIH_CAVEAT: &str =
    "PX4 support is based on local SIH evidence and still requires operator validation";
const ARDUPILOT_EVIDENCE_CAVEAT: &str =
    "ArduPilot acceptance is unknown until ArduPilot SITL or hardware evidence is captured";

/// Stable selector for MAVLink capability profiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MavlinkCapabilityProfileId {
    /// Syntax-level MAVLink Common profile.
    #[serde(rename = "mavlink_common_generic")]
    #[default]
    MavlinkCommonGeneric,
    /// Conservative PX4 profile.
    #[serde(rename = "px4")]
    Px4,
    /// Conservative ArduPilot profile.
    #[serde(rename = "ardupilot")]
    ArduPilot,
}

impl MavlinkCapabilityProfileId {
    /// Stable artifact/profile id.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MavlinkCommonGeneric => "mavlink_common_generic",
            Self::Px4 => "px4",
            Self::ArduPilot => "ardupilot",
        }
    }

    /// Static capability profile data.
    pub fn profile(self) -> &'static MavlinkCapabilityProfile {
        match self {
            Self::MavlinkCommonGeneric => &MAVLINK_COMMON_GENERIC_PROFILE,
            Self::Px4 => &PX4_PROFILE,
            Self::ArduPilot => &ARDUPILOT_PROFILE,
        }
    }

    /// Human-readable list for CLI errors and docs.
    pub fn supported_values() -> &'static str {
        "mavlink_common_generic|px4|ardupilot"
    }
}

impl FromStr for MavlinkCapabilityProfileId {
    type Err = MavlinkCapabilityProfileParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "mavlink_common_generic" => Ok(Self::MavlinkCommonGeneric),
            "px4" => Ok(Self::Px4),
            "ardupilot" => Ok(Self::ArduPilot),
            other => Err(MavlinkCapabilityProfileParseError {
                value: other.to_owned(),
            }),
        }
    }
}

impl std::fmt::Display for MavlinkCapabilityProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parse error for profile ids.
#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown MAVLink capability profile '{value}'")]
pub struct MavlinkCapabilityProfileParseError {
    /// User-provided unknown value.
    pub value: String,
}

/// Compatibility class for a command, frame, profile or aggregate report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MavlinkCompatibilityClass {
    /// Supported by the selected profile without profile-specific caveats.
    Supported,
    /// Supported, but with operational caveats that must remain visible.
    SupportedWithCaveats,
    /// Requires stack-specific mapping before claiming support.
    RequiresStackSpecificMapping,
    /// Supported by an explicit compiler fallback.
    SupportedViaFallback,
    /// Not supported by the selected profile.
    Unsupported,
    /// Cannot be claimed until SITL or hardware evidence is captured.
    UnknownUntilSitlOrHardware,
}

impl MavlinkCompatibilityClass {
    /// Stable artifact string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::SupportedWithCaveats => "supported_with_caveats",
            Self::RequiresStackSpecificMapping => "requires_stack_specific_mapping",
            Self::SupportedViaFallback => "supported_via_fallback",
            Self::Unsupported => "unsupported",
            Self::UnknownUntilSitlOrHardware => "unknown_until_sitl_or_hardware",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Supported => 0,
            Self::SupportedViaFallback => 1,
            Self::SupportedWithCaveats => 2,
            Self::RequiresStackSpecificMapping => 3,
            Self::UnknownUntilSitlOrHardware => 4,
            Self::Unsupported => 5,
        }
    }

    fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    /// True when this class must block hardware-facing success.
    pub fn blocks_hardware_facing_success(self) -> bool {
        matches!(
            self,
            Self::Unsupported
                | Self::UnknownUntilSitlOrHardware
                | Self::RequiresStackSpecificMapping
        )
    }
}

/// High-level execution mode assumed by a profile rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MavlinkExecutionMode {
    /// No concrete autopilot mode is proven by this profile.
    Unspecified,
    /// Command is expected before mission upload/start.
    Command,
    /// Mission execution mode.
    Mission,
    /// Guided/offboard-like mode.
    Guided,
    /// Return/landing lifecycle mode.
    Lifecycle,
}

/// Required mode transition or precondition category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MavlinkRequiredModeTransition {
    /// Vehicle must be connected and emitting heartbeat.
    Heartbeat,
    /// Vehicle must be armed.
    Armed,
    /// Takeoff acceptance must be confirmed by the autopilot.
    TakeoffAccepted,
    /// Mission upload must be accepted.
    MissionUploadAccepted,
    /// Mission mode/start must be accepted.
    MissionStartAccepted,
    /// RTL/land mode transition must be accepted.
    LifecycleAccepted,
}

/// Static capability profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MavlinkCapabilityProfile {
    /// Stable profile id.
    pub id: MavlinkCapabilityProfileId,
    /// Human-readable stack name.
    pub stack_name: &'static str,
    /// Supported coordinate frame strings.
    pub supported_frames: &'static [&'static str],
    /// Command support rules.
    pub command_rules: &'static [MavlinkCommandCapabilityRule],
    /// Mode transition rules.
    pub mode_transitions: &'static [MavlinkModeTransitionRule],
    /// Mission start semantics summary.
    pub mission_start_semantics: &'static str,
    /// Takeoff/landing constraints.
    pub takeoff_landing_constraints: &'static [&'static str],
    /// Geofence support classification.
    pub geofence_support: MavlinkCompatibilityClass,
    /// Parameter support classification.
    pub parameter_support: MavlinkCompatibilityClass,
    /// Profile-level caveats.
    pub known_caveats: &'static [&'static str],
}

/// Command support rule in a capability profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MavlinkCommandCapabilityRule {
    /// MAVLink Common command.
    pub command: MavlinkCommonCommandName,
    /// Compatibility classification for the command.
    pub classification: MavlinkCompatibilityClass,
    /// Stable explanation.
    pub reason: &'static str,
    /// Rule caveats.
    pub caveats: &'static [&'static str],
}

/// Mode transition rule associated with all commands or a specific command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MavlinkModeTransitionRule {
    /// Command this rule applies to, or all commands when absent.
    pub command: Option<MavlinkCommonCommandName>,
    /// Required execution mode.
    pub required_execution_mode: MavlinkExecutionMode,
    /// Required mode transitions.
    pub required_mode_transitions: &'static [MavlinkRequiredModeTransition],
    /// Required preconditions.
    pub preconditions: &'static [&'static str],
    /// Mode caveats.
    pub mode_caveats: &'static [&'static str],
}

/// Aggregate mode requirement copied into compatibility reports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MavlinkModeRequirement {
    /// Optional source command id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    /// MAVLink Common command.
    pub command: MavlinkCommonCommandName,
    /// Compiler phase.
    pub phase: MavlinkPlanPhase,
    /// Required execution mode.
    pub required_execution_mode: MavlinkExecutionMode,
    /// Required transitions.
    pub required_mode_transitions: Vec<MavlinkRequiredModeTransition>,
    /// Required preconditions.
    pub preconditions: Vec<String>,
    /// Mode caveats.
    pub mode_caveats: Vec<String>,
}

/// Per-command or per-mission-item compatibility result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MavlinkCommandCompatibility {
    /// Optional source command id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
    /// Optional mission item sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u16>,
    /// MAVLink Common command.
    pub command: MavlinkCommonCommandName,
    /// Compiler phase.
    pub phase: MavlinkPlanPhase,
    /// Optional mission item frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    /// Compatibility classification.
    pub classification: MavlinkCompatibilityClass,
    /// Required execution mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_execution_mode: Option<MavlinkExecutionMode>,
    /// Required transitions.
    pub required_mode_transitions: Vec<MavlinkRequiredModeTransition>,
    /// Required preconditions.
    pub preconditions: Vec<String>,
    /// Mode caveats.
    pub mode_caveats: Vec<String>,
    /// Command/profile caveats.
    pub caveats: Vec<String>,
    /// Stable explanation.
    pub reason: String,
}

/// Profile compatibility report attached to a MAVLink Common plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MavlinkCompatibilityReport {
    /// Selected capability profile.
    pub profile: MavlinkCapabilityProfileId,
    /// Aggregate classification.
    pub overall_classification: MavlinkCompatibilityClass,
    /// True only when this profile has no hardware-blocking unknown/unsupported result.
    pub hardware_facing_allowed: bool,
    /// Per-command/per-item results.
    pub command_results: Vec<MavlinkCommandCompatibility>,
    /// Aggregate mode requirements.
    pub aggregate_mode_requirements: Vec<MavlinkModeRequirement>,
    /// Deduplicated caveats.
    pub caveats: Vec<String>,
}

impl MavlinkCompatibilityReport {
    /// Number of unsupported command/item classifications.
    pub fn unsupported_count(&self) -> usize {
        self.command_results
            .iter()
            .filter(|result| result.classification == MavlinkCompatibilityClass::Unsupported)
            .count()
    }

    /// True when report contains hardware-blocking classes.
    pub fn has_hardware_blocking_classification(&self) -> bool {
        self.command_results
            .iter()
            .any(|result| result.classification.blocks_hardware_facing_success())
    }
}

/// Stable compatibility matrix row for docs synchronization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MavlinkCompatibilityMatrixRow {
    /// Stable row key used by docs tests.
    pub row_key: &'static str,
    /// Profile id.
    pub profile: MavlinkCapabilityProfileId,
    /// Capability dimension.
    pub dimension: &'static str,
    /// Dimension classification.
    pub classification: MavlinkCompatibilityClass,
    /// Short summary.
    pub summary: &'static str,
}

/// Return static compatibility matrix rows used by docs tests.
pub fn compatibility_matrix_rows() -> &'static [MavlinkCompatibilityMatrixRow] {
    COMPATIBILITY_MATRIX_ROWS
}

/// Classify a compiled MAVLink Common plan against a capability profile.
pub fn classify_mavlink_plan_compatibility(
    plan: &MavlinkCommonPlan,
    profile: &MavlinkCapabilityProfile,
) -> MavlinkCompatibilityReport {
    let mut command_results = Vec::new();
    for command in &plan.command_prelude {
        command_results.push(classify_command(command, profile));
    }
    for item in &plan.mission_items {
        command_results.push(classify_mission_item(item, profile));
    }
    if let Some(command) = &plan.mission_start {
        command_results.push(classify_command(command, profile));
    }
    for command in &plan.command_postlude {
        command_results.push(classify_command(command, profile));
    }

    let mut overall = MavlinkCompatibilityClass::Supported;
    for result in &command_results {
        overall = overall.max(result.classification);
    }
    let hardware_facing_allowed = profile.id != MavlinkCapabilityProfileId::MavlinkCommonGeneric
        && !command_results
            .iter()
            .any(|result| result.classification.blocks_hardware_facing_success());
    let aggregate_mode_requirements = command_results
        .iter()
        .filter_map(mode_requirement_from_result)
        .collect();
    let mut caveats = Vec::new();
    for caveat in profile.known_caveats {
        push_unique(&mut caveats, caveat);
    }
    for result in &command_results {
        for caveat in result.caveats.iter().chain(result.mode_caveats.iter()) {
            push_unique(&mut caveats, caveat);
        }
    }

    MavlinkCompatibilityReport {
        profile: profile.id,
        overall_classification: overall,
        hardware_facing_allowed,
        command_results,
        aggregate_mode_requirements,
        caveats,
    }
}

fn classify_command(
    command: &MavlinkCommonCommand,
    profile: &MavlinkCapabilityProfile,
) -> MavlinkCommandCompatibility {
    let rule = command_rule(profile, command.command);
    let mode_rule = mode_rule(profile, command.command);
    compatibility_from_parts(
        Some(command.command_id.clone()),
        None,
        command.command,
        command.phase,
        None,
        rule,
        mode_rule,
    )
}

fn classify_mission_item(
    item: &MavlinkCommonMissionItem,
    profile: &MavlinkCapabilityProfile,
) -> MavlinkCommandCompatibility {
    let frame_supported = profile
        .supported_frames
        .iter()
        .any(|frame| *frame == item.frame);
    if !frame_supported {
        return MavlinkCommandCompatibility {
            command_id: Some(item.command_id.clone()),
            seq: Some(item.seq),
            command: item.command,
            phase: MavlinkPlanPhase::MissionUpload,
            frame: Some(item.frame.clone()),
            classification: MavlinkCompatibilityClass::Unsupported,
            required_execution_mode: None,
            required_mode_transitions: Vec::new(),
            preconditions: Vec::new(),
            mode_caveats: Vec::new(),
            caveats: vec![format!(
                "frame '{}' is not supported by profile '{}'",
                item.frame,
                profile.id.as_str()
            )],
            reason: "unsupported coordinate frame".to_owned(),
        };
    }

    let rule = command_rule(profile, item.command);
    let mode_rule = mode_rule(profile, item.command);
    let mut result = compatibility_from_parts(
        Some(item.command_id.clone()),
        Some(item.seq),
        item.command,
        MavlinkPlanPhase::MissionUpload,
        Some(item.frame.clone()),
        rule,
        mode_rule,
    );
    if item.command == MavlinkCommonCommandName::NavWaypoint && item.command_id.starts_with("orbit")
    {
        result.classification = result
            .classification
            .max(MavlinkCompatibilityClass::SupportedViaFallback);
        push_unique(
            &mut result.caveats,
            "orbit intent is represented by waypoint approximation fallback",
        );
        result.reason = format!("{}; orbit waypoint fallback", result.reason);
    }
    result
}

fn compatibility_from_parts(
    command_id: Option<String>,
    seq: Option<u16>,
    command: MavlinkCommonCommandName,
    phase: MavlinkPlanPhase,
    frame: Option<String>,
    rule: Option<&'static MavlinkCommandCapabilityRule>,
    mode_rule: Option<&'static MavlinkModeTransitionRule>,
) -> MavlinkCommandCompatibility {
    let (classification, reason, mut caveats) = match rule {
        Some(rule) => (
            rule.classification,
            rule.reason.to_owned(),
            rule.caveats
                .iter()
                .map(|caveat| (*caveat).to_owned())
                .collect(),
        ),
        None => (
            MavlinkCompatibilityClass::Unsupported,
            "command is not listed in the selected capability profile".to_owned(),
            Vec::new(),
        ),
    };
    let (required_execution_mode, required_mode_transitions, preconditions, mode_caveats) =
        match mode_rule {
            Some(rule) => {
                for caveat in rule.mode_caveats {
                    push_unique(&mut caveats, caveat);
                }
                (
                    Some(rule.required_execution_mode),
                    rule.required_mode_transitions.to_vec(),
                    rule.preconditions
                        .iter()
                        .map(|value| (*value).to_owned())
                        .collect(),
                    rule.mode_caveats
                        .iter()
                        .map(|value| (*value).to_owned())
                        .collect(),
                )
            }
            None => (None, Vec::new(), Vec::new(), Vec::new()),
        };

    MavlinkCommandCompatibility {
        command_id,
        seq,
        command,
        phase,
        frame,
        classification,
        required_execution_mode,
        required_mode_transitions,
        preconditions,
        mode_caveats,
        caveats,
        reason,
    }
}

fn command_rule(
    profile: &MavlinkCapabilityProfile,
    command: MavlinkCommonCommandName,
) -> Option<&'static MavlinkCommandCapabilityRule> {
    profile
        .command_rules
        .iter()
        .find(|rule| rule.command == command)
}

fn mode_rule(
    profile: &MavlinkCapabilityProfile,
    command: MavlinkCommonCommandName,
) -> Option<&'static MavlinkModeTransitionRule> {
    profile
        .mode_transitions
        .iter()
        .find(|rule| rule.command == Some(command))
        .or_else(|| {
            profile
                .mode_transitions
                .iter()
                .find(|rule| rule.command.is_none())
        })
}

fn mode_requirement_from_result(
    result: &MavlinkCommandCompatibility,
) -> Option<MavlinkModeRequirement> {
    Some(MavlinkModeRequirement {
        command_id: result.command_id.clone(),
        command: result.command,
        phase: result.phase,
        required_execution_mode: result.required_execution_mode?,
        required_mode_transitions: result.required_mode_transitions.clone(),
        preconditions: result.preconditions.clone(),
        mode_caveats: result.mode_caveats.clone(),
    })
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_owned());
    }
}

const ALL_COMMON_COMMANDS_GENERIC: &[MavlinkCommandCapabilityRule] = &[
    generic_command_rule(MavlinkCommonCommandName::ComponentArmDisarm),
    generic_command_rule(MavlinkCommonCommandName::NavTakeoff),
    generic_command_rule(MavlinkCommonCommandName::NavLand),
    generic_command_rule(MavlinkCommonCommandName::NavReturnToLaunch),
    generic_command_rule(MavlinkCommonCommandName::NavWaypoint),
    generic_command_rule(MavlinkCommonCommandName::NavLoiterTime),
    generic_command_rule(MavlinkCommonCommandName::MissionStart),
];

const PX4_COMMANDS: &[MavlinkCommandCapabilityRule] = &[
    px4_command_rule(MavlinkCommonCommandName::ComponentArmDisarm),
    px4_command_rule(MavlinkCommonCommandName::NavTakeoff),
    px4_command_rule(MavlinkCommonCommandName::NavLand),
    px4_command_rule(MavlinkCommonCommandName::NavReturnToLaunch),
    px4_command_rule(MavlinkCommonCommandName::NavWaypoint),
    px4_command_rule(MavlinkCommonCommandName::NavLoiterTime),
    px4_command_rule(MavlinkCommonCommandName::MissionStart),
];

const ARDUPILOT_COMMANDS: &[MavlinkCommandCapabilityRule] = &[
    ardupilot_command_rule(MavlinkCommonCommandName::ComponentArmDisarm),
    ardupilot_command_rule(MavlinkCommonCommandName::NavTakeoff),
    ardupilot_command_rule(MavlinkCommonCommandName::NavLand),
    ardupilot_command_rule(MavlinkCommonCommandName::NavReturnToLaunch),
    ardupilot_command_rule(MavlinkCommonCommandName::NavWaypoint),
    ardupilot_command_rule(MavlinkCommonCommandName::NavLoiterTime),
    ardupilot_command_rule(MavlinkCommonCommandName::MissionStart),
];

const fn generic_command_rule(command: MavlinkCommonCommandName) -> MavlinkCommandCapabilityRule {
    MavlinkCommandCapabilityRule {
        command,
        classification: MavlinkCompatibilityClass::SupportedWithCaveats,
        reason: "MAVLink Common command is syntactically supported by the generic profile",
        caveats: &[GENERIC_CAVEAT],
    }
}

const fn px4_command_rule(command: MavlinkCommonCommandName) -> MavlinkCommandCapabilityRule {
    MavlinkCommandCapabilityRule {
        command,
        classification: MavlinkCompatibilityClass::SupportedWithCaveats,
        reason: "command is supported by the conservative PX4 profile with caveats",
        caveats: &[PX4_SIH_CAVEAT],
    }
}

const fn ardupilot_command_rule(command: MavlinkCommonCommandName) -> MavlinkCommandCapabilityRule {
    MavlinkCommandCapabilityRule {
        command,
        classification: MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        reason: "command syntax is known, but stack acceptance is not evidenced",
        caveats: &[ARDUPILOT_EVIDENCE_CAVEAT],
    }
}

const COMMON_MODE_TRANSITIONS: &[MavlinkModeTransitionRule] = &[MavlinkModeTransitionRule {
    command: None,
    required_execution_mode: MavlinkExecutionMode::Unspecified,
    required_mode_transitions: &[MavlinkRequiredModeTransition::Heartbeat],
    preconditions: &["MAVLink heartbeat is required before command execution"],
    mode_caveats: &[GENERIC_CAVEAT],
}];

const PX4_MODE_TRANSITIONS: &[MavlinkModeTransitionRule] = &[
    MavlinkModeTransitionRule {
        command: Some(MavlinkCommonCommandName::ComponentArmDisarm),
        required_execution_mode: MavlinkExecutionMode::Command,
        required_mode_transitions: &[
            MavlinkRequiredModeTransition::Heartbeat,
            MavlinkRequiredModeTransition::Armed,
        ],
        preconditions: &["vehicle heartbeat is present", "arming command is accepted"],
        mode_caveats: &[PX4_SIH_CAVEAT],
    },
    MavlinkModeTransitionRule {
        command: Some(MavlinkCommonCommandName::NavTakeoff),
        required_execution_mode: MavlinkExecutionMode::Command,
        required_mode_transitions: &[
            MavlinkRequiredModeTransition::Armed,
            MavlinkRequiredModeTransition::TakeoffAccepted,
        ],
        preconditions: &["vehicle is armed", "takeoff command is accepted"],
        mode_caveats: &[PX4_SIH_CAVEAT],
    },
    MavlinkModeTransitionRule {
        command: Some(MavlinkCommonCommandName::MissionStart),
        required_execution_mode: MavlinkExecutionMode::Mission,
        required_mode_transitions: &[
            MavlinkRequiredModeTransition::MissionUploadAccepted,
            MavlinkRequiredModeTransition::MissionStartAccepted,
        ],
        preconditions: &["mission upload is accepted before MAV_CMD_MISSION_START"],
        mode_caveats: &[PX4_SIH_CAVEAT],
    },
    MavlinkModeTransitionRule {
        command: None,
        required_execution_mode: MavlinkExecutionMode::Mission,
        required_mode_transitions: &[MavlinkRequiredModeTransition::MissionUploadAccepted],
        preconditions: &["mission item upload is accepted"],
        mode_caveats: &[PX4_SIH_CAVEAT],
    },
];

const ARDUPILOT_MODE_TRANSITIONS: &[MavlinkModeTransitionRule] = &[
    MavlinkModeTransitionRule {
        command: Some(MavlinkCommonCommandName::MissionStart),
        required_execution_mode: MavlinkExecutionMode::Mission,
        required_mode_transitions: &[
            MavlinkRequiredModeTransition::MissionUploadAccepted,
            MavlinkRequiredModeTransition::MissionStartAccepted,
        ],
        preconditions: &["ArduPilot mission mode/start behavior requires SITL evidence"],
        mode_caveats: &[ARDUPILOT_EVIDENCE_CAVEAT],
    },
    MavlinkModeTransitionRule {
        command: None,
        required_execution_mode: MavlinkExecutionMode::Unspecified,
        required_mode_transitions: &[MavlinkRequiredModeTransition::Heartbeat],
        preconditions: &["ArduPilot mode mapping is not validated by this repository yet"],
        mode_caveats: &[ARDUPILOT_EVIDENCE_CAVEAT],
    },
];

static MAVLINK_COMMON_GENERIC_PROFILE: MavlinkCapabilityProfile = MavlinkCapabilityProfile {
    id: MavlinkCapabilityProfileId::MavlinkCommonGeneric,
    stack_name: "MAVLink Common generic",
    supported_frames: &[MAVLINK_GLOBAL_RELATIVE_ALT_INT],
    command_rules: ALL_COMMON_COMMANDS_GENERIC,
    mode_transitions: COMMON_MODE_TRANSITIONS,
    mission_start_semantics:
        "Syntax-level MAV_CMD_MISSION_START only; no autopilot acceptance proof.",
    takeoff_landing_constraints: &["Takeoff/landing commands are syntax-level only."],
    geofence_support: MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
    parameter_support: MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
    known_caveats: &[GENERIC_CAVEAT],
};

static PX4_PROFILE: MavlinkCapabilityProfile = MavlinkCapabilityProfile {
    id: MavlinkCapabilityProfileId::Px4,
    stack_name: "PX4",
    supported_frames: &[MAVLINK_GLOBAL_RELATIVE_ALT_INT],
    command_rules: PX4_COMMANDS,
    mode_transitions: PX4_MODE_TRANSITIONS,
    mission_start_semantics: "PX4 SIH path uses mission upload followed by MAV_CMD_MISSION_START.",
    takeoff_landing_constraints: &[
        "Requires heartbeat and accepted arm/takeoff flow.",
        "Evidence is local PX4/SIH, not hardware certification.",
    ],
    geofence_support: MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
    parameter_support: MavlinkCompatibilityClass::SupportedWithCaveats,
    known_caveats: &[PX4_SIH_CAVEAT],
};

static ARDUPILOT_PROFILE: MavlinkCapabilityProfile = MavlinkCapabilityProfile {
    id: MavlinkCapabilityProfileId::ArduPilot,
    stack_name: "ArduPilot",
    supported_frames: &[MAVLINK_GLOBAL_RELATIVE_ALT_INT],
    command_rules: ARDUPILOT_COMMANDS,
    mode_transitions: ARDUPILOT_MODE_TRANSITIONS,
    mission_start_semantics:
        "ArduPilot mission start semantics are intentionally not claimed before SITL evidence.",
    takeoff_landing_constraints: &[
        "Mode mapping and command acceptance require ArduPilot SITL or hardware evidence.",
    ],
    geofence_support: MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
    parameter_support: MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
    known_caveats: &[ARDUPILOT_EVIDENCE_CAVEAT],
};

const COMPATIBILITY_MATRIX_ROWS: &[MavlinkCompatibilityMatrixRow] = &[
    matrix_row(
        "mavlink_common_generic:command_support",
        MavlinkCapabilityProfileId::MavlinkCommonGeneric,
        "command_support",
        MavlinkCompatibilityClass::SupportedWithCaveats,
        "Common commands are syntax-level only.",
    ),
    matrix_row(
        "mavlink_common_generic:frame_support",
        MavlinkCapabilityProfileId::MavlinkCommonGeneric,
        "frame_support",
        MavlinkCompatibilityClass::SupportedWithCaveats,
        "Uses MAV_FRAME_GLOBAL_RELATIVE_ALT_INT.",
    ),
    matrix_row(
        "mavlink_common_generic:mode_transitions",
        MavlinkCapabilityProfileId::MavlinkCommonGeneric,
        "mode_transitions",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "No concrete autopilot mode proof.",
    ),
    matrix_row(
        "mavlink_common_generic:mission_start",
        MavlinkCapabilityProfileId::MavlinkCommonGeneric,
        "mission_start",
        MavlinkCompatibilityClass::SupportedWithCaveats,
        "MAV_CMD_MISSION_START syntax only.",
    ),
    matrix_row(
        "mavlink_common_generic:loiter_orbit",
        MavlinkCapabilityProfileId::MavlinkCommonGeneric,
        "loiter_orbit",
        MavlinkCompatibilityClass::SupportedViaFallback,
        "Loiter time and waypoint orbit fallback only.",
    ),
    matrix_row(
        "mavlink_common_generic:geofence",
        MavlinkCapabilityProfileId::MavlinkCommonGeneric,
        "geofence",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "No geofence compiler output yet.",
    ),
    matrix_row(
        "mavlink_common_generic:parameters",
        MavlinkCapabilityProfileId::MavlinkCommonGeneric,
        "parameters",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "No autopilot metadata validation.",
    ),
    matrix_row(
        "px4:command_support",
        MavlinkCapabilityProfileId::Px4,
        "command_support",
        MavlinkCompatibilityClass::SupportedWithCaveats,
        "Core primitive commands have local PX4/SIH evidence.",
    ),
    matrix_row(
        "px4:frame_support",
        MavlinkCapabilityProfileId::Px4,
        "frame_support",
        MavlinkCompatibilityClass::SupportedWithCaveats,
        "Uses MAV_FRAME_GLOBAL_RELATIVE_ALT_INT in local SIH evidence.",
    ),
    matrix_row(
        "px4:mode_transitions",
        MavlinkCapabilityProfileId::Px4,
        "mode_transitions",
        MavlinkCompatibilityClass::SupportedWithCaveats,
        "Heartbeat, arm, takeoff, upload and mission-start assumptions are explicit.",
    ),
    matrix_row(
        "px4:mission_start",
        MavlinkCapabilityProfileId::Px4,
        "mission_start",
        MavlinkCompatibilityClass::SupportedWithCaveats,
        "MAV_CMD_MISSION_START is the current PX4 path.",
    ),
    matrix_row(
        "px4:loiter_orbit",
        MavlinkCapabilityProfileId::Px4,
        "loiter_orbit",
        MavlinkCompatibilityClass::SupportedViaFallback,
        "Orbit is waypoint approximation; direct orbit is not claimed.",
    ),
    matrix_row(
        "px4:geofence",
        MavlinkCapabilityProfileId::Px4,
        "geofence",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "Geofence profile is future work.",
    ),
    matrix_row(
        "px4:parameters",
        MavlinkCapabilityProfileId::Px4,
        "parameters",
        MavlinkCompatibilityClass::SupportedWithCaveats,
        "Only emitted primitive parameters are covered.",
    ),
    matrix_row(
        "ardupilot:command_support",
        MavlinkCapabilityProfileId::ArduPilot,
        "command_support",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "Common syntax is known; acceptance is not evidenced.",
    ),
    matrix_row(
        "ardupilot:frame_support",
        MavlinkCapabilityProfileId::ArduPilot,
        "frame_support",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "Frame syntax is known; ArduPilot acceptance needs SITL evidence.",
    ),
    matrix_row(
        "ardupilot:mode_transitions",
        MavlinkCapabilityProfileId::ArduPilot,
        "mode_transitions",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "Mode mapping requires ArduPilot SITL evidence.",
    ),
    matrix_row(
        "ardupilot:mission_start",
        MavlinkCapabilityProfileId::ArduPilot,
        "mission_start",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "Mission start semantics are not claimed yet.",
    ),
    matrix_row(
        "ardupilot:loiter_orbit",
        MavlinkCapabilityProfileId::ArduPilot,
        "loiter_orbit",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "Loiter/orbit acceptance is not evidenced.",
    ),
    matrix_row(
        "ardupilot:geofence",
        MavlinkCapabilityProfileId::ArduPilot,
        "geofence",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "Geofence profile is future work.",
    ),
    matrix_row(
        "ardupilot:parameters",
        MavlinkCapabilityProfileId::ArduPilot,
        "parameters",
        MavlinkCompatibilityClass::UnknownUntilSitlOrHardware,
        "No ArduPilot parameter metadata validation.",
    ),
];

const fn matrix_row(
    row_key: &'static str,
    profile: MavlinkCapabilityProfileId,
    dimension: &'static str,
    classification: MavlinkCompatibilityClass,
    summary: &'static str,
) -> MavlinkCompatibilityMatrixRow {
    MavlinkCompatibilityMatrixRow {
        row_key,
        profile,
        dimension,
        classification,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mavlink_common_plan::{
        compile_mavlink_common_plan, MavlinkCommonPlanOptions, MavlinkOrbitStrategy,
    };
    use crate::mavlink_coords::MavlinkCoordinateOrigin;
    use swarm_mission_ir::{
        AltitudeReference, CommandId, CompletionTolerance, CoordinateFrame, LocalPosition,
        MissionCommand, MissionCommandEntry, MissionCommandPlan, MissionId, Position,
        TerminalState, TimeoutAction, TimeoutPolicy,
    };

    fn origin() -> MavlinkCoordinateOrigin {
        MavlinkCoordinateOrigin {
            lat_deg: 47.397_742,
            lon_deg: 8.545_594,
            alt_m: 0.0,
        }
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
            mission_id: MissionId::from("m82-test".to_owned()),
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

    fn options(profile: MavlinkCapabilityProfileId) -> MavlinkCommonPlanOptions {
        MavlinkCommonPlanOptions {
            capability_profile: profile,
            home_origin: Some(origin()),
            default_hold_position: Some(Position::Local(LocalPosition {
                x_m: 0.0,
                y_m: 0.0,
                z_m: 3.0,
            })),
            orbit_strategy: MavlinkOrbitStrategy::WaypointApproximation {
                segments_per_turn: 4,
            },
            ..Default::default()
        }
    }

    #[test]
    fn profile_marks_supported_primitive_commands() {
        let compiled = compile_mavlink_common_plan(
            &plan(vec![
                entry("arm", MissionCommand::Arm),
                entry("takeoff", MissionCommand::Takeoff { altitude_m: 3.0 }),
                entry("land", MissionCommand::Land),
            ]),
            &options(MavlinkCapabilityProfileId::Px4),
        )
        .unwrap();
        let report = compiled.compatibility.unwrap();

        assert_eq!(report.profile, MavlinkCapabilityProfileId::Px4);
        assert_eq!(
            report.overall_classification,
            MavlinkCompatibilityClass::SupportedWithCaveats
        );
        assert!(report
            .command_results
            .iter()
            .all(|result| result.classification != MavlinkCompatibilityClass::Unsupported));
    }

    #[test]
    fn generic_profile_is_not_hardware_facing_success() {
        let compiled = compile_mavlink_common_plan(
            &plan(vec![entry(
                "takeoff",
                MissionCommand::Takeoff { altitude_m: 3.0 },
            )]),
            &options(MavlinkCapabilityProfileId::MavlinkCommonGeneric),
        )
        .unwrap();
        let report = compiled.compatibility.unwrap();

        assert_eq!(
            report.overall_classification,
            MavlinkCompatibilityClass::SupportedWithCaveats
        );
        assert!(!report.hardware_facing_allowed);
    }

    #[test]
    fn unknown_profile_value_is_not_treated_as_supported() {
        let error = MavlinkCapabilityProfileId::from_str("definitely-not-a-profile").unwrap_err();

        assert_eq!(error.value, "definitely-not-a-profile");
    }

    #[test]
    fn unsupported_frame_fails_compatibility_pass() {
        let mut compiled = compile_mavlink_common_plan(
            &plan(vec![entry(
                "goto",
                MissionCommand::GoTo {
                    position: Position::Local(LocalPosition {
                        x_m: 1.0,
                        y_m: 2.0,
                        z_m: 3.0,
                    }),
                },
            )]),
            &options(MavlinkCapabilityProfileId::Px4),
        )
        .unwrap();
        compiled.mission_items[0].frame = "MAV_FRAME_UNSUPPORTED_TEST".to_owned();

        let report = classify_mavlink_plan_compatibility(
            &compiled,
            MavlinkCapabilityProfileId::Px4.profile(),
        );

        assert_eq!(report.unsupported_count(), 1);
        assert!(!report.hardware_facing_allowed);
    }

    #[test]
    fn caveat_text_appears_for_supported_with_caveats() {
        let compiled = compile_mavlink_common_plan(
            &plan(vec![entry(
                "takeoff",
                MissionCommand::Takeoff { altitude_m: 3.0 },
            )]),
            &options(MavlinkCapabilityProfileId::Px4),
        )
        .unwrap();
        let report = compiled.compatibility.unwrap();

        assert!(report.caveats.iter().any(|caveat| caveat.contains("PX4")));
        assert!(report
            .command_results
            .iter()
            .any(|result| result.caveats.iter().any(|caveat| caveat.contains("PX4"))));
    }

    #[test]
    fn px4_and_ardupilot_profiles_classify_core_primitive_missions() {
        let mission = plan(vec![
            entry("arm", MissionCommand::Arm),
            entry("takeoff", MissionCommand::Takeoff { altitude_m: 3.0 }),
            entry(
                "goto",
                MissionCommand::GoTo {
                    position: Position::Local(LocalPosition {
                        x_m: 1.0,
                        y_m: 2.0,
                        z_m: 3.0,
                    }),
                },
            ),
            entry("land", MissionCommand::Land),
        ]);

        let px4 = compile_mavlink_common_plan(&mission, &options(MavlinkCapabilityProfileId::Px4))
            .unwrap()
            .compatibility
            .unwrap();
        let ardupilot =
            compile_mavlink_common_plan(&mission, &options(MavlinkCapabilityProfileId::ArduPilot))
                .unwrap()
                .compatibility
                .unwrap();

        assert_eq!(
            px4.overall_classification,
            MavlinkCompatibilityClass::SupportedWithCaveats
        );
        assert_eq!(
            ardupilot.overall_classification,
            MavlinkCompatibilityClass::UnknownUntilSitlOrHardware
        );
        assert!(!ardupilot.hardware_facing_allowed);
    }

    #[test]
    fn primitive_mission_classification_includes_mode_requirements() {
        let compiled = compile_mavlink_common_plan(
            &plan(vec![
                entry("takeoff", MissionCommand::Takeoff { altitude_m: 3.0 }),
                entry(
                    "goto",
                    MissionCommand::GoTo {
                        position: Position::Local(LocalPosition {
                            x_m: 1.0,
                            y_m: 2.0,
                            z_m: 3.0,
                        }),
                    },
                ),
            ]),
            &options(MavlinkCapabilityProfileId::Px4),
        )
        .unwrap();
        let report = compiled.compatibility.unwrap();
        let mission_start = report
            .command_results
            .iter()
            .find(|result| result.command == MavlinkCommonCommandName::MissionStart)
            .expect("mission start result");

        assert_eq!(
            mission_start.required_execution_mode,
            Some(MavlinkExecutionMode::Mission)
        );
        assert!(mission_start
            .required_mode_transitions
            .contains(&MavlinkRequiredModeTransition::MissionStartAccepted));
        assert!(!mission_start.preconditions.is_empty());
        assert!(!mission_start.mode_caveats.is_empty());
    }

    #[test]
    fn compatibility_matrix_contains_expected_profile_dimensions() {
        for profile in [
            MavlinkCapabilityProfileId::MavlinkCommonGeneric,
            MavlinkCapabilityProfileId::Px4,
            MavlinkCapabilityProfileId::ArduPilot,
        ] {
            for dimension in [
                "command_support",
                "frame_support",
                "mode_transitions",
                "mission_start",
                "loiter_orbit",
                "geofence",
                "parameters",
            ] {
                let key = format!("{}:{dimension}", profile.as_str());
                assert!(
                    compatibility_matrix_rows()
                        .iter()
                        .any(|row| row.row_key == key),
                    "missing matrix row {key}"
                );
            }
        }
    }
}
