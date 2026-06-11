#[cfg(feature = "mavlink-transport")]
use std::path::Path;

#[cfg(feature = "mavlink-transport")]
use crate::sitl_plan::{SitlError, SitlPlan};
#[cfg(feature = "mavlink-transport")]
use crate::sitl_report::{write_sitl_run_report, SitlRunFinalStatus, SitlRunMode, SitlRunReport};

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

#[cfg(feature = "mavlink-transport")]
pub(super) fn write_execution_artifact_if_requested(
    run_report_path: Option<&str>,
    artifact: &swarm_comms::MavlinkExecutionArtifact,
) -> Result<(), SitlError> {
    let Some(run_report_path) = run_report_path else {
        return Ok(());
    };
    let run_report_path = Path::new(run_report_path);
    let artifact_path = run_report_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("mavlink_execution_artifact.v1.json");
    let content =
        serde_json::to_string_pretty(artifact).map_err(|error| SitlError::RunReportWrite {
            path: artifact_path.clone(),
            message: error.to_string(),
        })?;
    if let Some(parent) = artifact_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| SitlError::RunReportWrite {
            path: artifact_path.clone(),
            message: error.to_string(),
        })?;
    }
    std::fs::write(&artifact_path, content).map_err(|error| SitlError::RunReportWrite {
        path: artifact_path.clone(),
        message: error.to_string(),
    })?;
    eprintln!(
        "MAVLink execution artifact written: {}",
        artifact_path.display()
    );
    Ok(())
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn sitl_run_status_name(status: &SitlRunFinalStatus) -> &'static str {
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
pub(super) fn progress_status_to_run_status(
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
