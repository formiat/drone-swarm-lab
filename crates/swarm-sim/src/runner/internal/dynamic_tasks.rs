use super::super::*;

pub(in crate::runner) fn tasks_injected_at_tick(
    dynamic_tasks: &[DynamicTaskEvent],
    current_tick: u64,
) -> Vec<Task> {
    dynamic_tasks
        .iter()
        .filter(|event| event.at_tick == current_tick)
        .map(|event| event.task.clone())
        .collect()
}

pub(in crate::runner) fn all_dynamic_tasks_injected(
    dynamic_tasks: &[DynamicTaskEvent],
    current_tick: u64,
) -> bool {
    dynamic_tasks
        .iter()
        .all(|event| current_tick >= event.at_tick)
}
