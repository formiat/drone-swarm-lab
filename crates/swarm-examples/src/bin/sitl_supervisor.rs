use std::path::Path;
use std::time::Duration;

use swarm_examples::sitl_connection::SitlConnectionLifecycle;
use swarm_examples::sitl_multi_agent::{
    build_multi_agent_manifest, load_multi_agent_config, MultiAgentSitlManifest,
};
use swarm_examples::sitl_plan::{load_sitl_suite, SitlError};
use swarm_examples::sitl_supervisor::{
    run_live_supervisor, run_mock_supervisor, SupervisorLiveConfig, SupervisorMetrics,
    SupervisorMockConfig,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SupervisorMode {
    DryRun,
    Mock,
    Connection,
}

struct CliArgs {
    mode: SupervisorMode,
    scenario: String,
    config: String,
    manifest: Option<String>,
    replay_log: Option<String>,
    run_report: Option<String>,
    safety_config: Option<String>,
    fail_agent: Option<String>,
    fail_after_ticks: u64,
    heartbeat_timeout_ticks: Option<u64>,
    max_ticks: Option<u64>,
    timeout: Duration,
    telemetry_timeout: Duration,
    no_progress_timeout: Duration,
    no_arm: bool,
    abort_after: Option<Duration>,
    allow_hardware_candidate: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        eprintln!(
            "usage: sitl_supervisor --dry-run|--mock|--connection --scenario <path> --config <path> [--manifest <path>] [--replay-log <path>] [--fail-agent <id>] [--fail-after-ticks N] [--heartbeat-timeout-ticks N] [--max-ticks N] [--execute] [--run-report <path>] [--safety-config <path>] [--timeout <duration>] [--telemetry-timeout <duration>] [--no-progress-timeout <duration>] [--no-arm] [--abort-after <duration>] [--allow-hardware-candidate]"
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
        SupervisorMode::Connection => {
            let live_config = SupervisorLiveConfig {
                scenario_path: cli.scenario.clone(),
                config_path: cli.config.clone(),
                safety_config_path: cli.safety_config.clone(),
                replay_log: cli.replay_log.clone(),
                run_report: cli.run_report.clone(),
                lifecycle: SitlConnectionLifecycle {
                    timeout: cli.timeout,
                    telemetry_timeout: cli.telemetry_timeout,
                    no_progress_timeout: cli.no_progress_timeout,
                    no_arm: cli.no_arm,
                    abort_after: cli.abort_after,
                },
                allow_hardware_candidate: cli.allow_hardware_candidate,
                run_id: None,
            };
            let _ = run_live_supervisor(&suite, &live_config, &manifest)?;
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
    let mut run_report = None;
    let mut safety_config = None;
    let mut fail_agent = None;
    let mut fail_after_ticks = 1;
    let mut heartbeat_timeout_ticks = None;
    let mut max_ticks = None;
    let mut execute = false;
    let mut timeout = Duration::from_secs(2);
    let mut timeout_set = false;
    let mut telemetry_timeout = Duration::from_secs(10);
    let mut telemetry_timeout_set = false;
    let mut no_progress_timeout = Duration::from_secs(60);
    let mut no_progress_timeout_set = false;
    let mut no_arm = false;
    let mut abort_after = None;
    let mut allow_hardware_candidate = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dry-run" => set_mode(&mut mode, SupervisorMode::DryRun)?,
            "--mock" => set_mode(&mut mode, SupervisorMode::Mock)?,
            "--connection" => set_mode(&mut mode, SupervisorMode::Connection)?,
            "--execute" => execute = true,
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
            "--run-report" => {
                i += 1;
                run_report = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--run-report",
                        })?
                        .clone(),
                );
            }
            "--safety-config" => {
                i += 1;
                safety_config = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--safety-config",
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
            "--timeout" => {
                i += 1;
                timeout_set = true;
                timeout = parse_duration_arg(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument { name: "--timeout" })?,
                    "--timeout",
                )?;
            }
            "--telemetry-timeout" => {
                i += 1;
                telemetry_timeout_set = true;
                telemetry_timeout = parse_duration_arg(
                    args.get(i).ok_or(SitlError::MissingArgument {
                        name: "--telemetry-timeout",
                    })?,
                    "--telemetry-timeout",
                )?;
            }
            "--no-progress-timeout" => {
                i += 1;
                no_progress_timeout_set = true;
                no_progress_timeout = parse_duration_arg(
                    args.get(i).ok_or(SitlError::MissingArgument {
                        name: "--no-progress-timeout",
                    })?,
                    "--no-progress-timeout",
                )?;
            }
            "--no-arm" => no_arm = true,
            "--abort-after" => {
                i += 1;
                abort_after = Some(parse_duration_arg(
                    args.get(i).ok_or(SitlError::MissingArgument {
                        name: "--abort-after",
                    })?,
                    "--abort-after",
                )?);
            }
            "--allow-hardware-candidate" => allow_hardware_candidate = true,
            arg => {
                return Err(SitlError::UnknownArgument {
                    arg: arg.to_owned(),
                });
            }
        }
        i += 1;
    }

    let mode = mode.ok_or(SitlError::MissingMode)?;
    validate_cli_arg_combinations(CliValidationArgs {
        mode,
        execute,
        run_report: run_report.as_deref(),
        safety_config: safety_config.as_deref(),
        fail_agent: fail_agent.as_deref(),
        heartbeat_timeout_ticks,
        max_ticks,
        live_options: LiveOptionFlags {
            timeout_set,
            telemetry_timeout_set,
            no_progress_timeout_set,
            no_arm,
            abort_after_set: abort_after.is_some(),
        },
    })?;

    Ok(CliArgs {
        mode,
        scenario: scenario.ok_or(SitlError::MissingArgument { name: "--scenario" })?,
        config: config.ok_or(SitlError::MissingArgument { name: "--config" })?,
        manifest,
        replay_log,
        run_report,
        safety_config,
        fail_agent,
        fail_after_ticks,
        heartbeat_timeout_ticks,
        max_ticks,
        timeout,
        telemetry_timeout,
        no_progress_timeout,
        no_arm,
        abort_after,
        allow_hardware_candidate,
    })
}

fn parse_u64_arg(value: &str, name: &'static str) -> Result<u64, SitlError> {
    value
        .parse::<u64>()
        .map_err(|_| SitlError::MultiAgentConfigInvalid {
            message: format!("invalid {name} value '{value}'"),
        })
}

fn parse_duration_arg(value: &str, name: &'static str) -> Result<Duration, SitlError> {
    let seconds = value
        .parse::<u64>()
        .map_err(|_| SitlError::InvalidDuration {
            name,
            value: value.to_owned(),
        })?;
    if seconds == 0 {
        return Err(SitlError::InvalidDuration {
            name,
            value: value.to_owned(),
        });
    }
    Ok(Duration::from_secs(seconds))
}

fn set_mode(mode: &mut Option<SupervisorMode>, next: SupervisorMode) -> Result<(), SitlError> {
    if mode.is_some() {
        return Err(SitlError::ConflictingModes);
    }
    *mode = Some(next);
    Ok(())
}

fn validate_cli_arg_combinations(args: CliValidationArgs<'_>) -> Result<(), SitlError> {
    if args.execute && args.mode != SupervisorMode::Connection {
        return Err(SitlError::LifecycleOptionRequiresConnection {
            option: "--execute",
        });
    }
    if args.run_report.is_some() && !(args.mode == SupervisorMode::Connection && args.execute) {
        return Err(SitlError::RunReportRequiresExecute {
            option: "--run-report",
        });
    }
    if args.safety_config.is_some() && !(args.mode == SupervisorMode::Connection && args.execute) {
        return Err(SitlError::MultiAgentConfigInvalid {
            message: "--safety-config requires --connection --execute".to_owned(),
        });
    }
    if args.mode == SupervisorMode::Connection && !args.execute {
        return Err(SitlError::LifecycleOptionRequiresExecute {
            option: "--connection",
        });
    }
    if args.mode != SupervisorMode::Connection {
        if let Some(option) = args.live_options.first_set_option() {
            return Err(SitlError::LifecycleOptionRequiresConnection { option });
        }
    }
    if args.mode != SupervisorMode::Mock {
        if args.fail_agent.is_some() {
            return Err(SitlError::MultiAgentConfigInvalid {
                message: "--fail-agent requires --mock".to_owned(),
            });
        }
        if args.heartbeat_timeout_ticks.is_some() {
            return Err(SitlError::MultiAgentConfigInvalid {
                message: "--heartbeat-timeout-ticks requires --mock".to_owned(),
            });
        }
        if args.max_ticks.is_some() {
            return Err(SitlError::MultiAgentConfigInvalid {
                message: "--max-ticks requires --mock".to_owned(),
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CliValidationArgs<'a> {
    mode: SupervisorMode,
    execute: bool,
    run_report: Option<&'a str>,
    safety_config: Option<&'a str>,
    fail_agent: Option<&'a str>,
    heartbeat_timeout_ticks: Option<u64>,
    max_ticks: Option<u64>,
    live_options: LiveOptionFlags,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LiveOptionFlags {
    timeout_set: bool,
    telemetry_timeout_set: bool,
    no_progress_timeout_set: bool,
    no_arm: bool,
    abort_after_set: bool,
}

impl LiveOptionFlags {
    fn first_set_option(self) -> Option<&'static str> {
        if self.timeout_set {
            Some("--timeout")
        } else if self.telemetry_timeout_set {
            Some("--telemetry-timeout")
        } else if self.no_progress_timeout_set {
            Some("--no-progress-timeout")
        } else if self.no_arm {
            Some("--no-arm")
        } else if self.abort_after_set {
            Some("--abort-after")
        } else {
            None
        }
    }
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
