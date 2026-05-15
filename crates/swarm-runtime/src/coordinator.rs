use swarm_types::{Agent, AgentId, Task, TaskId};

use crate::{FailureDetector, MembershipView, TaskRegistry};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoordinatorOutput {
    pub newly_failed: Vec<AgentId>,
    pub released_tasks: Vec<TaskId>,
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

    pub fn process_tick(
        &mut self,
        heartbeat_senders: Vec<AgentId>,
        current_tick: u64,
    ) -> CoordinatorOutput {
        for agent_id in heartbeat_senders {
            self.membership.record_heartbeat(&agent_id, current_tick);
        }

        let newly_failed = self.detector.detect(&self.membership, current_tick);
        let mut released_tasks = Vec::new();

        for agent_id in &newly_failed {
            self.membership.mark_dead(agent_id);
            released_tasks.extend(self.registry.release_agent_tasks(agent_id));
        }

        CoordinatorOutput {
            newly_failed,
            released_tasks,
        }
    }
}
