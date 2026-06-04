use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use swarm_comms::{
    MavlinkCommonCommand, MavlinkCommonCommandName, MavlinkCommonPlan, MavlinkExpectedAckKind,
    MAVLINK_COMMON_PLAN_SCHEMA_VERSION,
};
use swarm_safety::preflight::SafetyValidationReport;

use crate::sitl_multi_agent::{MultiAgentSitlManifest, MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION};
use crate::sitl_observability::{
    format_sitl_summary, read_sitl_event_log, summarize_sitl_event_log, SitlEvent, SitlEventLog,
    SitlEventLogSummary,
};
use crate::sitl_plan::SitlDryRunArtifact;
use crate::sitl_report::SitlMultiAgentRunReport;

pub const ARTIFACT_VALIDATION_REPORT_SCHEMA_VERSION: &str = "artifact_validation_report.v1";

pub const RULE_MANIFEST_MISSING: &str = "artifact.manifest_missing";
pub const RULE_MANIFEST_SCHEMA_UNSUPPORTED: &str = "artifact.manifest_schema_unsupported";
pub const RULE_MANIFEST_COMMAND_MISSING: &str = "artifact.manifest_command_missing";
pub const RULE_GIT_COMMIT_MISSING: &str = "artifact.git_commit_missing";
pub const RULE_BUILD_PROFILE_MISSING: &str = "artifact.build_profile_missing";
pub const RULE_RUN_ID_MISMATCH: &str = "artifact.run_id_mismatch";
pub const RULE_OUTPUT_DIR_MISMATCH: &str = "artifact.output_dir_mismatch";
pub const RULE_FINAL_STATUS_MISMATCH: &str = "artifact.final_status_mismatch";
pub const RULE_COMPLETED_TASK_MISSING_EVENT: &str = "artifact.completed_task_missing_event";
pub const RULE_REPLAY_SUMMARY_COUNT_MISMATCH: &str = "artifact.replay_summary_count_mismatch";
pub const RULE_REPLACEMENT_SEQ_MISMATCH: &str = "artifact.replacement_seq_mismatch";
pub const RULE_SAFETY_REPORT_MISSING: &str = "artifact.safety_report_missing";
pub const RULE_LIMITATIONS_MISSING: &str = "artifact.limitations_missing";
pub const RULE_OVERWRITE_POLICY_MISSING: &str = "artifact.overwrite_policy_missing";
pub const RULE_DEGRADED_RECORD_MISSING: &str = "artifact.degraded_record_missing";
pub const RULE_DEGRADED_EVENT_MISSING: &str = "artifact.degraded_event_missing";
pub const RULE_DEGRADED_FINAL_STATUS_MISMATCH: &str = "artifact.degraded_final_status_mismatch";
pub const RULE_DEGRADED_RECOVERY_TASK_MISMATCH: &str = "artifact.degraded_recovery_task_mismatch";
pub const RULE_DEGRADED_UNSUPPORTED_PATH_UNLABELED: &str =
    "artifact.degraded_unsupported_path_unlabeled";
pub const RULE_MAVLINK_PLAN_MISSING: &str = "artifact.mavlink_plan_missing";
pub const RULE_MAVLINK_PLAN_SCHEMA_UNSUPPORTED: &str = "artifact.mavlink_plan_schema_unsupported";
pub const RULE_MAVLINK_PLAN_COMMAND_MISSING: &str = "artifact.mavlink_plan_command_missing";
pub const RULE_MAVLINK_PLAN_ACK_MISSING: &str = "artifact.mavlink_plan_ack_missing";
pub const RULE_MAVLINK_PLAN_ORDER_UNSAFE: &str = "artifact.mavlink_plan_order_unsafe";
pub const RULE_MAVLINK_PLAN_UNSUPPORTED_REQUIRED: &str =
    "artifact.mavlink_plan_unsupported_required";
pub const RULE_MAVLINK_PLAN_IR_HASH_MISSING: &str = "artifact.mavlink_plan_ir_hash_missing";
pub const RULE_PARSE_FAILED: &str = "artifact.parse_failed";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactValidationMode {
    SupervisorRun,
    DryRun,
    Historical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtifactValidationOptions {
    pub mode: ArtifactValidationMode,
    pub allow_historical: bool,
    pub strict: bool,
}

impl Default for ArtifactValidationOptions {
    fn default() -> Self {
        Self {
            mode: ArtifactValidationMode::SupervisorRun,
            allow_historical: false,
            strict: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactPackPaths {
    pub output_dir: PathBuf,
    pub manifest: PathBuf,
    pub event_log: Option<PathBuf>,
    pub run_report: Option<PathBuf>,
    pub replay_summary: Option<PathBuf>,
    pub safety_report: Option<PathBuf>,
    pub scenario_snapshot: Option<PathBuf>,
    pub config_snapshot: Option<PathBuf>,
    pub command_capture: Option<PathBuf>,
    pub dry_run_artifact: Option<PathBuf>,
}

impl ArtifactPackPaths {
    pub fn from_output_dir(output_dir: impl AsRef<Path>) -> Self {
        let output_dir = output_dir.as_ref().to_path_buf();
        Self {
            manifest: output_dir.join("manifest.json"),
            event_log: optional_path(&output_dir, "events.sitl-log.json"),
            run_report: optional_path(&output_dir, "run-report.json"),
            replay_summary: optional_path(&output_dir, "replay-summary.txt"),
            safety_report: optional_path(&output_dir, "safety_validation_report.v1.json"),
            scenario_snapshot: optional_path(&output_dir, "scenario.snapshot.json"),
            config_snapshot: optional_path(&output_dir, "config.snapshot.json"),
            command_capture: optional_path(&output_dir, "command.txt"),
            dry_run_artifact: optional_first_path(
                &output_dir,
                &["sitl_dry_run_artifact.v1.json", "dry-run.json"],
            ),
            output_dir,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactValidationReport {
    pub schema_version: String,
    pub output_dir: PathBuf,
    pub passed: bool,
    pub violations: Vec<ArtifactValidationViolation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactValidationViolation {
    pub rule_id: String,
    pub severity: ArtifactValidationSeverity,
    pub path: Option<PathBuf>,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactValidationSeverity {
    Error,
    Warning,
}

pub fn validate_artifact_pack(
    paths: &ArtifactPackPaths,
    options: ArtifactValidationOptions,
) -> ArtifactValidationReport {
    let mut validator = Validator::new(paths, options);
    validator.validate();
    validator.into_report()
}

fn optional_path(output_dir: &Path, file_name: &str) -> Option<PathBuf> {
    let path = output_dir.join(file_name);
    path.exists().then_some(path)
}

fn optional_first_path(output_dir: &Path, file_names: &[&str]) -> Option<PathBuf> {
    file_names
        .iter()
        .map(|file_name| output_dir.join(file_name))
        .find(|path| path.exists())
}

struct Validator<'a> {
    paths: &'a ArtifactPackPaths,
    options: ArtifactValidationOptions,
    violations: Vec<ArtifactValidationViolation>,
}

impl<'a> Validator<'a> {
    fn new(paths: &'a ArtifactPackPaths, options: ArtifactValidationOptions) -> Self {
        Self {
            paths,
            options,
            violations: Vec::new(),
        }
    }

    fn validate(&mut self) {
        if matches!(self.options.mode, ArtifactValidationMode::DryRun) {
            self.validate_dry_run_artifact();
            return;
        }

        let Some(manifest) = self.load_manifest() else {
            return;
        };
        self.validate_manifest_metadata(&manifest);

        let event_log = self.load_event_log();
        let run_report = self.load_run_report();
        let replay_summary = self.load_replay_summary();
        let _safety_report = self.load_safety_report();

        if let Some(log) = &event_log {
            self.validate_replacement_completion_seq(log);
        }

        if let (Some(log), Some(report)) = (&event_log, &run_report) {
            let summary = summarize_sitl_event_log(log);
            self.validate_run_identity(&manifest, log, report);
            self.validate_output_dir_identity(report);
            self.validate_final_status(report, &summary);
            self.validate_completed_tasks(&manifest, log, report);
            self.validate_event_summary(report, &summary);
            self.validate_degraded_contract(log, report);
            if let Some(summary_text) = &replay_summary {
                self.validate_replay_summary(summary_text, &summary);
            }
        }

        if matches!(self.options.mode, ArtifactValidationMode::SupervisorRun) {
            self.validate_required_supervisor_files();
            if let Some(report) = &run_report {
                self.validate_limitations(report);
            }
        }
    }

    fn into_report(self) -> ArtifactValidationReport {
        let passed = self.violations.iter().all(|violation| {
            violation.severity != ArtifactValidationSeverity::Error
                && (!self.options.strict
                    || violation.severity != ArtifactValidationSeverity::Warning)
        });
        ArtifactValidationReport {
            schema_version: ARTIFACT_VALIDATION_REPORT_SCHEMA_VERSION.to_owned(),
            output_dir: self.paths.output_dir.clone(),
            passed,
            violations: self.violations,
        }
    }

    fn load_manifest(&mut self) -> Option<MultiAgentSitlManifest> {
        if !self.paths.manifest.exists() {
            self.push_error(
                RULE_MANIFEST_MISSING,
                Some(self.paths.manifest.clone()),
                "manifest.json is required for artifact validation",
            );
            return None;
        }
        let manifest = self.load_json::<MultiAgentSitlManifest>(&self.paths.manifest)?;
        if manifest.schema_version != MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION {
            self.push_error(
                RULE_MANIFEST_SCHEMA_UNSUPPORTED,
                Some(self.paths.manifest.clone()),
                format!(
                    "unsupported manifest schema_version '{}' (expected {MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION})",
                    manifest.schema_version
                ),
            );
        }
        Some(manifest)
    }

    fn load_event_log(&mut self) -> Option<SitlEventLog> {
        let path = self.paths.event_log.as_ref()?;
        match read_sitl_event_log(path) {
            Ok(log) => Some(log),
            Err(error) => {
                self.push_error(
                    RULE_PARSE_FAILED,
                    Some(path.clone()),
                    format!("event log parse failed: {error}"),
                );
                None
            }
        }
    }

    fn load_run_report(&mut self) -> Option<SitlMultiAgentRunReport> {
        self.paths
            .run_report
            .as_ref()
            .and_then(|path| self.load_json(path))
    }

    fn load_replay_summary(&mut self) -> Option<String> {
        let path = self.paths.replay_summary.as_ref()?;
        match fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(error) => {
                self.push_error(
                    RULE_PARSE_FAILED,
                    Some(path.clone()),
                    format!("replay summary read failed: {error}"),
                );
                None
            }
        }
    }

    fn load_safety_report(&mut self) -> Option<SafetyValidationReport> {
        let Some(path) = self.paths.safety_report.clone() else {
            if matches!(self.options.mode, ArtifactValidationMode::SupervisorRun) {
                self.push_error(
                    RULE_SAFETY_REPORT_MISSING,
                    None,
                    "supervisor-run artifacts must include safety_validation_report.v1.json",
                );
            }
            return None;
        };
        self.load_json(&path)
    }

    fn validate_dry_run_artifact(&mut self) {
        let Some(path) = self.paths.dry_run_artifact.clone() else {
            self.push_error(
                RULE_MAVLINK_PLAN_MISSING,
                None,
                "dry-run validation requires sitl_dry_run_artifact.v1.json or dry-run.json",
            );
            return;
        };
        let Some(artifact) = self.load_json::<SitlDryRunArtifact>(&path) else {
            return;
        };
        if artifact.schema_version != "sitl_dry_run_artifact.v1" {
            self.push_error(
                RULE_MAVLINK_PLAN_SCHEMA_UNSUPPORTED,
                Some(path.clone()),
                format!(
                    "unsupported dry-run artifact schema_version '{}' (expected sitl_dry_run_artifact.v1)",
                    artifact.schema_version
                ),
            );
        }
        let Some(plan) = artifact.mavlink_common_plan.as_ref() else {
            self.push_error(
                RULE_MAVLINK_PLAN_MISSING,
                Some(path),
                "dry-run artifact is missing mavlink_common_plan",
            );
            return;
        };
        self.validate_mavlink_common_plan(plan, &self.paths.dry_run_artifact);
    }

    fn validate_mavlink_common_plan(&mut self, plan: &MavlinkCommonPlan, path: &Option<PathBuf>) {
        if plan.schema_version != MAVLINK_COMMON_PLAN_SCHEMA_VERSION {
            self.push_error(
                RULE_MAVLINK_PLAN_SCHEMA_UNSUPPORTED,
                path.clone(),
                format!(
                    "unsupported mavlink_common_plan schema_version '{}' (expected {MAVLINK_COMMON_PLAN_SCHEMA_VERSION})",
                    plan.schema_version
                ),
            );
        }
        if plan.command_ir_hash.trim().is_empty() {
            self.push_error(
                RULE_MAVLINK_PLAN_IR_HASH_MISSING,
                path.clone(),
                "mavlink_common_plan.command_ir_hash is required",
            );
        }
        if plan.command_prelude.is_empty() && plan.mission_items.is_empty() {
            self.push_error(
                RULE_MAVLINK_PLAN_COMMAND_MISSING,
                path.clone(),
                "mavlink_common_plan must contain command_prelude or mission_items",
            );
        }
        self.validate_mavlink_mission_item_sequences(plan, path);
        self.validate_mavlink_ordering(plan, path);
        self.validate_mavlink_expected_acks(plan, path);
        let has_required_unsupported = plan
            .unsupported_features
            .iter()
            .any(|feature| feature.required);
        if has_required_unsupported && plan.validation_result.passed {
            self.push_error(
                RULE_MAVLINK_PLAN_UNSUPPORTED_REQUIRED,
                path.clone(),
                "validation_result.passed must be false when unsupported required features are present",
            );
        }
    }

    fn validate_mavlink_ordering(&mut self, plan: &MavlinkCommonPlan, path: &Option<PathBuf>) {
        if plan.mission_items.is_empty() {
            return;
        }
        for command in &plan.command_prelude {
            if matches!(
                command.command,
                MavlinkCommonCommandName::NavLand | MavlinkCommonCommandName::NavReturnToLaunch
            ) {
                self.push_error(
                    RULE_MAVLINK_PLAN_ORDER_UNSAFE,
                    path.clone(),
                    format!(
                        "post-route lifecycle command '{}' cannot be in command_prelude when mission_items are present",
                        command.command.as_str()
                    ),
                );
            }
        }
    }

    fn validate_mavlink_mission_item_sequences(
        &mut self,
        plan: &MavlinkCommonPlan,
        path: &Option<PathBuf>,
    ) {
        for (expected, item) in plan.mission_items.iter().enumerate() {
            if item.seq as usize != expected {
                self.push_error(
                    RULE_MAVLINK_PLAN_COMMAND_MISSING,
                    path.clone(),
                    format!(
                        "mavlink_common_plan mission item seq {} is not contiguous from expected {expected}",
                        item.seq
                    ),
                );
            }
        }
    }

    fn validate_mavlink_expected_acks(&mut self, plan: &MavlinkCommonPlan, path: &Option<PathBuf>) {
        self.validate_mavlink_command_acks(&plan.command_prelude, plan, path);
        self.validate_mavlink_command_acks(&plan.command_postlude, plan, path);
        if !plan.mission_items.is_empty()
            && !plan
                .expected_acks
                .iter()
                .any(|ack| ack.kind == MavlinkExpectedAckKind::MissionAck)
        {
            self.push_error(
                RULE_MAVLINK_PLAN_ACK_MISSING,
                path.clone(),
                "missing MISSION_ACK expectation for uploaded mission items",
            );
        }
        if let Some(start) = plan.mission_start.as_ref() {
            let covered = plan.expected_acks.iter().any(|ack| {
                ack.kind == MavlinkExpectedAckKind::CommandAck
                    && ack.command_id.as_deref() == Some(start.command_id.as_str())
                    && ack.command == Some(start.command)
            });
            if !covered {
                self.push_error(
                    RULE_MAVLINK_PLAN_ACK_MISSING,
                    path.clone(),
                    "missing COMMAND_ACK expectation for mission_start",
                );
            }
        }
    }

    fn validate_mavlink_command_acks(
        &mut self,
        commands: &[MavlinkCommonCommand],
        plan: &MavlinkCommonPlan,
        path: &Option<PathBuf>,
    ) {
        for command in commands {
            let covered = plan.expected_acks.iter().any(|ack| {
                ack.kind == MavlinkExpectedAckKind::CommandAck
                    && ack.command_id.as_deref() == Some(command.command_id.as_str())
                    && ack.command == Some(command.command)
            });
            if !covered {
                self.push_error(
                    RULE_MAVLINK_PLAN_ACK_MISSING,
                    path.clone(),
                    format!(
                        "missing COMMAND_ACK expectation for command_id '{}'",
                        command.command_id
                    ),
                );
            }
        }
    }

    fn load_json<T>(&mut self, path: &Path) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                self.push_error(
                    RULE_PARSE_FAILED,
                    Some(path.to_path_buf()),
                    format!("read failed: {error}"),
                );
                return None;
            }
        };
        match serde_json::from_str(&text) {
            Ok(value) => Some(value),
            Err(error) => {
                self.push_error(
                    RULE_PARSE_FAILED,
                    Some(path.to_path_buf()),
                    format!("json parse failed: {error}"),
                );
                None
            }
        }
    }

    fn validate_manifest_metadata(&mut self, manifest: &MultiAgentSitlManifest) {
        let metadata = &manifest.artifact_metadata;
        let severity = if self.options.allow_historical
            || matches!(self.options.mode, ArtifactValidationMode::Historical)
        {
            ArtifactValidationSeverity::Warning
        } else {
            ArtifactValidationSeverity::Error
        };
        self.require_metadata(
            !metadata.command.is_empty(),
            RULE_MANIFEST_COMMAND_MISSING,
            severity,
            "manifest artifact_metadata.command is required",
        );
        self.require_metadata(
            metadata
                .git_commit
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty() && value != "unknown"),
            RULE_GIT_COMMIT_MISSING,
            severity,
            "manifest artifact_metadata.git_commit is required",
        );
        self.require_metadata(
            !metadata.build_profile.trim().is_empty() && metadata.build_profile != "unknown",
            RULE_BUILD_PROFILE_MISSING,
            severity,
            "manifest artifact_metadata.build_profile is required",
        );
        self.require_metadata(
            metadata.command_path.is_some(),
            RULE_OVERWRITE_POLICY_MISSING,
            severity,
            "manifest artifact_metadata.command_path should identify captured command.txt",
        );
    }

    fn validate_required_supervisor_files(&mut self) {
        for (path, label) in [
            (&self.paths.event_log, "events.sitl-log.json"),
            (&self.paths.run_report, "run-report.json"),
            (&self.paths.replay_summary, "replay-summary.txt"),
        ] {
            if path.is_none() {
                self.push_error(
                    RULE_PARSE_FAILED,
                    None,
                    format!("supervisor-run artifacts must include {label}"),
                );
            }
        }
    }

    fn validate_run_identity(
        &mut self,
        manifest: &MultiAgentSitlManifest,
        log: &SitlEventLog,
        report: &SitlMultiAgentRunReport,
    ) {
        if report.run_id != log.run_id {
            self.push_error(
                RULE_RUN_ID_MISMATCH,
                self.paths.run_report.clone(),
                format!(
                    "run-report run_id '{}' does not match event log run_id '{}'",
                    report.run_id, log.run_id
                ),
            );
        }
        if let Some(manifest_run_id) = manifest.artifact_metadata.run_id.as_deref() {
            if manifest_run_id != report.run_id {
                self.push_error(
                    RULE_RUN_ID_MISMATCH,
                    Some(self.paths.manifest.clone()),
                    format!(
                        "manifest metadata run_id '{manifest_run_id}' does not match report run_id '{}'",
                        report.run_id
                    ),
                );
            }
        }
    }

    fn validate_output_dir_identity(&mut self, report: &SitlMultiAgentRunReport) {
        let Some(name) = self
            .paths
            .output_dir
            .file_name()
            .and_then(|name| name.to_str())
        else {
            return;
        };
        if name != report.run_id {
            self.push_warning(
                RULE_OUTPUT_DIR_MISMATCH,
                Some(self.paths.output_dir.clone()),
                format!(
                    "output directory basename '{name}' does not match report run_id '{}'",
                    report.run_id
                ),
            );
        }
    }

    fn validate_final_status(
        &mut self,
        report: &SitlMultiAgentRunReport,
        summary: &SitlEventLogSummary,
    ) {
        let Some(summary_status) = summary.final_status.as_deref() else {
            self.push_error(
                RULE_FINAL_STATUS_MISMATCH,
                self.paths.event_log.clone(),
                "event log summary has no final status",
            );
            return;
        };
        if summary_status != report.final_status && summary_status != report.overall_status {
            self.push_error(
                RULE_FINAL_STATUS_MISMATCH,
                self.paths.run_report.clone(),
                format!(
                    "report final_status '{}' / overall_status '{}' does not match event final_status '{summary_status}'",
                    report.final_status, report.overall_status
                ),
            );
        }
    }

    fn validate_completed_tasks(
        &mut self,
        manifest: &MultiAgentSitlManifest,
        log: &SitlEventLog,
        report: &SitlMultiAgentRunReport,
    ) {
        let manifest_tasks: HashSet<&str> = manifest
            .agents
            .iter()
            .flat_map(|agent| agent.task_ids.iter().map(String::as_str))
            .collect();
        let mut completed_count = 0;
        for event in &log.events {
            if let SitlEvent::MultiAgentTaskCompleted { task_id, .. } = event {
                completed_count += 1;
                if !manifest_tasks.contains(task_id.as_str()) {
                    self.push_error(
                        RULE_COMPLETED_TASK_MISSING_EVENT,
                        self.paths.event_log.clone(),
                        format!("completed task_id '{task_id}' is not present in manifest"),
                    );
                }
            }
        }
        if completed_count != report.total_completed_tasks {
            self.push_error(
                RULE_COMPLETED_TASK_MISSING_EVENT,
                self.paths.run_report.clone(),
                format!(
                    "report total_completed_tasks={} does not match event log completed task count={completed_count}",
                    report.total_completed_tasks
                ),
            );
        }
    }

    fn validate_event_summary(
        &mut self,
        report: &SitlMultiAgentRunReport,
        summary: &SitlEventLogSummary,
    ) {
        if report.events_summary != *summary {
            self.push_error(
                RULE_REPLAY_SUMMARY_COUNT_MISMATCH,
                self.paths.run_report.clone(),
                "run-report events_summary does not match recomputed event-log summary",
            );
        }
    }

    fn validate_degraded_contract(&mut self, log: &SitlEventLog, report: &SitlMultiAgentRunReport) {
        let historical = self.options.allow_historical
            || matches!(self.options.mode, ArtifactValidationMode::Historical);
        let needs_degraded = report.failed_agents > 0
            || report.final_status.contains("failed")
            || report.final_status.contains("reallocation")
            || report.overall_status.contains("failed")
            || report.overall_status.contains("reallocation");
        if needs_degraded && report.degraded.records.is_empty() {
            let severity = if historical {
                ArtifactValidationSeverity::Warning
            } else {
                ArtifactValidationSeverity::Error
            };
            self.push(
                RULE_DEGRADED_RECORD_MISSING,
                severity,
                self.paths.run_report.clone(),
                "failed/reallocated supervisor artifacts must include degraded records",
            );
            return;
        }
        if report.degraded.records.is_empty() {
            return;
        }

        for record in &report.degraded.records {
            let mode = record.failure_mode.as_str();
            let decision = record.decision.as_str();
            if mode == "unknown" && !historical {
                self.push_error(
                    RULE_DEGRADED_UNSUPPORTED_PATH_UNLABELED,
                    self.paths.run_report.clone(),
                    format!(
                        "degraded record for agent '{}' uses unknown failure_mode without historical mode",
                        record.affected_agent_id
                    ),
                );
            }
            let has_detected = log.events.iter().any(|event| {
                matches!(
                    event,
                    SitlEvent::SupervisorFailureDetected {
                        agent_id,
                        mode: event_mode,
                        ..
                    } if agent_id == &record.affected_agent_id && event_mode == mode
                )
            });
            let has_classified = log.events.iter().any(|event| {
                matches!(
                    event,
                    SitlEvent::SupervisorFailureClassified {
                        agent_id,
                        mode: event_mode,
                        decision: event_decision,
                        ..
                    } if agent_id == &record.affected_agent_id
                        && event_mode == mode
                        && event_decision == decision
                )
            });
            if !has_detected || !has_classified {
                self.push_error(
                    RULE_DEGRADED_EVENT_MISSING,
                    self.paths.event_log.clone(),
                    format!(
                        "degraded record for agent '{}' mode '{mode}' decision '{decision}' is missing matching replay events",
                        record.affected_agent_id
                    ),
                );
            }
            if !record.tasks_recovered.is_empty() {
                let recovered_event_tasks: HashSet<&str> = log
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        SitlEvent::SupervisorRecoveryCompleted {
                            recovered_task_ids, ..
                        } => Some(recovered_task_ids),
                        _ => None,
                    })
                    .flatten()
                    .map(String::as_str)
                    .collect();
                for task_id in &record.tasks_recovered {
                    if !recovered_event_tasks.contains(task_id.as_str()) {
                        self.push_error(
                            RULE_DEGRADED_RECOVERY_TASK_MISMATCH,
                            self.paths.run_report.clone(),
                            format!(
                                "degraded recovered task_id '{task_id}' is missing from supervisor recovery completed events"
                            ),
                        );
                    }
                }
            }
            if record.final_status != report.final_status
                && record.final_status != report.overall_status
                && record.final_status != "failed_recovery"
            {
                self.push_error(
                    RULE_DEGRADED_FINAL_STATUS_MISMATCH,
                    self.paths.run_report.clone(),
                    format!(
                        "degraded record final_status '{}' does not match report final_status '{}' / overall_status '{}'",
                        record.final_status, report.final_status, report.overall_status
                    ),
                );
            }
        }
    }

    fn validate_replay_summary(&mut self, summary_text: &str, summary: &SitlEventLogSummary) {
        let expected = format_sitl_summary(summary);
        if summary_text.trim_end() != expected.trim_end() {
            self.push_error(
                RULE_REPLAY_SUMMARY_COUNT_MISMATCH,
                self.paths.replay_summary.clone(),
                "replay-summary.txt does not match recomputed event-log summary",
            );
        }
    }

    fn validate_replacement_completion_seq(&mut self, log: &SitlEventLog) {
        let mut active_seq_by_agent_task = HashMap::new();
        for event in &log.events {
            match event {
                SitlEvent::MultiAgentMissionItemSent {
                    agent_id,
                    seq,
                    task_id: Some(task_id),
                    ..
                } => {
                    active_seq_by_agent_task.insert((agent_id.clone(), task_id.clone()), *seq);
                }
                SitlEvent::MultiAgentTaskCompleted {
                    agent_id,
                    seq,
                    task_id,
                    ..
                } => {
                    let key = (agent_id.clone(), task_id.clone());
                    match active_seq_by_agent_task.get(&key) {
                        Some(expected_seq) if expected_seq != seq => self.push_error(
                            RULE_REPLACEMENT_SEQ_MISMATCH,
                            self.paths.event_log.clone(),
                            format!(
                                "completion for agent '{agent_id}' task '{task_id}' uses seq {seq}, expected active mission seq {expected_seq}"
                            ),
                        ),
                        None => self.push_error(
                            RULE_REPLACEMENT_SEQ_MISMATCH,
                            self.paths.event_log.clone(),
                            format!(
                                "completion for agent '{agent_id}' task '{task_id}' has no prior mission item"
                            ),
                        ),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    fn validate_limitations(&mut self, report: &SitlMultiAgentRunReport) {
        if report.limitations.is_empty() || report.known_limitations.is_empty() {
            self.push_error(
                RULE_LIMITATIONS_MISSING,
                self.paths.run_report.clone(),
                "connection_execute run reports must include limitations and known_limitations",
            );
        }
    }

    fn require_metadata(
        &mut self,
        condition: bool,
        rule_id: &'static str,
        severity: ArtifactValidationSeverity,
        reason: &'static str,
    ) {
        if !condition {
            self.push(rule_id, severity, Some(self.paths.manifest.clone()), reason);
        }
    }

    fn push_error(
        &mut self,
        rule_id: &'static str,
        path: Option<PathBuf>,
        reason: impl Into<String>,
    ) {
        self.push(rule_id, ArtifactValidationSeverity::Error, path, reason);
    }

    fn push_warning(
        &mut self,
        rule_id: &'static str,
        path: Option<PathBuf>,
        reason: impl Into<String>,
    ) {
        self.push(rule_id, ArtifactValidationSeverity::Warning, path, reason);
    }

    fn push(
        &mut self,
        rule_id: &'static str,
        severity: ArtifactValidationSeverity,
        path: Option<PathBuf>,
        reason: impl Into<String>,
    ) {
        self.violations.push(ArtifactValidationViolation {
            rule_id: rule_id.to_owned(),
            severity,
            path,
            reason: reason.into(),
        });
    }
}
