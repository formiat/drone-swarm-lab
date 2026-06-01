use super::super::*;

pub(in crate::runner) fn advance_tick(clock: &mut Clock) -> u64 {
    clock.advance();
    u64::from(clock.now())
}

pub(in crate::runner) fn first_active_agent_id<T: Transport>(
    nodes: &[(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
) -> Option<AgentId> {
    nodes
        .iter()
        .find(|(_, id)| !crashed_agents.contains(id))
        .map(|(_, id)| id.clone())
}
