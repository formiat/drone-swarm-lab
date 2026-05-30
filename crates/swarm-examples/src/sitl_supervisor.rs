use std::path::Path;
use std::thread;
use std::time::Duration;

use swarm_alloc::GreedyAllocator;
use swarm_comms::{MockMavlinkTransport, RawMessage, Waypoint};
use swarm_runtime::{AgentNode, Coordinator, RuntimeMessage};
use swarm_types::{AgentId, TaskId, TaskStatus};

use crate::sitl_multi_agent::{
    MultiAgentLifecycle, MultiAgentSitlManifest, MultiAgentSitlManifestAgent,
};
use crate::sitl_observability::{
    write_sitl_event_log, SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
};
use crate::sitl_plan::{first_sitl_entry, SitlError, SitlWaypointItem};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupervisorMockConfig {
    pub scenario_path: String,
    pub replay_log: Option<String>,
    pub fail_agent: Option<String>,
    pub fail_after_ticks: u64,
    pub heartbeat_timeout_ticks: Option<u64>,
    pub max_ticks: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SupervisorMetrics {
    pub heartbeat_count: u64,
    pub completed_task_count: u64,
    pub lost_agent_count: u64,
    pub reassignment_count: u64,
    pub tasks_recovered: Vec<String>,
    pub reallocation_latency_ticks: Option<u64>,
}

impl SupervisorMetrics {
    pub fn finalize(&mut self) {
        self.tasks_recovered.sort();
        self.tasks_recovered.dedup();
    }

    pub fn format_summary_line(&self, agents_count: usize, final_status: &str) -> String {
        format!(
            "SUPERVISOR_METRICS agents={} heartbeats={} completed_tasks={} lost_agents={} reassignment_count={} tasks_recovered={} reallocation_latency_ticks={} final_status={}",
            agents_count,
            self.heartbeat_count,
            self.completed_task_count,
            self.lost_agent_count,
            self.reassignment_count,
            if self.tasks_recovered.is_empty() {
                "none".to_owned()
            } else {
                self.tasks_recovered.join(",")
            },
            self.reallocation_latency_ticks
                .map(|ticks| ticks.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            final_status
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentStep {
    pub agent_id: String,
    pub waypoint_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentProgress {
    pub agent_id: String,
    pub heartbeat_seen: bool,
}

pub trait AgentController {
    fn agent_id(&self) -> &str;
    fn lifecycle(&self) -> MultiAgentLifecycle;
    fn upload(&mut self, waypoints: &[SitlWaypointItem]) -> Result<AgentStep, SitlError>;
    fn start(&mut self) -> Result<AgentStep, SitlError>;
    fn poll(&mut self, tick: u64) -> Result<AgentProgress, SitlError>;
    fn abort(&mut self, reason: &str) -> Result<AgentStep, SitlError>;
}

pub struct MockAgentController {
    agent_id: String,
    lifecycle: MultiAgentLifecycle,
    fail_after_ticks: Option<u64>,
    transport: MockMavlinkTransport,
}

impl MockAgentController {
    pub fn new(agent: &MultiAgentSitlManifestAgent, fail_after_ticks: Option<u64>) -> Self {
        Self {
            agent_id: agent.agent_id.clone(),
            lifecycle: agent.lifecycle,
            fail_after_ticks,
            transport: MockMavlinkTransport::new(),
        }
    }

    pub fn waypoints_sent(&self) -> usize {
        self.transport.waypoints().len()
    }
}

impl AgentController for MockAgentController {
    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn lifecycle(&self) -> MultiAgentLifecycle {
        self.lifecycle
    }

    fn upload(&mut self, waypoints: &[SitlWaypointItem]) -> Result<AgentStep, SitlError> {
        for waypoint in waypoints {
            self.transport.send_waypoint(Waypoint {
                x: waypoint.x,
                y: waypoint.y,
                z: waypoint.z,
                seq: waypoint.seq,
            });
        }
        Ok(AgentStep {
            agent_id: self.agent_id.clone(),
            waypoint_count: self.waypoints_sent(),
        })
    }

    fn start(&mut self) -> Result<AgentStep, SitlError> {
        Ok(AgentStep {
            agent_id: self.agent_id.clone(),
            waypoint_count: self.waypoints_sent(),
        })
    }

    fn poll(&mut self, tick: u64) -> Result<AgentProgress, SitlError> {
        let heartbeat_seen = self
            .fail_after_ticks
            .is_none_or(|fail_after_ticks| tick < fail_after_ticks);
        Ok(AgentProgress {
            agent_id: self.agent_id.clone(),
            heartbeat_seen,
        })
    }

    fn abort(&mut self, _reason: &str) -> Result<AgentStep, SitlError> {
        Ok(AgentStep {
            agent_id: self.agent_id.clone(),
            waypoint_count: self.waypoints_sent(),
        })
    }
}

pub fn run_mock_supervisor(
    suite: &swarm_sim::ScenarioSuite,
    config: &SupervisorMockConfig,
    manifest: &MultiAgentSitlManifest,
) -> Result<SupervisorMetrics, SitlError> {
    validate_failure_agent(manifest, config.fail_agent.as_deref())?;
    let entry = first_sitl_entry(suite, &config.scenario_path)?;
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

    let mut controllers = build_mock_controllers(manifest, config);
    for (agent, controller) in manifest.agents.iter().zip(controllers.iter_mut()) {
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
        recorder.push_mission_count_sent(agent.waypoint_count);
        for waypoint in &agent.waypoints {
            recorder.push_mission_item_sent(waypoint.seq, Some(waypoint.task_id.clone()));
            eprintln!(
                "WAYPOINT agent={} seq={} task_id={} x={:.1} y={:.1} z={:.1}",
                agent.agent_id, waypoint.seq, waypoint.task_id, waypoint.x, waypoint.y, waypoint.z
            );
        }
        let upload = controller.upload(&agent.waypoints)?;
        controller.start()?;
        eprintln!(
            "Mock mode: agent={} waypoints sent={}",
            agent.agent_id, upload.waypoint_count
        );
    }

    let mut metrics = SupervisorMetrics::default();
    for tick in 0..=max_ticks {
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

    if let Some(path) = config.replay_log.as_deref() {
        write_sitl_event_log(path, recorder.log()).map_err(|error| SitlError::ReplayLogWrite {
            path: Path::new(path).to_path_buf(),
            message: error.to_string(),
        })?;
        eprintln!("SITL supervisor replay log written: {path}");
    }

    Ok(metrics)
}

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

fn poll_active_agent_ids(
    controllers: &mut [MockAgentController],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sitl_multi_agent::{build_multi_agent_manifest, MultiAgentSitlConfig};

    fn fixture_suite() -> swarm_sim::ScenarioSuite {
        serde_json::from_str(
            r#"{
  "schema_version": "0.1",
  "name": "Supervisor Unit Fixture",
  "description": "in-memory supervisor unit fixture",
  "scenarios": [
    {
      "mission": "sitl",
      "profile": "unit",
      "scenario": {
        "name": "supervisor_unit",
        "seed": 0,
        "agents": [
          {
            "id": "agent-0",
            "role": "scout",
            "health": "alive",
            "pose": { "x": 0.0, "y": 0.0 },
            "capabilities": [],
            "current_task": null,
            "battery": 100.0,
            "comms_range": 1000.0,
            "generation": 1,
            "speed": 0.0,
            "max_range": 1000.0,
            "battery_drain_rate": 0.0
          },
          {
            "id": "agent-1",
            "role": "scout",
            "health": "alive",
            "pose": { "x": 1.0, "y": 1.0 },
            "capabilities": [],
            "current_task": null,
            "battery": 100.0,
            "comms_range": 1000.0,
            "generation": 1,
            "speed": 0.0,
            "max_range": 1000.0,
            "battery_drain_rate": 0.0
          }
        ],
        "tasks": [
          {
            "id": "wp-0",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 10.0, "y": 20.0, "z": 3.5 },
            "grid_cell": null
          },
          {
            "id": "wp-1",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 30.0, "y": 40.0, "z": 4.5 },
            "grid_cell": null
          }
        ],
        "ground_nodes": [],
        "base_station": null
      },
      "run_config": { "max_ticks": 10, "timeout_ticks": 1 }
    }
  ]
}"#,
        )
        .unwrap()
    }

    fn fixture_config() -> MultiAgentSitlConfig {
        serde_json::from_str(
            r#"{
  "schema_version": "multi_sitl.v1",
  "agents": [
    {
      "agent_id": "agent-0",
      "system_id": 1,
      "component_id": 1,
      "connection_string": "udp:127.0.0.1:14550",
      "start_delay_ms": 0,
      "lifecycle": "upload_only",
      "task_ids": ["wp-0"]
    },
    {
      "agent_id": "agent-1",
      "system_id": 2,
      "component_id": 1,
      "connection_string": "udp:127.0.0.1:14560",
      "start_delay_ms": 0,
      "lifecycle": "execute",
      "task_ids": ["wp-1"]
    }
  ]
}"#,
        )
        .unwrap()
    }

    fn fixture_manifest() -> MultiAgentSitlManifest {
        let suite = fixture_suite();
        let config = fixture_config();
        build_multi_agent_manifest(
            &suite,
            "inline-scenario.json",
            "inline-config.json",
            &config,
        )
        .unwrap()
    }

    #[test]
    fn supervisor_metrics_formats_contract_line() {
        let metrics = SupervisorMetrics {
            heartbeat_count: 6,
            completed_task_count: 2,
            lost_agent_count: 1,
            reassignment_count: 1,
            tasks_recovered: vec!["wp-0".to_owned()],
            reallocation_latency_ticks: Some(0),
        };

        assert_eq!(
            metrics.format_summary_line(2, "completed"),
            "SUPERVISOR_METRICS agents=2 heartbeats=6 completed_tasks=2 lost_agents=1 reassignment_count=1 tasks_recovered=wp-0 reallocation_latency_ticks=0 final_status=completed"
        );
    }

    #[test]
    fn mock_agent_controller_uploads_and_polls_deterministically() {
        let manifest = fixture_manifest();
        let agent = &manifest.agents[0];
        let mut controller = MockAgentController::new(agent, Some(1));

        let upload = controller.upload(&agent.waypoints).unwrap();
        assert_eq!(upload.agent_id, "agent-0");
        assert_eq!(upload.waypoint_count, 1);
        assert_eq!(controller.waypoints_sent(), 1);
        assert!(controller.poll(0).unwrap().heartbeat_seen);
        assert!(!controller.poll(1).unwrap().heartbeat_seen);
    }

    #[test]
    fn mock_supervisor_returns_metrics_after_reallocation() {
        let suite = fixture_suite();
        let manifest = fixture_manifest();
        let config = SupervisorMockConfig {
            scenario_path: "inline-scenario.json".to_owned(),
            replay_log: None,
            fail_agent: Some("agent-0".to_owned()),
            fail_after_ticks: 0,
            heartbeat_timeout_ticks: Some(1),
            max_ticks: Some(6),
        };

        let metrics = run_mock_supervisor(&suite, &config, &manifest).unwrap();
        assert_eq!(metrics.lost_agent_count, 1);
        assert_eq!(metrics.reassignment_count, 1);
        assert_eq!(metrics.tasks_recovered, vec!["wp-0"]);
        assert_eq!(metrics.reallocation_latency_ticks, Some(0));
    }

    #[test]
    fn mock_supervisor_rejects_unknown_fail_agent() {
        let suite = fixture_suite();
        let manifest = fixture_manifest();
        let config = SupervisorMockConfig {
            scenario_path: "inline-scenario.json".to_owned(),
            replay_log: None,
            fail_agent: Some("missing-agent".to_owned()),
            fail_after_ticks: 0,
            heartbeat_timeout_ticks: Some(1),
            max_ticks: Some(6),
        };

        let error = run_mock_supervisor(&suite, &config, &manifest).unwrap_err();
        assert!(error.to_string().contains("--fail-agent 'missing-agent'"));
    }
}
