use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use swarm_comms::{
    FcContractValidationResult, MavlinkCapabilityProfileId, MavlinkCommonCommand,
    MavlinkCommonCommandName, MavlinkCommonPlan, MavlinkCompatibilityClass,
    MavlinkExecutionStepResult, MavlinkPlanExecutionReport, MavlinkPlanExecutor, MavlinkPlanPhase,
    MavlinkUnsupportedFeature, MissionExecuteLifecycleState, MockAckProvider, ScriptedAckProvider,
};
use swarm_mission_ir::{TerminalState, TimeoutAction, TimeoutPolicy};

use crate::sitl_plan::{
    dry_run_artifact_with_mavlink_profile, load_sitl_plan, SitlDryRunArtifact, SitlError,
};

pub const SITL_DUAL_STACK_EVIDENCE_SCHEMA_VERSION: &str = "sitl_dual_stack_evidence_pack.v1";
pub const SITL_DUAL_STACK_EVIDENCE_FILE: &str = "sitl_dual_stack_evidence_pack.v1.json";
pub const DUAL_STACK_EXECUTION_EVIDENCE_SCHEMA_VERSION: &str = "dual_stack_execution_evidence.v1";
pub const DUAL_STACK_EXECUTION_EVIDENCE_FILE: &str = "dual_stack_execution_evidence.v1.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplacementEvidenceStatus {
    NotApplicableSingleAgentPrimitive,
    CommandPlaneReplacementSupported,
    ManualEvidenceRequired,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SitlDualStackEvidencePack {
    pub schema_version: String,
    pub source_scenario_path: PathBuf,
    pub mission: String,
    pub profile: String,
    pub agent_id: String,
    pub command_ir_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abort_replacement: Option<DualStackAbortReplacementEvidence>,
    pub profiles: Vec<SitlDualStackProfileEvidence>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SitlDualStackProfileEvidence {
    pub mavlink_profile: MavlinkCapabilityProfileId,
    pub stack_name: String,
    pub dry_run_artifact_path: PathBuf,
    pub backend_profile: String,
    pub overall_classification: MavlinkCompatibilityClass,
    pub hardware_facing_allowed: bool,
    pub expected_ack_count: usize,
    pub telemetry_milestone_count: usize,
    pub command_prelude_count: usize,
    pub mission_item_count: usize,
    pub command_postlude_count: usize,
    pub safety_passed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abort_replacement: Option<ProfileAbortReplacementEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fc_safety_contract: Option<ProfileFcSafetyContractEvidence>,
    pub caveats: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DualStackAbortReplacementEvidence {
    pub timeout_policy: TimeoutPolicy,
    pub expected_terminal_state: TerminalState,
    pub replacement_policy: ReplacementEvidenceStatus,
    pub evidence_status: String,
    pub caveats: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileAbortReplacementEvidence {
    pub timeout_on_timeout: TimeoutAction,
    pub expected_terminal_state: TerminalState,
    pub abort_command: Option<MavlinkCommonCommandName>,
    pub rtl_available: MavlinkCompatibilityClass,
    pub replacement_policy: ReplacementEvidenceStatus,
    pub caveats: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileFcSafetyContractEvidence {
    pub safety_report_passed: bool,
    pub fence_summary_present: bool,
    pub fc_contract_result_present: bool,
    pub fc_contract_passed: Option<bool>,
    pub geofence_support: MavlinkCompatibilityClass,
    pub parameter_support: MavlinkCompatibilityClass,
    pub unsupported_or_unknown_claims: Vec<String>,
    pub caveats: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DualStackExecutionEvidence {
    pub schema_version: String,
    pub mission_id: String,
    pub command_ir_hash: String,
    pub created_at: DateTime<Utc>,
    pub git_commit: String,
    pub px4: StackExecutionRecord,
    pub ardupilot: StackExecutionRecord,
    pub comparison: StackComparisonSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StackExecutionRecord {
    pub profile_id: String,
    pub lifecycle_state: MissionExecuteLifecycleState,
    pub execution_report: MavlinkPlanExecutionReport,
    pub fc_contract_result: FcContractValidationResult,
    pub caveats: Vec<String>,
    pub unsupported_features: Vec<MavlinkUnsupportedFeature>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackComparisonSummary {
    pub same_command_ir_hash: bool,
    pub lifecycle_states_match: bool,
    pub step_count_delta: i32,
    pub caveat_count_delta: i32,
    pub unsupported_count_delta: i32,
    pub notable_differences: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SitlDualStackEvidenceError {
    #[error(transparent)]
    Sitl(#[from] SitlError),
    #[error("dry-run artifact for profile '{profile}' is missing command_ir_summary")]
    MissingCommandIrSummary { profile: MavlinkCapabilityProfileId },
    #[error("dry-run artifact for profile '{profile}' is missing mavlink_common_plan")]
    MissingMavlinkCommonPlan { profile: MavlinkCapabilityProfileId },
    #[error("dry-run artifact for profile '{profile}' is missing compatibility report")]
    MissingCompatibilityReport { profile: MavlinkCapabilityProfileId },
    #[error("dual-stack evidence requires matching command IR hash values")]
    CommandIrHashMismatch,
    #[error("dual-stack evidence requires exactly PX4 and ArduPilot profile evidence")]
    ProfileSetMismatch,
    #[error("output path already exists {path:?}; use --force to overwrite")]
    OutputAlreadyExists { path: PathBuf },
    #[error("dual-stack evidence write failed {path:?}: {message}")]
    WriteFailed { path: PathBuf, message: String },
}

pub fn build_dual_stack_evidence_pack(
    source_scenario_path: impl Into<PathBuf>,
    px4_artifact_path: impl Into<PathBuf>,
    px4: &SitlDryRunArtifact,
    ardupilot_artifact_path: impl Into<PathBuf>,
    ardupilot: &SitlDryRunArtifact,
) -> Result<SitlDualStackEvidencePack, SitlDualStackEvidenceError> {
    let px4_profile =
        build_profile_evidence(MavlinkCapabilityProfileId::Px4, px4_artifact_path, px4)?;
    let ardupilot_profile = build_profile_evidence(
        MavlinkCapabilityProfileId::ArduPilot,
        ardupilot_artifact_path,
        ardupilot,
    )?;
    if px4_profile.mavlink_profile != MavlinkCapabilityProfileId::Px4
        || ardupilot_profile.mavlink_profile != MavlinkCapabilityProfileId::ArduPilot
    {
        return Err(SitlDualStackEvidenceError::ProfileSetMismatch);
    }

    let px4_hash = px4
        .mavlink_common_plan
        .as_ref()
        .ok_or(SitlDualStackEvidenceError::MissingMavlinkCommonPlan {
            profile: MavlinkCapabilityProfileId::Px4,
        })?
        .command_ir_hash
        .clone();
    let ardupilot_hash = ardupilot
        .mavlink_common_plan
        .as_ref()
        .ok_or(SitlDualStackEvidenceError::MissingMavlinkCommonPlan {
            profile: MavlinkCapabilityProfileId::ArduPilot,
        })?
        .command_ir_hash
        .clone();
    if px4_hash != ardupilot_hash {
        return Err(SitlDualStackEvidenceError::CommandIrHashMismatch);
    }

    let summary = px4.command_ir_summary.as_ref().ok_or(
        SitlDualStackEvidenceError::MissingCommandIrSummary {
            profile: MavlinkCapabilityProfileId::Px4,
        },
    )?;
    let abort_replacement = DualStackAbortReplacementEvidence {
        timeout_policy: summary.timeout_policy.clone(),
        expected_terminal_state: summary.expected_terminal_state,
        replacement_policy: ReplacementEvidenceStatus::NotApplicableSingleAgentPrimitive,
        evidence_status: "not_applicable_single_agent_primitive".to_owned(),
        caveats: vec![
            "Primitive single-agent dry-run evidence records timeout abort policy and terminal state, but does not exercise live replacement.".to_owned(),
            "Command-plane replacement evidence remains in M87/M59 artifacts and is not implied by this pack.".to_owned(),
        ],
    };

    Ok(SitlDualStackEvidencePack {
        schema_version: SITL_DUAL_STACK_EVIDENCE_SCHEMA_VERSION.to_owned(),
        source_scenario_path: source_scenario_path.into(),
        mission: px4.mission.clone(),
        profile: px4.profile.clone(),
        agent_id: px4.agent_id.clone(),
        command_ir_hash: px4_hash,
        abort_replacement: Some(abort_replacement),
        profiles: vec![px4_profile, ardupilot_profile],
        limitations: vec![
            "Dual-stack dry-run evidence does not prove PX4/ArduPilot command acceptance equivalence.".to_owned(),
            "ArduPilot evidence is dry-run only until a local ArduPilot SITL artifact is captured.".to_owned(),
            "No hardware readiness, certified safety, or live failover claim is made by this artifact.".to_owned(),
        ],
    })
}

pub fn write_dual_stack_evidence_pack(
    scenario_path: impl AsRef<Path>,
    agent_id: &str,
    output_dir: impl AsRef<Path>,
    force: bool,
) -> Result<SitlDualStackEvidencePack, SitlDualStackEvidenceError> {
    let scenario_path = scenario_path.as_ref();
    let output_dir = output_dir.as_ref();
    ensure_output_path(output_dir, force)?;

    let plan = load_sitl_plan(scenario_path, agent_id.to_owned())?;
    let artifacts = write_dual_stack_profile_artifacts(scenario_path, agent_id, output_dir, &plan)?;

    let pack = build_dual_stack_evidence_pack(
        scenario_path.to_path_buf(),
        artifacts.px4_relative_path,
        &artifacts.px4_artifact,
        artifacts.ardupilot_relative_path,
        &artifacts.ardupilot_artifact,
    )?;
    write_json(&output_dir.join(SITL_DUAL_STACK_EVIDENCE_FILE), &pack)?;
    Ok(pack)
}

pub fn write_dual_stack_execution_evidence(
    scenario_path: impl AsRef<Path>,
    agent_id: &str,
    output_dir: impl AsRef<Path>,
    force: bool,
) -> Result<DualStackExecutionEvidence, SitlDualStackEvidenceError> {
    let scenario_path = scenario_path.as_ref();
    let output_dir = output_dir.as_ref();
    ensure_output_path(output_dir, force)?;

    let plan = load_sitl_plan(scenario_path, agent_id.to_owned())?;
    let artifacts = write_dual_stack_profile_artifacts(scenario_path, agent_id, output_dir, &plan)?;
    let dry_pack = build_dual_stack_evidence_pack(
        scenario_path.to_path_buf(),
        artifacts.px4_relative_path,
        &artifacts.px4_artifact,
        artifacts.ardupilot_relative_path,
        &artifacts.ardupilot_artifact,
    )?;
    write_json(&output_dir.join(SITL_DUAL_STACK_EVIDENCE_FILE), &dry_pack)?;

    let px4_plan = artifacts.px4_artifact.mavlink_common_plan.as_ref().ok_or(
        SitlDualStackEvidenceError::MissingMavlinkCommonPlan {
            profile: MavlinkCapabilityProfileId::Px4,
        },
    )?;
    let ardupilot_plan = artifacts
        .ardupilot_artifact
        .mavlink_common_plan
        .as_ref()
        .ok_or(SitlDualStackEvidenceError::MissingMavlinkCommonPlan {
            profile: MavlinkCapabilityProfileId::ArduPilot,
        })?;
    let evidence =
        build_dual_stack_execution_evidence(px4_plan, ardupilot_plan, current_git_commit())?;
    write_json(
        &output_dir.join(DUAL_STACK_EXECUTION_EVIDENCE_FILE),
        &evidence,
    )?;
    Ok(evidence)
}

pub fn build_dual_stack_execution_evidence(
    px4_plan: &MavlinkCommonPlan,
    ardupilot_plan: &MavlinkCommonPlan,
    git_commit: impl Into<String>,
) -> Result<DualStackExecutionEvidence, SitlDualStackEvidenceError> {
    if px4_plan.command_ir_hash != ardupilot_plan.command_ir_hash {
        return Err(SitlDualStackEvidenceError::CommandIrHashMismatch);
    }
    let px4 = build_px4_execution_record(px4_plan);
    let ardupilot = build_ardupilot_execution_record(ardupilot_plan);
    let comparison = compare_stack_execution_records(
        &px4_plan.command_ir_hash,
        &ardupilot_plan.command_ir_hash,
        &px4,
        &ardupilot,
    );

    Ok(DualStackExecutionEvidence {
        schema_version: DUAL_STACK_EXECUTION_EVIDENCE_SCHEMA_VERSION.to_owned(),
        mission_id: px4_plan.source_mission_id.clone(),
        command_ir_hash: px4_plan.command_ir_hash.clone(),
        created_at: Utc::now(),
        git_commit: git_commit.into(),
        px4,
        ardupilot,
        comparison,
    })
}

struct DualStackProfileArtifacts {
    px4_relative_path: PathBuf,
    px4_artifact: SitlDryRunArtifact,
    ardupilot_relative_path: PathBuf,
    ardupilot_artifact: SitlDryRunArtifact,
}

fn write_dual_stack_profile_artifacts(
    scenario_path: &Path,
    agent_id: &str,
    output_dir: &Path,
    plan: &crate::sitl_plan::SitlPlan,
) -> Result<DualStackProfileArtifacts, SitlDualStackEvidenceError> {
    let px4_dir = output_dir.join("px4");
    let ardupilot_dir = output_dir.join("ardupilot");
    fs::create_dir_all(&px4_dir).map_err(|error| SitlDualStackEvidenceError::WriteFailed {
        path: px4_dir.clone(),
        message: error.to_string(),
    })?;
    fs::create_dir_all(&ardupilot_dir).map_err(|error| {
        SitlDualStackEvidenceError::WriteFailed {
            path: ardupilot_dir.clone(),
            message: error.to_string(),
        }
    })?;

    let px4_artifact_path = px4_dir.join("sitl_dry_run_artifact.v1.json");
    let ardupilot_artifact_path = ardupilot_dir.join("sitl_dry_run_artifact.v1.json");
    let px4_artifact = dry_run_artifact_with_mavlink_profile(
        plan,
        command_for_profile(scenario_path, agent_id, MavlinkCapabilityProfileId::Px4),
        MavlinkCapabilityProfileId::Px4,
    );
    let ardupilot_artifact = dry_run_artifact_with_mavlink_profile(
        plan,
        command_for_profile(
            scenario_path,
            agent_id,
            MavlinkCapabilityProfileId::ArduPilot,
        ),
        MavlinkCapabilityProfileId::ArduPilot,
    );
    write_json(&px4_artifact_path, &px4_artifact)?;
    write_json(&ardupilot_artifact_path, &ardupilot_artifact)?;

    Ok(DualStackProfileArtifacts {
        px4_relative_path: PathBuf::from("px4").join("sitl_dry_run_artifact.v1.json"),
        px4_artifact,
        ardupilot_relative_path: PathBuf::from("ardupilot").join("sitl_dry_run_artifact.v1.json"),
        ardupilot_artifact,
    })
}

fn build_px4_execution_record(plan: &MavlinkCommonPlan) -> StackExecutionRecord {
    let mut executor = MavlinkPlanExecutor::new(MockAckProvider, 0);
    let report = executor.execute(plan);
    let caveats = execution_caveats(
        plan,
        "PX4 execution evidence is local executor evidence over the MavlinkPlanExecutor API; live FC/SIH artifacts remain separate.",
    );
    StackExecutionRecord {
        profile_id: MavlinkCapabilityProfileId::Px4.as_str().to_owned(),
        lifecycle_state: report.lifecycle_state.clone(),
        execution_report: report,
        fc_contract_result: fc_contract_result_or_not_requested(plan),
        caveats,
        unsupported_features: plan.unsupported_features.clone(),
    }
}

fn build_ardupilot_execution_record(plan: &MavlinkCommonPlan) -> StackExecutionRecord {
    let script = ardupilot_execution_script(plan);
    let mut executor = MavlinkPlanExecutor::new(ScriptedAckProvider::new(script), 0);
    let mut report = executor.execute(plan);
    let unsupported_features = ardupilot_unsupported_features(plan);
    if !unsupported_features.is_empty() {
        report.lifecycle_state = MissionExecuteLifecycleState::Unsupported;
    }
    let mut caveats = execution_caveats(
        plan,
        "ArduPilot execution evidence is experimental and local-only; skipped steps mean unsupported/unevidenced profile behavior, not flight acceptance.",
    );
    if report
        .steps
        .iter()
        .any(|(_, _, result)| matches!(result, MavlinkExecutionStepResult::Skipped { .. }))
    {
        push_unique(
            &mut caveats,
            "ArduPilot incompatible or unevidenced steps are represented as skipped executor steps."
                .to_owned(),
        );
    }
    StackExecutionRecord {
        profile_id: MavlinkCapabilityProfileId::ArduPilot.as_str().to_owned(),
        lifecycle_state: report.lifecycle_state.clone(),
        execution_report: report,
        fc_contract_result: fc_contract_result_or_not_requested(plan),
        caveats,
        unsupported_features,
    }
}

fn ardupilot_execution_script(plan: &MavlinkCommonPlan) -> Vec<MavlinkExecutionStepResult> {
    let mut script = Vec::new();
    for command in &plan.command_prelude {
        script.push(ardupilot_command_result(plan, command));
    }
    if !plan.mission_items.is_empty() {
        let blocks = plan.compatibility.as_ref().is_some_and(|compatibility| {
            compatibility.command_results.iter().any(|result| {
                result.phase == MavlinkPlanPhase::MissionUpload
                    && result.classification.blocks_hardware_facing_success()
            })
        });
        script.push(if blocks {
            MavlinkExecutionStepResult::Skipped {
                reason: "ardupilot_mission_upload_unsupported_until_sitl".to_owned(),
            }
        } else {
            MavlinkExecutionStepResult::Accepted
        });
    }
    if let Some(command) = &plan.mission_start {
        script.push(ardupilot_command_result(plan, command));
    }
    for command in &plan.command_postlude {
        script.push(ardupilot_command_result(plan, command));
    }
    script
}

fn ardupilot_command_result(
    plan: &MavlinkCommonPlan,
    command: &MavlinkCommonCommand,
) -> MavlinkExecutionStepResult {
    let blocks = plan
        .compatibility
        .as_ref()
        .and_then(|compatibility| {
            compatibility.command_results.iter().find(|result| {
                result.command_id.as_deref() == Some(command.command_id.as_str())
                    && result.command == command.command
                    && result.phase == command.phase
            })
        })
        .is_some_and(|result| result.classification.blocks_hardware_facing_success());
    if blocks {
        MavlinkExecutionStepResult::Skipped {
            reason: format!(
                "ardupilot_{}_unsupported_until_sitl",
                command.command.as_str().to_ascii_lowercase()
            ),
        }
    } else {
        MavlinkExecutionStepResult::Accepted
    }
}

fn ardupilot_unsupported_features(plan: &MavlinkCommonPlan) -> Vec<MavlinkUnsupportedFeature> {
    let mut features = plan.unsupported_features.clone();
    if let Some(compatibility) = &plan.compatibility {
        for result in &compatibility.command_results {
            if result.classification.blocks_hardware_facing_success() {
                features.push(MavlinkUnsupportedFeature {
                    rule_id: format!("ardupilot_profile_{}", result.classification.as_str()),
                    command_id: result
                        .command_id
                        .clone()
                        .unwrap_or_else(|| format!("seq-{}", result.seq.unwrap_or(0))),
                    command_kind: result.command.as_str().to_owned(),
                    required: true,
                    reason: format!(
                        "ArduPilot profile classifies this execution step as {}: {}",
                        result.classification.as_str(),
                        result.reason
                    ),
                });
            }
        }
    }
    features
}

fn execution_caveats(plan: &MavlinkCommonPlan, boundary: &str) -> Vec<String> {
    let mut caveats = vec![boundary.to_owned()];
    if let Some(compatibility) = &plan.compatibility {
        for caveat in &compatibility.caveats {
            push_unique(&mut caveats, caveat.clone());
        }
    }
    for feature in &plan.unsupported_features {
        push_unique(&mut caveats, feature.reason.clone());
    }
    caveats
}

fn fc_contract_result_or_not_requested(plan: &MavlinkCommonPlan) -> FcContractValidationResult {
    plan.fc_contract_result
        .clone()
        .unwrap_or_else(|| FcContractValidationResult {
            violations: Vec::new(),
            blocks_mission_start: false,
            summary: "FC contract not requested for this primitive execution evidence".to_owned(),
        })
}

fn compare_stack_execution_records(
    px4_hash: &str,
    ardupilot_hash: &str,
    px4: &StackExecutionRecord,
    ardupilot: &StackExecutionRecord,
) -> StackComparisonSummary {
    let mut notable_differences = Vec::new();
    if px4.lifecycle_state != ardupilot.lifecycle_state {
        notable_differences.push(format!(
            "lifecycle_state differs: px4={:?}, ardupilot={:?}",
            px4.lifecycle_state, ardupilot.lifecycle_state
        ));
    }
    let ardupilot_skipped = ardupilot
        .execution_report
        .steps
        .iter()
        .filter(|(_, _, result)| matches!(result, MavlinkExecutionStepResult::Skipped { .. }))
        .count();
    if ardupilot_skipped > 0 {
        notable_differences.push(format!(
            "ardupilot skipped {ardupilot_skipped} incompatible or unevidenced executor step(s)"
        ));
    }
    if px4.unsupported_features.len() != ardupilot.unsupported_features.len() {
        notable_differences.push(format!(
            "unsupported feature count differs: px4={}, ardupilot={}",
            px4.unsupported_features.len(),
            ardupilot.unsupported_features.len()
        ));
    }

    StackComparisonSummary {
        same_command_ir_hash: px4_hash == ardupilot_hash,
        lifecycle_states_match: px4.lifecycle_state == ardupilot.lifecycle_state,
        step_count_delta: px4.execution_report.steps.len() as i32
            - ardupilot.execution_report.steps.len() as i32,
        caveat_count_delta: px4.caveats.len() as i32 - ardupilot.caveats.len() as i32,
        unsupported_count_delta: px4.unsupported_features.len() as i32
            - ardupilot.unsupported_features.len() as i32,
        notable_differences,
    }
}

fn build_profile_evidence(
    profile_id: MavlinkCapabilityProfileId,
    dry_run_artifact_path: impl Into<PathBuf>,
    artifact: &SitlDryRunArtifact,
) -> Result<SitlDualStackProfileEvidence, SitlDualStackEvidenceError> {
    let summary = artifact.command_ir_summary.as_ref().ok_or(
        SitlDualStackEvidenceError::MissingCommandIrSummary {
            profile: profile_id,
        },
    )?;
    let plan = artifact.mavlink_common_plan.as_ref().ok_or(
        SitlDualStackEvidenceError::MissingMavlinkCommonPlan {
            profile: profile_id,
        },
    )?;
    let compatibility = plan.compatibility.as_ref().ok_or(
        SitlDualStackEvidenceError::MissingCompatibilityReport {
            profile: profile_id,
        },
    )?;
    if compatibility.profile != profile_id {
        return Err(SitlDualStackEvidenceError::ProfileSetMismatch);
    }

    let profile = profile_id.profile();
    let mut caveats = compatibility.caveats.clone();
    for caveat in plan
        .fence_summary
        .iter()
        .flat_map(|summary| summary.caveats.iter())
    {
        push_unique(&mut caveats, caveat.clone());
    }

    Ok(SitlDualStackProfileEvidence {
        mavlink_profile: profile_id,
        stack_name: profile.stack_name.to_owned(),
        dry_run_artifact_path: dry_run_artifact_path.into(),
        backend_profile: plan.backend_profile.clone(),
        overall_classification: compatibility.overall_classification,
        hardware_facing_allowed: compatibility.hardware_facing_allowed,
        expected_ack_count: plan.expected_acks.len(),
        telemetry_milestone_count: plan.telemetry_milestones.len(),
        command_prelude_count: plan.command_prelude.len(),
        mission_item_count: plan.mission_items.len(),
        command_postlude_count: plan.command_postlude.len(),
        safety_passed: artifact.safety_report.passed,
        abort_replacement: Some(ProfileAbortReplacementEvidence {
            timeout_on_timeout: summary.timeout_policy.on_timeout,
            expected_terminal_state: summary.expected_terminal_state,
            abort_command: Some(MavlinkCommonCommandName::NavReturnToLaunch),
            rtl_available: compatibility
                .command_results
                .iter()
                .find(|result| result.command == MavlinkCommonCommandName::NavReturnToLaunch)
                .map(|result| result.classification)
                .unwrap_or(MavlinkCompatibilityClass::UnknownUntilSitlOrHardware),
            replacement_policy: ReplacementEvidenceStatus::NotApplicableSingleAgentPrimitive,
            caveats: vec![
                "Replacement is not applicable for this primitive single-agent dry-run pack."
                    .to_owned(),
            ],
        }),
        fc_safety_contract: Some(ProfileFcSafetyContractEvidence {
            safety_report_passed: artifact.safety_report.passed,
            fence_summary_present: plan.fence_summary.is_some(),
            fc_contract_result_present: plan.fc_contract_result.is_some(),
            fc_contract_passed: plan
                .fc_contract_result
                .as_ref()
                .map(|result| !result.blocks_mission_start),
            geofence_support: profile.geofence_support,
            parameter_support: profile.parameter_support,
            unsupported_or_unknown_claims: unsupported_or_unknown_claims(plan, profile_id),
            caveats,
        }),
        caveats: compatibility.caveats.clone(),
    })
}

fn unsupported_or_unknown_claims(
    plan: &swarm_comms::MavlinkCommonPlan,
    profile_id: MavlinkCapabilityProfileId,
) -> Vec<String> {
    let mut claims = Vec::new();
    let profile = profile_id.profile();
    for (dimension, class) in [
        ("geofence_support", profile.geofence_support),
        ("parameter_support", profile.parameter_support),
    ] {
        if class.blocks_hardware_facing_success() {
            claims.push(format!("{dimension}={}", class.as_str()));
        }
    }
    if let Some(summary) = &plan.fence_summary {
        if summary
            .profile_classification
            .blocks_hardware_facing_success()
        {
            claims.push(format!(
                "fence_summary.profile_classification={}",
                summary.profile_classification.as_str()
            ));
        }
    }
    claims
}

pub fn validate_dual_stack_evidence_pack(
    pack: &SitlDualStackEvidencePack,
    dry_run_artifacts: &[SitlDryRunArtifact],
) -> Vec<String> {
    let mut violations = Vec::new();
    if pack.schema_version != SITL_DUAL_STACK_EVIDENCE_SCHEMA_VERSION {
        violations.push("schema_version".to_owned());
    }
    let Some(abort_replacement) = pack.abort_replacement.as_ref() else {
        violations.push("abort_replacement".to_owned());
        return violations;
    };
    if abort_replacement.evidence_status.trim().is_empty() {
        violations.push("abort_replacement".to_owned());
    }
    let profile_set = pack
        .profiles
        .iter()
        .map(|profile| profile.mavlink_profile)
        .collect::<HashSet<_>>();
    let expected_profiles = HashSet::from([
        MavlinkCapabilityProfileId::Px4,
        MavlinkCapabilityProfileId::ArduPilot,
    ]);
    if pack.profiles.len() != expected_profiles.len() || profile_set != expected_profiles {
        violations.push("profile_set".to_owned());
    }
    for artifact in dry_run_artifacts {
        let Some(plan) = artifact.mavlink_common_plan.as_ref() else {
            violations.push("mavlink_common_plan".to_owned());
            continue;
        };
        if plan.command_ir_hash != pack.command_ir_hash {
            violations.push("command_ir_hash".to_owned());
        }
        if let Some(summary) = artifact.command_ir_summary.as_ref() {
            if summary.timeout_policy != abort_replacement.timeout_policy
                || summary.expected_terminal_state != abort_replacement.expected_terminal_state
            {
                violations.push("abort_policy".to_owned());
            }
        }
    }
    for profile in &pack.profiles {
        let Some(profile_abort) = profile.abort_replacement.as_ref() else {
            violations.push("abort_replacement".to_owned());
            continue;
        };
        if profile_abort.replacement_policy != abort_replacement.replacement_policy {
            violations.push("replacement_policy".to_owned());
        }
        let Some(fc_contract) = profile.fc_safety_contract.as_ref() else {
            violations.push("fc_contract".to_owned());
            continue;
        };
        if !fc_contract.unsupported_or_unknown_claims.is_empty() && fc_contract.caveats.is_empty() {
            violations.push("fc_contract_caveat".to_owned());
        }
    }
    violations.sort();
    violations.dedup();
    violations
}

pub fn validate_dual_stack_profile_evidence(
    profile: &SitlDualStackProfileEvidence,
    artifact: &SitlDryRunArtifact,
) -> Vec<String> {
    let mut violations = Vec::new();
    let Some(plan) = artifact.mavlink_common_plan.as_ref() else {
        return vec!["profile_mismatch".to_owned()];
    };
    let Some(compatibility) = plan.compatibility.as_ref() else {
        return vec!["profile_mismatch".to_owned()];
    };
    if profile.mavlink_profile != compatibility.profile
        || profile.backend_profile != plan.backend_profile
        || profile.overall_classification != compatibility.overall_classification
        || profile.hardware_facing_allowed != compatibility.hardware_facing_allowed
    {
        violations.push("profile_mismatch".to_owned());
    }
    if profile.expected_ack_count != plan.expected_acks.len()
        || profile.telemetry_milestone_count != plan.telemetry_milestones.len()
        || profile.command_prelude_count != plan.command_prelude.len()
        || profile.mission_item_count != plan.mission_items.len()
        || profile.command_postlude_count != plan.command_postlude.len()
    {
        violations.push("profile_mismatch".to_owned());
    }
    if profile.safety_passed != artifact.safety_report.passed {
        violations.push("fc_contract".to_owned());
    }

    let Some(summary) = artifact.command_ir_summary.as_ref() else {
        violations.push("abort_policy".to_owned());
        return sorted_unique(violations);
    };
    let Some(profile_abort) = profile.abort_replacement.as_ref() else {
        violations.push("abort_replacement".to_owned());
        return sorted_unique(violations);
    };
    if profile_abort.timeout_on_timeout != summary.timeout_policy.on_timeout
        || profile_abort.expected_terminal_state != summary.expected_terminal_state
    {
        violations.push("abort_policy".to_owned());
    }

    let Some(fc_contract) = profile.fc_safety_contract.as_ref() else {
        violations.push("fc_contract".to_owned());
        return sorted_unique(violations);
    };
    let capability_profile = compatibility.profile.profile();
    if fc_contract.safety_report_passed != artifact.safety_report.passed
        || fc_contract.fence_summary_present != plan.fence_summary.is_some()
        || fc_contract.fc_contract_result_present != plan.fc_contract_result.is_some()
        || fc_contract.fc_contract_passed
            != plan
                .fc_contract_result
                .as_ref()
                .map(|result| !result.blocks_mission_start)
        || fc_contract.geofence_support != capability_profile.geofence_support
        || fc_contract.parameter_support != capability_profile.parameter_support
    {
        violations.push("fc_contract".to_owned());
    }
    let expected_claims = unsupported_or_unknown_claims(plan, compatibility.profile);
    if fc_contract.unsupported_or_unknown_claims != expected_claims {
        violations.push("fc_contract".to_owned());
    }
    if !expected_claims.is_empty() && fc_contract.caveats.is_empty() {
        violations.push("fc_contract_caveat".to_owned());
    }
    for caveat in &compatibility.caveats {
        if !profile.caveats.iter().any(|actual| actual == caveat) {
            violations.push("profile_mismatch".to_owned());
        }
    }

    sorted_unique(violations)
}

pub fn validate_dual_stack_execution_evidence(
    evidence: &DualStackExecutionEvidence,
) -> Vec<String> {
    let mut violations = Vec::new();
    if evidence.schema_version != DUAL_STACK_EXECUTION_EVIDENCE_SCHEMA_VERSION {
        violations.push("schema_version".to_owned());
    }
    if evidence.mission_id.trim().is_empty() {
        violations.push("mission_id".to_owned());
    }
    if evidence.command_ir_hash.trim().is_empty() {
        violations.push("command_ir_hash".to_owned());
    }
    if evidence.git_commit.trim().is_empty() {
        violations.push("git_commit".to_owned());
    }
    validate_stack_execution_record(
        &mut violations,
        "px4",
        MavlinkCapabilityProfileId::Px4,
        &evidence.px4,
    );
    validate_stack_execution_record(
        &mut violations,
        "ardupilot",
        MavlinkCapabilityProfileId::ArduPilot,
        &evidence.ardupilot,
    );
    if !evidence.comparison.same_command_ir_hash {
        violations.push("command_ir_hash".to_owned());
    }
    let lifecycle_states_match = evidence.px4.lifecycle_state == evidence.ardupilot.lifecycle_state;
    if evidence.comparison.lifecycle_states_match != lifecycle_states_match {
        violations.push("comparison".to_owned());
    }
    let step_count_delta = evidence.px4.execution_report.steps.len() as i32
        - evidence.ardupilot.execution_report.steps.len() as i32;
    if evidence.comparison.step_count_delta != step_count_delta {
        violations.push("comparison".to_owned());
    }
    let caveat_count_delta =
        evidence.px4.caveats.len() as i32 - evidence.ardupilot.caveats.len() as i32;
    if evidence.comparison.caveat_count_delta != caveat_count_delta {
        violations.push("comparison".to_owned());
    }
    let unsupported_count_delta = evidence.px4.unsupported_features.len() as i32
        - evidence.ardupilot.unsupported_features.len() as i32;
    if evidence.comparison.unsupported_count_delta != unsupported_count_delta {
        violations.push("comparison".to_owned());
    }
    if !evidence.ardupilot.unsupported_features.is_empty()
        && evidence.comparison.notable_differences.is_empty()
    {
        violations.push("comparison".to_owned());
    }
    sorted_unique(violations)
}

fn validate_stack_execution_record(
    violations: &mut Vec<String>,
    key: &str,
    expected_profile: MavlinkCapabilityProfileId,
    record: &StackExecutionRecord,
) {
    if record.profile_id != expected_profile.as_str() {
        violations.push(format!("{key}_profile"));
    }
    if record.execution_report.plan_id.trim().is_empty() {
        violations.push(format!("{key}_execution_report"));
    }
    if record.execution_report.lifecycle_state != record.lifecycle_state {
        violations.push(format!("{key}_lifecycle"));
    }
    if !record.unsupported_features.is_empty()
        && record.lifecycle_state == MissionExecuteLifecycleState::Completed
    {
        violations.push(format!("{key}_unsupported_completed"));
    }
    if matches!(
        record.lifecycle_state,
        MissionExecuteLifecycleState::Unsupported
    ) && record.caveats.is_empty()
    {
        violations.push(format!("{key}_caveats"));
    }
    if record.caveats.is_empty() {
        violations.push(format!("{key}_caveats"));
    }
    if record.fc_contract_result.blocks_mission_start
        && record.lifecycle_state == MissionExecuteLifecycleState::Completed
    {
        violations.push(format!("{key}_fc_contract"));
    }
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn command_for_profile(
    scenario_path: &Path,
    agent_id: &str,
    profile: MavlinkCapabilityProfileId,
) -> Vec<String> {
    vec![
        "sitl_agent".to_owned(),
        "--dry-run".to_owned(),
        "--scenario".to_owned(),
        scenario_path.display().to_string(),
        "--agent-id".to_owned(),
        agent_id.to_owned(),
        "--mavlink-profile".to_owned(),
        profile.as_str().to_owned(),
    ]
}

fn ensure_output_path(output_dir: &Path, force: bool) -> Result<(), SitlDualStackEvidenceError> {
    if output_dir.exists() && !force {
        return Err(SitlDualStackEvidenceError::OutputAlreadyExists {
            path: output_dir.to_path_buf(),
        });
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), SitlDualStackEvidenceError> {
    let content = serde_json::to_string_pretty(value).map_err(|error| {
        SitlDualStackEvidenceError::WriteFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    fs::write(path, content).map_err(|error| SitlDualStackEvidenceError::WriteFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn current_git_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_comms::MavlinkExecutionOutcome;

    #[test]
    fn replacement_status_serializes_stably() {
        let value =
            serde_json::to_value(ReplacementEvidenceStatus::NotApplicableSingleAgentPrimitive)
                .unwrap();
        assert_eq!(
            value,
            serde_json::json!("not_applicable_single_agent_primitive")
        );
    }

    #[test]
    fn dual_stack_evidence_command_ir_hash_matches() {
        let evidence = execution_evidence();

        assert!(evidence.comparison.same_command_ir_hash);
        assert!(!evidence.command_ir_hash.trim().is_empty());
        assert_eq!(
            evidence.px4.execution_report.plan_id,
            evidence.ardupilot.execution_report.plan_id
        );
    }

    #[test]
    fn px4_lifecycle_completes_takeoff_hold_land() {
        let evidence = execution_evidence();

        assert_eq!(
            evidence.px4.lifecycle_state,
            MissionExecuteLifecycleState::Completed
        );
        assert!(evidence.px4.execution_report.overall.is_success());
    }

    #[test]
    fn ardupilot_lifecycle_skips_incompatible_steps() {
        let evidence = execution_evidence();

        assert_eq!(
            evidence.ardupilot.lifecycle_state,
            MissionExecuteLifecycleState::Unsupported
        );
        assert!(evidence
            .ardupilot
            .execution_report
            .steps
            .iter()
            .any(|(_, _, result)| matches!(result, MavlinkExecutionStepResult::Skipped { .. })));
        assert!(!matches!(
            evidence.ardupilot.execution_report.overall,
            MavlinkExecutionOutcome::Aborted { .. } | MavlinkExecutionOutcome::Failed { .. }
        ));
    }

    #[test]
    fn ardupilot_unsupported_feature_is_not_marked_completed() {
        let evidence = execution_evidence();

        assert!(!evidence.ardupilot.unsupported_features.is_empty());
        assert_ne!(
            evidence.ardupilot.lifecycle_state,
            MissionExecuteLifecycleState::Completed
        );
    }

    #[test]
    fn stack_comparison_summary_correct_delta_counts() {
        let evidence = execution_evidence();
        let comparison = compare_stack_execution_records(
            &evidence.command_ir_hash,
            &evidence.command_ir_hash,
            &evidence.px4,
            &evidence.ardupilot,
        );

        assert_eq!(evidence.comparison, comparison);
        assert_eq!(evidence.comparison.step_count_delta, 0);
        assert!(evidence.comparison.unsupported_count_delta < 0);
        assert!(!evidence.comparison.lifecycle_states_match);
    }

    #[test]
    fn dual_stack_evidence_serde_roundtrip() {
        let evidence = execution_evidence();

        let json = serde_json::to_string_pretty(&evidence).unwrap();
        let parsed: DualStackExecutionEvidence = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, evidence);
    }

    fn execution_evidence() -> DualStackExecutionEvidence {
        let fixture = tempfile::tempdir().unwrap();
        write_dual_stack_execution_evidence(
            public_scenario_path("scenarios/primitive.takeoff-hold-land.json"),
            "agent-0",
            fixture.path(),
            true,
        )
        .unwrap()
    }

    fn public_scenario_path(path: &str) -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }
}
