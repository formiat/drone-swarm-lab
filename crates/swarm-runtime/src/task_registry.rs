use std::collections::HashMap;

use swarm_types::{AgentId, Task, TaskId, TaskStatus};

use crate::RuntimeError;

pub struct TaskRegistry {
    /// key: `task_id`
    tasks: HashMap<TaskId, Task>,
}

impl TaskRegistry {
    pub fn new(tasks: Vec<Task>) -> Self {
        Self {
            tasks: tasks
                .into_iter()
                .map(|task| (task.id.clone(), task))
                .collect(),
        }
    }

    /// Insert a task dynamically (e.g. via dynamic injection at runtime).
    pub fn insert(&mut self, task: Task) {
        self.tasks.insert(task.id.clone(), task);
    }

    pub fn assign(&mut self, task_id: &TaskId, agent_id: AgentId) -> Result<(), RuntimeError> {
        let task = self.task_mut(task_id)?;
        if task.status != TaskStatus::Unassigned || task.assigned_to.is_some() {
            return Err(RuntimeError::InvalidTransition {
                from: task.status.clone(),
                to: TaskStatus::Assigned,
            });
        }
        task.status = TaskStatus::Assigned;
        task.assigned_to = Some(agent_id);
        Ok(())
    }

    pub fn start(&mut self, task_id: &TaskId) -> Result<(), RuntimeError> {
        self.transition(task_id, &[TaskStatus::Assigned], TaskStatus::InProgress)
    }

    pub fn complete(&mut self, task_id: &TaskId) -> Result<(), RuntimeError> {
        self.transition(task_id, &[TaskStatus::InProgress], TaskStatus::Completed)
    }

    pub fn fail_task(&mut self, task_id: &TaskId) -> Result<(), RuntimeError> {
        self.transition(
            task_id,
            &[TaskStatus::Assigned, TaskStatus::InProgress],
            TaskStatus::Failed,
        )
    }

    pub fn release_agent_tasks(&mut self, agent_id: &AgentId) -> Vec<TaskId> {
        let mut released = Vec::new();
        for task in self.tasks.values_mut() {
            if task.assigned_to.as_ref() == Some(agent_id)
                && matches!(task.status, TaskStatus::Assigned | TaskStatus::InProgress)
            {
                task.status = TaskStatus::Unassigned;
                task.assigned_to = None;
                tracing::info!(
                    task_id = %task.id,
                    agent_id = %agent_id,
                    "task released after agent failure"
                );
                released.push(task.id.clone());
            }
        }
        released
    }

    /// Release a single task back to unassigned. Returns the previous owner.
    pub fn release_task(&mut self, task_id: &TaskId) -> Option<AgentId> {
        let task = self.tasks.get_mut(task_id)?;
        if !matches!(task.status, TaskStatus::Assigned | TaskStatus::InProgress) {
            return None;
        }
        let prev = task.assigned_to.take();
        task.status = TaskStatus::Unassigned;
        prev
    }

    /// Remove tasks whose expires_at <= current_tick. Returns expired TaskIds.
    ///
    /// Expiration rule (Milestone 2): only Unassigned and Assigned tasks expire.
    /// InProgress tasks are never expired — the agent is actively working on them.
    pub fn expire_tasks(&mut self, current_tick: u64) -> Vec<TaskId> {
        let expired: Vec<TaskId> = self
            .tasks
            .values()
            .filter(|task| {
                task.expires_at.is_some_and(|t| t <= current_tick)
                    && task.status != TaskStatus::InProgress
            })
            .map(|task| task.id.clone())
            .collect();
        for id in &expired {
            tracing::info!(task_id = %id, "task expired");
            self.tasks.remove(id);
        }
        expired
    }

    pub fn unassigned(&self) -> Vec<&Task> {
        self.tasks
            .values()
            .filter(|task| task.status == TaskStatus::Unassigned || task.assigned_to.is_none())
            .collect()
    }

    pub fn all_assigned_or_completed(&self) -> bool {
        self.tasks.values().all(|task| {
            task.status == TaskStatus::Completed
                || (task.status != TaskStatus::Unassigned && task.assigned_to.is_some())
        })
    }

    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.tasks.values()
    }

    fn task_mut(&mut self, task_id: &TaskId) -> Result<&mut Task, RuntimeError> {
        self.tasks
            .get_mut(task_id)
            .ok_or_else(|| RuntimeError::TaskNotFound(task_id.clone()))
    }

    fn transition(
        &mut self,
        task_id: &TaskId,
        allowed_from: &[TaskStatus],
        to: TaskStatus,
    ) -> Result<(), RuntimeError> {
        let task = self.task_mut(task_id)?;
        if !allowed_from.contains(&task.status) {
            return Err(RuntimeError::InvalidTransition {
                from: task.status.clone(),
                to,
            });
        }
        task.status = to;
        Ok(())
    }
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
        }
    }

    fn task_expiring(id: &str, expires_at: u64) -> Task {
        Task {
            expires_at: Some(expires_at),
            ..task(id)
        }
    }

    #[test]
    fn registry_assign_unassigned() {
        let task_id = TaskId::from("task-0".to_owned());
        let mut registry = TaskRegistry::new(vec![task("task-0")]);

        registry
            .assign(&task_id, AgentId::from("agent-0".to_owned()))
            .unwrap();

        assert!(registry.all_assigned_or_completed());
    }

    #[test]
    fn registry_assign_already_assigned_fails() {
        let task_id = TaskId::from("task-0".to_owned());
        let mut registry = TaskRegistry::new(vec![task("task-0")]);
        registry
            .assign(&task_id, AgentId::from("agent-0".to_owned()))
            .unwrap();

        assert!(matches!(
            registry.assign(&task_id, AgentId::from("agent-1".to_owned())),
            Err(RuntimeError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn registry_start_assigned_task() {
        let task_id = TaskId::from("task-0".to_owned());
        let mut registry = TaskRegistry::new(vec![task("task-0")]);
        registry
            .assign(&task_id, AgentId::from("agent-0".to_owned()))
            .unwrap();

        registry.start(&task_id).unwrap();

        assert_eq!(
            registry.tasks().next().unwrap().status,
            TaskStatus::InProgress
        );
    }

    #[test]
    fn registry_release_agent_tasks() {
        let task_id = TaskId::from("task-0".to_owned());
        let agent_id = AgentId::from("agent-0".to_owned());
        let mut registry = TaskRegistry::new(vec![task("task-0")]);
        registry.assign(&task_id, agent_id.clone()).unwrap();

        let released = registry.release_agent_tasks(&agent_id);

        assert_eq!(released, vec![task_id]);
        assert_eq!(registry.unassigned().len(), 1);
    }

    #[test]
    fn registry_all_assigned_or_completed() {
        let task_id = TaskId::from("task-0".to_owned());
        let mut registry = TaskRegistry::new(vec![task("task-0")]);
        assert!(!registry.all_assigned_or_completed());

        registry
            .assign(&task_id, AgentId::from("agent-0".to_owned()))
            .unwrap();

        assert!(registry.all_assigned_or_completed());
    }

    #[test]
    fn task_registry_expire_at_tick() {
        let mut registry = TaskRegistry::new(vec![task_expiring("t0", 5)]);
        let expired = registry.expire_tasks(5);
        assert_eq!(expired, vec![TaskId::from("t0".to_owned())]);
        assert_eq!(registry.tasks().count(), 0);
    }

    #[test]
    fn task_registry_expire_keeps_not_due() {
        let mut registry = TaskRegistry::new(vec![task_expiring("t0", 10)]);
        let expired = registry.expire_tasks(5);
        assert!(expired.is_empty());
        assert_eq!(registry.tasks().count(), 1);
    }

    #[test]
    fn task_registry_expire_assigned_task() {
        let task_id = TaskId::from("t0".to_owned());
        let mut registry = TaskRegistry::new(vec![task_expiring("t0", 5)]);
        registry
            .assign(&task_id, AgentId::from("a0".to_owned()))
            .unwrap();

        let expired = registry.expire_tasks(5);
        assert_eq!(expired.len(), 1);
        assert_eq!(registry.tasks().count(), 0);
    }

    #[test]
    fn task_registry_expire_skips_in_progress() {
        let task_id = TaskId::from("t0".to_owned());
        let mut registry = TaskRegistry::new(vec![task_expiring("t0", 5)]);
        registry
            .assign(&task_id, AgentId::from("a0".to_owned()))
            .unwrap();
        registry.start(&task_id).unwrap();

        let expired = registry.expire_tasks(5);
        assert!(expired.is_empty());
        assert_eq!(registry.tasks().count(), 1);
    }

    #[test]
    fn task_registry_second_assign_returns_err() {
        let task_id = TaskId::from("t0".to_owned());
        let mut registry = TaskRegistry::new(vec![task("t0")]);
        registry
            .assign(&task_id, AgentId::from("a0".to_owned()))
            .unwrap();
        registry.start(&task_id).unwrap();

        assert!(matches!(
            registry.assign(&task_id, AgentId::from("a1".to_owned())),
            Err(RuntimeError::InvalidTransition { .. })
        ));
    }
}
