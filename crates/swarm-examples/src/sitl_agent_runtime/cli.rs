use std::time::Duration;

use crate::sitl_plan::{SitlError, SitlMode};

pub(super) struct CliArgs {
    pub(super) mode: Option<SitlMode>,
    pub(super) scenario: String,
    pub(super) agent_id: String,
    pub(super) multi_agent_config: Option<String>,
    pub(super) safety_config: Option<String>,
    pub(super) run_report: Option<String>,
    pub(super) replay_log: Option<String>,
    pub(super) allow_hardware_candidate: bool,
    pub(super) lifecycle: LifecycleArgs,
    pub(super) lifecycle_from_cli: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LifecycleMode {
    UploadOnly,
    Execute,
}

pub(super) struct LifecycleArgs {
    pub(super) mode: LifecycleMode,
    pub(super) no_arm: bool,
    pub(super) abort_after: Option<Duration>,
    pub(super) timeout: Duration,
    pub(super) telemetry_timeout: Duration,
    pub(super) no_progress_timeout: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AgentRuntimeOptions {
    pub(super) start_delay_ms: u64,
    pub(super) target_system: u8,
    pub(super) target_component: u8,
}

impl Default for AgentRuntimeOptions {
    fn default() -> Self {
        Self {
            start_delay_ms: 0,
            target_system: 1,
            target_component: 1,
        }
    }
}

pub(super) fn parse_args() -> Result<CliArgs, SitlError> {
    let args: Vec<String> = std::env::args().collect();
    let mut mode: Option<SitlMode> = None;
    let mut scenario: Option<String> = None;
    let mut agent_id: Option<String> = None;
    let mut multi_agent_config: Option<String> = None;
    let mut safety_config: Option<String> = None;
    let mut run_report: Option<String> = None;
    let mut replay_log: Option<String> = None;
    let mut allow_hardware_candidate = false;
    let mut lifecycle_mode: Option<LifecycleMode> = None;
    let mut no_arm = false;
    let mut abort_after: Option<Duration> = None;
    let mut timeout = Duration::from_secs(2);
    let mut telemetry_timeout = Duration::from_secs(10);
    let mut no_progress_timeout = Duration::from_secs(60);
    let mut telemetry_timeout_set = false;
    let mut no_progress_timeout_set = false;
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
            "--multi-agent-config" => {
                i += 1;
                multi_agent_config = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--multi-agent-config <path>",
                        })?
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
            "--run-report" => {
                i += 1;
                run_report = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--run-report <path>",
                        })?
                        .clone(),
                );
                connection_only_option.get_or_insert("--run-report");
            }
            "--replay-log" => {
                i += 1;
                replay_log = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--replay-log <path>",
                        })?
                        .clone(),
                );
            }
            "--allow-hardware-candidate" => {
                allow_hardware_candidate = true;
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
            "--telemetry-timeout" => {
                i += 1;
                let value = args.get(i).ok_or(SitlError::MissingArgument {
                    name: "--telemetry-timeout <seconds>",
                })?;
                telemetry_timeout = parse_duration_arg("--telemetry-timeout", value, false)?;
                telemetry_timeout_set = true;
                connection_only_option.get_or_insert("--telemetry-timeout");
            }
            "--no-progress-timeout" => {
                i += 1;
                let value = args.get(i).ok_or(SitlError::MissingArgument {
                    name: "--no-progress-timeout <seconds>",
                })?;
                no_progress_timeout = parse_duration_arg("--no-progress-timeout", value, false)?;
                no_progress_timeout_set = true;
                connection_only_option.get_or_insert("--no-progress-timeout");
            }
            arg => {
                return Err(SitlError::UnknownArgument {
                    arg: arg.to_owned(),
                });
            }
        }
        i += 1;
    }

    if mode.is_none() && multi_agent_config.is_none() {
        return Err(SitlError::MissingMode);
    }
    let lifecycle_from_cli = lifecycle_mode.is_some();
    let lifecycle_mode = lifecycle_mode.unwrap_or(LifecycleMode::UploadOnly);
    let config_implies_connection = mode.is_none() && multi_agent_config.is_some();
    let explicit_connection_mode = matches!(mode, Some(SitlMode::Connection { .. }));
    if allow_hardware_candidate && !explicit_connection_mode && !config_implies_connection {
        return Err(SitlError::ConnectionOptionRequiresConnection {
            option: "--allow-hardware-candidate",
        });
    }
    if !explicit_connection_mode && !config_implies_connection {
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
    if telemetry_timeout_set && lifecycle_mode != LifecycleMode::Execute {
        return Err(SitlError::LifecycleOptionRequiresExecute {
            option: "--telemetry-timeout",
        });
    }
    if no_progress_timeout_set && lifecycle_mode != LifecycleMode::Execute {
        return Err(SitlError::LifecycleOptionRequiresExecute {
            option: "--no-progress-timeout",
        });
    }
    let run_report_may_be_valid = config_implies_connection
        || (explicit_connection_mode && lifecycle_mode == LifecycleMode::Execute);
    if run_report.is_some() && !run_report_may_be_valid {
        return Err(SitlError::RunReportRequiresExecute {
            option: "--run-report",
        });
    }
    if replay_log.is_some() && matches!(mode, Some(SitlMode::DryRun)) {
        return Err(SitlError::ReplayLogUnsupported {
            option: "--replay-log",
            mode: "dry-run",
        });
    }

    Ok(CliArgs {
        mode,
        scenario: scenario.ok_or(SitlError::MissingArgument { name: "--scenario" })?,
        agent_id: agent_id.ok_or(SitlError::MissingArgument { name: "--agent-id" })?,
        multi_agent_config,
        safety_config,
        run_report,
        replay_log,
        allow_hardware_candidate,
        lifecycle: LifecycleArgs {
            mode: lifecycle_mode,
            no_arm,
            abort_after,
            timeout,
            telemetry_timeout,
            no_progress_timeout,
        },
        lifecycle_from_cli,
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
