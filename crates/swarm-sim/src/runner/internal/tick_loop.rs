use super::super::*;
use std::collections::{HashMap, HashSet};

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

pub(in crate::runner) fn update_view_divergence<T: Transport>(
    nodes: &[(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    current_tick: u64,
    heal_tick: Option<u64>,
    max_view_divergence: &mut u64,
    convergence_ticks: &mut Option<u64>,
) {
    let maps: Vec<HashMap<TaskId, AgentId>> = nodes
        .iter()
        .filter(|(_, id)| !crashed_agents.contains(id))
        .map(|(node, _)| {
            node.coordinator
                .registry
                .tasks()
                .filter_map(|task| {
                    task.assigned_to
                        .clone()
                        .map(|agent_id| (task.id.clone(), agent_id))
                })
                .collect::<HashMap<_, _>>()
        })
        .collect();
    if maps.is_empty() {
        return;
    }

    let reference = &maps[0];
    let diverged = maps.iter().filter(|map| *map != reference).count() as u64;
    *max_view_divergence = (*max_view_divergence).max(diverged);

    if let Some(heal_at) = heal_tick {
        if current_tick > heal_at && diverged == 0 && convergence_ticks.is_none() {
            *convergence_ticks = Some(current_tick - heal_at);
        }
    }
}
