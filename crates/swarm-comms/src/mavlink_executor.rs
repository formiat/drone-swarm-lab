//! MAVLink plan execution bridge (M90).
//!
//! Connects `MavlinkCommonPlan` (compiled by M81) to step-by-step execution
//! against an `AckProvider`. Does not require a live FC — mock providers cover
//! all test scenarios without hardware.

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::mavlink_common_plan::{
    MavlinkCommonCommand, MavlinkCommonPlan, MavlinkTelemetryMilestoneKind,
};
use crate::mavlink_geofence::MavlinkFencePlan;
use crate::mavlink_parameters::{
    FcParamId, FcParamRequirement, FcParamSnapshot, FcParamValue, FcParamWritePlan,
};

pub const MAVLINK_EXECUTION_ARTIFACT_SCHEMA_VERSION: &str = "mavlink_execution_artifact.v1";

// ─── Step result ─────────────────────────────────────────────────────────────

/// Result of one execution step returned by an `AckProvider`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "result")]
pub enum MavlinkExecutionStepResult {
    /// FC accepted the command or upload.
    Accepted,
    /// FC rejected the command; aborting is appropriate.
    Rejected { reason: String },
    /// No response received within the expected window.
    Timeout { after_ms: u64 },
    /// Step skipped; execution continues. Used for incompatible profile steps.
    Skipped { reason: String },
}

impl MavlinkExecutionStepResult {
    /// Returns true if this result should trigger a retry attempt.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout { .. })
    }

    /// Returns true if this result causes a terminal failure.
    pub fn is_terminal_failure(&self) -> bool {
        matches!(self, Self::Rejected { .. } | Self::Timeout { .. })
    }
}

// ─── Execution outcome ───────────────────────────────────────────────────────

/// Overall outcome of a `MavlinkPlanExecutor::execute` call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum MavlinkExecutionOutcome {
    /// All phases completed successfully without retries.
    Completed,
    /// Completed successfully; one or more retries were needed.
    Retried { times: u32 },
    /// Execution stopped early due to FC rejection or contract violation.
    Aborted { at_step: usize, reason: String },
    /// Transport or adapter failed outside the logical ACK sequence.
    Failed { at_step: usize, reason: String },
}

impl MavlinkExecutionOutcome {
    /// Returns true if execution ended successfully (with or without retries).
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Completed | Self::Retried { .. })
    }
}

// ─── Lifecycle state ─────────────────────────────────────────────────────────

/// Lifecycle state of one mission execution attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionExecuteLifecycleState {
    /// Plan compiled and validated; no upload attempted.
    Planned,
    /// Upload accepted by FC.
    Uploaded,
    /// Mission start command accepted.
    Started,
    /// Mission reached terminal condition successfully.
    Completed,
    /// Execution aborted by supervisor or FC.
    Aborted,
    /// Feature or command not supported by the selected profile.
    Unsupported,
}

// ─── Execution report ────────────────────────────────────────────────────────

/// Full report produced by `MavlinkPlanExecutor::execute`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MavlinkPlanExecutionReport {
    /// Source mission id from the compiled plan.
    pub plan_id: String,
    /// Ordered execution steps.
    ///
    /// value: `(step_index, command_name_or_phase, result)`
    pub steps: Vec<(usize, String, MavlinkExecutionStepResult)>,
    /// Overall execution outcome.
    pub overall: MavlinkExecutionOutcome,
    /// Lifecycle state at the end of execution.
    pub lifecycle_state: MissionExecuteLifecycleState,
    /// Telemetry milestones treated as reached during mock execution.
    pub telemetry_milestones_reached: Vec<MavlinkTelemetryMilestoneKind>,
    /// Total number of retries across all steps.
    pub retry_count: u32,
}

/// Execution backend used to produce a MAVLink execution report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MavlinkExecutionEvidenceMode {
    /// Deterministic local executor backed by [`MockAckProvider`].
    LocalMockExecutor,
    /// Deterministic local executor backed by [`ScriptedAckProvider`].
    ScriptedProfileExecutor,
    /// Real MAVLink transport-backed execution path.
    TransportBacked,
}

/// Standalone machine-checkable artifact for one MAVLink execution attempt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MavlinkExecutionArtifact {
    pub schema_version: String,
    pub execution_mode: MavlinkExecutionEvidenceMode,
    pub profile_id: String,
    pub plan_id: String,
    pub git_commit: String,
    pub command: Vec<String>,
    pub execution_report: MavlinkPlanExecutionReport,
    pub caveats: Vec<String>,
}

impl MavlinkExecutionArtifact {
    pub fn new(
        execution_mode: MavlinkExecutionEvidenceMode,
        profile_id: impl Into<String>,
        git_commit: impl Into<String>,
        command: Vec<String>,
        execution_report: MavlinkPlanExecutionReport,
        caveats: Vec<String>,
    ) -> Self {
        let plan_id = execution_report.plan_id.clone();
        Self {
            schema_version: MAVLINK_EXECUTION_ARTIFACT_SCHEMA_VERSION.to_owned(),
            execution_mode,
            profile_id: profile_id.into(),
            plan_id,
            git_commit: git_commit.into(),
            command,
            execution_report,
            caveats,
        }
    }
}

// ─── AckProvider trait ───────────────────────────────────────────────────────

/// Provides ACKs for each plan phase during execution.
///
/// `MockAckProvider` accepts everything (dry-run).
/// `ScriptedAckProvider` returns a predetermined sequence (tests).
pub trait AckProvider {
    fn ack_prelude_command(&mut self, command: &MavlinkCommonCommand)
        -> MavlinkExecutionStepResult;

    fn ack_mission_upload(&mut self) -> MavlinkExecutionStepResult;

    fn ack_mission_start(&mut self) -> MavlinkExecutionStepResult;

    fn ack_postlude_command(
        &mut self,
        command: &MavlinkCommonCommand,
    ) -> MavlinkExecutionStepResult;
}

// ─── MockAckProvider ─────────────────────────────────────────────────────────

/// Accepts every command and upload phase. Used in dry-run and fast tests.
pub struct MockAckProvider;

impl AckProvider for MockAckProvider {
    fn ack_prelude_command(&mut self, _: &MavlinkCommonCommand) -> MavlinkExecutionStepResult {
        MavlinkExecutionStepResult::Accepted
    }

    fn ack_mission_upload(&mut self) -> MavlinkExecutionStepResult {
        MavlinkExecutionStepResult::Accepted
    }

    fn ack_mission_start(&mut self) -> MavlinkExecutionStepResult {
        MavlinkExecutionStepResult::Accepted
    }

    fn ack_postlude_command(&mut self, _: &MavlinkCommonCommand) -> MavlinkExecutionStepResult {
        MavlinkExecutionStepResult::Accepted
    }
}

// ─── ScriptedAckProvider ─────────────────────────────────────────────────────

/// Returns predetermined results in order. Used in deterministic unit tests.
///
/// When the script is exhausted, falls back to `Accepted`.
pub struct ScriptedAckProvider {
    /// Each entry consumed in order.
    script: VecDeque<MavlinkExecutionStepResult>,
}

impl ScriptedAckProvider {
    pub fn new(script: impl IntoIterator<Item = MavlinkExecutionStepResult>) -> Self {
        Self {
            script: script.into_iter().collect(),
        }
    }

    fn next_result(&mut self) -> MavlinkExecutionStepResult {
        self.script
            .pop_front()
            .unwrap_or(MavlinkExecutionStepResult::Accepted)
    }
}

impl AckProvider for ScriptedAckProvider {
    fn ack_prelude_command(&mut self, _: &MavlinkCommonCommand) -> MavlinkExecutionStepResult {
        self.next_result()
    }

    fn ack_mission_upload(&mut self) -> MavlinkExecutionStepResult {
        self.next_result()
    }

    fn ack_mission_start(&mut self) -> MavlinkExecutionStepResult {
        self.next_result()
    }

    fn ack_postlude_command(&mut self, _: &MavlinkCommonCommand) -> MavlinkExecutionStepResult {
        self.next_result()
    }
}

// ─── MavlinkPlanExecutor ─────────────────────────────────────────────────────

/// Executes a `MavlinkCommonPlan` phase by phase against an `AckProvider`.
///
/// Does not own a transport: execution semantics are separated from the
/// MAVLink wire protocol so they can be tested without a live FC.
pub struct MavlinkPlanExecutor<A: AckProvider> {
    ack: A,
    retry_budget: u32,
}

impl<A: AckProvider> MavlinkPlanExecutor<A> {
    pub fn new(ack: A, retry_budget: u32) -> Self {
        Self { ack, retry_budget }
    }

    /// Execute a compiled plan phase by phase, returning a full execution report.
    pub fn execute(&mut self, plan: &MavlinkCommonPlan) -> MavlinkPlanExecutionReport {
        let mut steps: Vec<(usize, String, MavlinkExecutionStepResult)> = Vec::new();
        let mut total_retries = 0u32;
        let mut step_index = 0usize;

        // Guard: FC contract violation blocks execution before any commands are sent.
        if let Some(contract) = &plan.fc_contract_result {
            if contract.blocks_mission_start {
                return MavlinkPlanExecutionReport {
                    plan_id: plan.source_mission_id.clone(),
                    steps,
                    overall: MavlinkExecutionOutcome::Aborted {
                        at_step: 0,
                        reason: contract.summary.clone(),
                    },
                    lifecycle_state: MissionExecuteLifecycleState::Aborted,
                    telemetry_milestones_reached: vec![],
                    retry_count: 0,
                };
            }
        }

        // Phase 1: prelude commands.
        for command in &plan.command_prelude {
            let label = command.command.as_str().to_owned();
            let (result, retries) = self.try_with_retry(|ack| ack.ack_prelude_command(command));
            total_retries += retries;
            steps.push((step_index, label.clone(), result.clone()));

            match result {
                MavlinkExecutionStepResult::Rejected { reason } => {
                    return MavlinkPlanExecutionReport {
                        plan_id: plan.source_mission_id.clone(),
                        steps,
                        overall: MavlinkExecutionOutcome::Aborted {
                            at_step: step_index,
                            reason,
                        },
                        lifecycle_state: MissionExecuteLifecycleState::Aborted,
                        telemetry_milestones_reached: vec![],
                        retry_count: total_retries,
                    };
                }
                MavlinkExecutionStepResult::Timeout { after_ms } => {
                    return MavlinkPlanExecutionReport {
                        plan_id: plan.source_mission_id.clone(),
                        steps,
                        overall: MavlinkExecutionOutcome::Aborted {
                            at_step: step_index,
                            reason: format!("timeout after {after_ms}ms for {label}"),
                        },
                        lifecycle_state: MissionExecuteLifecycleState::Aborted,
                        telemetry_milestones_reached: vec![],
                        retry_count: total_retries,
                    };
                }
                MavlinkExecutionStepResult::Accepted
                | MavlinkExecutionStepResult::Skipped { .. } => {}
            }
            step_index += 1;
        }

        // Phase 2: mission upload (only when items exist).
        if !plan.mission_items.is_empty() {
            let (result, retries) = self.try_with_retry(|ack| ack.ack_mission_upload());
            total_retries += retries;
            steps.push((step_index, "mission_upload".to_owned(), result.clone()));

            match result {
                MavlinkExecutionStepResult::Rejected { reason } => {
                    return MavlinkPlanExecutionReport {
                        plan_id: plan.source_mission_id.clone(),
                        steps,
                        overall: MavlinkExecutionOutcome::Aborted {
                            at_step: step_index,
                            reason,
                        },
                        lifecycle_state: MissionExecuteLifecycleState::Aborted,
                        telemetry_milestones_reached: vec![],
                        retry_count: total_retries,
                    };
                }
                MavlinkExecutionStepResult::Timeout { after_ms } => {
                    return MavlinkPlanExecutionReport {
                        plan_id: plan.source_mission_id.clone(),
                        steps,
                        overall: MavlinkExecutionOutcome::Aborted {
                            at_step: step_index,
                            reason: format!("upload timeout after {after_ms}ms"),
                        },
                        lifecycle_state: MissionExecuteLifecycleState::Aborted,
                        telemetry_milestones_reached: vec![],
                        retry_count: total_retries,
                    };
                }
                MavlinkExecutionStepResult::Accepted
                | MavlinkExecutionStepResult::Skipped { .. } => {}
            }
            step_index += 1;
        }

        // Phase 3: mission start command.
        if let Some(start_command) = &plan.mission_start {
            let label = start_command.command.as_str().to_owned();
            let (result, retries) = self.try_with_retry(|ack| ack.ack_mission_start());
            total_retries += retries;
            steps.push((step_index, label.clone(), result.clone()));

            match result {
                MavlinkExecutionStepResult::Rejected { reason } => {
                    return MavlinkPlanExecutionReport {
                        plan_id: plan.source_mission_id.clone(),
                        steps,
                        overall: MavlinkExecutionOutcome::Aborted {
                            at_step: step_index,
                            reason,
                        },
                        lifecycle_state: MissionExecuteLifecycleState::Aborted,
                        telemetry_milestones_reached: vec![],
                        retry_count: total_retries,
                    };
                }
                MavlinkExecutionStepResult::Timeout { after_ms } => {
                    return MavlinkPlanExecutionReport {
                        plan_id: plan.source_mission_id.clone(),
                        steps,
                        overall: MavlinkExecutionOutcome::Aborted {
                            at_step: step_index,
                            reason: format!("mission start timeout after {after_ms}ms"),
                        },
                        lifecycle_state: MissionExecuteLifecycleState::Aborted,
                        telemetry_milestones_reached: vec![],
                        retry_count: total_retries,
                    };
                }
                MavlinkExecutionStepResult::Accepted
                | MavlinkExecutionStepResult::Skipped { .. } => {}
            }
            step_index += 1;
        }

        // Phase 4: postlude commands.
        for command in &plan.command_postlude {
            let label = command.command.as_str().to_owned();
            let (result, retries) = self.try_with_retry(|ack| ack.ack_postlude_command(command));
            total_retries += retries;
            steps.push((step_index, label.clone(), result.clone()));

            match result {
                MavlinkExecutionStepResult::Rejected { reason } => {
                    return MavlinkPlanExecutionReport {
                        plan_id: plan.source_mission_id.clone(),
                        steps,
                        overall: MavlinkExecutionOutcome::Aborted {
                            at_step: step_index,
                            reason,
                        },
                        lifecycle_state: MissionExecuteLifecycleState::Aborted,
                        telemetry_milestones_reached: vec![],
                        retry_count: total_retries,
                    };
                }
                MavlinkExecutionStepResult::Timeout { after_ms } => {
                    return MavlinkPlanExecutionReport {
                        plan_id: plan.source_mission_id.clone(),
                        steps,
                        overall: MavlinkExecutionOutcome::Aborted {
                            at_step: step_index,
                            reason: format!("postlude timeout after {after_ms}ms for {label}"),
                        },
                        lifecycle_state: MissionExecuteLifecycleState::Aborted,
                        telemetry_milestones_reached: vec![],
                        retry_count: total_retries,
                    };
                }
                MavlinkExecutionStepResult::Accepted
                | MavlinkExecutionStepResult::Skipped { .. } => {}
            }
            step_index += 1;
        }

        // All phases completed.
        let overall = if total_retries == 0 {
            MavlinkExecutionOutcome::Completed
        } else {
            MavlinkExecutionOutcome::Retried {
                times: total_retries,
            }
        };

        MavlinkPlanExecutionReport {
            plan_id: plan.source_mission_id.clone(),
            steps,
            overall,
            lifecycle_state: MissionExecuteLifecycleState::Completed,
            telemetry_milestones_reached: vec![MavlinkTelemetryMilestoneKind::HeartbeatExpected],
            retry_count: total_retries,
        }
    }

    /// Call `call` repeatedly until it succeeds or the retry budget is exhausted.
    ///
    /// Returns the final result and the number of retries consumed.
    fn try_with_retry<F>(&mut self, mut call: F) -> (MavlinkExecutionStepResult, u32)
    where
        F: FnMut(&mut A) -> MavlinkExecutionStepResult,
    {
        let mut retry_count = 0u32;
        loop {
            let result = call(&mut self.ack);
            match &result {
                MavlinkExecutionStepResult::Timeout { .. } if retry_count < self.retry_budget => {
                    retry_count += 1;
                }
                _ => return (result, retry_count),
            }
        }
    }
}

// ─── FC config provider ──────────────────────────────────────────────────────

/// Summary of a successful geofence upload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeofenceUploadOk {
    /// Number of geofence items uploaded.
    pub items_uploaded: usize,
    /// Whether `MAV_CMD_DO_FENCE_ENABLE` was sent after the items.
    pub fence_enable_sent: bool,
}

/// Result of a geofence upload attempt.
pub type GeofenceUploadResult = Result<GeofenceUploadOk, FcConfigError>;

/// Summary of a successful parameter write.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamWriteOk {
    /// Number of parameters written.
    pub written_count: usize,
}

/// Result of a parameter write attempt.
pub type FcParamWriteResult = Result<FcParamWriteOk, FcConfigError>;

/// Errors from FC configuration operations.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum FcConfigError {
    #[error("geofence upload rejected: {reason}")]
    GeofenceUploadRejected { reason: String },
    #[error("param read failed for {param_id}: {reason}")]
    ParamReadFailed { param_id: String, reason: String },
    #[error("param write failed for {param_id}: {reason}")]
    ParamWriteFailed { param_id: String, reason: String },
    #[error("fc contract violation blocks config: {summary}")]
    ContractViolation { summary: String },
}

/// Provides geofence upload, param read, and param write against some FC channel.
///
/// Abstracted so that mock implementations can be used without a live FC.
pub trait FcConfigProvider {
    fn upload_fence(&mut self, plan: &MavlinkFencePlan) -> GeofenceUploadResult;
    fn read_params(
        &mut self,
        requirements: &[FcParamRequirement],
    ) -> Result<FcParamSnapshot, FcConfigError>;
    fn write_params(&mut self, plan: &FcParamWritePlan) -> FcParamWriteResult;
}

// ─── MockFcConfigProvider ────────────────────────────────────────────────────

/// Simulates FC config operations in-memory. Used in tests.
pub struct MockFcConfigProvider {
    accept_geofence: bool,
    /// key: `param_id`
    param_values: HashMap<FcParamId, FcParamValue>,
}

impl Default for MockFcConfigProvider {
    fn default() -> Self {
        Self {
            accept_geofence: true,
            param_values: HashMap::new(),
        }
    }
}

impl MockFcConfigProvider {
    pub fn with_geofence_rejection(mut self) -> Self {
        self.accept_geofence = false;
        self
    }

    pub fn with_param(mut self, id: FcParamId, value: FcParamValue) -> Self {
        self.param_values.insert(id, value);
        self
    }
}

impl FcConfigProvider for MockFcConfigProvider {
    fn upload_fence(&mut self, plan: &MavlinkFencePlan) -> GeofenceUploadResult {
        if !self.accept_geofence {
            return Err(FcConfigError::GeofenceUploadRejected {
                reason: "mock rejection".to_owned(),
            });
        }
        Ok(GeofenceUploadOk {
            items_uploaded: plan.items.len(),
            fence_enable_sent: plan.enable_fence,
        })
    }

    fn read_params(
        &mut self,
        requirements: &[FcParamRequirement],
    ) -> Result<FcParamSnapshot, FcConfigError> {
        let params = requirements
            .iter()
            .filter_map(|req| {
                self.param_values
                    .get(&req.param_id)
                    .map(|&value| (req.param_id.clone(), value))
            })
            .collect();
        Ok(FcParamSnapshot {
            params,
            description: "mock snapshot".to_owned(),
        })
    }

    fn write_params(&mut self, plan: &FcParamWritePlan) -> FcParamWriteResult {
        Ok(FcParamWriteOk {
            written_count: plan.writes.len(),
        })
    }
}

// ─── Standalone FC config functions ─────────────────────────────────────────

/// Upload a fence plan via a `FcConfigProvider`.
pub fn execute_geofence_upload<P: FcConfigProvider>(
    fence_plan: &MavlinkFencePlan,
    provider: &mut P,
) -> GeofenceUploadResult {
    provider.upload_fence(fence_plan)
}

/// Read a parameter snapshot for the given requirements via a `FcConfigProvider`.
pub fn execute_param_snapshot<P: FcConfigProvider>(
    requirements: &[FcParamRequirement],
    provider: &mut P,
) -> Result<FcParamSnapshot, FcConfigError> {
    provider.read_params(requirements)
}

/// Write parameters via a `FcConfigProvider`.
pub fn execute_param_write<P: FcConfigProvider>(
    plan: &FcParamWritePlan,
    provider: &mut P,
) -> FcParamWriteResult {
    provider.write_params(plan)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use swarm_mission_ir::{
        AltitudeReference, CommandId, CompletionTolerance, CoordinateFrame, LocalPosition,
        MissionCommand, MissionCommandEntry, MissionCommandPlan, MissionId, TerminalState,
        TimeoutAction, TimeoutPolicy,
    };

    use crate::mavlink_common_plan::{
        compile_mavlink_common_plan, MavlinkCommonCommandName, MavlinkCommonPlanOptions,
        MavlinkPlanPhase,
    };
    use crate::mavlink_coords::MavlinkCoordinateOrigin;
    use crate::mavlink_geofence::{
        FcGeofenceItem, FcGeofenceItemKind, FcGeofenceShape, MavlinkFencePlan,
    };
    use crate::mavlink_parameters::{
        FcParamId, FcParamRange, FcParamRequirement, FcParamSnapshot, FcParamValue,
        FcParamWritePlan,
    };

    use super::*;
    use crate::mavlink_common_plan::MavlinkCommonCommand;

    // ─── Plan builders ────────────────────────────────────────────────────────

    fn origin() -> MavlinkCoordinateOrigin {
        MavlinkCoordinateOrigin {
            lat_deg: 47.397_742,
            lon_deg: 8.545_594,
            alt_m: 0.0,
        }
    }

    fn local(x_m: f64, y_m: f64, z_m: f64) -> swarm_mission_ir::Position {
        swarm_mission_ir::Position::Local(LocalPosition { x_m, y_m, z_m })
    }

    fn entry(id: &str, command: MissionCommand) -> MissionCommandEntry {
        MissionCommandEntry {
            command_id: CommandId::from(id.to_owned()),
            command,
            source_task_id: None,
            source_route_id: None,
            source_agent_id: Some("agent-0".to_owned()),
        }
    }

    fn plan_ir(commands: Vec<MissionCommandEntry>) -> MissionCommandPlan {
        MissionCommandPlan {
            schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
            mission_id: MissionId::from("m90-test".to_owned()),
            coordinate_frame: CoordinateFrame::LocalEnu,
            altitude_reference: AltitudeReference::RelativeHome,
            timeout_policy: TimeoutPolicy {
                command_timeout_secs: 5.0,
                completion_timeout_secs: 120.0,
                on_timeout: TimeoutAction::Abort,
            },
            expected_terminal_state: TerminalState::Landed,
            completion_tolerance: CompletionTolerance {
                position_m: 1.0,
                altitude_m: 0.5,
            },
            commands,
        }
    }

    fn base_options() -> MavlinkCommonPlanOptions {
        MavlinkCommonPlanOptions {
            home_origin: Some(origin()),
            default_hold_position: Some(local(0.0, 0.0, 3.0)),
            ..Default::default()
        }
    }

    /// Takeoff → Hold → Land: produces prelude + mission_items + mission_start + postlude.
    fn takeoff_hold_land_plan() -> crate::mavlink_common_plan::MavlinkCommonPlan {
        compile_mavlink_common_plan(
            &plan_ir(vec![
                entry("takeoff", MissionCommand::Takeoff { altitude_m: 3.0 }),
                entry("hold", MissionCommand::Hold { duration_secs: 5.0 }),
                entry("land", MissionCommand::Land),
            ]),
            &base_options(),
        )
        .unwrap()
    }

    /// Arm → Takeoff → Land: prelude-only plan (no mission items, no upload/start).
    fn arm_takeoff_land_plan() -> crate::mavlink_common_plan::MavlinkCommonPlan {
        compile_mavlink_common_plan(
            &plan_ir(vec![
                entry("arm", MissionCommand::Arm),
                entry("takeoff", MissionCommand::Takeoff { altitude_m: 3.0 }),
                entry("land", MissionCommand::Land),
            ]),
            &base_options(),
        )
        .unwrap()
    }

    fn dummy_command() -> MavlinkCommonCommand {
        MavlinkCommonCommand {
            command_id: "test-0".to_owned(),
            command: MavlinkCommonCommandName::ComponentArmDisarm,
            phase: MavlinkPlanPhase::CommandPrelude,
            params: [Some(1.0), None, None, None, None, None, None],
        }
    }

    // ─── AckProvider tests ────────────────────────────────────────────────────

    #[test]
    fn scripted_ack_provider_returns_configured_sequence() {
        let expected = vec![
            MavlinkExecutionStepResult::Accepted,
            MavlinkExecutionStepResult::Rejected {
                reason: "nope".to_owned(),
            },
            MavlinkExecutionStepResult::Timeout { after_ms: 200 },
        ];
        let mut provider = ScriptedAckProvider::new(expected.clone());
        let cmd = dummy_command();
        assert_eq!(provider.ack_prelude_command(&cmd), expected[0]);
        assert_eq!(provider.ack_mission_upload(), expected[1]);
        assert_eq!(provider.ack_mission_start(), expected[2]);
    }

    #[test]
    fn scripted_ack_provider_falls_back_to_accepted_when_exhausted() {
        let mut provider = ScriptedAckProvider::new([MavlinkExecutionStepResult::Accepted]);
        let cmd = dummy_command();
        provider.ack_prelude_command(&cmd); // consume the one entry
        assert_eq!(
            provider.ack_mission_upload(),
            MavlinkExecutionStepResult::Accepted
        );
    }

    // ─── MavlinkPlanExecutor tests ────────────────────────────────────────────

    #[test]
    fn executor_completes_takeoff_hold_land_with_mock_ack() {
        let plan = takeoff_hold_land_plan();
        let mut executor = MavlinkPlanExecutor::new(MockAckProvider, 0);

        let report = executor.execute(&plan);

        assert!(report.overall.is_success(), "{:?}", report.overall);
        assert_eq!(
            report.lifecycle_state,
            MissionExecuteLifecycleState::Completed
        );
        assert!(report
            .telemetry_milestones_reached
            .contains(&MavlinkTelemetryMilestoneKind::HeartbeatExpected));
        assert_eq!(report.retry_count, 0);
    }

    #[test]
    fn executor_aborts_on_first_timeout() {
        // arm-takeoff-land has 3 prelude commands; first one times out
        let plan = arm_takeoff_land_plan();
        let mut executor = MavlinkPlanExecutor::new(
            ScriptedAckProvider::new([MavlinkExecutionStepResult::Timeout { after_ms: 100 }]),
            0,
        );

        let report = executor.execute(&plan);

        assert!(!report.overall.is_success());
        assert_eq!(
            report.lifecycle_state,
            MissionExecuteLifecycleState::Aborted
        );
        assert!(
            matches!(report.overall, MavlinkExecutionOutcome::Aborted { .. }),
            "{:?}",
            report.overall
        );
    }

    #[test]
    fn executor_skips_unsupported_feature_with_caveat() {
        // arm-takeoff-land: 3 prelude commands; first is skipped, rest accepted
        let plan = arm_takeoff_land_plan();
        let mut executor = MavlinkPlanExecutor::new(
            ScriptedAckProvider::new([
                MavlinkExecutionStepResult::Skipped {
                    reason: "ardupilot_mode_seq_differs".to_owned(),
                },
                MavlinkExecutionStepResult::Accepted,
                MavlinkExecutionStepResult::Accepted,
            ]),
            0,
        );

        let report = executor.execute(&plan);

        assert!(report.overall.is_success(), "{:?}", report.overall);
        assert!(report
            .steps
            .iter()
            .any(|(_, _, r)| matches!(r, MavlinkExecutionStepResult::Skipped { .. })));
    }

    #[test]
    fn executor_retries_within_budget_and_succeeds() {
        // First prelude command times out once, then succeeds on retry
        let plan = arm_takeoff_land_plan();
        let mut executor = MavlinkPlanExecutor::new(
            ScriptedAckProvider::new([
                MavlinkExecutionStepResult::Timeout { after_ms: 50 },
                MavlinkExecutionStepResult::Accepted, // succeeds on retry
                MavlinkExecutionStepResult::Accepted,
                MavlinkExecutionStepResult::Accepted,
            ]),
            1,
        );

        let report = executor.execute(&plan);

        assert!(report.overall.is_success(), "{:?}", report.overall);
        assert_eq!(report.retry_count, 1);
        assert!(matches!(
            report.overall,
            MavlinkExecutionOutcome::Retried { times: 1 }
        ));
    }

    #[test]
    fn executor_aborts_when_retry_budget_exhausted() {
        let plan = arm_takeoff_land_plan();
        let mut executor = MavlinkPlanExecutor::new(
            ScriptedAckProvider::new([
                MavlinkExecutionStepResult::Timeout { after_ms: 50 },
                MavlinkExecutionStepResult::Timeout { after_ms: 50 }, // retry also fails
            ]),
            1,
        );

        let report = executor.execute(&plan);

        assert!(!report.overall.is_success());
        assert!(
            matches!(report.overall, MavlinkExecutionOutcome::Aborted { .. }),
            "{:?}",
            report.overall
        );
        assert_eq!(report.retry_count, 1);
    }

    #[test]
    fn fc_contract_violation_blocks_execute() {
        let mut options = base_options();
        options.param_requirements = vec![FcParamRequirement {
            param_id: FcParamId::from("GF_ACTION".to_owned()),
            required_range: FcParamRange::IntBounds { min: 0, max: 5 },
            reason: "test".to_owned(),
        }];
        // Param value 99 is outside 0..=5 → violation → blocks_mission_start
        options.param_snapshot = Some(FcParamSnapshot {
            params: [(
                FcParamId::from("GF_ACTION".to_owned()),
                FcParamValue::Int32(99),
            )]
            .into(),
            description: "bad snapshot".to_owned(),
        });

        let plan = compile_mavlink_common_plan(
            &plan_ir(vec![entry(
                "takeoff",
                MissionCommand::Takeoff { altitude_m: 3.0 },
            )]),
            &options,
        )
        .unwrap();
        assert!(
            plan.fc_contract_result
                .as_ref()
                .unwrap()
                .blocks_mission_start
        );

        let mut executor = MavlinkPlanExecutor::new(MockAckProvider, 0);
        let report = executor.execute(&plan);

        assert!(!report.overall.is_success());
        assert_eq!(
            report.steps.len(),
            0,
            "no steps taken before contract guard"
        );
    }

    #[test]
    fn lifecycle_state_transitions_planned_to_completed() {
        let plan = takeoff_hold_land_plan(); // has mission_items → goes through upload+start
        let mut executor = MavlinkPlanExecutor::new(MockAckProvider, 0);

        let report = executor.execute(&plan);

        assert_eq!(
            report.lifecycle_state,
            MissionExecuteLifecycleState::Completed
        );
    }

    #[test]
    fn lifecycle_state_transitions_planned_to_aborted() {
        let plan = arm_takeoff_land_plan();
        let mut executor = MavlinkPlanExecutor::new(
            ScriptedAckProvider::new([MavlinkExecutionStepResult::Rejected {
                reason: "no".to_owned(),
            }]),
            0,
        );

        let report = executor.execute(&plan);

        assert_eq!(
            report.lifecycle_state,
            MissionExecuteLifecycleState::Aborted
        );
    }

    #[test]
    fn ardupilot_incompatible_step_is_skipped_not_panicked() {
        // arm-takeoff-land has 3 prelude commands; first two skipped, third accepted
        let plan = arm_takeoff_land_plan();
        let mut executor = MavlinkPlanExecutor::new(
            ScriptedAckProvider::new([
                MavlinkExecutionStepResult::Skipped {
                    reason: "ardupilot_mode_seq_differs".to_owned(),
                },
                MavlinkExecutionStepResult::Skipped {
                    reason: "ardupilot_mode_seq_differs".to_owned(),
                },
                MavlinkExecutionStepResult::Accepted,
            ]),
            0,
        );

        let report = executor.execute(&plan);

        assert!(report.overall.is_success(), "{:?}", report.overall);
        let skipped_count = report
            .steps
            .iter()
            .filter(|(_, _, r)| matches!(r, MavlinkExecutionStepResult::Skipped { .. }))
            .count();
        assert_eq!(skipped_count, 2);
    }

    #[test]
    fn executor_upload_phase_aborts_on_rejection() {
        // takeoff-hold-land has mission_items → upload phase is exercised
        let plan = takeoff_hold_land_plan();
        // prelude = [NavTakeoff]: 1 step, then upload is rejected
        let mut executor = MavlinkPlanExecutor::new(
            ScriptedAckProvider::new([
                MavlinkExecutionStepResult::Accepted, // prelude NavTakeoff
                MavlinkExecutionStepResult::Rejected {
                    reason: "upload refused".to_owned(),
                }, // upload
            ]),
            0,
        );

        let report = executor.execute(&plan);

        assert!(matches!(
            report.overall,
            MavlinkExecutionOutcome::Aborted { .. }
        ));
        assert_eq!(
            report.lifecycle_state,
            MissionExecuteLifecycleState::Aborted
        );
    }

    // ─── FC config provider tests ─────────────────────────────────────────────

    #[test]
    fn geofence_upload_emits_fence_enable_after_items() {
        let fence_plan = MavlinkFencePlan {
            items: vec![FcGeofenceItem {
                id: "poly".to_owned(),
                kind: FcGeofenceItemKind::PolygonInclusion,
                shape: FcGeofenceShape::Polygon {
                    vertices: vec![(1, 2), (3, 4), (5, 6)],
                },
            }],
            enable_fence: true,
        };
        let mut provider = MockFcConfigProvider::default();

        let ok = execute_geofence_upload(&fence_plan, &mut provider).unwrap();

        assert_eq!(ok.items_uploaded, 1);
        assert!(ok.fence_enable_sent);
    }

    #[test]
    fn geofence_upload_no_enable_when_disabled() {
        let fence_plan = MavlinkFencePlan {
            items: vec![FcGeofenceItem {
                id: "poly".to_owned(),
                kind: FcGeofenceItemKind::PolygonInclusion,
                shape: FcGeofenceShape::Polygon {
                    vertices: vec![(1, 2), (3, 4), (5, 6)],
                },
            }],
            enable_fence: false,
        };
        let mut provider = MockFcConfigProvider::default();

        let ok = execute_geofence_upload(&fence_plan, &mut provider).unwrap();

        assert!(!ok.fence_enable_sent);
    }

    #[test]
    fn geofence_upload_returns_typed_error_on_rejection() {
        let fence_plan = MavlinkFencePlan {
            items: vec![FcGeofenceItem {
                id: "poly".to_owned(),
                kind: FcGeofenceItemKind::PolygonInclusion,
                shape: FcGeofenceShape::Polygon {
                    vertices: vec![(1, 2), (3, 4), (5, 6)],
                },
            }],
            enable_fence: false,
        };
        let mut provider = MockFcConfigProvider::default().with_geofence_rejection();

        let result = execute_geofence_upload(&fence_plan, &mut provider);

        assert!(
            matches!(result, Err(FcConfigError::GeofenceUploadRejected { .. })),
            "{result:?}"
        );
    }

    #[test]
    fn param_snapshot_reads_all_required_params() {
        let requirements = vec![
            FcParamRequirement {
                param_id: FcParamId::from("GF_ACTION".to_owned()),
                required_range: FcParamRange::IntBounds { min: 0, max: 5 },
                reason: "test".to_owned(),
            },
            FcParamRequirement {
                param_id: FcParamId::from("GF_MAX_HOR_DIST".to_owned()),
                required_range: FcParamRange::FloatBounds {
                    min: 0.0,
                    max: 10_000.0,
                },
                reason: "test".to_owned(),
            },
        ];
        let mut provider = MockFcConfigProvider::default()
            .with_param(
                FcParamId::from("GF_ACTION".to_owned()),
                FcParamValue::Int32(1),
            )
            .with_param(
                FcParamId::from("GF_MAX_HOR_DIST".to_owned()),
                FcParamValue::Float(500.0),
            );

        let snapshot = execute_param_snapshot(&requirements, &mut provider).unwrap();

        assert_eq!(snapshot.params.len(), 2);
        assert!(snapshot
            .params
            .contains_key(&FcParamId::from("GF_ACTION".to_owned())));
        assert!(snapshot
            .params
            .contains_key(&FcParamId::from("GF_MAX_HOR_DIST".to_owned())));
    }

    #[test]
    fn param_snapshot_partial_when_param_absent() {
        let requirements = vec![FcParamRequirement {
            param_id: FcParamId::from("MISSING_PARAM".to_owned()),
            required_range: FcParamRange::ExactInt(1),
            reason: "test".to_owned(),
        }];
        let mut provider = MockFcConfigProvider::default(); // no params seeded

        let snapshot = execute_param_snapshot(&requirements, &mut provider).unwrap();

        assert!(snapshot.params.is_empty());
    }

    #[test]
    fn param_write_returns_written_count() {
        let plan = FcParamWritePlan {
            writes: vec![
                (
                    FcParamId::from("GF_ACTION".to_owned()),
                    FcParamValue::Int32(2),
                ),
                (
                    FcParamId::from("GF_MAX_HOR_DIST".to_owned()),
                    FcParamValue::Float(200.0),
                ),
            ],
            rationale: "test write".to_owned(),
        };
        let mut provider = MockFcConfigProvider::default();

        let ok = execute_param_write(&plan, &mut provider).unwrap();

        assert_eq!(ok.written_count, 2);
    }

    // ─── Serde roundtrip tests ────────────────────────────────────────────────

    #[test]
    fn mavlink_execution_step_result_serde_roundtrip() {
        let variants = vec![
            MavlinkExecutionStepResult::Accepted,
            MavlinkExecutionStepResult::Rejected {
                reason: "r".to_owned(),
            },
            MavlinkExecutionStepResult::Timeout { after_ms: 100 },
            MavlinkExecutionStepResult::Skipped {
                reason: "s".to_owned(),
            },
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let parsed: MavlinkExecutionStepResult = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, v);
        }
    }

    #[test]
    fn mavlink_execution_outcome_serde_roundtrip() {
        let variants = vec![
            MavlinkExecutionOutcome::Completed,
            MavlinkExecutionOutcome::Retried { times: 3 },
            MavlinkExecutionOutcome::Aborted {
                at_step: 1,
                reason: "r".to_owned(),
            },
            MavlinkExecutionOutcome::Failed {
                at_step: 2,
                reason: "f".to_owned(),
            },
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let parsed: MavlinkExecutionOutcome = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, v);
        }
    }

    #[test]
    fn mission_execute_lifecycle_state_serde_roundtrip() {
        let variants = vec![
            MissionExecuteLifecycleState::Planned,
            MissionExecuteLifecycleState::Uploaded,
            MissionExecuteLifecycleState::Started,
            MissionExecuteLifecycleState::Completed,
            MissionExecuteLifecycleState::Aborted,
            MissionExecuteLifecycleState::Unsupported,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let parsed: MissionExecuteLifecycleState = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, v);
        }
    }

    #[test]
    fn mavlink_execution_artifact_serde_roundtrip() {
        let plan = takeoff_hold_land_plan();
        let mut executor = MavlinkPlanExecutor::new(MockAckProvider, 0);
        let report = executor.execute(&plan);
        let artifact = MavlinkExecutionArtifact::new(
            MavlinkExecutionEvidenceMode::LocalMockExecutor,
            "px4",
            "test-commit",
            vec!["sitl_agent".to_owned(), "--execute".to_owned()],
            report,
            vec!["local_mock_executor".to_owned()],
        );

        let json = serde_json::to_string_pretty(&artifact).unwrap();
        let parsed: MavlinkExecutionArtifact = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, artifact);
        assert_eq!(
            parsed.schema_version,
            MAVLINK_EXECUTION_ARTIFACT_SCHEMA_VERSION
        );
    }

    #[test]
    fn fc_config_error_serde_roundtrip() {
        let variants = vec![
            FcConfigError::GeofenceUploadRejected {
                reason: "r".to_owned(),
            },
            FcConfigError::ParamReadFailed {
                param_id: "p".to_owned(),
                reason: "r".to_owned(),
            },
            FcConfigError::ContractViolation {
                summary: "s".to_owned(),
            },
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let parsed: FcConfigError = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, v);
        }
    }

    #[test]
    fn outcome_is_success_only_for_completed_and_retried() {
        assert!(MavlinkExecutionOutcome::Completed.is_success());
        assert!(MavlinkExecutionOutcome::Retried { times: 1 }.is_success());
        assert!(!MavlinkExecutionOutcome::Aborted {
            at_step: 0,
            reason: "r".to_owned()
        }
        .is_success());
        assert!(!MavlinkExecutionOutcome::Failed {
            at_step: 0,
            reason: "f".to_owned()
        }
        .is_success());
    }
}
