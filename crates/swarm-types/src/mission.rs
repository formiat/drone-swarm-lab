use std::collections::HashSet;

use crate::allocation::AllocationAgent;
use crate::edge::EdgeId;
use crate::pose::Pose;
use crate::task::{Task, TaskId, TaskKind};

/// Lightweight execution state for mission-progress checks.
///
/// Aggregates only the information needed by `MissionAdapter::is_completed`.
/// Runtime-specific types (`GridState`, `InspectionState`) are converted into
/// this form before calling the adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunState {
    /// Cells that have been scanned (SAR).
    pub scanned_cells: HashSet<(u32, u32)>,
    /// Edges that have been covered (Inspection).
    pub covered_edges: HashSet<EdgeId>,
    /// Tasks marked as completed (Coverage, Waypoint).
    pub completed_tasks: HashSet<TaskId>,
}

/// Mission-specific adapter that provides scoring, routing and completion
/// semantics for tasks of a particular kind.
///
/// Adapters are thread-safe (`Send + Sync`) so they can be stored inside
/// allocator structures.
pub trait MissionAdapter: Send + Sync {
    /// Returns the `TaskKind` that this adapter handles.
    fn task_kind(&self, task: &Task) -> TaskKind;

    /// Cost of moving from `from` to the location of `task`.
    fn route_cost(&self, from: Pose, task: &Task) -> f64;

    /// Whether `task` is considered completed given the current run state.
    fn is_completed(&self, task: &Task, state: &RunState) -> bool;

    /// Score of assigning `task` to `agent`. Higher is better.
    fn score(&self, agent: &AllocationAgent, task: &Task) -> f64;
}
