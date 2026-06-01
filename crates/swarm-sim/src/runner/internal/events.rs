use super::super::*;

pub(in crate::runner) fn record_tick_start(
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
    current_tick: u64,
) {
    if let Some(builder) = log_builder {
        builder.push(swarm_replay::Event::TickStart { tick: current_tick });
    }
}

pub(in crate::runner) fn record_agent_failures(
    failures: &[FailureEvent],
    current_tick: u64,
    crashed_agents: &mut HashSet<AgentId>,
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
) {
    for failure in failures
        .iter()
        .filter(|failure| failure.at_tick == current_tick)
    {
        crashed_agents.insert(failure.agent_id.clone());
        if let Some(builder) = log_builder {
            builder.push(swarm_replay::Event::AgentFailed {
                agent_id: failure.agent_id.clone(),
                tick: current_tick,
            });
        }
    }
}

pub(in crate::runner) fn record_final_poses<T: Transport>(
    nodes: &[(AgentNode<T>, AgentId)],
    total_ticks: u64,
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
) {
    if let Some(builder) = log_builder {
        for (node, agent_id) in nodes {
            if let Some(entry) = node.coordinator.membership.get(agent_id) {
                builder.push(swarm_replay::Event::PoseUpdated {
                    agent_id: agent_id.clone(),
                    pose: entry.pose,
                    tick: total_ticks,
                });
            }
        }
    }
}

pub(in crate::runner) fn all_failure_ticks_passed(
    failures: &[FailureEvent],
    current_tick: u64,
) -> bool {
    failures
        .iter()
        .all(|failure| current_tick >= failure.at_tick)
}

pub(in crate::runner) fn all_partitions_resolved(
    partition_events: &[PartitionEvent],
    current_tick: u64,
) -> bool {
    partition_events.iter().all(|event| {
        event
            .until_tick
            .is_some_and(|until_tick| current_tick >= until_tick)
    })
}
