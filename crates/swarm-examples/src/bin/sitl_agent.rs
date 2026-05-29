use swarm_comms::{MockMavlinkTransport, Waypoint};
use swarm_examples::sitl_plan::{
    format_dry_run_plan, load_sitl_plan, validate_connection_string, SitlError, SitlMode, SitlPlan,
};

struct CliArgs {
    mode: SitlMode,
    scenario: String,
    agent_id: String,
}

fn parse_args() -> Result<CliArgs, SitlError> {
    let args: Vec<String> = std::env::args().collect();
    let mut mode: Option<SitlMode> = None;
    let mut scenario: Option<String> = None;
    let mut agent_id: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--mock" => {
                set_mode(&mut mode, SitlMode::Mock)?;
            }
            "--dry-run" => {
                set_mode(&mut mode, SitlMode::DryRun)?;
            }
            "--connection" => {
                i += 1;
                let addr = args
                    .get(i)
                    .ok_or(SitlError::MissingArgument {
                        name: "--connection <addr>",
                    })?
                    .clone();
                set_mode(&mut mode, SitlMode::Connection { addr })?;
            }
            "--scenario" => {
                i += 1;
                scenario = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument { name: "--scenario" })?
                        .clone(),
                );
            }
            "--agent-id" => {
                i += 1;
                agent_id = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument { name: "--agent-id" })?
                        .clone(),
                );
            }
            arg => {
                return Err(SitlError::UnknownArgument {
                    arg: arg.to_owned(),
                });
            }
        }
        i += 1;
    }

    Ok(CliArgs {
        mode: mode.ok_or(SitlError::MissingMode)?,
        scenario: scenario.ok_or(SitlError::MissingArgument { name: "--scenario" })?,
        agent_id: agent_id.ok_or(SitlError::MissingArgument { name: "--agent-id" })?,
    })
}

fn set_mode(mode: &mut Option<SitlMode>, next: SitlMode) -> Result<(), SitlError> {
    if mode.is_some() {
        return Err(SitlError::ConflictingModes);
    }
    *mode = Some(next);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        eprintln!(
            "usage: sitl_agent --mock|--dry-run|--connection <addr> --scenario <path> --agent-id <id>"
        );
        std::process::exit(1);
    }
}

fn run() -> Result<(), SitlError> {
    let cli = parse_args()?;
    let plan = load_sitl_plan(&cli.scenario, cli.agent_id.clone())?;

    match cli.mode {
        SitlMode::Mock => run_mock(&plan),
        SitlMode::DryRun => {
            print!("{}", format_dry_run_plan(&plan));
            Ok(())
        }
        SitlMode::Connection { addr } => run_connection(&plan, &addr),
    }
}

fn run_mock(plan: &SitlPlan) -> Result<(), SitlError> {
    let mut transport = MockMavlinkTransport::new();
    eprintln!(
        "SITL Agent: {} | {} waypoints | mock=true",
        plan.agent_id,
        plan.waypoints.len()
    );

    for waypoint in &plan.waypoints {
        let waypoint = Waypoint {
            x: waypoint.x,
            y: waypoint.y,
            z: waypoint.z,
            seq: waypoint.seq,
        };
        eprintln!(
            "WAYPOINT seq={} x={:.1} y={:.1} z={:.1}",
            waypoint.seq, waypoint.x, waypoint.y, waypoint.z
        );
        transport.send_waypoint(waypoint);
    }
    eprintln!("Mock mode: {} waypoints sent.", transport.waypoints().len());
    Ok(())
}

fn run_connection(plan: &SitlPlan, connection_string: &str) -> Result<(), SitlError> {
    validate_connection_string(connection_string)?;

    #[cfg(feature = "mavlink-transport")]
    {
        use swarm_comms::{MavlinkTransport, MissionUploadOptions};

        let agent_id = swarm_types::AgentId::from(plan.agent_id.clone());
        let mut transport =
            MavlinkTransport::new(connection_string, agent_id).map_err(|error| {
                SitlError::ConnectionFailed {
                    message: error.to_string(),
                }
            })?;
        let waypoints: Vec<Waypoint> = plan
            .waypoints
            .iter()
            .map(|waypoint| Waypoint {
                x: waypoint.x,
                y: waypoint.y,
                z: waypoint.z,
                seq: waypoint.seq,
            })
            .collect();
        for waypoint in &waypoints {
            eprintln!(
                "WAYPOINT seq={} x={:.1} y={:.1} z={:.1}",
                waypoint.seq, waypoint.x, waypoint.y, waypoint.z
            );
        }

        let report = transport
            .upload_mission(&waypoints, MissionUploadOptions::default())
            .map_err(|error| SitlError::ConnectionFailed {
                message: error.to_string(),
            })?;
        eprintln!(
            "Real MAVLink mode: mission accepted; uploaded_count={} target_system={} target_component={} cleared_existing={}",
            report.uploaded_count,
            report.target_system,
            report.target_component,
            report.cleared_existing
        );
        Ok(())
    }

    #[cfg(not(feature = "mavlink-transport"))]
    {
        let _ = plan;
        Err(SitlError::FeatureMissing {
            feature: "mavlink-transport",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_string_validation_accepts_udp() {
        validate_connection_string("udp:127.0.0.1:14550").unwrap();
    }

    #[test]
    fn connection_string_validation_rejects_unknown_scheme() {
        let error = validate_connection_string("bad").unwrap_err();
        assert!(matches!(error, SitlError::BadConnectionString { .. }));
    }
}
