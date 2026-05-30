use std::path::Path;
use std::thread;
use std::time::Duration;

use swarm_alloc::GreedyAllocator;
use swarm_comms::{MockMavlinkTransport, RawMessage, Waypoint};
use swarm_examples::sitl_multi_agent::{
    build_multi_agent_manifest, load_multi_agent_config, MultiAgentSitlManifest,
};
use swarm_examples::sitl_observability::{
    write_sitl_event_log, SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
};
use swarm_examples::sitl_plan::{first_sitl_entry, load_sitl_suite, SitlError};
use swarm_runtime::{AgentNode, Coordinator, RuntimeMessage};
use swarm_types::{AgentId, TaskId, TaskStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SupervisorMode {
    DryRun,
    Mock,
}

struct CliArgs {
    mode: SupervisorMode,
    scenario: String,
    config: String,
    manifest: Option<String>,
    replay_log: Option<String>,
    fail_agent: Option<String>,
    fail_after_ticks: u64,
    heartbeat_timeout_ticks: Option<u64>,
    max_ticks: Option<u64>,
}

#[derive(Default)]
struct SupervisorMetrics {
    heartbeat_count: u64,
    completed_task_count: u64,
    lost_agent_count: u64,
    reassignment_count: u64,
    tasks_recovered: Vec<String>,
    reallocation_latency_ticks: Option<u64>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        eprintln!(
            "usage: sitl_supervisor --dry-run|--mock --scenario <path> --config <path> [--manifest <path>] [--replay-log <path>] [--fail-agent <id>] [--fail-after-ticks N] [--heartbeat-timeout-ticks N] [--max-ticks N]"
        );
        std::process::exit(1);
    }
}

fn run() -> Result<(), SitlError> {
    let cli = parse_args()?;
    let suite = load_sitl_suite(&cli.scenario)?;
    let config = load_multi_agent_config(&cli.config)?;
    let manifest = build_multi_agent_manifest(&suite, &cli.scenario, &cli.config, &config)?;

    match cli.mode {
        SupervisorMode::DryRun => {
            write_or_print_manifest(cli.manifest.as_deref(), &manifest)?;
        }
        SupervisorMode::Mock => {
            run_mock_supervisor(&suite, &cli, &manifest)?;
            write_or_print_manifest(cli.manifest.as_deref(), &manifest)?;
        }
    }
    Ok(())
}

fn parse_args() -> Result<CliArgs, SitlError> {
    let args: Vec<String> = std::env::args().collect();
    let mut mode = None;
    let mut scenario = None;
    let mut config = None;
    let mut manifest = None;
    let mut replay_log = None;
    let mut fail_agent = None;
    let mut fail_after_ticks = 1;
    let mut heartbeat_timeout_ticks = None;
    let mut max_ticks = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dry-run" => set_mode(&mut mode, SupervisorMode::DryRun)?,
            "--mock" => set_mode(&mut mode, SupervisorMode::Mock)?,
            "--scenario" => {
                i += 1;
                scenario = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument { name: "--scenario" })?
                        .clone(),
                );
            }
            "--config" => {
                i += 1;
                config = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument { name: "--config" })?
                        .clone(),
                );
            }
            "--manifest" => {
                i += 1;
                manifest = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument { name: "--manifest" })?
                        .clone(),
                );
            }
            "--replay-log" => {
                i += 1;
                replay_log = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--replay-log",
                        })?
                        .clone(),
                );
            }
            "--fail-agent" => {
                i += 1;
                fail_agent = Some(
                    args.get(i)
                        .ok_or(SitlError::MissingArgument {
                            name: "--fail-agent",
                        })?
                        .clone(),
                );
            }
            "--fail-after-ticks" => {
                i += 1;
                fail_after_ticks = parse_u64_arg(
                    args.get(i).ok_or(SitlError::MissingArgument {
                        name: "--fail-after-ticks",
                    })?,
                    "--fail-after-ticks",
                )?;
            }
            "--heartbeat-timeout-ticks" => {
                i += 1;
                heartbeat_timeout_ticks = Some(parse_u64_arg(
                    args.get(i).ok_or(SitlError::MissingArgument {
                        name: "--heartbeat-timeout-ticks",
                    })?,
                    "--heartbeat-timeout-ticks",
                )?);
            }
            "--max-ticks" => {
                i += 1;
                max_ticks = Some(parse_u64_arg(
                    args.get(i).ok_or(SitlError::MissingArgument {
                        name: "--max-ticks",
                    })?,
                    "--max-ticks",
                )?);
            }
            arg => {
                return Err(SitlError::UnknownArgument {
                    arg: arg.to_owned(),
                });
            }
        }
        i += 1;
    }

    Ok(CliArgs {
        mode: mode.ok_or(SitlError::MissingMode)?,
        scenario: scenario.ok_or(SitlError::MissingArgument { name: "--scenario" })?,
        config: config.ok_or(SitlError::MissingArgument { name: "--config" })?,
        manifest,
        replay_log,
        fail_agent,
        fail_after_ticks,
        heartbeat_timeout_ticks,
        max_ticks,
    })
}

fn parse_u64_arg(value: &str, name: &'static str) -> Result<u64, SitlError> {
    value
        .parse::<u64>()
        .map_err(|_| SitlError::MultiAgentConfigInvalid {
            message: format!("invalid {name} value '{value}'"),
        })
}

fn set_mode(mode: &mut Option<SupervisorMode>, next: SupervisorMode) -> Result<(), SitlError> {
    if mode.is_some() {
        return Err(SitlError::ConflictingModes);
    }
    *mode = Some(next);
    Ok(())
}

fn write_or_print_manifest(
    manifest_path: Option<&str>,
    manifest: &MultiAgentSitlManifest,
) -> Result<(), SitlError> {
    let json = serde_json::to_string_pretty(manifest).map_err(|error| {
        SitlError::MultiAgentConfigInvalid {
            message: error.to_string(),
        }
    })?;
    let Some(path) = manifest_path else {
        println!("{json}");
        return Ok(());
    };
    let path = Path::new(path);
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| SitlError::MultiAgentManifestWrite {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    std::fs::write(path, json).map_err(|error| SitlError::MultiAgentManifestWrite {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    eprintln!("Multi-agent SITL manifest written: {}", path.display());
    Ok(())
}

fn run_mock_supervisor(
    suite: &swarm_sim::ScenarioSuite,
    cli: &CliArgs,
    manifest: &MultiAgentSitlManifest,
) -> Result<(), SitlError> {
    validate_failure_agent(manifest, cli.fail_agent.as_deref())?;
    let entry = first_sitl_entry(suite, &cli.scenario)?;
    let timeout_ticks = cli
        .heartbeat_timeout_ticks
        .unwrap_or(entry.run_config.timeout_ticks);
    let max_ticks = cli.max_ticks.unwrap_or(
        entry
            .run_config
            .max_ticks
            .max(timeout_ticks + cli.fail_after_ticks + 3),
    );
    let own_id = supervisor_runtime_agent_id(manifest, cli.fail_agent.as_deref())?;
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
        timeout_ticks,
    );
    assign_manifest_tasks(&mut coordinator, manifest)?;

    let mut node = AgentNode::new(
        own_agent_id.clone(),
        peer_ids,
        coordinator,
        MockMavlinkTransport::new(),
    );
    node.gossip_interval_ticks = max_ticks.saturating_add(10);
    let mut allocator = GreedyAllocator;
    let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
        run_id: format!("sitl-supervisor-{}", manifest.scenario_name),
        scenario_path: manifest.scenario_path.clone(),
        scenario_name: manifest.scenario_name.clone(),
        mission: manifest.mission.clone(),
        profile: manifest.profile.clone(),
        agent_id: "supervisor".to_owned(),
        connection_string: None,
        mode: SitlEventLogMode::Mock,
    });
    recorder.push_connection_opened();

    eprintln!(
        "Multi-Agent SITL Foundation: mock agents={} assigned_tasks={} unassigned_pose_tasks={}",
        manifest.agents_count,
        manifest.ownership_summary.assigned_task_count,
        manifest.ownership_summary.unassigned_pose_tasks.len()
    );

    for agent in &manifest.agents {
        if agent.start_delay_ms > 0 {
            thread::sleep(Duration::from_millis(agent.start_delay_ms));
        }
        let mut transport = MockMavlinkTransport::new();
        eprintln!(
            "SITL Supervisor: agent={} system_id={} component_id={} connection={} waypoints={}",
            agent.agent_id,
            agent.system_id,
            agent.component_id,
            agent.connection_string,
            agent.waypoint_count
        );
        recorder.push_mission_count_sent(agent.waypoint_count);
        for waypoint in &agent.waypoints {
            transport.send_waypoint(Waypoint {
                x: waypoint.x,
                y: waypoint.y,
                z: waypoint.z,
                seq: waypoint.seq,
            });
            recorder.push_mission_item_sent(waypoint.seq, Some(waypoint.task_id.clone()));
            eprintln!(
                "WAYPOINT agent={} seq={} task_id={} x={:.1} y={:.1} z={:.1}",
                agent.agent_id, waypoint.seq, waypoint.task_id, waypoint.x, waypoint.y, waypoint.z
            );
        }
        eprintln!(
            "Mock mode: agent={} waypoints sent={}",
            agent.agent_id,
            transport.waypoints().len()
        );
    }

    let mut metrics = SupervisorMetrics::default();
    for tick in 0..=max_ticks {
        let active_agents = active_agent_ids(
            manifest,
            cli.fail_agent.as_deref(),
            cli.fail_after_ticks,
            tick,
        );
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

        for release in &output.failure_releases {
            metrics.lost_agent_count += 1;
            let failed_agent_id = release.failed_agent_id.to_string();
            recorder.push_agent_lost(failed_agent_id.clone());
            for task_id in &release.released_tasks {
                recorder.push_task_released(task_id.to_string(), failed_agent_id.clone());
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
                recorder.push_task_reassigned(
                    assignment.task_id.to_string(),
                    from_agent_id,
                    assignment.agent_id.to_string(),
                    output.reallocation_latency_ticks.unwrap_or(0),
                );
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

        metrics.completed_task_count +=
            complete_one_task_per_active_agent(&mut node, manifest, &active_agents, &mut recorder);

        if manifest_tasks_completed(&node, manifest) {
            recorder.push_run_completed("completed");
            break;
        }

        if tick == max_ticks {
            recorder.push_failure(
                "timeout",
                format!("supervisor did not complete manifest tasks by tick {max_ticks}"),
            );
            recorder.push_run_completed("timeout");
        }
    }

    metrics.tasks_recovered.sort();
    metrics.tasks_recovered.dedup();
    eprintln!(
        "SUPERVISOR_METRICS agents={} heartbeats={} completed_tasks={} lost_agents={} reassignment_count={} tasks_recovered={} reallocation_latency_ticks={} final_status={}",
        manifest.agents_count,
        metrics.heartbeat_count,
        metrics.completed_task_count,
        metrics.lost_agent_count,
        metrics.reassignment_count,
        if metrics.tasks_recovered.is_empty() {
            "none".to_owned()
        } else {
            metrics.tasks_recovered.join(",")
        },
        metrics
            .reallocation_latency_ticks
            .map(|ticks| ticks.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        if manifest_tasks_completed(&node, manifest) {
            "completed"
        } else {
            "timeout"
        }
    );

    if let Some(path) = cli.replay_log.as_deref() {
        write_sitl_event_log(path, recorder.log()).map_err(|error| SitlError::ReplayLogWrite {
            path: Path::new(path).to_path_buf(),
            message: error.to_string(),
        })?;
        eprintln!("SITL supervisor replay log written: {path}");
    }

    Ok(())
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

fn active_agent_ids(
    manifest: &MultiAgentSitlManifest,
    fail_agent: Option<&str>,
    fail_after_ticks: u64,
    tick: u64,
) -> Vec<String> {
    manifest
        .agents
        .iter()
        .filter(|agent| Some(agent.agent_id.as_str()) != fail_agent || tick < fail_after_ticks)
        .map(|agent| agent.agent_id.clone())
        .collect()
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
                recorder.push_waypoint_reached(seq, Some(task_id.to_string()));
                recorder.push_task_completed(seq, task_id.to_string());
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
