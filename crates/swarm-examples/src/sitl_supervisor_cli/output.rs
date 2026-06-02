use std::path::{Path, PathBuf};

use crate::sitl_multi_agent::MultiAgentSitlManifest;
use crate::sitl_observability::{format_sitl_summary, read_sitl_event_log};
use crate::sitl_plan::SitlError;
use swarm_safety::preflight::SafetyValidationReport;

use super::cli::{CliArgs, SupervisorMode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OutputPaths {
    pub(super) manifest: Option<PathBuf>,
    pub(super) replay_log: Option<PathBuf>,
    pub(super) run_report: Option<PathBuf>,
    pub(super) replay_summary: Option<PathBuf>,
    pub(super) safety_report: Option<PathBuf>,
    pub(super) scenario_snapshot: Option<PathBuf>,
    pub(super) config_snapshot: Option<PathBuf>,
    pub(super) command_capture: Option<PathBuf>,
    pub(super) run_id: Option<String>,
}

pub(super) fn resolve_output_paths(
    cli: &CliArgs,
    manifest: &MultiAgentSitlManifest,
) -> OutputPaths {
    let generated_run_id = cli.output_dir.as_ref().map(|_| {
        cli.run_id
            .clone()
            .unwrap_or_else(|| super::run::generated_run_id(manifest))
    });
    let run_id = cli.run_id.clone().or(generated_run_id.clone());

    if let Some(output_dir) = &cli.output_dir {
        let base = PathBuf::from(output_dir).join(generated_run_id.as_deref().unwrap());
        let replay_log = cli.replay_log.as_ref().map(PathBuf::from).or_else(|| {
            (cli.mode != SupervisorMode::DryRun).then(|| base.join("events.sitl-log.json"))
        });
        return OutputPaths {
            manifest: cli
                .manifest
                .as_ref()
                .map(PathBuf::from)
                .or_else(|| Some(base.join("manifest.json"))),
            replay_summary: replay_log.as_ref().map(|_| base.join("replay-summary.txt")),
            replay_log,
            run_report: cli.run_report.as_ref().map(PathBuf::from).or_else(|| {
                (cli.mode == SupervisorMode::Connection).then(|| base.join("run-report.json"))
            }),
            safety_report: Some(base.join("safety_validation_report.v1.json")),
            scenario_snapshot: Some(base.join("scenario.snapshot.json")),
            config_snapshot: Some(base.join("config.snapshot.json")),
            command_capture: Some(base.join("command.txt")),
            run_id,
        };
    }

    OutputPaths {
        manifest: cli.manifest.as_ref().map(PathBuf::from),
        replay_log: cli.replay_log.as_ref().map(PathBuf::from),
        run_report: cli.run_report.as_ref().map(PathBuf::from),
        replay_summary: None,
        safety_report: None,
        scenario_snapshot: None,
        config_snapshot: None,
        command_capture: None,
        run_id,
    }
}

pub(super) fn ensure_output_paths_available(
    paths: &OutputPaths,
    force: bool,
) -> Result<(), SitlError> {
    for path in [
        paths.manifest.as_deref(),
        paths.replay_log.as_deref(),
        paths.run_report.as_deref(),
        paths.replay_summary.as_deref(),
        paths.safety_report.as_deref(),
        paths.scenario_snapshot.as_deref(),
        paths.config_snapshot.as_deref(),
        paths.command_capture.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        ensure_output_path_available(path, force)?;
    }
    Ok(())
}

pub(super) fn write_artifact_snapshots_if_requested(
    paths: &OutputPaths,
    scenario_path: &Path,
    config_path: &Path,
    command: &[String],
    force: bool,
) -> Result<(), SitlError> {
    if let Some(path) = paths.scenario_snapshot.as_deref() {
        let contents = std::fs::read_to_string(scenario_path).map_err(|error| {
            run_report_write_error(scenario_path.to_path_buf(), error.to_string())
        })?;
        write_checked_file(path, contents, force, run_report_write_error)?;
        eprintln!(
            "SITL supervisor scenario snapshot written: {}",
            path.display()
        );
    }
    if let Some(path) = paths.config_snapshot.as_deref() {
        let contents = std::fs::read_to_string(config_path).map_err(|error| {
            run_report_write_error(config_path.to_path_buf(), error.to_string())
        })?;
        write_checked_file(path, contents, force, run_report_write_error)?;
        eprintln!(
            "SITL supervisor config snapshot written: {}",
            path.display()
        );
    }
    if let Some(path) = paths.command_capture.as_deref() {
        let contents = format!("{}\n", command.join(" "));
        write_checked_file(path, contents, force, run_report_write_error)?;
        eprintln!(
            "SITL supervisor command capture written: {}",
            path.display()
        );
    }
    Ok(())
}

pub(super) fn write_safety_report_if_requested(
    paths: &OutputPaths,
    report: &SafetyValidationReport,
    force: bool,
) -> Result<(), SitlError> {
    let Some(path) = paths.safety_report.as_deref() else {
        return Ok(());
    };
    let json = serde_json::to_string_pretty(report).map_err(|error| SitlError::RunReportWrite {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    write_checked_file(path, json, force, run_report_write_error)?;
    eprintln!("SITL supervisor safety report written: {}", path.display());
    Ok(())
}

pub(super) fn ensure_output_path_available(path: &Path, force: bool) -> Result<(), SitlError> {
    if !force && path.exists() {
        return Err(SitlError::OutputAlreadyExists {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

pub(super) fn write_checked_file(
    path: &Path,
    contents: impl AsRef<[u8]>,
    force: bool,
    map_error: fn(PathBuf, String) -> SitlError,
) -> Result<(), SitlError> {
    ensure_output_path_available(path, force)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| map_error(parent.to_path_buf(), error.to_string()))?;
    }
    std::fs::write(path, contents).map_err(|error| map_error(path.to_path_buf(), error.to_string()))
}

pub(super) fn write_replay_summary_if_requested(
    paths: &OutputPaths,
    force: bool,
) -> Result<(), SitlError> {
    let (Some(summary_path), Some(replay_log_path)) = (&paths.replay_summary, &paths.replay_log)
    else {
        return Ok(());
    };
    let log =
        read_sitl_event_log(replay_log_path).map_err(|error| SitlError::ReplaySummaryWrite {
            path: summary_path.to_path_buf(),
            message: format!(
                "source event log {} read failed: {error}",
                replay_log_path.display()
            ),
        })?;
    let summary = format_sitl_summary(&crate::sitl_observability::summarize_sitl_event_log(&log));
    write_checked_file(summary_path, summary, force, replay_summary_write_error)?;
    eprintln!(
        "SITL supervisor replay summary written: {}",
        summary_path.display()
    );
    Ok(())
}

pub(super) fn write_or_print_manifest(
    manifest_path: Option<&Path>,
    manifest: &MultiAgentSitlManifest,
    force: bool,
) -> Result<(), SitlError> {
    let json = serde_json::to_string_pretty(manifest).map_err(|error| {
        SitlError::MultiAgentConfigInvalid {
            message: error.to_string(),
        }
    })?;
    let Some(path) = manifest_path else {
        println!("{json}");
        return Ok(());
    };
    write_checked_file(path, json, force, manifest_write_error)?;
    eprintln!("Multi-agent SITL manifest written: {}", path.display());
    Ok(())
}

pub(super) fn manifest_write_error(path: PathBuf, message: String) -> SitlError {
    SitlError::MultiAgentManifestWrite { path, message }
}

pub(super) fn replay_summary_write_error(path: PathBuf, message: String) -> SitlError {
    SitlError::ReplaySummaryWrite { path, message }
}

pub(super) fn run_report_write_error(path: PathBuf, message: String) -> SitlError {
    SitlError::RunReportWrite { path, message }
}
