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
