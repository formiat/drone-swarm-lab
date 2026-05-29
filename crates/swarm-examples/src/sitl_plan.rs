use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use swarm_sim::{validate_scenario_suite, ScenarioSuite, ScenarioSuiteEntry};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SitlMode {
    Mock,
    DryRun,
    Connection { addr: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SitlCoordinateFrame {
    LocalSimulation,
}

impl SitlCoordinateFrame {
    pub fn name(self) -> &'static str {
        match self {
            Self::LocalSimulation => "local_simulation",
        }
    }

    pub fn from_name(name: &str) -> Result<Self, SitlError> {
        match name {
            "local_simulation" => Ok(Self::LocalSimulation),
            other => Err(SitlError::UnsupportedCoordinateFrame {
                frame: other.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SitlWaypointItem {
    pub seq: u16,
    pub task_id: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SitlPlan {
    pub agent_id: String,
    pub scenario_path: PathBuf,
    pub suite_name: String,
    pub scenario_name: String,
    pub mission: String,
    pub profile: String,
    pub coordinate_frame: SitlCoordinateFrame,
    pub altitude_source: String,
    pub waypoints: Vec<SitlWaypointItem>,
}

#[derive(Debug, thiserror::Error)]
pub enum SitlError {
    #[error("invalid scenario {path:?}: {message}")]
    InvalidScenario { path: PathBuf, message: String },
    #[error("no pose tasks found in scenario '{scenario_name}'")]
    NoPoseTasks { scenario_name: String },
    #[error(
        "feature missing: --connection requires feature '{feature}'. Build with: cargo build --bin sitl_agent --features {feature}"
    )]
    FeatureMissing { feature: &'static str },
    #[error(
        "bad connection string '{addr}': expected udp:<host>:<port>, tcp:<host>:<port>, or serial:<path>:<baud>"
    )]
    BadConnectionString { addr: String },
    #[error("unsupported coordinate frame '{frame}'")]
    UnsupportedCoordinateFrame { frame: String },
    #[error("connection failed: {message}")]
    ConnectionFailed { message: String },
    #[error("safety config read failed {path:?}: {message}")]
    SafetyConfigRead { path: PathBuf, message: String },
    #[error("safety config parse failed {path:?}: {message}")]
    SafetyConfigParse { path: PathBuf, message: String },
    #[error("safety config invalid {field}: {message}")]
    SafetyConfigInvalid { field: String, message: String },
    #[error("safety validation failed: {message}")]
    SafetyValidationFailed { message: String },
    #[error("missing SITL mode: specify exactly one of --mock, --dry-run, or --connection <addr>")]
    MissingMode,
    #[error(
        "conflicting SITL modes: specify exactly one of --mock, --dry-run, or --connection <addr>"
    )]
    ConflictingModes,
    #[error("missing required argument: {name}")]
    MissingArgument { name: &'static str },
    #[error("unknown argument: {arg}")]
    UnknownArgument { arg: String },
}

pub fn validate_connection_string(addr: &str) -> Result<(), SitlError> {
    let addr = addr.trim();
    let Some((scheme, rest)) = addr.split_once(':') else {
        return bad_connection_string(addr);
    };

    match scheme {
        "udp" | "tcp" => validate_host_port(addr, rest),
        "serial" => validate_serial(addr, rest),
        _ => bad_connection_string(addr),
    }
}

fn validate_host_port(addr: &str, rest: &str) -> Result<(), SitlError> {
    let Some((host, port)) = rest.rsplit_once(':') else {
        return bad_connection_string(addr);
    };
    if host.trim().is_empty() || port.trim().parse::<u16>().is_err() {
        return bad_connection_string(addr);
    }
    Ok(())
}

fn validate_serial(addr: &str, rest: &str) -> Result<(), SitlError> {
    let Some((path, baud)) = rest.rsplit_once(':') else {
        return bad_connection_string(addr);
    };
    if path.trim().is_empty() || baud.trim().parse::<u32>().is_err() {
        return bad_connection_string(addr);
    }
    Ok(())
}

fn bad_connection_string<T>(addr: &str) -> Result<T, SitlError> {
    Err(SitlError::BadConnectionString {
        addr: addr.to_owned(),
    })
}

pub fn load_sitl_plan(
    scenario_path: impl AsRef<Path>,
    agent_id: impl Into<String>,
) -> Result<SitlPlan, SitlError> {
    let scenario_path = scenario_path.as_ref();
    let suite = load_sitl_suite(scenario_path)?;
    build_sitl_plan(&suite, scenario_path, agent_id)
}

pub fn load_sitl_suite(scenario_path: impl AsRef<Path>) -> Result<ScenarioSuite, SitlError> {
    let scenario_path = scenario_path.as_ref();
    let scenario_path_string =
        scenario_path
            .to_str()
            .ok_or_else(|| SitlError::InvalidScenario {
                path: scenario_path.to_path_buf(),
                message: "scenario path is not valid UTF-8".to_owned(),
            })?;
    swarm_sim::load_scenario_suite(scenario_path_string).map_err(|error| {
        SitlError::InvalidScenario {
            path: scenario_path.to_path_buf(),
            message: error.to_string(),
        }
    })
}

pub fn first_sitl_entry(
    suite: &ScenarioSuite,
    scenario_path: impl AsRef<Path>,
) -> Result<&ScenarioSuiteEntry, SitlError> {
    suite
        .scenarios
        .first()
        .ok_or_else(|| SitlError::InvalidScenario {
            path: scenario_path.as_ref().to_path_buf(),
            message: "Scenario suite must contain at least one scenario".to_owned(),
        })
}

pub fn build_sitl_plan(
    suite: &ScenarioSuite,
    scenario_path: impl AsRef<Path>,
    agent_id: impl Into<String>,
) -> Result<SitlPlan, SitlError> {
    let scenario_path = scenario_path.as_ref().to_path_buf();
    let validation_errors = validate_scenario_suite(suite);
    let entry = first_sitl_entry(suite, &scenario_path)?;

    let waypoints: Vec<SitlWaypointItem> = entry
        .scenario
        .tasks
        .iter()
        .filter_map(|task| {
            let pose = task.pose?;
            Some((task, pose))
        })
        .enumerate()
        .map(|(seq, (task, pose))| SitlWaypointItem {
            seq: seq as u16,
            task_id: task.id.to_string(),
            x: pose.x,
            y: pose.y,
            z: pose.z,
        })
        .collect();

    if waypoints.is_empty() {
        return Err(SitlError::NoPoseTasks {
            scenario_name: entry.scenario.name.clone(),
        });
    }

    if !validation_errors.is_empty() {
        let message = validation_errors
            .iter()
            .map(|error| {
                let field = &error.field;
                let message = &error.message;
                format!("{field}: {message}")
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(SitlError::InvalidScenario {
            path: scenario_path,
            message,
        });
    }

    Ok(SitlPlan {
        agent_id: agent_id.into(),
        scenario_path,
        suite_name: suite.name.clone(),
        scenario_name: entry.scenario.name.clone(),
        mission: entry.mission.clone(),
        profile: entry.profile.clone(),
        coordinate_frame: SitlCoordinateFrame::LocalSimulation,
        altitude_source: "pose.z (serde default 0.0 when omitted)".to_owned(),
        waypoints,
    })
}

pub fn format_dry_run_plan(plan: &SitlPlan) -> String {
    let mut output = String::new();
    writeln!(output, "mode: dry-run").unwrap();
    writeln!(output, "agent_id: {}", plan.agent_id).unwrap();
    writeln!(output, "scenario_path: {}", plan.scenario_path.display()).unwrap();
    writeln!(output, "suite_name: {}", plan.suite_name).unwrap();
    writeln!(output, "scenario_name: {}", plan.scenario_name).unwrap();
    writeln!(output, "mission: {}", plan.mission).unwrap();
    writeln!(output, "profile: {}", plan.profile).unwrap();
    writeln!(output, "coordinate_frame: {}", plan.coordinate_frame.name()).unwrap();
    writeln!(output, "altitude_source: {}", plan.altitude_source).unwrap();
    writeln!(
        output,
        "limitations: x/y are local simulation coordinates, not WGS84 latitude/longitude; dry-run does not upload to PX4"
    )
    .unwrap();
    writeln!(output, "waypoints:").unwrap();
    for waypoint in &plan.waypoints {
        writeln!(
            output,
            "  seq={} task_id={} x={:.3} y={:.3} z={:.3}",
            waypoint.seq, waypoint.task_id, waypoint.x, waypoint.y, waypoint.z
        )
        .unwrap();
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_sim::{RunConfig, Scenario, ScenarioSuiteEntry};
    use swarm_types::{Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskStatus};

    fn agent() -> Agent {
        Agent {
            id: AgentId::from("agent-0".to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose::default(),
            capabilities: vec![],
            current_task: None,
            battery: 100.0,
            comms_range: 1000.0,
            generation: 1,
            speed: 0.0,
            max_range: 1000.0,
            battery_drain_rate: 0.0,
            battery_model: None,
        }
    }

    fn task(id: &str, pose: Option<Pose>) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose,
            grid_cell: None,
            edge_id: None,
            kind: None,
        }
    }

    fn suite(tasks: Vec<Task>) -> ScenarioSuite {
        ScenarioSuite {
            schema_version: "0.1".to_owned(),
            name: "SITL Waypoints".to_owned(),
            description: "test suite".to_owned(),
            scenarios: vec![ScenarioSuiteEntry {
                mission: "sitl".to_owned(),
                profile: "waypoints".to_owned(),
                scenario: Scenario {
                    name: "sitl_waypoints_0".to_owned(),
                    seed: 0,
                    agents: vec![agent()],
                    tasks,
                    ground_nodes: vec![],
                    base_station: None,
                },
                run_config: RunConfig {
                    max_ticks: 50,
                    ..Default::default()
                },
            }],
        }
    }

    #[test]
    fn helper_extracts_pose_tasks_with_sequential_ids() {
        let suite = suite(vec![
            task(
                "wp-0",
                Some(Pose {
                    x: 10.0,
                    y: 20.0,
                    z: 3.0,
                }),
            ),
            task("no-pose", None),
            task(
                "wp-1",
                Some(Pose {
                    x: 30.0,
                    y: 40.0,
                    z: 5.0,
                }),
            ),
        ]);
        let plan = build_sitl_plan(&suite, "scenario.json", "agent-0").unwrap();

        assert_eq!(plan.waypoints.len(), 2);
        assert_eq!(plan.waypoints[0].seq, 0);
        assert_eq!(plan.waypoints[0].task_id, "wp-0");
        assert_eq!(plan.waypoints[0].z, 3.0);
        assert_eq!(plan.waypoints[1].seq, 1);
        assert_eq!(plan.waypoints[1].task_id, "wp-1");
        assert_eq!(plan.waypoints[1].x, 30.0);
    }

    #[test]
    fn helper_returns_no_pose_tasks_error() {
        let suite = suite(vec![task("no-pose", None)]);
        let error = build_sitl_plan(&suite, "scenario.json", "agent-0").unwrap_err();

        assert!(matches!(error, SitlError::NoPoseTasks { .. }));
    }

    #[test]
    fn helper_returns_invalid_scenario_error() {
        let suite = ScenarioSuite {
            schema_version: "0.1".to_owned(),
            name: String::new(),
            description: "test suite".to_owned(),
            scenarios: vec![],
        };
        let error = build_sitl_plan(&suite, "scenario.json", "agent-0").unwrap_err();

        assert!(matches!(error, SitlError::InvalidScenario { .. }));
    }

    #[test]
    fn dry_run_format_contains_contract_fields() {
        let suite = suite(vec![task(
            "wp-0",
            Some(Pose {
                x: 10.0,
                y: 20.0,
                z: 0.0,
            }),
        )]);
        let plan = build_sitl_plan(&suite, "scenario.json", "agent-0").unwrap();
        let output = format_dry_run_plan(&plan);

        assert!(output.contains("mode: dry-run"));
        assert!(output.contains("agent_id: agent-0"));
        assert!(output.contains("scenario_path: scenario.json"));
        assert!(output.contains("suite_name: SITL Waypoints"));
        assert!(output.contains("scenario_name: sitl_waypoints_0"));
        assert!(output.contains("mission: sitl"));
        assert!(output.contains("profile: waypoints"));
        assert!(output.contains("coordinate_frame: local_simulation"));
        assert!(output.contains("altitude_source: pose.z"));
        assert!(output.contains("seq=0 task_id=wp-0 x=10.000 y=20.000 z=0.000"));
    }

    #[test]
    fn unsupported_coordinate_frame_is_typed_error() {
        let error = SitlCoordinateFrame::from_name("global").unwrap_err();

        assert!(matches!(
            error,
            SitlError::UnsupportedCoordinateFrame { frame } if frame == "global"
        ));
    }

    #[test]
    fn bad_connection_string_is_typed_error() {
        let error = validate_connection_string("not-a-connection").unwrap_err();

        assert!(matches!(error, SitlError::BadConnectionString { .. }));
    }

    #[test]
    fn udp_connection_requires_host_and_port() {
        for addr in [
            "udp:",
            "udp:127.0.0.1",
            "udp::14550",
            "udp:127.0.0.1:notaport",
        ] {
            let error = validate_connection_string(addr).unwrap_err();
            assert!(matches!(error, SitlError::BadConnectionString { .. }));
        }
    }

    #[test]
    fn tcp_connection_requires_host_and_port() {
        for addr in [
            "tcp:",
            "tcp:localhost",
            "tcp::5760",
            "tcp:localhost:notaport",
        ] {
            let error = validate_connection_string(addr).unwrap_err();
            assert!(matches!(error, SitlError::BadConnectionString { .. }));
        }
    }

    #[test]
    fn serial_connection_requires_path_and_baud() {
        for addr in [
            "serial:",
            "serial:/dev/ttyUSB0",
            "serial::57600",
            "serial:/dev/ttyUSB0:fast",
        ] {
            let error = validate_connection_string(addr).unwrap_err();
            assert!(matches!(error, SitlError::BadConnectionString { .. }));
        }
    }

    #[test]
    fn supported_connection_strings_are_valid() {
        validate_connection_string("udp:127.0.0.1:14550").unwrap();
        validate_connection_string("tcp:localhost:5760").unwrap();
        validate_connection_string("serial:/dev/ttyUSB0:57600").unwrap();
    }
}
