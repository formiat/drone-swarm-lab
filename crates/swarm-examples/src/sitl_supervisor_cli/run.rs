use std::time::{SystemTime, UNIX_EPOCH};

use crate::sitl_connection::SitlConnectionLifecycle;
use crate::sitl_multi_agent::{
    build_multi_agent_manifest, load_multi_agent_config, MultiAgentSitlManifest,
    SitlArtifactMetadata,
};
use crate::sitl_plan::{check_preflight_or_err, first_sitl_entry, load_sitl_suite, SitlError};
use crate::sitl_supervisor::{
    run_live_supervisor, run_mock_supervisor, SupervisorLiveConfig, SupervisorMetrics,
    SupervisorMockConfig,
};

use super::cli::{parse_args, SupervisorMode};
use super::exit_codes::report_failure_message;
use super::output::{
    ensure_output_paths_available, resolve_output_paths, write_artifact_snapshots_if_requested,
    write_or_print_manifest, write_replay_summary_if_requested,
};

pub(super) fn run() -> Result<(), SitlError> {
    let cli = parse_args()?;
    let suite = load_sitl_suite(&cli.scenario)?;
    let config = load_multi_agent_config(&cli.config)?;
    let mut manifest = build_multi_agent_manifest(&suite, &cli.scenario, &cli.config, &config)?;
    let output_paths = resolve_output_paths(&cli, &manifest);
    manifest.artifact_metadata = artifact_metadata(&output_paths, &cli.scenario, &cli.config);
    ensure_output_paths_available(&output_paths, cli.force)?;
    let entry = first_sitl_entry(&suite, &cli.scenario)?;
    let safety_report = check_preflight_or_err(entry)?;
    write_artifact_snapshots_if_requested(
        &output_paths,
        std::path::Path::new(&cli.scenario),
        std::path::Path::new(&cli.config),
        &manifest.artifact_metadata.command,
        cli.force,
    )?;
    super::output::write_safety_report_if_requested(&output_paths, &safety_report, cli.force)?;

    match cli.mode {
        SupervisorMode::DryRun => {
            write_or_print_manifest(output_paths.manifest.as_deref(), &manifest, cli.force)?;
        }
        SupervisorMode::Mock => {
            let mock_config = SupervisorMockConfig {
                scenario_path: cli.scenario.clone(),
                replay_log: output_paths
                    .replay_log
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                run_id: output_paths.run_id.clone(),
                fail_agent: cli.fail_agent.clone(),
                fail_after_ticks: cli.fail_after_ticks,
                heartbeat_timeout_ticks: cli.heartbeat_timeout_ticks,
                max_ticks: cli.max_ticks,
            };
            let _: SupervisorMetrics = run_mock_supervisor(&suite, &mock_config, &manifest)?;
            write_or_print_manifest(output_paths.manifest.as_deref(), &manifest, cli.force)?;
            write_replay_summary_if_requested(&output_paths, cli.force)?;
        }
        SupervisorMode::Connection => {
            let live_config = SupervisorLiveConfig {
                scenario_path: cli.scenario.clone(),
                config_path: cli.config.clone(),
                safety_config_path: cli.safety_config.clone(),
                replay_log: output_paths
                    .replay_log
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                run_report: output_paths
                    .run_report
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                lifecycle: SitlConnectionLifecycle {
                    timeout: cli.timeout,
                    telemetry_timeout: cli.telemetry_timeout,
                    no_progress_timeout: cli.no_progress_timeout,
                    no_arm: cli.no_arm,
                    abort_after: cli.abort_after,
                },
                allow_hardware_candidate: cli.allow_hardware_candidate,
                reupload_on_failure: cli.reupload_on_failure,
                run_id: output_paths.run_id.clone(),
            };
            let report = run_live_supervisor(&suite, &live_config, &manifest)?;
            write_or_print_manifest(output_paths.manifest.as_deref(), &manifest, cli.force)?;
            write_replay_summary_if_requested(&output_paths, cli.force)?;
            if !matches!(
                report.final_status.as_str(),
                "completed" | "completed_with_reallocation"
            ) {
                return Err(SitlError::ConnectionFailed {
                    message: report_failure_message(&report),
                });
            }
        }
    }
    Ok(())
}

pub(super) fn generated_run_id(manifest: &MultiAgentSitlManifest) -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!(
        "sitl-supervisor-{}-{seconds}",
        sanitize_run_id_component(&manifest.scenario_name)
    )
}

fn artifact_metadata(
    output_paths: &super::output::OutputPaths,
    scenario_path: &str,
    config_path: &str,
) -> SitlArtifactMetadata {
    let command = std::env::args().collect();
    let git_commit = current_git_commit();
    let build_profile = std::env::var("PROFILE").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "debug".to_owned()
        } else {
            "release".to_owned()
        }
    });
    SitlArtifactMetadata {
        command,
        git_commit,
        build_profile,
        run_id: output_paths.run_id.clone(),
        scenario_snapshot_path: output_paths
            .scenario_snapshot
            .clone()
            .or_else(|| Some(scenario_path.into())),
        config_snapshot_path: output_paths
            .config_snapshot
            .clone()
            .or_else(|| Some(config_path.into())),
        command_path: output_paths.command_capture.clone(),
    }
}

fn current_git_commit() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub(super) fn sanitize_run_id_component(value: &str) -> String {
    let mut sanitized = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            sanitized.push(ch);
        } else if !sanitized.ends_with('-') {
            sanitized.push('-');
        }
    }
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "run".to_owned()
    } else {
        sanitized.to_owned()
    }
}
