#[cfg(any(feature = "mavlink-transport", test))]
use std::path::PathBuf;

#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_multi_agent::MultiAgentSitlManifest;
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_observability::SitlEventLogSummary;
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_report::SitlMultiAgentRunReport;

#[cfg(any(feature = "mavlink-transport", test))]
use super::{LiveAgentRun, SupervisorLiveConfig, SupervisorMetrics};

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) struct LiveRunReportInput<'a> {
    pub(super) entry: &'a swarm_sim::ScenarioSuiteEntry,
    pub(super) config: &'a SupervisorLiveConfig,
    pub(super) manifest: &'a MultiAgentSitlManifest,
    pub(super) run_id: String,
    pub(super) overall_status: &'a str,
    pub(super) runs: &'a [LiveAgentRun],
    pub(super) metrics: &'a SupervisorMetrics,
    pub(super) events_summary: SitlEventLogSummary,
}

#[cfg(any(feature = "mavlink-transport", test))]
pub(super) fn live_run_report(input: LiveRunReportInput<'_>) -> SitlMultiAgentRunReport {
    let entry = input.entry;
    let config = input.config;
    let manifest = input.manifest;
    let runs = input.runs;
    let limitations = vec![
        "local PX4/SIH endpoints only unless --allow-hardware-candidate is explicit".to_owned(),
        "agents are started sequentially and polled stepwise in one supervisor process".to_owned(),
        if config.reupload_on_failure {
            "failed-agent reallocation uses controlled local active-survivor mission replacement; Gazebo, HIL, and hardware are not claimed".to_owned()
        } else {
            "live failed-agent reallocation requires explicit --reupload-on-failure".to_owned()
        },
    ];
    SitlMultiAgentRunReport {
        schema_version: "sitl_multi_agent_run_report.v1".to_owned(),
        run_id: input.run_id,
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
        overall_status: input.overall_status.to_owned(),
        event_log_path: config.replay_log.as_ref().map(PathBuf::from),
        task_ownership: manifest.ownership_summary.clone(),
        events_summary: input.events_summary,
        final_status: input.overall_status.to_owned(),
        reallocation: input.metrics.into(),
        limitations: limitations.clone(),
        known_limitations: limitations,
    }
}
