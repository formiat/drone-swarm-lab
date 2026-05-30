use std::path::Path;

use swarm_examples::sitl_multi_agent::{
    build_multi_agent_manifest, load_multi_agent_config, MultiAgentSitlManifest,
};
use swarm_examples::sitl_plan::{load_sitl_suite, SitlError};
use swarm_examples::sitl_supervisor::{
    run_mock_supervisor, SupervisorMetrics, SupervisorMockConfig,
};

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
    replay_log: Option<String>,
    fail_agent: Option<String>,
    fail_after_ticks: u64,
    heartbeat_timeout_ticks: Option<u64>,
    max_ticks: Option<u64>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        eprintln!(
            "usage: sitl_supervisor --dry-run|--mock --scenario <path> --config <path> [--manifest <path>] [--replay-log <path>] [--fail-agent <id>] [--fail-after-ticks N] [--heartbeat-timeout-ticks N] [--max-ticks N]"
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
            let mock_config = SupervisorMockConfig {
                scenario_path: cli.scenario.clone(),
                replay_log: cli.replay_log.clone(),
                fail_agent: cli.fail_agent.clone(),
                fail_after_ticks: cli.fail_after_ticks,
                heartbeat_timeout_ticks: cli.heartbeat_timeout_ticks,
                max_ticks: cli.max_ticks,
            };
            let _: SupervisorMetrics = run_mock_supervisor(&suite, &mock_config, &manifest)?;
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
    let mut replay_log = None;
    let mut fail_agent = None;
    let mut fail_after_ticks = 1;
    let mut heartbeat_timeout_ticks = None;
    let mut max_ticks = None;

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
            "--replay-log" => {
                i += 1;
                replay_log = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--replay-log",
                        })?
                        .clone(),
                );
            }
            "--fail-agent" => {
                i += 1;
                fail_agent = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--fail-agent",
                        })?
                        .clone(),
                );
            }
            "--fail-after-ticks" => {
                i += 1;
                fail_after_ticks = parse_u64_arg(
                    args.get(i).ok_or(SitlError::MissingArgument {
                        name: "--fail-after-ticks",
                    })?,
                    "--fail-after-ticks",
                )?;
            }
            "--heartbeat-timeout-ticks" => {
                i += 1;
                heartbeat_timeout_ticks = Some(parse_u64_arg(
                    args.get(i).ok_or(SitlError::MissingArgument {
                        name: "--heartbeat-timeout-ticks",
                    })?,
                    "--heartbeat-timeout-ticks",
                )?);
            }
            "--max-ticks" => {
                i += 1;
                max_ticks = Some(parse_u64_arg(
                    args.get(i).ok_or(SitlError::MissingArgument {
                        name: "--max-ticks",
                    })?,
                    "--max-ticks",
                )?);
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
        replay_log,
        fail_agent,
        fail_after_ticks,
        heartbeat_timeout_ticks,
        max_ticks,
    })
}

fn parse_u64_arg(value: &str, name: &'static str) -> Result<u64, SitlError> {
    value
        .parse::<u64>()
        .map_err(|_| SitlError::MultiAgentConfigInvalid {
            message: format!("invalid {name} value '{value}'"),
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
