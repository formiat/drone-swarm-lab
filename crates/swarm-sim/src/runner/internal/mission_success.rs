#[derive(Clone, Debug)]
pub(in crate::runner) struct MissionStopSnapshot {
    pub all_tasks_assigned: bool,
    pub all_failure_ticks_passed: bool,
    pub all_expected_failures_detected: bool,
    pub all_dynamic_tasks_injected: bool,
    pub post_partition_converged: bool,
    pub sar_complete: bool,
    pub inspection_complete: bool,
    pub adapter_complete: bool,
}

pub(in crate::runner) fn should_stop_tick(
    snapshot: MissionStopSnapshot,
    max_task_unassigned_ticks: u64,
    max_unassigned_ticks: u64,
) -> bool {
    snapshot.all_tasks_assigned
        && max_task_unassigned_ticks <= max_unassigned_ticks
        && snapshot.all_failure_ticks_passed
        && snapshot.all_expected_failures_detected
        && snapshot.all_dynamic_tasks_injected
        && snapshot.post_partition_converged
        && snapshot.sar_complete
        && snapshot.inspection_complete
        && snapshot.adapter_complete
}
