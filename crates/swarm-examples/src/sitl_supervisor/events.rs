#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_multi_agent::MultiAgentSitlManifest;
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_observability::SitlEventRecorder;

#[cfg(any(feature = "mavlink-transport", test))]
use super::config::{LiveAgentRun, MissionReplacementPlan};
#[cfg(any(feature = "mavlink-transport", test))]
use super::reallocation::completed_waypoints_for_run;

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn record_replacement_mission_items(
    recorder: &mut SitlEventRecorder,
    plan: &MissionReplacementPlan,
) {
    recorder
        .push_multi_agent_mission_count_sent(plan.target_agent_id.clone(), plan.waypoints.len());
    for waypoint in &plan.waypoints {
        recorder.push_multi_agent_mission_item_sent(
            plan.target_agent_id.clone(),
            waypoint.seq,
            Some(waypoint.task_id.clone()),
        );
    }
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn record_live_agent_run(
    recorder: &mut SitlEventRecorder,
    manifest: &MultiAgentSitlManifest,
    run: &LiveAgentRun,
) {
    let completed_waypoints = completed_waypoints_for_run(manifest, run);
    for waypoint in completed_waypoints {
        recorder.push_multi_agent_waypoint_reached(
            run.agent_id.clone(),
            waypoint.seq,
            Some(waypoint.task_id.clone()),
        );
        recorder.push_multi_agent_task_completed(
            run.agent_id.clone(),
            waypoint.seq,
            waypoint.task_id.clone(),
        );
    }
    if run.final_status != "completed" {
        recorder.push_multi_agent_failure(
            run.agent_id.clone(),
            run.final_status.clone(),
            run.error
                .clone()
                .unwrap_or_else(|| "agent did not complete mission".to_owned()),
        );
    }
}
