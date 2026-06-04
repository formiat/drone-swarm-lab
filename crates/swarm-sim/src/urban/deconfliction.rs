use std::collections::HashMap;

use swarm_types::{AgentId, UrbanEdgeId, UrbanRightOfWayPolicy};

/// Active ownership record for one Urban route segment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrbanSegmentLock {
    pub edge_id: UrbanEdgeId,
    pub holder_agent_id: AgentId,
    pub acquired_at_tick: u64,
    pub segment_index: usize,
}

/// Request to reserve one Urban route segment before entering it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentLockRequest {
    pub agent_id: AgentId,
    pub edge_id: UrbanEdgeId,
    pub segment_index: usize,
    pub request_order: usize,
}

/// Conflict record emitted when an agent cannot acquire a segment lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrbanSegmentConflictRecord {
    pub tick: u64,
    pub edge_id: UrbanEdgeId,
    pub holder_agent_id: AgentId,
    pub requester_agent_id: AgentId,
    pub policy: UrbanRightOfWayPolicy,
    pub reason: String,
}

/// Result of a segment-lock request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentLockDecision {
    Acquired(UrbanSegmentLock),
    AlreadyHeld(UrbanSegmentLock),
    Conflict(UrbanSegmentConflictRecord),
}

/// Runtime registry for mission-level Urban segment ownership.
#[derive(Clone, Debug, Default)]
pub struct UrbanSegmentLockRegistry {
    active_locks: HashMap<UrbanEdgeId, UrbanSegmentLock>,
    conflict_history: Vec<UrbanSegmentConflictRecord>,
    round_robin_next: HashMap<UrbanEdgeId, usize>,
}

impl UrbanSegmentLockRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_lock(
        &mut self,
        request: SegmentLockRequest,
        tick: u64,
        policy: &UrbanRightOfWayPolicy,
        priorities: &HashMap<AgentId, u8>,
    ) -> SegmentLockDecision {
        self.request_batch(vec![request], tick, policy, priorities)
            .into_iter()
            .next()
            .map(|(_, decision)| decision)
            .expect("one request yields one decision")
    }

    pub fn request_batch(
        &mut self,
        mut requests: Vec<SegmentLockRequest>,
        tick: u64,
        policy: &UrbanRightOfWayPolicy,
        priorities: &HashMap<AgentId, u8>,
    ) -> Vec<(AgentId, SegmentLockDecision)> {
        requests.sort_by(|left, right| {
            (
                left.edge_id.to_string(),
                left.request_order,
                left.agent_id.to_string(),
            )
                .cmp(&(
                    right.edge_id.to_string(),
                    right.request_order,
                    right.agent_id.to_string(),
                ))
        });

        let mut grouped: Vec<(UrbanEdgeId, Vec<SegmentLockRequest>)> = Vec::new();
        for request in requests {
            if let Some((edge_id, group)) = grouped.last_mut() {
                if edge_id == &request.edge_id {
                    group.push(request);
                    continue;
                }
            }
            grouped.push((request.edge_id.clone(), vec![request]));
        }

        let mut decisions = Vec::new();
        for (edge_id, group) in grouped {
            if let Some(lock) = self.active_locks.get(&edge_id).cloned() {
                for request in group {
                    let decision = if request.agent_id == lock.holder_agent_id {
                        SegmentLockDecision::AlreadyHeld(lock.clone())
                    } else {
                        SegmentLockDecision::Conflict(self.record_conflict(
                            tick,
                            &edge_id,
                            &lock.holder_agent_id,
                            &request.agent_id,
                            policy,
                            "segment already locked",
                        ))
                    };
                    decisions.push((request.agent_id, decision));
                }
                continue;
            }

            let winner_index = self.winner_index(&edge_id, &group, policy, priorities);
            let winner = &group[winner_index];
            let lock = UrbanSegmentLock {
                edge_id: edge_id.clone(),
                holder_agent_id: winner.agent_id.clone(),
                acquired_at_tick: tick,
                segment_index: winner.segment_index,
            };
            self.active_locks.insert(edge_id.clone(), lock.clone());
            if matches!(policy, UrbanRightOfWayPolicy::RoundRobin) {
                let next = (winner_index + 1) % group.len().max(1);
                self.round_robin_next.insert(edge_id.clone(), next);
            }

            for (index, request) in group.into_iter().enumerate() {
                if index == winner_index {
                    decisions.push((
                        request.agent_id,
                        SegmentLockDecision::Acquired(lock.clone()),
                    ));
                } else {
                    decisions.push((
                        request.agent_id.clone(),
                        SegmentLockDecision::Conflict(self.record_conflict(
                            tick,
                            &edge_id,
                            &lock.holder_agent_id,
                            &request.agent_id,
                            policy,
                            "right-of-way loser",
                        )),
                    ));
                }
            }
        }

        decisions.sort_by(|left, right| left.0.to_string().cmp(&right.0.to_string()));
        decisions
    }

    pub fn release(
        &mut self,
        edge_id: &UrbanEdgeId,
        agent_id: &AgentId,
        _tick: u64,
    ) -> Option<UrbanSegmentLock> {
        let lock = self.active_locks.get(edge_id)?;
        if &lock.holder_agent_id != agent_id {
            return None;
        }
        self.active_locks.remove(edge_id)
    }

    pub fn active_locks(&self) -> impl Iterator<Item = &UrbanSegmentLock> {
        self.active_locks.values()
    }

    pub fn conflict_history(&self) -> &[UrbanSegmentConflictRecord] {
        &self.conflict_history
    }

    pub fn is_locked_by_other(&self, edge_id: &UrbanEdgeId, agent_id: &AgentId) -> bool {
        self.active_locks
            .get(edge_id)
            .is_some_and(|lock| &lock.holder_agent_id != agent_id)
    }

    pub fn locked_edges_except(&self, agent_id: &AgentId) -> Vec<UrbanEdgeId> {
        self.active_locks
            .values()
            .filter(|lock| &lock.holder_agent_id != agent_id)
            .map(|lock| lock.edge_id.clone())
            .collect()
    }

    fn winner_index(
        &self,
        edge_id: &UrbanEdgeId,
        group: &[SegmentLockRequest],
        policy: &UrbanRightOfWayPolicy,
        priorities: &HashMap<AgentId, u8>,
    ) -> usize {
        match policy {
            UrbanRightOfWayPolicy::FirstCome | UrbanRightOfWayPolicy::MissionCriticalOverride => 0,
            UrbanRightOfWayPolicy::Priority => group
                .iter()
                .enumerate()
                .max_by(|(_, left), (_, right)| {
                    let left_priority = priorities.get(&left.agent_id).copied().unwrap_or(0);
                    let right_priority = priorities.get(&right.agent_id).copied().unwrap_or(0);
                    (left_priority, std::cmp::Reverse(left.agent_id.to_string())).cmp(&(
                        right_priority,
                        std::cmp::Reverse(right.agent_id.to_string()),
                    ))
                })
                .map(|(index, _)| index)
                .unwrap_or(0),
            UrbanRightOfWayPolicy::RoundRobin => {
                let cursor = self.round_robin_next.get(edge_id).copied().unwrap_or(0);
                cursor % group.len().max(1)
            }
        }
    }

    fn record_conflict(
        &mut self,
        tick: u64,
        edge_id: &UrbanEdgeId,
        holder_agent_id: &AgentId,
        requester_agent_id: &AgentId,
        policy: &UrbanRightOfWayPolicy,
        reason: &str,
    ) -> UrbanSegmentConflictRecord {
        let record = UrbanSegmentConflictRecord {
            tick,
            edge_id: edge_id.clone(),
            holder_agent_id: holder_agent_id.clone(),
            requester_agent_id: requester_agent_id.clone(),
            policy: policy.clone(),
            reason: reason.to_owned(),
        };
        self.conflict_history.push(record.clone());
        record
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(index: usize) -> AgentId {
        AgentId::from(format!("agent-{index}"))
    }

    fn edge() -> UrbanEdgeId {
        UrbanEdgeId::from("edge-a-b".to_owned())
    }

    fn request(index: usize) -> SegmentLockRequest {
        SegmentLockRequest {
            agent_id: agent(index),
            edge_id: edge(),
            segment_index: 0,
            request_order: index,
        }
    }

    #[test]
    fn first_come_allows_exactly_one_holder() {
        let mut registry = UrbanSegmentLockRegistry::new();
        let decisions = registry.request_batch(
            vec![request(0), request(1)],
            1,
            &UrbanRightOfWayPolicy::FirstCome,
            &HashMap::new(),
        );

        assert!(matches!(decisions[0].1, SegmentLockDecision::Acquired(_)));
        assert!(matches!(decisions[1].1, SegmentLockDecision::Conflict(_)));
        assert_eq!(registry.active_locks().count(), 1);
        assert_eq!(registry.conflict_history().len(), 1);
    }

    #[test]
    fn priority_prefers_higher_priority_with_stable_tie_break() {
        let mut registry = UrbanSegmentLockRegistry::new();
        let mut priorities = HashMap::new();
        priorities.insert(agent(0), 1);
        priorities.insert(agent(1), 9);
        let decisions = registry.request_batch(
            vec![request(0), request(1)],
            1,
            &UrbanRightOfWayPolicy::Priority,
            &priorities,
        );

        assert!(matches!(decisions[0].1, SegmentLockDecision::Conflict(_)));
        assert!(matches!(decisions[1].1, SegmentLockDecision::Acquired(_)));
    }

    #[test]
    fn round_robin_rotates_between_conflicts() {
        let mut registry = UrbanSegmentLockRegistry::new();
        let first = registry.request_batch(
            vec![request(0), request(1)],
            1,
            &UrbanRightOfWayPolicy::RoundRobin,
            &HashMap::new(),
        );
        let first_holder = registry.release(&edge(), &agent(0), 2).unwrap();
        assert_eq!(first_holder.holder_agent_id, agent(0));
        assert!(matches!(first[0].1, SegmentLockDecision::Acquired(_)));

        let second = registry.request_batch(
            vec![request(0), request(1)],
            3,
            &UrbanRightOfWayPolicy::RoundRobin,
            &HashMap::new(),
        );
        assert!(matches!(second[0].1, SegmentLockDecision::Conflict(_)));
        assert!(matches!(second[1].1, SegmentLockDecision::Acquired(_)));
    }
}
