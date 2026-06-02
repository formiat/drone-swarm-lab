use std::collections::HashSet;
use std::thread;
use std::time::Duration;

use super::config::SupervisorMockConfig;
use super::mock::MockAgentController;
use super::ports::AgentController;
use crate::sitl_multi_agent::MultiAgentSitlManifest;
use crate::sitl_observability::SitlEventRecorder;
use crate::sitl_plan::SitlError;

pub(super) fn build_mock_controllers(
    manifest: &MultiAgentSitlManifest,
    config: &SupervisorMockConfig,
) -> Vec<MockAgentController> {
    manifest
        .agents
        .iter()
        .map(|agent| {
            let fail_after_ticks = if Some(agent.agent_id.as_str()) == config.fail_agent.as_deref()
            {
                Some(config.fail_after_ticks)
            } else {
                None
            };
            MockAgentController::new(agent, fail_after_ticks)
        })
        .collect()
}

pub(super) fn upload_and_start_manifest_agents<C: AgentController>(
    manifest: &MultiAgentSitlManifest,
    controllers: &mut [C],
    recorder: &mut SitlEventRecorder,
    mode_label: &str,
) -> Result<(), SitlError> {
    for agent in &manifest.agents {
        if agent.start_delay_ms > 0 {
            thread::sleep(Duration::from_millis(agent.start_delay_ms));
        }
        eprintln!(
            "SITL Supervisor: agent={} system_id={} component_id={} connection={} waypoints={}",
            agent.agent_id,
            agent.system_id,
            agent.component_id,
            agent.connection_string,
            agent.waypoint_count
        );
        recorder.push_multi_agent_mission_count_sent(agent.agent_id.clone(), agent.waypoint_count);
        for waypoint in &agent.waypoints {
            recorder.push_multi_agent_mission_item_sent(
                agent.agent_id.clone(),
                waypoint.seq,
                Some(waypoint.task_id.clone()),
            );
            eprintln!(
                "WAYPOINT agent={} seq={} task_id={} x={:.1} y={:.1} z={:.1}",
                agent.agent_id, waypoint.seq, waypoint.task_id, waypoint.x, waypoint.y, waypoint.z
            );
        }

        let controller = controller_for_agent_mut(controllers, &agent.agent_id)?;
        let upload = controller.upload(&agent.waypoints)?;
        controller.start()?;
        eprintln!(
            "{} mode: agent={} waypoints sent={}",
            mode_label, agent.agent_id, upload.waypoint_count
        );
    }
    Ok(())
}

pub(super) fn controller_for_agent_mut<'a, C: AgentController>(
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

pub(super) fn validate_controller_set<C: AgentController>(
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

pub(super) fn poll_active_agent_ids<C: AgentController>(
    controllers: &mut [C],
    tick: u64,
) -> Result<Vec<String>, SitlError> {
    let mut active_agents = Vec::new();
    for controller in controllers {
        let progress = controller.poll(tick)?;
        if progress.heartbeat_seen {
            active_agents.push(progress.agent_id);
        }
    }
    Ok(active_agents)
}
