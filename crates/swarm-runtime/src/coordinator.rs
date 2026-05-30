use swarm_types::{Agent, AgentId, Task, TaskId};

use crate::{FailureDetector, MembershipView, TaskRegistry};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureRelease {
    pub failed_agent_id: AgentId,
    pub released_tasks: Vec<TaskId>,
    pub detected_at_tick: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoordinatorOutput {
    pub newly_failed: Vec<AgentId>,
    pub failure_releases: Vec<FailureRelease>,
    pub released_tasks: Vec<TaskId>,
    pub expired_task_ids: Vec<TaskId>,
}

pub struct Coordinator {
    pub membership: MembershipView,
    pub detector: FailureDetector,
    pub registry: TaskRegistry,
}

impl Coordinator {
    pub fn new(agents: Vec<Agent>, tasks: Vec<Task>, timeout_ticks: u64) -> Self {
        Self {
            membership: MembershipView::new(agents),
            detector: FailureDetector::new(timeout_ticks),
            registry: TaskRegistry::new(tasks),
        }
    }

    /// Add a task dynamically at runtime.
    pub fn inject_task(&mut self, task: Task) {
        self.registry.insert(task);
    }

    pub fn process_tick(
        &mut self,
        _heartbeat_senders: Vec<AgentId>,
        current_tick: u64,
        injected_tasks: Vec<Task>,
    ) -> CoordinatorOutput {
        tracing::debug!(tick = current_tick, "coordinator tick");
        for task in injected_tasks {
            self.registry.insert(task);
        }

        // Heartbeats are already recorded via MembershipView::record_heartbeat
        // by the dispatch loop in AgentNode. Coordinator only detects failures
        // and expires tasks.

        let newly_failed = self.detector.detect(&self.membership, current_tick);
        let mut released_tasks = Vec::new();
        let mut failure_releases = Vec::new();

        for agent_id in &newly_failed {
            self.membership.mark_dead(agent_id);
            let released = self.registry.release_agent_tasks(agent_id);
            released_tasks.extend(released.iter().cloned());
            failure_releases.push(FailureRelease {
                failed_agent_id: agent_id.clone(),
                released_tasks: released,
                detected_at_tick: current_tick,
            });
        }

        let expired_task_ids = self.registry.expire_tasks(current_tick);

        CoordinatorOutput {
            newly_failed,
            failure_releases,
            released_tasks,
            expired_task_ids,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::{Health, Pose, Role, TaskStatus};

    fn agent(id: &str) -> Agent {
        Agent {
            id: AgentId::from(id.to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            capabilities: vec![],
            current_task: None,
            battery: 100.0,
            comms_range: f64::INFINITY,
            generation: 1,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
            battery_model: None,
        }
    }

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
    fn coordinator_inject_task() {
        let mut coord = Coordinator::new(vec![agent("a0")], vec![], 5);
        coord.inject_task(task("t0"));
        assert_eq!(coord.registry.unassigned().len(), 1);
    }

    #[test]
    fn coordinator_process_tick_injects() {
        let mut coord = Coordinator::new(vec![agent("a0")], vec![], 5);
        coord.process_tick(vec![], 1, vec![task("t0")]);
        assert_eq!(coord.registry.unassigned().len(), 1);
    }

    #[test]
    fn coordinator_output_has_expired_ids() {
        let mut t = task("t0");
        t.expires_at = Some(1);
        let mut coord = Coordinator::new(vec![agent("a0")], vec![t], 5);
        let out = coord.process_tick(vec![], 1, vec![]);
        assert_eq!(out.expired_task_ids, vec![TaskId::from("t0".to_owned())]);
    }

    #[test]
    fn coordinator_output_links_failed_agent_to_released_tasks() {
        let survivor = AgentId::from("a0".to_owned());
        let failed = AgentId::from("a1".to_owned());
        let task_id = TaskId::from("t0".to_owned());
        let mut coord = Coordinator::new(vec![agent("a0"), agent("a1")], vec![task("t0")], 3);
        coord.membership.record_heartbeat(&survivor, 4, 1);
        coord.registry.assign(&task_id, failed.clone()).unwrap();

        let out = coord.process_tick(vec![survivor], 4, vec![]);

        assert_eq!(out.newly_failed, vec![failed.clone()]);
        assert_eq!(out.released_tasks, vec![task_id.clone()]);
        assert_eq!(
            out.failure_releases,
            vec![FailureRelease {
                failed_agent_id: failed,
                released_tasks: vec![task_id],
                detected_at_tick: 4,
            }]
        );
    }
}
