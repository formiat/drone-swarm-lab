use derive_more::{AsRef, Deref, DerefMut, Display, From, Into};
use serde::{Deserialize, Serialize};

use crate::agent::AgentId;

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

/// A unit of work to be executed by an agent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub status: TaskStatus,
    pub assigned_to: Option<AgentId>,
    pub priority: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
