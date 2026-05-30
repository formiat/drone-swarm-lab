use std::path::Path;
use std::thread;
use std::time::Duration;

use swarm_comms::{MockMavlinkTransport, Waypoint};
use swarm_examples::sitl_multi_agent::{
    build_multi_agent_manifest, load_multi_agent_config, MultiAgentSitlManifest,
};
use swarm_examples::sitl_plan::{load_sitl_suite, SitlError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SupervisorMode {
    DryRun,
    Mock,
}

struct CliArgs {
    mode: SupervisorMode,
    scenario: String,
    config: String,
    manifest: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        eprintln!(
            "usage: sitl_supervisor --dry-run|--mock --scenario <path> --config <path> [--manifest <path>]"
        );
        std::process::exit(1);
    }
}

fn run() -> Result<(), SitlError> {
    let cli = parse_args()?;
    let suite = load_sitl_suite(&cli.scenario)?;
    let config = load_multi_agent_config(&cli.config)?;
    let manifest = build_multi_agent_manifest(&suite, &cli.scenario, &cli.config, &config)?;

    match cli.mode {
        SupervisorMode::DryRun => {
            write_or_print_manifest(cli.manifest.as_deref(), &manifest)?;
        }
        SupervisorMode::Mock => {
            run_mock_supervisor(&manifest);
            write_or_print_manifest(cli.manifest.as_deref(), &manifest)?;
        }
    }
    Ok(())
}

fn parse_args() -> Result<CliArgs, SitlError> {
    let args: Vec<String> = std::env::args().collect();
    let mut mode = None;
    let mut scenario = None;
    let mut config = None;
    let mut manifest = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dry-run" => set_mode(&mut mode, SupervisorMode::DryRun)?,
            "--mock" => set_mode(&mut mode, SupervisorMode::Mock)?,
            "--scenario" => {
                i += 1;
                scenario = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument { name: "--scenario" })?
                        .clone(),
                );
            }
            "--config" => {
                i += 1;
                config = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument { name: "--config" })?
                        .clone(),
                );
            }
            "--manifest" => {
                i += 1;
                manifest = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument { name: "--manifest" })?
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
        config: config.ok_or(SitlError::MissingArgument { name: "--config" })?,
        manifest,
    })
}

fn set_mode(mode: &mut Option<SupervisorMode>, next: SupervisorMode) -> Result<(), SitlError> {
    if mode.is_some() {
        return Err(SitlError::ConflictingModes);
    }
    *mode = Some(next);
    Ok(())
}

fn write_or_print_manifest(
    manifest_path: Option<&str>,
    manifest: &MultiAgentSitlManifest,
) -> Result<(), SitlError> {
    let json = serde_json::to_string_pretty(manifest).map_err(|error| {
        SitlError::MultiAgentConfigInvalid {
            message: error.to_string(),
        }
    })?;
    let Some(path) = manifest_path else {
        println!("{json}");
        return Ok(());
    };
    let path = Path::new(path);
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| SitlError::MultiAgentManifestWrite {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    std::fs::write(path, json).map_err(|error| SitlError::MultiAgentManifestWrite {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    eprintln!("Multi-agent SITL manifest written: {}", path.display());
    Ok(())
}

fn run_mock_supervisor(manifest: &MultiAgentSitlManifest) {
    eprintln!(
        "Multi-Agent SITL Foundation: mock agents={} assigned_tasks={} unassigned_pose_tasks={}",
        manifest.agents_count,
        manifest.ownership_summary.assigned_task_count,
        manifest.ownership_summary.unassigned_pose_tasks.len()
    );
    for agent in &manifest.agents {
        if agent.start_delay_ms > 0 {
            thread::sleep(Duration::from_millis(agent.start_delay_ms));
        }
        let mut transport = MockMavlinkTransport::new();
        eprintln!(
            "SITL Supervisor: agent={} system_id={} component_id={} connection={} waypoints={}",
            agent.agent_id,
            agent.system_id,
            agent.component_id,
            agent.connection_string,
            agent.waypoint_count
        );
        for waypoint in &agent.waypoints {
            transport.send_waypoint(Waypoint {
                x: waypoint.x,
                y: waypoint.y,
                z: waypoint.z,
                seq: waypoint.seq,
            });
            eprintln!(
                "WAYPOINT agent={} seq={} task_id={} x={:.1} y={:.1} z={:.1}",
                agent.agent_id, waypoint.seq, waypoint.task_id, waypoint.x, waypoint.y, waypoint.z
            );
        }
        eprintln!(
            "Mock mode: agent={} waypoints sent={}",
            agent.agent_id,
            transport.waypoints().len()
        );
    }
}
