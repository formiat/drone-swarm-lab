use std::collections::{HashMap, HashSet};

use chrono::Utc;
use swarm_alloc::{AllocationAgent, AllocationTask, Allocator, CbbaAllocator};
use swarm_comms::{AgentMissionState, Lease, LeaseId, RawMessage, Transport};
use swarm_types::{AgentId, Task, TaskId};

use crate::autonomy::{
    AgentAutonomyConfig, GcsLostPolicy, MothershipLostPolicy, NeighborLostPolicy,
    StateReconcileReport,
};
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
    // M93: autonomy FSM events
    /// True if GCS was declared lost this tick.
    pub gcs_lost_this_tick: bool,
    /// Policy name applied when entering GcsLost.
    pub gcs_lost_policy_name: Option<String>,
    /// True if GCS reconnected this tick.
    pub gcs_reconnected_this_tick: bool,
    /// Ticks GCS was unavailable before this reconnect.
    pub gcs_recovered_lost_ticks: u64,
    /// Report generated when GCS reconnects.
    pub reconcile_report: Option<StateReconcileReport>,
    /// Peers declared lost this tick.
    pub neighbors_lost_this_tick: Vec<AgentId>,
    /// `(lease_id, policy_applied)` when a lease expired during GCS loss.
    pub lease_expired_in_gcs_loss: Option<(String, String)>,
    /// Lease ID when transitioning to `ContinuingUnderLease`.
    pub continuing_under_lease_this_tick: Option<String>,
    /// True if mothership was declared lost this tick (per `MothershipLostPolicy`).
    pub mothership_lost_this_tick: bool,
}

pub struct NodeConfig {
    pub tick_duration_ms: u64,
    pub enable_movement: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveLeaseRecord {
    pub lease_id: LeaseId,
    pub resource_id: String,
    pub resource_kind: String,
    pub granted_tick: u64,
    pub expiry_tick: u64,
}

impl ActiveLeaseRecord {
    pub fn is_valid_at_tick(&self, current_tick: u64) -> bool {
        current_tick < self.expiry_tick
    }

    pub fn as_lease(&self, holder: &AgentId, tick_duration_ms: u64) -> Lease {
        let epoch =
            chrono::DateTime::<Utc>::from_timestamp_millis(0).expect("unix epoch must be valid");
        let granted_ms = self.granted_tick.saturating_mul(tick_duration_ms);
        let expiry_ms = self.expiry_tick.saturating_mul(tick_duration_ms);
        Lease {
            lease_id: self.lease_id.clone(),
            holder: holder.clone(),
            resource_id: self.resource_id.clone(),
            resource_kind: self.resource_kind.clone(),
            granted_at: epoch + chrono::Duration::milliseconds(granted_ms as i64),
            expires_at: epoch + chrono::Duration::milliseconds(expiry_ms as i64),
        }
    }
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
    // M93: autonomy FSM
    pub autonomy: AgentAutonomyConfig,
    /// Current mission FSM state tracked by the autonomy layer.
    pub mission_state: AgentMissionState,
    /// Agent ID treated as GCS; `None` disables GCS heartbeat monitoring.
    pub gcs_id: Option<AgentId>,
    last_gcs_heartbeat_tick: Option<u64>,
    /// Tick at which the current `ContinuingUnderLease` entry expires.
    continuing_lease_expiry_tick: Option<u64>,
    /// Mission state to restore when GCS reconnects after an autonomy override.
    pre_gcs_loss_mission_state: Option<AgentMissionState>,
    /// key: `AgentId`
    last_peer_heartbeat_ticks: HashMap<AgentId, u64>,
    gcs_lost_since_tick: Option<u64>,
    /// key: `AgentId` — peers already declared lost (reset when they reconnect).
    lost_peers_detected: HashSet<AgentId>,
    /// Active leases held by this agent.
    pub active_leases: Vec<ActiveLeaseRecord>,
    // M93: mothership FSM
    /// Agent ID treated as mothership; `None` disables mothership monitoring.
    pub mothership_id: Option<AgentId>,
    last_mothership_heartbeat_tick: Option<u64>,
    /// Mission state to restore if a waited-for peer reconnects in time.
    pre_neighbor_wait_mission_state: Option<AgentMissionState>,
    waiting_for_neighbor_reconnect: Option<(AgentId, u64)>,
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
            autonomy: AgentAutonomyConfig::default(),
            mission_state: AgentMissionState::Idle,
            gcs_id: None,
            last_gcs_heartbeat_tick: None,
            continuing_lease_expiry_tick: None,
            pre_gcs_loss_mission_state: None,
            last_peer_heartbeat_ticks: HashMap::new(),
            gcs_lost_since_tick: None,
            lost_peers_detected: HashSet::new(),
            active_leases: Vec::new(),
            mothership_id: None,
            last_mothership_heartbeat_tick: None,
            pre_neighbor_wait_mission_state: None,
            waiting_for_neighbor_reconnect: None,
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

        // M93: update peer heartbeat tracker (exclude GCS, mothership, and self)
        for (from, _, _) in &hb_list {
            let is_gcs = self.gcs_id.as_ref() == Some(from);
            let is_mothership = self.mothership_id.as_ref() == Some(from);
            if !is_gcs && !is_mothership && *from != self.own_id {
                self.last_peer_heartbeat_ticks
                    .insert(from.clone(), current_tick);
                // Clear the "declared lost" flag when peer reconnects
                self.lost_peers_detected.remove(from);
                if self
                    .waiting_for_neighbor_reconnect
                    .as_ref()
                    .is_some_and(|(peer_id, _)| peer_id == from)
                {
                    self.waiting_for_neighbor_reconnect = None;
                    if let Some(state) = self.pre_neighbor_wait_mission_state.take() {
                        self.mission_state = state;
                    } else {
                        self.mission_state = AgentMissionState::Idle;
                    }
                }
            }
        }

        // M93: mothership heartbeat tracking and FSM transitions
        let mut mothership_lost_this_tick = false;
        if let Some(mothership_id) = self.mothership_id.clone() {
            let ms_sent_hb = hb_list.iter().any(|(id, _, _)| *id == mothership_id);
            if ms_sent_hb {
                self.last_mothership_heartbeat_tick = Some(current_tick);
            } else {
                let timeout = mothership_timeout(&self.autonomy);
                let ticks_since = match self.last_mothership_heartbeat_tick {
                    Some(last) => current_tick.saturating_sub(last),
                    None => current_tick,
                };
                if ticks_since >= timeout {
                    match &self.autonomy.mothership_lost_policy {
                        MothershipLostPolicy::ProceedAutonomously => {
                            // Continue mission uninterrupted — no FSM change
                        }
                        MothershipLostPolicy::WaitAtStaging { .. }
                        | MothershipLostPolicy::ReturnToLaunch => {
                            if !mothership_lost_this_tick
                                && !matches!(self.mission_state, AgentMissionState::Aborting { .. })
                            {
                                mothership_lost_this_tick = true;
                                self.mission_state = AgentMissionState::Aborting {
                                    reason: "mothership_lost".to_owned(),
                                };
                            }
                        }
                    }
                }
            }
        }

        // M93: GCS heartbeat tracking and FSM transitions
        let mut gcs_lost_this_tick = false;
        let mut gcs_lost_policy_name: Option<String> = None;
        let mut gcs_reconnected_this_tick = false;
        let mut gcs_recovered_lost_ticks: u64 = 0;
        let mut reconcile_report: Option<StateReconcileReport> = None;
        let mut neighbors_lost_this_tick: Vec<AgentId> = Vec::new();
        let mut lease_expired_in_gcs_loss: Option<(String, String)> = None;
        let mut continuing_under_lease_this_tick: Option<String> = None;

        if let Some(gcs_id) = self.gcs_id.clone() {
            let gcs_sent_hb = hb_list.iter().any(|(id, _, _)| *id == gcs_id);

            if gcs_sent_hb {
                self.last_gcs_heartbeat_tick = Some(current_tick);
                let was_autonomous = matches!(
                    self.mission_state,
                    AgentMissionState::GcsLost { .. }
                        | AgentMissionState::ContinuingUnderLease { .. }
                );
                if was_autonomous {
                    let since = self.gcs_lost_since_tick.unwrap_or(current_tick);
                    let lost_ticks = current_tick.saturating_sub(since);
                    let active: Vec<Lease> = self
                        .active_leases
                        .iter()
                        .filter(|lease| lease.is_valid_at_tick(current_tick))
                        .map(|lease| lease.as_lease(&self.own_id, self.config.tick_duration_ms))
                        .collect();
                    gcs_reconnected_this_tick = true;
                    gcs_recovered_lost_ticks = lost_ticks;
                    reconcile_report = Some(StateReconcileReport {
                        agent_id: self.own_id.clone(),
                        gcs_lost_ticks: lost_ticks,
                        policy_applied: policy_name(&self.autonomy.gcs_lost_policy),
                        completed_resources: Vec::new(),
                        active_leases_at_reconnect: active,
                        mission_state_at_reconnect: self.mission_state.clone(),
                    });
                    self.mission_state = self
                        .pre_gcs_loss_mission_state
                        .take()
                        .unwrap_or(AgentMissionState::Idle);
                    self.gcs_lost_since_tick = None;
                    self.continuing_lease_expiry_tick = None;
                }
            } else {
                let ticks_since_gcs = match self.last_gcs_heartbeat_tick {
                    Some(last) => current_tick.saturating_sub(last),
                    None => current_tick,
                };
                let threshold = gcs_timeout(&self.autonomy);

                if ticks_since_gcs >= threshold {
                    match self.mission_state.clone() {
                        AgentMissionState::GcsLost { .. } => {
                            // Already declared lost — nothing new
                        }
                        AgentMissionState::ContinuingUnderLease { lease_id, .. } => {
                            let still_valid = self
                                .continuing_lease_expiry_tick
                                .map(|exp| current_tick < exp)
                                .unwrap_or(false);
                            if !still_valid {
                                let policy_str = policy_name(&self.autonomy.gcs_lost_policy);
                                lease_expired_in_gcs_loss =
                                    Some((lease_id.as_ref().to_owned(), policy_str.clone()));
                                self.mission_state = AgentMissionState::GcsLost {
                                    since_tick: self.gcs_lost_since_tick.unwrap_or(current_tick),
                                    policy_engaged: policy_str,
                                };
                                self.continuing_lease_expiry_tick = None;
                            }
                        }
                        _ => {
                            let policy_str = policy_name(&self.autonomy.gcs_lost_policy);
                            let active_lease = self
                                .active_leases
                                .iter()
                                .find(|lease| lease.is_valid_at_tick(current_tick))
                                .cloned();

                            if let Some(active_lease) = active_lease {
                                continuing_under_lease_this_tick =
                                    Some(active_lease.lease_id.as_ref().to_owned());
                                self.continuing_lease_expiry_tick = Some(active_lease.expiry_tick);
                                self.gcs_lost_since_tick = Some(current_tick);
                                if self.pre_gcs_loss_mission_state.is_none() {
                                    self.pre_gcs_loss_mission_state =
                                        Some(self.mission_state.clone());
                                }
                                self.mission_state = AgentMissionState::ContinuingUnderLease {
                                    lease_id: active_lease.lease_id.clone(),
                                    lease_expires_at: active_lease
                                        .as_lease(&self.own_id, self.config.tick_duration_ms)
                                        .expires_at,
                                };
                            } else {
                                gcs_lost_this_tick = true;
                                gcs_lost_policy_name = Some(policy_str.clone());
                                self.gcs_lost_since_tick = Some(current_tick);
                                if self.pre_gcs_loss_mission_state.is_none() {
                                    self.pre_gcs_loss_mission_state =
                                        Some(self.mission_state.clone());
                                }
                                self.mission_state = AgentMissionState::GcsLost {
                                    since_tick: current_tick,
                                    policy_engaged: policy_str,
                                };
                            }
                        }
                    }
                }
            }
        }

        if let Some((peer_id, until_tick)) = self.waiting_for_neighbor_reconnect.clone() {
            if current_tick >= until_tick {
                self.waiting_for_neighbor_reconnect = None;
                self.pre_neighbor_wait_mission_state = None;
                self.mission_state = AgentMissionState::Aborting {
                    reason: format!("neighbor_reconnect_timeout:{}", peer_id.as_ref()),
                };
            }
        }

        // M93: peer heartbeat timeout — detect neighbour loss
        let peer_timeout = self.autonomy.peer_heartbeat_timeout_ticks;
        for peer_id in self.peer_ids.clone() {
            let ticks_since = match self.last_peer_heartbeat_ticks.get(&peer_id) {
                Some(&last) => current_tick.saturating_sub(last),
                None => current_tick,
            };
            if ticks_since >= peer_timeout && !self.lost_peers_detected.contains(&peer_id) {
                self.lost_peers_detected.insert(peer_id.clone());
                neighbors_lost_this_tick.push(peer_id.clone());
                match &self.autonomy.neighbor_lost_policy {
                    NeighborLostPolicy::AbortMission => {
                        self.mission_state = AgentMissionState::Aborting {
                            reason: format!("neighbor_lost:{}", peer_id.as_ref()),
                        };
                    }
                    NeighborLostPolicy::ReleaseLocksAndContinue => {
                        // Releasing segment locks held by a *different* agent requires a
                        // shared cross-agent lease registry, which is out of scope for M93.
                        // The policy is correctly reflected in the FSM event (no abort).
                        // See Non-Goals in autonomy.rs.
                    }
                    NeighborLostPolicy::WaitForReconnect { max_ticks } => {
                        if self.waiting_for_neighbor_reconnect.is_none() {
                            self.pre_neighbor_wait_mission_state = Some(self.mission_state.clone());
                            self.waiting_for_neighbor_reconnect =
                                Some((peer_id.clone(), current_tick.saturating_add(*max_ticks)));
                            self.mission_state = AgentMissionState::WaitingForNeighborReconnect {
                                neighbor_id: peer_id.clone(),
                                since_tick: current_tick,
                                until_tick: current_tick.saturating_add(*max_ticks),
                            };
                        }
                    }
                }
            }
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
            gcs_lost_this_tick,
            gcs_lost_policy_name,
            gcs_reconnected_this_tick,
            gcs_recovered_lost_ticks,
            reconcile_report,
            neighbors_lost_this_tick,
            lease_expired_in_gcs_loss,
            continuing_under_lease_this_tick,
            mothership_lost_this_tick,
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

/// Returns the human-readable name for a `GcsLostPolicy` variant.
fn policy_name(policy: &GcsLostPolicy) -> String {
    match policy {
        GcsLostPolicy::ContinueMission { .. } => "continue_mission".to_owned(),
        GcsLostPolicy::HoverInPlace { .. } => "hover_in_place".to_owned(),
        GcsLostPolicy::ReturnToLaunch { .. } => "return_to_launch".to_owned(),
        GcsLostPolicy::AbortImmediate => "abort_immediate".to_owned(),
    }
}

/// Returns the tick threshold after which GCS is declared lost.
fn gcs_timeout(autonomy: &AgentAutonomyConfig) -> u64 {
    match &autonomy.gcs_lost_policy {
        GcsLostPolicy::ReturnToLaunch { after_ticks } => *after_ticks,
        GcsLostPolicy::AbortImmediate => 1,
        _ => autonomy.gcs_heartbeat_timeout_ticks,
    }
}

/// Returns the tick threshold after which the mothership is declared lost.
fn mothership_timeout(autonomy: &AgentAutonomyConfig) -> u64 {
    match &autonomy.mothership_lost_policy {
        MothershipLostPolicy::WaitAtStaging { max_ticks } => *max_ticks,
        MothershipLostPolicy::ReturnToLaunch => 1,
        MothershipLostPolicy::ProceedAutonomously => u64::MAX,
    }
}
