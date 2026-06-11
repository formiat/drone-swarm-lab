use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use swarm_command_plane::SwarmCommandArtifactSummary;
use swarm_comms::{
    DeconflictionMode, FcContractValidationResult, FcParamSnapshot, MavlinkCapabilityProfileId,
    MavlinkFenceArtifact, MavlinkPlanExecutionReport, MavlinkPlanExecutor, MockAckProvider,
};
use swarm_safety::preflight::SafetyValidationReport;
use swarm_sim::{UrbanOperationalEvidence, URBAN_OPERATIONAL_EVIDENCE_SCHEMA_VERSION};
use swarm_types::AgentId;

use crate::sitl_dual_stack_evidence::{
    build_dual_stack_execution_evidence, DualStackExecutionEvidence,
};
use crate::sitl_plan::{dry_run_artifact_with_mavlink_profile, SitlError, SitlPlan};

pub const HARDWARE_ENTRY_PACK_SCHEMA_VERSION: &str = "hardware_entry_pack.v1";
pub const HARDWARE_ENTRY_PACK_FILE: &str = "hardware_entry_pack.v1.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HardwareEntryPack {
    pub schema_version: String,
    pub pack_id: String,
    pub created_at: DateTime<Utc>,
    pub git_commit: String,
    pub mission_families_covered: Vec<String>,
    pub primitive_evidence: Option<MavlinkPlanExecutionReport>,
    pub urban_evidence: Option<UrbanOperationalEvidence>,
    pub swarm_evidence: Option<SwarmCommandArtifactSummary>,
    pub dual_stack_evidence: Option<DualStackExecutionEvidence>,
    pub fc_contract_result: FcContractValidationResult,
    pub param_snapshot: Option<FcParamSnapshot>,
    pub fence_plan: Option<MavlinkFenceArtifact>,
    pub swarm_protocol_assumptions: Vec<String>,
    pub topology_assumptions: Vec<String>,
    pub degraded_policy_matrix: Vec<DegradedPolicyEntry>,
    pub preflight_report: SafetyValidationReport,
    pub hardware_entry_checklist: HardwareEntryChecklist,
    pub run_command: String,
    pub readiness_status: HardwareReadinessStatus,
    pub caveats: Vec<String>,
    pub limitations: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradedPolicyEntry {
    pub condition: String,
    pub policy: String,
    pub tested: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareReadinessStatus {
    DryRunOnly,
    ExecuteValidatedLocally,
    DegradedPartiallyEvidenced,
    UnsupportedOrUnknown { detail: String },
    Blocked { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareEntryChecklist {
    pub selected_autopilot: Option<String>,
    pub selected_airframe: Option<String>,
    pub selected_link_class: Option<String>,
    pub coordinate_frame_policy: Option<String>,
    pub altitude_reference: Option<String>,
    pub fence_and_failsafe_verified: bool,
    pub manual_abort_procedure_rehearsed: bool,
    pub first_allowed_mission_type: Option<String>,
    pub single_drone_gate_passed: bool,
    pub multi_drone_review_required: bool,
}

pub fn write_hardware_entry_pack(
    output_dir: impl AsRef<Path>,
    plan: &SitlPlan,
    command: Vec<String>,
) -> Result<HardwareEntryPack, SitlError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| SitlError::HardwareEntryPackWrite {
        path: output_dir.to_path_buf(),
        message: error.to_string(),
    })?;

    let pack = build_hardware_entry_pack(plan, command);
    let path = output_dir.join(HARDWARE_ENTRY_PACK_FILE);
    let content =
        serde_json::to_string_pretty(&pack).map_err(|error| SitlError::HardwareEntryPackWrite {
            path: path.clone(),
            message: error.to_string(),
        })?;
    fs::write(&path, content).map_err(|error| SitlError::HardwareEntryPackWrite {
        path,
        message: error.to_string(),
    })?;
    Ok(pack)
}

pub fn build_hardware_entry_pack(plan: &SitlPlan, command: Vec<String>) -> HardwareEntryPack {
    let git_commit = current_git_commit();
    let px4_artifact = dry_run_artifact_with_mavlink_profile(
        plan,
        command.clone(),
        MavlinkCapabilityProfileId::Px4,
    );
    let px4_plan = px4_artifact.mavlink_common_plan.as_ref();
    let primitive_evidence = if plan.primitive_mission.is_some() {
        px4_plan.map(execute_locally)
    } else {
        None
    };
    let urban_execution_report = if is_urban_plan(plan) {
        px4_plan.map(execute_locally)
    } else {
        None
    };
    let fc_contract_result = px4_plan
        .and_then(|plan| plan.fc_contract_result.clone())
        .unwrap_or_else(default_fc_contract_result);
    let fence_plan = px4_plan.and_then(|plan| plan.fence_summary.clone());
    let dual_stack_evidence = dual_stack_evidence_for(plan, &command, &git_commit);
    let urban_evidence = urban_evidence_for(plan, urban_execution_report, &git_commit);
    let mission_families_covered = mission_families_covered(plan);
    let multi_drone_review_required = mission_families_covered
        .iter()
        .any(|family| family.contains("multi"));
    let readiness_status = if plan.primitive_mission.is_some() {
        HardwareReadinessStatus::ExecuteValidatedLocally
    } else {
        HardwareReadinessStatus::DryRunOnly
    };

    HardwareEntryPack {
        schema_version: HARDWARE_ENTRY_PACK_SCHEMA_VERSION.to_owned(),
        pack_id: format!("m97-{}-{}", plan.scenario_name, plan.agent_id),
        created_at: Utc::now(),
        git_commit,
        mission_families_covered,
        primitive_evidence,
        urban_evidence,
        swarm_evidence: None,
        dual_stack_evidence,
        fc_contract_result,
        param_snapshot: None,
        fence_plan,
        swarm_protocol_assumptions: vec![
            "MAVLink transport is not exercised by this pack; evidence is compiler/local-executor only."
                .to_owned(),
            "Swarm command fanout evidence is absent unless an explicit swarm artifact is attached."
                .to_owned(),
        ],
        topology_assumptions: vec![
            "single_agent_no_network_topology".to_owned(),
            "multi_agent_hardware_requires_separate_review".to_owned(),
        ],
        degraded_policy_matrix: default_degraded_policy_matrix(),
        preflight_report: plan.safety_report.clone(),
        hardware_entry_checklist: HardwareEntryChecklist {
            selected_autopilot: Some(MavlinkCapabilityProfileId::Px4.as_str().to_owned()),
            selected_airframe: None,
            selected_link_class: Some("not_connected_local_artifact".to_owned()),
            coordinate_frame_policy: Some(plan.coordinate_frame.name().to_owned()),
            altitude_reference: Some(plan.altitude_source.clone()),
            fence_and_failsafe_verified: false,
            manual_abort_procedure_rehearsed: false,
            first_allowed_mission_type: Some(first_allowed_mission_type(plan).to_owned()),
            single_drone_gate_passed: true,
            multi_drone_review_required,
        },
        run_command: command.join(" "),
        readiness_status,
        caveats: vec![
            "No hardware flight was performed.".to_owned(),
            "No certification, regulatory approval, or operator training claim is made.".to_owned(),
            "SITL/local executor evidence must not be treated as hardware safety proof.".to_owned(),
        ],
        limitations: vec![
            "Real airframe failsafes, battery behavior, GNSS/estimator failures, RF loss, and obstacle avoidance are not verified.".to_owned(),
            "PX4 profile evidence is compiler/local-executor evidence unless a separate live SITL or hardware artifact is attached.".to_owned(),
            "ArduPilot profile evidence is conservative and may contain unsupported/caveated commands.".to_owned(),
        ],
        blockers: Vec::new(),
    }
}

fn execute_locally(plan: &swarm_comms::MavlinkCommonPlan) -> MavlinkPlanExecutionReport {
    let mut executor = MavlinkPlanExecutor::new(MockAckProvider, 0);
    executor.execute(plan)
}

fn default_fc_contract_result() -> FcContractValidationResult {
    FcContractValidationResult {
        violations: Vec::new(),
        blocks_mission_start: false,
        summary: "No FC fence/parameter contract was requested by this pack.".to_owned(),
    }
}

fn dual_stack_evidence_for(
    plan: &SitlPlan,
    command: &[String],
    git_commit: &str,
) -> Option<DualStackExecutionEvidence> {
    let px4_artifact = dry_run_artifact_with_mavlink_profile(
        plan,
        command.to_owned(),
        MavlinkCapabilityProfileId::Px4,
    );
    let ardupilot_artifact = dry_run_artifact_with_mavlink_profile(
        plan,
        command.to_owned(),
        MavlinkCapabilityProfileId::ArduPilot,
    );
    let px4_plan = px4_artifact.mavlink_common_plan.as_ref()?;
    let ardupilot_plan = ardupilot_artifact.mavlink_common_plan.as_ref()?;
    build_dual_stack_execution_evidence(px4_plan, ardupilot_plan, git_commit.to_owned()).ok()
}

fn urban_evidence_for(
    plan: &SitlPlan,
    execution_report: Option<MavlinkPlanExecutionReport>,
    git_commit: &str,
) -> Option<UrbanOperationalEvidence> {
    if !is_urban_plan(plan) {
        return None;
    }
    let route_slice = plan
        .waypoints
        .iter()
        .filter_map(|waypoint| waypoint.edge_id.as_deref())
        .collect::<Vec<_>>()
        .join("..");
    Some(UrbanOperationalEvidence {
        schema_version: URBAN_OPERATIONAL_EVIDENCE_SCHEMA_VERSION.to_owned(),
        mission_id: format!("m97-{}-{}", plan.scenario_name, plan.agent_id),
        mission_family: plan.mission.clone(),
        created_at: Utc::now(),
        git_commit: git_commit.to_owned(),
        deconfliction_mode: DeconflictionMode::SharedMemory,
        agent_count: 1,
        sector_assignments: vec![(
            AgentId::from(plan.agent_id.clone()),
            if route_slice.is_empty() {
                "single_agent_route".to_owned()
            } else {
                route_slice
            },
            true,
        )],
        handoff_events: Vec::new(),
        coordination_delay_ticks: 0,
        degraded_outcomes: Vec::new(),
        execution_report,
        preflight_report: plan.safety_report.clone(),
        caveats: vec![
            "single_agent_urban_export_evidence_only".to_owned(),
            "no_real_perception_or_collision_avoidance".to_owned(),
        ],
    })
}

fn is_urban_plan(plan: &SitlPlan) -> bool {
    plan.export_kind == "urban_route" || plan.mission.starts_with("urban-")
}

fn mission_families_covered(plan: &SitlPlan) -> Vec<String> {
    if plan.primitive_mission.is_some() {
        vec!["primitive:takeoff-hold-land".to_owned()]
    } else if is_urban_plan(plan) {
        vec!["urban:single-drone".to_owned()]
    } else {
        vec![format!("scenario:{}", plan.mission)]
    }
}

fn first_allowed_mission_type(plan: &SitlPlan) -> &'static str {
    if plan.primitive_mission.is_some() {
        "primitive_takeoff_hold_land"
    } else if is_urban_plan(plan) {
        "urban_single_drone"
    } else {
        "single_drone_sitl_dry_run"
    }
}

fn default_degraded_policy_matrix() -> Vec<DegradedPolicyEntry> {
    vec![
        DegradedPolicyEntry {
            condition: "gcs_unavailable".to_owned(),
            policy: "hold_or_abort_requires_external_operator_procedure".to_owned(),
            tested: false,
        },
        DegradedPolicyEntry {
            condition: "single_agent_mission_failure".to_owned(),
            policy: "do_not_continue_to_hardware_without_separate_sitl_or_bench_evidence"
                .to_owned(),
            tested: false,
        },
        DegradedPolicyEntry {
            condition: "multi_agent_link_or_agent_loss".to_owned(),
            policy: "requires separate multi-drone safety review before hardware".to_owned(),
            tested: false,
        },
    ]
}

fn current_git_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_readiness_status_serde_roundtrip_all_variants() {
        let variants = [
            HardwareReadinessStatus::DryRunOnly,
            HardwareReadinessStatus::ExecuteValidatedLocally,
            HardwareReadinessStatus::DegradedPartiallyEvidenced,
            HardwareReadinessStatus::UnsupportedOrUnknown {
                detail: "missing_profile".to_owned(),
            },
            HardwareReadinessStatus::Blocked {
                reason: "preflight_failed".to_owned(),
            },
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let roundtrip: HardwareReadinessStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(roundtrip, variant);
        }
    }

    #[test]
    fn hardware_entry_checklist_serde_roundtrip() {
        let checklist = HardwareEntryChecklist {
            selected_autopilot: Some("px4".to_owned()),
            selected_airframe: Some("bench_airframe".to_owned()),
            selected_link_class: Some("serial".to_owned()),
            coordinate_frame_policy: Some("local_simulation".to_owned()),
            altitude_reference: Some("relative".to_owned()),
            fence_and_failsafe_verified: false,
            manual_abort_procedure_rehearsed: false,
            first_allowed_mission_type: Some("primitive_takeoff_hold_land".to_owned()),
            single_drone_gate_passed: true,
            multi_drone_review_required: false,
        };
        let json = serde_json::to_string(&checklist).unwrap();
        let roundtrip: HardwareEntryChecklist = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, checklist);
    }
}
