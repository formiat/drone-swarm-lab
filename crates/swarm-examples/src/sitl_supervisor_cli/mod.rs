use std::process::ExitCode;

mod cli;
mod exit_codes;
mod output;
mod run;

use exit_codes::{prints_usage, supervisor_exit_code};

pub fn sitl_error_exit_code(error: &crate::sitl_plan::SitlError) -> u8 {
    supervisor_exit_code(error)
}

pub fn run_cli() -> ExitCode {
    match run::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let code = supervisor_exit_code(&error);
            eprintln!("error: {error}");
            if prints_usage(&error) {
                eprintln!("{}", cli::usage());
            }
            ExitCode::from(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::sitl_multi_agent::TaskOwnershipSummary;
    use crate::sitl_observability::SitlEventLogSummary;
    use crate::sitl_report::SitlMultiAgentRunReport;
    use crate::sitl_report::{SitlMultiAgentAgentReport, SitlMultiAgentReallocationReport};

    use exit_codes::{report_failure_exit_code, report_failure_message};

    fn report_with_failed_agent(
        final_status: &str,
        agent_status: &str,
        error: &str,
    ) -> SitlMultiAgentRunReport {
        SitlMultiAgentRunReport {
            schema_version: "sitl_multi_agent_run_report.v1".to_owned(),
            run_id: "run-m60-error-matrix".to_owned(),
            scenario_path: PathBuf::from("scenario.json"),
            scenario_name: "m60_error_matrix".to_owned(),
            config_path: PathBuf::from("config.json"),
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
                completed_task_count: usize::from(final_status == "partial_failed"),
                final_status: agent_status.to_owned(),
                error: Some(error.to_owned()),
                failure_mode: None,
                tasks_abandoned: Vec::new(),
            }],
            total_completed_tasks: usize::from(final_status == "partial_failed"),
            failed_agents: 1,
            aborted_agents: usize::from(agent_status == "aborted"),
            overall_status: final_status.to_owned(),
            event_log_path: Some(PathBuf::from("events.sitl-log.json")),
            task_ownership: TaskOwnershipSummary::default(),
            events_summary: SitlEventLogSummary::default(),
            final_status: final_status.to_owned(),
            reallocation: SitlMultiAgentReallocationReport::default(),
            degraded: crate::sitl_supervisor::SitlDegradedRunReport::default(),
            limitations: Vec::new(),
            known_limitations: Vec::new(),
        }
    }

    #[test]
    fn live_report_failure_exit_code_matrix_uses_agent_error() {
        let cases = [
            (
                "failed",
                "failed",
                "connection open failed: endpoint udp:127.0.0.1:14550 unavailable",
                5,
            ),
            (
                "failed",
                "failed",
                "mission upload failed: MAV_MISSION_ERROR",
                3,
            ),
            (
                "failed",
                "failed",
                "command rejected: MAV_CMD_MISSION_START MAV_RESULT_DENIED",
                3,
            ),
            ("failed", "failed", "heartbeat timeout before start", 3),
            ("failed", "failed", "telemetry timeout after start", 3),
            ("failed", "failed", "no mission progress before timeout", 3),
            ("failed", "aborted", "abort failed: command rejected", 3),
            (
                "partial_failed",
                "failed",
                "agent completed one task then failed after start",
                3,
            ),
        ];

        for (final_status, agent_status, error, expected_code) in cases {
            let report = report_with_failed_agent(final_status, agent_status, error);
            assert_eq!(
                report_failure_exit_code(&report),
                expected_code,
                "wrong exit code for error: {error}"
            );
            assert!(report_failure_message(&report).contains(error));
        }
    }
}
