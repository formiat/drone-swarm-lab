use std::path::Path;
use std::time::Duration;

use swarm_comms::{MockMavlinkTransport, Waypoint};
use swarm_examples::sitl_plan::{
    first_sitl_entry, format_dry_run_plan, load_sitl_suite, validate_connection_string, SitlError,
    SitlMode, SitlPlan,
};
use swarm_examples::sitl_safety::{load_sitl_safety_config, validate_pre_upload_safety};

struct CliArgs {
    mode: SitlMode,
    scenario: String,
    agent_id: String,
    safety_config: Option<String>,
    lifecycle: LifecycleArgs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LifecycleMode {
    UploadOnly,
    Execute,
}

struct LifecycleArgs {
    mode: LifecycleMode,
    no_arm: bool,
    abort_after: Option<Duration>,
    timeout: Duration,
}

fn parse_args() -> Result<CliArgs, SitlError> {
    let args: Vec<String> = std::env::args().collect();
    let mut mode: Option<SitlMode> = None;
    let mut scenario: Option<String> = None;
    let mut agent_id: Option<String> = None;
    let mut safety_config: Option<String> = None;
    let mut lifecycle_mode: Option<LifecycleMode> = None;
    let mut no_arm = false;
    let mut abort_after: Option<Duration> = None;
    let mut timeout = Duration::from_secs(2);
    let mut connection_only_option: Option<&'static str> = None;

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
            "--safety-config" => {
                i += 1;
                safety_config = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--safety-config <path>",
                        })?
                        .clone(),
                );
            }
            "--upload-only" => {
                set_lifecycle_mode(&mut lifecycle_mode, LifecycleMode::UploadOnly)?;
                connection_only_option.get_or_insert("--upload-only");
            }
            "--execute" => {
                set_lifecycle_mode(&mut lifecycle_mode, LifecycleMode::Execute)?;
                connection_only_option.get_or_insert("--execute");
            }
            "--no-arm" => {
                no_arm = true;
                connection_only_option.get_or_insert("--no-arm");
            }
            "--abort-after" => {
                i += 1;
                let value = args.get(i).ok_or(SitlError::MissingArgument {
                    name: "--abort-after <seconds>",
                })?;
                abort_after = Some(parse_duration_arg("--abort-after", value, true)?);
                connection_only_option.get_or_insert("--abort-after");
            }
            "--timeout" => {
                i += 1;
                let value = args.get(i).ok_or(SitlError::MissingArgument {
                    name: "--timeout <seconds>",
                })?;
                timeout = parse_duration_arg("--timeout", value, false)?;
                connection_only_option.get_or_insert("--timeout");
            }
            arg => {
                return Err(SitlError::UnknownArgument {
                    arg: arg.to_owned(),
                });
            }
        }
        i += 1;
    }

    let mode = mode.ok_or(SitlError::MissingMode)?;
    let lifecycle_mode = lifecycle_mode.unwrap_or(LifecycleMode::UploadOnly);
    if !matches!(mode, SitlMode::Connection { .. }) {
        if let Some(option) = connection_only_option {
            return Err(SitlError::LifecycleOptionRequiresConnection { option });
        }
    }
    if no_arm && lifecycle_mode != LifecycleMode::Execute {
        return Err(SitlError::LifecycleOptionRequiresExecute { option: "--no-arm" });
    }
    if abort_after.is_some() && lifecycle_mode != LifecycleMode::Execute {
        return Err(SitlError::LifecycleOptionRequiresExecute {
            option: "--abort-after",
        });
    }

    Ok(CliArgs {
        mode,
        scenario: scenario.ok_or(SitlError::MissingArgument { name: "--scenario" })?,
        agent_id: agent_id.ok_or(SitlError::MissingArgument { name: "--agent-id" })?,
        safety_config,
        lifecycle: LifecycleArgs {
            mode: lifecycle_mode,
            no_arm,
            abort_after,
            timeout,
        },
    })
}

fn set_mode(mode: &mut Option<SitlMode>, next: SitlMode) -> Result<(), SitlError> {
    if mode.is_some() {
        return Err(SitlError::ConflictingModes);
    }
    *mode = Some(next);
    Ok(())
}

fn set_lifecycle_mode(
    mode: &mut Option<LifecycleMode>,
    next: LifecycleMode,
) -> Result<(), SitlError> {
    if mode.is_some() {
        return Err(SitlError::ConflictingLifecycleModes);
    }
    *mode = Some(next);
    Ok(())
}

fn parse_duration_arg(
    name: &'static str,
    value: &str,
    allow_zero: bool,
) -> Result<Duration, SitlError> {
    let seconds = value
        .parse::<f64>()
        .map_err(|_| SitlError::InvalidDuration {
            name,
            value: value.to_owned(),
        })?;
    if !seconds.is_finite() || seconds < 0.0 || (!allow_zero && seconds == 0.0) {
        return Err(SitlError::InvalidDuration {
            name,
            value: value.to_owned(),
        });
    }
    Duration::try_from_secs_f64(seconds).map_err(|_| SitlError::InvalidDuration {
        name,
        value: value.to_owned(),
    })
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        eprintln!(
            "usage: sitl_agent --mock|--dry-run|--connection <addr> --scenario <path> --agent-id <id> [--safety-config <path>] [--upload-only|--execute] [--no-arm] [--abort-after <seconds>] [--timeout <seconds>]"
        );
        std::process::exit(1);
    }
}

fn run() -> Result<(), SitlError> {
    let cli = parse_args()?;
    let suite = load_sitl_suite(&cli.scenario)?;

    if let SitlMode::Connection { addr } = &cli.mode {
        validate_connection_string(addr)?;
        let safety_config = load_sitl_safety_config(cli.safety_config.as_deref().map(Path::new))?;
        let entry = first_sitl_entry(&suite, &cli.scenario)?;
        validate_pre_upload_safety(entry, &cli.agent_id, &safety_config)?;
    }

    let plan = swarm_examples::sitl_plan::build_sitl_plan(&suite, &cli.scenario, cli.agent_id)?;

    match cli.mode {
        SitlMode::Mock => run_mock(&plan),
        SitlMode::DryRun => {
            print!("{}", format_dry_run_plan(&plan));
            Ok(())
        }
        SitlMode::Connection { addr } => run_connection(&plan, &addr, &cli.lifecycle),
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

fn run_connection(
    plan: &SitlPlan,
    connection_string: &str,
    lifecycle: &LifecycleArgs,
) -> Result<(), SitlError> {
    validate_connection_string(connection_string)?;

    #[cfg(feature = "mavlink-transport")]
    {
        use swarm_comms::{MavlinkTransport, MissionLifecycleOptions, MissionUploadOptions};

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

        let upload_options = MissionUploadOptions {
            timeout: lifecycle.timeout,
            ..MissionUploadOptions::default()
        };
        match lifecycle.mode {
            LifecycleMode::UploadOnly => {
                let report = transport
                    .upload_mission(&waypoints, upload_options)
                    .map_err(|error| SitlError::ConnectionFailed {
                        message: error.to_string(),
                    })?;
                eprintln!(
                    "Real MAVLink mode: mission accepted; lifecycle=upload-only uploaded_count={} target_system={} target_component={} cleared_existing={}",
                    report.uploaded_count,
                    report.target_system,
                    report.target_component,
                    report.cleared_existing
                );
            }
            LifecycleMode::Execute => {
                let lifecycle_options = MissionLifecycleOptions {
                    target_system: upload_options.target_system,
                    target_component: upload_options.target_component,
                    timeout: lifecycle.timeout,
                    no_arm: lifecycle.no_arm,
                    abort_after: lifecycle.abort_after,
                    takeoff_altitude_m: default_takeoff_altitude(&waypoints),
                };
                let report = transport
                    .upload_and_execute_mission(&waypoints, upload_options, lifecycle_options)
                    .map_err(|error| SitlError::ConnectionFailed {
                        message: error.to_string(),
                    })?;
                eprintln!(
                    "Real MAVLink mode: mission executed; uploaded_count={} armed={} took_off={} started={} post_start_heartbeat={} abort_result={:?}",
                    report.upload.uploaded_count,
                    report.lifecycle.armed,
                    report.lifecycle.took_off,
                    report.lifecycle.started,
                    report.lifecycle.post_start_heartbeat,
                    report.lifecycle.abort_result
                );
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "mavlink-transport"))]
    {
        let _ = plan;
        let _ = (
            lifecycle.mode,
            lifecycle.no_arm,
            lifecycle.abort_after,
            lifecycle.timeout,
        );
        Err(SitlError::FeatureMissing {
            feature: "mavlink-transport",
        })
    }
}

#[cfg(feature = "mavlink-transport")]
fn default_takeoff_altitude(waypoints: &[Waypoint]) -> f32 {
    waypoints
        .first()
        .map(|waypoint| waypoint.z.max(2.5) as f32)
        .unwrap_or(2.5)
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
