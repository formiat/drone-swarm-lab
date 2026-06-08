use swarm_comms::{InMemAgentTransport, RawMessage};
use swarm_runtime::RuntimeMessage;

use super::super::*;

/// Injects a synthetic GCS heartbeat from `base_id` into the shared bus for
/// every alive agent node. The network partition mechanism will silently drop
/// the message when `base_id` is partitioned from a particular agent, which
/// is the mechanism used to simulate GCS loss.
pub(in crate::runner) fn send_gcs_heartbeats(
    bus: &Rc<RefCell<InMemNetwork>>,
    nodes: &[(AgentNode<InMemAgentTransport>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    base_id: &AgentId,
    current_tick: u64,
) {
    let payload = RuntimeMessage::heartbeat(current_tick, 1);
    for (_, agent_id) in nodes {
        if crashed_agents.contains(agent_id) {
            continue;
        }
        let msg = RawMessage {
            from: base_id.clone(),
            to: agent_id.clone(),
            payload: payload.clone(),
        };
        let _ = bus.borrow_mut().send(msg);
    }
}

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
