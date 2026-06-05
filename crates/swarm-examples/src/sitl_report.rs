use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use swarm_command_plane::SwarmCommandArtifactSummary;

use crate::sitl_multi_agent::TaskOwnershipSummary;
use crate::sitl_observability::SitlEventLogSummary;
use crate::sitl_supervisor::SitlDegradedRunReport;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SitlRunMode {
    ConnectionExecute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SitlRunFinalStatus {
    Completed,
    Failed,
    Disconnected,
    Rejected,
    TimedOutNoProgress,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitlRunReport {
    pub schema_version: String,
    pub scenario_path: PathBuf,
    pub scenario_name: String,
    pub mission: String,
    pub profile: String,
    pub agent_id: String,
    pub connection_string: String,
    pub mode: SitlRunMode,
    pub mission_item_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub final_status: SitlRunFinalStatus,
    pub error: Option<String>,
    pub abort_result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitlMultiAgentRunReport {
    pub schema_version: String,
    pub run_id: String,
    pub scenario_path: PathBuf,
    pub scenario_name: String,
    pub config_path: PathBuf,
    pub mission: String,
    pub profile: String,
    pub mode: String,
    pub agents: Vec<SitlMultiAgentAgentReport>,
    pub total_completed_tasks: usize,
    pub failed_agents: usize,
    pub aborted_agents: usize,
    pub overall_status: String,
    pub event_log_path: Option<PathBuf>,
    #[serde(default)]
    pub task_ownership: TaskOwnershipSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_plane: Option<SwarmCommandArtifactSummary>,
    #[serde(default)]
    pub events_summary: SitlEventLogSummary,
    #[serde(default)]
    pub final_status: String,
    #[serde(default)]
    pub reallocation: SitlMultiAgentReallocationReport,
    #[serde(default)]
    pub degraded: SitlDegradedRunReport,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub known_limitations: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitlMultiAgentReallocationReport {
    pub lost_agent_count: u64,
    pub released_tasks: Vec<String>,
    pub reassigned_tasks: Vec<String>,
    pub reassignment_count: u64,
    pub tasks_recovered: Vec<String>,
    pub reallocation_latency_ticks: Option<u64>,
    pub survivor_mission_updates: u64,
    pub final_completed_after_reallocation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitlMultiAgentAgentReport {
    pub agent_id: String,
    pub connection_string: String,
    pub system_id: u8,
    pub component_id: u8,
    pub lifecycle: String,
    pub mission_item_count: usize,
    pub completed_task_count: usize,
    pub final_status: String,
    pub error: Option<String>,
    #[serde(default)]
    pub failure_mode: Option<String>,
    #[serde(default)]
    pub tasks_abandoned: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SitlReportError {
    #[error("report directory create failed {path:?}: {message}")]
    CreateDir { path: PathBuf, message: String },
    #[error("report serialization failed: {message}")]
    Serialize { message: String },
    #[error("report write failed {path:?}: {message}")]
    Write { path: PathBuf, message: String },
}

pub fn write_sitl_run_report(
    path: impl AsRef<Path>,
    report: &SitlRunReport,
) -> Result<(), SitlReportError> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| SitlReportError::CreateDir {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    let json =
        serde_json::to_string_pretty(report).map_err(|error| SitlReportError::Serialize {
            message: error.to_string(),
        })?;
    fs::write(path, json).map_err(|error| SitlReportError::Write {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

pub fn write_sitl_multi_agent_run_report(
    path: impl AsRef<Path>,
    report: &SitlMultiAgentRunReport,
) -> Result<(), SitlReportError> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| SitlReportError::CreateDir {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    let json =
        serde_json::to_string_pretty(report).map_err(|error| SitlReportError::Serialize {
            message: error.to_string(),
        })?;
    fs::write(path, json).map_err(|error| SitlReportError::Write {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn success_report() -> SitlRunReport {
        SitlRunReport {
            schema_version: "sitl_run_report.v1".to_owned(),
            scenario_path: PathBuf::from("scenarios/sitl.waypoints.json"),
            scenario_name: "sitl_waypoints_0".to_owned(),
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            agent_id: "agent-0".to_owned(),
            connection_string: "udp:127.0.0.1:14550".to_owned(),
            mode: SitlRunMode::ConnectionExecute,
            mission_item_count: 3,
            completed_count: 3,
            failed_count: 0,
            final_status: SitlRunFinalStatus::Completed,
            error: None,
            abort_result: None,
        }
    }

    #[test]
    fn success_report_serializes_snake_case_status() {
        let json = serde_json::to_string(&success_report()).unwrap();

        assert!(json.contains(r#""final_status":"completed""#));
        assert!(json.contains(r#""mode":"connection_execute""#));
    }

    #[test]
    fn failure_report_roundtrips_with_error_and_abort_result() {
        let mut report = success_report();
        report.completed_count = 1;
        report.failed_count = 2;
        report.final_status = SitlRunFinalStatus::TimedOutNoProgress;
        report.error = Some("no mission progress before 60s".to_owned());
        report.abort_result = Some("Accepted".to_owned());

        let json = serde_json::to_string_pretty(&report).unwrap();
        let roundtrip: SitlRunReport = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip, report);
        assert!(json.contains("timed_out_no_progress"));
    }

    #[test]
    fn report_writer_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("report.json");

        write_sitl_run_report(&path, &success_report()).unwrap();

        let json = fs::read_to_string(path).unwrap();
        assert!(json.contains(r#""agent_id": "agent-0""#));
    }

    #[test]
    fn multi_agent_report_serializes_statuses() {
        let report = SitlMultiAgentRunReport {
            schema_version: "sitl_multi_agent_run_report.v1".to_owned(),
            run_id: "run-1".to_owned(),
            scenario_path: PathBuf::from("scenarios/sitl.multi-agent.json"),
            scenario_name: "sitl_multi_agent".to_owned(),
            config_path: PathBuf::from("scenarios/sitl.multi-agent.config.json"),
            mission: "sitl".to_owned(),
            profile: "multi-agent".to_owned(),
            mode: "connection_execute".to_owned(),
            agents: vec![SitlMultiAgentAgentReport {
                agent_id: "agent-0".to_owned(),
                connection_string: "udp:127.0.0.1:14550".to_owned(),
                system_id: 1,
                component_id: 1,
                lifecycle: "execute".to_owned(),
                mission_item_count: 2,
                completed_task_count: 2,
                final_status: "completed".to_owned(),
                error: None,
                failure_mode: None,
                tasks_abandoned: Vec::new(),
            }],
            total_completed_tasks: 2,
            failed_agents: 0,
            aborted_agents: 0,
            overall_status: "completed".to_owned(),
            event_log_path: Some(PathBuf::from("run.sitl-log.json")),
            task_ownership: TaskOwnershipSummary {
                total_pose_tasks: 2,
                assigned_task_count: 2,
                unassigned_pose_tasks: Vec::new(),
                duplicate_task_ids: Vec::new(),
            },
            command_plane: None,
            events_summary: SitlEventLogSummary {
                run_id: "run-1".to_owned(),
                scenario_name: "sitl_multi_agent".to_owned(),
                agent_id: "supervisor".to_owned(),
                total_events: 8,
                final_status: Some("completed".to_owned()),
                ..Default::default()
            },
            final_status: "completed".to_owned(),
            reallocation: SitlMultiAgentReallocationReport::default(),
            degraded: SitlDegradedRunReport::default(),
            limitations: vec!["local PX4/SIH only".to_owned()],
            known_limitations: vec!["local PX4/SIH only".to_owned()],
        };

        let json = serde_json::to_string(&report).unwrap();

        assert!(json.contains("sitl_multi_agent_run_report.v1"));
        assert!(json.contains("connection_execute"));
        assert!(json.contains("completed"));
        assert!(json.contains("task_ownership"));
        assert!(json.contains("events_summary"));
        assert!(json.contains("final_status"));
        assert!(json.contains("limitations"));
    }

    #[test]
    fn multi_agent_report_roundtrips_reallocation_metrics() {
        let report = SitlMultiAgentRunReport {
            schema_version: "sitl_multi_agent_run_report.v1".to_owned(),
            run_id: "run-reallocation".to_owned(),
            scenario_path: PathBuf::from("scenarios/sitl.multi-agent.json"),
            scenario_name: "sitl_multi_agent".to_owned(),
            config_path: PathBuf::from("scenarios/sitl.multi-agent.config.json"),
            mission: "sitl".to_owned(),
            profile: "multi-agent".to_owned(),
            mode: "connection_execute".to_owned(),
            agents: Vec::new(),
            total_completed_tasks: 2,
            failed_agents: 1,
            aborted_agents: 0,
            overall_status: "completed_with_reallocation".to_owned(),
            event_log_path: Some(PathBuf::from("run.sitl-log.json")),
            task_ownership: TaskOwnershipSummary {
                total_pose_tasks: 2,
                assigned_task_count: 2,
                unassigned_pose_tasks: Vec::new(),
                duplicate_task_ids: Vec::new(),
            },
            command_plane: None,
            events_summary: SitlEventLogSummary {
                run_id: "run-reallocation".to_owned(),
                scenario_name: "sitl_multi_agent".to_owned(),
                agent_id: "supervisor".to_owned(),
                total_events: 12,
                final_status: Some("completed_with_reallocation".to_owned()),
                ..Default::default()
            },
            final_status: "completed_with_reallocation".to_owned(),
            reallocation: SitlMultiAgentReallocationReport {
                lost_agent_count: 1,
                released_tasks: vec!["wp-0".to_owned()],
                reassigned_tasks: vec!["wp-0".to_owned()],
                reassignment_count: 1,
                tasks_recovered: vec!["wp-0".to_owned()],
                reallocation_latency_ticks: Some(0),
                survivor_mission_updates: 1,
                final_completed_after_reallocation: 2,
            },
            degraded: SitlDegradedRunReport::default(),
            limitations: vec!["controlled local PX4/SIH only".to_owned()],
            known_limitations: vec!["controlled local PX4/SIH only".to_owned()],
        };

        let json = serde_json::to_string_pretty(&report).unwrap();
        let roundtrip: SitlMultiAgentRunReport = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip, report);
        assert!(json.contains("completed_with_reallocation"));
        assert!(json.contains("survivor_mission_updates"));
    }

    #[test]
    fn multi_agent_report_defaults_missing_reallocation_section() {
        let json = r#"{
          "schema_version": "sitl_multi_agent_run_report.v1",
          "run_id": "run-old",
          "scenario_path": "scenario.json",
          "scenario_name": "s",
          "config_path": "config.json",
          "mission": "sitl",
          "profile": "multi-agent",
          "mode": "connection_execute",
          "agents": [],
          "total_completed_tasks": 0,
          "failed_agents": 0,
          "aborted_agents": 0,
          "overall_status": "completed",
          "event_log_path": null,
          "known_limitations": []
        }"#;

        let report: SitlMultiAgentRunReport = serde_json::from_str(json).unwrap();

        assert_eq!(
            report.reallocation,
            SitlMultiAgentReallocationReport::default()
        );
        assert_eq!(report.task_ownership, TaskOwnershipSummary::default());
        assert_eq!(report.events_summary, SitlEventLogSummary::default());
        assert_eq!(report.degraded, SitlDegradedRunReport::default());
        assert_eq!(report.final_status, "");
        assert_eq!(report.limitations, Vec::<String>::new());
    }
}
