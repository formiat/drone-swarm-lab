mod events;
mod judge_report;
mod separation;
mod trace;
mod writers;

pub use events::*;
pub use judge_report::*;
pub use separation::*;
pub use trace::*;
pub use writers::*;

use serde::{Deserialize, Serialize};
use swarm_types::{AgentId, Pose, UrbanEdgeId, UrbanNodeId, UrbanObstacleId};

pub const URBAN_ANALYSIS_DEFAULT_SEPARATION_THRESHOLD_M: f64 = 5.0;

/// Text-artifact route trace reconstructed from an Urban replay log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanRouteTrace {
    pub run_id: String,
    pub scenario_name: String,
    pub seed: u64,
    pub agents: Vec<UrbanAgentRouteTrace>,
    pub event_counts: UrbanEventCounts,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanAgentRouteTrace {
    pub agent_id: AgentId,
    pub planned_edge_ids: Vec<UrbanEdgeId>,
    pub route_length_m: f64,
    pub segments: Vec<UrbanTraceSegment>,
    pub pose_trace: Vec<UrbanPoseTracePoint>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanTraceSegment {
    pub segment_index: usize,
    pub edge_id: UrbanEdgeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<UrbanNodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<UrbanNodeId>,
    pub status: UrbanSegmentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entered_tick: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_tick: Option<u64>,
    #[serde(default)]
    pub violation_ticks: Vec<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrbanSegmentStatus {
    Planned,
    Entered,
    Completed,
    Violated,
    NotCompleted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanPoseTracePoint {
    pub tick: u64,
    pub pose: Pose,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanJudgeReport {
    pub run_id: String,
    pub scenario_name: String,
    pub violations: Vec<UrbanJudgeViolationRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanJudgeViolationRecord {
    pub agent_id: AgentId,
    pub tick: u64,
    pub violation_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id: Option<UrbanEdgeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obstacle_id: Option<UrbanObstacleId>,
    pub pose: Pose,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UrbanEventCounts {
    pub route_planned: u64,
    pub segment_entered: u64,
    pub segment_completed: u64,
    pub violation: u64,
    pub patrol_completed: u64,
    pub bus_observed: u64,
    pub bus_detected: u64,
    pub bus_false_positive: u64,
    pub search_completed: u64,
    pub pose_updated: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanSeparationSummary {
    pub threshold_m: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_separation_m: Option<f64>,
    pub separation_violation_count: u64,
    pub route_conflict_count: u64,
    pub conflicts: Vec<UrbanRouteConflict>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanRouteConflict {
    pub agent_a: AgentId,
    pub agent_b: AgentId,
    pub tick: u64,
    pub distance_m: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index_a: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id_a: Option<UrbanEdgeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index_b: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id_b: Option<UrbanEdgeId>,
}

fn optional_id<T: ToString>(id: Option<&T>) -> String {
    id.map(ToString::to_string).unwrap_or_default()
}

#[cfg(test)]
mod tests;
