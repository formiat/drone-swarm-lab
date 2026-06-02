use std::time::Duration;

use crate::sitl_plan::SitlError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SupervisorMode {
    DryRun,
    Mock,
    Connection,
}

pub(super) struct CliArgs {
    pub(super) mode: SupervisorMode,
    pub(super) scenario: String,
    pub(super) config: String,
    pub(super) manifest: Option<String>,
    pub(super) replay_log: Option<String>,
    pub(super) run_report: Option<String>,
    pub(super) output_dir: Option<String>,
    pub(super) run_id: Option<String>,
    pub(super) force: bool,
    pub(super) safety_config: Option<String>,
    pub(super) fail_agent: Option<String>,
    pub(super) fail_after_ticks: u64,
    pub(super) heartbeat_timeout_ticks: Option<u64>,
    pub(super) max_ticks: Option<u64>,
    pub(super) timeout: Duration,
    pub(super) telemetry_timeout: Duration,
    pub(super) no_progress_timeout: Duration,
    pub(super) no_arm: bool,
    pub(super) abort_after: Option<Duration>,
    pub(super) allow_hardware_candidate: bool,
    pub(super) reupload_on_failure: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CliValidationArgs<'a> {
    pub(super) mode: SupervisorMode,
    pub(super) execute: bool,
    pub(super) run_report: Option<&'a str>,
    pub(super) safety_config: Option<&'a str>,
    pub(super) fail_agent: Option<&'a str>,
    pub(super) heartbeat_timeout_ticks: Option<u64>,
    pub(super) max_ticks: Option<u64>,
    pub(super) reupload_on_failure: bool,
    pub(super) live_options: LiveOptionFlags,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct LiveOptionFlags {
    pub(super) timeout_set: bool,
    pub(super) telemetry_timeout_set: bool,
    pub(super) no_progress_timeout_set: bool,
    pub(super) no_arm: bool,
    pub(super) abort_after_set: bool,
}

impl LiveOptionFlags {
    pub(super) fn first_set_option(self) -> Option<&'static str> {
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

pub(super) fn parse_args() -> Result<CliArgs, SitlError> {
    let args: Vec<String> = std::env::args().collect();
    let mut mode = None;
    let mut scenario = None;
    let mut config = None;
    let mut manifest = None;
    let mut replay_log = None;
    let mut run_report = None;
    let mut output_dir = None;
    let mut run_id = None;
    let mut force = false;
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
    let mut reupload_on_failure = false;

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
            "--output-dir" => {
                i += 1;
                output_dir = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--output-dir",
                        })?
                        .clone(),
                );
            }
            "--run-id" => {
                i += 1;
                run_id = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument { name: "--run-id" })?
                        .clone(),
                );
            }
            "--force" => force = true,
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
            "--reupload-on-failure" => reupload_on_failure = true,
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
        reupload_on_failure,
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
        output_dir,
        run_id,
        force,
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
        reupload_on_failure,
    })
}

pub(super) fn parse_u64_arg(value: &str, name: &'static str) -> Result<u64, SitlError> {
    value
        .parse::<u64>()
        .map_err(|_| SitlError::MultiAgentConfigInvalid {
            message: format!("invalid {name} value '{value}'"),
        })
}

pub(super) fn parse_duration_arg(value: &str, name: &'static str) -> Result<Duration, SitlError> {
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

pub(super) fn set_mode(
    mode: &mut Option<SupervisorMode>,
    next: SupervisorMode,
) -> Result<(), SitlError> {
    if mode.is_some() {
        return Err(SitlError::ConflictingModes);
    }
    *mode = Some(next);
    Ok(())
}

pub(super) fn validate_cli_arg_combinations(args: CliValidationArgs<'_>) -> Result<(), SitlError> {
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
    if args.reupload_on_failure && !(args.mode == SupervisorMode::Connection && args.execute) {
        return Err(SitlError::MultiAgentConfigInvalid {
            message: "--reupload-on-failure requires --connection --execute".to_owned(),
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

pub(super) fn usage() -> &'static str {
    "usage: sitl_supervisor --dry-run|--mock|--connection --scenario <path> --config <path> [--manifest <path>] [--output-dir <dir>] [--run-id <id>] [--force] [--replay-log <path>] [--fail-agent <id>] [--fail-after-ticks N] [--heartbeat-timeout-ticks N] [--max-ticks N] [--execute] [--run-report <path>] [--safety-config <path>] [--timeout <duration>] [--telemetry-timeout <duration>] [--no-progress-timeout <duration>] [--no-arm] [--abort-after <duration>] [--allow-hardware-candidate] [--reupload-on-failure]"
}
