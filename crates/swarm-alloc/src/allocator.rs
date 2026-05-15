use swarm_types::{AgentId, Task, TaskId};

pub trait Allocator {
    fn allocate(&self, tasks: &[&Task], agents: &[&AgentId]) -> Vec<(TaskId, AgentId)>;
}

pub struct GreedyAllocator;

impl Allocator for GreedyAllocator {
    fn allocate(&self, tasks: &[&Task], agents: &[&AgentId]) -> Vec<(TaskId, AgentId)> {
        if agents.is_empty() {
            return Vec::new();
        }

        let mut ordered_tasks = tasks.to_vec();
        ordered_tasks.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
        });

        ordered_tasks
            .into_iter()
            .enumerate()
            .map(|(index, task)| (task.id.clone(), (*agents[index % agents.len()]).clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::TaskStatus;

    fn task(id: &str, priority: u8) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority,
        }
    }

    #[test]
    fn greedy_assigns_to_alive_agents() {
        let tasks = [task("task-0", 1), task("task-1", 1), task("task-2", 1)];
        let task_refs: Vec<_> = tasks.iter().collect();
        let agents = [
            AgentId::from("agent-0".to_owned()),
            AgentId::from("agent-1".to_owned()),
            AgentId::from("agent-2".to_owned()),
        ];
        let agent_refs: Vec<_> = agents.iter().collect();

        let assignments = GreedyAllocator.allocate(&task_refs, &agent_refs);

        assert_eq!(assignments.len(), 3);
    }

    #[test]
    fn greedy_no_agents_returns_empty() {
        let tasks = [task("task-0", 1)];
        let task_refs: Vec<_> = tasks.iter().collect();

        let assignments = GreedyAllocator.allocate(&task_refs, &[]);

        assert!(assignments.is_empty());
    }

    #[test]
    fn greedy_more_tasks_than_agents() {
        let tasks = [
            task("task-0", 1),
            task("task-1", 1),
            task("task-2", 1),
            task("task-3", 1),
            task("task-4", 1),
        ];
        let task_refs: Vec<_> = tasks.iter().collect();
        let agents = [
            AgentId::from("agent-0".to_owned()),
            AgentId::from("agent-1".to_owned()),
        ];
        let agent_refs: Vec<_> = agents.iter().collect();

        let assignments = GreedyAllocator.allocate(&task_refs, &agent_refs);

        assert_eq!(assignments.len(), 5);
        assert_eq!(assignments[0].1, agents[0]);
        assert_eq!(assignments[1].1, agents[1]);
        assert_eq!(assignments[2].1, agents[0]);
    }
}
