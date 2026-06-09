use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use swarm_comms::{
    AgentAbsenceKind, ConflictResolution, ConnectivityLossKind, DegradedDecisionLog, Lease,
    LeaseTickSnapshot, OwnershipConflict, PartitionReport, ReconciliationReport,
    SupervisorDecision, SupervisorReconcileResult,
};
use swarm_runtime::{ActiveLeaseRecord, AgentNode};
use swarm_types::AgentId;

use super::super::*;

pub(in crate::runner) fn canonical_partition_pair(
    agent_a: &AgentId,
    agent_b: &AgentId,
) -> (AgentId, AgentId) {
    if agent_a.as_ref() <= agent_b.as_ref() {
        (agent_a.clone(), agent_b.clone())
    } else {
        (agent_b.clone(), agent_a.clone())
    }
}

fn connected_components(
    agent_ids: &[AgentId],
    partition_pairs: &HashSet<(AgentId, AgentId)>,
) -> Vec<Vec<AgentId>> {
    let mut visited = HashSet::new();
    let mut components = Vec::new();

    for agent_id in agent_ids {
        if visited.contains(agent_id) {
            continue;
        }
        let mut stack = vec![agent_id.clone()];
        let mut component = Vec::new();
        while let Some(agent_id) = stack.pop() {
            if !visited.insert(agent_id.clone()) {
                continue;
            }
            component.push(agent_id.clone());
            for peer_id in agent_ids {
                if agent_id == *peer_id {
                    continue;
                }
                let pair = canonical_partition_pair(&agent_id, peer_id);
                if partition_pairs.contains(&pair) || visited.contains(peer_id) {
                    continue;
                }
                stack.push(peer_id.clone());
            }
        }
        component.sort_by(|left, right| left.as_ref().cmp(right.as_ref()));
        components.push(component);
    }

    components.sort_by(|left, right| {
        left.len().cmp(&right.len()).then_with(|| {
            left.first()
                .map(|agent_id| agent_id.as_ref())
                .cmp(&right.first().map(|agent_id| agent_id.as_ref()))
        })
    });
    components
}

fn lease_snapshot(holder: &AgentId, lease: &ActiveLeaseRecord) -> LeaseTickSnapshot {
    LeaseTickSnapshot {
        lease_id: lease.lease_id.clone(),
        holder: holder.clone(),
        resource_id: lease.resource_id.clone(),
        resource_kind: lease.resource_kind.clone(),
        granted_tick: lease.granted_tick,
        expiry_tick: lease.expiry_tick,
    }
}

fn current_valid_leases<T: Transport>(
    nodes: &[(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    current_tick: u64,
) -> Vec<(AgentId, ActiveLeaseRecord)> {
    let mut leases = Vec::new();
    for (node, agent_id) in nodes {
        if crashed_agents.contains(agent_id) {
            continue;
        }
        for lease in &node.active_leases {
            if lease.is_valid_at_tick(current_tick) {
                leases.push((agent_id.clone(), lease.clone()));
            }
        }
    }
    leases
}

#[allow(clippy::too_many_arguments)]
pub(in crate::runner) fn handle_partition_activation<T: Transport>(
    nodes: &[(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    active_partition_pairs: &HashSet<(AgentId, AgentId)>,
    partition_pair: &(AgentId, AgentId),
    current_tick: u64,
    degraded_decision_log: &mut Vec<DegradedDecisionLog>,
    partition_reports: &mut Vec<PartitionReport>,
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
) {
    let agent_ids: Vec<AgentId> = nodes
        .iter()
        .filter(|(_, agent_id)| !crashed_agents.contains(agent_id))
        .map(|(_, agent_id)| agent_id.clone())
        .collect();
    let components = connected_components(&agent_ids, active_partition_pairs);
    let group_sizes = components.iter().map(Vec::len).collect::<Vec<_>>();
    let affected_agents = vec![partition_pair.0.clone(), partition_pair.1.clone()];
    let leases_at_partition = current_valid_leases(nodes, crashed_agents, current_tick)
        .into_iter()
        .filter(|(holder, _)| affected_agents.contains(holder))
        .map(|(holder, lease)| lease_snapshot(&holder, &lease))
        .collect::<Vec<_>>();
    partition_reports.push(PartitionReport {
        partition_tick: current_tick,
        heal_tick: None,
        affected_agents: affected_agents.clone(),
        leases_at_partition: leases_at_partition.clone(),
    });

    let mut resources_by_agent: HashMap<AgentId, Vec<String>> = HashMap::new();
    for lease in &leases_at_partition {
        resources_by_agent
            .entry(lease.holder.clone())
            .or_default()
            .push(lease.resource_id.clone());
    }
    for agent_id in &affected_agents {
        let affected_resources = resources_by_agent
            .get(agent_id)
            .cloned()
            .unwrap_or_default();
        let decision = if affected_resources.is_empty() {
            SupervisorDecision::ForbidReassignment
        } else {
            SupervisorDecision::ContinueUnderLease
        };
        degraded_decision_log.push(DegradedDecisionLog {
            tick: current_tick,
            condition: ConnectivityLossKind::SwarmPartitioned {
                group_sizes: group_sizes.clone(),
            },
            decision: decision.clone(),
            affected_resources: affected_resources.clone(),
            affected_agents: vec![agent_id.clone()],
            absence_kind: Some(AgentAbsenceKind::LinkLoss {
                partition_tick: current_tick,
            }),
        });
        if let Some(builder) = log_builder {
            builder.push(swarm_replay::Event::SupervisorDegradedDecision {
                tick: current_tick,
                condition: ConnectivityLossKind::SwarmPartitioned {
                    group_sizes: group_sizes.clone(),
                },
                decision,
                resources: affected_resources,
            });
        }
    }

    if let Some(builder) = log_builder {
        if components.len() >= 2 {
            builder.push(swarm_replay::Event::PartitionDetected {
                tick: current_tick,
                group_a: components[0].clone(),
                group_b: components[1].clone(),
            });
        } else {
            builder.push(swarm_replay::Event::PartitionDetected {
                tick: current_tick,
                group_a: affected_agents.clone(),
                group_b: Vec::new(),
            });
        }
    }
}

fn resolve_conflict(
    holder_a: &AgentId,
    lease_a: &Lease,
    holder_b: &AgentId,
    lease_b: &Lease,
) -> ConflictResolution {
    if lease_a.granted_at < lease_b.granted_at {
        ConflictResolution::OlderLeaseWins {
            winner: holder_a.clone(),
        }
    } else if lease_b.granted_at < lease_a.granted_at {
        ConflictResolution::OlderLeaseWins {
            winner: holder_b.clone(),
        }
    } else {
        ConflictResolution::SupervisorReset
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::runner) fn handle_partition_heal<T: Transport>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    current_tick: u64,
    healed_pair: &(AgentId, AgentId),
    tick_duration_ms: u64,
    degraded_decision_log: &mut Vec<DegradedDecisionLog>,
    partition_reports: &mut [PartitionReport],
    reconciliation_reports: &mut Vec<ReconciliationReport>,
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
) {
    if let Some(report) = partition_reports.iter_mut().rev().find(|report| {
        report.heal_tick.is_none()
            && report.affected_agents.contains(&healed_pair.0)
            && report.affected_agents.contains(&healed_pair.1)
    }) {
        report.heal_tick = Some(current_tick);
    }

    let mut valid_by_resource: BTreeMap<String, Vec<(usize, AgentId, ActiveLeaseRecord)>> =
        BTreeMap::new();
    let mut stale_by_resource: BTreeMap<String, Vec<(usize, AgentId, ActiveLeaseRecord)>> =
        BTreeMap::new();
    for (index, (node, agent_id)) in nodes.iter().enumerate() {
        if crashed_agents.contains(agent_id) {
            continue;
        }
        for lease in &node.active_leases {
            if lease.is_valid_at_tick(current_tick) {
                valid_by_resource
                    .entry(lease.resource_id.clone())
                    .or_default()
                    .push((index, agent_id.clone(), lease.clone()));
            } else {
                stale_by_resource
                    .entry(lease.resource_id.clone())
                    .or_default()
                    .push((index, agent_id.clone(), lease.clone()));
            }
        }
    }

    let mut accepted = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut conflicts = Vec::new();

    for (resource_id, contenders) in valid_by_resource {
        if stale_by_resource.contains_key(&resource_id) {
            rejected.insert(resource_id.clone());
        }
        if contenders.len() == 1 {
            accepted.insert(resource_id);
            continue;
        }
        let leases = contenders
            .iter()
            .map(|(_, holder, lease)| {
                (
                    holder.clone(),
                    lease.as_lease(holder, tick_duration_ms),
                    lease.granted_tick,
                )
            })
            .collect::<Vec<_>>();
        let (holder_a, lease_a, _) = &leases[0];
        let (holder_b, lease_b, _) = &leases[1];
        let resolution = resolve_conflict(holder_a, lease_a, holder_b, lease_b);
        conflicts.push(OwnershipConflict {
            resource_id: resource_id.clone(),
            holder_a: holder_a.clone(),
            lease_a: lease_a.clone(),
            holder_b: holder_b.clone(),
            lease_b: lease_b.clone(),
            resolution: resolution.clone(),
        });

        match resolution {
            ConflictResolution::OlderLeaseWins { winner } => {
                accepted.insert(resource_id.clone());
                for (index, holder, lease) in contenders {
                    if holder == winner {
                        continue;
                    }
                    let node = &mut nodes[index].0;
                    node.active_leases.retain(|candidate| {
                        !(candidate.resource_id == resource_id
                            && candidate.lease_id == lease.lease_id)
                    });
                    rejected.insert(resource_id.clone());
                    if let Some(builder) = log_builder {
                        builder.push(swarm_replay::Event::OwnershipConflict {
                            tick: current_tick,
                            resource_id: resource_id.clone(),
                            claimant_a: winner.clone(),
                            claimant_b: holder.clone(),
                        });
                    }
                }
            }
            ConflictResolution::SupervisorReset => {
                for (index, _, lease) in contenders {
                    let node = &mut nodes[index].0;
                    node.active_leases.retain(|candidate| {
                        !(candidate.resource_id == resource_id
                            && candidate.lease_id == lease.lease_id)
                    });
                }
                rejected.insert(resource_id.clone());
                degraded_decision_log.push(DegradedDecisionLog {
                    tick: current_tick,
                    condition: ConnectivityLossKind::SwarmPartitioned {
                        group_sizes: vec![1, 1],
                    },
                    decision: SupervisorDecision::ForbidReassignment,
                    affected_resources: vec![resource_id.clone()],
                    affected_agents: vec![holder_a.clone(), holder_b.clone()],
                    absence_kind: None,
                });
                if let Some(builder) = log_builder {
                    builder.push(swarm_replay::Event::CommandSuppressed {
                        tick: current_tick,
                        resource_id: resource_id.clone(),
                        reason: "ambiguous_authority".to_owned(),
                    });
                }
            }
        }
    }

    for resource_id in stale_by_resource.keys() {
        if !accepted.contains(resource_id) {
            rejected.insert(resource_id.clone());
        }
    }

    let result = SupervisorReconcileResult {
        accepted: accepted.into_iter().collect(),
        rejected: rejected.into_iter().collect(),
        conflicts: conflicts.clone(),
    };
    reconciliation_reports.push(ReconciliationReport {
        reconnect_tick: current_tick,
        result: result.clone(),
    });

    if let Some(builder) = log_builder {
        builder.push(swarm_replay::Event::PartitionHealed { tick: current_tick });
        builder.push(swarm_replay::Event::SupervisorReconciled {
            tick: current_tick,
            result_summary: result,
        });
    }
}

pub(in crate::runner) fn handle_node_failures<T: Transport>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    newly_failed_agents: &HashSet<AgentId>,
    current_tick: u64,
    degraded_decision_log: &mut Vec<DegradedDecisionLog>,
) {
    for (node, agent_id) in nodes {
        if !newly_failed_agents.contains(agent_id) {
            continue;
        }
        let affected_resources = node
            .active_leases
            .iter()
            .filter(|lease| lease.is_valid_at_tick(current_tick))
            .map(|lease| lease.resource_id.clone())
            .collect::<Vec<_>>();
        if affected_resources.is_empty() {
            continue;
        }
        node.active_leases
            .retain(|lease| !lease.is_valid_at_tick(current_tick));
        degraded_decision_log.push(DegradedDecisionLog {
            tick: current_tick,
            condition: ConnectivityLossKind::DroneIsolated,
            decision: SupervisorDecision::ReleaseAfterTimeout { ticks: 0 },
            affected_resources,
            affected_agents: vec![agent_id.clone()],
            absence_kind: Some(AgentAbsenceKind::NodeFailure),
        });
    }
}
