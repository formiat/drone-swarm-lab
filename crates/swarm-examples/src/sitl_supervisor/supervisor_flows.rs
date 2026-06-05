#[cfg(any(feature = "mavlink-transport", test))]
use std::collections::{HashMap, HashSet};
use std::path::Path;
#[cfg(any(feature = "mavlink-transport", test))]
use std::thread;
#[cfg(any(feature = "mavlink-transport", test))]
use std::time::Duration;

use super::config::{
    SupervisorLiveConfig, SupervisorLoopConfig, SupervisorMetrics, SupervisorMockConfig,
};
use super::controller_helpers::{
    build_mock_controllers, poll_active_agent_ids, upload_and_start_manifest_agents,
    validate_controller_set,
};
#[cfg(any(feature = "mavlink-transport", test))]
use super::degraded::{
    classify_live_failure, terminal_decision_for_run, DegradedRunRecord, SupervisorDecision,
    SupervisorFailureMode,
};
use super::live_helpers::validate_live_manifest;
#[cfg(any(feature = "mavlink-transport", test))]
use super::live_helpers::{
    live_active_run_snapshots, live_controller_for_agent_mut, live_overall_status,
    validate_live_controller_set,
};
use super::mock_runtime::{
    assign_manifest_tasks, complete_one_task_per_active_agent, manifest_tasks_completed,
    supervisor_runtime_agent_id, validate_failure_agent,
};
use super::ports::AgentController;
#[cfg(any(feature = "mavlink-transport", test))]
use super::ports::LiveAgentController;
#[cfg(not(any(feature = "mavlink-transport", test)))]
use super::reallocation::record_reallocation_output;
#[cfg(any(feature = "mavlink-transport", test))]
use super::reallocation::{
    live_reallocation_after_failure, record_reallocation_output, LiveReallocationContext,
};
use crate::sitl_connection::SitlSafetyGate;
use crate::sitl_multi_agent::MultiAgentSitlManifest;
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_observability::summarize_sitl_event_log;
use crate::sitl_observability::{
    write_sitl_event_log, SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
};
use crate::sitl_plan::{check_preflight_or_err, first_sitl_entry, SitlError};
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_report::write_sitl_multi_agent_run_report;
use crate::sitl_report::SitlMultiAgentRunReport;
use swarm_alloc::GreedyAllocator;
use swarm_command_plane::{
    SwarmCommandPlan, SwarmOwnershipKind, SwarmOwnershipStatus, SynchronizedCommandKind,
};
use swarm_comms::{MockMavlinkTransport, RawMessage};
use swarm_runtime::{AgentNode, Coordinator, RuntimeMessage};
use swarm_types::AgentId;

#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_supervisor::artifacts::{live_run_report, LiveRunReportInput};
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_supervisor::events::{record_live_agent_run, record_replacement_mission_items};
#[cfg(feature = "mavlink-transport")]
use crate::sitl_supervisor::Px4AgentController;

pub fn run_mock_supervisor(
    suite: &swarm_sim::ScenarioSuite,
    config: &SupervisorMockConfig,
    manifest: &MultiAgentSitlManifest,
) -> Result<SupervisorMetrics, SitlError> {
    validate_failure_agent(manifest, config.fail_agent.as_deref())?;
    let entry = first_sitl_entry(suite, &config.scenario_path)?;
    check_preflight_or_err(entry)?;
    let timeout_ticks = config
        .heartbeat_timeout_ticks
        .unwrap_or(entry.run_config.timeout_ticks);
    let max_ticks = config.max_ticks.unwrap_or(
        entry
            .run_config
            .max_ticks
            .max(timeout_ticks + config.fail_after_ticks + 3),
    );
    let own_id = supervisor_runtime_agent_id(manifest, config.fail_agent.as_deref())?;
    let controllers = build_mock_controllers(manifest, config);
    let loop_config = SupervisorLoopConfig {
        replay_log: config.replay_log.as_deref(),
        run_id: config.run_id.as_deref(),
        timeout_ticks,
        max_ticks,
        own_id,
        mode_label: "Mock",
    };
    run_supervisor_with_controllers(entry, manifest, controllers, &loop_config)
}

pub fn run_live_supervisor(
    suite: &swarm_sim::ScenarioSuite,
    config: &SupervisorLiveConfig,
    manifest: &MultiAgentSitlManifest,
) -> Result<SitlMultiAgentRunReport, SitlError> {
    let entry = first_sitl_entry(suite, &config.scenario_path)?;
    check_preflight_or_err(entry)?;
    validate_live_manifest(manifest, config)?;

    let safety_gate = SitlSafetyGate::new(config.safety_config_path.clone());
    for agent in &manifest.agents {
        safety_gate.validate_agent_task_subset(entry, &agent.agent_id, &agent.task_ids)?;
    }

    #[cfg(not(feature = "mavlink-transport"))]
    {
        let _ = (entry, config, manifest);
        Err(SitlError::FeatureMissing {
            feature: "mavlink-transport",
        })
    }

    #[cfg(feature = "mavlink-transport")]
    {
        let controllers: Vec<Px4AgentController> = manifest
            .agents
            .iter()
            .cloned()
            .map(|agent| Px4AgentController::new(agent, config.lifecycle.clone()))
            .collect();
        run_live_supervisor_with_controllers(entry, config, manifest, controllers)
    }
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn run_live_supervisor_with_controllers<C: LiveAgentController>(
    entry: &swarm_sim::ScenarioSuiteEntry,
    config: &SupervisorLiveConfig,
    manifest: &MultiAgentSitlManifest,
    mut controllers: Vec<C>,
) -> Result<SitlMultiAgentRunReport, SitlError> {
    let safety_gate = SitlSafetyGate::new(config.safety_config_path.clone());
    run_live_supervisor_with_controllers_and_safety_gate(
        entry,
        config,
        manifest,
        &mut controllers,
        &safety_gate,
    )
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) trait LiveSupervisorSafetyGate {
    fn validate_agent_task_subset(
        &self,
        entry: &swarm_sim::ScenarioSuiteEntry,
        agent_id: &str,
        task_ids: &[String],
    ) -> Result<(), SitlError>;
}

#[cfg(any(feature = "mavlink-transport", test))]
impl LiveSupervisorSafetyGate for SitlSafetyGate {
    fn validate_agent_task_subset(
        &self,
        entry: &swarm_sim::ScenarioSuiteEntry,
        agent_id: &str,
        task_ids: &[String],
    ) -> Result<(), SitlError> {
        SitlSafetyGate::validate_agent_task_subset(self, entry, agent_id, task_ids)
    }
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn run_live_supervisor_with_controllers_and_safety_gate<
    C: LiveAgentController,
    G: LiveSupervisorSafetyGate,
>(
    entry: &swarm_sim::ScenarioSuiteEntry,
    config: &SupervisorLiveConfig,
    manifest: &MultiAgentSitlManifest,
    controllers: &mut [C],
    safety_gate: &G,
) -> Result<SitlMultiAgentRunReport, SitlError> {
    validate_live_controller_set(manifest, controllers)?;
    let run_id = config
        .run_id
        .clone()
        .unwrap_or_else(|| format!("sitl-supervisor-{}", manifest.scenario_name));
    let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
        run_id: run_id.clone(),
        scenario_path: manifest.scenario_path.clone(),
        scenario_name: manifest.scenario_name.clone(),
        mission: manifest.mission.clone(),
        profile: manifest.profile.clone(),
        agent_id: "supervisor".to_owned(),
        connection_string: None,
        mode: SitlEventLogMode::ConnectionExecute,
    });
    recorder.push_multi_agent_run_started(manifest.agents_count, manifest.scenario_name.clone());
    record_swarm_command_plane_dispatch(&mut recorder, manifest);

    eprintln!(
        "Multi-Agent SITL Execute: agents={} scenario={} config={}",
        manifest.agents_count, manifest.scenario_name, config.config_path
    );

    let mut live_metrics = SupervisorMetrics::default();
    let mut reallocation_target_counts: HashMap<String, usize> = HashMap::new();
    let mut lost_agents = HashSet::new();
    let mut runs = Vec::with_capacity(manifest.agents.len());
    let mut degraded_records = Vec::new();
    let mut active_agent_ids = Vec::with_capacity(manifest.agents.len());

    for agent in &manifest.agents {
        let start_delay_ms =
            live_controller_for_agent_mut(controllers, &agent.agent_id)?.start_delay_ms();
        if start_delay_ms > 0 {
            thread::sleep(Duration::from_millis(start_delay_ms));
        }
        let mission_waypoints = live_controller_for_agent_mut(controllers, &agent.agent_id)?
            .mission_waypoints()
            .to_vec();
        recorder
            .push_multi_agent_mission_count_sent(agent.agent_id.clone(), mission_waypoints.len());
        for waypoint in &mission_waypoints {
            recorder.push_multi_agent_mission_item_sent(
                agent.agent_id.clone(),
                waypoint.seq,
                Some(waypoint.task_id.clone()),
            );
        }
        eprintln!(
            "SITL Supervisor execute: agent={} system_id={} component_id={} connection={} waypoints={}",
            agent.agent_id,
            agent.system_id,
            agent.component_id,
            agent.connection_string,
            mission_waypoints.len()
        );
        recorder.push_multi_agent_agent_started(
            agent.agent_id.clone(),
            agent.connection_string.clone(),
            agent.system_id,
            agent.component_id,
        );

        live_controller_for_agent_mut(controllers, &agent.agent_id)?.start()?;
        active_agent_ids.push(agent.agent_id.clone());
    }

    while !active_agent_ids.is_empty() {
        let poll_agent_ids = active_agent_ids.clone();
        let mut made_progress = false;
        for agent_id in poll_agent_ids {
            if !active_agent_ids.iter().any(|active| active == &agent_id) {
                continue;
            }
            let Some(mut run) = live_controller_for_agent_mut(controllers, &agent_id)?.poll()?
            else {
                continue;
            };
            made_progress = true;
            active_agent_ids.retain(|active| active != &run.agent_id);
            if run.final_status != "completed" {
                let failure_mode = classify_live_failure(&run);
                run.failure_mode.get_or_insert(failure_mode);
            }
            record_live_agent_run(&mut recorder, manifest, &run);
            if run.final_status == "completed" {
                if let Some(expected_count) = reallocation_target_counts.remove(&run.agent_id) {
                    live_metrics.final_completed_after_reallocation +=
                        run.completed_task_count.min(expected_count) as u64;
                }
            }
            recorder.push_multi_agent_agent_finished(
                run.agent_id.clone(),
                run.final_status.clone(),
                run.completed_task_count,
            );
            eprintln!(
                "SITL Supervisor execute result: agent={} status={} completed_tasks={}/{}",
                run.agent_id, run.final_status, run.completed_task_count, run.mission_item_count
            );
            if let Some(error) = &run.error {
                eprintln!(
                    "SITL Supervisor execute error: agent={} error={error}",
                    run.agent_id
                );
            }
            if run.final_status != "completed" && lost_agents.insert(run.agent_id.clone()) {
                let failure_mode = classify_live_failure(&run);
                let mut record = DegradedRunRecord::from_failed_run(&run, failure_mode.clone());
                recorder.push_supervisor_failure_detected(
                    run.agent_id.clone(),
                    failure_mode.as_str(),
                    run.completed_task_ids.clone(),
                );
                if matches!(
                    failure_mode,
                    SupervisorFailureMode::NoProgressTimeout
                        | SupervisorFailureMode::StaleTelemetry
                ) {
                    live_metrics.record_decision(SupervisorDecision::Wait);
                }
                let active_runs =
                    live_active_run_snapshots(manifest, controllers, &active_agent_ids);
                let survivor_ids: Vec<String> = active_agent_ids
                    .iter()
                    .filter(|candidate| *candidate != &run.agent_id)
                    .cloned()
                    .collect();
                if !config.reupload_on_failure || survivor_ids.is_empty() {
                    record.decision = terminal_decision_for_run(&run);
                    record.final_status = live_overall_status(
                        &runs
                            .iter()
                            .cloned()
                            .chain(std::iter::once(run.clone()))
                            .collect::<Vec<_>>(),
                        manifest,
                    )
                    .to_owned();
                    recorder.push_supervisor_failure_classified(
                        run.agent_id.clone(),
                        record.failure_mode.as_str(),
                        record.decision.as_str(),
                    );
                    live_metrics.record_degraded(&record);
                    degraded_records.push(record);
                    runs.push(run);
                    continue;
                }
                record.decision = SupervisorDecision::ContinueWithSurvivor;
                recorder.push_supervisor_failure_classified(
                    run.agent_id.clone(),
                    record.failure_mode.as_str(),
                    record.decision.as_str(),
                );
                let context = LiveReallocationContext {
                    entry,
                    manifest,
                    finished_runs: &runs,
                    active_runs: &active_runs,
                    failed_run: &run,
                    survivor_ids: &survivor_ids,
                };
                let plans = match live_reallocation_after_failure(
                    context,
                    &mut recorder,
                    &mut live_metrics,
                ) {
                    Ok(plans) => plans,
                    Err(error) => {
                        record.failure_mode = SupervisorFailureMode::BadWaypointOrMissionItem;
                        record.decision = SupervisorDecision::Abort;
                        record
                            .tasks_abandoned
                            .extend(run.completed_task_ids.clone());
                        record.final_status = "failed_recovery".to_owned();
                        recorder.push_supervisor_failure_detected(
                            run.agent_id.clone(),
                            record.failure_mode.as_str(),
                            run.completed_task_ids.clone(),
                        );
                        recorder.push_supervisor_failure_classified(
                            run.agent_id.clone(),
                            record.failure_mode.as_str(),
                            record.decision.as_str(),
                        );
                        recorder.push_supervisor_recovery_failed(
                            run.agent_id.clone(),
                            record.failure_mode.as_str(),
                            error.to_string(),
                        );
                        live_metrics.record_degraded(&record);
                        degraded_records.push(record);
                        runs.push(run);
                        continue;
                    }
                };
                let mut recovery_failed = false;
                for plan in plans {
                    recorder.push_supervisor_recovery_started(
                        plan.target_agent_id.clone(),
                        plan.policy.clone(),
                        plan.task_ids.clone(),
                    );
                    live_metrics.record_decision(SupervisorDecision::ReleaseTasksToPool);
                    live_metrics.record_decision(SupervisorDecision::ReassignUnfinishedTasks);
                    if let Err(error) = safety_gate.validate_agent_task_subset(
                        entry,
                        &plan.target_agent_id,
                        &plan.task_ids,
                    ) {
                        recovery_failed = true;
                        record.failure_mode = SupervisorFailureMode::UnsafeReplacementRoute;
                        record.decision = SupervisorDecision::RefuseUnsafeReplacement;
                        record.tasks_abandoned.extend(plan.task_ids.clone());
                        record.final_status = "failed_recovery".to_owned();
                        recorder.push_supervisor_failure_detected(
                            run.agent_id.clone(),
                            record.failure_mode.as_str(),
                            run.completed_task_ids.clone(),
                        );
                        recorder.push_supervisor_failure_classified(
                            run.agent_id.clone(),
                            record.failure_mode.as_str(),
                            record.decision.as_str(),
                        );
                        recorder.push_supervisor_recovery_failed(
                            plan.target_agent_id.clone(),
                            record.failure_mode.as_str(),
                            error.to_string(),
                        );
                        continue;
                    }
                    recorder.push_survivor_mission_update_started(
                        plan.target_agent_id.clone(),
                        plan.policy.clone(),
                        plan.task_ids.clone(),
                    );
                    record_replacement_mission_items(&mut recorder, &plan);
                    if let Err(error) =
                        live_controller_for_agent_mut(controllers, &plan.target_agent_id)?
                            .replace_mission(&plan)
                    {
                        recovery_failed = true;
                        record.failure_mode = SupervisorFailureMode::ReplacementMissionRejected;
                        record.decision = if run.completed_task_count > 0 {
                            SupervisorDecision::MarkPartialSuccess
                        } else {
                            SupervisorDecision::Abort
                        };
                        record.tasks_abandoned.extend(plan.task_ids.clone());
                        record.final_status = "failed_recovery".to_owned();
                        recorder.push_supervisor_failure_detected(
                            run.agent_id.clone(),
                            record.failure_mode.as_str(),
                            run.completed_task_ids.clone(),
                        );
                        recorder.push_supervisor_failure_classified(
                            run.agent_id.clone(),
                            record.failure_mode.as_str(),
                            record.decision.as_str(),
                        );
                        recorder.push_supervisor_recovery_failed(
                            plan.target_agent_id.clone(),
                            record.failure_mode.as_str(),
                            error.to_string(),
                        );
                        continue;
                    }
                    recorder.push_survivor_mission_update_completed(
                        plan.target_agent_id.clone(),
                        plan.policy.clone(),
                        plan.task_ids.clone(),
                        plan.mission_item_count(),
                    );
                    let recovered_task_ids: Vec<String> = plan
                        .task_ids
                        .iter()
                        .filter(|task_id| live_metrics.tasks_recovered.contains(*task_id))
                        .cloned()
                        .collect();
                    recorder.push_supervisor_replacement_uploaded(
                        plan.target_agent_id.clone(),
                        format!("replacement:{}:{}", run.agent_id, plan.target_agent_id),
                        plan.mission_item_count(),
                    );
                    recorder.push_supervisor_recovery_completed(
                        plan.target_agent_id.clone(),
                        recovered_task_ids.clone(),
                        live_metrics.reallocation_latency_ticks,
                    );
                    live_metrics.survivor_mission_updates += 1;
                    reallocation_target_counts
                        .insert(plan.target_agent_id.clone(), plan.mission_item_count());
                    record.tasks_recovered.extend(recovered_task_ids);
                    record.replacement_mission_id = Some(format!(
                        "replacement:{}:{}",
                        run.agent_id, plan.target_agent_id
                    ));
                }
                if recovery_failed {
                    record.tasks_abandoned.sort();
                    record.tasks_abandoned.dedup();
                }
                record.tasks_recovered.sort();
                record.tasks_recovered.dedup();
                record.recovery_latency_ticks = live_metrics.reallocation_latency_ticks;
                if !record.tasks_recovered.is_empty() && !recovery_failed {
                    record.final_status = "completed_with_reallocation".to_owned();
                }
                live_metrics.record_degraded(&record);
                degraded_records.push(record);
            }
            runs.push(run);
        }
        if !made_progress {
            thread::sleep(Duration::from_millis(10));
        }
    }

    live_metrics.completed_task_count =
        runs.iter().map(|run| run.completed_task_count as u64).sum();
    live_metrics.finalize();
    let overall_status = live_overall_status(&runs, manifest);
    recorder.push_supervisor_final_status(overall_status, !degraded_records.is_empty());
    recorder.push_multi_agent_run_finished(overall_status);
    recorder.push_run_completed(overall_status);
    let events_summary = summarize_sitl_event_log(recorder.log());

    let report = live_run_report(LiveRunReportInput {
        entry,
        config,
        manifest,
        run_id,
        overall_status,
        runs: &runs,
        metrics: &live_metrics,
        degraded_records: &degraded_records,
        events_summary,
    });
    if let Some(path) = &config.replay_log {
        write_sitl_event_log(path, recorder.log()).map_err(|error| SitlError::ReplayLogWrite {
            path: Path::new(path).to_path_buf(),
            message: error.to_string(),
        })?;
        eprintln!("SITL supervisor replay log written: {path}");
    }
    if let Some(path) = &config.run_report {
        write_sitl_multi_agent_run_report(path, &report).map_err(|error| {
            SitlError::RunReportWrite {
                path: Path::new(path).to_path_buf(),
                message: error.to_string(),
            }
        })?;
        eprintln!("SITL supervisor run report written: {path}");
    }
    Ok(report)
}

pub(super) fn run_supervisor_with_controllers<C: AgentController>(
    entry: &swarm_sim::ScenarioSuiteEntry,
    manifest: &MultiAgentSitlManifest,
    mut controllers: Vec<C>,
    config: &SupervisorLoopConfig<'_>,
) -> Result<SupervisorMetrics, SitlError> {
    validate_controller_set(manifest, &controllers)?;
    let own_id = config.own_id.clone();
    let own_agent_id = AgentId::from(own_id.clone());
    let peer_ids: Vec<AgentId> = manifest
        .agents
        .iter()
        .filter(|agent| agent.agent_id != own_id)
        .map(|agent| AgentId::from(agent.agent_id.clone()))
        .collect();
    let mut coordinator = Coordinator::new(
        entry.scenario.agents.clone(),
        entry.scenario.tasks.clone(),
        config.timeout_ticks,
    );
    assign_manifest_tasks(&mut coordinator, manifest)?;

    let mut node = AgentNode::new(
        own_agent_id.clone(),
        peer_ids,
        coordinator,
        MockMavlinkTransport::new(),
    );
    node.gossip_interval_ticks = config.max_ticks.saturating_add(10);
    let mut allocator = GreedyAllocator::default();
    let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
        run_id: config
            .run_id
            .map(str::to_owned)
            .unwrap_or_else(|| format!("sitl-supervisor-{}", manifest.scenario_name)),
        scenario_path: manifest.scenario_path.clone(),
        scenario_name: manifest.scenario_name.clone(),
        mission: manifest.mission.clone(),
        profile: manifest.profile.clone(),
        agent_id: "supervisor".to_owned(),
        connection_string: None,
        mode: SitlEventLogMode::Mock,
    });
    recorder.push_connection_opened();
    recorder.push_multi_agent_run_started(manifest.agents_count, manifest.scenario_name.clone());
    record_swarm_command_plane_dispatch(&mut recorder, manifest);

    eprintln!(
        "Multi-Agent SITL Foundation: mock agents={} assigned_tasks={} unassigned_pose_tasks={}",
        manifest.agents_count,
        manifest.ownership_summary.assigned_task_count,
        manifest.ownership_summary.unassigned_pose_tasks.len()
    );

    upload_and_start_manifest_agents(manifest, &mut controllers, &mut recorder, config.mode_label)?;

    let mut metrics = SupervisorMetrics::default();
    for tick in 0..=config.max_ticks {
        let active_agents = poll_active_agent_ids(&mut controllers, tick)?;
        for agent_id in active_agents.iter().filter(|agent_id| *agent_id != &own_id) {
            node.transport.push_incoming(RawMessage {
                from: AgentId::from((*agent_id).clone()),
                to: own_agent_id.clone(),
                payload: RuntimeMessage::heartbeat(tick, 1),
            });
            metrics.heartbeat_count += 1;
            recorder.push_heartbeat_seen();
        }
        if active_agents.iter().any(|agent_id| agent_id == &own_id) {
            metrics.heartbeat_count += 1;
            recorder.push_heartbeat_seen();
        }

        let output = node
            .process_inbox_and_allocate(tick, &mut allocator, Vec::new())
            .map_err(|error| SitlError::ConnectionFailed {
                message: error.to_string(),
            })?;

        let _ = record_reallocation_output(&output, &mut recorder, &mut metrics);

        metrics.completed_task_count +=
            complete_one_task_per_active_agent(&mut node, manifest, &active_agents, &mut recorder);

        if manifest_tasks_completed(&node, manifest) {
            recorder.push_run_completed("completed");
            break;
        }

        if tick == config.max_ticks {
            recorder.push_failure(
                "timeout",
                format!(
                    "supervisor did not complete manifest tasks by tick {}",
                    config.max_ticks
                ),
            );
            recorder.push_run_completed("timeout");
        }
    }

    metrics.finalize();
    let final_status = if manifest_tasks_completed(&node, manifest) {
        "completed"
    } else {
        "timeout"
    };
    eprintln!(
        "{}",
        metrics.format_summary_line(manifest.agents_count, final_status)
    );

    if let Some(path) = config.replay_log {
        write_sitl_event_log(path, recorder.log()).map_err(|error| SitlError::ReplayLogWrite {
            path: Path::new(path).to_path_buf(),
            message: error.to_string(),
        })?;
        eprintln!("SITL supervisor replay log written: {path}");
    }

    Ok(metrics)
}

fn record_swarm_command_plane_dispatch(
    recorder: &mut SitlEventRecorder,
    manifest: &MultiAgentSitlManifest,
) {
    let Some(plan) = manifest.command_plane_artifact.as_ref() else {
        return;
    };
    recorder.push_swarm_supervisor_state_changed("planned", "dispatched", "manifest_loaded");
    recorder.push_swarm_command_plan_dispatched(plan.plan_id.clone(), plan.agents.len());
    for agent in &plan.agents {
        recorder.push_swarm_agent_command_dispatched(
            plan.plan_id.clone(),
            agent.agent_id.to_string(),
            agent.command_plan.commands.len(),
        );
    }
    record_swarm_ownership_events(recorder, plan);
    for sync in &plan.sync_operations {
        recorder.push_swarm_sync_command_issued(
            sync_kind_label(&sync.kind),
            sync.agent_ids.iter().map(ToString::to_string).collect(),
        );
    }
    for result in &plan.sync_results {
        recorder.push_swarm_sync_command_result(
            sync_kind_label(&result.kind),
            result.succeeded.iter().map(ToString::to_string).collect(),
            result.failed.iter().map(ToString::to_string).collect(),
            result.timed_out.iter().map(ToString::to_string).collect(),
            !result.failed.is_empty() || !result.timed_out.is_empty(),
        );
    }
}

fn record_swarm_ownership_events(recorder: &mut SitlEventRecorder, plan: &SwarmCommandPlan) {
    for ownership in &plan.ownership {
        match ownership.status {
            SwarmOwnershipStatus::Active => recorder.push_swarm_ownership_acquired(
                ownership.agent_id.to_string(),
                ownership_kind_label(&ownership.kind),
                ownership.resource_id.clone(),
                ownership.reason.clone(),
            ),
            SwarmOwnershipStatus::Released => recorder.push_swarm_ownership_released(
                ownership.agent_id.to_string(),
                ownership_kind_label(&ownership.kind),
                ownership.resource_id.clone(),
                ownership.reason.clone(),
            ),
        }
    }
    for handoff in &plan.handoffs {
        recorder.push_swarm_ownership_handoff(
            handoff.from_agent_id.to_string(),
            handoff.to_agent_id.to_string(),
            ownership_kind_label(&handoff.kind),
            handoff.resource_id.clone(),
            handoff.reason.clone(),
        );
    }
}

fn ownership_kind_label(kind: &SwarmOwnershipKind) -> &'static str {
    match kind {
        SwarmOwnershipKind::Task => "task",
        SwarmOwnershipKind::RouteSegment => "route_segment",
        SwarmOwnershipKind::Target => "target",
        SwarmOwnershipKind::ReplacementMission => "replacement_mission",
    }
}

fn sync_kind_label(kind: &SynchronizedCommandKind) -> &'static str {
    match kind {
        SynchronizedCommandKind::ArmAll => "arm_all",
        SynchronizedCommandKind::TakeoffAll => "takeoff_all",
        SynchronizedCommandKind::StartAll => "start_all",
        SynchronizedCommandKind::AbortAll => "abort_all",
    }
}
