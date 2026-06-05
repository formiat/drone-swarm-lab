use std::collections::{HashMap, HashSet};

use swarm_types::AgentId;

use crate::types::{PartialSuccessPolicy, SynchronizedCommandResult, SynchronizedCommandWindow};

/// Scripted fake outcome for one agent inside a synchronized command window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncAgentOutcome {
    Succeeded,
    Failed,
    TimedOut,
}

/// Evaluate a synchronized command window against deterministic fake outcomes.
pub fn evaluate_synchronized_command(
    window: &SynchronizedCommandWindow,
    outcomes: &HashMap<AgentId, SyncAgentOutcome>,
) -> SynchronizedCommandResult {
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    let mut timed_out = Vec::new();
    let mut seen = HashSet::new();

    for agent_id in &window.agent_ids {
        if !seen.insert(agent_id.clone()) {
            continue;
        }
        match outcomes
            .get(agent_id)
            .copied()
            .unwrap_or(SyncAgentOutcome::Succeeded)
        {
            SyncAgentOutcome::Succeeded => succeeded.push(agent_id.clone()),
            SyncAgentOutcome::Failed => failed.push(agent_id.clone()),
            SyncAgentOutcome::TimedOut => timed_out.push(agent_id.clone()),
        }
    }

    let accepted = match window.partial_success_policy {
        PartialSuccessPolicy::RequireAll => {
            succeeded.len() == seen.len() && failed.is_empty() && timed_out.is_empty()
        }
        PartialSuccessPolicy::AtLeast { agents } => succeeded.len() >= agents,
    };

    SynchronizedCommandResult {
        kind: window.kind.clone(),
        succeeded,
        failed,
        timed_out,
        accepted,
    }
}
