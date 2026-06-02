use crate::message::RuntimeMessage;
use crate::Coordinator;

/// Apply ordered gossip state to the local coordinator.
///
/// value: `(merged_assignments, stale_assignments)`
pub(super) fn apply_gossip_messages(
    coordinator: &mut Coordinator,
    buffer: &[RuntimeMessage],
) -> (u64, u64) {
    let mut merged: u64 = 0;
    let mut stale: u64 = 0;

    for msg in buffer {
        if let RuntimeMessage::Gossip {
            assignments,
            generations,
        } = msg
        {
            let mut ordered_assignments: Vec<_> = assignments.iter().collect();
            ordered_assignments
                .sort_by(|(left_id, _), (right_id, _)| left_id.as_ref().cmp(right_id.as_ref()));
            for (task_id, remote_agent_id) in ordered_assignments {
                let local_owner = coordinator
                    .registry
                    .tasks()
                    .find(|task| &task.id == task_id)
                    .and_then(|task| task.assigned_to.clone());

                match local_owner {
                    None => {
                        if coordinator.membership.is_alive(remote_agent_id) {
                            let _ = coordinator
                                .registry
                                .assign(task_id, remote_agent_id.clone());
                            merged += 1;
                        } else {
                            stale += 1;
                        }
                    }
                    Some(ref local_id) if local_id == remote_agent_id => {}
                    Some(ref local_id) => {
                        if !coordinator.membership.is_alive(remote_agent_id) {
                            stale += 1;
                            continue;
                        }

                        let local_generation = coordinator.membership.generation_of(local_id);
                        let remote_generation =
                            generations.get(remote_agent_id).copied().unwrap_or(1);

                        if remote_generation > local_generation
                            || (remote_generation == local_generation
                                && remote_agent_id.as_ref() > local_id.as_ref())
                        {
                            coordinator.registry.release_task(task_id);
                            let _ = coordinator
                                .registry
                                .assign(task_id, remote_agent_id.clone());
                            merged += 1;
                        } else {
                            stale += 1;
                        }
                    }
                }
            }

            let mut ordered_generations: Vec<_> = generations.iter().collect();
            ordered_generations
                .sort_by(|(left_id, _), (right_id, _)| left_id.as_ref().cmp(right_id.as_ref()));
            for (agent_id, remote_generation) in ordered_generations {
                let local_generation = coordinator.membership.generation_of(agent_id);
                if *remote_generation > local_generation {
                    coordinator
                        .membership
                        .record_heartbeat(agent_id, 0, *remote_generation);
                }
            }
        }
    }

    (merged, stale)
}
