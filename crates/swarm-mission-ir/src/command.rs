use serde::{Deserialize, Serialize};

use crate::{ids::RouteId, orbit::OrbitDirection, position::Position, waypoint::MissionWaypoint};

/// A hardware-agnostic mission command primitive.
///
/// This is the core IR type. It represents mission intent without encoding
/// MAVLink message fields, PX4/ArduPilot modes, or any backend-specific
/// serialisation. A backend compiler (M81+) translates a sequence of these
/// commands into a protocol-specific plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MissionCommand {
    /// Arm the vehicle motors.
    Arm,
    /// Disarm the vehicle motors.
    Disarm,
    /// Ascend to the given altitude and enter hover.
    Takeoff {
        /// Target altitude in metres (above the `AltitudeReference` in the plan).
        altitude_m: f64,
    },
    /// Hold the current position for the given duration.
    Hold {
        /// Hold duration in seconds (must be positive).
        duration_secs: f64,
    },
    /// Land at the current horizontal position.
    Land,
    /// Return to the launch / home position and land.
    ReturnToLaunch,
    /// Fly to a specific position.
    GoTo {
        /// Target position.
        position: Position,
    },
    /// Follow a named ordered sequence of waypoints.
    FollowRoute {
        /// Identifier for the route (used for logging and deconfliction).
        route_id: RouteId,
        /// Ordered waypoints defining the route (must be non-empty).
        waypoints: Vec<MissionWaypoint>,
    },
    /// Loiter at the current position for the given duration.
    LoiterTime {
        /// Loiter duration in seconds (must be positive).
        duration_secs: f64,
    },
    /// Perform a circular orbit around a centre point.
    Orbit {
        /// Centre of the orbit.
        center: Position,
        /// Orbit radius in metres (must be positive).
        radius_m: f64,
        /// Number of full turns to complete (must be positive).
        turns: f64,
        /// Direction of travel.
        direction: OrbitDirection,
    },
    /// Pause execution of the mission (vehicle holds position).
    Pause,
    /// Resume execution after a `Pause`.
    Resume,
    /// Abort the mission immediately.
    Abort,
}

impl MissionCommand {
    /// Returns the kebab-case kind name used for logging and metrics.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Arm => "arm",
            Self::Disarm => "disarm",
            Self::Takeoff { .. } => "takeoff",
            Self::Hold { .. } => "hold",
            Self::Land => "land",
            Self::ReturnToLaunch => "return_to_launch",
            Self::GoTo { .. } => "go_to",
            Self::FollowRoute { .. } => "follow_route",
            Self::LoiterTime { .. } => "loiter_time",
            Self::Orbit { .. } => "orbit",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Abort => "abort",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        orbit::OrbitDirection,
        position::{GeoPosition, LocalPosition, Position},
    };

    fn local(x: f64, y: f64, z: f64) -> Position {
        Position::Local(LocalPosition {
            x_m: x,
            y_m: y,
            z_m: z,
        })
    }

    fn roundtrip(cmd: MissionCommand) -> MissionCommand {
        serde_json::from_str(&serde_json::to_string(&cmd).unwrap()).unwrap()
    }

    #[test]
    fn arm_roundtrip() {
        assert_eq!(MissionCommand::Arm, roundtrip(MissionCommand::Arm));
    }

    #[test]
    fn disarm_roundtrip() {
        assert_eq!(MissionCommand::Disarm, roundtrip(MissionCommand::Disarm));
    }

    #[test]
    fn takeoff_roundtrip() {
        let cmd = MissionCommand::Takeoff { altitude_m: 10.0 };
        assert_eq!(cmd, roundtrip(cmd.clone()));
    }

    #[test]
    fn hold_roundtrip() {
        let cmd = MissionCommand::Hold { duration_secs: 5.0 };
        assert_eq!(cmd, roundtrip(cmd.clone()));
    }

    #[test]
    fn land_roundtrip() {
        assert_eq!(MissionCommand::Land, roundtrip(MissionCommand::Land));
    }

    #[test]
    fn return_to_launch_roundtrip() {
        assert_eq!(
            MissionCommand::ReturnToLaunch,
            roundtrip(MissionCommand::ReturnToLaunch)
        );
    }

    #[test]
    fn go_to_roundtrip() {
        let cmd = MissionCommand::GoTo {
            position: local(1.0, 2.0, 5.0),
        };
        assert_eq!(cmd, roundtrip(cmd.clone()));
    }

    #[test]
    fn follow_route_roundtrip() {
        let cmd = MissionCommand::FollowRoute {
            route_id: RouteId::from("r1".to_owned()),
            waypoints: vec![
                MissionWaypoint {
                    position: local(0.0, 0.0, 5.0),
                    acceptance_radius_m: None,
                },
                MissionWaypoint {
                    position: local(10.0, 0.0, 5.0),
                    acceptance_radius_m: None,
                },
            ],
        };
        assert_eq!(cmd, roundtrip(cmd.clone()));
    }

    #[test]
    fn loiter_time_roundtrip() {
        let cmd = MissionCommand::LoiterTime {
            duration_secs: 30.0,
        };
        assert_eq!(cmd, roundtrip(cmd.clone()));
    }

    #[test]
    fn orbit_roundtrip() {
        let cmd = MissionCommand::Orbit {
            center: local(5.0, 5.0, 5.0),
            radius_m: 10.0,
            turns: 2.0,
            direction: OrbitDirection::Clockwise,
        };
        assert_eq!(cmd, roundtrip(cmd.clone()));
    }

    #[test]
    fn pause_roundtrip() {
        assert_eq!(MissionCommand::Pause, roundtrip(MissionCommand::Pause));
    }

    #[test]
    fn resume_roundtrip() {
        assert_eq!(MissionCommand::Resume, roundtrip(MissionCommand::Resume));
    }

    #[test]
    fn abort_roundtrip() {
        assert_eq!(MissionCommand::Abort, roundtrip(MissionCommand::Abort));
    }

    #[test]
    fn follow_route_waypoint_order_preserved() {
        let waypoints: Vec<MissionWaypoint> = (0..5)
            .map(|i| MissionWaypoint {
                position: local(i as f64, 0.0, 5.0),
                acceptance_radius_m: None,
            })
            .collect();
        let cmd = MissionCommand::FollowRoute {
            route_id: RouteId::from("r".to_owned()),
            waypoints: waypoints.clone(),
        };
        let restored = roundtrip(cmd);
        if let MissionCommand::FollowRoute {
            waypoints: restored_wps,
            ..
        } = restored
        {
            assert_eq!(waypoints, restored_wps);
        } else {
            panic!("not a follow_route");
        }
    }

    #[test]
    fn orbit_with_geo_center() {
        let cmd = MissionCommand::Orbit {
            center: Position::Geo(GeoPosition {
                lat_deg: 47.4,
                lon_deg: 8.5,
                alt_m: 50.0,
            }),
            radius_m: 20.0,
            turns: 1.5,
            direction: OrbitDirection::CounterClockwise,
        };
        assert_eq!(cmd, roundtrip(cmd.clone()));
    }

    #[test]
    fn kind_names_are_correct() {
        let cases: &[(&str, MissionCommand)] = &[
            ("arm", MissionCommand::Arm),
            ("disarm", MissionCommand::Disarm),
            ("takeoff", MissionCommand::Takeoff { altitude_m: 5.0 }),
            ("hold", MissionCommand::Hold { duration_secs: 1.0 }),
            ("land", MissionCommand::Land),
            ("return_to_launch", MissionCommand::ReturnToLaunch),
            (
                "go_to",
                MissionCommand::GoTo {
                    position: local(0.0, 0.0, 0.0),
                },
            ),
            (
                "follow_route",
                MissionCommand::FollowRoute {
                    route_id: RouteId::from("r".to_owned()),
                    waypoints: vec![MissionWaypoint {
                        position: local(0.0, 0.0, 0.0),
                        acceptance_radius_m: None,
                    }],
                },
            ),
            (
                "loiter_time",
                MissionCommand::LoiterTime { duration_secs: 1.0 },
            ),
            (
                "orbit",
                MissionCommand::Orbit {
                    center: local(0.0, 0.0, 0.0),
                    radius_m: 1.0,
                    turns: 1.0,
                    direction: OrbitDirection::Clockwise,
                },
            ),
            ("pause", MissionCommand::Pause),
            ("resume", MissionCommand::Resume),
            ("abort", MissionCommand::Abort),
        ];
        for (expected, cmd) in cases {
            assert_eq!(
                *expected,
                cmd.kind_name(),
                "kind_name mismatch for {:?}",
                cmd
            );
        }
    }
}
