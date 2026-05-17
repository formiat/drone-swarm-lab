use crate::event_log::{Event, EventLog};
use swarm_types::{AgentId, TaskId};

/// Minimal replay state that reconstructs the system from an event log.
///
/// The replay engine does not re-run the simulation; it reconstructs
/// the final state by applying events in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplayState {
    pub failed_agents: Vec<(AgentId, u64)>,
    pub assigned_tasks: Vec<(TaskId, AgentId, u64)>,
    pub messages_sent: u64,
    pub messages_dropped: u64,
    pub partition_events: u64,
    pub final_poses: Vec<(AgentId, swarm_types::Pose)>,
}

/// Replay an event log and produce the final reconstructed state.
pub fn replay(log: &EventLog) -> ReplayState {
    let mut state = ReplayState::default();

    for event in &log.events {
        match event {
            Event::AgentFailed { agent_id, tick } => {
                state.failed_agents.push((agent_id.clone(), *tick));
            }
            Event::TaskAssigned {
                task_id,
                agent_id,
                tick,
            } => {
                state
                    .assigned_tasks
                    .push((task_id.clone(), agent_id.clone(), *tick));
            }
            Event::MessageSent { .. } => {
                state.messages_sent += 1;
            }
            Event::MessageDropped { .. } => {
                state.messages_dropped += 1;
            }
            Event::PartitionAdded { .. } | Event::PartitionRemoved { .. } => {
                state.partition_events += 1;
            }
            Event::PoseUpdated { agent_id, pose, .. } => {
                // Overwrite previous pose for this agent
                if let Some(entry) = state.final_poses.iter_mut().find(|(id, _)| id == agent_id) {
                    entry.1 = *pose;
                } else {
                    state.final_poses.push((agent_id.clone(), *pose));
                }
            }
            Event::TickStart { .. } => {}
        }
    }

    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_log::EventLogBuilder;
    use swarm_types::{AgentId, TaskId};

    #[test]
    fn replay_reconstructs_state() {
        let mut builder = EventLogBuilder::new("test", 0, "scenario");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::MessageSent {
            from: AgentId::from("a0".to_owned()),
            to: AgentId::from("a1".to_owned()),
            tick: 1,
            payload_len: 10,
        });
        builder.push(Event::TaskAssigned {
            task_id: TaskId::from("t0".to_owned()),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 2,
        });
        builder.push(Event::AgentFailed {
            agent_id: AgentId::from("a1".to_owned()),
            tick: 3,
        });

        let log = builder.build();
        let state = replay(&log);

        assert_eq!(state.messages_sent, 1);
        assert_eq!(state.assigned_tasks.len(), 1);
        assert_eq!(state.failed_agents.len(), 1);
    }
}
