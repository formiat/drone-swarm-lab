#![allow(unused_imports)]
use super::*;
pub(super) fn run_connection(
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
        let task_id_by_seq = sitl_task_ids_by_seq(plan);

        let upload_options = MissionUploadOptions {
            target_system: runtime_options.target_system,
            target_component: runtime_options.target_component,
            timeout: lifecycle.timeout,
            ..MissionUploadOptions::default()
        };
        match lifecycle.mode {
            LifecycleMode::UploadOnly => {
                let upload_result = if let Some(recorder) = event_recorder.as_mut() {
                    let mut observer = SitlMavlinkObserver {
                        recorder,
                        task_id_by_seq: &task_id_by_seq,
                    };
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
                        task_id_by_seq,
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
pub(super) struct SitlExecutionSuccess {
    pub(super) uploaded_count: usize,
    pub(super) progress_report: crate::sitl_progress::SitlMissionProgressReport,
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SitlExecutionFailure {
    pub(super) final_status: SitlRunFinalStatus,
    pub(super) mission_item_count: usize,
    pub(super) completed_count: usize,
    pub(super) failed_count: usize,
    pub(super) error: String,
    pub(super) abort_result: Option<String>,
}

#[cfg(feature = "mavlink-transport")]
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SitlMissionStartReport {
    pub(super) uploaded_count: usize,
    pub(super) armed: bool,
    pub(super) took_off: bool,
    pub(super) started: bool,
    pub(super) post_start_heartbeat: bool,
    pub(super) abort_result: Option<swarm_comms::AbortCommandResult>,
}

#[cfg(feature = "mavlink-transport")]
pub(super) trait SitlGoldenPathDriver {
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
    ) -> Result<crate::sitl_progress::SitlMissionProgressReport, SitlExecutionFailure>;

    fn record_run_completed(&mut self, _status: &str) {}

    fn record_failure(&mut self, _status: &str, _error: &str) {}
}

#[cfg(feature = "mavlink-transport")]
pub(super) struct MavlinkGoldenPathDriver<'a> {
    pub(super) transport: &'a mut swarm_comms::MavlinkTransport,
    pub(super) recorder: Option<&'a mut SitlEventRecorder>,
    pub(super) task_id_by_seq: BTreeMap<u16, String>,
}

#[cfg(feature = "mavlink-transport")]
pub(super) struct SitlMavlinkObserver<'a> {
    pub(super) recorder: &'a mut SitlEventRecorder,
    pub(super) task_id_by_seq: &'a BTreeMap<u16, String>,
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
                self.recorder
                    .push_mission_item_sent(seq, self.task_id_by_seq.get(&seq).cloned());
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
            let mut observer = SitlMavlinkObserver {
                recorder,
                task_id_by_seq: &self.task_id_by_seq,
            };
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
    ) -> Result<crate::sitl_progress::SitlMissionProgressReport, SitlExecutionFailure> {
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
pub(super) struct SitlGoldenPathRun<'a> {
    pub(super) plan: &'a SitlPlan,
    pub(super) waypoints: &'a [Waypoint],
    pub(super) connection_string: &'a str,
    pub(super) upload_options: swarm_comms::MissionUploadOptions,
    pub(super) lifecycle_options: swarm_comms::MissionLifecycleOptions,
    pub(super) lifecycle: &'a LifecycleArgs,
    pub(super) run_report: Option<&'a str>,
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn run_golden_path_with_driver<D: SitlGoldenPathDriver>(
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
pub(super) fn flight_error_to_execution_failure(
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
pub(super) fn telemetry_error_to_execution_failure(
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
    status: crate::sitl_progress::SitlMissionFinalStatus,
) -> SitlRunFinalStatus {
    match status {
        crate::sitl_progress::SitlMissionFinalStatus::Completed => SitlRunFinalStatus::Completed,
        crate::sitl_progress::SitlMissionFinalStatus::Failed => SitlRunFinalStatus::Failed,
        crate::sitl_progress::SitlMissionFinalStatus::Disconnected => {
            SitlRunFinalStatus::Disconnected
        }
        crate::sitl_progress::SitlMissionFinalStatus::Rejected => SitlRunFinalStatus::Rejected,
        crate::sitl_progress::SitlMissionFinalStatus::TimedOutNoProgress => {
            SitlRunFinalStatus::TimedOutNoProgress
        }
    }
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn success_run_report(
    plan: &SitlPlan,
    connection_string: &str,
    progress_report: &crate::sitl_progress::SitlMissionProgressReport,
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
pub(super) fn failure_run_report(
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
pub(super) fn write_run_report_if_requested(
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
