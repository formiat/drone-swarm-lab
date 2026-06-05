use serde::{Deserialize, Serialize};
use swarm_comms::{MavlinkCommonPlan, MavlinkExpectedAck, MavlinkTelemetryMilestone};
use swarm_mission_ir::MissionCommandPlan;
use swarm_types::{AgentId, Role};

/// Schema identifier for M87/M88 command-plane artifacts.
pub const SWARM_COMMAND_PLANE_SCHEMA_VERSION: &str = "swarm_command_plane.v1";

/// Mission-level role used by the swarm command plane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmCommandRole {
    Scout,
    Observer,
    Relay,
    Leader,
    Coordinator,
    Mothership,
    Carrier,
    Reserve,
    Recovery,
}

impl From<&Role> for SwarmCommandRole {
    fn from(role: &Role) -> Self {
        match role {
            Role::Scout => Self::Scout,
            Role::Relay => Self::Relay,
            Role::Mapper | Role::Inspector | Role::Thermal => Self::Observer,
            Role::Carrier => Self::Carrier,
        }
    }
}

/// Command-plane supervisor lifecycle state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmSupervisorState {
    Planned,
    Dispatched,
    Active,
    Degraded,
    Replacing,
    Aborting,
    Completed,
    Failed,
}

/// Per-agent and global failure policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmAbortPolicy {
    AbortAgentOnly,
    AbortMission,
    ContinueDegraded,
    ReplaceFromReserve,
}

/// Resource category owned by an agent at mission coordination level.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmOwnershipKind {
    Task,
    RouteSegment,
    Target,
    ReplacementMission,
}

/// Ownership status for a resource record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmOwnershipStatus {
    Active,
    Released,
}

/// Reference from an agent plan to an owned resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmOwnershipRef {
    pub kind: SwarmOwnershipKind,
    pub resource_id: String,
}

/// Timestamped ownership record in the command plane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmOwnershipRecord {
    pub agent_id: AgentId,
    pub kind: SwarmOwnershipKind,
    pub resource_id: String,
    pub status: SwarmOwnershipStatus,
    pub tick: u64,
    pub reason: String,
}

/// Explicit ownership handoff between agents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmOwnershipHandoff {
    pub from_agent_id: AgentId,
    pub to_agent_id: AgentId,
    pub kind: SwarmOwnershipKind,
    pub resource_id: String,
    pub tick: u64,
    pub reason: String,
}

/// Synchronized GCS command kind represented by M87.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SynchronizedCommandKind {
    ArmAll,
    TakeoffAll,
    StartAll,
    AbortAll,
}

/// Policy for accepting synchronized command partial success.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialSuccessPolicy {
    RequireAll,
    AtLeast { agents: usize },
}

/// A single synchronized command window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynchronizedCommandWindow {
    pub kind: SynchronizedCommandKind,
    pub agent_ids: Vec<AgentId>,
    pub timeout_ms: u64,
    pub partial_success_policy: PartialSuccessPolicy,
}

/// Deterministic result of one synchronized command window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynchronizedCommandResult {
    pub kind: SynchronizedCommandKind,
    pub succeeded: Vec<AgentId>,
    pub failed: Vec<AgentId>,
    pub timed_out: Vec<AgentId>,
    pub accepted: bool,
}

/// Logical topology kind used by the command plane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmTopologyKind {
    CentralizedGcs,
    P2pLogical,
    Mothership,
    Relay,
    Mesh,
}

/// Node category in a logical command-plane topology.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmTopologyNodeKind {
    Gcs,
    Agent,
    Relay,
    Mothership,
    Carrier,
}

/// Transport abstraction described by a topology artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmTransportDeliveryModel {
    InMemory,
    Logical,
    LegacyUdp,
    FutureMavlinkRouter,
}

/// Explicit non-hardware assumptions for topology routing evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SwarmTransportAssumptions {
    pub delivery_model: SwarmTransportDeliveryModel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_delay_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_rate: Option<f64>,
    pub hardware_boundary: String,
}

/// One logical topology node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmTopologyNode {
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub kind: SwarmTopologyNodeKind,
    #[serde(default = "default_available")]
    pub available: bool,
}

/// One directed logical topology link.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SwarmTopologyLink {
    pub from_node_id: String,
    pub to_node_id: String,
    #[serde(default = "default_available")]
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Mission-level parent/child dependency for mothership-style coordination.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmMothershipDependency {
    pub parent_agent_id: AgentId,
    pub child_agent_id: AgentId,
    pub dependency_kind: String,
    pub reason: String,
}

/// Logical topology configuration embedded in command-plane artifacts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SwarmTopologyConfig {
    pub kind: SwarmTopologyKind,
    pub gcs_node_id: String,
    pub nodes: Vec<SwarmTopologyNode>,
    pub links: Vec<SwarmTopologyLink>,
    pub transport: SwarmTransportAssumptions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mothership_dependencies: Vec<SwarmMothershipDependency>,
}

/// Deterministic routing decision for a command delivered to an agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmCommandRoute {
    pub route_id: String,
    pub from_node_id: String,
    pub to_agent_id: AgentId,
    pub via_node_ids: Vec<String>,
    pub allowed: bool,
    pub degraded: bool,
    pub reason: String,
}

fn default_available() -> bool {
    true
}

/// Per-agent command plan produced by M87 fanout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SwarmAgentCommandPlan {
    pub agent_id: AgentId,
    pub role: SwarmCommandRole,
    pub command_plan: MissionCommandPlan,
    pub mavlink_plan: MavlinkCommonPlan,
    pub expected_acks: Vec<MavlinkExpectedAck>,
    pub telemetry_milestones: Vec<MavlinkTelemetryMilestone>,
    pub abort_policy: SwarmAbortPolicy,
    pub ownership_refs: Vec<SwarmOwnershipRef>,
}

/// Compact command-plane summary suitable for manifests/reports.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmCommandArtifactSummary {
    pub schema_version: String,
    pub plan_id: String,
    pub agent_plan_count: usize,
    pub active_ownership_count: usize,
    pub handoff_count: usize,
    pub sync_operation_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology_kind: Option<SwarmTopologyKind>,
    #[serde(default)]
    pub topology_node_count: usize,
    #[serde(default)]
    pub topology_link_count: usize,
    #[serde(default)]
    pub command_route_count: usize,
    #[serde(default)]
    pub degraded_route_count: usize,
    #[serde(default)]
    pub mothership_dependency_count: usize,
}

/// Complete M87/M88 command-plane artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SwarmCommandPlan {
    pub schema_version: String,
    pub plan_id: String,
    pub supervisor_state: SwarmSupervisorState,
    pub agents: Vec<SwarmAgentCommandPlan>,
    pub ownership: Vec<SwarmOwnershipRecord>,
    pub handoffs: Vec<SwarmOwnershipHandoff>,
    pub global_abort_policy: SwarmAbortPolicy,
    pub sync_operations: Vec<SynchronizedCommandWindow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sync_results: Vec<SynchronizedCommandResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<SwarmTopologyConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_routes: Vec<SwarmCommandRoute>,
    pub summary: SwarmCommandArtifactSummary,
}
