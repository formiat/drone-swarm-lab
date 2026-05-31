use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use swarm_examples::sitl_connection::SitlConnectionLifecycle;
use swarm_examples::sitl_multi_agent::{
    build_multi_agent_manifest, load_multi_agent_config, MultiAgentSitlManifest,
};
use swarm_examples::sitl_observability::{format_sitl_summary, read_sitl_event_log};
use swarm_examples::sitl_plan::{load_sitl_suite, SitlError};
use swarm_examples::sitl_report::SitlMultiAgentRunReport;
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
    output_dir: Option<String>,
    run_id: Option<String>,
    force: bool,
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
    reupload_on_failure: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OutputPaths {
    manifest: Option<PathBuf>,
    replay_log: Option<PathBuf>,
    run_report: Option<PathBuf>,
    replay_summary: Option<PathBuf>,
    run_id: Option<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let code = supervisor_exit_code(&error);
            eprintln!("error: {error}");
            if prints_usage(&error) {
                eprintln!("{}", usage());
            }
            ExitCode::from(code)
        }
    }
}

fn run() -> Result<(), SitlError> {
    let cli = parse_args()?;
    let suite = load_sitl_suite(&cli.scenario)?;
    let config = load_multi_agent_config(&cli.config)?;
    let manifest = build_multi_agent_manifest(&suite, &cli.scenario, &cli.config, &config)?;
    let output_paths = resolve_output_paths(&cli, &manifest);
    ensure_output_paths_available(&output_paths, cli.force)?;

    match cli.mode {
        SupervisorMode::DryRun => {
            write_or_print_manifest(output_paths.manifest.as_deref(), &manifest, cli.force)?;
        }
        SupervisorMode::Mock => {
            let mock_config = SupervisorMockConfig {
                scenario_path: cli.scenario.clone(),
                replay_log: output_paths
                    .replay_log
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                run_id: output_paths.run_id.clone(),
                fail_agent: cli.fail_agent.clone(),
                fail_after_ticks: cli.fail_after_ticks,
                heartbeat_timeout_ticks: cli.heartbeat_timeout_ticks,
                max_ticks: cli.max_ticks,
            };
            let _: SupervisorMetrics = run_mock_supervisor(&suite, &mock_config, &manifest)?;
            write_or_print_manifest(output_paths.manifest.as_deref(), &manifest, cli.force)?;
            write_replay_summary_if_requested(&output_paths, cli.force)?;
        }
        SupervisorMode::Connection => {
            let live_config = SupervisorLiveConfig {
                scenario_path: cli.scenario.clone(),
                config_path: cli.config.clone(),
                safety_config_path: cli.safety_config.clone(),
                replay_log: output_paths
                    .replay_log
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                run_report: output_paths
                    .run_report
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                lifecycle: SitlConnectionLifecycle {
                    timeout: cli.timeout,
                    telemetry_timeout: cli.telemetry_timeout,
                    no_progress_timeout: cli.no_progress_timeout,
                    no_arm: cli.no_arm,
                    abort_after: cli.abort_after,
                },
                allow_hardware_candidate: cli.allow_hardware_candidate,
                reupload_on_failure: cli.reupload_on_failure,
                run_id: output_paths.run_id.clone(),
            };
            let report = run_live_supervisor(&suite, &live_config, &manifest)?;
            write_or_print_manifest(output_paths.manifest.as_deref(), &manifest, cli.force)?;
            write_replay_summary_if_requested(&output_paths, cli.force)?;
            if !matches!(
                report.final_status.as_str(),
                "completed" | "completed_with_reallocation"
            ) {
                return Err(SitlError::ConnectionFailed {
                    message: report_failure_message(&report),
                });
            }
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

fn resolve_output_paths(cli: &CliArgs, manifest: &MultiAgentSitlManifest) -> OutputPaths {
    let generated_run_id = cli.output_dir.as_ref().map(|_| {
        cli.run_id
            .clone()
            .unwrap_or_else(|| generated_run_id(manifest))
    });
    let run_id = cli.run_id.clone().or(generated_run_id.clone());

    if let Some(output_dir) = &cli.output_dir {
        let base = PathBuf::from(output_dir).join(generated_run_id.as_deref().unwrap());
        let replay_log = cli.replay_log.as_ref().map(PathBuf::from).or_else(|| {
            (cli.mode != SupervisorMode::DryRun).then(|| base.join("events.sitl-log.json"))
        });
        return OutputPaths {
            manifest: cli
                .manifest
                .as_ref()
                .map(PathBuf::from)
                .or_else(|| Some(base.join("manifest.json"))),
            replay_summary: replay_log.as_ref().map(|_| base.join("replay-summary.txt")),
            replay_log,
            run_report: cli.run_report.as_ref().map(PathBuf::from).or_else(|| {
                (cli.mode == SupervisorMode::Connection).then(|| base.join("run-report.json"))
            }),
            run_id,
        };
    }

    OutputPaths {
        manifest: cli.manifest.as_ref().map(PathBuf::from),
        replay_log: cli.replay_log.as_ref().map(PathBuf::from),
        run_report: cli.run_report.as_ref().map(PathBuf::from),
        replay_summary: None,
        run_id,
    }
}

fn generated_run_id(manifest: &MultiAgentSitlManifest) -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!(
        "sitl-supervisor-{}-{seconds}",
        sanitize_run_id_component(&manifest.scenario_name)
    )
}

fn sanitize_run_id_component(value: &str) -> String {
    let mut sanitized = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            sanitized.push(ch);
        } else if !sanitized.ends_with('-') {
            sanitized.push('-');
        }
    }
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "run".to_owned()
    } else {
        sanitized.to_owned()
    }
}

fn ensure_output_paths_available(paths: &OutputPaths, force: bool) -> Result<(), SitlError> {
    for path in [
        paths.manifest.as_deref(),
        paths.replay_log.as_deref(),
        paths.run_report.as_deref(),
        paths.replay_summary.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        ensure_output_path_available(path, force)?;
    }
    Ok(())
}

fn ensure_output_path_available(path: &Path, force: bool) -> Result<(), SitlError> {
    if !force && path.exists() {
        return Err(SitlError::OutputAlreadyExists {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn write_checked_file(
    path: &Path,
    contents: impl AsRef<[u8]>,
    force: bool,
    map_error: fn(PathBuf, String) -> SitlError,
) -> Result<(), SitlError> {
    ensure_output_path_available(path, force)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| map_error(parent.to_path_buf(), error.to_string()))?;
    }
    std::fs::write(path, contents).map_err(|error| map_error(path.to_path_buf(), error.to_string()))
}

fn write_replay_summary_if_requested(paths: &OutputPaths, force: bool) -> Result<(), SitlError> {
    let (Some(summary_path), Some(replay_log_path)) = (&paths.replay_summary, &paths.replay_log)
    else {
        return Ok(());
    };
    let log =
        read_sitl_event_log(replay_log_path).map_err(|error| SitlError::ReplaySummaryWrite {
            path: summary_path.to_path_buf(),
            message: format!(
                "source event log {} read failed: {error}",
                replay_log_path.display()
            ),
        })?;
    let summary =
        format_sitl_summary(&swarm_examples::sitl_observability::summarize_sitl_event_log(&log));
    write_checked_file(summary_path, summary, force, replay_summary_write_error)?;
    eprintln!(
        "SITL supervisor replay summary written: {}",
        summary_path.display()
    );
    Ok(())
}

fn supervisor_exit_code(error: &SitlError) -> u8 {
    match error {
        SitlError::SafetyConfigRead { .. }
        | SitlError::SafetyConfigParse { .. }
        | SitlError::SafetyConfigInvalid { .. }
        | SitlError::SafetyValidationFailed { .. }
        | SitlError::HardwareCandidateRequiresExplicitAllow { .. } => 3,
        SitlError::FeatureMissing { .. } => 20,
        SitlError::RunReportWrite { .. }
        | SitlError::ReplayLogWrite { .. }
        | SitlError::ReplaySummaryWrite { .. }
        | SitlError::MultiAgentManifestWrite { .. }
        | SitlError::OutputAlreadyExists { .. } => 40,
        SitlError::ConnectionFailed { message } => classify_connection_failure_exit_code(message),
        _ => 2,
    }
}

fn classify_connection_failure_exit_code(message: &str) -> u8 {
    let lower = message.to_ascii_lowercase();
    if lower.contains("endpoint")
        || lower.contains("connection open")
        || lower.contains("open failed")
        || lower.contains("transport")
        || lower.contains("connection refused")
        || lower.contains("connection failed")
        || lower.contains("failed to connect")
        || lower.contains("unable to connect")
    {
        20
    } else if lower.contains("heartbeat")
        || lower.contains("telemetry")
        || lower.contains("progress")
        || lower.contains("timeout")
    {
        22
    } else if lower.contains("abort") {
        23
    } else if lower.contains("upload")
        || lower.contains("mission")
        || lower.contains("ack")
        || lower.contains("reject")
        || lower.contains("command")
    {
        21
    } else if lower.contains("final_status")
        || lower.contains("partial")
        || lower.contains("failed")
        || lower.contains("failure")
    {
        30
    } else {
        20
    }
}

fn report_failure_message(report: &SitlMultiAgentRunReport) -> String {
    let Some(agent) = report
        .agents
        .iter()
        .find(|agent| agent.final_status != "completed")
    else {
        return format!(
            "supervisor run finished with final_status '{}'",
            report.final_status
        );
    };
    format!(
        "supervisor run finished with final_status '{}'; failed agent '{}' final_status '{}' error: {}",
        report.final_status,
        agent.agent_id,
        agent.final_status,
        agent
            .error
            .as_deref()
            .unwrap_or("agent did not report an error")
    )
}

#[cfg(test)]
fn report_failure_exit_code(report: &SitlMultiAgentRunReport) -> u8 {
    classify_connection_failure_exit_code(&report_failure_message(report))
}

fn manifest_write_error(path: PathBuf, message: String) -> SitlError {
    SitlError::MultiAgentManifestWrite { path, message }
}

fn replay_summary_write_error(path: PathBuf, message: String) -> SitlError {
    SitlError::ReplaySummaryWrite { path, message }
}

fn prints_usage(error: &SitlError) -> bool {
    matches!(
        error,
        SitlError::MissingMode
            | SitlError::ConflictingModes
            | SitlError::MissingArgument { .. }
            | SitlError::UnknownArgument { .. }
            | SitlError::LifecycleOptionRequiresConnection { .. }
            | SitlError::LifecycleOptionRequiresExecute { .. }
            | SitlError::InvalidDuration { .. }
            | SitlError::RunReportRequiresExecute { .. }
            | SitlError::MultiAgentConfigInvalid { .. }
    )
}

fn usage() -> &'static str {
    "usage: sitl_supervisor --dry-run|--mock|--connection --scenario <path> --config <path> [--manifest <path>] [--output-dir <dir>] [--run-id <id>] [--force] [--replay-log <path>] [--fail-agent <id>] [--fail-after-ticks N] [--heartbeat-timeout-ticks N] [--max-ticks N] [--execute] [--run-report <path>] [--safety-config <path>] [--timeout <duration>] [--telemetry-timeout <duration>] [--no-progress-timeout <duration>] [--no-arm] [--abort-after <duration>] [--allow-hardware-candidate] [--reupload-on-failure]"
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
    reupload_on_failure: bool,
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
    manifest_path: Option<&Path>,
    manifest: &MultiAgentSitlManifest,
    force: bool,
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
    write_checked_file(path, json, force, manifest_write_error)?;
    eprintln!("Multi-agent SITL manifest written: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use swarm_examples::sitl_multi_agent::TaskOwnershipSummary;
    use swarm_examples::sitl_observability::SitlEventLogSummary;
    use swarm_examples::sitl_report::{
        SitlMultiAgentAgentReport, SitlMultiAgentReallocationReport,
    };

    fn report_with_failed_agent(
        final_status: &str,
        agent_status: &str,
        error: &str,
    ) -> SitlMultiAgentRunReport {
        SitlMultiAgentRunReport {
            schema_version: "sitl_multi_agent_run_report.v1".to_owned(),
            run_id: "run-m60-error-matrix".to_owned(),
            scenario_path: PathBuf::from("scenario.json"),
            scenario_name: "m60_error_matrix".to_owned(),
            config_path: PathBuf::from("config.json"),
            mission: "sitl".to_owned(),
            profile: "multi-agent".to_owned(),
            mode: "connection_execute".to_owned(),
            agents: vec![SitlMultiAgentAgentReport {
                agent_id: "agent-0".to_owned(),
                connection_string: "udp:127.0.0.1:14550".to_owned(),
                system_id: 1,
                component_id: 1,
                lifecycle: "execute".to_owned(),
                mission_item_count: 2,
                completed_task_count: usize::from(final_status == "partial_failed"),
                final_status: agent_status.to_owned(),
                error: Some(error.to_owned()),
            }],
            total_completed_tasks: usize::from(final_status == "partial_failed"),
            failed_agents: 1,
            aborted_agents: usize::from(agent_status == "aborted"),
            overall_status: final_status.to_owned(),
            event_log_path: Some(PathBuf::from("events.sitl-log.json")),
            task_ownership: TaskOwnershipSummary::default(),
            events_summary: SitlEventLogSummary::default(),
            final_status: final_status.to_owned(),
            reallocation: SitlMultiAgentReallocationReport::default(),
            limitations: Vec::new(),
            known_limitations: Vec::new(),
        }
    }

    #[test]
    fn live_report_failure_exit_code_matrix_uses_agent_error() {
        let cases = [
            (
                "failed",
                "failed",
                "connection open failed: endpoint udp:127.0.0.1:14550 unavailable",
                20,
            ),
            (
                "failed",
                "failed",
                "mission upload failed: MAV_MISSION_ERROR",
                21,
            ),
            (
                "failed",
                "failed",
                "command rejected: MAV_CMD_MISSION_START MAV_RESULT_DENIED",
                21,
            ),
            ("failed", "failed", "heartbeat timeout before start", 22),
            ("failed", "failed", "telemetry timeout after start", 22),
            ("failed", "failed", "no mission progress before timeout", 22),
            ("failed", "aborted", "abort failed: command rejected", 23),
            (
                "partial_failed",
                "failed",
                "agent completed one task then failed after start",
                30,
            ),
        ];

        for (final_status, agent_status, error, expected_code) in cases {
            let report = report_with_failed_agent(final_status, agent_status, error);
            assert_eq!(
                report_failure_exit_code(&report),
                expected_code,
                "wrong exit code for error: {error}"
            );
            assert!(report_failure_message(&report).contains(error));
        }
    }
}
