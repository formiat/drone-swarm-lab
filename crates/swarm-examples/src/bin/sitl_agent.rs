use std::path::Path;
use std::time::Duration;
#[cfg(feature = "mavlink-transport")]
use std::time::Instant;

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
    telemetry_timeout: Duration,
    no_progress_timeout: Duration,
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
            telemetry_timeout,
            no_progress_timeout,
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
            "usage: sitl_agent --mock|--dry-run|--connection <addr> --scenario <path> --agent-id <id> [--safety-config <path>] [--upload-only|--execute] [--no-arm] [--abort-after <seconds>] [--timeout <seconds>] [--telemetry-timeout <seconds>] [--no-progress-timeout <seconds>]"
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
                    .upload_and_execute_mission(
                        &waypoints,
                        upload_options,
                        lifecycle_options.clone(),
                    )
                    .map_err(|error| SitlError::ConnectionFailed {
                        message: error.to_string(),
                    })?;
                eprintln!(
                    "Real MAVLink mode: mission started; uploaded_count={} armed={} took_off={} started={} post_start_heartbeat={} abort_result={:?}",
                    report.upload.uploaded_count,
                    report.lifecycle.armed,
                    report.lifecycle.took_off,
                    report.lifecycle.started,
                    report.lifecycle.post_start_heartbeat,
                    report.lifecycle.abort_result
                );
                if let Some(abort_result) = report.lifecycle.abort_result {
                    return Err(SitlError::ConnectionFailed {
                        message: format!(
                            "mission aborted before telemetry completion; abort_result={abort_result:?}"
                        ),
                    });
                }
                let progress_report = run_telemetry_progress_loop(
                    &mut transport,
                    plan,
                    lifecycle,
                    &lifecycle_options,
                )
                .map_err(|error| SitlError::ConnectionFailed {
                    message: error.to_string(),
                })?;
                eprintln!(
                    "Real MAVLink mode: mission complete; completed={} failed={} total={}",
                    progress_report.completed_count,
                    progress_report.failed_count,
                    progress_report.total_tasks
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
            lifecycle.telemetry_timeout,
            lifecycle.no_progress_timeout,
        );
        Err(SitlError::FeatureMissing {
            feature: "mavlink-transport",
        })
    }
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
) -> Result<swarm_examples::sitl_progress::SitlMissionProgressReport, SitlTelemetryLoopError> {
    let mut runtime = MavlinkTelemetryRuntime {
        transport,
        started_at: Instant::now(),
    };
    run_telemetry_progress_loop_with_runtime(&mut runtime, plan, lifecycle, lifecycle_options)
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
) -> Result<swarm_examples::sitl_progress::SitlMissionProgressReport, SitlTelemetryLoopError> {
    let mut monitor = SitlTelemetryMonitor::from_plan(plan, lifecycle)?;

    loop {
        if let Some(report) = monitor.check_timeouts(runtime.elapsed())? {
            return Err(SitlTelemetryLoopError::Failed {
                report,
                abort_result: runtime.abort_mission(lifecycle_options),
            });
        }

        if let Some(event) = runtime.poll_telemetry_event()? {
            match monitor.apply_event(event, runtime.elapsed())? {
                SitlTelemetryLoopStep::Continue(update) => {
                    print_progress_update(update);
                }
                SitlTelemetryLoopStep::Completed(report) => return Ok(report),
                SitlTelemetryLoopStep::Failed(report) => {
                    return Err(SitlTelemetryLoopError::Failed {
                        report,
                        abort_result: runtime.abort_mission(lifecycle_options),
                    });
                }
            }
            if let Some(report) = monitor.check_timeouts(runtime.elapsed())? {
                return Err(SitlTelemetryLoopError::Failed {
                    report,
                    abort_result: runtime.abort_mission(lifecycle_options),
                });
            }
        } else {
            runtime.sleep(Duration::from_millis(10));
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
