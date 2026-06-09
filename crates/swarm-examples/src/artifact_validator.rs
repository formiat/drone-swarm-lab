use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use swarm_command_plane::{
    validate_swarm_command_plan, SwarmCommandArtifactSummary, SwarmCommandPlan,
    SwarmCommandPlaneError, SWARM_COMMAND_PLANE_SCHEMA_VERSION,
};
use swarm_comms::{
    ConflictResolution, DegradedDecisionLog, MavlinkCapabilityProfileId,
    MavlinkCommandCompatibility, MavlinkCommonCommand, MavlinkCommonCommandName, MavlinkCommonPlan,
    MavlinkCompatibilityClass, MavlinkExpectedAckKind, MavlinkPlanPhase, PartitionReport,
    ReconciliationReport, MAVLINK_COMMON_PLAN_SCHEMA_VERSION,
};
use swarm_safety::preflight::SafetyValidationReport;

use crate::sitl_dual_stack_evidence::{
    validate_dual_stack_evidence_pack, validate_dual_stack_profile_evidence,
    ReplacementEvidenceStatus, SitlDualStackEvidencePack, SITL_DUAL_STACK_EVIDENCE_FILE,
};
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
pub const RULE_MAVLINK_PLAN_TELEMETRY_MISSING: &str = "artifact.mavlink_plan_telemetry_missing";
pub const RULE_DRY_RUN_POLICY_MISSING: &str = "artifact.dry_run_policy_missing";
pub const RULE_DRY_RUN_SAFETY_REPORT_FAILED: &str = "artifact.dry_run_safety_report_failed";
pub const RULE_MAVLINK_PLAN_UNSUPPORTED_REQUIRED: &str =
    "artifact.mavlink_plan_unsupported_required";
pub const RULE_MAVLINK_PLAN_IR_HASH_MISSING: &str = "artifact.mavlink_plan_ir_hash_missing";
pub const RULE_MAVLINK_PROFILE_MISSING: &str = "artifact.mavlink_profile_missing";
pub const RULE_MAVLINK_PROFILE_UNKNOWN: &str = "artifact.mavlink_profile_unknown";
pub const RULE_MAVLINK_PROFILE_UNSUPPORTED: &str = "artifact.mavlink_profile_unsupported";
pub const RULE_MAVLINK_PROFILE_HARDWARE_BLOCKING: &str =
    "artifact.mavlink_profile_hardware_blocking";
pub const RULE_MAVLINK_PROFILE_RESULT_MISMATCH: &str = "artifact.mavlink_profile_result_mismatch";
pub const RULE_URBAN_COORDINATE_MODE_MISSING: &str = "artifact.urban_coordinate_mode_missing";
pub const RULE_URBAN_GEO_ROUTE_METADATA_MISSING: &str = "artifact.urban_geo_route_metadata_missing";
pub const RULE_URBAN_WGS84_GEO_MISSING: &str = "artifact.urban_wgs84_geo_missing";
pub const RULE_URBAN_MOCK_PERCEPTION_MISSING: &str = "artifact.urban_mock_perception_missing";
pub const RULE_URBAN_DECONFLICTION_DUPLICATE_SEGMENT_OWNER: &str =
    "artifact.urban_deconfliction_duplicate_segment_owner";
pub const RULE_SWARM_COMMAND_PLANE_MISSING: &str = "artifact.swarm_command_plane_missing";
pub const RULE_SWARM_AGENT_PLAN_MISSING: &str = "artifact.swarm_agent_plan_missing";
pub const RULE_SWARM_DUPLICATE_OWNERSHIP: &str = "artifact.swarm_duplicate_ownership";
pub const RULE_SWARM_ACK_MISMATCH: &str = "artifact.swarm_ack_mismatch";
pub const RULE_SWARM_HANDOFF_MISSING: &str = "artifact.swarm_handoff_missing";
pub const RULE_SWARM_SYNC_PARTIAL_UNREPORTED: &str = "artifact.swarm_sync_partial_unreported";
pub const RULE_SWARM_TOPOLOGY_MISSING: &str = "artifact.swarm_topology_missing";
pub const RULE_SWARM_TOPOLOGY_ROUTE_MISSING: &str = "artifact.swarm_topology_route_missing";
pub const RULE_SWARM_TOPOLOGY_BLOCKED_UNREPORTED: &str =
    "artifact.swarm_topology_blocked_unreported";
pub const RULE_SWARM_MOTHERSHIP_DEPENDENCY_INVALID: &str =
    "artifact.swarm_mothership_dependency_invalid";
pub const RULE_SWARM_TRANSPORT_ASSUMPTION_MISSING: &str =
    "artifact.swarm_transport_assumption_missing";
pub const RULE_DUAL_STACK_EVIDENCE_MISSING: &str = "artifact.dual_stack_evidence_missing";
pub const RULE_DUAL_STACK_PROFILE_MISSING: &str = "artifact.dual_stack_profile_missing";
pub const RULE_DUAL_STACK_PROFILE_MISMATCH: &str = "artifact.dual_stack_profile_mismatch";
pub const RULE_DUAL_STACK_IR_HASH_MISMATCH: &str = "artifact.dual_stack_ir_hash_mismatch";
pub const RULE_DUAL_STACK_HARDWARE_CLAIM_UNSAFE: &str = "artifact.dual_stack_hardware_claim_unsafe";
pub const RULE_DUAL_STACK_ABORT_REPLACEMENT_MISSING: &str =
    "artifact.dual_stack_abort_replacement_missing";
pub const RULE_DUAL_STACK_ABORT_POLICY_MISMATCH: &str = "artifact.dual_stack_abort_policy_mismatch";
pub const RULE_DUAL_STACK_REPLACEMENT_POLICY_MISMATCH: &str =
    "artifact.dual_stack_replacement_policy_mismatch";
pub const RULE_DUAL_STACK_FC_CONTRACT_MISSING: &str = "artifact.dual_stack_fc_contract_missing";
pub const RULE_DUAL_STACK_FC_CONTRACT_HIDDEN_CAVEAT: &str =
    "artifact.dual_stack_fc_contract_hidden_caveat";
pub const RULE_DUAL_STACK_FC_CONTRACT_CLAIM_UNSAFE: &str =
    "artifact.dual_stack_fc_contract_claim_unsafe";
pub const RULE_PARTITION_REPORT_INVALID: &str = "artifact.partition_report_invalid";
pub const RULE_RECONCILIATION_REPORT_INVALID: &str = "artifact.reconciliation_report_invalid";
pub const RULE_PARSE_FAILED: &str = "artifact.parse_failed";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactValidationMode {
    SupervisorRun,
    DryRun,
    DualStackEvidence,
    Historical,
    BenchmarkPack,
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
    pub dual_stack_evidence: Option<PathBuf>,
    pub urban_analysis_manifest: Option<PathBuf>,
    pub partition_supervisor_reports: Option<PathBuf>,
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
            dual_stack_evidence: optional_path(&output_dir, SITL_DUAL_STACK_EVIDENCE_FILE),
            urban_analysis_manifest: optional_path(&output_dir, "urban_analysis/manifest.json"),
            partition_supervisor_reports: optional_path(
                &output_dir,
                "partition_supervisor_reports.json",
            ),
            output_dir,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PartitionSupervisorArtifact {
    #[serde(default)]
    partition_reports: Vec<PartitionReport>,
    #[serde(default)]
    reconciliation_reports: Vec<ReconciliationReport>,
    #[serde(default)]
    degraded_decision_log: Vec<DegradedDecisionLog>,
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

fn scaled_e7(value: f64) -> i32 {
    (value * 10_000_000.0).round() as i32
}

fn swarm_command_plane_rule_for_error(error: &SwarmCommandPlaneError) -> &'static str {
    match error {
        SwarmCommandPlaneError::DuplicateOwnership { .. } => RULE_SWARM_DUPLICATE_OWNERSHIP,
        SwarmCommandPlaneError::MissingHandoffEvidence { .. } => RULE_SWARM_HANDOFF_MISSING,
        SwarmCommandPlaneError::DuplicateAgentPlan { .. }
        | SwarmCommandPlaneError::MissingActiveOwnership { .. }
        | SwarmCommandPlaneError::SourceAgentMismatch { .. }
        | SwarmCommandPlaneError::MissingReplacementAgent
        | SwarmCommandPlaneError::MissingFailedAgent { .. }
        | SwarmCommandPlaneError::MissingAbortTargets => RULE_SWARM_AGENT_PLAN_MISSING,
        SwarmCommandPlaneError::MissingTopologyNode { .. }
        | SwarmCommandPlaneError::DuplicateTopologyNode { .. }
        | SwarmCommandPlaneError::UnknownTopologyLinkEndpoint { .. }
        | SwarmCommandPlaneError::UnknownCommandRouteNode { .. } => RULE_SWARM_TOPOLOGY_MISSING,
        SwarmCommandPlaneError::MissingCommandRoute { .. }
        | SwarmCommandPlaneError::CommandRoutePathMismatch { .. } => {
            RULE_SWARM_TOPOLOGY_ROUTE_MISSING
        }
        SwarmCommandPlaneError::BlockedRouteWithoutReason { .. } => {
            RULE_SWARM_TOPOLOGY_BLOCKED_UNREPORTED
        }
        SwarmCommandPlaneError::MothershipDependencyCycle { .. }
        | SwarmCommandPlaneError::UnknownMothershipDependencyAgent { .. }
        | SwarmCommandPlaneError::MothershipRouteBypassesParent { .. } => {
            RULE_SWARM_MOTHERSHIP_DEPENDENCY_INVALID
        }
        SwarmCommandPlaneError::MissingTransportAssumption => {
            RULE_SWARM_TRANSPORT_ASSUMPTION_MISSING
        }
        SwarmCommandPlaneError::UnsupportedSchema { .. } => RULE_SWARM_COMMAND_PLANE_MISSING,
    }
}

fn dual_stack_rule_for_key(key: &str) -> &'static str {
    match key {
        "schema_version" => RULE_DUAL_STACK_EVIDENCE_MISSING,
        "abort_replacement" => RULE_DUAL_STACK_ABORT_REPLACEMENT_MISSING,
        "profile_set" | "profile_mismatch" => RULE_DUAL_STACK_PROFILE_MISMATCH,
        "command_ir_hash" => RULE_DUAL_STACK_IR_HASH_MISMATCH,
        "abort_policy" => RULE_DUAL_STACK_ABORT_POLICY_MISMATCH,
        "replacement_policy" => RULE_DUAL_STACK_REPLACEMENT_POLICY_MISMATCH,
        "fc_contract" => RULE_DUAL_STACK_FC_CONTRACT_MISSING,
        "fc_contract_caveat" => RULE_DUAL_STACK_FC_CONTRACT_HIDDEN_CAVEAT,
        _ => RULE_DUAL_STACK_PROFILE_MISMATCH,
    }
}

struct Validator<'a> {
    paths: &'a ArtifactPackPaths,
    options: ArtifactValidationOptions,
    violations: Vec<ArtifactValidationViolation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MavlinkCompatibilityExpected<'a> {
    command_id: Option<&'a str>,
    seq: Option<u16>,
    command: MavlinkCommonCommandName,
    phase: MavlinkPlanPhase,
    frame: Option<&'a str>,
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
        if matches!(self.options.mode, ArtifactValidationMode::DualStackEvidence) {
            self.validate_dual_stack_evidence();
            return;
        }
        if matches!(self.options.mode, ArtifactValidationMode::BenchmarkPack) {
            self.validate_urban_analysis_ownership();
            self.validate_partition_supervisor_reports();
            return;
        }

        let Some(manifest) = self.load_manifest() else {
            return;
        };
        self.validate_manifest_metadata(&manifest);
        self.validate_swarm_command_plane_manifest(&manifest);

        let event_log = self.load_event_log();
        let run_report = self.load_run_report();
        let replay_summary = self.load_replay_summary();
        let _safety_report = self.load_safety_report();

        if let Some(log) = &event_log {
            self.validate_replacement_completion_seq(log);
            self.validate_swarm_topology_event_evidence(&manifest, log);
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

        self.validate_urban_analysis_ownership();
        self.validate_partition_supervisor_reports();
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
        if !artifact.safety_report.passed {
            self.push_error(
                RULE_DRY_RUN_SAFETY_REPORT_FAILED,
                Some(path.clone()),
                "dry-run artifact safety_report.passed must be true for current strict validation",
            );
        }
        if let Some(summary) = artifact.command_ir_summary.as_ref() {
            if summary.timeout_policy.command_timeout_secs <= 0.0
                || summary.timeout_policy.completion_timeout_secs <= 0.0
            {
                self.push_error(
                    RULE_DRY_RUN_POLICY_MISSING,
                    Some(path.clone()),
                    "command_ir_summary timeout_policy must contain positive timeouts",
                );
            }
        } else if self.options.strict
            && !matches!(self.options.mode, ArtifactValidationMode::Historical)
            && !self.options.allow_historical
        {
            self.push_error(
                RULE_DRY_RUN_POLICY_MISSING,
                Some(path.clone()),
                "current dry-run artifact must include command_ir_summary policy fields",
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
        self.validate_urban_dry_run_metadata(&artifact, plan, &path);
        self.validate_mavlink_common_plan(plan, &self.paths.dry_run_artifact);
    }

    fn validate_dual_stack_evidence(&mut self) {
        let Some(path) = self.paths.dual_stack_evidence.clone() else {
            self.push_error(
                RULE_DUAL_STACK_EVIDENCE_MISSING,
                None,
                format!("dual-stack validation requires {SITL_DUAL_STACK_EVIDENCE_FILE}"),
            );
            return;
        };
        let Some(pack) = self.load_json::<SitlDualStackEvidencePack>(&path) else {
            return;
        };
        let mut artifacts = Vec::new();
        for profile in &pack.profiles {
            let artifact_path = self.paths.output_dir.join(&profile.dry_run_artifact_path);
            let Some(artifact) = self.load_json::<SitlDryRunArtifact>(&artifact_path) else {
                self.push_error(
                    RULE_DUAL_STACK_PROFILE_MISSING,
                    Some(artifact_path),
                    format!(
                        "profile '{}' references a missing or invalid dry-run artifact",
                        profile.mavlink_profile
                    ),
                );
                continue;
            };
            for key in validate_dual_stack_profile_evidence(profile, &artifact) {
                let rule_id = dual_stack_rule_for_key(&key);
                self.push_error(
                    rule_id,
                    Some(artifact_path.clone()),
                    format!(
                        "profile '{}' does not match referenced dry-run artifact: {key}",
                        profile.mavlink_profile
                    ),
                );
            }
            artifacts.push(artifact);

            let dry_run_dir = artifact_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| self.paths.output_dir.clone());
            let report = validate_artifact_pack(
                &ArtifactPackPaths::from_output_dir(dry_run_dir),
                ArtifactValidationOptions {
                    mode: ArtifactValidationMode::DryRun,
                    allow_historical: self.options.allow_historical,
                    strict: self.options.strict,
                },
            );
            for violation in report.violations {
                self.violations.push(violation);
            }
        }

        for key in validate_dual_stack_evidence_pack(&pack, &artifacts) {
            let rule_id = dual_stack_rule_for_key(&key);
            self.push_error(
                rule_id,
                Some(path.clone()),
                format!("dual-stack evidence validation failed: {key}"),
            );
        }

        if !pack
            .profiles
            .iter()
            .any(|profile| profile.mavlink_profile == MavlinkCapabilityProfileId::Px4)
        {
            self.push_error(
                RULE_DUAL_STACK_PROFILE_MISSING,
                Some(path.clone()),
                "dual-stack evidence must include PX4 profile evidence",
            );
        }
        if !pack
            .profiles
            .iter()
            .any(|profile| profile.mavlink_profile == MavlinkCapabilityProfileId::ArduPilot)
        {
            self.push_error(
                RULE_DUAL_STACK_PROFILE_MISSING,
                Some(path.clone()),
                "dual-stack evidence must include ArduPilot profile evidence",
            );
        }
        for profile in &pack.profiles {
            if profile.hardware_facing_allowed
                && profile.mavlink_profile == MavlinkCapabilityProfileId::ArduPilot
            {
                self.push_error(
                    RULE_DUAL_STACK_HARDWARE_CLAIM_UNSAFE,
                    Some(path.clone()),
                    "ArduPilot dry-run evidence must not claim hardware-facing readiness",
                );
            }
            let Some(profile_abort) = profile.abort_replacement.as_ref() else {
                self.push_error(
                    RULE_DUAL_STACK_ABORT_REPLACEMENT_MISSING,
                    Some(path.clone()),
                    format!(
                        "profile '{}' is missing abort/replacement evidence",
                        profile.mavlink_profile
                    ),
                );
                continue;
            };
            let pack_replacement_policy = pack
                .abort_replacement
                .as_ref()
                .map(|evidence| &evidence.replacement_policy);
            if profile_abort.replacement_policy
                != ReplacementEvidenceStatus::NotApplicableSingleAgentPrimitive
                && pack_replacement_policy
                    == Some(&ReplacementEvidenceStatus::NotApplicableSingleAgentPrimitive)
            {
                self.push_error(
                    RULE_DUAL_STACK_REPLACEMENT_POLICY_MISMATCH,
                    Some(path.clone()),
                    format!(
                        "profile '{}' replacement policy does not match pack-level primitive boundary",
                        profile.mavlink_profile
                    ),
                );
            }
            let Some(fc_contract) = profile.fc_safety_contract.as_ref() else {
                self.push_error(
                    RULE_DUAL_STACK_FC_CONTRACT_MISSING,
                    Some(path.clone()),
                    format!(
                        "profile '{}' is missing FC safety contract evidence",
                        profile.mavlink_profile
                    ),
                );
                continue;
            };
            if !fc_contract.safety_report_passed {
                self.push_error(
                    RULE_DUAL_STACK_FC_CONTRACT_CLAIM_UNSAFE,
                    Some(path.clone()),
                    format!(
                        "profile '{}' has failing safety report evidence",
                        profile.mavlink_profile
                    ),
                );
            }
            if !fc_contract.unsupported_or_unknown_claims.is_empty()
                && fc_contract.caveats.is_empty()
            {
                self.push_error(
                    RULE_DUAL_STACK_FC_CONTRACT_HIDDEN_CAVEAT,
                    Some(path.clone()),
                    format!(
                        "profile '{}' hides unsupported/unknown FC safety caveats",
                        profile.mavlink_profile
                    ),
                );
            }
        }
    }

    fn validate_urban_dry_run_metadata(
        &mut self,
        artifact: &SitlDryRunArtifact,
        plan: &MavlinkCommonPlan,
        path: &Path,
    ) {
        let is_urban =
            artifact.export_kind == "urban_route" || artifact.mission.starts_with("urban-");
        if !is_urban {
            return;
        }
        if artifact.coordinate_mode.trim().is_empty() {
            self.push_error(
                RULE_URBAN_COORDINATE_MODE_MISSING,
                Some(path.to_path_buf()),
                "urban dry-run artifacts must include coordinate_mode",
            );
        }
        if artifact.coordinate_mode == "wgs84_node_geo" {
            if artifact.waypoints.is_empty() {
                self.push_error(
                    RULE_URBAN_GEO_ROUTE_METADATA_MISSING,
                    Some(path.to_path_buf()),
                    "wgs84_node_geo urban artifacts must include full waypoint metadata",
                );
            }
            let has_geo_start = artifact
                .start_waypoint
                .as_ref()
                .and_then(|waypoint| waypoint.geo)
                .is_some();
            let has_geo_end = artifact
                .end_waypoint
                .as_ref()
                .and_then(|waypoint| waypoint.geo)
                .is_some();
            if !has_geo_start || !has_geo_end {
                self.push_error(
                    RULE_URBAN_WGS84_GEO_MISSING,
                    Some(path.to_path_buf()),
                    "wgs84_node_geo urban artifacts must include geo on start and end waypoints",
                );
            }
            self.validate_urban_wgs84_mission_items(artifact, plan, path);
        }
        if artifact.mission == "urban-search" && artifact.urban_mock_perception.is_none() {
            self.push_error(
                RULE_URBAN_MOCK_PERCEPTION_MISSING,
                Some(path.to_path_buf()),
                "urban-search dry-run artifacts must include urban_mock_perception metadata",
            );
        }
    }

    fn validate_urban_wgs84_mission_items(
        &mut self,
        artifact: &SitlDryRunArtifact,
        plan: &MavlinkCommonPlan,
        path: &Path,
    ) {
        let geo_waypoints: Vec<_> = artifact
            .waypoints
            .iter()
            .filter_map(|waypoint| waypoint.geo)
            .collect();
        if geo_waypoints.len() != artifact.waypoints.len() {
            self.push_error(
                RULE_URBAN_WGS84_GEO_MISSING,
                Some(path.to_path_buf()),
                "all wgs84_node_geo urban waypoints must include geo metadata",
            );
            return;
        }
        if plan.mission_items.len() != geo_waypoints.len() {
            self.push_error(
                RULE_URBAN_GEO_ROUTE_METADATA_MISSING,
                Some(path.to_path_buf()),
                format!(
                    "mavlink mission item count {} does not match urban geo waypoint count {}",
                    plan.mission_items.len(),
                    geo_waypoints.len()
                ),
            );
            return;
        }
        for (index, (item, geo)) in plan.mission_items.iter().zip(geo_waypoints).enumerate() {
            let expected_lat_e7 = scaled_e7(geo.lat_deg);
            let expected_lon_e7 = scaled_e7(geo.lon_deg);
            let expected_alt_m = geo.alt_m as f32;
            if item.lat_e7 != expected_lat_e7
                || item.lon_e7 != expected_lon_e7
                || (item.relative_alt_m - expected_alt_m).abs() > 0.001
            {
                self.push_error(
                    RULE_URBAN_GEO_ROUTE_METADATA_MISSING,
                    Some(path.to_path_buf()),
                    format!(
                        "mavlink mission_items[{index}] does not match urban waypoint geo: expected lat_e7={expected_lat_e7} lon_e7={expected_lon_e7} relative_alt_m={expected_alt_m:.3}, got lat_e7={} lon_e7={} relative_alt_m={:.3}",
                        item.lat_e7,
                        item.lon_e7,
                        item.relative_alt_m
                    ),
                );
            }
        }
    }

    fn validate_partition_supervisor_reports(&mut self) {
        let Some(path) = self.paths.partition_supervisor_reports.clone() else {
            return;
        };
        let Some(artifact) = self.load_json::<PartitionSupervisorArtifact>(&path) else {
            return;
        };

        for report in &artifact.partition_reports {
            if report.affected_agents.is_empty() {
                self.push_error(
                    RULE_PARTITION_REPORT_INVALID,
                    Some(path.clone()),
                    "partition report must list affected_agents",
                );
            }
            if report
                .heal_tick
                .is_some_and(|heal_tick| heal_tick < report.partition_tick)
            {
                self.push_error(
                    RULE_PARTITION_REPORT_INVALID,
                    Some(path.clone()),
                    format!(
                        "partition report heal_tick {:?} is earlier than partition_tick {}",
                        report.heal_tick, report.partition_tick
                    ),
                );
            }
        }

        for report in &artifact.reconciliation_reports {
            for conflict in &report.result.conflicts {
                if let ConflictResolution::OlderLeaseWins { winner } = &conflict.resolution {
                    if winner != &conflict.holder_a && winner != &conflict.holder_b {
                        self.push_error(
                            RULE_RECONCILIATION_REPORT_INVALID,
                            Some(path.clone()),
                            format!(
                                "older_lease_wins winner '{}' must match one of the conflict holders",
                                winner
                            ),
                        );
                    }
                }
            }
            if report.result.conflicts.is_empty()
                && report.result.accepted.is_empty()
                && report.result.rejected.is_empty()
            {
                self.push_error(
                    RULE_RECONCILIATION_REPORT_INVALID,
                    Some(path.clone()),
                    "reconciliation report must not be empty",
                );
            }
        }
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
        if !plan.mission_items.is_empty() && plan.telemetry_milestones.is_empty() {
            self.push_error(
                RULE_MAVLINK_PLAN_TELEMETRY_MISSING,
                path.clone(),
                "mavlink_common_plan.telemetry_milestones is required when mission_items are present",
            );
        }
        self.validate_mavlink_mission_item_sequences(plan, path);
        self.validate_mavlink_ordering(plan, path);
        self.validate_mavlink_expected_acks(plan, path);
        self.validate_mavlink_compatibility(plan, path);
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

    fn validate_mavlink_compatibility(&mut self, plan: &MavlinkCommonPlan, path: &Option<PathBuf>) {
        if plan
            .backend_profile
            .parse::<MavlinkCapabilityProfileId>()
            .is_err()
        {
            self.push_error(
                RULE_MAVLINK_PROFILE_UNKNOWN,
                path.clone(),
                format!(
                    "mavlink_common_plan.backend_profile '{}' is not a known M82 profile",
                    plan.backend_profile
                ),
            );
        }

        let Some(report) = plan.compatibility.as_ref() else {
            let severity = if self.options.allow_historical
                || matches!(self.options.mode, ArtifactValidationMode::Historical)
            {
                ArtifactValidationSeverity::Warning
            } else {
                ArtifactValidationSeverity::Error
            };
            self.push(
                RULE_MAVLINK_PROFILE_MISSING,
                severity,
                path.clone(),
                "current dry-run mavlink_common_plan must include M82 compatibility report",
            );
            return;
        };

        if plan.backend_profile != report.profile.as_str() {
            self.push_error(
                RULE_MAVLINK_PROFILE_UNKNOWN,
                path.clone(),
                format!(
                    "backend_profile '{}' does not match compatibility.profile '{}'",
                    plan.backend_profile,
                    report.profile.as_str()
                ),
            );
        }

        let expected_results = expected_mavlink_compatibility_results(plan);
        if report.command_results.len() != expected_results.len() {
            self.push_error(
                RULE_MAVLINK_PLAN_COMMAND_MISSING,
                path.clone(),
                format!(
                    "compatibility command_results length {} does not match compiled command/item count {expected_results}",
                    report.command_results.len(),
                    expected_results = expected_results.len()
                ),
            );
        }
        for (index, expected) in expected_results.iter().enumerate() {
            let Some(actual) = report.command_results.get(index) else {
                break;
            };
            if !mavlink_compatibility_result_matches(actual, expected) {
                self.push_error(
                    RULE_MAVLINK_PROFILE_RESULT_MISMATCH,
                    path.clone(),
                    format!(
                        "compatibility command_results[{index}] does not match compiled plan element: expected command_id={:?} seq={:?} command={} phase={:?} frame={:?}, got command_id={:?} seq={:?} command={} phase={:?} frame={:?}",
                        expected.command_id,
                        expected.seq,
                        expected.command.as_str(),
                        expected.phase,
                        expected.frame,
                        actual.command_id.as_deref(),
                        actual.seq,
                        actual.command.as_str(),
                        actual.phase,
                        actual.frame.as_deref()
                    ),
                );
            }
        }

        if report
            .command_results
            .iter()
            .any(|result| result.classification == MavlinkCompatibilityClass::Unsupported)
        {
            self.push_error(
                RULE_MAVLINK_PROFILE_UNSUPPORTED,
                path.clone(),
                "compatibility report contains unsupported command or frame",
            );
        }

        let hardware_blocking = report
            .command_results
            .iter()
            .any(|result| result.classification.blocks_hardware_facing_success());
        if hardware_blocking && report.hardware_facing_allowed {
            self.push_error(
                RULE_MAVLINK_PROFILE_HARDWARE_BLOCKING,
                path.clone(),
                "hardware_facing_allowed must be false when unsupported/unknown profile behavior remains",
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

    fn validate_swarm_command_plane_manifest(&mut self, manifest: &MultiAgentSitlManifest) {
        let Some(summary) = manifest.command_plane.as_ref() else {
            if self.options.strict && !self.options.allow_historical {
                self.push_error(
                    RULE_SWARM_COMMAND_PLANE_MISSING,
                    Some(self.paths.manifest.clone()),
                    "strict current supervisor artifacts must include M87 command_plane summary; historical artifacts are allowed without it",
                );
            }
            return;
        };
        if summary.schema_version != SWARM_COMMAND_PLANE_SCHEMA_VERSION {
            self.push_error(
                RULE_SWARM_COMMAND_PLANE_MISSING,
                Some(self.paths.manifest.clone()),
                format!(
                    "unsupported command_plane schema_version '{}' (expected {SWARM_COMMAND_PLANE_SCHEMA_VERSION})",
                    summary.schema_version
                ),
            );
        }
        let Some(artifact) = manifest.command_plane_artifact.as_ref() else {
            if self.options.strict && !self.options.allow_historical {
                self.push_error(
                    RULE_SWARM_COMMAND_PLANE_MISSING,
                    Some(self.paths.manifest.clone()),
                    "strict current supervisor artifacts must include full M87 command_plane_artifact, not only command_plane summary",
                );
            }
            return;
        };
        if artifact.summary != *summary {
            self.push_error(
                RULE_SWARM_AGENT_PLAN_MISSING,
                Some(self.paths.manifest.clone()),
                "command_plane summary does not match command_plane_artifact.summary",
            );
        }
        if let Err(error) = validate_swarm_command_plan(artifact) {
            self.push_error(
                swarm_command_plane_rule_for_error(&error),
                Some(self.paths.manifest.clone()),
                format!("command_plane_artifact validation failed: {error}"),
            );
        }
        self.validate_swarm_topology_summary(summary, artifact, manifest);
        for agent in &artifact.agents {
            if agent.expected_acks != agent.mavlink_plan.expected_acks {
                self.push_error(
                    RULE_SWARM_ACK_MISMATCH,
                    Some(self.paths.manifest.clone()),
                    format!(
                        "command_plane_artifact agent '{}' expected_acks do not match mavlink_plan.expected_acks",
                        agent.agent_id
                    ),
                );
            }
        }
        for result in &artifact.sync_results {
            let has_matching_window = artifact
                .sync_operations
                .iter()
                .any(|window| window.kind == result.kind);
            let has_partial = !result.failed.is_empty() || !result.timed_out.is_empty();
            if has_partial && !has_matching_window {
                self.push_error(
                    RULE_SWARM_SYNC_PARTIAL_UNREPORTED,
                    Some(self.paths.manifest.clone()),
                    "partial synchronized command result has no matching sync operation window",
                );
            }
        }
        if summary.agent_plan_count == 0 {
            self.push_error(
                RULE_SWARM_AGENT_PLAN_MISSING,
                Some(self.paths.manifest.clone()),
                "command_plane.agent_plan_count must be positive when command_plane is present",
            );
        }
        if summary.agent_plan_count != manifest.agents_count {
            self.push_error(
                RULE_SWARM_AGENT_PLAN_MISSING,
                Some(self.paths.manifest.clone()),
                format!(
                    "command_plane.agent_plan_count {} does not match manifest agents_count {}",
                    summary.agent_plan_count, manifest.agents_count
                ),
            );
        }
    }

    fn validate_swarm_topology_summary(
        &mut self,
        summary: &SwarmCommandArtifactSummary,
        artifact: &SwarmCommandPlan,
        manifest: &MultiAgentSitlManifest,
    ) {
        let Some(topology) = artifact.topology.as_ref() else {
            if self.options.strict && !self.options.allow_historical {
                self.push_error(
                    RULE_SWARM_TOPOLOGY_MISSING,
                    Some(self.paths.manifest.clone()),
                    "strict current supervisor command-plane artifact must include M88 topology",
                );
            }
            return;
        };
        if topology.transport.hardware_boundary.trim().is_empty() {
            self.push_error(
                RULE_SWARM_TRANSPORT_ASSUMPTION_MISSING,
                Some(self.paths.manifest.clone()),
                "M88 topology transport assumptions must include an explicit hardware boundary",
            );
        }
        if self.options.strict
            && !self.options.allow_historical
            && !matches!(self.options.mode, ArtifactValidationMode::Historical)
            && (topology.transport.max_delay_ms.is_none() || topology.transport.drop_rate.is_none())
        {
            self.push_error(
                RULE_SWARM_TRANSPORT_ASSUMPTION_MISSING,
                Some(self.paths.manifest.clone()),
                "strict current M88 topology transport assumptions must include max_delay_ms and drop_rate policy values",
            );
        }
        if summary.topology_kind.as_ref() != Some(&topology.kind)
            || summary.topology_node_count != topology.nodes.len()
            || summary.topology_link_count != topology.links.len()
            || summary.command_route_count != artifact.command_routes.len()
            || summary.degraded_route_count
                != artifact
                    .command_routes
                    .iter()
                    .filter(|route| route.degraded)
                    .count()
        {
            self.push_error(
                RULE_SWARM_TOPOLOGY_MISSING,
                Some(self.paths.manifest.clone()),
                "command_plane topology summary does not match command_plane_artifact topology",
            );
        }
        for agent in &manifest.agents {
            let has_route = artifact
                .command_routes
                .iter()
                .any(|route| route.to_agent_id.to_string() == agent.agent_id);
            if !has_route {
                self.push_error(
                    RULE_SWARM_TOPOLOGY_ROUTE_MISSING,
                    Some(self.paths.manifest.clone()),
                    format!(
                        "M88 topology has no route decision for agent '{}'",
                        agent.agent_id
                    ),
                );
            }
        }
        for route in &artifact.command_routes {
            if !route.allowed && route.reason.trim().is_empty() {
                self.push_error(
                    RULE_SWARM_TOPOLOGY_BLOCKED_UNREPORTED,
                    Some(self.paths.manifest.clone()),
                    format!("blocked M88 route '{}' has no reason", route.route_id),
                );
            }
        }
    }

    fn validate_swarm_topology_event_evidence(
        &mut self,
        manifest: &MultiAgentSitlManifest,
        event_log: &SitlEventLog,
    ) {
        if !self.options.strict
            || self.options.allow_historical
            || matches!(self.options.mode, ArtifactValidationMode::Historical)
        {
            return;
        }
        let Some(artifact) = manifest.command_plane_artifact.as_ref() else {
            return;
        };
        for route in artifact
            .command_routes
            .iter()
            .filter(|route| !route.allowed)
        {
            let has_matching_event = event_log.events.iter().any(|event| {
                matches!(
                    event,
                    SitlEvent::SwarmCommandRouteBlocked {
                        route_id,
                        from_node_id,
                        to_agent_id,
                        reason,
                        ..
                    } if route_id == &route.route_id
                        && from_node_id == &route.from_node_id
                        && to_agent_id == &route.to_agent_id.to_string()
                        && reason == &route.reason
                )
            });
            if !has_matching_event {
                self.push_error(
                    RULE_SWARM_TOPOLOGY_BLOCKED_UNREPORTED,
                    self.paths.event_log.clone(),
                    format!(
                        "blocked M88 route '{}' has no matching SwarmCommandRouteBlocked event",
                        route.route_id
                    ),
                );
            }
        }
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

    fn validate_urban_analysis_ownership(&mut self) {
        let Some(manifest_path) = self.paths.urban_analysis_manifest.clone() else {
            return;
        };
        let Some(manifest) = self.load_json::<UrbanAnalysisValidationManifest>(&manifest_path)
        else {
            return;
        };

        for artifact in manifest.artifacts {
            let Some(relative_path) = artifact.segment_ownership_json else {
                continue;
            };
            let path = if relative_path.is_absolute() {
                relative_path
            } else {
                self.paths.output_dir.join(relative_path)
            };
            let Some(report) = self.load_json::<UrbanSegmentOwnershipValidationReport>(&path)
            else {
                continue;
            };
            self.validate_urban_segment_ownership_report(&path, &report);
        }
    }

    fn validate_urban_segment_ownership_report(
        &mut self,
        path: &Path,
        report: &UrbanSegmentOwnershipValidationReport,
    ) {
        let mut intervals = Vec::<UrbanSegmentOwnershipInterval>::new();
        for record in &report.records {
            let start = record.acquired_tick;
            let end = record
                .released_tick
                .or_else(|| {
                    record
                        .held_ticks
                        .map(|held_ticks| record.acquired_tick.saturating_add(held_ticks))
                })
                .unwrap_or(u64::MAX);
            for interval in &intervals {
                if interval.edge_id == record.edge_id
                    && intervals_overlap(start, end, interval.start_tick, interval.end_tick)
                {
                    self.push_error(
                        RULE_URBAN_DECONFLICTION_DUPLICATE_SEGMENT_OWNER,
                        Some(path.to_path_buf()),
                        format!(
                            "edge '{}' has overlapping owners '{}' and '{}' across ticks {}..{} and {}..{}",
                            record.edge_id,
                            interval.agent_id,
                            record.agent_id,
                            interval.start_tick,
                            display_interval_end(interval.end_tick),
                            start,
                            display_interval_end(end)
                        ),
                    );
                }
            }
            intervals.push(UrbanSegmentOwnershipInterval {
                edge_id: record.edge_id.clone(),
                agent_id: record.agent_id.clone(),
                start_tick: start,
                end_tick: end,
            });
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

fn expected_mavlink_compatibility_results(
    plan: &MavlinkCommonPlan,
) -> Vec<MavlinkCompatibilityExpected<'_>> {
    let expected_len = plan.command_prelude.len()
        + plan.mission_items.len()
        + usize::from(plan.mission_start.is_some())
        + plan.command_postlude.len();
    let mut expected_results = Vec::with_capacity(expected_len);
    expected_results.extend(plan.command_prelude.iter().map(expected_mavlink_command));
    expected_results.extend(
        plan.mission_items
            .iter()
            .map(|item| MavlinkCompatibilityExpected {
                command_id: Some(item.command_id.as_str()),
                seq: Some(item.seq),
                command: item.command,
                phase: MavlinkPlanPhase::MissionUpload,
                frame: Some(item.frame.as_str()),
            }),
    );
    if let Some(command) = &plan.mission_start {
        expected_results.push(expected_mavlink_command(command));
    }
    expected_results.extend(plan.command_postlude.iter().map(expected_mavlink_command));
    expected_results
}

fn expected_mavlink_command(command: &MavlinkCommonCommand) -> MavlinkCompatibilityExpected<'_> {
    MavlinkCompatibilityExpected {
        command_id: Some(command.command_id.as_str()),
        seq: None,
        command: command.command,
        phase: command.phase,
        frame: None,
    }
}

fn mavlink_compatibility_result_matches(
    actual: &MavlinkCommandCompatibility,
    expected: &MavlinkCompatibilityExpected<'_>,
) -> bool {
    actual.command_id.as_deref() == expected.command_id
        && actual.seq == expected.seq
        && actual.command == expected.command
        && actual.phase == expected.phase
        && actual.frame.as_deref() == expected.frame
}

#[derive(Debug, Deserialize)]
struct UrbanAnalysisValidationManifest {
    artifacts: Vec<UrbanAnalysisValidationManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct UrbanAnalysisValidationManifestEntry {
    #[serde(default)]
    segment_ownership_json: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct UrbanSegmentOwnershipValidationReport {
    records: Vec<UrbanSegmentOwnershipValidationRecord>,
}

#[derive(Debug, Deserialize)]
struct UrbanSegmentOwnershipValidationRecord {
    edge_id: String,
    agent_id: String,
    acquired_tick: u64,
    #[serde(default)]
    released_tick: Option<u64>,
    #[serde(default)]
    held_ticks: Option<u64>,
}

#[derive(Debug)]
struct UrbanSegmentOwnershipInterval {
    edge_id: String,
    agent_id: String,
    start_tick: u64,
    end_tick: u64,
}

fn intervals_overlap(left_start: u64, left_end: u64, right_start: u64, right_end: u64) -> bool {
    left_start < right_end && right_start < left_end
}

fn display_interval_end(tick: u64) -> String {
    if tick == u64::MAX {
        "open".to_owned()
    } else {
        tick.to_string()
    }
}
