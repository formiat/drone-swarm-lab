use swarm_types::{TaskId, TaskStatus};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    #[error("task not found: {0:?}")]
    TaskNotFound(TaskId),
    #[error("invalid state transition from {from:?} to {to:?}")]
    InvalidTransition { from: TaskStatus, to: TaskStatus },
}
