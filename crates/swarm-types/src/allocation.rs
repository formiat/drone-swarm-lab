use crate::agent::{AgentId, Capability, Role};
use crate::pose::Pose;

/// Enriched agent context passed to allocators.
///
/// Uses owned copies to avoid lifetime conflicts when building from MembershipView.
#[derive(Clone)]
pub struct AllocationAgent {
    pub id: AgentId,
    pub pose: Pose,
    pub battery: f64,
    pub capabilities: Vec<Capability>,
    pub role: Role,
    pub comms_range: f64,
}
