use super::super::*;

pub(in crate::runner) fn send_alive_heartbeats<T: Transport>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    current_tick: u64,
) {
    for (node, agent_id) in nodes {
        if crashed_agents.contains(agent_id) {
            continue;
        }
        let _ = node.send_heartbeats(current_tick);
    }
}

pub(in crate::runner) fn process_alive_nodes<T, A>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    current_tick: u64,
    allocator: &mut A,
    injected: &[Task],
) -> Vec<(AgentId, NodeTickOutput)>
where
    T: Transport,
    A: Allocator,
{
    let mut tick_outputs = Vec::new();
    for (node, agent_id) in nodes {
        if crashed_agents.contains(agent_id) {
            continue;
        }

        let output =
            match node.process_inbox_and_allocate(current_tick, allocator, injected.to_vec()) {
                Ok(out) => out,
                Err(_) => continue,
            };
        tick_outputs.push((agent_id.clone(), output));
    }
    tick_outputs
}
