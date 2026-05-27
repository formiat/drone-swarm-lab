use std::collections::{HashMap, HashSet};

use swarm_alloc::{AllocationAgent, AllocationTask, Allocator, CbbaAllocator, ConnectivityContext};
use swarm_comms::{ConnectivitySnapshot, RawMessage, Transport};
use swarm_types::{AgentId, Health, Task, TaskId};

use crate::message::RuntimeMessage;
use crate::Coordinator;

pub struct NodeTickOutput {
    pub newly_failed: Vec<AgentId>,
    pub released_tasks: Vec<TaskId>,
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
                conflicting_assignments += allocate_unassigned(&mut self.coordinator, allocator);
            }
        }

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
            released_tasks: output.released_tasks,
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
        let mut merged: u64 = 0;
        let mut stale: u64 = 0;

        for msg in buffer {
            if let RuntimeMessage::Gossip {
                assignments,
                generations,
            } = msg
            {
                for (task_id, remote_agent_id) in assignments {
                    let local_owner = self
                        .coordinator
                        .registry
                        .tasks()
                        .find(|t| &t.id == task_id)
                        .and_then(|t| t.assigned_to.clone());

                    match local_owner {
                        None => {
                            if self.coordinator.membership.is_alive(remote_agent_id) {
                                let _ = self
                                    .coordinator
                                    .registry
                                    .assign(task_id, remote_agent_id.clone());
                                merged += 1;
                            } else {
                                stale += 1;
                            }
                        }
                        Some(ref local_id) if local_id == remote_agent_id => {
                            // Already agree
                        }
                        Some(ref local_id) => {
                            if !self.coordinator.membership.is_alive(remote_agent_id) {
                                stale += 1;
                                continue;
                            }

                            let local_gen = self.coordinator.membership.generation_of(local_id);
                            let remote_gen = generations.get(remote_agent_id).copied().unwrap_or(1);

                            if remote_gen > local_gen {
                                // Remote agent has higher generation — authoritative
                                self.coordinator.registry.release_task(task_id);
                                let _ = self
                                    .coordinator
                                    .registry
                                    .assign(task_id, remote_agent_id.clone());
                                merged += 1;
                            } else if remote_gen == local_gen
                                && remote_agent_id.as_ref() > local_id.as_ref()
                            {
                                // Equal generation, deterministic tiebreaker: max AgentId wins
                                self.coordinator.registry.release_task(task_id);
                                let _ = self
                                    .coordinator
                                    .registry
                                    .assign(task_id, remote_agent_id.clone());
                                merged += 1;
                            } else {
                                stale += 1;
                            }
                        }
                    }
                }

                for (agent_id, remote_gen) in generations {
                    let local_gen = self.coordinator.membership.generation_of(agent_id);
                    if *remote_gen > local_gen {
                        self.coordinator
                            .membership
                            .record_heartbeat(agent_id, 0, *remote_gen);
                    }
                }
            }
        }

        (merged, stale)
    }
}

fn allocate_unassigned<A: Allocator>(coordinator: &mut Coordinator, allocator: &mut A) -> u64 {
    let mut tasks: Vec<Task> = coordinator
        .registry
        .unassigned()
        .into_iter()
        .cloned()
        .collect();
    tasks.sort_by(|a, b| a.id.as_ref().cmp(b.id.as_ref()));
    let allocation_tasks: Vec<AllocationTask<'_>> =
        tasks.iter().map(|task| AllocationTask { task }).collect();

    let mut agents: Vec<AllocationAgent> = coordinator
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
    agents.sort_by(|a, b| a.id.as_ref().cmp(b.id.as_ref()));

    // Build connectivity context for v0.5+ allocators
    let agent_entries: Vec<(AgentId, swarm_types::Pose, f64, Health)> = coordinator
        .membership
        .alive_agents()
        .map(|(id, entry)| (id.clone(), entry.pose, entry.comms_range, Health::Alive))
        .collect();
    let base_id = agents
        .first()
        .map(|a| a.id.clone())
        .unwrap_or_else(|| AgentId::from("base".to_owned()));
    let base_pose = agents.first().map(|a| a.pose).unwrap_or(swarm_types::Pose {
        x: 0.0,
        y: 0.0,
        ..Default::default()
    });
    let connectivity = ConnectivityContext {
        snapshot: ConnectivitySnapshot {
            agent_entries,
            ground_nodes: vec![],
            base_id: base_id.to_string(),
            base_pose,
        },
        base_id: base_id.clone(),
    };

    let decisions = allocator.allocate_with_connectivity(&allocation_tasks, &agents, &connectivity);

    let mut seen = HashSet::new();
    let mut conflicts: u64 = 0;
    for (task_id, agent_id) in decisions {
        if !seen.insert(task_id.clone()) {
            conflicts += 1;
            continue;
        }
        if coordinator.registry.assign(&task_id, agent_id).is_err() {
            conflicts += 1;
        }
    }
    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use swarm_alloc::GreedyAllocator;
    use swarm_comms::{InMemAgentTransport, InMemNetwork, NetworkConfig};
    use swarm_types::{Agent, Health, Pose, Role, TaskStatus};

    fn agent_entry(id: &str) -> Agent {
        Agent {
            id: AgentId::from(id.to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            capabilities: vec![],
            current_task: None,
            battery: 100.0,
            comms_range: f64::INFINITY,
            generation: 1,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
            battery_model: None,
        }
    }

    fn task_entry(id: &str) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
            grid_cell: None,
            edge_id: None,
            kind: None,
        }
    }

    fn make_network_config() -> NetworkConfig {
        NetworkConfig {
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            latency_per_hop: 0,
            seed: 42,
            partitions: HashSet::new(),
            comms_jitter_ticks: 0,
        }
    }

    fn make_bus() -> Rc<RefCell<InMemNetwork>> {
        Rc::new(RefCell::new(InMemNetwork::new(make_network_config())))
    }

    fn make_hb_msg(from: &str, to: &str, tick: u64, gen: u64) -> RawMessage {
        RawMessage {
            from: AgentId::from(from.to_owned()),
            to: AgentId::from(to.to_owned()),
            payload: RuntimeMessage::heartbeat(tick, gen),
        }
    }

    #[test]
    fn dispatch_heartbeat_updates_membership() {
        let bus = make_bus();
        let transport = InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));
        bus.borrow_mut().advance_tick();
        bus.borrow_mut()
            .send(make_hb_msg("agent-1", "agent-0", 5, 1))
            .unwrap();
        bus.borrow_mut().advance_tick();

        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![AgentId::from("agent-1".to_owned())],
            Coordinator::new(
                vec![agent_entry("agent-0"), agent_entry("agent-1")],
                vec![],
                5,
            ),
            transport,
        );
        node.gossip_interval_ticks = 999;

        let mut allocator = GreedyAllocator;
        node.tick(1, &mut allocator, vec![]).unwrap();

        let entry = node
            .coordinator
            .membership
            .get(&AgentId::from("agent-1".to_owned()))
            .unwrap();
        assert_eq!(entry.last_heartbeat_tick, 5);
    }

    #[test]
    fn dispatch_gossip_does_not_affect_heartbeat_senders() {
        let bus = make_bus();
        let transport = InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));

        let agent_0 = AgentId::from("agent-0".to_owned());
        let agent_1 = AgentId::from("agent-1".to_owned());
        bus.borrow_mut().advance_tick();

        // Send gossip (not heartbeat) from agent-1
        let gossip_payload = RuntimeMessage::gossip(HashMap::new(), {
            let mut m = HashMap::new();
            m.insert(agent_1.clone(), 1);
            m
        });
        bus.borrow_mut()
            .send(RawMessage {
                from: agent_1.clone(),
                to: agent_0.clone(),
                payload: gossip_payload,
            })
            .unwrap();
        bus.borrow_mut().advance_tick();

        let mut node = AgentNode::new(
            agent_0.clone(),
            vec![agent_1],
            Coordinator::new(
                vec![agent_entry("agent-0"), agent_entry("agent-1")],
                vec![],
                5,
            ),
            transport,
        );
        node.gossip_interval_ticks = 999;

        let mut allocator = GreedyAllocator;
        let out = node.tick(1, &mut allocator, vec![]).unwrap();

        // Gossip-only message should NOT count as heartbeat
        assert!(out.newly_failed.is_empty());
    }

    #[test]
    fn dispatch_unknown_payload_is_discarded() {
        let bus = make_bus();
        let transport = InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));
        bus.borrow_mut().advance_tick();
        bus.borrow_mut()
            .send(RawMessage {
                from: AgentId::from("agent-X".to_owned()),
                to: AgentId::from("agent-0".to_owned()),
                payload: b"garbage".to_vec(),
            })
            .unwrap();
        bus.borrow_mut().advance_tick();

        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![AgentId::from("agent-1".to_owned())],
            Coordinator::new(
                vec![agent_entry("agent-0"), agent_entry("agent-1")],
                vec![],
                5,
            ),
            transport,
        );
        node.gossip_interval_ticks = 999;

        let mut allocator = GreedyAllocator;
        let out = node.tick(1, &mut allocator, vec![]).unwrap();
        assert_eq!(out.discarded_messages, 1);
    }

    #[test]
    fn gossip_merge_unassigned_task_from_remote() {
        let task = task_entry("task-0");
        let mut coord = Coordinator::new(
            vec![
                agent_entry("agent-0"),
                agent_entry("agent-1"),
                agent_entry("agent-2"),
            ],
            vec![task],
            5,
        );
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-0".to_owned()), 0, 1);
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);

        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![],
            coord,
            InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
        );

        let gossip = RuntimeMessage::Gossip {
            assignments: HashMap::from([(
                TaskId::from("task-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            )]),
            generations: HashMap::from([
                (AgentId::from("agent-0".to_owned()), 1),
                (AgentId::from("agent-1".to_owned()), 1),
            ]),
        };
        node.apply_gossip_buffer(&[gossip]);

        let t = node
            .coordinator
            .registry
            .tasks()
            .find(|t| t.id == TaskId::from("task-0".to_owned()))
            .unwrap();
        assert_eq!(t.assigned_to, Some(AgentId::from("agent-1".to_owned())));
    }

    #[test]
    fn gossip_merge_higher_generation_overrides_local() {
        let mut task = task_entry("task-0");
        task.status = TaskStatus::Assigned;
        task.assigned_to = Some(AgentId::from("agent-1".to_owned()));

        let mut coord = Coordinator::new(
            vec![
                agent_entry("agent-0"),
                agent_entry("agent-1"),
                agent_entry("agent-2"),
            ],
            vec![task],
            5,
        );
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-2".to_owned()), 0, 3);

        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![],
            coord,
            InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
        );

        let gossip = RuntimeMessage::Gossip {
            assignments: HashMap::from([(
                TaskId::from("task-0".to_owned()),
                AgentId::from("agent-2".to_owned()),
            )]),
            generations: HashMap::from([
                (AgentId::from("agent-1".to_owned()), 1),
                (AgentId::from("agent-2".to_owned()), 3),
            ]),
        };
        node.apply_gossip_buffer(&[gossip]);

        let t = node
            .coordinator
            .registry
            .tasks()
            .find(|t| t.id == TaskId::from("task-0".to_owned()))
            .unwrap();
        assert_eq!(t.assigned_to, Some(AgentId::from("agent-2".to_owned())));
    }

    #[test]
    fn gossip_merge_equal_generation_max_agentid_wins() {
        let mut task = task_entry("task-0");
        task.status = TaskStatus::Assigned;
        task.assigned_to = Some(AgentId::from("agent-1".to_owned()));

        let mut coord = Coordinator::new(
            vec![
                agent_entry("agent-0"),
                agent_entry("agent-1"),
                agent_entry("agent-2"),
            ],
            vec![task],
            5,
        );
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-2".to_owned()), 0, 1);

        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![],
            coord,
            InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
        );

        let gossip = RuntimeMessage::Gossip {
            assignments: HashMap::from([(
                TaskId::from("task-0".to_owned()),
                AgentId::from("agent-2".to_owned()),
            )]),
            generations: HashMap::from([
                (AgentId::from("agent-1".to_owned()), 1),
                (AgentId::from("agent-2".to_owned()), 1),
            ]),
        };
        node.apply_gossip_buffer(&[gossip]);

        // agent-2 > agent-1 lexicographically, so remote wins
        let t = node
            .coordinator
            .registry
            .tasks()
            .find(|t| t.id == TaskId::from("task-0".to_owned()))
            .unwrap();
        assert_eq!(t.assigned_to, Some(AgentId::from("agent-2".to_owned())));
    }

    #[test]
    fn gossip_merge_lower_generation_is_ignored() {
        let mut task = task_entry("task-0");
        task.status = TaskStatus::Assigned;
        task.assigned_to = Some(AgentId::from("agent-2".to_owned()));

        let mut coord = Coordinator::new(
            vec![
                agent_entry("agent-0"),
                agent_entry("agent-1"),
                agent_entry("agent-2"),
            ],
            vec![task],
            5,
        );
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-2".to_owned()), 0, 3);

        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![],
            coord,
            InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
        );

        let gossip = RuntimeMessage::Gossip {
            assignments: HashMap::from([(
                TaskId::from("task-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            )]),
            generations: HashMap::from([(AgentId::from("agent-1".to_owned()), 1)]),
        };
        node.apply_gossip_buffer(&[gossip]);

        let t = node
            .coordinator
            .registry
            .tasks()
            .find(|t| t.id == TaskId::from("task-0".to_owned()))
            .unwrap();
        // Local owner agent-2 has gen=3 > remote gen=1, so local wins
        assert_eq!(t.assigned_to, Some(AgentId::from("agent-2".to_owned())));
    }

    #[test]
    fn gossip_merge_same_owner_no_op() {
        let mut task = task_entry("task-0");
        task.status = TaskStatus::Assigned;
        task.assigned_to = Some(AgentId::from("agent-1".to_owned()));

        let mut coord = Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![task],
            5,
        );
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);

        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![],
            coord,
            InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
        );

        let gossip = RuntimeMessage::Gossip {
            assignments: HashMap::from([(
                TaskId::from("task-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            )]),
            generations: HashMap::from([(AgentId::from("agent-1".to_owned()), 1)]),
        };
        let (merged, _) = node.apply_gossip_buffer(&[gossip]);
        assert_eq!(merged, 0);
    }

    #[test]
    fn gossip_merge_updates_membership_generations() {
        let coord = Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        );

        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![],
            coord,
            InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
        );

        let gossip = RuntimeMessage::Gossip {
            assignments: HashMap::new(),
            generations: HashMap::from([
                (AgentId::from("agent-0".to_owned()), 1),
                (AgentId::from("agent-1".to_owned()), 5),
            ]),
        };
        node.apply_gossip_buffer(&[gossip]);

        let gen = node
            .coordinator
            .membership
            .generation_of(&AgentId::from("agent-1".to_owned()));
        assert_eq!(gen, 5);
    }

    #[test]
    fn duplicate_assignment_returns_err_not_panics() {
        let task_id = TaskId::from("task-0".to_owned());
        let mut coord = Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![task_entry("task-0")],
            5,
        );

        coord
            .registry
            .assign(&task_id, AgentId::from("agent-0".to_owned()))
            .unwrap();
        let result = coord
            .registry
            .assign(&task_id, AgentId::from("agent-1".to_owned()));
        assert!(result.is_err());
    }

    #[test]
    fn reordered_gossip_messages_produce_same_result() {
        let mut task = task_entry("task-0");
        task.status = TaskStatus::Assigned;
        task.assigned_to = Some(AgentId::from("agent-1".to_owned()));

        let make_node = || {
            let mut coord = Coordinator::new(
                vec![
                    agent_entry("agent-0"),
                    agent_entry("agent-1"),
                    agent_entry("agent-2"),
                ],
                vec![task.clone()],
                5,
            );
            coord
                .membership
                .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);
            coord
                .membership
                .record_heartbeat(&AgentId::from("agent-2".to_owned()), 0, 3);
            AgentNode::new(
                AgentId::from("agent-0".to_owned()),
                vec![],
                coord,
                InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
            )
        };

        let g1 = RuntimeMessage::Gossip {
            assignments: HashMap::from([(
                TaskId::from("task-0".to_owned()),
                AgentId::from("agent-2".to_owned()),
            )]),
            generations: HashMap::from([
                (AgentId::from("agent-1".to_owned()), 1),
                (AgentId::from("agent-2".to_owned()), 3),
            ]),
        };
        let g2 = RuntimeMessage::Gossip {
            assignments: HashMap::new(),
            generations: HashMap::from([(AgentId::from("agent-2".to_owned()), 3)]),
        };

        let mut node_a = make_node();
        node_a.apply_gossip_buffer(&[g1.clone(), g2.clone()]);

        let mut node_b = make_node();
        node_b.apply_gossip_buffer(&[g2, g1]);

        let owner_a = node_a
            .coordinator
            .registry
            .tasks()
            .find(|t| t.id == TaskId::from("task-0".to_owned()))
            .unwrap()
            .assigned_to
            .clone();
        let owner_b = node_b
            .coordinator
            .registry
            .tasks()
            .find(|t| t.id == TaskId::from("task-0".to_owned()))
            .unwrap()
            .assigned_to
            .clone();
        assert_eq!(owner_a, owner_b);
    }

    #[test]
    fn gossip_merge_ignores_dead_remote_owner() {
        let mut task = task_entry("task-0");
        task.status = TaskStatus::Unassigned;
        task.assigned_to = None;

        let mut coord = Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![task],
            5,
        );
        coord
            .membership
            .mark_dead(&AgentId::from("agent-1".to_owned()));

        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![],
            coord,
            InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
        );

        let gossip = RuntimeMessage::Gossip {
            assignments: HashMap::from([(
                TaskId::from("task-0".to_owned()),
                AgentId::from("agent-1".to_owned()),
            )]),
            generations: HashMap::from([(AgentId::from("agent-1".to_owned()), 1)]),
        };
        let (merged, stale) = node.apply_gossip_buffer(&[gossip]);
        assert_eq!(merged, 0);
        assert!(stale > 0);
    }

    #[test]
    fn gossip_merge_preserves_unrelated_tasks() {
        let mut task0 = task_entry("task-0");
        task0.status = TaskStatus::Assigned;
        task0.assigned_to = Some(AgentId::from("agent-1".to_owned()));

        let mut task1 = task_entry("task-1");
        task1.status = TaskStatus::Assigned;
        task1.assigned_to = Some(AgentId::from("agent-1".to_owned()));

        let mut coord = Coordinator::new(
            vec![
                agent_entry("agent-0"),
                agent_entry("agent-1"),
                agent_entry("agent-2"),
            ],
            vec![task0, task1],
            5,
        );
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-2".to_owned()), 0, 3);

        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![],
            coord,
            InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
        );

        // Gossip claims agent-2 (gen=3) owns task-0. Should override agent-1 (gen=1).
        // But task-1 should remain assigned to agent-1.
        let gossip = RuntimeMessage::Gossip {
            assignments: HashMap::from([(
                TaskId::from("task-0".to_owned()),
                AgentId::from("agent-2".to_owned()),
            )]),
            generations: HashMap::from([
                (AgentId::from("agent-1".to_owned()), 1),
                (AgentId::from("agent-2".to_owned()), 3),
            ]),
        };
        node.apply_gossip_buffer(&[gossip]);

        let t0 = node
            .coordinator
            .registry
            .tasks()
            .find(|t| t.id == TaskId::from("task-0".to_owned()))
            .unwrap();
        assert_eq!(t0.assigned_to, Some(AgentId::from("agent-2".to_owned())));

        let t1 = node
            .coordinator
            .registry
            .tasks()
            .find(|t| t.id == TaskId::from("task-1".to_owned()))
            .unwrap();
        assert_eq!(
            t1.assigned_to,
            Some(AgentId::from("agent-1".to_owned())),
            "unrelated task-1 should remain assigned to agent-1"
        );
    }
}
