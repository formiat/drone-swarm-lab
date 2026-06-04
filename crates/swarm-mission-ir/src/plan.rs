use serde::{Deserialize, Serialize};

use crate::{
    command::MissionCommand,
    frame::{AltitudeReference, CoordinateFrame},
    ids::{CommandId, MissionId},
    policy::{CompletionTolerance, TerminalState, TimeoutPolicy},
};

/// A single command entry in a mission sequence, with identity and provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissionCommandEntry {
    /// Unique identifier for this command within the plan.
    pub command_id: CommandId,
    /// The command primitive.
    pub command: MissionCommand,
    /// Source task id from which this command was derived, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_task_id: Option<String>,
    /// Source route id associated with this command, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_route_id: Option<String>,
    /// Source agent id that owns this command, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_agent_id: Option<String>,
}

/// A complete hardware-agnostic mission command plan — the M80 IR.
///
/// This is **not** a MAVLink plan. It is an intermediate representation that a
/// backend compiler (M81+) translates into protocol-specific command sequences.
/// The plan encodes mission intent only: no PX4/ArduPilot modes, no MAVLink
/// message fields, no hardware execution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissionCommandPlan {
    /// Schema identifier for this artifact.
    pub schema_version: String,
    /// Unique mission identifier.
    pub mission_id: MissionId,
    /// Coordinate reference frame used by all positions in this plan.
    pub coordinate_frame: CoordinateFrame,
    /// Altitude reference datum for all altitude values.
    pub altitude_reference: AltitudeReference,
    /// Timeout configuration.
    pub timeout_policy: TimeoutPolicy,
    /// Expected vehicle state after the plan completes successfully.
    pub expected_terminal_state: TerminalState,
    /// Acceptable position and altitude error for command completion.
    pub completion_tolerance: CompletionTolerance,
    /// Ordered sequence of commands.
    pub commands: Vec<MissionCommandEntry>,
}

impl MissionCommandPlan {
    /// Current schema version string for newly constructed plans.
    pub const SCHEMA_VERSION: &'static str = "mission_command_ir.v1";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::MissionCommand,
        ids::RouteId,
        orbit::OrbitDirection,
        policy::TimeoutAction,
        position::{LocalPosition, Position},
        waypoint::MissionWaypoint,
    };

    fn default_plan() -> MissionCommandPlan {
        MissionCommandPlan {
            schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
            mission_id: MissionId::from("m-1".to_owned()),
            coordinate_frame: CoordinateFrame::LocalNed,
            altitude_reference: AltitudeReference::RelativeHome,
            timeout_policy: TimeoutPolicy {
                command_timeout_secs: 5.0,
                completion_timeout_secs: 60.0,
                on_timeout: TimeoutAction::Abort,
            },
            expected_terminal_state: TerminalState::Landed,
            completion_tolerance: CompletionTolerance {
                position_m: 1.0,
                altitude_m: 0.5,
            },
            commands: vec![
                MissionCommandEntry {
                    command_id: CommandId::from("c-1".to_owned()),
                    command: MissionCommand::Arm,
                    source_task_id: None,
                    source_route_id: None,
                    source_agent_id: None,
                },
                MissionCommandEntry {
                    command_id: CommandId::from("c-2".to_owned()),
                    command: MissionCommand::Takeoff { altitude_m: 5.0 },
                    source_task_id: None,
                    source_route_id: None,
                    source_agent_id: None,
                },
                MissionCommandEntry {
                    command_id: CommandId::from("c-3".to_owned()),
                    command: MissionCommand::FollowRoute {
                        route_id: RouteId::from("r-1".to_owned()),
                        waypoints: vec![
                            MissionWaypoint {
                                position: Position::Local(LocalPosition {
                                    x_m: 10.0,
                                    y_m: 0.0,
                                    z_m: 5.0,
                                }),
                                acceptance_radius_m: None,
                            },
                            MissionWaypoint {
                                position: Position::Local(LocalPosition {
                                    x_m: 10.0,
                                    y_m: 10.0,
                                    z_m: 5.0,
                                }),
                                acceptance_radius_m: None,
                            },
                        ],
                    },
                    source_task_id: None,
                    source_route_id: Some("r-1".to_owned()),
                    source_agent_id: None,
                },
                MissionCommandEntry {
                    command_id: CommandId::from("c-4".to_owned()),
                    command: MissionCommand::Land,
                    source_task_id: None,
                    source_route_id: None,
                    source_agent_id: None,
                },
            ],
        }
    }

    #[test]
    fn plan_roundtrip() {
        let plan = default_plan();
        let json = serde_json::to_string(&plan).unwrap();
        let back: MissionCommandPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan, back);
    }

    #[test]
    fn schema_version_constant() {
        assert_eq!(MissionCommandPlan::SCHEMA_VERSION, "mission_command_ir.v1");
    }

    #[test]
    fn command_order_preserved_after_roundtrip() {
        let plan = default_plan();
        let json = serde_json::to_string(&plan).unwrap();
        let back: MissionCommandPlan = serde_json::from_str(&json).unwrap();
        let ids_original: Vec<_> = plan.commands.iter().map(|e| e.command_id.clone()).collect();
        let ids_back: Vec<_> = back.commands.iter().map(|e| e.command_id.clone()).collect();
        assert_eq!(ids_original, ids_back);
    }

    #[test]
    fn orbit_plan_roundtrip() {
        let mut plan = default_plan();
        plan.commands.push(MissionCommandEntry {
            command_id: CommandId::from("c-orbit".to_owned()),
            command: MissionCommand::Orbit {
                center: Position::Local(LocalPosition {
                    x_m: 0.0,
                    y_m: 0.0,
                    z_m: 5.0,
                }),
                radius_m: 10.0,
                turns: 3.0,
                direction: OrbitDirection::CounterClockwise,
            },
            source_task_id: None,
            source_route_id: None,
            source_agent_id: None,
        });
        let json = serde_json::to_string(&plan).unwrap();
        let back: MissionCommandPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan, back);
    }
}
