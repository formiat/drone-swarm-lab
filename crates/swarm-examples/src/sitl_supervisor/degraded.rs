use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::LiveAgentRun;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisorFailureMode {
    AgentLostBeforeUpload,
    UploadRejected,
    AgentLostAfterUploadBeforeMissionStart,
    NoProgressTimeout,
    HeartbeatLost,
    StaleTelemetry,
    PartialCompletionThenFailure,
    ReplacementMissionRejected,
    SurvivorFailedAfterReplacement,
    UnsafeReplacementRoute,
    BadWaypointOrMissionItem,
    Unknown,
}

impl SupervisorFailureMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AgentLostBeforeUpload => "agent_lost_before_upload",
            Self::UploadRejected => "upload_rejected",
            Self::AgentLostAfterUploadBeforeMissionStart => {
                "agent_lost_after_upload_before_mission_start"
            }
            Self::NoProgressTimeout => "no_progress_timeout",
            Self::HeartbeatLost => "heartbeat_lost",
            Self::StaleTelemetry => "stale_telemetry",
            Self::PartialCompletionThenFailure => "partial_completion_then_failure",
            Self::ReplacementMissionRejected => "replacement_mission_rejected",
            Self::SurvivorFailedAfterReplacement => "survivor_failed_after_replacement",
            Self::UnsafeReplacementRoute => "unsafe_replacement_route",
            Self::BadWaypointOrMissionItem => "bad_waypoint_or_mission_item",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisorDecision {
    Abort,
    Wait,
    ReassignUnfinishedTasks,
    ReleaseTasksToPool,
    MarkPartialSuccess,
    MarkTotalFailure,
    ContinueWithSurvivor,
    RefuseUnsafeReplacement,
}

impl SupervisorDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Abort => "abort",
            Self::Wait => "wait",
            Self::ReassignUnfinishedTasks => "reassign_unfinished_tasks",
            Self::ReleaseTasksToPool => "release_tasks_to_pool",
            Self::MarkPartialSuccess => "mark_partial_success",
            Self::MarkTotalFailure => "mark_total_failure",
            Self::ContinueWithSurvivor => "continue_with_survivor",
            Self::RefuseUnsafeReplacement => "refuse_unsafe_replacement",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradedRunRecord {
    pub failure_mode: SupervisorFailureMode,
    pub decision: SupervisorDecision,
    pub detected_tick: Option<u64>,
    pub detected_after_ms: Option<u64>,
    pub affected_agent_id: String,
    pub tasks_completed_before_failure: Vec<String>,
    pub tasks_recovered: Vec<String>,
    pub tasks_abandoned: Vec<String>,
    pub replacement_mission_id: Option<String>,
    pub recovery_latency_ticks: Option<u64>,
    pub final_status: String,
}

impl Default for DegradedRunRecord {
    fn default() -> Self {
        Self {
            failure_mode: SupervisorFailureMode::Unknown,
            decision: SupervisorDecision::Abort,
            detected_tick: None,
            detected_after_ms: None,
            affected_agent_id: String::new(),
            tasks_completed_before_failure: Vec::new(),
            tasks_recovered: Vec::new(),
            tasks_abandoned: Vec::new(),
            replacement_mission_id: None,
            recovery_latency_ticks: None,
            final_status: "failed".to_owned(),
        }
    }
}

impl DegradedRunRecord {
    pub fn from_failed_run(run: &LiveAgentRun, failure_mode: SupervisorFailureMode) -> Self {
        let decision = default_decision_for_run(run, &failure_mode);
        Self {
            failure_mode,
            decision,
            detected_tick: None,
            detected_after_ms: run.detected_after_ms,
            affected_agent_id: run.agent_id.clone(),
            tasks_completed_before_failure: run.completed_task_ids.clone(),
            tasks_recovered: Vec::new(),
            tasks_abandoned: run.tasks_abandoned.clone(),
            replacement_mission_id: None,
            recovery_latency_ticks: None,
            final_status: run.final_status.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitlDegradedRunReport {
    pub records: Vec<DegradedRunRecord>,
    pub failure_mode_counts: BTreeMap<String, u64>,
    pub decision_counts: BTreeMap<String, u64>,
    pub tasks_abandoned: Vec<String>,
    pub recovery_failed_count: u64,
}

pub fn classify_live_failure(run: &LiveAgentRun) -> SupervisorFailureMode {
    if let Some(failure_mode) = &run.failure_mode {
        return failure_mode.clone();
    }
    let error = run.error.as_deref().unwrap_or("").to_ascii_lowercase();
    if error.contains("before upload") || error.contains("connection open failed") {
        SupervisorFailureMode::AgentLostBeforeUpload
    } else if error.contains("mission_ack")
        || error.contains("mission upload failed")
        || error.contains("upload rejection")
    {
        SupervisorFailureMode::UploadRejected
    } else if error.contains("before start") || error.contains("heartbeat timeout before start") {
        SupervisorFailureMode::AgentLostAfterUploadBeforeMissionStart
    } else if error.contains("no mission progress") || error.contains("no progress") {
        SupervisorFailureMode::NoProgressTimeout
    } else if error.contains("stale telemetry") {
        SupervisorFailureMode::StaleTelemetry
    } else if error.contains("mission replacement failed")
        || error.contains("replacement reject")
        || error.contains("replacement rejected")
    {
        SupervisorFailureMode::ReplacementMissionRejected
    } else if error.contains("survivor failed after replacement") {
        SupervisorFailureMode::SurvivorFailedAfterReplacement
    } else if error.contains("disconnected") || error.contains("heartbeat") {
        SupervisorFailureMode::HeartbeatLost
    } else if error.contains("bad waypoint") || error.contains("bad mission item") {
        SupervisorFailureMode::BadWaypointOrMissionItem
    } else if run.completed_task_count > 0 {
        SupervisorFailureMode::PartialCompletionThenFailure
    } else {
        SupervisorFailureMode::Unknown
    }
}

pub fn default_decision_for_run(
    run: &LiveAgentRun,
    failure_mode: &SupervisorFailureMode,
) -> SupervisorDecision {
    match failure_mode {
        SupervisorFailureMode::AgentLostBeforeUpload
        | SupervisorFailureMode::UploadRejected
        | SupervisorFailureMode::AgentLostAfterUploadBeforeMissionStart
        | SupervisorFailureMode::NoProgressTimeout
        | SupervisorFailureMode::StaleTelemetry
        | SupervisorFailureMode::BadWaypointOrMissionItem
        | SupervisorFailureMode::Unknown => SupervisorDecision::Abort,
        SupervisorFailureMode::PartialCompletionThenFailure
        | SupervisorFailureMode::SurvivorFailedAfterReplacement => {
            if run.completed_task_count > 0 {
                SupervisorDecision::MarkPartialSuccess
            } else {
                SupervisorDecision::MarkTotalFailure
            }
        }
        SupervisorFailureMode::HeartbeatLost => SupervisorDecision::ContinueWithSurvivor,
        SupervisorFailureMode::ReplacementMissionRejected => SupervisorDecision::MarkPartialSuccess,
        SupervisorFailureMode::UnsafeReplacementRoute => {
            SupervisorDecision::RefuseUnsafeReplacement
        }
    }
}

pub fn terminal_decision_for_run(run: &LiveAgentRun) -> SupervisorDecision {
    if run.completed_task_count > 0 {
        SupervisorDecision::MarkPartialSuccess
    } else {
        SupervisorDecision::MarkTotalFailure
    }
}
