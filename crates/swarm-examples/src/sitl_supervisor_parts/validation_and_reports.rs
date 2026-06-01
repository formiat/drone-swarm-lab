fn build_mock_controllers(
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

fn upload_and_start_manifest_agents<C: AgentController>(
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

fn controller_for_agent_mut<'a, C: AgentController>(
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

fn validate_controller_set<C: AgentController>(
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

#[cfg(any(feature = "mavlink-transport", test))]
fn validate_live_controller_set<C: LiveAgentController>(
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

fn validate_live_manifest(
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
fn live_controller_for_agent_mut<'a, C: LiveAgentController>(
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
fn live_active_run_snapshots<C: LiveAgentController>(
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
            })
        })
        .collect()
}

#[cfg(any(feature = "mavlink-transport", test))]
fn record_replacement_mission_items(
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
fn record_live_agent_run(
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

#[cfg(any(feature = "mavlink-transport", test))]
fn manifest_waypoint_for_task_id<'a>(
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
fn live_overall_status(runs: &[LiveAgentRun], manifest: &MultiAgentSitlManifest) -> &'static str {
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

#[cfg(any(feature = "mavlink-transport", test))]
struct LiveRunReportInput<'a> {
    entry: &'a swarm_sim::ScenarioSuiteEntry,
    config: &'a SupervisorLiveConfig,
    manifest: &'a MultiAgentSitlManifest,
    run_id: String,
    overall_status: &'a str,
    runs: &'a [LiveAgentRun],
    metrics: &'a SupervisorMetrics,
    events_summary: SitlEventLogSummary,
}

#[cfg(any(feature = "mavlink-transport", test))]
fn live_run_report(input: LiveRunReportInput<'_>) -> SitlMultiAgentRunReport {
    let entry = input.entry;
    let config = input.config;
    let manifest = input.manifest;
    let runs = input.runs;
    let limitations = vec![
        "local PX4/SIH endpoints only unless --allow-hardware-candidate is explicit".to_owned(),
        "agents are started sequentially and polled stepwise in one supervisor process".to_owned(),
        if config.reupload_on_failure {
            "failed-agent reallocation uses controlled local active-survivor mission replacement; Gazebo, HIL, and hardware are not claimed".to_owned()
        } else {
            "live failed-agent reallocation requires explicit --reupload-on-failure".to_owned()
        },
    ];
    SitlMultiAgentRunReport {
        schema_version: "sitl_multi_agent_run_report.v1".to_owned(),
        run_id: input.run_id,
        scenario_path: manifest.scenario_path.clone(),
        scenario_name: entry.scenario.name.clone(),
        config_path: PathBuf::from(&config.config_path),
        mission: entry.mission.clone(),
        profile: entry.profile.clone(),
        mode: "connection_execute".to_owned(),
        agents: runs.iter().map(LiveAgentRun::report).collect(),
        total_completed_tasks: runs.iter().map(|run| run.completed_task_count).sum(),
        failed_agents: runs
            .iter()
            .filter(|run| run.final_status != "completed")
            .count(),
        aborted_agents: runs
            .iter()
            .filter(|run| run.final_status == "aborted")
            .count(),
        overall_status: input.overall_status.to_owned(),
        event_log_path: config.replay_log.as_ref().map(PathBuf::from),
        task_ownership: manifest.ownership_summary.clone(),
        events_summary: input.events_summary,
        final_status: input.overall_status.to_owned(),
        reallocation: input.metrics.into(),
        limitations: limitations.clone(),
        known_limitations: limitations,
    }
}

fn poll_active_agent_ids<C: AgentController>(
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

fn validate_failure_agent(
    manifest: &MultiAgentSitlManifest,
    fail_agent: Option<&str>,
) -> Result<(), SitlError> {
    let Some(fail_agent) = fail_agent else {
        return Ok(());
    };
    if manifest
        .agents
        .iter()
        .any(|agent| agent.agent_id == fail_agent)
    {
        Ok(())
    } else {
        Err(SitlError::MultiAgentConfigInvalid {
            message: format!("--fail-agent '{fail_agent}' is not present in manifest"),
        })
    }
}

fn supervisor_runtime_agent_id(
    manifest: &MultiAgentSitlManifest,
    fail_agent: Option<&str>,
) -> Result<String, SitlError> {
    manifest
        .agents
        .iter()
        .find(|agent| Some(agent.agent_id.as_str()) != fail_agent)
        .or_else(|| manifest.agents.first())
        .map(|agent| agent.agent_id.clone())
        .ok_or_else(|| SitlError::MultiAgentConfigInvalid {
            message: "manifest must contain at least one agent".to_owned(),
        })
}

fn assign_manifest_tasks(
    coordinator: &mut Coordinator,
    manifest: &MultiAgentSitlManifest,
) -> Result<(), SitlError> {
    for agent in &manifest.agents {
        let agent_id = AgentId::from(agent.agent_id.clone());
        for task_id in &agent.task_ids {
            coordinator
                .registry
                .assign(&TaskId::from(task_id.clone()), agent_id.clone())
                .map_err(|error| SitlError::MultiAgentConfigInvalid {
                    message: format!(
                        "failed to assign task_id '{task_id}' to '{}': {error}",
                        agent.agent_id
                    ),
                })?;
        }
    }
    Ok(())
}

fn complete_one_task_per_active_agent(
    node: &mut AgentNode<MockMavlinkTransport>,
    manifest: &MultiAgentSitlManifest,
    active_agents: &[String],
    recorder: &mut SitlEventRecorder,
) -> u64 {
    let mut completed = 0;
    for agent_id in active_agents {
        let agent_id_typed = AgentId::from(agent_id.clone());
        let Some(task_id) = first_assigned_manifest_task(node, manifest, &agent_id_typed) else {
            continue;
        };
        if let Some(previous_agent_id) = node.coordinator.registry.complete_assigned_task(&task_id)
        {
            if previous_agent_id == agent_id_typed {
                let seq = manifest_seq_for_task(manifest, &task_id).unwrap_or(0);
                recorder.push_multi_agent_waypoint_reached(
                    agent_id.clone(),
                    seq,
                    Some(task_id.to_string()),
                );
                recorder.push_multi_agent_task_completed(
                    agent_id.clone(),
                    seq,
                    task_id.to_string(),
                );
                completed += 1;
            }
        }
    }
    completed
}

fn first_assigned_manifest_task(
    node: &AgentNode<MockMavlinkTransport>,
    manifest: &MultiAgentSitlManifest,
    agent_id: &AgentId,
) -> Option<TaskId> {
    let manifest_task_ids: std::collections::HashSet<String> = manifest
        .agents
        .iter()
        .flat_map(|agent| agent.task_ids.iter().cloned())
        .collect();
    let mut candidates: Vec<TaskId> = node
        .coordinator
        .registry
        .tasks()
        .filter(|task| {
            manifest_task_ids.contains(task.id.as_ref())
                && task.assigned_to.as_ref() == Some(agent_id)
                && matches!(task.status, TaskStatus::Assigned | TaskStatus::InProgress)
        })
        .map(|task| task.id.clone())
        .collect();
    candidates.sort_by(|a, b| a.as_ref().cmp(b.as_ref()));
    candidates.into_iter().next()
}

fn manifest_seq_for_task(manifest: &MultiAgentSitlManifest, task_id: &TaskId) -> Option<u16> {
    manifest
        .agents
        .iter()
        .flat_map(|agent| agent.waypoints.iter())
        .find(|waypoint| waypoint.task_id.as_str() == task_id.as_ref())
        .map(|waypoint| waypoint.seq)
}

fn manifest_tasks_completed(
    node: &AgentNode<MockMavlinkTransport>,
    manifest: &MultiAgentSitlManifest,
) -> bool {
    let manifest_task_ids: std::collections::HashSet<String> = manifest
        .agents
        .iter()
        .flat_map(|agent| agent.task_ids.iter().cloned())
        .collect();
    node.coordinator
        .registry
        .tasks()
        .filter(|task| manifest_task_ids.contains(task.id.as_ref()))
        .all(|task| task.status == TaskStatus::Completed)
}

#[cfg(feature = "mavlink-transport")]
fn event_advances_progress(
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
fn append_abort_to_report(
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
fn live_progress_status_name(status: crate::sitl_progress::SitlMissionFinalStatus) -> &'static str {
    match status {
        crate::sitl_progress::SitlMissionFinalStatus::Completed => "completed",
        crate::sitl_progress::SitlMissionFinalStatus::Failed => "failed",
        crate::sitl_progress::SitlMissionFinalStatus::Disconnected => "disconnected",
        crate::sitl_progress::SitlMissionFinalStatus::Rejected => "rejected",
        crate::sitl_progress::SitlMissionFinalStatus::TimedOutNoProgress => "timed_out_no_progress",
    }
}
