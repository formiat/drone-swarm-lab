use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::sitl_observability::{
    write_sitl_event_log, SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
};
use crate::sitl_plan::{SitlError, SitlPlan};
use swarm_comms::{MockMavlinkTransport, Waypoint};

pub(super) fn apply_start_delay(start_delay_ms: u64) {
    if start_delay_ms > 0 {
        thread::sleep(Duration::from_millis(start_delay_ms));
    }
}

pub(super) fn run_mock(plan: &SitlPlan, replay_log: Option<&str>) -> Result<(), SitlError> {
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
