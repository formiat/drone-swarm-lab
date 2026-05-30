use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const SITL_EVENT_LOG_SCHEMA_VERSION: &str = "sitl_event_log.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitlEventLog {
    pub schema_version: String,
    pub run_id: String,
    pub scenario_path: PathBuf,
    pub scenario_name: String,
    pub mission: String,
    pub profile: String,
    pub agent_id: String,
    pub connection_string: Option<String>,
    pub mode: SitlEventLogMode,
    pub events: Vec<SitlEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SitlEventLogMode {
    Mock,
    ConnectionUploadOnly,
    ConnectionExecute,
}

impl SitlEventLogMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::ConnectionUploadOnly => "connection_upload_only",
            Self::ConnectionExecute => "connection_execute",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum SitlEvent {
    ConnectionOpened {
        step: u64,
        mode: SitlEventLogMode,
        connection_string: Option<String>,
    },
    HeartbeatSeen {
        step: u64,
    },
    MissionClearSent {
        step: u64,
    },
    MissionCountSent {
        step: u64,
        count: usize,
    },
    MissionItemRequested {
        step: u64,
        seq: u16,
    },
    MissionItemSent {
        step: u64,
        seq: u16,
        task_id: Option<String>,
    },
    MissionAckReceived {
        step: u64,
        result: String,
        accepted: bool,
    },
    CommandSent {
        step: u64,
        command: String,
    },
    CommandAckReceived {
        step: u64,
        command: String,
        result: String,
        accepted: bool,
    },
    CurrentSeqChanged {
        step: u64,
        seq: u16,
        task_id: Option<String>,
    },
    WaypointReached {
        step: u64,
        seq: u16,
        task_id: Option<String>,
    },
    TaskCompleted {
        step: u64,
        seq: u16,
        task_id: String,
    },
    AbortRequested {
        step: u64,
        result: Option<String>,
    },
    Disconnected {
        step: u64,
        reason: String,
    },
    Failure {
        step: u64,
        status: String,
        error: String,
    },
    AgentLost {
        step: u64,
        agent_id: String,
    },
    TaskReleased {
        step: u64,
        task_id: String,
        previous_agent_id: String,
    },
    TaskReassigned {
        step: u64,
        task_id: String,
        from_agent_id: String,
        to_agent_id: String,
        latency_ticks: u64,
    },
    ReallocationCompleted {
        step: u64,
        failed_agent_id: String,
        reassignment_count: usize,
        tasks_recovered: Vec<String>,
        latency_ticks: u64,
    },
    RunCompleted {
        step: u64,
        status: String,
    },
}

#[derive(Debug, Clone)]
pub struct SitlEventRecorder {
    log: SitlEventLog,
    next_step: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitlEventLogMetadata {
    pub run_id: String,
    pub scenario_path: PathBuf,
    pub scenario_name: String,
    pub mission: String,
    pub profile: String,
    pub agent_id: String,
    pub connection_string: Option<String>,
    pub mode: SitlEventLogMode,
}

impl SitlEventRecorder {
    pub fn new(metadata: SitlEventLogMetadata) -> Self {
        Self {
            log: SitlEventLog {
                schema_version: SITL_EVENT_LOG_SCHEMA_VERSION.to_owned(),
                run_id: metadata.run_id,
                scenario_path: metadata.scenario_path,
                scenario_name: metadata.scenario_name,
                mission: metadata.mission,
                profile: metadata.profile,
                agent_id: metadata.agent_id,
                connection_string: metadata.connection_string,
                mode: metadata.mode,
                events: Vec::new(),
            },
            next_step: 0,
        }
    }

    pub fn push_connection_opened(&mut self) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::ConnectionOpened {
            step,
            mode: self.log.mode.clone(),
            connection_string: self.log.connection_string.clone(),
        });
    }

    pub fn push_heartbeat_seen(&mut self) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::HeartbeatSeen { step });
    }

    pub fn push_mission_clear_sent(&mut self) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::MissionClearSent { step });
    }

    pub fn push_mission_count_sent(&mut self, count: usize) {
        let step = self.next_step();
        self.log
            .events
            .push(SitlEvent::MissionCountSent { step, count });
    }

    pub fn push_mission_item_requested(&mut self, seq: u16) {
        let step = self.next_step();
        self.log
            .events
            .push(SitlEvent::MissionItemRequested { step, seq });
    }

    pub fn push_mission_item_sent(&mut self, seq: u16, task_id: Option<String>) {
        let step = self.next_step();
        self.log
            .events
            .push(SitlEvent::MissionItemSent { step, seq, task_id });
    }

    pub fn push_mission_ack_received(&mut self, result: impl Into<String>, accepted: bool) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::MissionAckReceived {
            step,
            result: result.into(),
            accepted,
        });
    }

    pub fn push_command_sent(&mut self, command: impl Into<String>) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::CommandSent {
            step,
            command: command.into(),
        });
    }

    pub fn push_command_ack_received(
        &mut self,
        command: impl Into<String>,
        result: impl Into<String>,
        accepted: bool,
    ) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::CommandAckReceived {
            step,
            command: command.into(),
            result: result.into(),
            accepted,
        });
    }

    pub fn push_current_seq_changed(&mut self, seq: u16, task_id: Option<String>) {
        let step = self.next_step();
        self.log
            .events
            .push(SitlEvent::CurrentSeqChanged { step, seq, task_id });
    }

    pub fn push_waypoint_reached(&mut self, seq: u16, task_id: Option<String>) {
        let step = self.next_step();
        self.log
            .events
            .push(SitlEvent::WaypointReached { step, seq, task_id });
    }

    pub fn push_task_completed(&mut self, seq: u16, task_id: impl Into<String>) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::TaskCompleted {
            step,
            seq,
            task_id: task_id.into(),
        });
    }

    pub fn push_abort_requested(&mut self, result: Option<String>) {
        let step = self.next_step();
        self.log
            .events
            .push(SitlEvent::AbortRequested { step, result });
    }

    pub fn push_disconnected(&mut self, reason: impl Into<String>) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::Disconnected {
            step,
            reason: reason.into(),
        });
    }

    pub fn push_failure(&mut self, status: impl Into<String>, error: impl Into<String>) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::Failure {
            step,
            status: status.into(),
            error: error.into(),
        });
    }

    pub fn push_agent_lost(&mut self, agent_id: impl Into<String>) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::AgentLost {
            step,
            agent_id: agent_id.into(),
        });
    }

    pub fn push_task_released(
        &mut self,
        task_id: impl Into<String>,
        previous_agent_id: impl Into<String>,
    ) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::TaskReleased {
            step,
            task_id: task_id.into(),
            previous_agent_id: previous_agent_id.into(),
        });
    }

    pub fn push_task_reassigned(
        &mut self,
        task_id: impl Into<String>,
        from_agent_id: impl Into<String>,
        to_agent_id: impl Into<String>,
        latency_ticks: u64,
    ) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::TaskReassigned {
            step,
            task_id: task_id.into(),
            from_agent_id: from_agent_id.into(),
            to_agent_id: to_agent_id.into(),
            latency_ticks,
        });
    }

    pub fn push_reallocation_completed(
        &mut self,
        failed_agent_id: impl Into<String>,
        reassignment_count: usize,
        tasks_recovered: Vec<String>,
        latency_ticks: u64,
    ) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::ReallocationCompleted {
            step,
            failed_agent_id: failed_agent_id.into(),
            reassignment_count,
            tasks_recovered,
            latency_ticks,
        });
    }

    pub fn push_run_completed(&mut self, status: impl Into<String>) {
        let step = self.next_step();
        self.log.events.push(SitlEvent::RunCompleted {
            step,
            status: status.into(),
        });
    }

    pub fn log(&self) -> &SitlEventLog {
        &self.log
    }

    pub fn into_log(self) -> SitlEventLog {
        self.log
    }

    fn next_step(&mut self) -> u64 {
        let step = self.next_step;
        self.next_step += 1;
        step
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SitlEventLogSummary {
    pub run_id: String,
    pub scenario_name: String,
    pub agent_id: String,
    pub mode: Option<SitlEventLogMode>,
    pub total_events: usize,
    pub connection_opened: usize,
    pub heartbeat_seen: usize,
    pub mission_clear_sent: usize,
    pub mission_count_sent: usize,
    pub mission_item_requested: usize,
    pub mission_item_sent: usize,
    pub mission_ack_accepted: usize,
    pub mission_ack_rejected: usize,
    pub commands_sent: usize,
    pub command_ack_accepted: usize,
    pub command_ack_rejected: usize,
    pub current_seq_changed: usize,
    pub waypoint_reached: usize,
    pub task_completed: usize,
    pub abort_requested: usize,
    pub disconnected: usize,
    pub failures: usize,
    pub agent_lost: usize,
    pub task_released: usize,
    pub task_reassigned: usize,
    pub reallocation_completed: usize,
    pub tasks_recovered: usize,
    pub reallocation_latency_ticks: Option<u64>,
    pub final_status: Option<String>,
}

pub fn summarize_sitl_event_log(log: &SitlEventLog) -> SitlEventLogSummary {
    let mut summary = SitlEventLogSummary {
        run_id: log.run_id.clone(),
        scenario_name: log.scenario_name.clone(),
        agent_id: log.agent_id.clone(),
        mode: Some(log.mode.clone()),
        total_events: log.events.len(),
        ..Default::default()
    };

    for event in &log.events {
        match event {
            SitlEvent::ConnectionOpened { .. } => summary.connection_opened += 1,
            SitlEvent::HeartbeatSeen { .. } => summary.heartbeat_seen += 1,
            SitlEvent::MissionClearSent { .. } => summary.mission_clear_sent += 1,
            SitlEvent::MissionCountSent { .. } => summary.mission_count_sent += 1,
            SitlEvent::MissionItemRequested { .. } => summary.mission_item_requested += 1,
            SitlEvent::MissionItemSent { .. } => summary.mission_item_sent += 1,
            SitlEvent::MissionAckReceived {
                accepted, result, ..
            } => {
                if *accepted {
                    summary.mission_ack_accepted += 1;
                } else {
                    summary.mission_ack_rejected += 1;
                    summary.final_status.get_or_insert_with(|| result.clone());
                }
            }
            SitlEvent::CommandSent { .. } => summary.commands_sent += 1,
            SitlEvent::CommandAckReceived {
                accepted, result, ..
            } => {
                if *accepted {
                    summary.command_ack_accepted += 1;
                } else {
                    summary.command_ack_rejected += 1;
                    summary.final_status.get_or_insert_with(|| result.clone());
                }
            }
            SitlEvent::CurrentSeqChanged { .. } => summary.current_seq_changed += 1,
            SitlEvent::WaypointReached { .. } => summary.waypoint_reached += 1,
            SitlEvent::TaskCompleted { .. } => summary.task_completed += 1,
            SitlEvent::AbortRequested { .. } => summary.abort_requested += 1,
            SitlEvent::Disconnected { .. } => summary.disconnected += 1,
            SitlEvent::Failure { status, .. } => {
                summary.failures += 1;
                summary.final_status = Some(status.clone());
            }
            SitlEvent::AgentLost { .. } => summary.agent_lost += 1,
            SitlEvent::TaskReleased { .. } => summary.task_released += 1,
            SitlEvent::TaskReassigned { .. } => summary.task_reassigned += 1,
            SitlEvent::ReallocationCompleted {
                tasks_recovered,
                latency_ticks,
                ..
            } => {
                summary.reallocation_completed += 1;
                summary.tasks_recovered += tasks_recovered.len();
                summary
                    .reallocation_latency_ticks
                    .get_or_insert(*latency_ticks);
            }
            SitlEvent::RunCompleted { status, .. } => {
                summary.final_status = Some(status.clone());
            }
        }
    }

    summary
}

pub fn format_sitl_summary(summary: &SitlEventLogSummary) -> String {
    let mode = summary
        .mode
        .as_ref()
        .map(SitlEventLogMode::as_str)
        .unwrap_or("unknown");
    let final_status = summary.final_status.as_deref().unwrap_or("unknown");
    [
        format!("SITL run: {}", summary.run_id),
        format!(
            "Scenario: {} | Agent: {} | Mode: {}",
            summary.scenario_name, summary.agent_id, mode
        ),
        format!("Events: {}", summary.total_events),
        format!(
            "Upload: clear={} count={} requested={} sent={} ack_accepted={} ack_rejected={}",
            summary.mission_clear_sent,
            summary.mission_count_sent,
            summary.mission_item_requested,
            summary.mission_item_sent,
            summary.mission_ack_accepted,
            summary.mission_ack_rejected
        ),
        format!(
            "Commands: sent={} ack_accepted={} ack_rejected={}",
            summary.commands_sent, summary.command_ack_accepted, summary.command_ack_rejected
        ),
        format!(
            "Telemetry: heartbeat={} current_seq={} waypoint_reached={} task_completed={}",
            summary.heartbeat_seen,
            summary.current_seq_changed,
            summary.waypoint_reached,
            summary.task_completed
        ),
        format!(
            "Failures: aborts={} disconnected={} failures={} final_status={}",
            summary.abort_requested, summary.disconnected, summary.failures, final_status
        ),
        format!(
            "Reallocation: agent_lost={} task_released={} task_reassigned={} completed={} tasks_recovered={} latency_ticks={}",
            summary.agent_lost,
            summary.task_released,
            summary.task_reassigned,
            summary.reallocation_completed,
            summary.tasks_recovered,
            summary
                .reallocation_latency_ticks
                .map(|ticks| ticks.to_string())
                .unwrap_or_else(|| "none".to_owned())
        ),
    ]
    .join("\n")
}

#[derive(Debug, thiserror::Error)]
pub enum SitlEventLogError {
    #[error("SITL event log directory create failed {path:?}: {message}")]
    CreateDir { path: PathBuf, message: String },
    #[error("SITL event log serialization failed: {message}")]
    Serialize { message: String },
    #[error("SITL event log read failed {path:?}: {message}")]
    Read { path: PathBuf, message: String },
    #[error("SITL event log deserialization failed {path:?}: {message}")]
    Deserialize { path: PathBuf, message: String },
    #[error("SITL event log write failed {path:?}: {message}")]
    Write { path: PathBuf, message: String },
}

pub fn write_sitl_event_log(
    path: impl AsRef<Path>,
    log: &SitlEventLog,
) -> Result<(), SitlEventLogError> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| SitlEventLogError::CreateDir {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    let json = serde_json::to_string_pretty(log).map_err(|error| SitlEventLogError::Serialize {
        message: error.to_string(),
    })?;
    fs::write(path, json).map_err(|error| SitlEventLogError::Write {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

pub fn read_sitl_event_log(path: impl AsRef<Path>) -> Result<SitlEventLog, SitlEventLogError> {
    let path = path.as_ref();
    let json = fs::read_to_string(path).map_err(|error| SitlEventLogError::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    serde_json::from_str(&json).map_err(|error| SitlEventLogError::Deserialize {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_log() -> SitlEventLog {
        let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
            run_id: "sitl-agent-0".to_owned(),
            scenario_path: PathBuf::from("scenarios/sitl.waypoints.json"),
            scenario_name: "sitl_waypoints_0".to_owned(),
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            agent_id: "agent-0".to_owned(),
            connection_string: Some("udp:127.0.0.1:14550".to_owned()),
            mode: SitlEventLogMode::ConnectionExecute,
        });
        recorder.push_connection_opened();
        recorder.push_heartbeat_seen();
        recorder.push_mission_clear_sent();
        recorder.push_mission_count_sent(2);
        recorder.push_mission_item_requested(0);
        recorder.push_mission_item_sent(0, Some("wp-0".to_owned()));
        recorder.push_mission_item_requested(1);
        recorder.push_mission_item_sent(1, Some("wp-1".to_owned()));
        recorder.push_mission_ack_received("MAV_MISSION_ACCEPTED", true);
        recorder.push_command_sent("MAV_CMD_MISSION_START");
        recorder.push_command_ack_received("MAV_CMD_MISSION_START", "MAV_RESULT_ACCEPTED", true);
        recorder.push_current_seq_changed(1, Some("wp-1".to_owned()));
        recorder.push_waypoint_reached(1, Some("wp-1".to_owned()));
        recorder.push_task_completed(1, "wp-1");
        recorder.push_run_completed("completed");
        recorder.into_log()
    }

    #[test]
    fn event_log_roundtrips_with_snake_case_events() {
        let log = sample_log();
        let json = serde_json::to_string(&log).unwrap();

        assert!(json.contains(r#""type":"mission_item_requested""#));
        assert!(json.contains(r#""mode":"connection_execute""#));
        let restored: SitlEventLog = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, log);
    }

    #[test]
    fn writer_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("sitl-log.json");

        write_sitl_event_log(&path, &sample_log()).unwrap();

        let restored = read_sitl_event_log(path).unwrap();
        assert_eq!(restored.run_id, "sitl-agent-0");
    }

    #[test]
    fn summary_counts_upload_telemetry_and_completion_events() {
        let summary = summarize_sitl_event_log(&sample_log());

        assert_eq!(summary.mission_count_sent, 1);
        assert_eq!(summary.mission_item_requested, 2);
        assert_eq!(summary.mission_item_sent, 2);
        assert_eq!(summary.mission_ack_accepted, 1);
        assert_eq!(summary.waypoint_reached, 1);
        assert_eq!(summary.task_completed, 1);
        assert_eq!(summary.final_status, Some("completed".to_owned()));
    }

    #[test]
    fn summary_counts_failure_and_abort_events() {
        let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
            run_id: "sitl-failed".to_owned(),
            scenario_path: PathBuf::from("scenario.json"),
            scenario_name: "s".to_owned(),
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            agent_id: "agent-0".to_owned(),
            connection_string: None,
            mode: SitlEventLogMode::Mock,
        });
        recorder.push_abort_requested(Some("Accepted".to_owned()));
        recorder.push_disconnected("telemetry timeout");
        recorder.push_failure("disconnected", "telemetry timeout");

        let summary = summarize_sitl_event_log(recorder.log());

        assert_eq!(summary.abort_requested, 1);
        assert_eq!(summary.disconnected, 1);
        assert_eq!(summary.failures, 1);
        assert_eq!(summary.final_status, Some("disconnected".to_owned()));
    }

    #[test]
    fn reallocation_events_roundtrip_and_summarize() {
        let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
            run_id: "sitl-reallocation".to_owned(),
            scenario_path: PathBuf::from("scenario.json"),
            scenario_name: "s".to_owned(),
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            agent_id: "agent-0".to_owned(),
            connection_string: None,
            mode: SitlEventLogMode::Mock,
        });
        recorder.push_agent_lost("agent-1");
        recorder.push_task_released("task-0", "agent-1");
        recorder.push_task_reassigned("task-0", "agent-1", "agent-0", 0);
        recorder.push_reallocation_completed("agent-1", 1, vec!["task-0".to_owned()], 0);
        let log = recorder.into_log();

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains(r#""type":"agent_lost""#));
        assert!(json.contains(r#""type":"task_reassigned""#));
        assert!(json.contains(r#""type":"reallocation_completed""#));
        let restored: SitlEventLog = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, log);

        let summary = summarize_sitl_event_log(&restored);
        assert_eq!(summary.agent_lost, 1);
        assert_eq!(summary.task_released, 1);
        assert_eq!(summary.task_reassigned, 1);
        assert_eq!(summary.reallocation_completed, 1);
        assert_eq!(summary.tasks_recovered, 1);
        assert_eq!(summary.reallocation_latency_ticks, Some(0));
    }

    #[test]
    fn formatted_summary_is_compact_and_contains_counts() {
        let summary = summarize_sitl_event_log(&sample_log());
        let text = format_sitl_summary(&summary);

        assert!(text.contains("SITL run: sitl-agent-0"));
        assert!(text.contains("requested=2"));
        assert!(text.contains("waypoint_reached=1"));
        assert!(text.contains("Reallocation: agent_lost=0"));
        assert!(text.contains("final_status=completed"));
    }
}
