use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use swarm_comms::{
    MavlinkCapabilityProfileId, MavlinkCommonCommandName, MavlinkCompatibilityClass,
};
use swarm_mission_ir::{TerminalState, TimeoutAction, TimeoutPolicy};

use crate::sitl_plan::{
    dry_run_artifact_with_mavlink_profile, load_sitl_plan, SitlDryRunArtifact, SitlError,
};

pub const SITL_DUAL_STACK_EVIDENCE_SCHEMA_VERSION: &str = "sitl_dual_stack_evidence_pack.v1";
pub const SITL_DUAL_STACK_EVIDENCE_FILE: &str = "sitl_dual_stack_evidence_pack.v1.json";

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
        &plan,
        command_for_profile(scenario_path, agent_id, MavlinkCapabilityProfileId::Px4),
        MavlinkCapabilityProfileId::Px4,
    );
    let ardupilot_artifact = dry_run_artifact_with_mavlink_profile(
        &plan,
        command_for_profile(
            scenario_path,
            agent_id,
            MavlinkCapabilityProfileId::ArduPilot,
        ),
        MavlinkCapabilityProfileId::ArduPilot,
    );
    write_json(&px4_artifact_path, &px4_artifact)?;
    write_json(&ardupilot_artifact_path, &ardupilot_artifact)?;

    let pack = build_dual_stack_evidence_pack(
        scenario_path.to_path_buf(),
        PathBuf::from("px4").join("sitl_dry_run_artifact.v1.json"),
        &px4_artifact,
        PathBuf::from("ardupilot").join("sitl_dry_run_artifact.v1.json"),
        &ardupilot_artifact,
    )?;
    write_json(&output_dir.join(SITL_DUAL_STACK_EVIDENCE_FILE), &pack)?;
    Ok(pack)
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
