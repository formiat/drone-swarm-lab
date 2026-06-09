use serde::{Deserialize, Serialize};
use swarm_comms::{AgentMissionState, Lease};
use swarm_types::AgentId;

/// What action to take when the GCS link goes silent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum GcsLostPolicy {
    /// Continue executing plan; abort only after `max_gcs_lost_ticks` without GCS.
    ContinueMission { max_gcs_lost_ticks: u64 },
    /// Halt at current position; abort after `max_gcs_lost_ticks` without GCS.
    HoverInPlace { max_gcs_lost_ticks: u64 },
    /// Initiate RTL after `after_ticks` without a GCS heartbeat.
    ReturnToLaunch { after_ticks: u64 },
    /// Immediately abort and RTL on the first missed GCS heartbeat.
    AbortImmediate,
}

impl Default for GcsLostPolicy {
    fn default() -> Self {
        Self::HoverInPlace {
            max_gcs_lost_ticks: 30,
        }
    }
}

/// What action to take when the mothership link goes silent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum MothershipLostPolicy {
    /// Remain at staging point for up to `max_ticks` then abort.
    WaitAtStaging { max_ticks: u64 },
    /// Continue the current sub-mission autonomously.
    ProceedAutonomously,
    /// Initiate RTL immediately.
    ReturnToLaunch,
}

impl Default for MothershipLostPolicy {
    fn default() -> Self {
        Self::WaitAtStaging { max_ticks: 50 }
    }
}

/// What action to take when a neighbour agent disappears from the swarm.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum NeighborLostPolicy {
    /// Release any segment locks held by the lost agent and keep going.
    ///
    /// **Non-Goal (M93):** releasing segment locks held by a *different* agent requires a
    /// shared cross-agent lease registry, which is out of scope for this milestone.
    /// The FSM records the event and does not abort; the planner layer is responsible
    /// for acting on stale lock state once such a registry exists.
    #[default]
    ReleaseLocksAndContinue,
    /// Suspend progress and wait for reconnect for up to `max_ticks`.
    /// If the peer does not return before the timeout, abort the mission
    /// conservatively instead of continuing with ambiguous ownership.
    WaitForReconnect { max_ticks: u64 },
    /// Abort the current mission.
    AbortMission,
}

fn default_gcs_heartbeat_timeout() -> u64 {
    10
}

fn default_peer_heartbeat_timeout() -> u64 {
    15
}

/// Per-agent failsafe and lease-aware autonomy configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentAutonomyConfig {
    #[serde(default)]
    pub gcs_lost_policy: GcsLostPolicy,
    #[serde(default)]
    pub mothership_lost_policy: MothershipLostPolicy,
    #[serde(default)]
    pub neighbor_lost_policy: NeighborLostPolicy,
    /// Ticks without heartbeat before declaring GCS lost.
    #[serde(default = "default_gcs_heartbeat_timeout")]
    pub gcs_heartbeat_timeout_ticks: u64,
    /// Ticks without heartbeat before declaring a peer agent lost.
    #[serde(default = "default_peer_heartbeat_timeout")]
    pub peer_heartbeat_timeout_ticks: u64,
}

impl Default for AgentAutonomyConfig {
    fn default() -> Self {
        Self {
            gcs_lost_policy: GcsLostPolicy::default(),
            mothership_lost_policy: MothershipLostPolicy::default(),
            neighbor_lost_policy: NeighborLostPolicy::default(),
            gcs_heartbeat_timeout_ticks: default_gcs_heartbeat_timeout(),
            peer_heartbeat_timeout_ticks: default_peer_heartbeat_timeout(),
        }
    }
}

/// Summary of what an agent did while the GCS was unreachable.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateReconcileReport {
    pub agent_id: AgentId,
    /// Number of ticks the GCS was unavailable.
    pub gcs_lost_ticks: u64,
    /// Human-readable name of the policy that was active during the loss.
    pub policy_applied: String,
    /// Resource IDs completed while autonomous.
    pub completed_resources: Vec<String>,
    /// Leases that were still active when the GCS reconnected.
    pub active_leases_at_reconnect: Vec<Lease>,
    /// Mission FSM state at the moment of reconnection.
    pub mission_state_at_reconnect: AgentMissionState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gcs_lost_policy_round_trips() {
        let policies = [
            GcsLostPolicy::ContinueMission {
                max_gcs_lost_ticks: 20,
            },
            GcsLostPolicy::HoverInPlace {
                max_gcs_lost_ticks: 15,
            },
            GcsLostPolicy::ReturnToLaunch { after_ticks: 5 },
            GcsLostPolicy::AbortImmediate,
        ];
        for p in &policies {
            let json = serde_json::to_string(p).unwrap();
            let back: GcsLostPolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(p, &back);
        }
    }

    #[test]
    fn agent_autonomy_config_default_deserializes() {
        let json = "{}";
        let cfg: AgentAutonomyConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.gcs_heartbeat_timeout_ticks, 10);
        assert_eq!(cfg.peer_heartbeat_timeout_ticks, 15);
    }
}
