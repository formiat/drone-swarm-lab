use std::collections::{HashMap, HashSet};

use swarm_alloc::{AllocationAgent, AllocationTask, Allocator, CbbaAllocator};
use swarm_comms::{RawMessage, Transport};
use swarm_types::{AgentId, Task, TaskId};

use crate::message::RuntimeMessage;
use crate::Coordinator;

use super::gossip::apply_gossip_messages;
use super::reallocation::allocate_unassigned;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssignmentChange {
    pub task_id: TaskId,
    pub agent_id: AgentId,
}

pub struct NodeTickOutput {
    pub newly_failed: Vec<AgentId>,
    pub failure_releases: Vec<crate::coordinator::FailureRelease>,
    pub released_tasks: Vec<TaskId>,
    pub reassigned_tasks: Vec<AssignmentChange>,
    pub tasks_recovered: Vec<TaskId>,
    pub reassignment_count: u64,
    pub reallocation_latency_ticks: Option<u64>,
    pub expired_task_ids: Vec<TaskId>,
    pub conflicting_assignments: u64,
    pub discarded_messages: u64,
    pub distance_travelled: Vec<(AgentId, f64)>,
}

pub struct NodeConfig {
    pub tick_duration_ms: u64,
    pub enable_movement: bool,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            tick_duration_ms: 100,
            enable_movement: false,
        }
    }
}

pub struct AgentNode<T> {
    pub coordinator: Coordinator,
    pub transport: T,
    pub own_id: AgentId,
    pub peer_ids: Vec<AgentId>,
    pub gossip_interval_ticks: u64,
    pub generation: u64,
    pub config: NodeConfig,
    ticks_since_last_gossip: u64,
    discarded_this_tick: u64,
    pub cbba: Option<CbbaAllocator>,
}

impl<T: Transport> AgentNode<T> {
    pub fn new(
        own_id: AgentId,
        peer_ids: Vec<AgentId>,
        coordinator: Coordinator,
        transport: T,
    ) -> Self {
        Self {
            coordinator,
            transport,
            own_id,
            peer_ids,
            gossip_interval_ticks: 3,
            generation: 1,
            config: NodeConfig::default(),
            ticks_since_last_gossip: 0,
            discarded_this_tick: 0,
            cbba: None,
        }
    }

    pub fn tick<A: Allocator>(
        &mut self,
        current_tick: u64,
        allocator: &mut A,
        injected: Vec<Task>,
    ) -> Result<NodeTickOutput, T::Error> {
        self.send_heartbeats(current_tick)?;
        self.process_inbox_and_allocate(current_tick, allocator, injected)
    }

    pub fn send_heartbeats(&mut self, current_tick: u64) -> Result<(), T::Error> {
        let payload = RuntimeMessage::heartbeat(current_tick, self.generation);
        let hb = RawMessage {
            from: self.own_id.clone(),
            to: AgentId::from("placeholder".to_owned()),
            payload,
        };

        for peer_id in &self.peer_ids {
            let mut msg = hb.clone();
            msg.to = peer_id.clone();
            self.transport.send(msg)?;
        }
        Ok(())
    }

    pub fn process_inbox_and_allocate<A: Allocator>(
        &mut self,
        current_tick: u64,
        allocator: &mut A,
        injected: Vec<Task>,
    ) -> Result<NodeTickOutput, T::Error> {
        let mut all_msgs: Vec<RawMessage> = Vec::new();
        loop {
            match self.transport.poll() {
                Ok(Some(msg)) => all_msgs.push(msg),
                Ok(None) => break,
                Err(_) => break,
            }
        }

        self.discarded_this_tick = 0;

        let mut hb_list: Vec<(AgentId, u64, u64)> = Vec::new();
        let mut gossip_buffer: Vec<RuntimeMessage> = Vec::new();
        for msg in &all_msgs {
            match RuntimeMessage::from_payload(&msg.payload) {
                Some(RuntimeMessage::Heartbeat {
                    sender_tick,
                    generation,
                }) => {
                    hb_list.push((msg.from.clone(), sender_tick, generation));
                }
                Some(RuntimeMessage::Gossip { .. }) => {
                    if let Some(rt) = RuntimeMessage::from_payload(&msg.payload) {
                        gossip_buffer.push(rt);
                    }
                }
                Some(RuntimeMessage::Cbba {
                    round: _,
                    winning_bids,
                    sender_bundle: _,
                }) => {
                    if let Some(ref mut cbba) = self.cbba {
                        #[allow(clippy::type_complexity)]
                        let remote_bids: Vec<(
                            AgentId,
                            HashMap<TaskId, (AgentId, f64)>,
                        )> = vec![(
                            msg.from.clone(),
                            winning_bids
                                .iter()
                                .map(|(tid, bid)| (tid.clone(), (bid.agent_id.clone(), bid.value)))
                                .collect(),
                        )];
                        cbba.apply_remote_bids(&remote_bids);
                        for (task_id, bid) in winning_bids {
                            if self.coordinator.registry.tasks().any(|t| t.id == task_id) {
                                let _ = self
                                    .coordinator
                                    .registry
                                    .assign(&task_id, bid.agent_id.clone());
                            }
                        }
                    }
                }
                None => {
                    self.discarded_this_tick += 1;
                    tracing::warn!(
                        from = %msg.from,
                        "discarded unknown message payload"
                    );
                }
            }
        }

        self.coordinator
            .membership
            .record_heartbeat(&self.own_id, current_tick, self.generation);
        for (from, sender_tick, gen) in &hb_list {
            self.coordinator
                .membership
                .record_heartbeat(from, *sender_tick, *gen);
        }

        let mut heartbeat_senders: Vec<AgentId> =
            hb_list.iter().map(|(id, _, _)| id.clone()).collect();
        if !heartbeat_senders.contains(&self.own_id) {
            heartbeat_senders.push(self.own_id.clone());
        }

        let output = self
            .coordinator
            .process_tick(heartbeat_senders, current_tick, injected);

        let mut conflicting_assignments: u64 = 0;
        let mut reassigned_tasks = Vec::new();

        if !gossip_buffer.is_empty() {
            let (merged, _stale) = self.apply_gossip_buffer(&gossip_buffer);
            conflicting_assignments += merged;
        }

        let has_idle_agents = self.coordinator.membership.alive_agents().any(|(id, _)| {
            self.coordinator
                .registry
                .tasks()
                .find(|t| t.assigned_to.as_ref() == Some(id))
                .is_none()
        });
        if !output.released_tasks.is_empty()
            || !output.expired_task_ids.is_empty()
            || !self.coordinator.registry.unassigned().is_empty()
            || has_idle_agents
        {
            // Skip centralized allocation when CBBA is active (distributed path)
            if self.cbba.is_none() {
                let allocation = allocate_unassigned(&mut self.coordinator, allocator);
                conflicting_assignments += allocation.conflicting_assignments;
                reassigned_tasks.extend(allocation.assignments);
            }
        }

        let released_task_ids: HashSet<TaskId> = output.released_tasks.iter().cloned().collect();
        let mut tasks_recovered: Vec<TaskId> = reassigned_tasks
            .iter()
            .filter(|assignment| released_task_ids.contains(&assignment.task_id))
            .map(|assignment| assignment.task_id.clone())
            .collect();
        tasks_recovered.sort_by(|a, b| a.as_ref().cmp(b.as_ref()));
        let reassignment_count = tasks_recovered.len() as u64;
        let reallocation_latency_ticks = if tasks_recovered.is_empty() {
            None
        } else {
            Some(0)
        };

        // CBBA Phase 1: Bundle building (local)
        if let Some(ref mut cbba) = self.cbba {
            cbba.current_round += 1;
            cbba.check_convergence();
            let agents: Vec<AllocationAgent> = self
                .coordinator
                .membership
                .alive_agents()
                .map(|(id, entry)| AllocationAgent {
                    id: id.clone(),
                    pose: entry.pose,
                    battery: entry.battery,
                    capabilities: entry.capabilities.clone(),
                    role: entry.role.clone(),
                    comms_range: entry.comms_range,
                    speed: 0.0,
                    max_range: 0.0,
                    battery_drain_rate: 0.0,
                })
                .collect();
            let all_tasks: Vec<AllocationTask<'_>> = self
                .coordinator
                .registry
                .tasks()
                .map(|t| AllocationTask { task: t })
                .collect();
            cbba.build_bundles(&agents, &all_tasks);
            for (task_id, agent_id) in cbba.current_assignments() {
                if agent_id == self.own_id {
                    let _ = self.coordinator.registry.assign(&task_id, agent_id);
                }
            }
        }

        let _ = self.maybe_send_gossip();

        let mut distance_travelled = Vec::new();
        if self.config.enable_movement {
            let (exhausted, distances) = self
                .coordinator
                .membership
                .apply_movement(&self.coordinator.registry, self.config.tick_duration_ms);
            for agent_id in &exhausted {
                self.coordinator.membership.mark_dead(agent_id);
                self.coordinator.registry.release_agent_tasks(agent_id);
            }
            distance_travelled = distances;
        }

        Ok(NodeTickOutput {
            newly_failed: output.newly_failed,
            failure_releases: output.failure_releases,
            released_tasks: output.released_tasks,
            reassigned_tasks,
            tasks_recovered,
            reassignment_count,
            reallocation_latency_ticks,
            expired_task_ids: output.expired_task_ids,
            conflicting_assignments,
            discarded_messages: self.discarded_this_tick,
            distance_travelled,
        })
    }

    fn maybe_send_gossip(&mut self) -> Result<(), T::Error> {
        self.ticks_since_last_gossip += 1;
        if self.ticks_since_last_gossip >= self.gossip_interval_ticks {
            self.send_gossip()?;
            self.ticks_since_last_gossip = 0;
            if self.cbba.is_some() {
                let _ = self.send_cbba_bids();
            }
        }
        Ok(())
    }

    pub fn send_gossip(&mut self) -> Result<(), T::Error> {
        let assignments: HashMap<TaskId, AgentId> = self
            .coordinator
            .registry
            .tasks()
            .filter_map(|t| t.assigned_to.clone().map(|a| (t.id.clone(), a)))
            .collect();

        let generations: HashMap<AgentId, u64> = self
            .coordinator
            .membership
            .all_generations()
            .map(|(id, gen)| (id.clone(), gen))
            .collect();

        let payload = RuntimeMessage::gossip(assignments, generations);
        let msg = RawMessage {
            from: self.own_id.clone(),
            to: AgentId::from("placeholder".to_owned()),
            payload,
        };

        for peer_id in &self.peer_ids {
            let mut m = msg.clone();
            m.to = peer_id.clone();
            self.transport.send(m)?;
        }
        Ok(())
    }

    pub fn send_cbba_bids(&mut self) -> Result<(), T::Error> {
        let Some(ref cbba) = self.cbba else {
            return Ok(());
        };
        let winning_bids: std::collections::HashMap<_, _> = cbba
            .winning_bids
            .iter()
            .map(|(tid, (aid, v))| {
                (
                    tid.clone(),
                    crate::message::CbbaBid {
                        agent_id: aid.clone(),
                        value: *v,
                    },
                )
            })
            .collect();
        let bundle = cbba.bundles.get(&self.own_id).cloned().unwrap_or_default();
        let payload =
            crate::message::RuntimeMessage::cbba(cbba.current_round, winning_bids, bundle);
        let msg = RawMessage {
            from: self.own_id.clone(),
            to: AgentId::from("".to_owned()),
            payload,
        };
        for peer_id in &self.peer_ids {
            let mut m = msg.clone();
            m.to = peer_id.clone();
            self.transport.send(m)?;
        }
        Ok(())
    }

    pub fn apply_gossip_buffer(&mut self, buffer: &[RuntimeMessage]) -> (u64, u64) {
        apply_gossip_messages(&mut self.coordinator, buffer)
    }
}
