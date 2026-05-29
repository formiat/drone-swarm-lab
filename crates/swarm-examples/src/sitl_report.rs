use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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
}
