use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use swarm_comms::{DeconflictionMode, MavlinkExecutionEvidenceMode, MavlinkPlanExecutionReport};
use swarm_replay::{Event, EventLog};
use swarm_safety::preflight::SafetyValidationReport;
use swarm_types::AgentId;

pub const URBAN_OPERATIONAL_EVIDENCE_SCHEMA_VERSION: &str = "urban_operational_evidence.v1";
pub const URBAN_OPERATIONAL_EVIDENCE_PACK_SCHEMA_VERSION: &str =
    "urban_operational_evidence_pack.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanOperationalEvidence {
    pub schema_version: String,
    pub mission_id: String,
    pub mission_family: String,
    pub created_at: DateTime<Utc>,
    pub git_commit: String,
    pub deconfliction_mode: DeconflictionMode,
    pub agent_count: usize,
    /// value: `(agent_id, sector_or_route_slice_id, completed)`
    pub sector_assignments: Vec<(AgentId, String, bool)>,
    /// value: `(tick, from_agent, to_agent, resource_id)`
    pub handoff_events: Vec<(u64, AgentId, AgentId, String)>,
    pub coordination_delay_ticks: u64,
    pub degraded_outcomes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_mode: Option<MavlinkExecutionEvidenceMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_report: Option<MavlinkPlanExecutionReport>,
    pub preflight_report: SafetyValidationReport,
    pub caveats: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanOperationalEvidencePack {
    pub schema_version: String,
    pub evidence: Vec<UrbanOperationalEvidence>,
}

impl UrbanOperationalEvidencePack {
    pub fn new(evidence: Vec<UrbanOperationalEvidence>) -> Self {
        Self {
            schema_version: URBAN_OPERATIONAL_EVIDENCE_PACK_SCHEMA_VERSION.to_owned(),
            evidence,
        }
    }
}

pub fn build_urban_operational_evidence_from_replay(
    log: &EventLog,
    git_commit: impl Into<String>,
    deconfliction_mode: DeconflictionMode,
) -> Option<UrbanOperationalEvidence> {
    let mut agents = Vec::<AgentId>::new();
    let mut sector_assignments = Vec::new();
    let mut handoff_events = Vec::new();
    let mut coordination_delay_ticks = 0u64;
    let mut degraded_outcomes = Vec::new();

    for event in &log.events {
        match event {
            Event::UrbanRoutePlanned {
                agent_id, edge_ids, ..
            } => {
                push_unique_agent(&mut agents, agent_id);
                let completed = edge_ids.iter().all(|edge_id| {
                    log.events.iter().any(|event| {
                        matches!(
                            event,
                            Event::UrbanSegmentCompleted {
                                agent_id: completed_agent,
                                edge_id: completed_edge,
                                ..
                            } if completed_agent == agent_id && completed_edge == edge_id
                        )
                    })
                });
                sector_assignments.push((
                    agent_id.clone(),
                    route_slice_id(edge_ids),
                    completed && !edge_ids.is_empty(),
                ));
            }
            Event::UrbanSegmentEntered { agent_id, .. }
            | Event::UrbanSegmentCompleted { agent_id, .. }
            | Event::UrbanPatrolCompleted { agent_id, .. }
            | Event::UrbanSearchCompleted { agent_id, .. } => {
                push_unique_agent(&mut agents, agent_id);
            }
            Event::SwarmOwnershipHandoff {
                tick,
                from_agent_id,
                to_agent_id,
                resource_id,
                ..
            } => {
                handoff_events.push((
                    *tick,
                    from_agent_id.clone(),
                    to_agent_id.clone(),
                    resource_id.clone(),
                ));
            }
            Event::UrbanDeconflictWait { .. } => coordination_delay_ticks += 1,
            Event::UrbanNoRouteAvailable { reason, .. } => {
                degraded_outcomes.push(format!("no_route_available:{reason}"));
            }
            Event::CommandSuppressed {
                resource_id,
                reason,
                ..
            } => {
                degraded_outcomes.push(format!("command_suppressed:{resource_id}:{reason}"));
            }
            Event::SupervisorDegradedDecision {
                decision,
                resources,
                ..
            } => {
                degraded_outcomes.push(format!(
                    "supervisor_degraded:{decision:?}:{}",
                    resources.join(",")
                ));
            }
            _ => {}
        }
    }

    if agents.is_empty() && sector_assignments.is_empty() {
        return None;
    }

    Some(UrbanOperationalEvidence {
        schema_version: URBAN_OPERATIONAL_EVIDENCE_SCHEMA_VERSION.to_owned(),
        mission_id: log.run_id.clone(),
        mission_family: mission_family(&log.scenario_name),
        created_at: Utc::now(),
        git_commit: git_commit.into(),
        deconfliction_mode,
        agent_count: agents.len().max(sector_assignments.len()),
        sector_assignments,
        handoff_events,
        coordination_delay_ticks,
        degraded_outcomes,
        execution_mode: None,
        execution_report: None,
        preflight_report: SafetyValidationReport::ok(),
        caveats: vec![
            "simulation_only".to_owned(),
            "no_physical_collision_avoidance".to_owned(),
            "transport_backend_may_be_in_memory".to_owned(),
        ],
    })
}

pub fn urban_evidence_with_execution_report(
    mut evidence: UrbanOperationalEvidence,
    execution_mode: MavlinkExecutionEvidenceMode,
    execution_report: MavlinkPlanExecutionReport,
) -> UrbanOperationalEvidence {
    evidence.execution_mode = Some(execution_mode);
    evidence.execution_report = Some(execution_report);
    evidence
}

fn push_unique_agent(agents: &mut Vec<AgentId>, agent_id: &AgentId) {
    if !agents.iter().any(|existing| existing == agent_id) {
        agents.push(agent_id.clone());
        agents.sort_by(|left, right| left.as_ref().cmp(right.as_ref()));
    }
}

fn route_slice_id(edge_ids: &[swarm_types::UrbanEdgeId]) -> String {
    if edge_ids.is_empty() {
        return "empty-route-slice".to_owned();
    }
    format!(
        "{}..{}",
        edge_ids.first().expect("checked non-empty").as_ref(),
        edge_ids.last().expect("checked non-empty").as_ref()
    )
}

fn mission_family(scenario_name: &str) -> String {
    if scenario_name.contains("search") {
        "urban-search-until-detection".to_owned()
    } else if scenario_name.contains("corridor") || scenario_name.contains("inspection") {
        "urban-corridor-inspection".to_owned()
    } else if scenario_name.contains("blocked") || scenario_name.contains("replan") {
        "urban-blocked-route-recovery".to_owned()
    } else {
        "urban-perimeter-patrol".to_owned()
    }
}
