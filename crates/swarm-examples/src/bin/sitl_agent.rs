use std::path::Path;
use std::thread;
use std::time::Duration;
#[cfg(feature = "mavlink-transport")]
use std::time::Instant;

use swarm_comms::{MockMavlinkTransport, Waypoint};
use swarm_examples::sitl_multi_agent::{
    agent_config, build_multi_agent_manifest, load_multi_agent_config, MultiAgentLifecycle,
    MultiAgentSitlAgentConfig,
};
use swarm_examples::sitl_observability::{
    write_sitl_event_log, SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
};
use swarm_examples::sitl_plan::{
    build_sitl_plan_for_task_ids, first_sitl_entry, format_dry_run_plan, load_sitl_suite,
    validate_connection_string, SitlError, SitlMode, SitlPlan,
};
#[cfg(feature = "mavlink-transport")]
use swarm_examples::sitl_report::{
    write_sitl_run_report, SitlRunFinalStatus, SitlRunMode, SitlRunReport,
};
use swarm_examples::sitl_safety::{
    load_sitl_safety_config, validate_pre_upload_safety, validate_pre_upload_safety_for_task_ids,
};

struct CliArgs {
    mode: Option<SitlMode>,
    scenario: String,
    agent_id: String,
    multi_agent_config: Option<String>,
    safety_config: Option<String>,
    run_report: Option<String>,
    replay_log: Option<String>,
    lifecycle: LifecycleArgs,
    lifecycle_from_cli: bool,
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
    telemetry_timeout: Duration,
    no_progress_timeout: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AgentRuntimeOptions {
    start_delay_ms: u64,
    target_system: u8,
    target_component: u8,
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

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        eprintln!(
            "usage: sitl_agent --mock|--dry-run|--connection <addr> --scenario <path> --agent-id <id> [--multi-agent-config <path>] [--safety-config <path>] [--upload-only|--execute] [--no-arm] [--abort-after <seconds>] [--timeout <seconds>] [--telemetry-timeout <seconds>] [--no-progress-timeout <seconds>] [--run-report <path>] [--replay-log <path>]"
        );
        std::process::exit(1);
    }
}

fn run() -> Result<(), SitlError> {
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
        swarm_examples::sitl_plan::build_sitl_plan(&suite, &cli.scenario, cli.agent_id.clone())?
    };

    let mode = mode.ok_or(SitlError::MissingMode)?;
    if cli.run_report.is_some() && lifecycle.mode != LifecycleMode::Execute {
        return Err(SitlError::RunReportRequiresExecute {
            option: "--run-report",
        });
    }

    if let SitlMode::Connection { addr } = &mode {
        validate_connection_string(addr)?;
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

fn apply_start_delay(start_delay_ms: u64) {
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

fn new_sitl_event_recorder(
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

fn write_replay_log_if_requested(
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

fn run_connection(
    plan: &SitlPlan,
    connection_string: &str,
    lifecycle: &LifecycleArgs,
    runtime_options: AgentRuntimeOptions,
    run_report: Option<&str>,
    replay_log: Option<&str>,
) -> Result<(), SitlError> {
    validate_connection_string(connection_string)?;
    apply_start_delay(runtime_options.start_delay_ms);

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
        let event_mode = match lifecycle.mode {
            LifecycleMode::UploadOnly => SitlEventLogMode::ConnectionUploadOnly,
            LifecycleMode::Execute => SitlEventLogMode::ConnectionExecute,
        };
        let mut event_recorder =
            replay_log.map(|_| new_sitl_event_recorder(plan, Some(connection_string), event_mode));
        if let Some(recorder) = event_recorder.as_mut() {
            recorder.push_connection_opened();
        }
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
            target_system: runtime_options.target_system,
            target_component: runtime_options.target_component,
            timeout: lifecycle.timeout,
            ..MissionUploadOptions::default()
        };
        match lifecycle.mode {
            LifecycleMode::UploadOnly => {
                let upload_result = if let Some(recorder) = event_recorder.as_mut() {
                    let mut observer = SitlMavlinkObserver { recorder };
                    transport.upload_mission_observed(&waypoints, upload_options, &mut observer)
                } else {
                    transport.upload_mission(&waypoints, upload_options)
                };
                let report = match upload_result {
                    Ok(report) => report,
                    Err(error) => {
                        if let Some(recorder) = event_recorder.as_mut() {
                            recorder.push_failure("failed", error.to_string());
                            write_replay_log_if_requested(replay_log, recorder)?;
                        }
                        return Err(SitlError::ConnectionFailed {
                            message: error.to_string(),
                        });
                    }
                };
                eprintln!(
                    "Real MAVLink mode: mission accepted; lifecycle=upload-only uploaded_count={} target_system={} target_component={} cleared_existing={}",
                    report.uploaded_count,
                    report.target_system,
                    report.target_component,
                    report.cleared_existing
                );
                if let Some(recorder) = event_recorder.as_mut() {
                    recorder.push_run_completed("upload_accepted");
                    write_replay_log_if_requested(replay_log, recorder)?;
                }
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
                let result = {
                    let mut driver = MavlinkGoldenPathDriver {
                        transport: &mut transport,
                        recorder: event_recorder.as_mut(),
                    };
                    run_golden_path_with_driver(
                        &mut driver,
                        SitlGoldenPathRun {
                            plan,
                            waypoints: &waypoints,
                            connection_string,
                            upload_options,
                            lifecycle_options,
                            lifecycle,
                            run_report,
                        },
                    )
                };
                if let Some(recorder) = event_recorder.as_ref() {
                    write_replay_log_if_requested(replay_log, recorder)?;
                }
                result?;
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "mavlink-transport"))]
    {
        let _ = plan;
        let _ = run_report;
        let _ = replay_log;
        let _ = runtime_options;
        let _ = (
            lifecycle.mode,
            lifecycle.no_arm,
            lifecycle.abort_after,
            lifecycle.timeout,
            lifecycle.telemetry_timeout,
            lifecycle.no_progress_timeout,
        );
        Err(SitlError::FeatureMissing {
            feature: "mavlink-transport",
        })
    }
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq)]
struct SitlExecutionSuccess {
    uploaded_count: usize,
    progress_report: swarm_examples::sitl_progress::SitlMissionProgressReport,
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct SitlExecutionFailure {
    final_status: SitlRunFinalStatus,
    mission_item_count: usize,
    completed_count: usize,
    failed_count: usize,
    error: String,
    abort_result: Option<String>,
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq)]
struct SitlMissionStartReport {
    uploaded_count: usize,
    armed: bool,
    took_off: bool,
    started: bool,
    post_start_heartbeat: bool,
    abort_result: Option<swarm_comms::AbortCommandResult>,
}

#[cfg(feature = "mavlink-transport")]
trait SitlGoldenPathDriver {
    fn upload_and_start_mission(
        &mut self,
        waypoints: &[Waypoint],
        upload_options: swarm_comms::MissionUploadOptions,
        lifecycle_options: swarm_comms::MissionLifecycleOptions,
    ) -> Result<SitlMissionStartReport, SitlExecutionFailure>;

    fn run_telemetry_progress(
        &mut self,
        plan: &SitlPlan,
        lifecycle: &LifecycleArgs,
        lifecycle_options: &swarm_comms::MissionLifecycleOptions,
        mission_item_count: usize,
    ) -> Result<swarm_examples::sitl_progress::SitlMissionProgressReport, SitlExecutionFailure>;

    fn record_run_completed(&mut self, _status: &str) {}

    fn record_failure(&mut self, _status: &str, _error: &str) {}
}

#[cfg(feature = "mavlink-transport")]
struct MavlinkGoldenPathDriver<'a> {
    transport: &'a mut swarm_comms::MavlinkTransport,
    recorder: Option<&'a mut SitlEventRecorder>,
}

#[cfg(feature = "mavlink-transport")]
struct SitlMavlinkObserver<'a> {
    recorder: &'a mut SitlEventRecorder,
}

#[cfg(feature = "mavlink-transport")]
impl swarm_comms::MavlinkMissionObserver for SitlMavlinkObserver<'_> {
    fn on_event(&mut self, event: swarm_comms::MavlinkMissionEvent) {
        match event {
            swarm_comms::MavlinkMissionEvent::HeartbeatSeen => {
                self.recorder.push_heartbeat_seen();
            }
            swarm_comms::MavlinkMissionEvent::MissionClearSent => {
                self.recorder.push_mission_clear_sent();
            }
            swarm_comms::MavlinkMissionEvent::MissionCountSent { count } => {
                self.recorder.push_mission_count_sent(count);
            }
            swarm_comms::MavlinkMissionEvent::MissionItemRequested { seq } => {
                self.recorder.push_mission_item_requested(seq);
            }
            swarm_comms::MavlinkMissionEvent::MissionItemSent { seq } => {
                self.recorder.push_mission_item_sent(seq, None);
            }
            swarm_comms::MavlinkMissionEvent::MissionAckReceived { result, accepted } => {
                self.recorder.push_mission_ack_received(result, accepted);
            }
            swarm_comms::MavlinkMissionEvent::CommandSent { command } => {
                self.recorder.push_command_sent(command);
            }
            swarm_comms::MavlinkMissionEvent::CommandAckReceived {
                command,
                result,
                accepted,
            } => {
                self.recorder
                    .push_command_ack_received(command, result, accepted);
            }
            swarm_comms::MavlinkMissionEvent::AbortRequested { result } => {
                self.recorder.push_abort_requested(Some(result));
            }
        }
    }
}

#[cfg(feature = "mavlink-transport")]
impl SitlGoldenPathDriver for MavlinkGoldenPathDriver<'_> {
    fn upload_and_start_mission(
        &mut self,
        waypoints: &[Waypoint],
        upload_options: swarm_comms::MissionUploadOptions,
        lifecycle_options: swarm_comms::MissionLifecycleOptions,
    ) -> Result<SitlMissionStartReport, SitlExecutionFailure> {
        let report = if let Some(recorder) = self.recorder.as_deref_mut() {
            let mut observer = SitlMavlinkObserver { recorder };
            self.transport.upload_and_execute_mission_observed(
                waypoints,
                upload_options,
                lifecycle_options,
                &mut observer,
            )
        } else {
            self.transport
                .upload_and_execute_mission(waypoints, upload_options, lifecycle_options)
        }
        .map_err(|error| flight_error_to_execution_failure(waypoints.len(), error))?;
        Ok(SitlMissionStartReport {
            uploaded_count: report.upload.uploaded_count,
            armed: report.lifecycle.armed,
            took_off: report.lifecycle.took_off,
            started: report.lifecycle.started,
            post_start_heartbeat: report.lifecycle.post_start_heartbeat,
            abort_result: report.lifecycle.abort_result,
        })
    }

    fn run_telemetry_progress(
        &mut self,
        plan: &SitlPlan,
        lifecycle: &LifecycleArgs,
        lifecycle_options: &swarm_comms::MissionLifecycleOptions,
        mission_item_count: usize,
    ) -> Result<swarm_examples::sitl_progress::SitlMissionProgressReport, SitlExecutionFailure>
    {
        run_telemetry_progress_loop(
            self.transport,
            plan,
            lifecycle,
            lifecycle_options,
            self.recorder.as_deref_mut(),
        )
        .map_err(|error| telemetry_error_to_execution_failure(mission_item_count, error))
    }

    fn record_run_completed(&mut self, status: &str) {
        if let Some(recorder) = self.recorder.as_deref_mut() {
            recorder.push_run_completed(status);
        }
    }

    fn record_failure(&mut self, status: &str, error: &str) {
        if let Some(recorder) = self.recorder.as_deref_mut() {
            recorder.push_failure(status, error);
        }
    }
}

#[cfg(feature = "mavlink-transport")]
struct SitlGoldenPathRun<'a> {
    plan: &'a SitlPlan,
    waypoints: &'a [Waypoint],
    connection_string: &'a str,
    upload_options: swarm_comms::MissionUploadOptions,
    lifecycle_options: swarm_comms::MissionLifecycleOptions,
    lifecycle: &'a LifecycleArgs,
    run_report: Option<&'a str>,
}

#[cfg(feature = "mavlink-transport")]
fn run_golden_path_with_driver<D: SitlGoldenPathDriver>(
    driver: &mut D,
    run: SitlGoldenPathRun<'_>,
) -> Result<(), SitlError> {
    let execution = execute_sitl_golden_path_with_driver(
        driver,
        run.waypoints,
        run.upload_options,
        run.lifecycle_options,
        run.plan,
        run.lifecycle,
    );
    match execution {
        Ok(success) => {
            driver.record_run_completed("completed");
            let report =
                success_run_report(run.plan, run.connection_string, &success.progress_report);
            write_run_report_if_requested(run.run_report, &report)?;
            eprintln!(
                "Real MAVLink mode: mission complete; uploaded_count={} completed={} failed={} total={}",
                success.uploaded_count,
                success.progress_report.completed_count,
                success.progress_report.failed_count,
                success.progress_report.total_tasks
            );
            Ok(())
        }
        Err(failure) => {
            driver.record_failure(sitl_run_status_name(&failure.final_status), &failure.error);
            let report = failure_run_report(run.plan, run.connection_string, &failure);
            write_run_report_if_requested(run.run_report, &report)?;
            Err(SitlError::ConnectionFailed {
                message: failure.error,
            })
        }
    }
}

#[cfg(feature = "mavlink-transport")]
fn execute_sitl_golden_path_with_driver<D: SitlGoldenPathDriver>(
    driver: &mut D,
    waypoints: &[Waypoint],
    upload_options: swarm_comms::MissionUploadOptions,
    lifecycle_options: swarm_comms::MissionLifecycleOptions,
    plan: &SitlPlan,
    lifecycle: &LifecycleArgs,
) -> Result<SitlExecutionSuccess, SitlExecutionFailure> {
    let report =
        driver.upload_and_start_mission(waypoints, upload_options, lifecycle_options.clone())?;
    eprintln!(
        "Real MAVLink mode: mission started; uploaded_count={} armed={} took_off={} started={} post_start_heartbeat={} abort_result={:?}",
        report.uploaded_count,
        report.armed,
        report.took_off,
        report.started,
        report.post_start_heartbeat,
        report.abort_result
    );
    if let Some(abort_result) = report.abort_result {
        let error =
            format!("mission aborted before telemetry completion; abort_result={abort_result:?}");
        return Err(SitlExecutionFailure {
            final_status: SitlRunFinalStatus::Aborted,
            mission_item_count: waypoints.len(),
            completed_count: 0,
            failed_count: waypoints.len(),
            error,
            abort_result: Some(format!("{abort_result:?}")),
        });
    }
    let progress_report =
        driver.run_telemetry_progress(plan, lifecycle, &lifecycle_options, waypoints.len())?;
    Ok(SitlExecutionSuccess {
        uploaded_count: report.uploaded_count,
        progress_report,
    })
}

#[cfg(feature = "mavlink-transport")]
fn flight_error_to_execution_failure(
    mission_item_count: usize,
    error: swarm_comms::MavlinkFlightError,
) -> SitlExecutionFailure {
    let final_status = match &error {
        swarm_comms::MavlinkFlightError::MissionUpload(
            swarm_comms::MavlinkMissionError::MissionRejected(_),
        ) => SitlRunFinalStatus::Rejected,
        _ => SitlRunFinalStatus::Failed,
    };
    let abort_result = match &error {
        swarm_comms::MavlinkFlightError::Lifecycle(error) => lifecycle_abort_result(error),
        _ => None,
    };
    SitlExecutionFailure {
        final_status,
        mission_item_count,
        completed_count: 0,
        failed_count: 0,
        error: error.to_string(),
        abort_result,
    }
}

#[cfg(feature = "mavlink-transport")]
fn lifecycle_abort_result(error: &swarm_comms::MavlinkLifecycleError) -> Option<String> {
    match error {
        swarm_comms::MavlinkLifecycleError::CommandAckTimeout { abort_result, .. }
        | swarm_comms::MavlinkLifecycleError::CommandRejected { abort_result, .. } => {
            abort_result.as_ref().map(format_abort_result)
        }
        swarm_comms::MavlinkLifecycleError::PostStartHeartbeatTimeout { abort_result }
        | swarm_comms::MavlinkLifecycleError::AbortFailed { abort_result } => {
            Some(format_abort_result(abort_result))
        }
        swarm_comms::MavlinkLifecycleError::InvalidTakeoffAltitude { .. }
        | swarm_comms::MavlinkLifecycleError::WriteFailed(_)
        | swarm_comms::MavlinkLifecycleError::ReadFailed(_) => None,
    }
}

#[cfg(feature = "mavlink-transport")]
fn format_abort_result(abort_result: &swarm_comms::AbortCommandResult) -> String {
    format!("{abort_result:?}")
}

#[cfg(feature = "mavlink-transport")]
fn telemetry_error_to_execution_failure(
    mission_item_count: usize,
    error: SitlTelemetryLoopError,
) -> SitlExecutionFailure {
    match error {
        SitlTelemetryLoopError::Failed {
            report,
            abort_result,
        } => SitlExecutionFailure {
            final_status: progress_status_to_run_status(report.final_status),
            mission_item_count,
            completed_count: report.completed_count,
            failed_count: report.failed_count,
            error: report
                .failure_reason
                .unwrap_or_else(|| "SITL telemetry failure".to_owned()),
            abort_result: Some(format!("{abort_result:?}")),
        },
        error => SitlExecutionFailure {
            final_status: SitlRunFinalStatus::Failed,
            mission_item_count,
            completed_count: 0,
            failed_count: 0,
            error: error.to_string(),
            abort_result: None,
        },
    }
}

#[cfg(feature = "mavlink-transport")]
fn progress_status_to_run_status(
    status: swarm_examples::sitl_progress::SitlMissionFinalStatus,
) -> SitlRunFinalStatus {
    match status {
        swarm_examples::sitl_progress::SitlMissionFinalStatus::Completed => {
            SitlRunFinalStatus::Completed
        }
        swarm_examples::sitl_progress::SitlMissionFinalStatus::Failed => SitlRunFinalStatus::Failed,
        swarm_examples::sitl_progress::SitlMissionFinalStatus::Disconnected => {
            SitlRunFinalStatus::Disconnected
        }
        swarm_examples::sitl_progress::SitlMissionFinalStatus::Rejected => {
            SitlRunFinalStatus::Rejected
        }
        swarm_examples::sitl_progress::SitlMissionFinalStatus::TimedOutNoProgress => {
            SitlRunFinalStatus::TimedOutNoProgress
        }
    }
}

#[cfg(feature = "mavlink-transport")]
fn success_run_report(
    plan: &SitlPlan,
    connection_string: &str,
    progress_report: &swarm_examples::sitl_progress::SitlMissionProgressReport,
) -> SitlRunReport {
    SitlRunReport {
        schema_version: "sitl_run_report.v1".to_owned(),
        scenario_path: plan.scenario_path.clone(),
        scenario_name: plan.scenario_name.clone(),
        mission: plan.mission.clone(),
        profile: plan.profile.clone(),
        agent_id: plan.agent_id.clone(),
        connection_string: connection_string.to_owned(),
        mode: SitlRunMode::ConnectionExecute,
        mission_item_count: plan.waypoints.len(),
        completed_count: progress_report.completed_count,
        failed_count: progress_report.failed_count,
        final_status: SitlRunFinalStatus::Completed,
        error: None,
        abort_result: None,
    }
}

#[cfg(feature = "mavlink-transport")]
fn failure_run_report(
    plan: &SitlPlan,
    connection_string: &str,
    failure: &SitlExecutionFailure,
) -> SitlRunReport {
    SitlRunReport {
        schema_version: "sitl_run_report.v1".to_owned(),
        scenario_path: plan.scenario_path.clone(),
        scenario_name: plan.scenario_name.clone(),
        mission: plan.mission.clone(),
        profile: plan.profile.clone(),
        agent_id: plan.agent_id.clone(),
        connection_string: connection_string.to_owned(),
        mode: SitlRunMode::ConnectionExecute,
        mission_item_count: failure.mission_item_count,
        completed_count: failure.completed_count,
        failed_count: failure.failed_count,
        final_status: failure.final_status.clone(),
        error: Some(failure.error.clone()),
        abort_result: failure.abort_result.clone(),
    }
}

#[cfg(feature = "mavlink-transport")]
fn sitl_run_status_name(status: &SitlRunFinalStatus) -> &'static str {
    match status {
        SitlRunFinalStatus::Completed => "completed",
        SitlRunFinalStatus::Failed => "failed",
        SitlRunFinalStatus::Disconnected => "disconnected",
        SitlRunFinalStatus::Rejected => "rejected",
        SitlRunFinalStatus::TimedOutNoProgress => "timed_out_no_progress",
        SitlRunFinalStatus::Aborted => "aborted",
    }
}

#[cfg(feature = "mavlink-transport")]
fn write_run_report_if_requested(
    path: Option<&str>,
    report: &SitlRunReport,
) -> Result<(), SitlError> {
    let Some(path) = path else {
        return Ok(());
    };
    write_sitl_run_report(path, report).map_err(|error| SitlError::RunReportWrite {
        path: Path::new(path).to_path_buf(),
        message: error.to_string(),
    })?;
    eprintln!("SITL run report written: {path}");
    Ok(())
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, thiserror::Error)]
enum SitlTelemetryLoopError {
    #[error("SITL progress mapping failed: {0}")]
    Progress(#[from] swarm_examples::sitl_progress::SitlProgressError),
    #[error("SITL telemetry failed: {0}")]
    Telemetry(#[from] swarm_comms::MavlinkTelemetryError),
    #[error("SITL telemetry failure: report={report:?}; abort_result={abort_result:?}")]
    Failed {
        report: swarm_examples::sitl_progress::SitlMissionProgressReport,
        abort_result: swarm_comms::AbortCommandResult,
    },
}

#[cfg(feature = "mavlink-transport")]
fn run_telemetry_progress_loop(
    transport: &mut swarm_comms::MavlinkTransport,
    plan: &SitlPlan,
    lifecycle: &LifecycleArgs,
    lifecycle_options: &swarm_comms::MissionLifecycleOptions,
    recorder: Option<&mut SitlEventRecorder>,
) -> Result<swarm_examples::sitl_progress::SitlMissionProgressReport, SitlTelemetryLoopError> {
    let mut runtime = MavlinkTelemetryRuntime {
        transport,
        started_at: Instant::now(),
    };
    run_telemetry_progress_loop_with_runtime(
        &mut runtime,
        plan,
        lifecycle,
        lifecycle_options,
        recorder,
    )
}

#[cfg(feature = "mavlink-transport")]
trait SitlTelemetryRuntime {
    fn poll_telemetry_event(
        &mut self,
    ) -> Result<Option<swarm_comms::MavlinkTelemetryEvent>, swarm_comms::MavlinkTelemetryError>;
    fn abort_mission(
        &mut self,
        options: &swarm_comms::MissionLifecycleOptions,
    ) -> swarm_comms::AbortCommandResult;
    fn sleep(&mut self, duration: Duration);
    fn elapsed(&self) -> Duration;
}

#[cfg(feature = "mavlink-transport")]
struct MavlinkTelemetryRuntime<'a> {
    transport: &'a mut swarm_comms::MavlinkTransport,
    started_at: Instant,
}

#[cfg(feature = "mavlink-transport")]
impl SitlTelemetryRuntime for MavlinkTelemetryRuntime<'_> {
    fn poll_telemetry_event(
        &mut self,
    ) -> Result<Option<swarm_comms::MavlinkTelemetryEvent>, swarm_comms::MavlinkTelemetryError>
    {
        self.transport.poll_telemetry_event()
    }

    fn abort_mission(
        &mut self,
        options: &swarm_comms::MissionLifecycleOptions,
    ) -> swarm_comms::AbortCommandResult {
        self.transport.abort_mission(options)
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn elapsed(&self) -> Duration {
        Instant::now().duration_since(self.started_at)
    }
}

#[cfg(feature = "mavlink-transport")]
fn run_telemetry_progress_loop_with_runtime<R: SitlTelemetryRuntime>(
    runtime: &mut R,
    plan: &SitlPlan,
    lifecycle: &LifecycleArgs,
    lifecycle_options: &swarm_comms::MissionLifecycleOptions,
    mut recorder: Option<&mut SitlEventRecorder>,
) -> Result<swarm_examples::sitl_progress::SitlMissionProgressReport, SitlTelemetryLoopError> {
    let mut monitor = SitlTelemetryMonitor::from_plan(plan, lifecycle)?;

    loop {
        if let Some(report) = monitor.check_timeouts(runtime.elapsed())? {
            let abort_result = runtime.abort_mission(lifecycle_options);
            record_telemetry_failure(recorder.as_deref_mut(), &report, &abort_result);
            return Err(SitlTelemetryLoopError::Failed {
                report,
                abort_result,
            });
        }

        if let Some(event) = runtime.poll_telemetry_event()? {
            let previous_seq = monitor.current_seq();
            let previous_completed_count = monitor.completed_count();
            let step = monitor.apply_event(event.clone(), runtime.elapsed())?;
            let current_seq_changed = match &event {
                swarm_comms::MavlinkTelemetryEvent::MissionCurrent { seq } => {
                    previous_seq != Some(*seq)
                }
                _ => false,
            };
            let task_completed = monitor.completed_count() > previous_completed_count;
            record_telemetry_step(
                recorder.as_deref_mut(),
                plan,
                &event,
                &step,
                current_seq_changed,
                task_completed,
            );
            match step {
                SitlTelemetryLoopStep::Continue(update) => {
                    print_progress_update(update);
                }
                SitlTelemetryLoopStep::Completed(report) => return Ok(report),
                SitlTelemetryLoopStep::Failed(report) => {
                    let abort_result = runtime.abort_mission(lifecycle_options);
                    record_telemetry_failure(recorder.as_deref_mut(), &report, &abort_result);
                    return Err(SitlTelemetryLoopError::Failed {
                        report,
                        abort_result,
                    });
                }
            }
            if let Some(report) = monitor.check_timeouts(runtime.elapsed())? {
                let abort_result = runtime.abort_mission(lifecycle_options);
                record_telemetry_failure(recorder.as_deref_mut(), &report, &abort_result);
                return Err(SitlTelemetryLoopError::Failed {
                    report,
                    abort_result,
                });
            }
        } else {
            runtime.sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(feature = "mavlink-transport")]
fn record_telemetry_step(
    recorder: Option<&mut SitlEventRecorder>,
    plan: &SitlPlan,
    event: &swarm_comms::MavlinkTelemetryEvent,
    step: &SitlTelemetryLoopStep,
    current_seq_changed: bool,
    task_completed: bool,
) {
    let Some(recorder) = recorder else {
        return;
    };
    match event {
        swarm_comms::MavlinkTelemetryEvent::Heartbeat => {
            recorder.push_heartbeat_seen();
        }
        swarm_comms::MavlinkTelemetryEvent::MissionCurrent { seq } => {
            if !current_seq_changed {
                return;
            }
            let task_id = match step {
                SitlTelemetryLoopStep::Continue(
                    swarm_examples::sitl_progress::SitlProgressUpdate::Current { task_id, .. },
                ) => Some(task_id.clone()),
                _ => None,
            };
            recorder.push_current_seq_changed(*seq, task_id);
        }
        swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq } => {
            let task_id = sitl_task_id_for_seq(plan, *seq);
            recorder.push_waypoint_reached(*seq, task_id.clone());
            if task_completed {
                if let Some(task_id) = task_id {
                    recorder.push_task_completed(*seq, task_id);
                }
            }
        }
        swarm_comms::MavlinkTelemetryEvent::MissionComplete
        | swarm_comms::MavlinkTelemetryEvent::MissionRejected { .. }
        | swarm_comms::MavlinkTelemetryEvent::Disconnected => {}
    }
}

#[cfg(feature = "mavlink-transport")]
fn sitl_task_id_for_seq(plan: &SitlPlan, seq: u16) -> Option<String> {
    plan.waypoints
        .iter()
        .find(|waypoint| waypoint.seq == seq)
        .map(|waypoint| waypoint.task_id.clone())
}

#[cfg(feature = "mavlink-transport")]
fn record_telemetry_failure(
    recorder: Option<&mut SitlEventRecorder>,
    report: &swarm_examples::sitl_progress::SitlMissionProgressReport,
    abort_result: &swarm_comms::AbortCommandResult,
) {
    let Some(recorder) = recorder else {
        return;
    };
    let status = progress_final_status_name(report.final_status);
    if matches!(
        report.final_status,
        swarm_examples::sitl_progress::SitlMissionFinalStatus::Disconnected
    ) {
        recorder.push_disconnected(
            report
                .failure_reason
                .clone()
                .unwrap_or_else(|| "telemetry disconnected".to_owned()),
        );
    }
    recorder.push_abort_requested(Some(format!("{abort_result:?}")));
    recorder.push_failure(
        status,
        report
            .failure_reason
            .clone()
            .unwrap_or_else(|| "SITL telemetry failure".to_owned()),
    );
}

#[cfg(feature = "mavlink-transport")]
fn progress_final_status_name(
    status: swarm_examples::sitl_progress::SitlMissionFinalStatus,
) -> &'static str {
    match status {
        swarm_examples::sitl_progress::SitlMissionFinalStatus::Completed => "completed",
        swarm_examples::sitl_progress::SitlMissionFinalStatus::Failed => "failed",
        swarm_examples::sitl_progress::SitlMissionFinalStatus::Disconnected => "disconnected",
        swarm_examples::sitl_progress::SitlMissionFinalStatus::Rejected => "rejected",
        swarm_examples::sitl_progress::SitlMissionFinalStatus::TimedOutNoProgress => {
            "timed_out_no_progress"
        }
    }
}

#[cfg(feature = "mavlink-transport")]
struct SitlTelemetryMonitor {
    progress: swarm_examples::sitl_progress::SitlTaskProgress,
    telemetry_timeout: Duration,
    no_progress_timeout: Duration,
    last_heartbeat_at: Duration,
    last_progress_at: Duration,
}

#[cfg(feature = "mavlink-transport")]
impl SitlTelemetryMonitor {
    fn from_plan(
        plan: &SitlPlan,
        lifecycle: &LifecycleArgs,
    ) -> Result<Self, swarm_examples::sitl_progress::SitlProgressError> {
        Ok(Self {
            progress: swarm_examples::sitl_progress::SitlTaskProgress::from_plan(plan)?,
            telemetry_timeout: lifecycle.telemetry_timeout,
            no_progress_timeout: lifecycle.no_progress_timeout,
            last_heartbeat_at: Duration::ZERO,
            last_progress_at: Duration::ZERO,
        })
    }

    fn apply_event(
        &mut self,
        event: swarm_comms::MavlinkTelemetryEvent,
        now: Duration,
    ) -> Result<SitlTelemetryLoopStep, swarm_examples::sitl_progress::SitlProgressError> {
        let is_heartbeat = matches!(event, swarm_comms::MavlinkTelemetryEvent::Heartbeat);
        let mission_current_seq = match &event {
            swarm_comms::MavlinkTelemetryEvent::MissionCurrent { seq } => Some(*seq),
            _ => None,
        };
        let is_waypoint_reached = matches!(
            event,
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { .. }
        );
        let is_mission_complete =
            matches!(event, swarm_comms::MavlinkTelemetryEvent::MissionComplete);
        let previous_seq = self.progress.current_seq();
        let previous_completed_count = self.progress.completed_count();

        let update = self.progress.apply_event(event, now)?;

        if is_heartbeat {
            self.last_heartbeat_at = now;
        }
        if mission_current_seq.is_some_and(|seq| previous_seq != Some(seq))
            || (is_waypoint_reached && self.progress.completed_count() > previous_completed_count)
            || (is_mission_complete
                && matches!(
                    update,
                    swarm_examples::sitl_progress::SitlProgressUpdate::Completed(_)
                ))
        {
            self.last_progress_at = now;
        }

        Ok(match update {
            swarm_examples::sitl_progress::SitlProgressUpdate::Completed(report) => {
                SitlTelemetryLoopStep::Completed(report)
            }
            swarm_examples::sitl_progress::SitlProgressUpdate::Failed(report) => {
                SitlTelemetryLoopStep::Failed(report)
            }
            update => SitlTelemetryLoopStep::Continue(update),
        })
    }

    fn check_timeouts(
        &mut self,
        now: Duration,
    ) -> Result<
        Option<swarm_examples::sitl_progress::SitlMissionProgressReport>,
        swarm_examples::sitl_progress::SitlProgressError,
    > {
        if now.saturating_sub(self.last_heartbeat_at) >= self.telemetry_timeout {
            let update = self.apply_event(swarm_comms::MavlinkTelemetryEvent::Disconnected, now)?;
            let SitlTelemetryLoopStep::Failed(report) = update else {
                unreachable!("disconnected event must fail SITL progress");
            };
            return Ok(Some(report));
        }
        if now.saturating_sub(self.last_progress_at) >= self.no_progress_timeout {
            return Ok(Some(self.progress.mark_no_progress_timeout(format!(
                "no mission progress before {:?}",
                self.no_progress_timeout
            ))));
        }
        Ok(None)
    }

    fn current_seq(&self) -> Option<u16> {
        self.progress.current_seq()
    }

    fn completed_count(&self) -> usize {
        self.progress.completed_count()
    }
}

#[cfg(feature = "mavlink-transport")]
enum SitlTelemetryLoopStep {
    Continue(swarm_examples::sitl_progress::SitlProgressUpdate),
    Completed(swarm_examples::sitl_progress::SitlMissionProgressReport),
    Failed(swarm_examples::sitl_progress::SitlMissionProgressReport),
}

#[cfg(feature = "mavlink-transport")]
fn print_progress_update(update: swarm_examples::sitl_progress::SitlProgressUpdate) {
    match update {
        swarm_examples::sitl_progress::SitlProgressUpdate::Heartbeat => {
            eprintln!("progress: heartbeat");
        }
        swarm_examples::sitl_progress::SitlProgressUpdate::Current {
            seq,
            task_id,
            completed_count,
            total_count,
        } => {
            eprintln!(
                "progress: current seq={seq} task_id={task_id} completed={completed_count}/{total_count}"
            );
        }
        swarm_examples::sitl_progress::SitlProgressUpdate::Reached {
            seq,
            task_id,
            completed_count,
            total_count,
        } => {
            eprintln!(
                "progress: reached seq={seq} task_id={task_id} completed={completed_count}/{total_count}"
            );
        }
        swarm_examples::sitl_progress::SitlProgressUpdate::Completed(_)
        | swarm_examples::sitl_progress::SitlProgressUpdate::Failed(_) => {
            unreachable!("terminal progress updates are handled before printing");
        }
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

    #[cfg(feature = "mavlink-transport")]
    use mavlink::dialects::common;
    #[cfg(feature = "mavlink-transport")]
    use std::collections::VecDeque;
    #[cfg(feature = "mavlink-transport")]
    use std::path::PathBuf;

    #[test]
    fn connection_string_validation_accepts_udp() {
        validate_connection_string("udp:127.0.0.1:14550").unwrap();
    }

    #[test]
    fn connection_string_validation_rejects_unknown_scheme() {
        let error = validate_connection_string("bad").unwrap_err();
        assert!(matches!(error, SitlError::BadConnectionString { .. }));
    }

    #[cfg(feature = "mavlink-transport")]
    struct FakeTelemetryRuntime {
        events: VecDeque<swarm_comms::MavlinkTelemetryEvent>,
        now: Duration,
        poll_step: Duration,
        abort_attempts: usize,
        abort_result: swarm_comms::AbortCommandResult,
    }

    #[cfg(feature = "mavlink-transport")]
    impl FakeTelemetryRuntime {
        fn new(events: impl IntoIterator<Item = swarm_comms::MavlinkTelemetryEvent>) -> Self {
            Self {
                events: events.into_iter().collect(),
                now: Duration::ZERO,
                poll_step: Duration::ZERO,
                abort_attempts: 0,
                abort_result: swarm_comms::AbortCommandResult::Accepted,
            }
        }

        fn with_poll_step(mut self, poll_step: Duration) -> Self {
            self.poll_step = poll_step;
            self
        }
    }

    #[cfg(feature = "mavlink-transport")]
    impl SitlTelemetryRuntime for FakeTelemetryRuntime {
        fn poll_telemetry_event(
            &mut self,
        ) -> Result<Option<swarm_comms::MavlinkTelemetryEvent>, swarm_comms::MavlinkTelemetryError>
        {
            self.now += self.poll_step;
            Ok(self.events.pop_front())
        }

        fn abort_mission(
            &mut self,
            _options: &swarm_comms::MissionLifecycleOptions,
        ) -> swarm_comms::AbortCommandResult {
            self.abort_attempts += 1;
            self.abort_result.clone()
        }

        fn sleep(&mut self, duration: Duration) {
            self.now += duration;
        }

        fn elapsed(&self) -> Duration {
            self.now
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn test_plan() -> SitlPlan {
        SitlPlan {
            agent_id: "agent-0".to_owned(),
            scenario_path: PathBuf::from("scenario.json"),
            suite_name: "SITL Waypoints".to_owned(),
            scenario_name: "sitl_waypoints_test".to_owned(),
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            coordinate_frame: swarm_examples::sitl_plan::SitlCoordinateFrame::LocalSimulation,
            altitude_source: "pose.z".to_owned(),
            waypoints: vec![
                swarm_examples::sitl_plan::SitlWaypointItem {
                    seq: 0,
                    task_id: "wp-0".to_owned(),
                    x: 10.0,
                    y: 20.0,
                    z: 3.0,
                },
                swarm_examples::sitl_plan::SitlWaypointItem {
                    seq: 1,
                    task_id: "wp-1".to_owned(),
                    x: 30.0,
                    y: 40.0,
                    z: 4.0,
                },
            ],
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_args(telemetry_timeout: Duration, no_progress_timeout: Duration) -> LifecycleArgs {
        LifecycleArgs {
            mode: LifecycleMode::Execute,
            no_arm: false,
            abort_after: None,
            timeout: Duration::from_millis(1),
            telemetry_timeout,
            no_progress_timeout,
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn completed_progress_report() -> swarm_examples::sitl_progress::SitlMissionProgressReport {
        swarm_examples::sitl_progress::SitlMissionProgressReport {
            final_status: swarm_examples::sitl_progress::SitlMissionFinalStatus::Completed,
            total_tasks: 2,
            completed_count: 2,
            failed_count: 0,
            current_task_id: Some("wp-1".to_owned()),
            failure_reason: None,
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn test_waypoints() -> Vec<Waypoint> {
        test_plan()
            .waypoints
            .iter()
            .map(|waypoint| Waypoint {
                x: waypoint.x,
                y: waypoint.y,
                z: waypoint.z,
                seq: waypoint.seq,
            })
            .collect()
    }

    #[cfg(feature = "mavlink-transport")]
    fn mission_start_success() -> SitlMissionStartReport {
        SitlMissionStartReport {
            uploaded_count: 2,
            armed: true,
            took_off: true,
            started: true,
            post_start_heartbeat: true,
            abort_result: None,
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn fake_execution_failure(
        final_status: SitlRunFinalStatus,
        error: &str,
    ) -> SitlExecutionFailure {
        SitlExecutionFailure {
            final_status,
            mission_item_count: 2,
            completed_count: 0,
            failed_count: 2,
            error: error.to_owned(),
            abort_result: None,
        }
    }

    #[cfg(feature = "mavlink-transport")]
    struct FakeGoldenPathDriver {
        start_result: Result<SitlMissionStartReport, SitlExecutionFailure>,
        telemetry_result:
            Result<swarm_examples::sitl_progress::SitlMissionProgressReport, SitlExecutionFailure>,
        upload_calls: usize,
        telemetry_calls: usize,
        last_upload_waypoint_count: Option<usize>,
        last_telemetry_mission_item_count: Option<usize>,
    }

    #[cfg(feature = "mavlink-transport")]
    impl FakeGoldenPathDriver {
        fn new(
            start_result: Result<SitlMissionStartReport, SitlExecutionFailure>,
            telemetry_result: Result<
                swarm_examples::sitl_progress::SitlMissionProgressReport,
                SitlExecutionFailure,
            >,
        ) -> Self {
            Self {
                start_result,
                telemetry_result,
                upload_calls: 0,
                telemetry_calls: 0,
                last_upload_waypoint_count: None,
                last_telemetry_mission_item_count: None,
            }
        }
    }

    #[cfg(feature = "mavlink-transport")]
    impl SitlGoldenPathDriver for FakeGoldenPathDriver {
        fn upload_and_start_mission(
            &mut self,
            waypoints: &[Waypoint],
            _upload_options: swarm_comms::MissionUploadOptions,
            _lifecycle_options: swarm_comms::MissionLifecycleOptions,
        ) -> Result<SitlMissionStartReport, SitlExecutionFailure> {
            self.upload_calls += 1;
            self.last_upload_waypoint_count = Some(waypoints.len());
            self.start_result.clone()
        }

        fn run_telemetry_progress(
            &mut self,
            _plan: &SitlPlan,
            _lifecycle: &LifecycleArgs,
            _lifecycle_options: &swarm_comms::MissionLifecycleOptions,
            mission_item_count: usize,
        ) -> Result<swarm_examples::sitl_progress::SitlMissionProgressReport, SitlExecutionFailure>
        {
            self.telemetry_calls += 1;
            self.last_telemetry_mission_item_count = Some(mission_item_count);
            self.telemetry_result.clone()
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn test_golden_path_run<'a>(
        plan: &'a SitlPlan,
        waypoints: &'a [Waypoint],
        lifecycle: &'a LifecycleArgs,
        run_report: Option<&'a str>,
    ) -> SitlGoldenPathRun<'a> {
        SitlGoldenPathRun {
            plan,
            waypoints,
            connection_string: "udp:127.0.0.1:14550",
            upload_options: swarm_comms::MissionUploadOptions::default(),
            lifecycle_options: swarm_comms::MissionLifecycleOptions::default(),
            lifecycle,
            run_report,
        }
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_golden_path_driver_success_writes_completed_report() {
        let plan = test_plan();
        let waypoints = test_waypoints();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let report_dir = tempfile::tempdir().unwrap();
        let report_path = report_dir.path().join("nested").join("report.json");
        let mut driver =
            FakeGoldenPathDriver::new(Ok(mission_start_success()), Ok(completed_progress_report()));

        run_golden_path_with_driver(
            &mut driver,
            test_golden_path_run(&plan, &waypoints, &lifecycle, report_path.to_str()),
        )
        .unwrap();

        assert_eq!(driver.upload_calls, 1);
        assert_eq!(driver.telemetry_calls, 1);
        assert_eq!(driver.last_upload_waypoint_count, Some(2));
        assert_eq!(driver.last_telemetry_mission_item_count, Some(2));
        let json = std::fs::read_to_string(report_path).unwrap();
        let report: SitlRunReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.final_status, SitlRunFinalStatus::Completed);
        assert_eq!(report.completed_count, 2);
        assert_eq!(report.failed_count, 0);
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_golden_path_driver_upload_failure_writes_error_report() {
        let plan = test_plan();
        let waypoints = test_waypoints();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let report_dir = tempfile::tempdir().unwrap();
        let report_path = report_dir.path().join("report.json");
        let failure =
            fake_execution_failure(SitlRunFinalStatus::Rejected, "mission rejected by vehicle");
        let mut driver = FakeGoldenPathDriver::new(Err(failure), Ok(completed_progress_report()));

        let error = run_golden_path_with_driver(
            &mut driver,
            test_golden_path_run(&plan, &waypoints, &lifecycle, report_path.to_str()),
        )
        .unwrap_err();

        assert!(matches!(error, SitlError::ConnectionFailed { .. }));
        assert_eq!(driver.upload_calls, 1);
        assert_eq!(driver.telemetry_calls, 0);
        let json = std::fs::read_to_string(report_path).unwrap();
        let report: SitlRunReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.final_status, SitlRunFinalStatus::Rejected);
        assert_eq!(report.error, Some("mission rejected by vehicle".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_golden_path_driver_lifecycle_abort_writes_aborted_report() {
        let plan = test_plan();
        let waypoints = test_waypoints();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let report_dir = tempfile::tempdir().unwrap();
        let report_path = report_dir.path().join("report.json");
        let mut start = mission_start_success();
        start.abort_result = Some(swarm_comms::AbortCommandResult::Accepted);
        let mut driver = FakeGoldenPathDriver::new(Ok(start), Ok(completed_progress_report()));

        let error = run_golden_path_with_driver(
            &mut driver,
            test_golden_path_run(&plan, &waypoints, &lifecycle, report_path.to_str()),
        )
        .unwrap_err();

        assert!(matches!(error, SitlError::ConnectionFailed { .. }));
        assert_eq!(driver.upload_calls, 1);
        assert_eq!(driver.telemetry_calls, 0);
        let json = std::fs::read_to_string(report_path).unwrap();
        let report: SitlRunReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.final_status, SitlRunFinalStatus::Aborted);
        assert_eq!(report.abort_result, Some("Accepted".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_golden_path_driver_telemetry_failure_writes_error_report() {
        let plan = test_plan();
        let waypoints = test_waypoints();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let report_dir = tempfile::tempdir().unwrap();
        let report_path = report_dir.path().join("report.json");
        let mut failure = fake_execution_failure(
            SitlRunFinalStatus::TimedOutNoProgress,
            "no mission progress before 60s",
        );
        failure.completed_count = 1;
        failure.failed_count = 1;
        failure.abort_result = Some("Accepted".to_owned());
        let mut driver = FakeGoldenPathDriver::new(Ok(mission_start_success()), Err(failure));

        let error = run_golden_path_with_driver(
            &mut driver,
            test_golden_path_run(&plan, &waypoints, &lifecycle, report_path.to_str()),
        )
        .unwrap_err();

        assert!(matches!(error, SitlError::ConnectionFailed { .. }));
        assert_eq!(driver.upload_calls, 1);
        assert_eq!(driver.telemetry_calls, 1);
        let json = std::fs::read_to_string(report_path).unwrap();
        let report: SitlRunReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.final_status, SitlRunFinalStatus::TimedOutNoProgress);
        assert_eq!(report.completed_count, 1);
        assert_eq!(report.failed_count, 1);
        assert_eq!(report.abort_result, Some("Accepted".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_golden_path_summary_builds_success_report() {
        let plan = test_plan();

        let report = success_run_report(&plan, "udp:127.0.0.1:14550", &completed_progress_report());

        assert_eq!(report.scenario_name, "sitl_waypoints_test");
        assert_eq!(report.agent_id, "agent-0");
        assert_eq!(report.mission_item_count, 2);
        assert_eq!(report.completed_count, 2);
        assert_eq!(report.failed_count, 0);
        assert_eq!(report.final_status, SitlRunFinalStatus::Completed);
        assert_eq!(report.error, None);
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_failure_summary_builds_error_report() {
        let plan = test_plan();
        let failure = SitlExecutionFailure {
            final_status: SitlRunFinalStatus::Disconnected,
            mission_item_count: 2,
            completed_count: 1,
            failed_count: 1,
            error: "telemetry disconnected".to_owned(),
            abort_result: Some("Accepted".to_owned()),
        };

        let report = failure_run_report(&plan, "udp:127.0.0.1:14550", &failure);

        assert_eq!(report.final_status, SitlRunFinalStatus::Disconnected);
        assert_eq!(report.completed_count, 1);
        assert_eq!(report.failed_count, 1);
        assert_eq!(report.error, Some("telemetry disconnected".to_owned()));
        assert_eq!(report.abort_result, Some("Accepted".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn upload_failure_maps_to_failed_report_failure() {
        let error = swarm_comms::MavlinkFlightError::MissionUpload(
            swarm_comms::MavlinkMissionError::MissionRequestTimeout { expected_seq: 1 },
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(failure.mission_item_count, 2);
        assert_eq!(failure.completed_count, 0);
        assert!(failure.error.contains("mission upload failed"));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_failure_maps_to_failed_report_failure() {
        let error = swarm_comms::MavlinkFlightError::Lifecycle(
            swarm_comms::MavlinkLifecycleError::ReadFailed("ack read failed".to_owned()),
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(failure.mission_item_count, 2);
        assert!(failure.error.contains("mission lifecycle failed"));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_command_ack_timeout_keeps_abort_result() {
        let error = swarm_comms::MavlinkFlightError::Lifecycle(
            swarm_comms::MavlinkLifecycleError::CommandAckTimeout {
                command: common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                abort_result: Some(swarm_comms::AbortCommandResult::Accepted),
            },
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(failure.abort_result, Some("Accepted".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_command_rejected_keeps_abort_failure() {
        let error = swarm_comms::MavlinkFlightError::Lifecycle(
            swarm_comms::MavlinkLifecycleError::CommandRejected {
                command: common::MavCmd::MAV_CMD_MISSION_START,
                result: common::MavResult::MAV_RESULT_DENIED,
                abort_result: Some(swarm_comms::AbortCommandResult::Failed(
                    "rtl write failed".to_owned(),
                )),
            },
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(
            failure.abort_result,
            Some("Failed(\"rtl write failed\")".to_owned())
        );
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_post_start_timeout_keeps_abort_result() {
        let error = swarm_comms::MavlinkFlightError::Lifecycle(
            swarm_comms::MavlinkLifecycleError::PostStartHeartbeatTimeout {
                abort_result: swarm_comms::AbortCommandResult::Accepted,
            },
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(failure.abort_result, Some("Accepted".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_abort_failed_keeps_abort_result() {
        let error = swarm_comms::MavlinkFlightError::Lifecycle(
            swarm_comms::MavlinkLifecycleError::AbortFailed {
                abort_result: swarm_comms::AbortCommandResult::AckTimeout,
            },
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(failure.abort_result, Some("AckTimeout".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn telemetry_failure_keeps_progress_counts_and_abort_result() {
        let progress_report = swarm_examples::sitl_progress::SitlMissionProgressReport {
            final_status: swarm_examples::sitl_progress::SitlMissionFinalStatus::TimedOutNoProgress,
            total_tasks: 2,
            completed_count: 1,
            failed_count: 1,
            current_task_id: Some("wp-1".to_owned()),
            failure_reason: Some("no mission progress before 60s".to_owned()),
        };
        let error = SitlTelemetryLoopError::Failed {
            report: progress_report,
            abort_result: swarm_comms::AbortCommandResult::Accepted,
        };

        let failure = telemetry_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::TimedOutNoProgress);
        assert_eq!(failure.completed_count, 1);
        assert_eq!(failure.failed_count, 1);
        assert_eq!(failure.abort_result, Some("Accepted".to_owned()));
        assert_eq!(failure.error, "no mission progress before 60s");
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn report_write_failure_returns_typed_error() {
        let dir = tempfile::tempdir().unwrap();
        let plan = test_plan();
        let report = success_run_report(&plan, "udp:127.0.0.1:14550", &completed_progress_report());

        let error = write_run_report_if_requested(dir.path().to_str(), &report).unwrap_err();

        match error {
            SitlError::RunReportWrite { path, message } => {
                assert_eq!(path, dir.path());
                assert!(!message.is_empty());
            }
            error => panic!("expected run report write error, got {error:?}"),
        }
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn telemetry_loop_recorder_tracks_terminal_waypoint_completion() {
        let plan = test_plan();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let lifecycle_options = swarm_comms::MissionLifecycleOptions::default();
        let mut runtime = FakeTelemetryRuntime::new([
            swarm_comms::MavlinkTelemetryEvent::Heartbeat,
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 1 },
        ]);
        let mut recorder = new_sitl_event_recorder(
            &plan,
            Some("udp:127.0.0.1:14550"),
            SitlEventLogMode::ConnectionExecute,
        );

        let report = run_telemetry_progress_loop_with_runtime(
            &mut runtime,
            &plan,
            &lifecycle,
            &lifecycle_options,
            Some(&mut recorder),
        )
        .unwrap();

        assert_eq!(report.completed_count, 2);
        let summary = swarm_examples::sitl_observability::summarize_sitl_event_log(recorder.log());
        assert_eq!(summary.waypoint_reached, 2);
        assert_eq!(summary.task_completed, 2);
        let final_waypoint_task_id =
            recorder
                .log()
                .events
                .iter()
                .rev()
                .find_map(|event| match event {
                    swarm_examples::sitl_observability::SitlEvent::WaypointReached {
                        seq,
                        task_id,
                        ..
                    } if *seq == 1 => Some(task_id.clone()),
                    _ => None,
                });
        assert_eq!(final_waypoint_task_id, Some(Some("wp-1".to_owned())));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn telemetry_loop_recorder_logs_current_seq_only_on_change() {
        let plan = test_plan();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let lifecycle_options = swarm_comms::MissionLifecycleOptions::default();
        let mut runtime = FakeTelemetryRuntime::new([
            swarm_comms::MavlinkTelemetryEvent::MissionCurrent { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::MissionCurrent { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::MissionCurrent { seq: 1 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 1 },
        ]);
        let mut recorder = new_sitl_event_recorder(
            &plan,
            Some("udp:127.0.0.1:14550"),
            SitlEventLogMode::ConnectionExecute,
        );

        run_telemetry_progress_loop_with_runtime(
            &mut runtime,
            &plan,
            &lifecycle,
            &lifecycle_options,
            Some(&mut recorder),
        )
        .unwrap();

        let summary = swarm_examples::sitl_observability::summarize_sitl_event_log(recorder.log());
        assert_eq!(summary.current_seq_changed, 2);
        let current_seq_events: Vec<_> = recorder
            .log()
            .events
            .iter()
            .filter_map(|event| match event {
                swarm_examples::sitl_observability::SitlEvent::CurrentSeqChanged {
                    seq,
                    task_id,
                    ..
                } => Some((*seq, task_id.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            current_seq_events,
            vec![(0, Some("wp-0".to_owned())), (1, Some("wp-1".to_owned()))]
        );
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn telemetry_loop_duplicate_reached_does_not_reset_no_progress_timeout() {
        let plan = test_plan();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(3));
        let lifecycle_options = swarm_comms::MissionLifecycleOptions::default();
        let mut runtime = FakeTelemetryRuntime::new([
            swarm_comms::MavlinkTelemetryEvent::Heartbeat,
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
        ])
        .with_poll_step(Duration::from_secs(1));

        let error = run_telemetry_progress_loop_with_runtime(
            &mut runtime,
            &plan,
            &lifecycle,
            &lifecycle_options,
            None,
        )
        .unwrap_err();

        let SitlTelemetryLoopError::Failed {
            report,
            abort_result,
        } = error
        else {
            panic!("expected telemetry loop failure");
        };
        assert_eq!(
            report.final_status,
            swarm_examples::sitl_progress::SitlMissionFinalStatus::TimedOutNoProgress
        );
        assert_eq!(report.completed_count, 1);
        assert_eq!(report.failed_count, 1);
        assert_eq!(abort_result, swarm_comms::AbortCommandResult::Accepted);
        assert_eq!(runtime.abort_attempts, 1);
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn telemetry_loop_disconnect_timeout_attempts_abort() {
        let plan = test_plan();
        let lifecycle = lifecycle_args(Duration::from_millis(20), Duration::from_secs(30));
        let lifecycle_options = swarm_comms::MissionLifecycleOptions::default();
        let mut runtime = FakeTelemetryRuntime::new([]);

        let error = run_telemetry_progress_loop_with_runtime(
            &mut runtime,
            &plan,
            &lifecycle,
            &lifecycle_options,
            None,
        )
        .unwrap_err();

        let SitlTelemetryLoopError::Failed {
            report,
            abort_result,
        } = error
        else {
            panic!("expected telemetry loop failure");
        };
        assert_eq!(
            report.final_status,
            swarm_examples::sitl_progress::SitlMissionFinalStatus::Disconnected
        );
        assert_eq!(report.completed_count, 0);
        assert_eq!(report.failed_count, 2);
        assert_eq!(abort_result, swarm_comms::AbortCommandResult::Accepted);
        assert_eq!(runtime.abort_attempts, 1);
    }
}
