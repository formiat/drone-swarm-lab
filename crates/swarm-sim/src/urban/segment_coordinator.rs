use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use chrono::{TimeZone, Utc};
use swarm_comms::{
    Lease, LeaseId, SegmentDenyReason, SwarmMessage, SwarmMessageEnvelope, Transport,
    SWARM_PROTOCOL_SCHEMA_VERSION,
};
use swarm_types::{AgentId, UrbanEdgeId, UrbanRightOfWayPolicy};

use super::UrbanSegmentLock;

/// Network-facing analog of `UrbanSegmentLockRegistry`.
pub struct SegmentCoordinator<T: Transport> {
    transport: T,
    coordinator_id: AgentId,
    /// key: `UrbanEdgeId`
    active_locks: HashMap<UrbanEdgeId, (UrbanSegmentLock, Lease)>,
    policy: UrbanRightOfWayPolicy,
    /// key: `AgentId`
    priorities: HashMap<AgentId, u8>,
    default_lease_ticks: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordinatorEvent {
    GrantSent {
        edge_id: UrbanEdgeId,
        to: AgentId,
    },
    DenySent {
        edge_id: UrbanEdgeId,
        to: AgentId,
        reason: SegmentDenyReason,
    },
    Released {
        edge_id: UrbanEdgeId,
        agent_id: AgentId,
    },
    LeaseExpired {
        edge_id: UrbanEdgeId,
        agent_id: AgentId,
    },
}

#[derive(Debug)]
pub enum SegmentCoordinatorError<E> {
    Transport(E),
}

impl<E: fmt::Display> fmt::Display for SegmentCoordinatorError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "transport error: {error}"),
        }
    }
}

impl<E> Error for SegmentCoordinatorError<E> where E: Error + 'static {}

impl<T: Transport> SegmentCoordinator<T> {
    pub fn new(
        coordinator_id: AgentId,
        transport: T,
        policy: UrbanRightOfWayPolicy,
        priorities: HashMap<AgentId, u8>,
    ) -> Self {
        Self {
            transport,
            coordinator_id,
            active_locks: HashMap::new(),
            policy,
            priorities,
            default_lease_ticks: 30,
        }
    }

    pub fn with_default_lease_ticks(mut self, default_lease_ticks: u64) -> Self {
        self.default_lease_ticks = default_lease_ticks.max(1);
        self
    }

    pub fn active_locks(&self) -> impl Iterator<Item = &(UrbanSegmentLock, Lease)> {
        self.active_locks.values()
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn handle_incoming(
        &mut self,
        tick: u64,
    ) -> Result<Vec<CoordinatorEvent>, SegmentCoordinatorError<T::Error>> {
        let mut events = Vec::new();
        self.expire_leases(tick, &mut events);

        while let Some(raw) = self
            .transport
            .poll()
            .map_err(SegmentCoordinatorError::Transport)?
        {
            let Some(envelope) = SwarmMessageEnvelope::from_raw_message(&raw) else {
                continue;
            };
            match envelope.message {
                SwarmMessage::SegmentReserve {
                    edge_id,
                    segment_index,
                    requester,
                    ..
                } => self.handle_reserve(tick, edge_id, segment_index, requester, &mut events)?,
                SwarmMessage::SegmentRelease { edge_id, lease_id } => {
                    self.handle_release(edge_id, lease_id, envelope.from, &mut events);
                }
                _ => {}
            }
        }

        Ok(events)
    }

    fn handle_reserve(
        &mut self,
        tick: u64,
        edge_id: UrbanEdgeId,
        segment_index: usize,
        requester: AgentId,
        events: &mut Vec<CoordinatorEvent>,
    ) -> Result<(), SegmentCoordinatorError<T::Error>> {
        if let Some((lock, lease)) = self.active_locks.get(&edge_id).cloned() {
            if lock.holder_agent_id == requester {
                self.send_grant(tick, &edge_id, &requester, lease)?;
                events.push(CoordinatorEvent::GrantSent {
                    edge_id,
                    to: requester,
                });
                return Ok(());
            }
            let reason = self.deny_reason(&requester, &lock.holder_agent_id);
            self.send_deny(
                tick,
                &edge_id,
                &requester,
                &lock.holder_agent_id,
                reason.clone(),
            )?;
            events.push(CoordinatorEvent::DenySent {
                edge_id,
                to: requester,
                reason,
            });
            return Ok(());
        }

        let lease = self.lease_for(tick, &edge_id, &requester);
        let lock = UrbanSegmentLock {
            edge_id: edge_id.clone(),
            holder_agent_id: requester.clone(),
            acquired_at_tick: tick,
            segment_index,
        };
        self.active_locks
            .insert(edge_id.clone(), (lock, lease.clone()));
        self.send_grant(tick, &edge_id, &requester, lease)?;
        events.push(CoordinatorEvent::GrantSent {
            edge_id,
            to: requester,
        });
        Ok(())
    }

    fn handle_release(
        &mut self,
        edge_id: UrbanEdgeId,
        lease_id: LeaseId,
        from: AgentId,
        events: &mut Vec<CoordinatorEvent>,
    ) {
        let Some((lock, lease)) = self.active_locks.get(&edge_id) else {
            return;
        };
        if lock.holder_agent_id != from || lease.lease_id != lease_id {
            return;
        }
        self.active_locks.remove(&edge_id);
        events.push(CoordinatorEvent::Released {
            edge_id,
            agent_id: from,
        });
    }

    fn expire_leases(&mut self, tick: u64, events: &mut Vec<CoordinatorEvent>) {
        let now = tick_to_time(tick);
        let expired = self
            .active_locks
            .iter()
            .filter(|(_, (_, lease))| !lease.is_valid_at(now))
            .map(|(edge_id, (lock, _))| (edge_id.clone(), lock.holder_agent_id.clone()))
            .collect::<Vec<_>>();
        for (edge_id, agent_id) in expired {
            self.active_locks.remove(&edge_id);
            events.push(CoordinatorEvent::LeaseExpired { edge_id, agent_id });
        }
    }

    fn lease_for(&self, tick: u64, edge_id: &UrbanEdgeId, holder: &AgentId) -> Lease {
        Lease {
            lease_id: LeaseId::from(format!(
                "lease-{}-{}-{tick}",
                edge_id.as_ref(),
                holder.as_ref()
            )),
            holder: holder.clone(),
            resource_id: edge_id.to_string(),
            resource_kind: "urban_edge".to_owned(),
            granted_at: tick_to_time(tick),
            expires_at: tick_to_time(tick.saturating_add(self.default_lease_ticks)),
        }
    }

    fn deny_reason(&self, requester: &AgentId, holder: &AgentId) -> SegmentDenyReason {
        if matches!(
            self.policy,
            UrbanRightOfWayPolicy::Priority | UrbanRightOfWayPolicy::MissionCriticalOverride
        ) {
            let requester_priority = self.priorities.get(requester).copied().unwrap_or(0);
            let holder_priority = self.priorities.get(holder).copied().unwrap_or(0);
            if requester_priority > holder_priority {
                return SegmentDenyReason::PolicyDenied;
            }
        }
        SegmentDenyReason::AlreadyHeld
    }

    fn send_grant(
        &mut self,
        tick: u64,
        edge_id: &UrbanEdgeId,
        to: &AgentId,
        lease: Lease,
    ) -> Result<(), SegmentCoordinatorError<T::Error>> {
        self.send_message(
            tick,
            to,
            SwarmMessage::SegmentGrant {
                edge_id: edge_id.clone(),
                to: to.clone(),
                lease,
            },
        )
    }

    fn send_deny(
        &mut self,
        tick: u64,
        edge_id: &UrbanEdgeId,
        to: &AgentId,
        holder: &AgentId,
        reason: SegmentDenyReason,
    ) -> Result<(), SegmentCoordinatorError<T::Error>> {
        self.send_message(
            tick,
            to,
            SwarmMessage::SegmentDeny {
                edge_id: edge_id.clone(),
                to: to.clone(),
                holder: holder.clone(),
                reason,
            },
        )
    }

    fn send_message(
        &mut self,
        tick: u64,
        to: &AgentId,
        message: SwarmMessage,
    ) -> Result<(), SegmentCoordinatorError<T::Error>> {
        let envelope = SwarmMessageEnvelope {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
            envelope_id: format!(
                "segment-coordinator-{}-{}-{tick}",
                self.coordinator_id.as_ref(),
                to.as_ref()
            ),
            correlation_id: None,
            from: self.coordinator_id.clone(),
            to: to.clone(),
            sent_at: tick_to_time(tick),
            ttl_ticks: 10,
            message,
        };
        self.transport
            .send(envelope.into_raw_message())
            .map_err(SegmentCoordinatorError::Transport)
    }
}

fn tick_to_time(tick: u64) -> chrono::DateTime<Utc> {
    let tick = i64::try_from(tick).unwrap_or(i64::MAX);
    Utc.timestamp_opt(tick, 0)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().expect("unix epoch"))
}
