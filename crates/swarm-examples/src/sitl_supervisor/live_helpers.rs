#[cfg(any(feature = "mavlink-transport", test))]
use std::collections::HashSet;

#[cfg(any(feature = "mavlink-transport", test))]
use super::config::LiveAgentRun;
use super::config::SupervisorLiveConfig;
#[cfg(any(feature = "mavlink-transport", test))]
use super::ports::LiveAgentController;
#[cfg(any(feature = "mavlink-transport", test))]
use super::reallocation::task_ids_from_completed_waypoints;
use crate::sitl_multi_agent::{MultiAgentLifecycle, MultiAgentSitlManifest};
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_plan::SitlWaypointItem;
use crate::sitl_plan::{classify_connection_string, SitlConnectionClass, SitlError};

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn validate_live_controller_set<C: LiveAgentController>(
    manifest: &MultiAgentSitlManifest,
    controllers: &[C],
) -> Result<(), SitlError> {
    let expected: HashSet<&str> = manifest
        .agents
        .iter()
        .map(|agent| agent.agent_id.as_str())
        .collect();
    let mut seen = HashSet::new();

    for controller in controllers {
        let agent_id = controller.agent_id();
        if !expected.contains(agent_id) {
            return Err(SitlError::MultiAgentConfigInvalid {
                message: format!("controller '{agent_id}' is not present in manifest"),
            });
        }
        if !seen.insert(agent_id.to_owned()) {
            return Err(SitlError::MultiAgentConfigInvalid {
                message: format!("duplicate controller for manifest agent '{agent_id}'"),
            });
        }
    }

    for agent_id in expected {
        if !seen.contains(agent_id) {
            return Err(SitlError::MultiAgentConfigInvalid {
                message: format!("missing controller for manifest agent '{agent_id}'"),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_live_manifest(
    manifest: &MultiAgentSitlManifest,
    config: &SupervisorLiveConfig,
) -> Result<(), SitlError> {
    for agent in &manifest.agents {
        if agent.lifecycle != MultiAgentLifecycle::Execute {
            return Err(SitlError::MultiAgentConfigInvalid {
                message: format!(
                    "live supervisor execute requires lifecycle=execute for agent '{}'",
                    agent.agent_id
                ),
            });
        }
        let class = classify_connection_string(&agent.connection_string)?;
        if class == SitlConnectionClass::HardwareCandidate && !config.allow_hardware_candidate {
            return Err(SitlError::HardwareCandidateRequiresExplicitAllow {
                addr: agent.connection_string.clone(),
                class: class.name(),
            });
        }
    }
    Ok(())
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn live_controller_for_agent_mut<'a, C: LiveAgentController>(
    controllers: &'a mut [C],
    agent_id: &str,
) -> Result<&'a mut C, SitlError> {
    controllers
        .iter_mut()
        .find(|controller| controller.agent_id() == agent_id)
        .ok_or_else(|| SitlError::MultiAgentConfigInvalid {
            message: format!("missing controller for manifest agent '{agent_id}'"),
        })
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn live_active_run_snapshots<C: LiveAgentController>(
    manifest: &MultiAgentSitlManifest,
    controllers: &[C],
    active_agent_ids: &[String],
) -> Vec<LiveAgentRun> {
    active_agent_ids
        .iter()
        .filter_map(|agent_id| {
            let controller = controllers
                .iter()
                .find(|controller| controller.agent_id() == agent_id)?;
            let agent = manifest
                .agents
                .iter()
                .find(|agent| agent.agent_id == *agent_id)?;
            let completed_waypoints = controller.completed_waypoints();
            let completed_task_ids = task_ids_from_completed_waypoints(&completed_waypoints);
            Some(LiveAgentRun {
                agent_id: agent.agent_id.clone(),
                connection_string: agent.connection_string.clone(),
                system_id: agent.system_id,
                component_id: agent.component_id,
                lifecycle: agent.lifecycle,
                mission_item_count: controller.mission_waypoints().len(),
                completed_task_count: completed_waypoints.len(),
                completed_waypoints,
                completed_task_ids,
                final_status: "running".to_owned(),
                error: None,
                failure_mode: None,
                detected_after_ms: None,
                tasks_abandoned: Vec::new(),
            })
        })
        .collect()
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn manifest_waypoint_for_task_id<'a>(
    manifest: &'a MultiAgentSitlManifest,
    task_id: &str,
) -> Option<&'a SitlWaypointItem> {
    manifest
        .agents
        .iter()
        .flat_map(|agent| agent.waypoints.iter())
        .find(|waypoint| waypoint.task_id == task_id)
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn live_overall_status(
    runs: &[LiveAgentRun],
    manifest: &MultiAgentSitlManifest,
) -> &'static str {
    if runs.iter().all(|run| run.final_status == "completed") {
        "completed"
    } else if runs
        .iter()
        .map(|run| run.completed_task_count)
        .sum::<usize>()
        >= manifest.ownership_summary.assigned_task_count
    {
        "completed_with_reallocation"
    } else if runs.iter().any(|run| run.completed_task_count > 0) {
        "partial_failed"
    } else {
        "failed"
    }
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn event_advances_progress(
    event: &swarm_comms::MavlinkTelemetryEvent,
    previous_seq: Option<u16>,
    previous_completed_count: usize,
    update: &crate::sitl_progress::SitlProgressUpdate,
) -> bool {
    match event {
        swarm_comms::MavlinkTelemetryEvent::MissionCurrent { seq } => previous_seq != Some(*seq),
        swarm_comms::MavlinkTelemetryEvent::WaypointReached { .. } => match update {
            crate::sitl_progress::SitlProgressUpdate::Reached {
                completed_count, ..
            }
            | crate::sitl_progress::SitlProgressUpdate::Completed(
                crate::sitl_progress::SitlMissionProgressReport {
                    completed_count, ..
                },
            ) => *completed_count > previous_completed_count,
            _ => false,
        },
        swarm_comms::MavlinkTelemetryEvent::MissionComplete => matches!(
            update,
            crate::sitl_progress::SitlProgressUpdate::Completed(_)
        ),
        swarm_comms::MavlinkTelemetryEvent::Heartbeat
        | swarm_comms::MavlinkTelemetryEvent::MissionRejected { .. }
        | swarm_comms::MavlinkTelemetryEvent::Disconnected => false,
    }
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn append_abort_to_report(
    mut report: crate::sitl_progress::SitlMissionProgressReport,
    abort: swarm_comms::AbortCommandResult,
) -> crate::sitl_progress::SitlMissionProgressReport {
    let abort_message = format!("abort_result={abort:?}");
    report.failure_reason = Some(match report.failure_reason.take() {
        Some(reason) => format!("{reason}; {abort_message}"),
        None => abort_message,
    });
    report
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn live_progress_status_name(
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
