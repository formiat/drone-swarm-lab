use derive_more::{AsRef, Deref, DerefMut, Display, From, Into};
use serde::{Deserialize, Serialize};

use crate::agent::{AgentId, Capability, Role};
use crate::pose::Pose;

/// Unique identifier for a mission task.
#[derive(
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct TaskId(String);

/// Lifecycle status of a task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Unassigned,
    Assigned,
    InProgress,
    Completed,
    Failed,
}

/// Semantic kind of a mission task.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    CoverageCell,
    SarScan,
    SarConfirmationScan,
    InspectionEdge,
    RelayPlacement,
    Waypoint,
}

/// A unit of work to be executed by an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub status: TaskStatus,
    pub assigned_to: Option<AgentId>,
    pub priority: u8,
    /// Hard constraint: agent must hold all listed capabilities to be assigned this task.
    pub required_capabilities: Vec<Capability>,
    /// Hard constraint: agent must have this role to be assigned this task.
    #[serde(default)]
    pub required_role: Option<Role>,
    /// Soft constraint: agent matching this role gets a cost bonus.
    pub preferred_role: Option<Role>,
    /// Task expires (is removed) when the simulation tick reaches this value.
    pub expires_at: Option<u64>,
    /// Geographic position of the task used in the distance cost function.
    pub pose: Option<Pose>,
    /// If set, this task represents scanning a specific grid cell.
    #[serde(default)]
    pub grid_cell: Option<(u32, u32)>,
    /// If set, this task represents inspecting a specific edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id: Option<crate::edge::EdgeId>,
    /// Semantic kind of the task (e.g. SAR scan, inspection edge, waypoint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<TaskKind>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
            grid_cell: None,
            edge_id: None,
            kind: None,
        }
    }

    #[test]
    fn task_id_newtype_roundtrip() {
        let id = TaskId::from("task-1".to_owned());
        assert_eq!(*id, "task-1");
    }

    #[test]
    fn task_status_serde_snake_case() {
        let json = serde_json::to_string(&TaskStatus::Unassigned).unwrap();
        assert_eq!(json, r#""unassigned""#);
    }

    #[test]
    fn task_required_capabilities_serde() {
        let mut t = task("t");
        t.required_capabilities = vec![Capability::from("thermal".to_owned())];
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("thermal"));
    }

    #[test]
    fn task_expires_at_serde() {
        let mut t = task("t");
        t.expires_at = Some(42);
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("42"));
    }
}
