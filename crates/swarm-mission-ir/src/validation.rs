use std::collections::HashSet;

use crate::{
    command::MissionCommand, error::MissionIrError, frame::CoordinateFrame,
    plan::MissionCommandPlan, position::Position,
};

/// Validates a [`MissionCommandPlan`] for internal consistency.
///
/// Checks performed:
/// - No duplicate `CommandId` values.
/// - `Takeoff` altitude must be positive.
/// - `Hold` and `LoiterTime` durations must be positive.
/// - `Orbit` radius and turn count must be positive.
/// - `FollowRoute` must have at least one waypoint.
/// - All position coordinates must be finite (not NaN or infinite).
/// - Positions must be consistent with the plan's `CoordinateFrame`.
pub fn validate(plan: &MissionCommandPlan) -> Result<(), MissionIrError> {
    check_duplicate_ids(plan)?;
    for entry in &plan.commands {
        validate_command(&entry.command, plan.coordinate_frame)?;
    }
    Ok(())
}

fn check_duplicate_ids(plan: &MissionCommandPlan) -> Result<(), MissionIrError> {
    let mut seen = HashSet::new();
    for entry in &plan.commands {
        let id = entry.command_id.as_ref().as_str();
        if !seen.insert(id) {
            return Err(MissionIrError::DuplicateCommandId(id.to_owned()));
        }
    }
    Ok(())
}

fn validate_command(cmd: &MissionCommand, frame: CoordinateFrame) -> Result<(), MissionIrError> {
    match cmd {
        MissionCommand::Takeoff { altitude_m } => {
            if !altitude_m.is_finite() || *altitude_m <= 0.0 {
                return Err(MissionIrError::InvalidTakeoffAltitude {
                    altitude_m: *altitude_m,
                });
            }
        }
        MissionCommand::Hold { duration_secs } => {
            if !duration_secs.is_finite() || *duration_secs <= 0.0 {
                return Err(MissionIrError::InvalidDuration {
                    duration_secs: *duration_secs,
                });
            }
        }
        MissionCommand::LoiterTime { duration_secs } => {
            if !duration_secs.is_finite() || *duration_secs <= 0.0 {
                return Err(MissionIrError::InvalidDuration {
                    duration_secs: *duration_secs,
                });
            }
        }
        MissionCommand::GoTo { position } => {
            validate_position(position, "go_to.position".to_owned(), frame)?;
        }
        MissionCommand::FollowRoute {
            route_id,
            waypoints,
        } => {
            if waypoints.is_empty() {
                return Err(MissionIrError::EmptyRoute {
                    route_id: route_id.as_ref().clone(),
                });
            }
            for (i, wp) in waypoints.iter().enumerate() {
                let context = format!("follow_route.waypoints[{i}]");
                validate_position(&wp.position, context, frame)?;
            }
        }
        MissionCommand::Orbit {
            center,
            radius_m,
            turns,
            ..
        } => {
            validate_position(center, "orbit.center".to_owned(), frame)?;
            if !radius_m.is_finite() || *radius_m <= 0.0 {
                return Err(MissionIrError::InvalidOrbitRadius {
                    radius_m: *radius_m,
                });
            }
            if !turns.is_finite() || *turns <= 0.0 {
                return Err(MissionIrError::InvalidOrbitTurns { turns: *turns });
            }
        }
        MissionCommand::Arm
        | MissionCommand::Disarm
        | MissionCommand::Land
        | MissionCommand::ReturnToLaunch
        | MissionCommand::Pause
        | MissionCommand::Resume
        | MissionCommand::Abort => {}
    }
    Ok(())
}

fn validate_position(
    position: &Position,
    context: String,
    frame: CoordinateFrame,
) -> Result<(), MissionIrError> {
    match position {
        Position::Geo(geo) => {
            if !geo.lat_deg.is_finite() || !geo.lon_deg.is_finite() || !geo.alt_m.is_finite() {
                return Err(MissionIrError::NonFiniteCoordinate {
                    context,
                    x: geo.lat_deg,
                    y: geo.lon_deg,
                    z: geo.alt_m,
                });
            }
            if frame != CoordinateFrame::Wgs84 {
                return Err(MissionIrError::AmbiguousCoordinateFrame {
                    kind: "geo".to_owned(),
                    frame: format!("{frame:?}"),
                });
            }
        }
        Position::Local(loc) => {
            if !loc.x_m.is_finite() || !loc.y_m.is_finite() || !loc.z_m.is_finite() {
                return Err(MissionIrError::NonFiniteCoordinate {
                    context,
                    x: loc.x_m,
                    y: loc.y_m,
                    z: loc.z_m,
                });
            }
            if frame == CoordinateFrame::Wgs84 {
                return Err(MissionIrError::AmbiguousCoordinateFrame {
                    kind: "local".to_owned(),
                    frame: "Wgs84".to_owned(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::MissionCommand,
        frame::{AltitudeReference, CoordinateFrame},
        ids::{CommandId, MissionId, RouteId},
        orbit::OrbitDirection,
        plan::{MissionCommandEntry, MissionCommandPlan},
        policy::{CompletionTolerance, TerminalState, TimeoutAction, TimeoutPolicy},
        position::{GeoPosition, LocalPosition, Position},
    };

    fn make_plan(frame: CoordinateFrame, commands: Vec<MissionCommandEntry>) -> MissionCommandPlan {
        MissionCommandPlan {
            schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
            mission_id: MissionId::from("m-test".to_owned()),
            coordinate_frame: frame,
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

    fn entry(id: &str, cmd: MissionCommand) -> MissionCommandEntry {
        MissionCommandEntry {
            command_id: CommandId::from(id.to_owned()),
            command: cmd,
            source_task_id: None,
            source_route_id: None,
            source_agent_id: None,
        }
    }

    fn local(x: f64, y: f64, z: f64) -> Position {
        Position::Local(LocalPosition {
            x_m: x,
            y_m: y,
            z_m: z,
        })
    }

    fn geo(lat: f64, lon: f64, alt: f64) -> Position {
        Position::Geo(GeoPosition {
            lat_deg: lat,
            lon_deg: lon,
            alt_m: alt,
        })
    }

    // Happy-path

    #[test]
    fn valid_plan_passes() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![
                entry("c1", MissionCommand::Arm),
                entry("c2", MissionCommand::Takeoff { altitude_m: 5.0 }),
                entry(
                    "c3",
                    MissionCommand::Hold {
                        duration_secs: 10.0,
                    },
                ),
                entry("c4", MissionCommand::Land),
            ],
        );
        assert!(validate(&plan).is_ok());
    }

    // Takeoff altitude

    #[test]
    fn negative_takeoff_altitude_fails() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![entry("c1", MissionCommand::Takeoff { altitude_m: -5.0 })],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::InvalidTakeoffAltitude { .. })
        ));
    }

    #[test]
    fn zero_takeoff_altitude_fails() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![entry("c1", MissionCommand::Takeoff { altitude_m: 0.0 })],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::InvalidTakeoffAltitude { .. })
        ));
    }

    // Hold / LoiterTime duration

    #[test]
    fn zero_hold_duration_fails() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![entry("c1", MissionCommand::Hold { duration_secs: 0.0 })],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::InvalidDuration { .. })
        ));
    }

    #[test]
    fn negative_hold_duration_fails() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![entry(
                "c1",
                MissionCommand::Hold {
                    duration_secs: -1.0,
                },
            )],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::InvalidDuration { .. })
        ));
    }

    #[test]
    fn zero_loiter_duration_fails() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![entry(
                "c1",
                MissionCommand::LoiterTime { duration_secs: 0.0 },
            )],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::InvalidDuration { .. })
        ));
    }

    // Orbit

    #[test]
    fn zero_orbit_radius_fails() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![entry(
                "c1",
                MissionCommand::Orbit {
                    center: local(0.0, 0.0, 5.0),
                    radius_m: 0.0,
                    turns: 1.0,
                    direction: OrbitDirection::Clockwise,
                },
            )],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::InvalidOrbitRadius { .. })
        ));
    }

    #[test]
    fn negative_orbit_turns_fails() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![entry(
                "c1",
                MissionCommand::Orbit {
                    center: local(0.0, 0.0, 5.0),
                    radius_m: 10.0,
                    turns: -1.0,
                    direction: OrbitDirection::Clockwise,
                },
            )],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::InvalidOrbitTurns { .. })
        ));
    }

    // Non-finite coordinates

    #[test]
    fn nan_coordinate_in_go_to_fails() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![entry(
                "c1",
                MissionCommand::GoTo {
                    position: Position::Local(LocalPosition {
                        x_m: f64::NAN,
                        y_m: 0.0,
                        z_m: 0.0,
                    }),
                },
            )],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::NonFiniteCoordinate { .. })
        ));
    }

    #[test]
    fn infinite_coord_in_orbit_center_fails() {
        let plan = make_plan(
            CoordinateFrame::Wgs84,
            vec![entry(
                "c1",
                MissionCommand::Orbit {
                    center: geo(f64::INFINITY, 8.5, 50.0),
                    radius_m: 10.0,
                    turns: 1.0,
                    direction: OrbitDirection::Clockwise,
                },
            )],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::NonFiniteCoordinate { .. })
        ));
    }

    // Empty route

    #[test]
    fn empty_follow_route_fails() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![entry(
                "c1",
                MissionCommand::FollowRoute {
                    route_id: RouteId::from("r1".to_owned()),
                    waypoints: vec![],
                },
            )],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::EmptyRoute { .. })
        ));
    }

    // Duplicate command ids

    #[test]
    fn duplicate_command_ids_fail() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![
                entry("c1", MissionCommand::Arm),
                entry("c1", MissionCommand::Disarm),
            ],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::DuplicateCommandId(_))
        ));
    }

    // Ambiguous coordinate frame

    #[test]
    fn wgs84_frame_with_local_position_fails() {
        let plan = make_plan(
            CoordinateFrame::Wgs84,
            vec![entry(
                "c1",
                MissionCommand::GoTo {
                    position: local(1.0, 2.0, 5.0),
                },
            )],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::AmbiguousCoordinateFrame { .. })
        ));
    }

    #[test]
    fn local_ned_frame_with_geo_position_fails() {
        let plan = make_plan(
            CoordinateFrame::LocalNed,
            vec![entry(
                "c1",
                MissionCommand::GoTo {
                    position: geo(47.4, 8.5, 10.0),
                },
            )],
        );
        assert!(matches!(
            validate(&plan),
            Err(MissionIrError::AmbiguousCoordinateFrame { .. })
        ));
    }
}
