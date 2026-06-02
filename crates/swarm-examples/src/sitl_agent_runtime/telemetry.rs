#[cfg(feature = "mavlink-transport")]
use std::collections::BTreeMap;
#[cfg(feature = "mavlink-transport")]
use std::time::{Duration, Instant};

#[cfg(feature = "mavlink-transport")]
use super::cli::LifecycleArgs;
#[cfg(feature = "mavlink-transport")]
use crate::sitl_observability::SitlEventRecorder;
#[cfg(feature = "mavlink-transport")]
use crate::sitl_plan::SitlPlan;
#[cfg(feature = "mavlink-transport")]
use swarm_comms::Waypoint;

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, thiserror::Error)]
pub(super) enum SitlTelemetryLoopError {
    #[error("SITL progress mapping failed: {0}")]
    Progress(#[from] crate::sitl_progress::SitlProgressError),
    #[error("SITL telemetry failed: {0}")]
    Telemetry(#[from] swarm_comms::MavlinkTelemetryError),
    #[error("SITL telemetry failure: report={report:?}; abort_result={abort_result:?}")]
    Failed {
        report: crate::sitl_progress::SitlMissionProgressReport,
        abort_result: swarm_comms::AbortCommandResult,
    },
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn run_telemetry_progress_loop(
    transport: &mut swarm_comms::MavlinkTransport,
    plan: &SitlPlan,
    lifecycle: &LifecycleArgs,
    lifecycle_options: &swarm_comms::MissionLifecycleOptions,
    recorder: Option<&mut SitlEventRecorder>,
) -> Result<crate::sitl_progress::SitlMissionProgressReport, SitlTelemetryLoopError> {
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
pub(super) trait SitlTelemetryRuntime {
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
pub(super) fn run_telemetry_progress_loop_with_runtime<R: SitlTelemetryRuntime>(
    runtime: &mut R,
    plan: &SitlPlan,
    lifecycle: &LifecycleArgs,
    lifecycle_options: &swarm_comms::MissionLifecycleOptions,
    mut recorder: Option<&mut SitlEventRecorder>,
) -> Result<crate::sitl_progress::SitlMissionProgressReport, SitlTelemetryLoopError> {
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
                    crate::sitl_progress::SitlProgressUpdate::Current { task_id, .. },
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
pub(super) fn sitl_task_ids_by_seq(plan: &SitlPlan) -> BTreeMap<u16, String> {
    plan.waypoints
        .iter()
        .map(|waypoint| (waypoint.seq, waypoint.task_id.clone()))
        .collect()
}

#[cfg(feature = "mavlink-transport")]
fn record_telemetry_failure(
    recorder: Option<&mut SitlEventRecorder>,
    report: &crate::sitl_progress::SitlMissionProgressReport,
    abort_result: &swarm_comms::AbortCommandResult,
) {
    let Some(recorder) = recorder else {
        return;
    };
    let status = progress_final_status_name(report.final_status);
    if matches!(
        report.final_status,
        crate::sitl_progress::SitlMissionFinalStatus::Disconnected
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
    status: crate::sitl_progress::SitlMissionFinalStatus,
) -> &'static str {
    match status {
        crate::sitl_progress::SitlMissionFinalStatus::Completed => "completed",
        crate::sitl_progress::SitlMissionFinalStatus::Failed => "failed",
        crate::sitl_progress::SitlMissionFinalStatus::Disconnected => "disconnected",
        crate::sitl_progress::SitlMissionFinalStatus::Rejected => "rejected",
        crate::sitl_progress::SitlMissionFinalStatus::TimedOutNoProgress => "timed_out_no_progress",
    }
}

#[cfg(feature = "mavlink-transport")]
struct SitlTelemetryMonitor {
    progress: crate::sitl_progress::SitlTaskProgress,
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
    ) -> Result<Self, crate::sitl_progress::SitlProgressError> {
        Ok(Self {
            progress: crate::sitl_progress::SitlTaskProgress::from_plan(plan)?,
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
    ) -> Result<SitlTelemetryLoopStep, crate::sitl_progress::SitlProgressError> {
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
                    crate::sitl_progress::SitlProgressUpdate::Completed(_)
                ))
        {
            self.last_progress_at = now;
        }

        Ok(match update {
            crate::sitl_progress::SitlProgressUpdate::Completed(report) => {
                SitlTelemetryLoopStep::Completed(report)
            }
            crate::sitl_progress::SitlProgressUpdate::Failed(report) => {
                SitlTelemetryLoopStep::Failed(report)
            }
            update => SitlTelemetryLoopStep::Continue(update),
        })
    }

    fn check_timeouts(
        &mut self,
        now: Duration,
    ) -> Result<
        Option<crate::sitl_progress::SitlMissionProgressReport>,
        crate::sitl_progress::SitlProgressError,
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
    Continue(crate::sitl_progress::SitlProgressUpdate),
    Completed(crate::sitl_progress::SitlMissionProgressReport),
    Failed(crate::sitl_progress::SitlMissionProgressReport),
}

#[cfg(feature = "mavlink-transport")]
fn print_progress_update(update: crate::sitl_progress::SitlProgressUpdate) {
    match update {
        crate::sitl_progress::SitlProgressUpdate::Heartbeat => {
            eprintln!("progress: heartbeat");
        }
        crate::sitl_progress::SitlProgressUpdate::Current {
            seq,
            task_id,
            completed_count,
            total_count,
        } => {
            eprintln!(
                "progress: current seq={seq} task_id={task_id} completed={completed_count}/{total_count}"
            );
        }
        crate::sitl_progress::SitlProgressUpdate::Reached {
            seq,
            task_id,
            completed_count,
            total_count,
        } => {
            eprintln!(
                "progress: reached seq={seq} task_id={task_id} completed={completed_count}/{total_count}"
            );
        }
        crate::sitl_progress::SitlProgressUpdate::Completed(_)
        | crate::sitl_progress::SitlProgressUpdate::Failed(_) => {
            unreachable!("terminal progress updates are handled before printing");
        }
    }
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn default_takeoff_altitude(waypoints: &[Waypoint]) -> f32 {
    waypoints
        .first()
        .map(|waypoint| waypoint.z.max(2.5) as f32)
        .unwrap_or(2.5)
}
