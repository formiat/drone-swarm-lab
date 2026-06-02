use crate::sitl_plan::SitlError;
use crate::sitl_report::SitlMultiAgentRunReport;

pub(super) fn supervisor_exit_code(error: &SitlError) -> u8 {
    match error {
        SitlError::SafetyConfigRead { .. }
        | SitlError::SafetyConfigParse { .. }
        | SitlError::SafetyConfigInvalid { .. }
        | SitlError::SafetyValidationFailed { .. }
        | SitlError::PreflightFailed { .. }
        | SitlError::InvalidScenario { .. }
        | SitlError::UnsupportedCoordinateFrame { .. }
        | SitlError::UrbanRouteExport { .. } => 2,
        SitlError::FeatureMissing { .. }
        | SitlError::BadConnectionString { .. }
        | SitlError::HardwareCandidateRequiresExplicitAllow { .. } => 5,
        SitlError::RunReportWrite { .. }
        | SitlError::ReplayLogWrite { .. }
        | SitlError::ReplaySummaryWrite { .. }
        | SitlError::MultiAgentManifestWrite { .. }
        | SitlError::DryRunArtifactWrite { .. }
        | SitlError::OutputAlreadyExists { .. } => 4,
        SitlError::ConnectionFailed { message } => classify_connection_failure_exit_code(message),
        _ => 2,
    }
}

pub(super) fn classify_connection_failure_exit_code(message: &str) -> u8 {
    let lower = message.to_ascii_lowercase();
    if lower.contains("endpoint")
        || lower.contains("connection open")
        || lower.contains("open failed")
        || lower.contains("transport")
        || lower.contains("connection refused")
        || lower.contains("connection failed")
        || lower.contains("failed to connect")
        || lower.contains("unable to connect")
    {
        5
    } else {
        3
    }
}

pub(super) fn report_failure_message(report: &SitlMultiAgentRunReport) -> String {
    let Some(agent) = report
        .agents
        .iter()
        .find(|agent| agent.final_status != "completed")
    else {
        return format!(
            "supervisor run finished with final_status '{}'",
            report.final_status
        );
    };
    format!(
        "supervisor run finished with final_status '{}'; failed agent '{}' final_status '{}' error: {}",
        report.final_status,
        agent.agent_id,
        agent.final_status,
        agent
            .error
            .as_deref()
            .unwrap_or("agent did not report an error")
    )
}

#[cfg(test)]
pub(super) fn report_failure_exit_code(report: &SitlMultiAgentRunReport) -> u8 {
    classify_connection_failure_exit_code(&report_failure_message(report))
}

pub(super) fn prints_usage(error: &SitlError) -> bool {
    matches!(
        error,
        SitlError::MissingMode
            | SitlError::ConflictingModes
            | SitlError::MissingArgument { .. }
            | SitlError::UnknownArgument { .. }
            | SitlError::LifecycleOptionRequiresConnection { .. }
            | SitlError::LifecycleOptionRequiresExecute { .. }
            | SitlError::InvalidDuration { .. }
            | SitlError::RunReportRequiresExecute { .. }
            | SitlError::MultiAgentConfigInvalid { .. }
    )
}
