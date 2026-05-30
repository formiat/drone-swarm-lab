use std::collections::{HashMap, HashSet};
use std::path::Path;
#[cfg(any(feature = "mavlink-transport", test))]
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use swarm_alloc::GreedyAllocator;
use swarm_comms::{MockMavlinkTransport, RawMessage, Waypoint};
use swarm_runtime::{AgentNode, Coordinator, NodeTickOutput, RuntimeMessage};
use swarm_types::{AgentId, TaskId, TaskStatus};

#[cfg(feature = "mavlink-transport")]
use crate::sitl_connection::{
    default_takeoff_altitude, task_ids_by_seq_from_items, waypoints_from_sitl_items,
};
use crate::sitl_connection::{SitlConnectionLifecycle, SitlSafetyGate};
use crate::sitl_multi_agent::{
    MultiAgentLifecycle, MultiAgentSitlManifest, MultiAgentSitlManifestAgent,
};
use crate::sitl_observability::{
    write_sitl_event_log, SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
};
use crate::sitl_plan::{
    classify_connection_string, first_sitl_entry, SitlConnectionClass, SitlError, SitlWaypointItem,
};
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_report::{write_sitl_multi_agent_run_report, SitlMultiAgentAgentReport};
use crate::sitl_report::{SitlMultiAgentReallocationReport, SitlMultiAgentRunReport};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupervisorMockConfig {
    pub scenario_path: String,
    pub replay_log: Option<String>,
    pub fail_agent: Option<String>,
    pub fail_after_ticks: u64,
    pub heartbeat_timeout_ticks: Option<u64>,
    pub max_ticks: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupervisorLiveConfig {
    pub scenario_path: String,
    pub config_path: String,
    pub safety_config_path: Option<String>,
    pub replay_log: Option<String>,
    pub run_report: Option<String>,
    pub lifecycle: SitlConnectionLifecycle,
    pub allow_hardware_candidate: bool,
    pub reupload_on_failure: bool,
    pub run_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SupervisorLoopConfig<'a> {
    replay_log: Option<&'a str>,
    timeout_ticks: u64,
    max_ticks: u64,
    own_id: String,
    mode_label: &'a str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SupervisorMetrics {
    pub heartbeat_count: u64,
    pub completed_task_count: u64,
    pub lost_agent_count: u64,
    pub released_tasks: Vec<String>,
    pub reassigned_tasks: Vec<String>,
    pub reassignment_count: u64,
    pub tasks_recovered: Vec<String>,
    pub reallocation_latency_ticks: Option<u64>,
    pub survivor_mission_updates: u64,
    pub final_completed_after_reallocation: u64,
}

impl SupervisorMetrics {
    pub fn finalize(&mut self) {
        self.released_tasks.sort();
        self.released_tasks.dedup();
        self.reassigned_tasks.sort();
        self.reassigned_tasks.dedup();
        self.tasks_recovered.sort();
        self.tasks_recovered.dedup();
    }

    pub fn format_summary_line(&self, agents_count: usize, final_status: &str) -> String {
        format!(
            "SUPERVISOR_METRICS agents={} heartbeats={} completed_tasks={} lost_agents={} released_tasks={} reassigned_tasks={} reassignment_count={} tasks_recovered={} reallocation_latency_ticks={} survivor_mission_updates={} final_completed_after_reallocation={} final_status={}",
            agents_count,
            self.heartbeat_count,
            self.completed_task_count,
            self.lost_agent_count,
            if self.released_tasks.is_empty() {
                "none".to_owned()
            } else {
                self.released_tasks.join(",")
            },
            if self.reassigned_tasks.is_empty() {
                "none".to_owned()
            } else {
                self.reassigned_tasks.join(",")
            },
            self.reassignment_count,
            if self.tasks_recovered.is_empty() {
                "none".to_owned()
            } else {
                self.tasks_recovered.join(",")
            },
            self.reallocation_latency_ticks
                .map(|ticks| ticks.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            self.survivor_mission_updates,
            self.final_completed_after_reallocation,
            final_status
        )
    }
}

impl From<&SupervisorMetrics> for SitlMultiAgentReallocationReport {
    fn from(metrics: &SupervisorMetrics) -> Self {
        Self {
            lost_agent_count: metrics.lost_agent_count,
            released_tasks: metrics.released_tasks.clone(),
            reassigned_tasks: metrics.reassigned_tasks.clone(),
            reassignment_count: metrics.reassignment_count,
            tasks_recovered: metrics.tasks_recovered.clone(),
            reallocation_latency_ticks: metrics.reallocation_latency_ticks,
            survivor_mission_updates: metrics.survivor_mission_updates,
            final_completed_after_reallocation: metrics.final_completed_after_reallocation,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MissionReplacementPlan {
    pub target_agent_id: String,
    pub failed_agent_id: String,
    pub policy: String,
    pub task_ids: Vec<String>,
    pub waypoints: Vec<SitlWaypointItem>,
}

impl MissionReplacementPlan {
    #[cfg(any(feature = "mavlink-transport", test))]
    fn mission_item_count(&self) -> usize {
        self.waypoints.len()
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveAgentRun {
    pub agent_id: String,
    pub connection_string: String,
    pub system_id: u8,
    pub component_id: u8,
    pub lifecycle: MultiAgentLifecycle,
    pub mission_item_count: usize,
    pub completed_task_count: usize,
    pub final_status: String,
    pub error: Option<String>,
}

impl LiveAgentRun {
    #[cfg(any(feature = "mavlink-transport", test))]
    fn report(&self) -> SitlMultiAgentAgentReport {
        SitlMultiAgentAgentReport {
            agent_id: self.agent_id.clone(),
            connection_string: self.connection_string.clone(),
            system_id: self.system_id,
            component_id: self.component_id,
            lifecycle: match self.lifecycle {
                MultiAgentLifecycle::UploadOnly => "upload_only",
                MultiAgentLifecycle::Execute => "execute",
            }
            .to_owned(),
            mission_item_count: self.mission_item_count,
            completed_task_count: self.completed_task_count,
            final_status: self.final_status.clone(),
            error: self.error.clone(),
        }
    }
}

pub trait LiveAgentController {
    fn agent_id(&self) -> &str;
    fn start_delay_ms(&self) -> u64;
    fn mission_waypoints(&self) -> &[SitlWaypointItem];
    fn replace_mission(&mut self, plan: &MissionReplacementPlan) -> Result<(), SitlError>;
    fn run(&mut self) -> Result<LiveAgentRun, SitlError>;
}

#[cfg(feature = "mavlink-transport")]
pub struct Px4AgentController {
    agent: MultiAgentSitlManifestAgent,
    lifecycle: SitlConnectionLifecycle,
}

#[cfg(feature = "mavlink-transport")]
impl Px4AgentController {
    pub fn new(agent: MultiAgentSitlManifestAgent, lifecycle: SitlConnectionLifecycle) -> Self {
        Self { agent, lifecycle }
    }

    fn failed_run(&self, error: impl Into<String>, completed_task_count: usize) -> LiveAgentRun {
        LiveAgentRun {
            agent_id: self.agent.agent_id.clone(),
            connection_string: self.agent.connection_string.clone(),
            system_id: self.agent.system_id,
            component_id: self.agent.component_id,
            lifecycle: self.agent.lifecycle,
            mission_item_count: self.agent.waypoint_count,
            completed_task_count,
            final_status: "failed".to_owned(),
            error: Some(error.into()),
        }
    }
}

#[cfg(feature = "mavlink-transport")]
impl LiveAgentController for Px4AgentController {
    fn agent_id(&self) -> &str {
        &self.agent.agent_id
    }

    fn start_delay_ms(&self) -> u64 {
        self.agent.start_delay_ms
    }

    fn mission_waypoints(&self) -> &[SitlWaypointItem] {
        &self.agent.waypoints
    }

    fn replace_mission(&mut self, plan: &MissionReplacementPlan) -> Result<(), SitlError> {
        if plan.target_agent_id != self.agent.agent_id {
            return Err(SitlError::MultiAgentConfigInvalid {
                message: format!(
                    "mission replacement target '{}' does not match controller '{}'",
                    plan.target_agent_id, self.agent.agent_id
                ),
            });
        }
        self.agent.task_ids = plan.task_ids.clone();
        self.agent.waypoint_count = plan.waypoints.len();
        self.agent.waypoints = plan.waypoints.clone();
        Ok(())
    }

    fn run(&mut self) -> Result<LiveAgentRun, SitlError> {
        let waypoints = waypoints_from_sitl_items(&self.agent.waypoints);
        let mut transport = match swarm_comms::MavlinkTransport::new(
            &self.agent.connection_string,
            AgentId::from(self.agent.agent_id.clone()),
        ) {
            Ok(transport) => transport,
            Err(error) => return Ok(self.failed_run(error.to_string(), 0)),
        };
        let upload_options = swarm_comms::MissionUploadOptions {
            target_system: self.agent.system_id,
            target_component: self.agent.component_id,
            timeout: self.lifecycle.timeout,
            ..Default::default()
        };
        let lifecycle_options = swarm_comms::MissionLifecycleOptions {
            target_system: self.agent.system_id,
            target_component: self.agent.component_id,
            timeout: self.lifecycle.timeout,
            no_arm: self.lifecycle.no_arm,
            abort_after: self.lifecycle.abort_after,
            takeoff_altitude_m: default_takeoff_altitude(&self.agent.waypoints),
        };

        if let Err(error) = transport.upload_and_execute_mission(
            &waypoints,
            upload_options,
            lifecycle_options.clone(),
        ) {
            return Ok(self.failed_run(error.to_string(), 0));
        }

        match track_live_agent_progress(
            &mut transport,
            &self.agent,
            &self.lifecycle,
            &lifecycle_options,
        ) {
            Ok(report) => Ok(LiveAgentRun {
                agent_id: self.agent.agent_id.clone(),
                connection_string: self.agent.connection_string.clone(),
                system_id: self.agent.system_id,
                component_id: self.agent.component_id,
                lifecycle: self.agent.lifecycle,
                mission_item_count: self.agent.waypoint_count,
                completed_task_count: report.completed_count,
                final_status: live_progress_status_name(report.final_status).to_owned(),
                error: report.failure_reason,
            }),
            Err(error) => Ok(self.failed_run(error.to_string(), 0)),
        }
    }
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
    let controllers = build_mock_controllers(manifest, config);
    let loop_config = SupervisorLoopConfig {
        replay_log: config.replay_log.as_deref(),
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
fn run_live_supervisor_with_controllers<C: LiveAgentController>(
    entry: &swarm_sim::ScenarioSuiteEntry,
    config: &SupervisorLiveConfig,
    manifest: &MultiAgentSitlManifest,
    mut controllers: Vec<C>,
) -> Result<SitlMultiAgentRunReport, SitlError> {
    validate_live_controller_set(manifest, &controllers)?;
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

    eprintln!(
        "Multi-Agent SITL Execute: agents={} scenario={} config={}",
        manifest.agents_count, manifest.scenario_name, config.config_path
    );

    let safety_gate = SitlSafetyGate::new(config.safety_config_path.clone());
    let mut live_metrics = SupervisorMetrics::default();
    let mut reallocation_target_counts: HashMap<String, usize> = HashMap::new();
    let mut lost_agents = HashSet::new();
    let mut runs = Vec::with_capacity(manifest.agents.len());
    for agent in &manifest.agents {
        let start_delay_ms =
            live_controller_for_agent_mut(&mut controllers, &agent.agent_id)?.start_delay_ms();
        if start_delay_ms > 0 {
            thread::sleep(Duration::from_millis(start_delay_ms));
        }
        let mission_waypoints = live_controller_for_agent_mut(&mut controllers, &agent.agent_id)?
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

        let run = live_controller_for_agent_mut(&mut controllers, &agent.agent_id)?.run()?;
        record_live_agent_run(&mut recorder, &mission_waypoints, &run);
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
        if config.reupload_on_failure
            && run.final_status != "completed"
            && lost_agents.insert(run.agent_id.clone())
        {
            let plans = live_reallocation_after_failure(
                entry,
                manifest,
                &runs,
                &run,
                &mut recorder,
                &mut live_metrics,
            )?;
            for plan in plans {
                safety_gate.validate_agent_task_subset(
                    entry,
                    &plan.target_agent_id,
                    &plan.task_ids,
                )?;
                recorder.push_survivor_mission_update_started(
                    plan.target_agent_id.clone(),
                    plan.policy.clone(),
                    plan.task_ids.clone(),
                );
                live_controller_for_agent_mut(&mut controllers, &plan.target_agent_id)?
                    .replace_mission(&plan)?;
                recorder.push_survivor_mission_update_completed(
                    plan.target_agent_id.clone(),
                    plan.policy.clone(),
                    plan.task_ids.clone(),
                    plan.mission_item_count(),
                );
                live_metrics.survivor_mission_updates += 1;
                reallocation_target_counts
                    .insert(plan.target_agent_id.clone(), plan.mission_item_count());
            }
        }
        runs.push(run);
    }

    live_metrics.completed_task_count =
        runs.iter().map(|run| run.completed_task_count as u64).sum();
    live_metrics.finalize();
    let overall_status = live_overall_status(&runs, manifest);
    recorder.push_multi_agent_run_finished(overall_status);
    recorder.push_run_completed(overall_status);

    let report = live_run_report(
        entry,
        config,
        manifest,
        run_id,
        overall_status,
        &runs,
        &live_metrics,
    );
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

fn run_supervisor_with_controllers<C: AgentController>(
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

#[cfg(any(feature = "mavlink-transport", test))]
fn live_reallocation_after_failure(
    entry: &swarm_sim::ScenarioSuiteEntry,
    manifest: &MultiAgentSitlManifest,
    previous_runs: &[LiveAgentRun],
    failed_run: &LiveAgentRun,
    recorder: &mut SitlEventRecorder,
    metrics: &mut SupervisorMetrics,
) -> Result<Vec<MissionReplacementPlan>, SitlError> {
    let previous_agent_ids: HashSet<String> = previous_runs
        .iter()
        .map(|run| run.agent_id.clone())
        .collect();
    let survivor_id = manifest
        .agents
        .iter()
        .find(|agent| {
            agent.agent_id != failed_run.agent_id && !previous_agent_ids.contains(&agent.agent_id)
        })
        .map(|agent| agent.agent_id.clone())
        .ok_or_else(|| SitlError::MultiAgentConfigInvalid {
            message: format!(
                "cannot reallocate failed agent '{}' without a pending survivor",
                failed_run.agent_id
            ),
        })?;
    let own_agent_id = AgentId::from(survivor_id.clone());
    let peer_ids: Vec<AgentId> = manifest
        .agents
        .iter()
        .filter(|agent| agent.agent_id != survivor_id)
        .map(|agent| AgentId::from(agent.agent_id.clone()))
        .collect();
    let mut coordinator = Coordinator::new(
        entry.scenario.agents.clone(),
        entry.scenario.tasks.clone(),
        1,
    );
    assign_manifest_tasks(&mut coordinator, manifest)?;
    for agent_id in &previous_agent_ids {
        coordinator
            .membership
            .mark_dead(&AgentId::from(agent_id.clone()));
    }

    for task_id in completed_live_task_ids(manifest, previous_runs, failed_run) {
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
    for agent in manifest
        .agents
        .iter()
        .filter(|agent| agent.agent_id != failed_run.agent_id && agent.agent_id != survivor_id)
    {
        node.transport.push_incoming(RawMessage {
            from: AgentId::from(agent.agent_id.clone()),
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
    mission_replacement_plans(manifest, previous_runs, failed_run, recovered_by_agent)
}

#[cfg(any(feature = "mavlink-transport", test))]
fn completed_live_task_ids(
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
fn completed_task_ids_for_run(
    manifest: &MultiAgentSitlManifest,
    run: &LiveAgentRun,
) -> Vec<String> {
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

fn record_reallocation_output(
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

fn dedup_strings_preserve_order(items: &mut Vec<String>) {
    let mut seen = HashSet::new();
    items.retain(|item| seen.insert(item.clone()));
}

#[cfg(any(feature = "mavlink-transport", test))]
fn mission_replacement_plans(
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
fn push_recovered_tasks_in_manifest_order(
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
fn push_unique_replacement_task(
    task_ids: &mut Vec<String>,
    task_id: &str,
    completed: &HashSet<String>,
) {
    if !completed.contains(task_id) && !task_ids.iter().any(|existing| existing == task_id) {
        task_ids.push(task_id.to_owned());
    }
}

#[cfg(any(feature = "mavlink-transport", test))]
fn replacement_waypoints_for_task_ids(
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
fn record_live_agent_run(
    recorder: &mut SitlEventRecorder,
    mission_waypoints: &[SitlWaypointItem],
    run: &LiveAgentRun,
) {
    let completed_count = run.completed_task_count.min(mission_waypoints.len());
    for waypoint in mission_waypoints.iter().take(completed_count) {
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
fn live_run_report(
    entry: &swarm_sim::ScenarioSuiteEntry,
    config: &SupervisorLiveConfig,
    manifest: &MultiAgentSitlManifest,
    run_id: String,
    overall_status: &str,
    runs: &[LiveAgentRun],
    metrics: &SupervisorMetrics,
) -> SitlMultiAgentRunReport {
    SitlMultiAgentRunReport {
        schema_version: "sitl_multi_agent_run_report.v1".to_owned(),
        run_id,
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
        overall_status: overall_status.to_owned(),
        event_log_path: config.replay_log.as_ref().map(PathBuf::from),
        reallocation: metrics.into(),
        known_limitations: vec![
            "local PX4/SIH endpoints only unless --allow-hardware-candidate is explicit".to_owned(),
            "agents are orchestrated sequentially in one supervisor process".to_owned(),
            if config.reupload_on_failure {
                "failed-agent reallocation uses controlled local mission replacement; Gazebo, HIL, and hardware are not claimed".to_owned()
            } else {
                "live failed-agent reallocation requires explicit --reupload-on-failure".to_owned()
            },
        ],
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
fn track_live_agent_progress(
    transport: &mut swarm_comms::MavlinkTransport,
    agent: &MultiAgentSitlManifestAgent,
    lifecycle: &SitlConnectionLifecycle,
    lifecycle_options: &swarm_comms::MissionLifecycleOptions,
) -> Result<crate::sitl_progress::SitlMissionProgressReport, SitlError> {
    let mut progress = crate::sitl_progress::SitlTaskProgress::from_waypoints(
        task_ids_by_seq_from_items(&agent.waypoints),
    )
    .map_err(|error| SitlError::ConnectionFailed {
        message: error.to_string(),
    })?;
    let started_at = std::time::Instant::now();
    let mut last_heartbeat_at = Duration::ZERO;
    let mut last_progress_at = Duration::ZERO;

    loop {
        let now = started_at.elapsed();
        if now.saturating_sub(last_heartbeat_at) >= lifecycle.telemetry_timeout {
            let report = progress
                .apply_event(swarm_comms::MavlinkTelemetryEvent::Disconnected, now)
                .map_err(|error| SitlError::ConnectionFailed {
                    message: error.to_string(),
                })?;
            let crate::sitl_progress::SitlProgressUpdate::Failed(report) = report else {
                unreachable!("disconnected telemetry event must fail live SITL progress");
            };
            let abort = transport.abort_mission(lifecycle_options);
            return Ok(append_abort_to_report(report, abort));
        }
        if now.saturating_sub(last_progress_at) >= lifecycle.no_progress_timeout {
            let report = progress.mark_no_progress_timeout(format!(
                "no mission progress before {:?}",
                lifecycle.no_progress_timeout
            ));
            let abort = transport.abort_mission(lifecycle_options);
            return Ok(append_abort_to_report(report, abort));
        }

        let Some(event) =
            transport
                .poll_telemetry_event()
                .map_err(|error| SitlError::ConnectionFailed {
                    message: error.to_string(),
                })?
        else {
            thread::sleep(Duration::from_millis(10));
            continue;
        };

        let previous_seq = progress.current_seq();
        let previous_completed_count = progress.completed_count();
        if matches!(event, swarm_comms::MavlinkTelemetryEvent::Heartbeat) {
            last_heartbeat_at = now;
        }
        let progress_update = progress.apply_event(event.clone(), now).map_err(|error| {
            SitlError::ConnectionFailed {
                message: error.to_string(),
            }
        })?;
        if event_advances_progress(
            &event,
            previous_seq,
            previous_completed_count,
            &progress_update,
        ) {
            last_progress_at = now;
        }

        match progress_update {
            crate::sitl_progress::SitlProgressUpdate::Completed(report) => return Ok(report),
            crate::sitl_progress::SitlProgressUpdate::Failed(report) => {
                let abort = transport.abort_mission(lifecycle_options);
                return Ok(append_abort_to_report(report, abort));
            }
            crate::sitl_progress::SitlProgressUpdate::Heartbeat
            | crate::sitl_progress::SitlProgressUpdate::Current { .. }
            | crate::sitl_progress::SitlProgressUpdate::Reached { .. } => {}
        }
    }
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

    fn fixture_execute_config() -> MultiAgentSitlConfig {
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
      "lifecycle": "execute",
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

    fn fixture_execute_manifest() -> MultiAgentSitlManifest {
        let suite = fixture_suite();
        let config = fixture_execute_config();
        build_multi_agent_manifest(
            &suite,
            "inline-scenario.json",
            "inline-config.json",
            &config,
        )
        .unwrap()
    }

    fn fixture_nonlexical_suite() -> swarm_sim::ScenarioSuite {
        serde_json::from_str(
            r#"{
  "schema_version": "0.1",
  "name": "Supervisor Nonlexical Unit Fixture",
  "description": "in-memory supervisor unit fixture with nonlexical task ids",
  "scenarios": [
    {
      "mission": "sitl",
      "profile": "unit",
      "scenario": {
        "name": "supervisor_nonlexical_unit",
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
            "id": "wp-2",
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
            "id": "wp-10",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 20.0, "y": 30.0, "z": 4.0 },
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

    fn fixture_nonlexical_execute_config() -> MultiAgentSitlConfig {
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
      "lifecycle": "execute",
      "task_ids": ["wp-2", "wp-10"]
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

    fn fixture_nonlexical_execute_manifest() -> MultiAgentSitlManifest {
        let suite = fixture_nonlexical_suite();
        let config = fixture_nonlexical_execute_config();
        build_multi_agent_manifest(
            &suite,
            "nonlexical-scenario.json",
            "inline-config.json",
            &config,
        )
        .unwrap()
    }

    fn fixture_live_config() -> SupervisorLiveConfig {
        SupervisorLiveConfig {
            scenario_path: "inline-scenario.json".to_owned(),
            config_path: "inline-config.json".to_owned(),
            safety_config_path: None,
            replay_log: None,
            run_report: None,
            lifecycle: SitlConnectionLifecycle::default(),
            allow_hardware_candidate: false,
            reupload_on_failure: false,
            run_id: Some("unit-live-run".to_owned()),
        }
    }

    struct FakeAgentController {
        agent_id: String,
        lifecycle: MultiAgentLifecycle,
        fail_upload: bool,
        fail_start: bool,
        heartbeat_until_tick: Option<u64>,
        uploaded: bool,
        started: bool,
        waypoint_count: usize,
        poll_ticks: Vec<u64>,
    }

    impl FakeAgentController {
        fn alive(agent_id: impl Into<String>) -> Self {
            Self {
                agent_id: agent_id.into(),
                lifecycle: MultiAgentLifecycle::Execute,
                fail_upload: false,
                fail_start: false,
                heartbeat_until_tick: None,
                uploaded: false,
                started: false,
                waypoint_count: 0,
                poll_ticks: Vec::new(),
            }
        }

        fn stops_at(agent_id: impl Into<String>, tick: u64) -> Self {
            Self {
                heartbeat_until_tick: Some(tick),
                ..Self::alive(agent_id)
            }
        }

        fn with_upload_failure(mut self) -> Self {
            self.fail_upload = true;
            self
        }

        fn with_start_failure(mut self) -> Self {
            self.fail_start = true;
            self
        }
    }

    impl AgentController for FakeAgentController {
        fn agent_id(&self) -> &str {
            &self.agent_id
        }

        fn lifecycle(&self) -> MultiAgentLifecycle {
            self.lifecycle
        }

        fn upload(&mut self, waypoints: &[SitlWaypointItem]) -> Result<AgentStep, SitlError> {
            if self.fail_upload {
                return Err(SitlError::ConnectionFailed {
                    message: format!("fake upload failure for {}", self.agent_id),
                });
            }
            self.uploaded = true;
            self.waypoint_count = waypoints.len();
            Ok(AgentStep {
                agent_id: self.agent_id.clone(),
                waypoint_count: self.waypoint_count,
            })
        }

        fn start(&mut self) -> Result<AgentStep, SitlError> {
            if self.fail_start {
                return Err(SitlError::ConnectionFailed {
                    message: format!("fake start failure for {}", self.agent_id),
                });
            }
            self.started = true;
            Ok(AgentStep {
                agent_id: self.agent_id.clone(),
                waypoint_count: self.waypoint_count,
            })
        }

        fn poll(&mut self, tick: u64) -> Result<AgentProgress, SitlError> {
            self.poll_ticks.push(tick);
            let heartbeat_seen = self
                .heartbeat_until_tick
                .is_none_or(|heartbeat_until_tick| tick < heartbeat_until_tick);
            Ok(AgentProgress {
                agent_id: self.agent_id.clone(),
                heartbeat_seen,
            })
        }

        fn abort(&mut self, _reason: &str) -> Result<AgentStep, SitlError> {
            Ok(AgentStep {
                agent_id: self.agent_id.clone(),
                waypoint_count: self.waypoint_count,
            })
        }
    }

    struct FakeLiveAgentController {
        run: LiveAgentRun,
        start_delay_ms: u64,
        mission_waypoints: Vec<SitlWaypointItem>,
    }

    impl FakeLiveAgentController {
        fn completed(agent: &MultiAgentSitlManifestAgent) -> Self {
            Self {
                run: LiveAgentRun {
                    agent_id: agent.agent_id.clone(),
                    connection_string: agent.connection_string.clone(),
                    system_id: agent.system_id,
                    component_id: agent.component_id,
                    lifecycle: agent.lifecycle,
                    mission_item_count: agent.waypoint_count,
                    completed_task_count: agent.waypoint_count,
                    final_status: "completed".to_owned(),
                    error: None,
                },
                start_delay_ms: agent.start_delay_ms,
                mission_waypoints: agent.waypoints.clone(),
            }
        }

        fn failed(agent: &MultiAgentSitlManifestAgent, completed_task_count: usize) -> Self {
            Self {
                run: LiveAgentRun {
                    agent_id: agent.agent_id.clone(),
                    connection_string: agent.connection_string.clone(),
                    system_id: agent.system_id,
                    component_id: agent.component_id,
                    lifecycle: agent.lifecycle,
                    mission_item_count: agent.waypoint_count,
                    completed_task_count,
                    final_status: "failed".to_owned(),
                    error: Some("fake live failure".to_owned()),
                },
                start_delay_ms: agent.start_delay_ms,
                mission_waypoints: agent.waypoints.clone(),
            }
        }
    }

    impl LiveAgentController for FakeLiveAgentController {
        fn agent_id(&self) -> &str {
            &self.run.agent_id
        }

        fn start_delay_ms(&self) -> u64 {
            self.start_delay_ms
        }

        fn mission_waypoints(&self) -> &[SitlWaypointItem] {
            &self.mission_waypoints
        }

        fn replace_mission(&mut self, plan: &MissionReplacementPlan) -> Result<(), SitlError> {
            if plan.target_agent_id != self.run.agent_id {
                return Err(SitlError::MultiAgentConfigInvalid {
                    message: format!(
                        "fake live replacement target '{}' does not match '{}'",
                        plan.target_agent_id, self.run.agent_id
                    ),
                });
            }
            self.mission_waypoints = plan.waypoints.clone();
            self.run.mission_item_count = plan.waypoints.len();
            if self.run.final_status == "completed" {
                self.run.completed_task_count = plan.waypoints.len();
            }
            Ok(())
        }

        fn run(&mut self) -> Result<LiveAgentRun, SitlError> {
            Ok(self.run.clone())
        }
    }

    fn fake_controllers() -> Vec<FakeAgentController> {
        vec![
            FakeAgentController::alive("agent-0"),
            FakeAgentController::alive("agent-1"),
        ]
    }

    fn run_fake_supervisor(
        controllers: Vec<FakeAgentController>,
        own_id: &str,
    ) -> Result<SupervisorMetrics, SitlError> {
        run_fake_supervisor_with_ticks(controllers, own_id, 1, 6)
    }

    fn run_fake_supervisor_with_ticks(
        controllers: Vec<FakeAgentController>,
        own_id: &str,
        timeout_ticks: u64,
        max_ticks: u64,
    ) -> Result<SupervisorMetrics, SitlError> {
        let suite = fixture_suite();
        let manifest = fixture_manifest();
        let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
        let loop_config = SupervisorLoopConfig {
            replay_log: None,
            timeout_ticks,
            max_ticks,
            own_id: own_id.to_owned(),
            mode_label: "Fake",
        };
        run_supervisor_with_controllers(entry, &manifest, controllers, &loop_config)
    }

    #[test]
    fn supervisor_metrics_formats_contract_line() {
        let metrics = SupervisorMetrics {
            heartbeat_count: 6,
            completed_task_count: 2,
            lost_agent_count: 1,
            released_tasks: vec!["wp-0".to_owned()],
            reassigned_tasks: vec!["wp-0".to_owned()],
            reassignment_count: 1,
            tasks_recovered: vec!["wp-0".to_owned()],
            reallocation_latency_ticks: Some(0),
            survivor_mission_updates: 1,
            final_completed_after_reallocation: 2,
        };

        assert_eq!(
            metrics.format_summary_line(2, "completed"),
            "SUPERVISOR_METRICS agents=2 heartbeats=6 completed_tasks=2 lost_agents=1 released_tasks=wp-0 reassigned_tasks=wp-0 reassignment_count=1 tasks_recovered=wp-0 reallocation_latency_ticks=0 survivor_mission_updates=1 final_completed_after_reallocation=2 final_status=completed"
        );
    }

    #[test]
    fn fake_supervisor_boundary_completes_happy_path() {
        let metrics = run_fake_supervisor(fake_controllers(), "agent-0").unwrap();

        assert_eq!(metrics.completed_task_count, 2);
        assert_eq!(metrics.lost_agent_count, 0);
        assert_eq!(metrics.reassignment_count, 0);
        assert!(metrics.tasks_recovered.is_empty());
        assert_eq!(metrics.reallocation_latency_ticks, None);
    }

    #[test]
    fn fake_supervisor_boundary_reallocates_after_progress_loss() {
        let controllers = vec![
            FakeAgentController::stops_at("agent-0", 0),
            FakeAgentController::alive("agent-1"),
        ];

        let metrics = run_fake_supervisor(controllers, "agent-1").unwrap();

        assert_eq!(metrics.lost_agent_count, 1);
        assert_eq!(metrics.reassignment_count, 1);
        assert_eq!(metrics.tasks_recovered, vec!["wp-0"]);
        assert_eq!(metrics.reallocation_latency_ticks, Some(0));
        assert_eq!(metrics.completed_task_count, 2);
    }

    #[test]
    fn fake_supervisor_boundary_propagates_upload_failure() {
        let controllers = vec![
            FakeAgentController::alive("agent-0").with_upload_failure(),
            FakeAgentController::alive("agent-1"),
        ];

        let error = run_fake_supervisor(controllers, "agent-0").unwrap_err();
        assert!(error.to_string().contains("fake upload failure"));
    }

    #[test]
    fn fake_supervisor_boundary_propagates_start_failure_after_upload() {
        let controllers = vec![
            FakeAgentController::alive("agent-0").with_start_failure(),
            FakeAgentController::alive("agent-1"),
        ];

        let error = run_fake_supervisor(controllers, "agent-0").unwrap_err();
        assert!(error.to_string().contains("fake start failure"));
    }

    #[test]
    fn fake_supervisor_boundary_rejects_missing_controller() {
        let controllers = vec![FakeAgentController::alive("agent-0")];

        let error = run_fake_supervisor(controllers, "agent-0").unwrap_err();
        assert!(error
            .to_string()
            .contains("missing controller for manifest agent 'agent-1'"));
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

    #[test]
    fn live_supervisor_rejects_upload_only_agent() {
        let manifest = fixture_manifest();
        let config = fixture_live_config();

        let error = validate_live_manifest(&manifest, &config).unwrap_err();

        assert!(error
            .to_string()
            .contains("live supervisor execute requires lifecycle=execute"));
    }

    #[test]
    fn live_supervisor_rejects_hardware_candidate_without_explicit_allow() {
        let mut manifest = fixture_execute_manifest();
        manifest.agents[0].connection_string = "tcpout:192.168.1.10:5760".to_owned();
        let config = fixture_live_config();

        let error = validate_live_manifest(&manifest, &config).unwrap_err();

        assert!(error
            .to_string()
            .contains("requires --allow-hardware-candidate"));
    }

    #[test]
    fn fake_live_supervisor_writes_report_and_replay_log() {
        let suite = fixture_suite();
        let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
        let manifest = fixture_execute_manifest();
        let dir = tempfile::tempdir().unwrap();
        let replay_log = dir.path().join("multi.sitl-log.json");
        let run_report = dir.path().join("multi.run-report.json");
        let mut config = fixture_live_config();
        config.replay_log = Some(replay_log.to_string_lossy().into_owned());
        config.run_report = Some(run_report.to_string_lossy().into_owned());
        let controllers = manifest
            .agents
            .iter()
            .map(FakeLiveAgentController::completed)
            .collect();

        let report =
            run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

        assert_eq!(report.overall_status, "completed");
        assert_eq!(report.total_completed_tasks, 2);
        assert_eq!(report.failed_agents, 0);
        assert_eq!(report.agents.len(), 2);
        assert_eq!(
            report.reallocation,
            SitlMultiAgentReallocationReport::default()
        );

        let log = crate::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
        let summary = crate::sitl_observability::summarize_sitl_event_log(&log);
        assert_eq!(summary.multi_agent_run_started, 1);
        assert_eq!(summary.multi_agent_run_finished, 1);
        assert_eq!(summary.multi_agent_agent_started, 2);
        assert_eq!(summary.multi_agent_agent_finished, 2);
        assert_eq!(summary.multi_agent_mission_count_sent, 2);
        assert_eq!(summary.multi_agent_mission_item_sent, 2);
        assert_eq!(summary.multi_agent_waypoint_reached, 2);
        assert_eq!(summary.multi_agent_task_completed, 2);
        assert_eq!(summary.mission_count_sent, 2);
        assert_eq!(summary.mission_item_sent, 2);
        assert_eq!(summary.waypoint_reached, 2);
        assert_eq!(summary.task_completed, 2);
        assert_eq!(summary.survivor_mission_updates, 0);
        assert_eq!(summary.multi_agent_agent_count, Some(2));
        assert_eq!(summary.final_status.as_deref(), Some("completed"));
        let mission_items: Vec<(String, u16, String)> = log
            .events
            .iter()
            .filter_map(|event| match event {
                crate::sitl_observability::SitlEvent::MultiAgentMissionItemSent {
                    agent_id,
                    seq,
                    task_id: Some(task_id),
                    ..
                } => Some((agent_id.clone(), *seq, task_id.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            mission_items,
            vec![
                ("agent-0".to_owned(), 0, "wp-0".to_owned()),
                ("agent-1".to_owned(), 0, "wp-1".to_owned())
            ]
        );
        let task_completed: Vec<(String, u16, String)> = log
            .events
            .iter()
            .filter_map(|event| match event {
                crate::sitl_observability::SitlEvent::MultiAgentTaskCompleted {
                    agent_id,
                    seq,
                    task_id,
                    ..
                } => Some((agent_id.clone(), *seq, task_id.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            task_completed,
            vec![
                ("agent-0".to_owned(), 0, "wp-0".to_owned()),
                ("agent-1".to_owned(), 0, "wp-1".to_owned())
            ]
        );
        assert!(log.events.iter().all(|event| !matches!(
            event,
            crate::sitl_observability::SitlEvent::MissionCountSent { .. }
                | crate::sitl_observability::SitlEvent::MissionItemSent { .. }
                | crate::sitl_observability::SitlEvent::WaypointReached { .. }
                | crate::sitl_observability::SitlEvent::TaskCompleted { .. }
                | crate::sitl_observability::SitlEvent::Failure { .. }
        )));

        let report_json: SitlMultiAgentRunReport =
            serde_json::from_str(&std::fs::read_to_string(run_report).unwrap()).unwrap();
        assert_eq!(report_json, report);
    }

    #[test]
    fn fake_live_supervisor_reallocates_lost_before_start_to_pending_survivor() {
        let suite = fixture_suite();
        let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
        let manifest = fixture_execute_manifest();
        let dir = tempfile::tempdir().unwrap();
        let replay_log = dir.path().join("m59.sitl-log.json");
        let run_report = dir.path().join("m59.run-report.json");
        let mut config = fixture_live_config();
        config.reupload_on_failure = true;
        config.replay_log = Some(replay_log.to_string_lossy().into_owned());
        config.run_report = Some(run_report.to_string_lossy().into_owned());
        let controllers = vec![
            FakeLiveAgentController::failed(&manifest.agents[0], 0),
            FakeLiveAgentController::completed(&manifest.agents[1]),
        ];

        let report =
            run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

        assert_eq!(report.overall_status, "completed_with_reallocation");
        assert_eq!(report.total_completed_tasks, 2);
        assert_eq!(report.failed_agents, 1);
        assert_eq!(report.reallocation.lost_agent_count, 1);
        assert_eq!(report.reallocation.released_tasks, vec!["wp-0"]);
        assert_eq!(report.reallocation.reassigned_tasks, vec!["wp-0"]);
        assert_eq!(report.reallocation.reassignment_count, 1);
        assert_eq!(report.reallocation.tasks_recovered, vec!["wp-0"]);
        assert_eq!(report.reallocation.reallocation_latency_ticks, Some(0));
        assert_eq!(report.reallocation.survivor_mission_updates, 1);
        assert_eq!(report.reallocation.final_completed_after_reallocation, 2);
        assert_eq!(report.agents[1].mission_item_count, 2);
        assert_eq!(report.agents[1].completed_task_count, 2);

        let log = crate::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
        let summary = crate::sitl_observability::summarize_sitl_event_log(&log);
        assert_eq!(summary.agent_lost, 1);
        assert_eq!(summary.task_released, 1);
        assert_eq!(summary.task_reassigned, 1);
        assert_eq!(summary.reallocation_completed, 1);
        assert_eq!(summary.tasks_recovered, 1);
        assert_eq!(summary.survivor_mission_update_started, 1);
        assert_eq!(summary.survivor_mission_update_completed, 1);
        assert_eq!(summary.survivor_mission_updates, 1);
        assert_eq!(
            summary.final_status.as_deref(),
            Some("completed_with_reallocation")
        );

        let mission_items: Vec<(String, u16, String)> = log
            .events
            .iter()
            .filter_map(|event| match event {
                crate::sitl_observability::SitlEvent::MultiAgentMissionItemSent {
                    agent_id,
                    seq,
                    task_id: Some(task_id),
                    ..
                } => Some((agent_id.clone(), *seq, task_id.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            mission_items,
            vec![
                ("agent-0".to_owned(), 0, "wp-0".to_owned()),
                ("agent-1".to_owned(), 0, "wp-1".to_owned()),
                ("agent-1".to_owned(), 1, "wp-0".to_owned())
            ]
        );

        let report_json: SitlMultiAgentRunReport =
            serde_json::from_str(&std::fs::read_to_string(run_report).unwrap()).unwrap();
        assert_eq!(report_json, report);
    }

    #[test]
    fn fake_live_supervisor_replacement_appends_recovered_tasks_in_manifest_order() {
        let suite = fixture_nonlexical_suite();
        let entry = first_sitl_entry(&suite, "nonlexical-scenario.json").unwrap();
        let manifest = fixture_nonlexical_execute_manifest();
        let dir = tempfile::tempdir().unwrap();
        let replay_log = dir.path().join("m59-nonlexical.sitl-log.json");
        let mut config = fixture_live_config();
        config.reupload_on_failure = true;
        config.scenario_path = "nonlexical-scenario.json".to_owned();
        config.replay_log = Some(replay_log.to_string_lossy().into_owned());
        let controllers = vec![
            FakeLiveAgentController::failed(&manifest.agents[0], 0),
            FakeLiveAgentController::completed(&manifest.agents[1]),
        ];

        let report =
            run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

        assert_eq!(report.overall_status, "completed_with_reallocation");
        assert_eq!(report.total_completed_tasks, 3);
        assert_eq!(report.failed_agents, 1);
        assert_eq!(report.reallocation.survivor_mission_updates, 1);
        assert_eq!(report.reallocation.final_completed_after_reallocation, 3);
        assert_eq!(report.agents[1].mission_item_count, 3);

        let log = crate::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
        let mission_items = multi_agent_mission_items(&log);
        assert_eq!(
            mission_items,
            vec![
                ("agent-0".to_owned(), 0, "wp-2".to_owned()),
                ("agent-0".to_owned(), 1, "wp-10".to_owned()),
                ("agent-1".to_owned(), 0, "wp-1".to_owned()),
                ("agent-1".to_owned(), 1, "wp-2".to_owned()),
                ("agent-1".to_owned(), 2, "wp-10".to_owned())
            ]
        );
    }

    #[test]
    fn fake_live_supervisor_excludes_completed_failed_task_from_replacement() {
        let suite = fixture_nonlexical_suite();
        let entry = first_sitl_entry(&suite, "nonlexical-scenario.json").unwrap();
        let manifest = fixture_nonlexical_execute_manifest();
        let dir = tempfile::tempdir().unwrap();
        let replay_log = dir.path().join("m59-after-one.sitl-log.json");
        let mut config = fixture_live_config();
        config.reupload_on_failure = true;
        config.scenario_path = "nonlexical-scenario.json".to_owned();
        config.replay_log = Some(replay_log.to_string_lossy().into_owned());
        let controllers = vec![
            FakeLiveAgentController::failed(&manifest.agents[0], 1),
            FakeLiveAgentController::completed(&manifest.agents[1]),
        ];

        let report =
            run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

        assert_eq!(report.overall_status, "completed_with_reallocation");
        assert_eq!(report.total_completed_tasks, 3);
        assert_eq!(report.failed_agents, 1);
        assert_eq!(report.reallocation.released_tasks, vec!["wp-10"]);
        assert_eq!(report.reallocation.reassigned_tasks, vec!["wp-10"]);
        assert_eq!(report.reallocation.tasks_recovered, vec!["wp-10"]);
        assert_eq!(report.reallocation.survivor_mission_updates, 1);
        assert_eq!(report.reallocation.final_completed_after_reallocation, 2);
        assert_eq!(report.agents[1].mission_item_count, 2);

        let log = crate::sitl_observability::read_sitl_event_log(&replay_log).unwrap();
        let mission_items = multi_agent_mission_items(&log);
        assert_eq!(
            mission_items,
            vec![
                ("agent-0".to_owned(), 0, "wp-2".to_owned()),
                ("agent-0".to_owned(), 1, "wp-10".to_owned()),
                ("agent-1".to_owned(), 0, "wp-1".to_owned()),
                ("agent-1".to_owned(), 1, "wp-10".to_owned())
            ]
        );
    }

    #[test]
    fn fake_live_supervisor_rejects_reallocation_without_pending_survivor() {
        let suite = fixture_suite();
        let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
        let manifest = fixture_execute_manifest();
        let mut config = fixture_live_config();
        config.reupload_on_failure = true;
        let controllers = vec![
            FakeLiveAgentController::completed(&manifest.agents[0]),
            FakeLiveAgentController::failed(&manifest.agents[1], 0),
        ];

        let error = run_live_supervisor_with_controllers(entry, &config, &manifest, controllers)
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("cannot reallocate failed agent 'agent-1' without a pending survivor"));
    }

    #[test]
    fn fake_live_supervisor_reports_partial_failure() {
        let suite = fixture_suite();
        let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
        let manifest = fixture_execute_manifest();
        let config = fixture_live_config();
        let controllers = vec![
            FakeLiveAgentController::completed(&manifest.agents[0]),
            FakeLiveAgentController::failed(&manifest.agents[1], 0),
        ];

        let report =
            run_live_supervisor_with_controllers(entry, &config, &manifest, controllers).unwrap();

        assert_eq!(report.overall_status, "partial_failed");
        assert_eq!(report.total_completed_tasks, 1);
        assert_eq!(report.failed_agents, 1);
        assert_eq!(report.agents[1].error.as_deref(), Some("fake live failure"));
    }

    fn multi_agent_mission_items(
        log: &crate::sitl_observability::SitlEventLog,
    ) -> Vec<(String, u16, String)> {
        log.events
            .iter()
            .filter_map(|event| match event {
                crate::sitl_observability::SitlEvent::MultiAgentMissionItemSent {
                    agent_id,
                    seq,
                    task_id: Some(task_id),
                    ..
                } => Some((agent_id.clone(), *seq, task_id.clone())),
                _ => None,
            })
            .collect()
    }
}
