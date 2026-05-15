use std::collections::HashMap;

use swarm_types::{AgentId, Task, TaskId, TaskStatus};

use crate::RuntimeError;

pub struct TaskRegistry {
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
                released.push(task.id.clone());
            }
        }
        released
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
}
