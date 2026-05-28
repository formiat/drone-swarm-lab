use crate::allocation::AllocationAgent;
use crate::mission::{MissionAdapter, RunState};
use crate::pose::Pose;
use crate::task::{Task, TaskKind};

/// Adapter for coverage missions (grid cell coverage).
#[derive(Clone, Debug, Default)]
pub struct CoverageAdapter;

impl MissionAdapter for CoverageAdapter {
    fn task_kind(&self, _task: &Task) -> TaskKind {
        TaskKind::CoverageCell
    }

    fn route_cost(&self, from: Pose, task: &Task) -> f64 {
        let to = task.pose.unwrap_or_default();
        from.distance_to_2d(&to)
    }

    fn is_completed(&self, task: &Task, state: &RunState) -> bool {
        state.completed_tasks.contains(&task.id)
    }

    fn score(&self, agent: &AllocationAgent, task: &Task) -> f64 {
        let distance = self.route_cost(agent.pose, task);
        let battery_factor = agent.battery / 100.0;
        // Higher score is better: closer, more battery
        1000.0 - distance + battery_factor * 100.0
    }
}

/// Adapter for SAR missions (search and rescue grid scans).
#[derive(Clone, Debug, Default)]
pub struct SarAdapter;

impl MissionAdapter for SarAdapter {
    fn task_kind(&self, _task: &Task) -> TaskKind {
        TaskKind::SarScan
    }

    fn route_cost(&self, from: Pose, task: &Task) -> f64 {
        let to = task.pose.unwrap_or_default();
        from.distance_to_2d(&to)
    }

    fn is_completed(&self, task: &Task, state: &RunState) -> bool {
        task.grid_cell
            .map(|cell| state.scanned_cells.contains(&cell))
            .unwrap_or(false)
    }

    fn score(&self, agent: &AllocationAgent, task: &Task) -> f64 {
        let distance = self.route_cost(agent.pose, task);
        let battery_factor = agent.battery / 100.0;
        // SAR: prioritize proximity and battery
        1000.0 - distance + battery_factor * 50.0 + f64::from(task.priority) * 10.0
    }
}

/// Adapter for infrastructure inspection missions (edge coverage).
#[derive(Clone, Debug, Default)]
pub struct InspectionAdapter;

impl MissionAdapter for InspectionAdapter {
    fn task_kind(&self, _task: &Task) -> TaskKind {
        TaskKind::InspectionEdge
    }

    fn route_cost(&self, from: Pose, task: &Task) -> f64 {
        let to = task.pose.unwrap_or_default();
        from.distance_to_2d(&to)
    }

    fn is_completed(&self, task: &Task, state: &RunState) -> bool {
        task.edge_id
            .as_ref()
            .map(|eid| state.covered_edges.contains(eid))
            .unwrap_or(false)
    }

    fn score(&self, agent: &AllocationAgent, task: &Task) -> f64 {
        let distance = self.route_cost(agent.pose, task);
        let battery_factor = agent.battery / 100.0;
        // Inspection: balance distance, battery, and priority
        1000.0 - distance + battery_factor * 50.0 + f64::from(task.priority) * 5.0
    }
}

/// Adapter for wildfire / flood mapping missions (zone mapping).
#[derive(Clone, Debug, Default)]
pub struct WildfireAdapter;

impl MissionAdapter for WildfireAdapter {
    fn task_kind(&self, _task: &Task) -> TaskKind {
        TaskKind::MappingZone
    }

    fn route_cost(&self, from: Pose, task: &Task) -> f64 {
        let to = task.pose.unwrap_or_default();
        from.distance_to_2d(&to)
    }

    fn is_completed(&self, task: &Task, state: &RunState) -> bool {
        state.mapped_zones.contains(&task.id.to_string())
    }

    fn score(&self, agent: &AllocationAgent, task: &Task) -> f64 {
        let distance = self.route_cost(agent.pose, task);
        let battery_factor = agent.battery / 100.0;
        // Wildfire: prioritize high-priority zones and proximity
        // v0.38: threat urgency bonus for critically high priority
        let threat_urgency = if task.priority >= 8 { 200.0 } else { 0.0 };
        1000.0 - distance + battery_factor * 50.0 + f64::from(task.priority) * 20.0 + threat_urgency
    }
}

/// Adapter for emergency mesh relay placement.
#[derive(Clone, Debug, Default)]
pub struct RelayAdapter;

impl MissionAdapter for RelayAdapter {
    fn task_kind(&self, _task: &Task) -> TaskKind {
        TaskKind::RelayPlacement
    }

    fn route_cost(&self, from: Pose, task: &Task) -> f64 {
        let to = task.pose.unwrap_or_default();
        from.distance_to_2d(&to)
    }

    fn is_completed(&self, task: &Task, state: &RunState) -> bool {
        state.completed_tasks.contains(&task.id)
    }

    fn score(&self, agent: &AllocationAgent, task: &Task) -> f64 {
        let distance = self.route_cost(agent.pose, task);
        let battery_factor = agent.battery / 100.0;
        1000.0 - distance + battery_factor * 50.0
    }
}

/// Adapter for waypoint navigation (SITL).
#[derive(Clone, Debug, Default)]
pub struct WaypointAdapter;

impl MissionAdapter for WaypointAdapter {
    fn task_kind(&self, _task: &Task) -> TaskKind {
        TaskKind::Waypoint
    }

    fn route_cost(&self, from: Pose, task: &Task) -> f64 {
        let to = task.pose.unwrap_or_default();
        from.distance_to_2d(&to)
    }

    fn is_completed(&self, task: &Task, state: &RunState) -> bool {
        state.completed_tasks.contains(&task.id)
    }

    fn score(&self, agent: &AllocationAgent, task: &Task) -> f64 {
        let distance = self.route_cost(agent.pose, task);
        let battery_factor = agent.battery / 100.0;
        1000.0 - distance + battery_factor * 50.0
    }
}

/// Registry that maps TaskKind to the appropriate adapter.
///
/// Used by the runner and allocator to look up the correct semantic layer
/// for a task without hard-coding the mapping at every call site.
#[derive(Clone, Debug, Default)]
pub struct AdapterRegistry {
    coverage: CoverageAdapter,
    sar: SarAdapter,
    inspection: InspectionAdapter,
    wildfire: WildfireAdapter,
    relay: RelayAdapter,
    waypoint: WaypointAdapter,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the adapter for the given task kind.
    pub fn get(&self, kind: &TaskKind) -> &dyn MissionAdapter {
        match kind {
            TaskKind::CoverageCell => &self.coverage,
            TaskKind::SarScan | TaskKind::SarConfirmationScan => &self.sar,
            TaskKind::InspectionEdge => &self.inspection,
            TaskKind::MappingZone => &self.wildfire,
            TaskKind::RelayPlacement => &self.relay,
            TaskKind::Waypoint => &self.waypoint,
        }
    }

    /// Convenience: look up adapter by task's optional kind.
    /// Returns `None` if the task has no kind.
    pub fn for_task(&self, task: &Task) -> Option<&dyn MissionAdapter> {
        task.kind.as_ref().map(|k| self.get(k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentId, Role};
    use crate::allocation::AllocationAgent;
    use crate::edge::EdgeId;
    use crate::pose::Pose;
    use crate::task::{Task, TaskId, TaskKind, TaskStatus};

    fn task(id: &str, kind: TaskKind) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(Pose {
                x: 10.0,
                y: 10.0,
                z: 0.0,
            }),
            grid_cell: None,
            edge_id: None,
            kind: Some(kind),
        }
    }

    fn agent_at(x: f64, y: f64) -> AllocationAgent {
        AllocationAgent {
            id: AgentId::from("a0".to_owned()),
            pose: Pose { x, y, z: 0.0 },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        }
    }

    #[test]
    fn coverage_adapter_task_kind() {
        let adapter = CoverageAdapter;
        let t = task("t0", TaskKind::CoverageCell);
        assert_eq!(adapter.task_kind(&t), TaskKind::CoverageCell);
    }

    #[test]
    fn sar_adapter_task_kind() {
        let adapter = SarAdapter;
        let t = task("t0", TaskKind::SarScan);
        assert_eq!(adapter.task_kind(&t), TaskKind::SarScan);
    }

    #[test]
    fn inspection_adapter_task_kind() {
        let adapter = InspectionAdapter;
        let t = task("t0", TaskKind::InspectionEdge);
        assert_eq!(adapter.task_kind(&t), TaskKind::InspectionEdge);
    }

    #[test]
    fn wildfire_adapter_task_kind() {
        let adapter = WildfireAdapter;
        let t = task("t0", TaskKind::MappingZone);
        assert_eq!(adapter.task_kind(&t), TaskKind::MappingZone);
    }

    #[test]
    fn sar_adapter_is_completed_when_scanned() {
        let adapter = SarAdapter;
        let mut t = task("t0", TaskKind::SarScan);
        t.grid_cell = Some((5, 5));

        let mut state = RunState::default();
        assert!(!adapter.is_completed(&t, &state));

        state.scanned_cells.insert((5, 5));
        assert!(adapter.is_completed(&t, &state));
    }

    #[test]
    fn inspection_adapter_is_completed_when_covered() {
        let adapter = InspectionAdapter;
        let mut t = task("t0", TaskKind::InspectionEdge);
        t.edge_id = Some(EdgeId::from("e0".to_owned()));

        let mut state = RunState::default();
        assert!(!adapter.is_completed(&t, &state));

        state.covered_edges.insert(EdgeId::from("e0".to_owned()));
        assert!(adapter.is_completed(&t, &state));
    }

    #[test]
    fn wildfire_adapter_is_completed_when_mapped() {
        let adapter = WildfireAdapter;
        let t = task("zone-0", TaskKind::MappingZone);

        let mut state = RunState::default();
        assert!(!adapter.is_completed(&t, &state));

        state.mapped_zones.insert("zone-0".to_owned());
        assert!(adapter.is_completed(&t, &state));
    }

    #[test]
    fn coverage_adapter_is_completed_when_in_completed_tasks() {
        let adapter = CoverageAdapter;
        let t = task("t0", TaskKind::CoverageCell);

        let mut state = RunState::default();
        assert!(!adapter.is_completed(&t, &state));

        state.completed_tasks.insert(TaskId::from("t0".to_owned()));
        assert!(adapter.is_completed(&t, &state));
    }

    #[test]
    fn adapter_registry_lookup() {
        let registry = AdapterRegistry::new();
        let t = task("t0", TaskKind::SarScan);
        let adapter = registry.for_task(&t).unwrap();
        assert_eq!(adapter.task_kind(&t), TaskKind::SarScan);
    }

    #[test]
    fn adapter_registry_none_for_missing_kind() {
        let registry = AdapterRegistry::new();
        let t = Task {
            id: TaskId::from("t0".to_owned()),
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
        };
        assert!(registry.for_task(&t).is_none());
    }

    #[test]
    fn sar_adapter_score_prefers_closer_agent() {
        let adapter = SarAdapter;
        let t = task("t0", TaskKind::SarScan);
        let close = agent_at(9.0, 10.0);
        let far = agent_at(100.0, 10.0);
        assert!(adapter.score(&close, &t) > adapter.score(&far, &t));
    }

    #[test]
    fn wildfire_adapter_score_prefers_higher_priority() {
        let adapter = WildfireAdapter;
        let mut low = task("low", TaskKind::MappingZone);
        low.priority = 1;
        let mut high = task("high", TaskKind::MappingZone);
        high.priority = 10;
        let agent = agent_at(0.0, 0.0);
        assert!(adapter.score(&agent, &high) > adapter.score(&agent, &low));
    }
}
