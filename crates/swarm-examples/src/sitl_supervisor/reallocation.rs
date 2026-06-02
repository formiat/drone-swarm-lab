use std::collections::{HashMap, HashSet};

use super::config::SupervisorMetrics;
#[cfg(any(feature = "mavlink-transport", test))]
use super::config::{CompletedWaypoint, LiveAgentRun, MissionReplacementPlan};
#[cfg(any(feature = "mavlink-transport", test))]
use super::validation_and_reports::{assign_manifest_tasks, manifest_waypoint_for_task_id};
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_multi_agent::MultiAgentSitlManifest;
use crate::sitl_observability::SitlEventRecorder;
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_plan::{SitlError, SitlWaypointItem};
#[cfg(any(feature = "mavlink-transport", test))]
use swarm_alloc::GreedyAllocator;
#[cfg(any(feature = "mavlink-transport", test))]
use swarm_comms::{MockMavlinkTransport, RawMessage};
use swarm_runtime::NodeTickOutput;
#[cfg(any(feature = "mavlink-transport", test))]
use swarm_runtime::{AgentNode, Coordinator, RuntimeMessage};
#[cfg(any(feature = "mavlink-transport", test))]
use swarm_types::{AgentId, TaskId};

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) struct LiveReallocationContext<'a> {
    pub(super) entry: &'a swarm_sim::ScenarioSuiteEntry,
    pub(super) manifest: &'a MultiAgentSitlManifest,
    pub(super) finished_runs: &'a [LiveAgentRun],
    pub(super) active_runs: &'a [LiveAgentRun],
    pub(super) failed_run: &'a LiveAgentRun,
    pub(super) survivor_ids: &'a [String],
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn live_reallocation_after_failure(
    context: LiveReallocationContext<'_>,
    recorder: &mut SitlEventRecorder,
    metrics: &mut SupervisorMetrics,
) -> Result<Vec<MissionReplacementPlan>, SitlError> {
    let survivor_id = context
        .survivor_ids
        .iter()
        .find(|agent_id| agent_id.as_str() != context.failed_run.agent_id)
        .cloned()
        .ok_or_else(|| SitlError::MultiAgentConfigInvalid {
            message: format!(
                "cannot reallocate failed agent '{}' without an active survivor",
                context.failed_run.agent_id
            ),
        })?;
    let mut known_runs =
        Vec::with_capacity(context.finished_runs.len() + context.active_runs.len());
    known_runs.extend_from_slice(context.finished_runs);
    known_runs.extend_from_slice(context.active_runs);
    let own_agent_id = AgentId::from(survivor_id.clone());
    let peer_ids: Vec<AgentId> = context
        .manifest
        .agents
        .iter()
        .filter(|agent| agent.agent_id != survivor_id)
        .map(|agent| AgentId::from(agent.agent_id.clone()))
        .collect();
    let mut coordinator = Coordinator::new(
        context.entry.scenario.agents.clone(),
        context.entry.scenario.tasks.clone(),
        1,
    );
    assign_manifest_tasks(&mut coordinator, context.manifest)?;

    for task_id in completed_live_task_ids(context.manifest, &known_runs, context.failed_run) {
        let task_id = TaskId::from(task_id);
        let _ = coordinator.registry.complete_assigned_task(&task_id);
    }

    let mut node = AgentNode::new(
        own_agent_id.clone(),
        peer_ids,
        coordinator,
        MockMavlinkTransport::new(),
    );
    node.gossip_interval_ticks = 10;
    let tick = 2;
    for agent_id in context
        .survivor_ids
        .iter()
        .filter(|agent_id| *agent_id != &context.failed_run.agent_id && *agent_id != &survivor_id)
    {
        node.transport.push_incoming(RawMessage {
            from: AgentId::from(agent_id.clone()),
            to: own_agent_id.clone(),
            payload: RuntimeMessage::heartbeat(tick, 1),
        });
    }

    let mut allocator = GreedyAllocator;
    let output = node
        .process_inbox_and_allocate(tick, &mut allocator, Vec::new())
        .map_err(|error| SitlError::ConnectionFailed {
            message: error.to_string(),
        })?;
    let recovered_by_agent = record_reallocation_output(&output, recorder, metrics);
    mission_replacement_plans(
        context.manifest,
        &known_runs,
        context.failed_run,
        recovered_by_agent,
    )
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn completed_live_task_ids(
    manifest: &MultiAgentSitlManifest,
    previous_runs: &[LiveAgentRun],
    failed_run: &LiveAgentRun,
) -> HashSet<String> {
    previous_runs
        .iter()
        .chain(std::iter::once(failed_run))
        .flat_map(|run| completed_task_ids_for_run(manifest, run))
        .collect()
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn completed_task_ids_for_run(
    manifest: &MultiAgentSitlManifest,
    run: &LiveAgentRun,
) -> Vec<String> {
    if !run.completed_waypoints.is_empty() {
        return task_ids_from_completed_waypoints(&run.completed_waypoints);
    }
    if !run.completed_task_ids.is_empty() {
        return run.completed_task_ids.clone();
    }
    manifest
        .agents
        .iter()
        .find(|agent| agent.agent_id == run.agent_id)
        .map(|agent| {
            agent
                .waypoints
                .iter()
                .take(run.completed_task_count)
                .map(|waypoint| waypoint.task_id.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn completed_waypoints_for_run(
    manifest: &MultiAgentSitlManifest,
    run: &LiveAgentRun,
) -> Vec<CompletedWaypoint> {
    if !run.completed_waypoints.is_empty() {
        return run.completed_waypoints.clone();
    }
    if !run.completed_task_ids.is_empty() {
        return run
            .completed_task_ids
            .iter()
            .filter_map(|task_id| manifest_waypoint_for_task_id(manifest, task_id))
            .map(|waypoint| CompletedWaypoint {
                seq: waypoint.seq,
                task_id: waypoint.task_id.clone(),
            })
            .collect();
    }
    manifest
        .agents
        .iter()
        .find(|agent| agent.agent_id == run.agent_id)
        .map(|agent| {
            agent
                .waypoints
                .iter()
                .take(run.completed_task_count)
                .map(|waypoint| CompletedWaypoint {
                    seq: waypoint.seq,
                    task_id: waypoint.task_id.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn record_reallocation_output(
    output: &NodeTickOutput,
    recorder: &mut SitlEventRecorder,
    metrics: &mut SupervisorMetrics,
) -> HashMap<String, Vec<String>> {
    let mut recovered_by_agent: HashMap<String, Vec<String>> = HashMap::new();
    for release in &output.failure_releases {
        metrics.lost_agent_count += 1;
        let failed_agent_id = release.failed_agent_id.to_string();
        recorder.push_agent_lost(failed_agent_id.clone());
        for task_id in &release.released_tasks {
            let task_id = task_id.to_string();
            metrics.released_tasks.push(task_id.clone());
            recorder.push_task_released(task_id, failed_agent_id.clone());
        }
    }
    for assignment in &output.reassigned_tasks {
        if output
            .tasks_recovered
            .iter()
            .any(|task_id| task_id == &assignment.task_id)
        {
            let from_agent_id = output
                .failure_releases
                .iter()
                .find(|release| release.released_tasks.contains(&assignment.task_id))
                .map(|release| release.failed_agent_id.to_string())
                .unwrap_or_else(|| "unknown".to_owned());
            let task_id = assignment.task_id.to_string();
            let to_agent_id = assignment.agent_id.to_string();
            metrics.reassigned_tasks.push(task_id.clone());
            recorder.push_task_reassigned(
                task_id.clone(),
                from_agent_id,
                to_agent_id.clone(),
                output.reallocation_latency_ticks.unwrap_or(0),
            );
            recovered_by_agent
                .entry(to_agent_id)
                .or_default()
                .push(task_id);
        }
    }
    for release in &output.failure_releases {
        let recovered: Vec<String> = output
            .tasks_recovered
            .iter()
            .filter(|task_id| release.released_tasks.contains(task_id))
            .map(ToString::to_string)
            .collect();
        if !recovered.is_empty() {
            recorder.push_reallocation_completed(
                release.failed_agent_id.to_string(),
                recovered.len(),
                recovered.clone(),
                output.reallocation_latency_ticks.unwrap_or(0),
            );
            metrics.reassignment_count += recovered.len() as u64;
            metrics.tasks_recovered.extend(recovered);
            metrics.reallocation_latency_ticks = metrics
                .reallocation_latency_ticks
                .or(output.reallocation_latency_ticks);
        }
    }
    for recovered in recovered_by_agent.values_mut() {
        dedup_strings_preserve_order(recovered);
    }
    recovered_by_agent
}

pub(super) fn dedup_strings_preserve_order(items: &mut Vec<String>) {
    let mut seen = HashSet::new();
    items.retain(|item| seen.insert(item.clone()));
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn completed_waypoints_from_progress(
    progress: &crate::sitl_progress::SitlTaskProgress,
) -> Vec<CompletedWaypoint> {
    progress
        .completed_waypoints()
        .into_iter()
        .map(|(seq, task_id)| CompletedWaypoint { seq, task_id })
        .collect()
}

#[cfg(test)]
pub(super) fn completed_waypoints_from_items(items: &[SitlWaypointItem]) -> Vec<CompletedWaypoint> {
    items
        .iter()
        .map(|waypoint| CompletedWaypoint {
            seq: waypoint.seq,
            task_id: waypoint.task_id.clone(),
        })
        .collect()
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn task_ids_from_completed_waypoints(waypoints: &[CompletedWaypoint]) -> Vec<String> {
    waypoints
        .iter()
        .map(|waypoint| waypoint.task_id.clone())
        .collect()
}

#[cfg(feature = "mavlink-transport")]
pub(super) fn dedup_completed_waypoints_preserve_order(waypoints: &mut Vec<CompletedWaypoint>) {
    let mut seen = HashSet::new();
    waypoints.retain(|waypoint| seen.insert(waypoint.task_id.clone()));
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn mission_replacement_plans(
    manifest: &MultiAgentSitlManifest,
    previous_runs: &[LiveAgentRun],
    failed_run: &LiveAgentRun,
    recovered_by_agent: HashMap<String, Vec<String>>,
) -> Result<Vec<MissionReplacementPlan>, SitlError> {
    let completed = completed_live_task_ids(manifest, previous_runs, failed_run);
    let mut plans = Vec::new();
    for (target_agent_id, recovered_task_ids) in recovered_by_agent {
        if target_agent_id == failed_run.agent_id {
            continue;
        }
        let target_agent = manifest
            .agents
            .iter()
            .find(|agent| agent.agent_id == target_agent_id)
            .ok_or_else(|| SitlError::MultiAgentConfigInvalid {
                message: format!(
                    "reallocation target '{target_agent_id}' is not present in manifest"
                ),
            })?;
        let recovered_task_ids: HashSet<String> = recovered_task_ids.into_iter().collect();

        let mut task_ids = Vec::new();
        for task_id in &target_agent.task_ids {
            push_unique_replacement_task(&mut task_ids, task_id, &completed);
        }
        push_recovered_tasks_in_manifest_order(
            &mut task_ids,
            manifest,
            &recovered_task_ids,
            &completed,
        );
        if task_ids.is_empty() {
            continue;
        }
        let waypoints = replacement_waypoints_for_task_ids(manifest, &task_ids)?;
        plans.push(MissionReplacementPlan {
            target_agent_id,
            failed_agent_id: failed_run.agent_id.clone(),
            policy: "mission_replacement".to_owned(),
            task_ids,
            waypoints,
        });
    }
    plans.sort_by(|left, right| left.target_agent_id.cmp(&right.target_agent_id));
    Ok(plans)
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn push_recovered_tasks_in_manifest_order(
    task_ids: &mut Vec<String>,
    manifest: &MultiAgentSitlManifest,
    recovered_task_ids: &HashSet<String>,
    completed: &HashSet<String>,
) {
    for waypoint in manifest
        .agents
        .iter()
        .flat_map(|agent| agent.waypoints.iter())
    {
        if recovered_task_ids.contains(&waypoint.task_id) {
            push_unique_replacement_task(task_ids, &waypoint.task_id, completed);
        }
    }
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn push_unique_replacement_task(
    task_ids: &mut Vec<String>,
    task_id: &str,
    completed: &HashSet<String>,
) {
    if !completed.contains(task_id) && !task_ids.iter().any(|existing| existing == task_id) {
        task_ids.push(task_id.to_owned());
    }
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn replacement_waypoints_for_task_ids(
    manifest: &MultiAgentSitlManifest,
    task_ids: &[String],
) -> Result<Vec<SitlWaypointItem>, SitlError> {
    task_ids
        .iter()
        .enumerate()
        .map(|(seq, task_id)| {
            let mut waypoint = manifest
                .agents
                .iter()
                .flat_map(|agent| agent.waypoints.iter())
                .find(|waypoint| waypoint.task_id == *task_id)
                .cloned()
                .ok_or_else(|| SitlError::MultiAgentConfigInvalid {
                    message: format!("replacement task_id '{task_id}' is not present in manifest"),
                })?;
            waypoint.seq = u16::try_from(seq).map_err(|_| SitlError::MultiAgentConfigInvalid {
                message: "replacement mission contains more than u16::MAX waypoints".to_owned(),
            })?;
            Ok(waypoint)
        })
        .collect()
}
