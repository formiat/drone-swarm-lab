#![allow(unused_imports)]
use super::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;
#[cfg(any(feature = "mavlink-transport", test))]
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
#[cfg(feature = "mavlink-transport")]
use std::time::Instant;

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
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_observability::{summarize_sitl_event_log, SitlEventLogSummary};
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
    pub run_id: Option<String>,
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
pub(super) struct SupervisorLoopConfig<'a> {
    pub(super) replay_log: Option<&'a str>,
    pub(super) run_id: Option<&'a str>,
    pub(super) timeout_ticks: u64,
    pub(super) max_ticks: u64,
    pub(super) own_id: String,
    pub(super) mode_label: &'a str,
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
    pub(super) fn mission_item_count(&self) -> usize {
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
pub struct CompletedWaypoint {
    pub seq: u16,
    pub task_id: String,
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
    pub completed_waypoints: Vec<CompletedWaypoint>,
    pub completed_task_ids: Vec<String>,
    pub final_status: String,
    pub error: Option<String>,
}

impl LiveAgentRun {
    #[cfg(any(feature = "mavlink-transport", test))]
    pub(super) fn report(&self) -> SitlMultiAgentAgentReport {
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
    fn start(&mut self) -> Result<(), SitlError> {
        Ok(())
    }
    fn poll(&mut self) -> Result<Option<LiveAgentRun>, SitlError> {
        Ok(Some(self.run()?))
    }
    fn completed_task_count(&self) -> usize {
        0
    }
    fn completed_waypoints(&self) -> Vec<CompletedWaypoint> {
        Vec::new()
    }
    fn completed_task_ids(&self) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(feature = "mavlink-transport")]
pub struct Px4AgentController {
    agent: MultiAgentSitlManifestAgent,
    lifecycle: SitlConnectionLifecycle,
    state: Option<Px4AgentState>,
    completed_waypoints: Vec<CompletedWaypoint>,
    finished_run: Option<LiveAgentRun>,
}

#[cfg(feature = "mavlink-transport")]
struct Px4AgentState {
    transport: swarm_comms::MavlinkTransport,
    progress: crate::sitl_progress::SitlTaskProgress,
    lifecycle_options: swarm_comms::MissionLifecycleOptions,
    started_at: Instant,
    last_heartbeat_at: Duration,
    last_progress_at: Duration,
}

#[cfg(feature = "mavlink-transport")]
impl Px4AgentController {
    pub fn new(agent: MultiAgentSitlManifestAgent, lifecycle: SitlConnectionLifecycle) -> Self {
        Self {
            agent,
            lifecycle,
            state: None,
            completed_waypoints: Vec::new(),
            finished_run: None,
        }
    }

    fn upload_options(&self) -> swarm_comms::MissionUploadOptions {
        swarm_comms::MissionUploadOptions {
            target_system: self.agent.system_id,
            target_component: self.agent.component_id,
            timeout: self.lifecycle.timeout,
            ..Default::default()
        }
    }

    fn lifecycle_options(&self) -> swarm_comms::MissionLifecycleOptions {
        swarm_comms::MissionLifecycleOptions {
            target_system: self.agent.system_id,
            target_component: self.agent.component_id,
            timeout: self.lifecycle.timeout,
            no_arm: self.lifecycle.no_arm,
            abort_after: self.lifecycle.abort_after,
            takeoff_altitude_m: default_takeoff_altitude(&self.agent.waypoints),
        }
    }

    fn progress_for_current_mission(
        &self,
    ) -> Result<crate::sitl_progress::SitlTaskProgress, SitlError> {
        crate::sitl_progress::SitlTaskProgress::from_waypoints(task_ids_by_seq_from_items(
            &self.agent.waypoints,
        ))
        .map_err(|error| SitlError::ConnectionFailed {
            message: error.to_string(),
        })
    }

    fn failed_run(
        &self,
        error: impl Into<String>,
        completed_waypoints: Vec<CompletedWaypoint>,
    ) -> LiveAgentRun {
        let completed_task_ids = task_ids_from_completed_waypoints(&completed_waypoints);
        LiveAgentRun {
            agent_id: self.agent.agent_id.clone(),
            connection_string: self.agent.connection_string.clone(),
            system_id: self.agent.system_id,
            component_id: self.agent.component_id,
            lifecycle: self.agent.lifecycle,
            mission_item_count: self.agent.waypoint_count,
            completed_task_count: completed_waypoints.len(),
            completed_waypoints,
            completed_task_ids,
            final_status: "failed".to_owned(),
            error: Some(error.into()),
        }
    }

    fn run_from_progress_report(
        &self,
        report: crate::sitl_progress::SitlMissionProgressReport,
        completed_waypoints: Vec<CompletedWaypoint>,
    ) -> LiveAgentRun {
        let completed_task_ids = task_ids_from_completed_waypoints(&completed_waypoints);
        LiveAgentRun {
            agent_id: self.agent.agent_id.clone(),
            connection_string: self.agent.connection_string.clone(),
            system_id: self.agent.system_id,
            component_id: self.agent.component_id,
            lifecycle: self.agent.lifecycle,
            mission_item_count: self.agent.waypoint_count,
            completed_task_count: completed_waypoints.len(),
            completed_waypoints,
            completed_task_ids,
            final_status: live_progress_status_name(report.final_status).to_owned(),
            error: report.failure_reason,
        }
    }

    fn merge_completed_waypoints(&mut self, waypoints: Vec<CompletedWaypoint>) {
        self.completed_waypoints.extend(waypoints);
        dedup_completed_waypoints_preserve_order(&mut self.completed_waypoints);
    }

    fn merge_completed_from_state(&mut self) {
        if let Some(state) = &self.state {
            self.merge_completed_waypoints(completed_waypoints_from_progress(&state.progress));
        }
    }

    fn start_current_mission(&mut self) -> Result<(), SitlError> {
        if self.state.is_some() {
            return Ok(());
        }
        let waypoints = waypoints_from_sitl_items(&self.agent.waypoints);
        let mut transport = match swarm_comms::MavlinkTransport::new(
            &self.agent.connection_string,
            AgentId::from(self.agent.agent_id.clone()),
        ) {
            Ok(transport) => transport,
            Err(error) => {
                self.finished_run =
                    Some(self.failed_run(error.to_string(), self.completed_waypoints.clone()));
                return Ok(());
            }
        };
        let upload_options = self.upload_options();
        let lifecycle_options = self.lifecycle_options();

        if let Err(error) = transport.upload_and_execute_mission(
            &waypoints,
            upload_options,
            lifecycle_options.clone(),
        ) {
            self.finished_run =
                Some(self.failed_run(error.to_string(), self.completed_waypoints.clone()));
            return Ok(());
        }

        self.state = Some(Px4AgentState {
            transport,
            progress: self.progress_for_current_mission()?,
            lifecycle_options,
            started_at: Instant::now(),
            last_heartbeat_at: Duration::ZERO,
            last_progress_at: Duration::ZERO,
        });
        Ok(())
    }

    fn poll_current_mission(&mut self) -> Result<Option<LiveAgentRun>, SitlError> {
        if let Some(run) = self.finished_run.take() {
            return Ok(Some(run));
        }
        let Some(state) = self.state.as_mut() else {
            return Ok(Some(self.failed_run(
                "live PX4 controller was polled before start",
                self.completed_waypoints.clone(),
            )));
        };

        let now = state.started_at.elapsed();
        if now.saturating_sub(state.last_heartbeat_at) >= self.lifecycle.telemetry_timeout {
            let completed_waypoints = completed_waypoints_from_progress(&state.progress);
            let report = state
                .progress
                .apply_event(swarm_comms::MavlinkTelemetryEvent::Disconnected, now)
                .map_err(|error| SitlError::ConnectionFailed {
                    message: error.to_string(),
                })?;
            let crate::sitl_progress::SitlProgressUpdate::Failed(report) = report else {
                unreachable!("disconnected telemetry event must fail live SITL progress");
            };
            let abort = state.transport.abort_mission(&state.lifecycle_options);
            let report = append_abort_to_report(report, abort);
            self.merge_completed_waypoints(completed_waypoints);
            let run = self.run_from_progress_report(report, self.completed_waypoints.clone());
            self.state = None;
            return Ok(Some(run));
        }
        if now.saturating_sub(state.last_progress_at) >= self.lifecycle.no_progress_timeout {
            let completed_waypoints = completed_waypoints_from_progress(&state.progress);
            let report = state.progress.mark_no_progress_timeout(format!(
                "no mission progress before {:?}",
                self.lifecycle.no_progress_timeout
            ));
            let abort = state.transport.abort_mission(&state.lifecycle_options);
            let report = append_abort_to_report(report, abort);
            self.merge_completed_waypoints(completed_waypoints);
            let run = self.run_from_progress_report(report, self.completed_waypoints.clone());
            self.state = None;
            return Ok(Some(run));
        }

        let Some(event) = state.transport.poll_telemetry_event().map_err(|error| {
            SitlError::ConnectionFailed {
                message: error.to_string(),
            }
        })?
        else {
            return Ok(None);
        };

        let previous_seq = state.progress.current_seq();
        let previous_completed_count = state.progress.completed_count();
        if matches!(event, swarm_comms::MavlinkTelemetryEvent::Heartbeat) {
            state.last_heartbeat_at = now;
        }
        let progress_update = state
            .progress
            .apply_event(event.clone(), now)
            .map_err(|error| SitlError::ConnectionFailed {
                message: error.to_string(),
            })?;
        if event_advances_progress(
            &event,
            previous_seq,
            previous_completed_count,
            &progress_update,
        ) {
            state.last_progress_at = now;
        }

        match progress_update {
            crate::sitl_progress::SitlProgressUpdate::Completed(report) => {
                let completed_waypoints = completed_waypoints_from_progress(&state.progress);
                self.merge_completed_waypoints(completed_waypoints);
                let run = self.run_from_progress_report(report, self.completed_waypoints.clone());
                self.state = None;
                Ok(Some(run))
            }
            crate::sitl_progress::SitlProgressUpdate::Failed(report) => {
                let completed_waypoints = completed_waypoints_from_progress(&state.progress);
                let abort = state.transport.abort_mission(&state.lifecycle_options);
                let report = append_abort_to_report(report, abort);
                self.merge_completed_waypoints(completed_waypoints);
                let run = self.run_from_progress_report(report, self.completed_waypoints.clone());
                self.state = None;
                Ok(Some(run))
            }
            crate::sitl_progress::SitlProgressUpdate::Heartbeat
            | crate::sitl_progress::SitlProgressUpdate::Current { .. }
            | crate::sitl_progress::SitlProgressUpdate::Reached { .. } => Ok(None),
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
        let was_active = self.state.is_some();
        if was_active {
            self.merge_completed_from_state();
        }
        self.agent.task_ids = plan.task_ids.clone();
        self.agent.waypoint_count = plan.waypoints.len();
        self.agent.waypoints = plan.waypoints.clone();
        if was_active {
            let waypoints = waypoints_from_sitl_items(&self.agent.waypoints);
            let upload_options = self.upload_options();
            let lifecycle_options = self.lifecycle_options();
            let progress = self.progress_for_current_mission()?;
            let Some(state) = self.state.as_mut() else {
                return Ok(());
            };
            let abort_result = state.transport.abort_mission(&state.lifecycle_options);
            if let Err(error) = state.transport.upload_and_execute_mission(
                &waypoints,
                upload_options,
                lifecycle_options.clone(),
            ) {
                self.finished_run = Some(self.failed_run(
                    format!(
                        "mission replacement failed after abort_result={abort_result:?}: {error}"
                    ),
                    self.completed_waypoints.clone(),
                ));
                self.state = None;
                return Ok(());
            }
            state.progress = progress;
            state.lifecycle_options = lifecycle_options;
            state.started_at = Instant::now();
            state.last_heartbeat_at = Duration::ZERO;
            state.last_progress_at = Duration::ZERO;
        }
        Ok(())
    }

    fn run(&mut self) -> Result<LiveAgentRun, SitlError> {
        self.start_current_mission()?;
        loop {
            if let Some(run) = self.poll_current_mission()? {
                return Ok(run);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn start(&mut self) -> Result<(), SitlError> {
        self.start_current_mission()
    }

    fn poll(&mut self) -> Result<Option<LiveAgentRun>, SitlError> {
        self.poll_current_mission()
    }

    fn completed_task_count(&self) -> usize {
        self.completed_waypoints().len()
    }

    fn completed_waypoints(&self) -> Vec<CompletedWaypoint> {
        let mut completed = self.completed_waypoints.clone();
        if let Some(state) = &self.state {
            completed.extend(completed_waypoints_from_progress(&state.progress));
        }
        dedup_completed_waypoints_preserve_order(&mut completed);
        completed
    }

    fn completed_task_ids(&self) -> Vec<String> {
        task_ids_from_completed_waypoints(&self.completed_waypoints())
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
