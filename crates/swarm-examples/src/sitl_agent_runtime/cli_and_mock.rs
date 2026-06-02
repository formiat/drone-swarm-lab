use std::path::Path;
use std::thread;
use std::time::Duration;

use super::connection_and_reports::run_connection;
use crate::sitl_multi_agent::{
    agent_config, build_multi_agent_manifest, load_multi_agent_config, MultiAgentLifecycle,
    MultiAgentSitlAgentConfig,
};
use crate::sitl_observability::{
    write_sitl_event_log, SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
};
use crate::sitl_plan::{
    build_sitl_plan_for_task_ids, classify_connection_string, first_sitl_entry,
    format_dry_run_plan, load_sitl_suite, SitlConnectionClass, SitlError, SitlMode, SitlPlan,
};
use crate::sitl_safety::{
    load_sitl_safety_config, validate_pre_upload_safety, validate_pre_upload_safety_for_task_ids,
};
use swarm_comms::{MockMavlinkTransport, Waypoint};

pub(super) struct CliArgs {
    mode: Option<SitlMode>,
    scenario: String,
    agent_id: String,
    multi_agent_config: Option<String>,
    safety_config: Option<String>,
    run_report: Option<String>,
    replay_log: Option<String>,
    allow_hardware_candidate: bool,
    lifecycle: LifecycleArgs,
    lifecycle_from_cli: bool,
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

fn parse_args() -> Result<CliArgs, SitlError> {
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

pub fn run() -> Result<(), SitlError> {
    let cli = parse_args()?;
    let suite = load_sitl_suite(&cli.scenario)?;
    let multi_agent_config = cli
        .multi_agent_config
        .as_deref()
        .map(load_multi_agent_config)
        .transpose()?;

    let mut lifecycle = cli.lifecycle;
    let mut runtime_options = AgentRuntimeOptions::default();
    let mut mode = cli.mode.clone();
    let mut safety_task_ids: Option<Vec<String>> = None;
    let plan = if let Some(config) = multi_agent_config.as_ref() {
        let config_path = cli.multi_agent_config.as_ref().expect("config path exists");
        let manifest = build_multi_agent_manifest(&suite, &cli.scenario, config_path, config)?;
        let agent = agent_config(config, &cli.agent_id)?;
        if !cli.lifecycle_from_cli {
            lifecycle.mode = lifecycle_mode_from_config(agent.lifecycle);
        }
        runtime_options = runtime_options_from_config(agent);
        if mode.is_none() {
            mode = Some(SitlMode::Connection {
                addr: agent.connection_string.clone(),
            });
        }
        safety_task_ids = Some(agent.task_ids.clone());
        if matches!(mode, Some(SitlMode::DryRun)) {
            let agent_manifest = manifest
                .agents
                .iter()
                .find(|item| item.agent_id == cli.agent_id)
                .expect("validated manifest contains agent");
            eprintln!(
                "Multi-agent SITL: agent={} system_id={} component_id={} connection={} lifecycle={:?} start_delay_ms={} task_ids={}",
                agent_manifest.agent_id,
                agent_manifest.system_id,
                agent_manifest.component_id,
                agent_manifest.connection_string,
                agent_manifest.lifecycle,
                agent_manifest.start_delay_ms,
                agent_manifest.task_ids.join(",")
            );
        }
        build_sitl_plan_for_task_ids(&suite, &cli.scenario, &cli.agent_id, &agent.task_ids)?
    } else {
        crate::sitl_plan::build_sitl_plan(&suite, &cli.scenario, cli.agent_id.clone())?
    };

    let mode = mode.ok_or(SitlError::MissingMode)?;
    if cli.run_report.is_some() && lifecycle.mode != LifecycleMode::Execute {
        return Err(SitlError::RunReportRequiresExecute {
            option: "--run-report",
        });
    }

    if let SitlMode::Connection { addr } = &mode {
        enforce_hardware_candidate_boundary(addr, cli.allow_hardware_candidate)?;
        let safety_config = load_sitl_safety_config(cli.safety_config.as_deref().map(Path::new))?;
        let entry = first_sitl_entry(&suite, &cli.scenario)?;
        if let Some(task_ids) = safety_task_ids.as_ref() {
            validate_pre_upload_safety_for_task_ids(
                entry,
                &plan.agent_id,
                &safety_config,
                task_ids,
            )?;
        } else {
            validate_pre_upload_safety(entry, &plan.agent_id, &safety_config)?;
        }
    }

    match mode {
        SitlMode::Mock => {
            apply_start_delay(runtime_options.start_delay_ms);
            run_mock(&plan, cli.replay_log.as_deref())
        }
        SitlMode::DryRun => {
            print!("{}", format_dry_run_plan(&plan));
            Ok(())
        }
        SitlMode::Connection { addr } => run_connection(
            &plan,
            &addr,
            &lifecycle,
            runtime_options,
            cli.run_report.as_deref(),
            cli.replay_log.as_deref(),
        ),
    }
}

fn enforce_hardware_candidate_boundary(
    addr: &str,
    allow_hardware_candidate: bool,
) -> Result<(), SitlError> {
    let class = classify_connection_string(addr)?;
    if matches!(class, SitlConnectionClass::HardwareCandidate) {
        if allow_hardware_candidate {
            print_hardware_candidate_warning(addr, class);
        } else {
            return Err(SitlError::HardwareCandidateRequiresExplicitAllow {
                addr: addr.to_owned(),
                class: class.name(),
            });
        }
    }
    Ok(())
}

fn print_hardware_candidate_warning(addr: &str, class: SitlConnectionClass) {
    eprintln!(
        "WARNING: connection '{addr}' is classified as {}. This may target real hardware or a remote endpoint. This project is not hardware-ready, does not provide a certified safety layer, and requires the operator checklist in docs/HARDWARE_READINESS.md before any hardware experiment.",
        class.name()
    );
}

fn lifecycle_mode_from_config(lifecycle: MultiAgentLifecycle) -> LifecycleMode {
    match lifecycle {
        MultiAgentLifecycle::UploadOnly => LifecycleMode::UploadOnly,
        MultiAgentLifecycle::Execute => LifecycleMode::Execute,
    }
}

fn runtime_options_from_config(config: &MultiAgentSitlAgentConfig) -> AgentRuntimeOptions {
    AgentRuntimeOptions {
        start_delay_ms: config.start_delay_ms,
        target_system: config.system_id,
        target_component: config.component_id,
    }
}

pub(super) fn apply_start_delay(start_delay_ms: u64) {
    if start_delay_ms > 0 {
        thread::sleep(Duration::from_millis(start_delay_ms));
    }
}

fn run_mock(plan: &SitlPlan, replay_log: Option<&str>) -> Result<(), SitlError> {
    let mut transport = MockMavlinkTransport::new();
    let mut recorder =
        replay_log.map(|_| new_sitl_event_recorder(plan, None, SitlEventLogMode::Mock));
    if let Some(recorder) = recorder.as_mut() {
        recorder.push_connection_opened();
        recorder.push_mission_count_sent(plan.waypoints.len());
    }
    eprintln!(
        "SITL Agent: {} | {} waypoints | mock=true",
        plan.agent_id,
        plan.waypoints.len()
    );

    for waypoint_item in &plan.waypoints {
        let waypoint = Waypoint {
            x: waypoint_item.x,
            y: waypoint_item.y,
            z: waypoint_item.z,
            seq: waypoint_item.seq,
        };
        eprintln!(
            "WAYPOINT seq={} x={:.1} y={:.1} z={:.1}",
            waypoint.seq, waypoint.x, waypoint.y, waypoint.z
        );
        if let Some(recorder) = recorder.as_mut() {
            recorder.push_mission_item_sent(waypoint.seq, Some(waypoint_item.task_id.clone()));
            recorder.push_task_completed(waypoint.seq, waypoint_item.task_id.clone());
        }
        transport.send_waypoint(waypoint);
    }
    eprintln!("Mock mode: {} waypoints sent.", transport.waypoints().len());
    if let Some(recorder) = recorder.as_mut() {
        recorder.push_run_completed("completed");
        write_replay_log_if_requested(replay_log, recorder)?;
    }
    Ok(())
}

pub(super) fn new_sitl_event_recorder(
    plan: &SitlPlan,
    connection_string: Option<&str>,
    mode: SitlEventLogMode,
) -> SitlEventRecorder {
    let mode_name = mode.as_str();
    let run_id = format!("{}:{}:{mode_name}", plan.scenario_name, plan.agent_id);
    SitlEventRecorder::new(SitlEventLogMetadata {
        run_id,
        scenario_path: plan.scenario_path.clone(),
        scenario_name: plan.scenario_name.clone(),
        mission: plan.mission.clone(),
        profile: plan.profile.clone(),
        agent_id: plan.agent_id.clone(),
        connection_string: connection_string.map(str::to_owned),
        mode,
    })
}

pub(super) fn write_replay_log_if_requested(
    path: Option<&str>,
    recorder: &SitlEventRecorder,
) -> Result<(), SitlError> {
    let Some(path) = path else {
        return Ok(());
    };
    write_sitl_event_log(path, recorder.log()).map_err(|error| SitlError::ReplayLogWrite {
        path: Path::new(path).to_path_buf(),
        message: error.to_string(),
    })?;
    eprintln!("SITL replay log written: {path}");
    Ok(())
}
