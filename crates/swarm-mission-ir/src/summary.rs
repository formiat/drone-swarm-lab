use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    command::MissionCommand,
    plan::MissionCommandPlan,
    policy::{CompletionTolerance, TerminalState, TimeoutPolicy},
};

/// Compact summary of a `MissionCommandPlan` for inclusion in dry-run artifacts.
///
/// Uses `BTreeMap` to guarantee deterministic key order in JSON output.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MissionCommandSummary {
    pub mission_id: String,
    pub command_count: usize,
    /// key: command kind name, value: count
    pub commands_by_kind: BTreeMap<String, usize>,
    pub coordinate_frame: String,
    pub altitude_reference: String,
    /// Total number of waypoints across all `follow_route` and `go_to` commands.
    pub total_waypoints: usize,
    #[serde(default = "default_timeout_policy")]
    pub timeout_policy: TimeoutPolicy,
    #[serde(default = "default_terminal_state")]
    pub expected_terminal_state: TerminalState,
    #[serde(default = "default_completion_tolerance")]
    pub completion_tolerance: CompletionTolerance,
}

fn default_timeout_policy() -> TimeoutPolicy {
    TimeoutPolicy {
        command_timeout_secs: 5.0,
        completion_timeout_secs: 120.0,
        on_timeout: crate::policy::TimeoutAction::Abort,
    }
}

fn default_terminal_state() -> TerminalState {
    TerminalState::Landed
}

fn default_completion_tolerance() -> CompletionTolerance {
    CompletionTolerance {
        position_m: 1.0,
        altitude_m: 0.5,
    }
}

impl MissionCommandSummary {
    /// Builds a summary from a validated (or unvalidated) plan.
    pub fn from_plan(plan: &MissionCommandPlan) -> Self {
        let mut commands_by_kind: BTreeMap<String, usize> = BTreeMap::new();
        let mut total_waypoints: usize = 0;

        for entry in &plan.commands {
            *commands_by_kind
                .entry(entry.command.kind_name().to_owned())
                .or_insert(0) += 1;

            match &entry.command {
                MissionCommand::FollowRoute { waypoints, .. } => {
                    total_waypoints += waypoints.len();
                }
                MissionCommand::GoTo { .. } => {
                    total_waypoints += 1;
                }
                _ => {}
            }
        }

        let coordinate_frame = serde_json::to_value(plan.coordinate_frame)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let altitude_reference = serde_json::to_value(plan.altitude_reference)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        Self {
            mission_id: plan.mission_id.as_ref().clone(),
            command_count: plan.commands.len(),
            commands_by_kind,
            coordinate_frame,
            altitude_reference,
            total_waypoints,
            timeout_policy: plan.timeout_policy.clone(),
            expected_terminal_state: plan.expected_terminal_state,
            completion_tolerance: plan.completion_tolerance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::MissionCommand,
        frame::{AltitudeReference, CoordinateFrame},
        ids::{CommandId, MissionId, RouteId},
        plan::{MissionCommandEntry, MissionCommandPlan},
        policy::{CompletionTolerance, TerminalState, TimeoutAction, TimeoutPolicy},
        position::{LocalPosition, Position},
        waypoint::MissionWaypoint,
    };

    fn make_entry(id: &str, cmd: MissionCommand) -> MissionCommandEntry {
        MissionCommandEntry {
            command_id: CommandId::from(id.to_owned()),
            command: cmd,
            source_task_id: None,
            source_route_id: None,
            source_agent_id: None,
        }
    }

    fn local(x: f64, y: f64) -> MissionWaypoint {
        MissionWaypoint {
            position: Position::Local(LocalPosition {
                x_m: x,
                y_m: y,
                z_m: 5.0,
            }),
            acceptance_radius_m: None,
        }
    }

    fn simple_plan(commands: Vec<MissionCommandEntry>) -> MissionCommandPlan {
        MissionCommandPlan {
            schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
            mission_id: MissionId::from("m-test".to_owned()),
            coordinate_frame: CoordinateFrame::LocalNed,
            altitude_reference: AltitudeReference::RelativeHome,
            timeout_policy: TimeoutPolicy {
                command_timeout_secs: 5.0,
                completion_timeout_secs: 30.0,
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

    #[test]
    fn summary_counts_commands_correctly() {
        let plan = simple_plan(vec![
            make_entry("c1", MissionCommand::Arm),
            make_entry("c2", MissionCommand::Takeoff { altitude_m: 5.0 }),
            make_entry(
                "c3",
                MissionCommand::Hold {
                    duration_secs: 10.0,
                },
            ),
            make_entry("c4", MissionCommand::Land),
            make_entry("c5", MissionCommand::Arm),
        ]);
        let summary = MissionCommandSummary::from_plan(&plan);
        assert_eq!(summary.command_count, 5);
        assert_eq!(*summary.commands_by_kind.get("arm").unwrap(), 2);
        assert_eq!(*summary.commands_by_kind.get("takeoff").unwrap(), 1);
        assert_eq!(*summary.commands_by_kind.get("land").unwrap(), 1);
        assert_eq!(*summary.commands_by_kind.get("hold").unwrap(), 1);
    }

    #[test]
    fn summary_counts_waypoints() {
        let plan = simple_plan(vec![
            make_entry(
                "c1",
                MissionCommand::FollowRoute {
                    route_id: RouteId::from("r".to_owned()),
                    waypoints: vec![local(0.0, 0.0), local(10.0, 0.0), local(10.0, 10.0)],
                },
            ),
            make_entry(
                "c2",
                MissionCommand::GoTo {
                    position: Position::Local(LocalPosition {
                        x_m: 5.0,
                        y_m: 5.0,
                        z_m: 5.0,
                    }),
                },
            ),
        ]);
        let summary = MissionCommandSummary::from_plan(&plan);
        assert_eq!(summary.total_waypoints, 4); // 3 from follow_route + 1 from go_to
    }

    #[test]
    fn summary_mission_id_matches() {
        let plan = simple_plan(vec![]);
        let summary = MissionCommandSummary::from_plan(&plan);
        assert_eq!(summary.mission_id, "m-test");
    }

    #[test]
    fn summary_coordinate_frame_uses_serde_snake_case() {
        let plan = simple_plan(vec![]);
        let summary = MissionCommandSummary::from_plan(&plan);
        // simple_plan uses LocalNed — must serialise as "local_ned", not "localned"
        assert_eq!(summary.coordinate_frame, "local_ned");
        // simple_plan uses RelativeHome — must serialise as "relative_home", not "relativehome"
        assert_eq!(summary.altitude_reference, "relative_home");
    }

    #[test]
    fn summary_wgs84_frame_name() {
        use crate::frame::CoordinateFrame;
        let mut plan = simple_plan(vec![]);
        plan.coordinate_frame = CoordinateFrame::Wgs84;
        let summary = MissionCommandSummary::from_plan(&plan);
        assert_eq!(summary.coordinate_frame, "wgs84");
    }

    // Ensure the old broken behavior is absent (regression guard).
    #[test]
    fn summary_coordinate_frame_is_not_debug_lowercased() {
        let plan = simple_plan(vec![]);
        let summary = MissionCommandSummary::from_plan(&plan);
        assert_ne!(
            summary.coordinate_frame, "localned",
            "coordinate_frame must not be Debug-lowercased"
        );
        assert_ne!(
            summary.altitude_reference, "relativehome",
            "altitude_reference must not be Debug-lowercased"
        );
    }

    #[test]
    fn summary_includes_policy_fields() {
        let summary = MissionCommandSummary::from_plan(&simple_plan(vec![]));

        assert_eq!(summary.timeout_policy.on_timeout, TimeoutAction::Abort);
        assert_eq!(summary.timeout_policy.command_timeout_secs, 5.0);
        assert_eq!(summary.timeout_policy.completion_timeout_secs, 30.0);
        assert_eq!(summary.expected_terminal_state, TerminalState::Landed);
        assert_eq!(
            summary.completion_tolerance,
            CompletionTolerance {
                position_m: 1.0,
                altitude_m: 0.5,
            }
        );
    }
}
