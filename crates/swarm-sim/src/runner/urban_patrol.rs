use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use swarm_comms::{
    DeconflictionMode, InMemAgentTransport, InMemNetwork, NetworkConfig, RawMessage,
    SegmentDenyReason, SwarmMessage, SwarmMessageEnvelope, Transport,
    SWARM_PROTOCOL_SCHEMA_VERSION,
};
use swarm_types::{
    AgentId, Pose, UrbanBlockedPolicy, UrbanEdgeId, UrbanNodeId, UrbanPlannedRoute,
    UrbanRightOfWayPolicy,
};

use super::{
    advance_urban_analysis_agent, current_urban_pose, finish_urban_run_metrics,
    push_segment_conflict, push_segment_entered, push_segment_lock_acquired,
    push_segment_lock_released, push_urban_analysis_agent_started, push_urban_violation_event,
    route_efficiency, speed_m_per_tick, urban_analysis_agent_states, urban_patrol_metrics, Agent,
    FailureEvent, Health, RunConfig, RunMetrics, Scenario, ScenarioRunner,
};

/// Transient state for blocked-route decision logic during a patrol run.
struct BlockedRouteState {
    /// Edge the agent is currently waiting for, if any.
    waiting_for: Option<UrbanEdgeId>,
    /// Tick at which the current wait started.
    wait_start_tick: u64,
    /// Accumulated ticks spent waiting.
    wait_ticks: u64,
    /// Number of times a blocked edge was detected ahead.
    blocked_edge_detections: u64,
    /// Number of times the route was successfully replanned.
    replan_count: u64,
    /// Number of blockages that could not be resolved (abort/no-route).
    unresolved_blockages: u64,
}

impl BlockedRouteState {
    fn new() -> Self {
        Self {
            waiting_for: None,
            wait_start_tick: 0,
            wait_ticks: 0,
            blocked_edge_detections: 0,
            replan_count: 0,
            unresolved_blockages: 0,
        }
    }

    fn is_waiting(&self) -> bool {
        self.waiting_for.is_some()
    }

    fn replan_success_rate(&self) -> f64 {
        if self.blocked_edge_detections == 0 {
            return 0.0;
        }
        self.replan_count as f64 / self.blocked_edge_detections as f64
    }
}

/// Per-agent Urban patrol state used only by M85 deconfliction runs.
struct DeconflictedAgentState {
    agent_id: AgentId,
    route: UrbanPlannedRoute,
    segment_index: usize,
    distance_on_segment: f64,
    speed_m_per_tick: f64,
    completed: bool,
    aborted: bool,
    waiting_for: Option<UrbanEdgeId>,
    wait_start_tick: u64,
    wait_ticks: u64,
    total_distance_travelled_m: f64,
    replan_count: u64,
    unresolved_blockages: u64,
    completion_tick: Option<u64>,
    failed: bool,
    handoff_target: Option<AgentId>,
}

impl DeconflictedAgentState {
    fn new(agent: &Agent, route: &UrbanPlannedRoute, tick_duration_ms: u64) -> Self {
        Self {
            agent_id: agent.id.clone(),
            route: route.clone(),
            segment_index: 0,
            distance_on_segment: 0.0,
            speed_m_per_tick: speed_m_per_tick(agent, tick_duration_ms),
            completed: route.segments.is_empty(),
            aborted: false,
            waiting_for: None,
            wait_start_tick: 0,
            wait_ticks: 0,
            total_distance_travelled_m: 0.0,
            replan_count: 0,
            unresolved_blockages: 0,
            completion_tick: route.segments.is_empty().then_some(0),
            failed: false,
            handoff_target: None,
        }
    }

    fn active(&self) -> bool {
        !self.completed && !self.aborted && !self.failed
    }

    fn current_edge_id(&self) -> Option<&UrbanEdgeId> {
        self.route
            .segments
            .get(self.segment_index)
            .map(|segment| &segment.edge_id)
    }
}

struct NetworkSegmentRuntime {
    coordinator: crate::urban::SegmentCoordinator<InMemAgentTransport>,
    /// key: `agent_id`
    agent_transports: HashMap<AgentId, InMemAgentTransport>,
    coordinator_id: AgentId,
    policy: UrbanRightOfWayPolicy,
    /// key: `agent_id`
    priorities: HashMap<AgentId, u8>,
    /// key: `edge_id`
    round_robin_next: HashMap<UrbanEdgeId, usize>,
    conflict_history: Vec<crate::urban::UrbanSegmentConflictRecord>,
}

impl NetworkSegmentRuntime {
    fn new(
        agent_ids: Vec<AgentId>,
        coordinator_id: AgentId,
        policy: UrbanRightOfWayPolicy,
        priorities: HashMap<AgentId, u8>,
    ) -> Self {
        let bus = Rc::new(RefCell::new(InMemNetwork::new(NetworkConfig {
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            latency_per_hop: 0,
            seed: 95,
            partitions: HashSet::new(),
            comms_jitter_ticks: 0,
        })));
        let coordinator_transport = InMemAgentTransport::new(bus.clone(), coordinator_id.clone());
        let coordinator = crate::urban::SegmentCoordinator::new(
            coordinator_id.clone(),
            coordinator_transport,
            policy.clone(),
            priorities.clone(),
        );
        let agent_transports = agent_ids
            .into_iter()
            .map(|agent_id| {
                (
                    agent_id.clone(),
                    InMemAgentTransport::new(bus.clone(), agent_id),
                )
            })
            .collect();
        Self {
            coordinator,
            agent_transports,
            coordinator_id,
            policy,
            priorities,
            round_robin_next: HashMap::new(),
            conflict_history: Vec::new(),
        }
    }

    fn request_batch(
        &mut self,
        requests: Vec<crate::urban::SegmentLockRequest>,
        tick: u64,
        log_builder: Option<&mut swarm_replay::EventLogBuilder>,
    ) -> Vec<(AgentId, crate::urban::SegmentLockDecision)> {
        let mut log_builder = log_builder;
        let mut request_segments = HashMap::<(AgentId, UrbanEdgeId), usize>::new();
        for request in self.order_requests(requests) {
            request_segments.insert(
                (request.agent_id.clone(), request.edge_id.clone()),
                request.segment_index,
            );
            self.send_agent_message(
                &request.agent_id,
                tick,
                SwarmMessage::SegmentReserve {
                    edge_id: request.edge_id,
                    segment_index: request.segment_index,
                    requester: request.agent_id.clone(),
                    request_tick: tick,
                },
                "segment_reserve",
                log_builder.as_deref_mut(),
            );
        }

        let events = self
            .coordinator
            .handle_incoming(tick)
            .unwrap_or_else(|error| match error {
                crate::urban::SegmentCoordinatorError::Transport(error) => match error {},
            });
        for event in events {
            self.push_coordinator_event(tick, &event, log_builder.as_deref_mut());
        }

        let mut decisions = Vec::new();
        let agent_ids = self
            .agent_transports
            .keys()
            .cloned()
            .collect::<Vec<AgentId>>();
        for agent_id in agent_ids {
            while let Some(raw) = self.poll_agent(&agent_id) {
                let Some(envelope) = SwarmMessageEnvelope::from_raw_message(&raw) else {
                    continue;
                };
                match envelope.message {
                    SwarmMessage::SegmentGrant {
                        edge_id,
                        to,
                        lease: _,
                    } => {
                        self.push_protocol_event(
                            tick,
                            &raw,
                            "segment_grant",
                            log_builder.as_deref_mut(),
                        );
                        let segment_index = request_segments
                            .get(&(to.clone(), edge_id.clone()))
                            .copied()
                            .unwrap_or(0);
                        let lock = crate::urban::UrbanSegmentLock {
                            edge_id,
                            holder_agent_id: to.clone(),
                            acquired_at_tick: tick,
                            segment_index,
                        };
                        decisions.push((to, crate::urban::SegmentLockDecision::Acquired(lock)));
                    }
                    SwarmMessage::SegmentDeny {
                        edge_id,
                        to,
                        holder,
                        reason,
                    } => {
                        self.push_protocol_event(
                            tick,
                            &raw,
                            "segment_deny",
                            log_builder.as_deref_mut(),
                        );
                        let conflict = crate::urban::UrbanSegmentConflictRecord {
                            tick,
                            edge_id,
                            holder_agent_id: holder,
                            requester_agent_id: to.clone(),
                            policy: self.policy.clone(),
                            reason: segment_deny_reason(reason),
                        };
                        self.conflict_history.push(conflict.clone());
                        decisions.push((to, crate::urban::SegmentLockDecision::Conflict(conflict)));
                    }
                    _ => {}
                }
            }
        }
        decisions.sort_by(|left, right| left.0.as_ref().cmp(right.0.as_ref()));
        decisions
    }

    fn order_requests(
        &mut self,
        mut requests: Vec<crate::urban::SegmentLockRequest>,
    ) -> Vec<crate::urban::SegmentLockRequest> {
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

        let mut grouped: Vec<(UrbanEdgeId, Vec<crate::urban::SegmentLockRequest>)> = Vec::new();
        for request in requests {
            if let Some((edge_id, group)) = grouped.last_mut() {
                if edge_id == &request.edge_id {
                    group.push(request);
                    continue;
                }
            }
            grouped.push((request.edge_id.clone(), vec![request]));
        }

        let mut ordered = Vec::new();
        for (edge_id, mut group) in grouped {
            if self.is_edge_locked(&edge_id) || group.len() <= 1 {
                ordered.extend(group);
                continue;
            }

            let winner_index = self.winner_index(&edge_id, &group);
            if matches!(self.policy, UrbanRightOfWayPolicy::RoundRobin) {
                let next = (winner_index + 1) % group.len().max(1);
                self.round_robin_next.insert(edge_id, next);
            }
            let winner = group.remove(winner_index);
            ordered.push(winner);
            ordered.extend(group);
        }
        ordered
    }

    fn winner_index(
        &self,
        edge_id: &UrbanEdgeId,
        group: &[crate::urban::SegmentLockRequest],
    ) -> usize {
        match self.policy {
            UrbanRightOfWayPolicy::FirstCome | UrbanRightOfWayPolicy::MissionCriticalOverride => 0,
            UrbanRightOfWayPolicy::Priority => group
                .iter()
                .enumerate()
                .max_by(|(_, left), (_, right)| {
                    let left_priority = self.priorities.get(&left.agent_id).copied().unwrap_or(0);
                    let right_priority = self.priorities.get(&right.agent_id).copied().unwrap_or(0);
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

    fn release(
        &mut self,
        edge_id: &UrbanEdgeId,
        agent_id: &AgentId,
        tick: u64,
        log_builder: Option<&mut swarm_replay::EventLogBuilder>,
    ) -> Option<crate::urban::UrbanSegmentLock> {
        let mut log_builder = log_builder;
        let (lock, lease) = self
            .coordinator
            .active_locks()
            .find(|(lock, _)| &lock.edge_id == edge_id && &lock.holder_agent_id == agent_id)
            .cloned()?;
        self.send_agent_message(
            agent_id,
            tick,
            SwarmMessage::SegmentRelease {
                edge_id: edge_id.clone(),
                lease_id: lease.lease_id,
            },
            "segment_release",
            log_builder.as_deref_mut(),
        );
        let released = self
            .coordinator
            .handle_incoming(tick)
            .ok()
            .into_iter()
            .flatten()
            .any(|event| {
                self.push_coordinator_event(tick, &event, log_builder.as_deref_mut());
                matches!(
                    event,
                    crate::urban::CoordinatorEvent::Released {
                        ref edge_id,
                        ref agent_id
                    } if edge_id == &lock.edge_id && agent_id == &lock.holder_agent_id
                )
            });
        released.then_some(lock)
    }

    fn is_locked_by_other(&self, edge_id: &UrbanEdgeId, agent_id: &AgentId) -> bool {
        self.coordinator
            .active_locks()
            .any(|(lock, _)| &lock.edge_id == edge_id && &lock.holder_agent_id != agent_id)
    }

    fn is_edge_locked(&self, edge_id: &UrbanEdgeId) -> bool {
        self.coordinator
            .active_locks()
            .any(|(lock, _)| &lock.edge_id == edge_id)
    }

    fn locked_edges_except(&self, agent_id: &AgentId) -> Vec<UrbanEdgeId> {
        self.coordinator
            .active_locks()
            .filter(|(lock, _)| &lock.holder_agent_id != agent_id)
            .map(|(lock, _)| lock.edge_id.clone())
            .collect()
    }

    fn conflict_count(&self) -> u64 {
        self.conflict_history.len() as u64
    }

    fn send_agent_message(
        &mut self,
        agent_id: &AgentId,
        tick: u64,
        message: SwarmMessage,
        kind: &str,
        log_builder: Option<&mut swarm_replay::EventLogBuilder>,
    ) {
        let envelope = SwarmMessageEnvelope {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
            envelope_id: format!("urban-{kind}-{}-{tick}", agent_id.as_ref()),
            correlation_id: None,
            from: agent_id.clone(),
            to: self.coordinator_id.clone(),
            sent_at: chrono::Utc::now(),
            ttl_ticks: 10,
            message,
        };
        let raw = envelope.into_raw_message();
        if let Some(transport) = self.agent_transports.get_mut(agent_id) {
            let _ = transport.send(raw.clone());
        }
        self.push_protocol_event(tick, &raw, kind, log_builder);
    }

    fn poll_agent(&mut self, agent_id: &AgentId) -> Option<RawMessage> {
        self.agent_transports
            .get_mut(agent_id)
            .and_then(|transport| transport.poll().ok())
            .flatten()
    }

    fn push_protocol_event(
        &self,
        tick: u64,
        raw: &RawMessage,
        kind: &str,
        log_builder: Option<&mut swarm_replay::EventLogBuilder>,
    ) {
        if let Some(builder) = log_builder {
            builder.push(swarm_replay::Event::SwarmProtocolMessage {
                tick,
                from: raw.from.clone(),
                to: raw.to.clone(),
                envelope_id: format!("urban-{kind}-{}-{}", raw.from.as_ref(), tick),
                kind: kind.to_owned(),
            });
        }
    }

    fn push_coordinator_event(
        &self,
        tick: u64,
        event: &crate::urban::CoordinatorEvent,
        log_builder: Option<&mut swarm_replay::EventLogBuilder>,
    ) {
        let Some(builder) = log_builder else {
            return;
        };
        let (edge_id, agent_id, event, reason) = match event {
            crate::urban::CoordinatorEvent::GrantSent { edge_id, to } => {
                (edge_id.clone(), to.clone(), "grant_sent".to_owned(), None)
            }
            crate::urban::CoordinatorEvent::DenySent {
                edge_id,
                to,
                reason,
            } => (
                edge_id.clone(),
                to.clone(),
                "deny_sent".to_owned(),
                Some(segment_deny_reason(reason.clone())),
            ),
            crate::urban::CoordinatorEvent::Released { edge_id, agent_id } => (
                edge_id.clone(),
                agent_id.clone(),
                "released".to_owned(),
                None,
            ),
            crate::urban::CoordinatorEvent::LeaseExpired { edge_id, agent_id } => (
                edge_id.clone(),
                agent_id.clone(),
                "lease_expired".to_owned(),
                None,
            ),
        };
        builder.push(swarm_replay::Event::UrbanSegmentCoordinatorEvent {
            tick,
            edge_id,
            agent_id,
            event,
            reason,
        });
    }
}

fn segment_deny_reason(reason: SegmentDenyReason) -> String {
    match reason {
        SegmentDenyReason::AlreadyHeld => "segment already locked".to_owned(),
        SegmentDenyReason::PolicyDenied => "right-of-way policy denied request".to_owned(),
        SegmentDenyReason::CoordinatorUnavailable => "coordinator unavailable".to_owned(),
    }
}

fn is_segment_locked_by_other(
    network: Option<&NetworkSegmentRuntime>,
    registry: &crate::urban::UrbanSegmentLockRegistry,
    edge_id: &UrbanEdgeId,
    agent_id: &AgentId,
) -> bool {
    network
        .map(|network| network.is_locked_by_other(edge_id, agent_id))
        .unwrap_or_else(|| registry.is_locked_by_other(edge_id, agent_id))
}

fn locked_edges_except(
    network: Option<&NetworkSegmentRuntime>,
    registry: &crate::urban::UrbanSegmentLockRegistry,
    agent_id: &AgentId,
) -> Vec<UrbanEdgeId> {
    network
        .map(|network| network.locked_edges_except(agent_id))
        .unwrap_or_else(|| registry.locked_edges_except(agent_id))
}

struct DeconflictedFailureContext<'a> {
    tick: u64,
    failures: &'a [FailureEvent],
    registry: &'a mut crate::urban::UrbanSegmentLockRegistry,
    map: &'a swarm_types::UrbanMap,
    planner_mode: crate::urban::UrbanPlannerMode,
    effective_blocked: &'a HashSet<UrbanEdgeId>,
    network_runtime: Option<&'a mut NetworkSegmentRuntime>,
    log_builder: Option<&'a mut swarm_replay::EventLogBuilder>,
}

fn process_deconflicted_failures(
    states: &mut [DeconflictedAgentState],
    context: DeconflictedFailureContext<'_>,
) {
    let DeconflictedFailureContext {
        tick,
        failures,
        registry,
        map,
        planner_mode,
        effective_blocked,
        network_runtime,
        log_builder,
    } = context;
    let mut network_runtime = network_runtime;
    let mut log_builder = log_builder;
    let failed_indices = failures
        .iter()
        .filter(|failure| failure.at_tick == tick)
        .filter_map(|failure| {
            states
                .iter()
                .position(|state| state.agent_id == failure.agent_id && !state.failed)
        })
        .collect::<Vec<_>>();

    for failed_index in failed_indices {
        let failed_agent_id = states[failed_index].agent_id.clone();
        let remaining_route = remaining_route_from_state(&states[failed_index]);
        let held_edge_id = states[failed_index].current_edge_id().cloned();
        states[failed_index].failed = true;
        states[failed_index].waiting_for = None;

        if let Some(ref mut builder) = log_builder {
            builder.push(swarm_replay::Event::AgentFailed {
                agent_id: failed_agent_id.clone(),
                tick,
            });
        }

        let mut released_failure_resource_id = None;
        if let Some(edge_id) = held_edge_id {
            let released_lock = if let Some(runtime) = network_runtime.as_deref_mut() {
                runtime.release(&edge_id, &failed_agent_id, tick, log_builder.as_deref_mut())
            } else {
                registry.release(&edge_id, &failed_agent_id, tick)
            };
            if let Some(lock) = released_lock {
                released_failure_resource_id = Some(lock.edge_id.to_string());
                if let Some(ref mut builder) = log_builder {
                    push_segment_lock_released(builder, &lock, tick);
                    builder.push(swarm_replay::Event::SwarmOwnershipReleased {
                        tick,
                        agent_id: failed_agent_id.clone(),
                        ownership_kind: "urban_segment".to_owned(),
                        resource_id: lock.edge_id.to_string(),
                        reason: "agent_failed".to_owned(),
                    });
                }
            }
        }

        if remaining_route.segments.is_empty() {
            continue;
        }
        let Some(resource_id) = released_failure_resource_id else {
            if let (Some(ref mut builder), Some(segment)) =
                (log_builder.as_deref_mut(), remaining_route.segments.first())
            {
                builder.push(swarm_replay::Event::UrbanDeconflictAbort {
                    agent_id: failed_agent_id.clone(),
                    tick,
                    edge_id: segment.edge_id.clone(),
                    reason: "failure handoff requires agent_failed ownership release".to_owned(),
                });
            }
            continue;
        };
        let Some(target_index) = select_failure_handoff_target(states, failed_index) else {
            if let (Some(ref mut builder), Some(edge_id)) =
                (log_builder.as_deref_mut(), remaining_route.segments.first())
            {
                builder.push(swarm_replay::Event::UrbanDeconflictAbort {
                    agent_id: failed_agent_id.clone(),
                    tick,
                    edge_id: edge_id.edge_id.clone(),
                    reason: "no reserve agent available for failure handoff".to_owned(),
                });
            }
            continue;
        };

        let target_agent_id = states[target_index].agent_id.clone();
        let replacement_route = match connected_replacement_route(
            map,
            &states[target_index],
            remaining_route,
            planner_mode,
            effective_blocked,
        ) {
            Ok(route) => route,
            Err((edge_id, reason)) => {
                if let Some(ref mut builder) = log_builder {
                    builder.push(swarm_replay::Event::UrbanDeconflictAbort {
                        agent_id: target_agent_id,
                        tick,
                        edge_id,
                        reason,
                    });
                }
                continue;
            }
        };
        append_replacement_route(&mut states[target_index], replacement_route);
        states[failed_index].handoff_target = Some(target_agent_id.clone());

        if let Some(ref mut builder) = log_builder {
            builder.push(swarm_replay::Event::SwarmOwnershipHandoff {
                tick,
                from_agent_id: failed_agent_id.clone(),
                to_agent_id: target_agent_id.clone(),
                ownership_kind: "urban_segment".to_owned(),
                resource_id,
                reason: "agent_failed".to_owned(),
            });
            builder.push(swarm_replay::Event::UrbanRoutePlanned {
                agent_id: target_agent_id,
                tick,
                edge_ids: states[target_index]
                    .route
                    .segments
                    .iter()
                    .map(|segment| segment.edge_id.clone())
                    .collect(),
                route_length_m: states[target_index].route.total_length_m,
            });
        }
    }
}

fn remaining_route_from_state(state: &DeconflictedAgentState) -> UrbanPlannedRoute {
    let segments = state
        .route
        .segments
        .get(state.segment_index..)
        .unwrap_or_default()
        .to_vec();
    route_from_segments(segments)
}

fn connected_replacement_route(
    map: &swarm_types::UrbanMap,
    target: &DeconflictedAgentState,
    replacement: UrbanPlannedRoute,
    planner: crate::urban::UrbanPlannerMode,
    effective_blocked: &HashSet<UrbanEdgeId>,
) -> Result<UrbanPlannedRoute, (UrbanEdgeId, String)> {
    let Some(first_replacement) = replacement.segments.first() else {
        return Ok(replacement);
    };
    let first_replacement_edge_id = first_replacement.edge_id.clone();
    let first_replacement_from = first_replacement.from.clone();
    let Some(target_end) = route_end_node(target) else {
        return Err((
            first_replacement_edge_id,
            "failure handoff target has no route anchor".to_owned(),
        ));
    };
    if target_end == first_replacement_from {
        return Ok(replacement);
    }

    let bridge = crate::urban::plan_route_excluding(
        map,
        &target_end,
        &first_replacement_from,
        effective_blocked,
        planner,
    )
    .map_err(|error| {
        (
            first_replacement_edge_id.clone(),
            format!("no bridge route for failure handoff: {error}"),
        )
    })?;
    let segments = bridge
        .segments
        .into_iter()
        .chain(replacement.segments)
        .collect::<Vec<_>>();
    if let Some((left, right)) = first_disconnected_pair(&segments) {
        return Err((
            right.edge_id.clone(),
            format!(
                "failure handoff replacement route is disconnected: {} -> {}",
                left.edge_id, right.edge_id
            ),
        ));
    }
    let route = route_from_segments(segments);
    if let Some(blocked_edge) = route
        .segments
        .iter()
        .find(|segment| effective_blocked.contains(&segment.edge_id))
    {
        return Err((
            blocked_edge.edge_id.clone(),
            "failure handoff bridge uses blocked edge".to_owned(),
        ));
    }
    if let Some(violation) = crate::urban::judge_route(map, &route).first() {
        return Err((
            route
                .segments
                .first()
                .map(|segment| segment.edge_id.clone())
                .unwrap_or(first_replacement_edge_id),
            format!("failure handoff bridge violates urban map: {violation:?}"),
        ));
    }
    Ok(route)
}

fn route_end_node(state: &DeconflictedAgentState) -> Option<UrbanNodeId> {
    state
        .route
        .segments
        .last()
        .map(|segment| segment.to.clone())
}

fn first_disconnected_pair(
    segments: &[swarm_types::UrbanRouteSegment],
) -> Option<(
    &swarm_types::UrbanRouteSegment,
    &swarm_types::UrbanRouteSegment,
)> {
    segments.windows(2).find_map(|pair| {
        let left = &pair[0];
        let right = &pair[1];
        (left.to != right.from).then_some((left, right))
    })
}

fn append_replacement_route(state: &mut DeconflictedAgentState, replacement: UrbanPlannedRoute) {
    if replacement.segments.is_empty() {
        return;
    }
    let old_len = state.route.segments.len();
    state.route.segments.extend(replacement.segments);
    state.route.total_length_m = state
        .route
        .segments
        .iter()
        .map(|segment| segment.length_m)
        .sum();
    state.route.total_cost = state
        .route
        .segments
        .iter()
        .map(|segment| segment.cost)
        .sum();
    if state.completed {
        state.completed = false;
        state.completion_tick = None;
        state.segment_index = old_len;
        state.distance_on_segment = 0.0;
    }
}

fn select_failure_handoff_target(
    states: &[DeconflictedAgentState],
    failed_index: usize,
) -> Option<usize> {
    states
        .iter()
        .enumerate()
        .filter(|(index, state)| {
            *index != failed_index
                && !state.failed
                && !state.aborted
                && state.handoff_target.is_none()
        })
        .min_by(|(_, left), (_, right)| {
            (u8::from(!left.completed), left.agent_id.to_string())
                .cmp(&(u8::from(!right.completed), right.agent_id.to_string()))
        })
        .map(|(index, _)| index)
}

fn route_from_segments(segments: Vec<swarm_types::UrbanRouteSegment>) -> UrbanPlannedRoute {
    UrbanPlannedRoute {
        total_length_m: segments.iter().map(|segment| segment.length_m).sum(),
        total_cost: segments.iter().map(|segment| segment.cost).sum(),
        segments,
    }
}

fn split_route_for_agent(
    route: &UrbanPlannedRoute,
    agent_index: usize,
    agent_count: usize,
) -> UrbanPlannedRoute {
    if agent_count <= 1 || route.segments.is_empty() {
        return route.clone();
    }
    let base_len = route.segments.len() / agent_count;
    let extra = route.segments.len() % agent_count;
    let start = agent_index * base_len + agent_index.min(extra);
    let len = base_len + usize::from(agent_index < extra);
    let end = start.saturating_add(len).min(route.segments.len());
    let segments = if start < end {
        route.segments[start..end].to_vec()
    } else {
        Vec::new()
    };
    let total_length_m = segments.iter().map(|segment| segment.length_m).sum();
    let total_cost = segments.iter().map(|segment| segment.cost).sum();
    UrbanPlannedRoute {
        segments,
        total_length_m,
        total_cost,
    }
}

fn first_route_node_pose<'a>(
    map: &'a swarm_types::UrbanMap,
    route: &UrbanPlannedRoute,
) -> Option<&'a Pose> {
    let first_node = &route.segments.first()?.from;
    map.nodes
        .iter()
        .find(|node| &node.id == first_node)
        .map(|node| &node.pose)
}

impl ScenarioRunner {
    pub(super) fn run_urban_patrol(
        scenario: &Scenario,
        config: RunConfig,
        mut log_builder: Option<swarm_replay::EventLogBuilder>,
    ) -> (RunMetrics, Option<swarm_replay::EventLog>) {
        let Some(urban_state) = config.urban_state.clone() else {
            unreachable!("run_urban_patrol is called only for urban_state runs");
        };
        if urban_state.deconfliction.enabled {
            return Self::run_urban_deconflicted_patrol(scenario, config, urban_state, log_builder);
        }
        let initial_route = match crate::urban::expand_route_loop_with_planner_name(
            &urban_state.map,
            &urban_state.route_loop,
            &urban_state.planner,
        ) {
            Ok(route) => route,
            Err(error) => {
                return (
                    urban_patrol_metrics(
                        scenario,
                        0,
                        false,
                        false,
                        0.0,
                        0.0,
                        1,
                        false,
                        None,
                        0.0,
                        0.0,
                        Some(error.to_string()),
                        0,
                        0,
                        0,
                        0.0,
                        0,
                    ),
                    log_builder.map(|builder| builder.build()),
                );
            }
        };

        let Some(agent) = scenario
            .agents
            .iter()
            .find(|agent| agent.health == Health::Alive)
        else {
            return (
                urban_patrol_metrics(
                    scenario,
                    0,
                    false,
                    true,
                    initial_route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &initial_route),
                    0,
                    false,
                    None,
                    0.0,
                    0.0,
                    Some("urban_patrol_no_alive_agent".to_owned()),
                    0,
                    0,
                    0,
                    0.0,
                    0,
                ),
                log_builder.map(|builder| builder.build()),
            );
        };
        let agent_id = agent.id.clone();
        let start_node = match crate::urban::route_start_node(
            &urban_state.map,
            &urban_state.route_loop,
            &initial_route,
            urban_state.start_node.as_ref(),
        ) {
            Ok(start_node) => start_node,
            Err(error) => {
                return (
                    urban_patrol_metrics(
                        scenario,
                        0,
                        false,
                        true,
                        initial_route.total_length_m,
                        crate::urban::route_risk_score(&urban_state.map, &initial_route),
                        0,
                        false,
                        None,
                        0.0,
                        0.0,
                        Some(format!("urban_patrol_invalid_start: {error}")),
                        0,
                        0,
                        0,
                        0.0,
                        0,
                    ),
                    log_builder.map(|builder| builder.build()),
                );
            }
        };
        let start_pose_distance = agent.pose.distance_to(&start_node.pose);
        if start_pose_distance > crate::urban::URBAN_START_POSE_TOLERANCE_M {
            return (
                urban_patrol_metrics(
                    scenario,
                    0,
                    false,
                    true,
                    initial_route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &initial_route),
                    0,
                    false,
                    None,
                    0.0,
                    0.0,
                    Some(format!(
                        "urban_patrol_invalid_start: agent '{}' starts {:.3}m from start_node '{}'",
                        agent.id, start_pose_distance, start_node.id
                    )),
                    0,
                    0,
                    0,
                    0.0,
                    0,
                ),
                log_builder.map(|builder| builder.build()),
            );
        }

        let mut analysis_agent_states = urban_analysis_agent_states(
            scenario,
            &agent_id,
            start_node.pose,
            config.tick_duration_ms,
        );

        if let Some(ref mut builder) = log_builder {
            builder.push(swarm_replay::Event::UrbanRoutePlanned {
                agent_id: agent_id.clone(),
                tick: 0,
                edge_ids: initial_route
                    .segments
                    .iter()
                    .map(|segment| segment.edge_id.clone())
                    .collect(),
                route_length_m: initial_route.total_length_m,
            });
            builder.push(swarm_replay::Event::PoseUpdated {
                agent_id: agent_id.clone(),
                pose: start_node.pose,
                tick: 0,
            });
            for state in &analysis_agent_states {
                push_urban_analysis_agent_started(builder, state, &urban_state.map, &initial_route);
            }
        }

        let static_violations = crate::urban::judge_route(&urban_state.map, &initial_route);
        if !static_violations.is_empty() {
            if let Some(ref mut builder) = log_builder {
                for violation in &static_violations {
                    push_urban_violation_event(builder, &agent_id, 0, &initial_route, violation);
                }
            }
            return (
                urban_patrol_metrics(
                    scenario,
                    0,
                    false,
                    true,
                    initial_route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &initial_route),
                    static_violations.len() as u64,
                    false,
                    None,
                    0.0,
                    0.0,
                    None,
                    0,
                    0,
                    0,
                    0.0,
                    0,
                ),
                log_builder.map(|builder| builder.build()),
            );
        }

        let planner_mode = crate::urban::UrbanPlannerMode::parse(&urban_state.planner)
            .unwrap_or(crate::urban::UrbanPlannerMode::Dijkstra);
        let initial_route_length_m = initial_route.total_length_m;
        let initial_route_risk = crate::urban::route_risk_score(&urban_state.map, &initial_route);
        let perimeter_length_m = match urban_state.perimeter_patrol.as_ref() {
            Some(perimeter) => {
                match crate::urban::perimeter_waypoints(&perimeter.polygon, perimeter.spacing_m) {
                    Ok(waypoints) => Some(perimeter_length_m(&waypoints)),
                    Err(error) => {
                        return (
                            urban_patrol_metrics(
                                scenario,
                                0,
                                false,
                                true,
                                initial_route_length_m,
                                initial_route_risk,
                                0,
                                false,
                                None,
                                0.0,
                                0.0,
                                Some(format!("urban_perimeter_invalid: {error}")),
                                0,
                                0,
                                0,
                                0.0,
                                0,
                            ),
                            log_builder.map(|builder| builder.build()),
                        );
                    }
                }
            }
            None => None,
        };

        let speed_m_per_tick = speed_m_per_tick(agent, config.tick_duration_ms);
        let mut route = initial_route;
        let mut total_ticks = 0;
        let mut completed = route.segments.is_empty();
        let mut completion_tick = completed.then_some(0);
        let mut total_distance_travelled = 0.0;
        let mut segment_index = 0usize;
        let mut distance_on_segment = 0.0;
        let mut violation_count = 0u64;
        let mut aborted = false;
        let mut brs = BlockedRouteState::new();

        if completed {
            if let Some(ref mut builder) = log_builder {
                builder.push(swarm_replay::Event::UrbanPatrolCompleted {
                    agent_id: agent_id.clone(),
                    tick: 0,
                    route_length_m: route.total_length_m,
                    distance_travelled_m: 0.0,
                });
            }
        } else if let Some(first_segment) = route.segments.first() {
            if let Some(ref mut builder) = log_builder {
                push_segment_entered(builder, &agent_id, 0, 0, first_segment);
            }
        }

        'tick_loop: for tick in 1..=config.max_ticks {
            if completed || aborted {
                break;
            }
            total_ticks = tick;
            if let Some(ref mut builder) = log_builder {
                builder.push(swarm_replay::Event::TickStart { tick });
            }

            // Compute effective blocked set for this tick.
            let effective_blocked = crate::urban::effective_blocked_edges(
                &urban_state.map,
                &urban_state.temporary_obstacles,
                tick,
            );

            // Handle ongoing wait: check if the blocked edge has cleared.
            if brs.is_waiting() {
                let waiting_edge = brs.waiting_for.clone().unwrap();
                if !effective_blocked.contains(&waiting_edge) {
                    let waited = tick.saturating_sub(brs.wait_start_tick);
                    brs.wait_ticks += waited;
                    if let Some(ref mut builder) = log_builder {
                        builder.push(swarm_replay::Event::UrbanEdgeUnblocked {
                            agent_id: agent_id.clone(),
                            tick,
                            edge_id: waiting_edge.clone(),
                        });
                        builder.push(swarm_replay::Event::UrbanWaitCompleted {
                            agent_id: agent_id.clone(),
                            tick,
                            edge_id: waiting_edge,
                            waited_ticks: waited,
                        });
                    }
                    brs.waiting_for = None;
                    // Fall through to movement.
                } else {
                    // Still blocked — skip movement this tick.
                    if let Some(ref mut builder) = log_builder {
                        if let Some(pose) = current_urban_pose(
                            &urban_state.map,
                            &route,
                            segment_index,
                            distance_on_segment,
                            completed,
                        ) {
                            builder.push(swarm_replay::Event::PoseUpdated {
                                agent_id: agent_id.clone(),
                                pose,
                                tick,
                            });
                        }
                    }
                    continue 'tick_loop;
                }
            }

            // Movement: advance through route segments.
            let mut remaining = speed_m_per_tick;
            'move_loop: while remaining > 0.0 && segment_index < route.segments.len() {
                // At a segment boundary: run the blocked-ahead detector.
                if distance_on_segment == 0.0 {
                    if let Some((_, ref blocked_edge_id)) = crate::urban::detect_blocked_ahead(
                        &route,
                        segment_index,
                        &effective_blocked,
                        crate::urban::URBAN_BLOCKED_LOOKAHEAD_SEGMENTS,
                    ) {
                        brs.blocked_edge_detections += 1;
                        if let Some(ref mut builder) = log_builder {
                            builder.push(swarm_replay::Event::UrbanObstacleDetected {
                                agent_id: agent_id.clone(),
                                tick,
                                edge_id: blocked_edge_id.clone(),
                                lookahead_segments: crate::urban::URBAN_BLOCKED_LOOKAHEAD_SEGMENTS,
                            });
                        }

                        match urban_state.blocked_route_policy {
                            UrbanBlockedPolicy::Wait => {
                                if let Some(ref mut builder) = log_builder {
                                    builder.push(swarm_replay::Event::UrbanEdgeBlocked {
                                        agent_id: agent_id.clone(),
                                        tick,
                                        edge_id: blocked_edge_id.clone(),
                                        reason: None,
                                    });
                                    builder.push(swarm_replay::Event::UrbanWaitStarted {
                                        agent_id: agent_id.clone(),
                                        tick,
                                        edge_id: blocked_edge_id.clone(),
                                    });
                                    builder.push(swarm_replay::Event::UrbanPolicyDecision {
                                        agent_id: agent_id.clone(),
                                        tick,
                                        edge_id: blocked_edge_id.clone(),
                                        policy: "wait".to_owned(),
                                    });
                                }
                                brs.waiting_for = Some(blocked_edge_id.clone());
                                brs.wait_start_tick = tick;
                                break 'move_loop;
                            }
                            UrbanBlockedPolicy::Replan => {
                                let current_from = route.segments[segment_index].from.clone();
                                match try_replan(
                                    &urban_state.map,
                                    &current_from,
                                    &effective_blocked,
                                    planner_mode,
                                    segment_index,
                                    &route,
                                ) {
                                    Some(new_route) => {
                                        if let Some(ref mut builder) = log_builder {
                                            builder.push(
                                                swarm_replay::Event::UrbanRouteReplanned {
                                                    agent_id: agent_id.clone(),
                                                    tick,
                                                    edge_ids: new_route
                                                        .segments
                                                        .iter()
                                                        .map(|s| s.edge_id.clone())
                                                        .collect(),
                                                    route_length_m: new_route.total_length_m,
                                                },
                                            );
                                            builder.push(
                                                swarm_replay::Event::UrbanPolicyDecision {
                                                    agent_id: agent_id.clone(),
                                                    tick,
                                                    edge_id: blocked_edge_id.clone(),
                                                    policy: "replan".to_owned(),
                                                },
                                            );
                                        }
                                        brs.replan_count += 1;
                                        route = new_route;
                                        segment_index = 0;
                                        distance_on_segment = 0.0;
                                        // Restart move loop with new route.
                                        break 'move_loop;
                                    }
                                    None => {
                                        // Replan failed — abort.
                                        brs.unresolved_blockages += 1;
                                        let dest_node = route
                                            .segments
                                            .last()
                                            .map(|s| s.to.clone())
                                            .unwrap_or_else(|| {
                                                route.segments[segment_index].from.clone()
                                            });
                                        if let Some(ref mut builder) = log_builder {
                                            builder.push(swarm_replay::Event::UrbanNoRouteAvailable {
                                                agent_id: agent_id.clone(),
                                                tick,
                                                from: route.segments[segment_index].from.clone(),
                                                to: dest_node,
                                                reason: format!(
                                                    "no alternate route around blocked edge '{blocked_edge_id}'"
                                                ),
                                            });
                                            builder.push(
                                                swarm_replay::Event::UrbanPolicyDecision {
                                                    agent_id: agent_id.clone(),
                                                    tick,
                                                    edge_id: blocked_edge_id.clone(),
                                                    policy: "abort".to_owned(),
                                                },
                                            );
                                        }
                                        aborted = true;
                                        break 'move_loop;
                                    }
                                }
                            }
                            UrbanBlockedPolicy::Abort => {
                                brs.unresolved_blockages += 1;
                                let dest_node =
                                    route.segments.last().map(|s| s.to.clone()).unwrap_or_else(
                                        || route.segments[segment_index].from.clone(),
                                    );
                                if let Some(ref mut builder) = log_builder {
                                    builder.push(swarm_replay::Event::UrbanNoRouteAvailable {
                                        agent_id: agent_id.clone(),
                                        tick,
                                        from: route.segments[segment_index].from.clone(),
                                        to: dest_node,
                                        reason: format!(
                                            "route blocked at edge '{blocked_edge_id}', policy=abort"
                                        ),
                                    });
                                    builder.push(swarm_replay::Event::UrbanPolicyDecision {
                                        agent_id: agent_id.clone(),
                                        tick,
                                        edge_id: blocked_edge_id.clone(),
                                        policy: "abort".to_owned(),
                                    });
                                }
                                aborted = true;
                                break 'move_loop;
                            }
                        }
                    }
                }

                let segment = &route.segments[segment_index];
                // Guard: check if this segment is in the effective blocked set (enforcement).
                if effective_blocked.contains(&segment.edge_id) && distance_on_segment == 0.0 {
                    // Entering a blocked segment without a policy action: record violation.
                    use swarm_types::UrbanViolation;
                    let violation = UrbanViolation::BlockedEdge {
                        edge_id: segment.edge_id.clone(),
                    };
                    violation_count += 1;
                    if let Some(ref mut builder) = log_builder {
                        push_urban_violation_event(builder, &agent_id, tick, &route, &violation);
                    }
                }

                let segment_remaining = (segment.length_m - distance_on_segment).max(0.0);
                if remaining + f64::EPSILON >= segment_remaining {
                    total_distance_travelled += segment_remaining;
                    remaining -= segment_remaining;
                    distance_on_segment = segment.length_m;

                    if let Some(ref mut builder) = log_builder {
                        builder.push(swarm_replay::Event::UrbanSegmentCompleted {
                            agent_id: agent_id.clone(),
                            tick,
                            segment_index,
                            edge_id: segment.edge_id.clone(),
                        });
                    }

                    segment_index += 1;
                    if segment_index == route.segments.len() {
                        completed = true;
                        completion_tick = Some(tick);
                        if let Some(ref mut builder) = log_builder {
                            builder.push(swarm_replay::Event::UrbanPatrolCompleted {
                                agent_id: agent_id.clone(),
                                tick,
                                route_length_m: route.total_length_m,
                                distance_travelled_m: total_distance_travelled,
                            });
                        }
                        break 'move_loop;
                    }

                    distance_on_segment = 0.0;
                    if let Some(ref mut builder) = log_builder {
                        push_segment_entered(
                            builder,
                            &agent_id,
                            tick,
                            segment_index,
                            &route.segments[segment_index],
                        );
                    }
                } else {
                    distance_on_segment += remaining;
                    total_distance_travelled += remaining;
                    remaining = 0.0;
                }
            }

            if let Some(ref mut builder) = log_builder {
                if let Some(pose) = current_urban_pose(
                    &urban_state.map,
                    &route,
                    segment_index,
                    distance_on_segment,
                    completed,
                ) {
                    builder.push(swarm_replay::Event::PoseUpdated {
                        agent_id: agent_id.clone(),
                        pose,
                        tick,
                    });
                }
                for state in &mut analysis_agent_states {
                    advance_urban_analysis_agent(builder, state, &urban_state.map, &route, tick);
                }
            }
        }

        if completed && total_ticks == 0 {
            total_ticks = completion_tick.unwrap_or(0);
        }

        let success = completed && !aborted && violation_count == 0;
        let route_eff = route_efficiency(initial_route_length_m, total_distance_travelled);
        let replan_rate = brs.replan_success_rate();

        let mut metrics = urban_patrol_metrics(
            scenario,
            total_ticks,
            success,
            true,
            initial_route_length_m,
            initial_route_risk,
            violation_count,
            completed,
            completion_tick,
            total_distance_travelled,
            route_eff,
            None,
            brs.replan_count,
            brs.wait_ticks,
            brs.blocked_edge_detections,
            replan_rate,
            brs.unresolved_blockages,
        );
        if let Some(perimeter_length_m) = perimeter_length_m {
            metrics.perimeter_length_m = perimeter_length_m;
            metrics.perimeter_completion_rate = if perimeter_length_m > 0.0 {
                (total_distance_travelled / perimeter_length_m).clamp(0.0, 1.0)
            } else if completed {
                1.0
            } else {
                0.0
            };
            metrics.time_to_complete_perimeter = completion_tick;
            metrics.perimeter_violations = violation_count;
        }
        finish_urban_run_metrics(metrics, log_builder)
    }

    fn run_urban_deconflicted_patrol(
        scenario: &Scenario,
        config: RunConfig,
        urban_state: super::UrbanState,
        mut log_builder: Option<swarm_replay::EventLogBuilder>,
    ) -> (RunMetrics, Option<swarm_replay::EventLog>) {
        if matches!(
            urban_state.deconfliction.right_of_way_policy,
            UrbanRightOfWayPolicy::MissionCriticalOverride
        ) {
            let metrics = urban_patrol_metrics(
                scenario,
                0,
                false,
                false,
                0.0,
                0.0,
                0,
                false,
                None,
                0.0,
                0.0,
                Some("urban_deconfliction_mission_critical_override_unsupported".to_owned()),
                0,
                0,
                0,
                0.0,
                0,
            );
            return finish_urban_run_metrics(metrics, log_builder);
        }

        let initial_route = match crate::urban::expand_route_loop_with_planner_name(
            &urban_state.map,
            &urban_state.route_loop,
            &urban_state.planner,
        ) {
            Ok(route) => route,
            Err(error) => {
                let metrics = urban_patrol_metrics(
                    scenario,
                    0,
                    false,
                    false,
                    0.0,
                    0.0,
                    1,
                    false,
                    None,
                    0.0,
                    0.0,
                    Some(error.to_string()),
                    0,
                    0,
                    0,
                    0.0,
                    0,
                );
                return finish_urban_run_metrics(metrics, log_builder);
            }
        };

        let start_node = match crate::urban::route_start_node(
            &urban_state.map,
            &urban_state.route_loop,
            &initial_route,
            urban_state.start_node.as_ref(),
        ) {
            Ok(start_node) => start_node,
            Err(error) => {
                let metrics = urban_patrol_metrics(
                    scenario,
                    0,
                    false,
                    true,
                    initial_route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &initial_route),
                    0,
                    false,
                    None,
                    0.0,
                    0.0,
                    Some(format!("urban_patrol_invalid_start: {error}")),
                    0,
                    0,
                    0,
                    0.0,
                    0,
                );
                return finish_urban_run_metrics(metrics, log_builder);
            }
        };

        let mut alive_agents: Vec<_> = scenario
            .agents
            .iter()
            .filter(|agent| agent.health == Health::Alive)
            .collect();
        alive_agents.sort_by(|left, right| left.id.as_ref().cmp(right.id.as_ref()));
        if alive_agents.is_empty() {
            let metrics = urban_patrol_metrics(
                scenario,
                0,
                false,
                true,
                initial_route.total_length_m,
                crate::urban::route_risk_score(&urban_state.map, &initial_route),
                0,
                false,
                None,
                0.0,
                0.0,
                Some("urban_patrol_no_alive_agent".to_owned()),
                0,
                0,
                0,
                0.0,
                0,
            );
            return finish_urban_run_metrics(metrics, log_builder);
        }

        let network_deconfliction = matches!(
            urban_state.deconfliction.mode,
            DeconflictionMode::NetworkProtocol { .. }
        );
        let agent_routes = alive_agents
            .iter()
            .enumerate()
            .map(|(index, agent)| {
                let route = if network_deconfliction {
                    split_route_for_agent(&initial_route, index, alive_agents.len())
                } else {
                    initial_route.clone()
                };
                (agent.id.clone(), route)
            })
            .collect::<HashMap<_, _>>();

        for agent in &alive_agents {
            let assigned_route = agent_routes
                .get(&agent.id)
                .expect("route generated for every alive agent");
            let start_pose_distance = agent.pose.distance_to(&start_node.pose);
            let slice_start_pose_distance = if network_deconfliction {
                first_route_node_pose(&urban_state.map, assigned_route)
                    .map(|pose| agent.pose.distance_to(pose))
            } else {
                None
            };
            let starts_at_global_or_slice = start_pose_distance
                <= crate::urban::URBAN_START_POSE_TOLERANCE_M
                || slice_start_pose_distance
                    .is_some_and(|distance| distance <= crate::urban::URBAN_START_POSE_TOLERANCE_M);
            if !starts_at_global_or_slice {
                let metrics = urban_patrol_metrics(
                    scenario,
                    0,
                    false,
                    true,
                    initial_route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &initial_route),
                    0,
                    false,
                    None,
                    0.0,
                    0.0,
                    Some(format!(
                        "urban_patrol_invalid_start: agent '{}' starts {:.3}m from start_node '{}' and not at assigned route slice start",
                        agent.id, start_pose_distance, start_node.id
                    )),
                    0,
                    0,
                    0,
                    0.0,
                    0,
                );
                return finish_urban_run_metrics(metrics, log_builder);
            }
        }

        let static_violations = crate::urban::judge_route(&urban_state.map, &initial_route);
        if !static_violations.is_empty() {
            if let Some(ref mut builder) = log_builder {
                for agent in &alive_agents {
                    for violation in &static_violations {
                        push_urban_violation_event(
                            builder,
                            &agent.id,
                            0,
                            &initial_route,
                            violation,
                        );
                    }
                }
            }
            let metrics = urban_patrol_metrics(
                scenario,
                0,
                false,
                true,
                initial_route.total_length_m,
                crate::urban::route_risk_score(&urban_state.map, &initial_route),
                static_violations.len() as u64,
                false,
                None,
                0.0,
                0.0,
                None,
                0,
                0,
                0,
                0.0,
                0,
            );
            return finish_urban_run_metrics(metrics, log_builder);
        }

        let planner_mode = crate::urban::UrbanPlannerMode::parse(&urban_state.planner)
            .unwrap_or(crate::urban::UrbanPlannerMode::Dijkstra);
        let initial_route_length_m = initial_route.total_length_m;
        let initial_route_risk = crate::urban::route_risk_score(&urban_state.map, &initial_route);
        let mut states: Vec<_> = alive_agents
            .iter()
            .map(|agent| {
                let route = agent_routes
                    .get(&agent.id)
                    .expect("route generated for every alive agent");
                DeconflictedAgentState::new(agent, route, config.tick_duration_ms)
            })
            .collect();
        states.sort_by(|left, right| left.agent_id.to_string().cmp(&right.agent_id.to_string()));

        if let Some(ref mut builder) = log_builder {
            for state in &states {
                builder.push(swarm_replay::Event::UrbanRoutePlanned {
                    agent_id: state.agent_id.clone(),
                    tick: 0,
                    edge_ids: state
                        .route
                        .segments
                        .iter()
                        .map(|segment| segment.edge_id.clone())
                        .collect(),
                    route_length_m: state.route.total_length_m,
                });
                builder.push(swarm_replay::Event::PoseUpdated {
                    agent_id: state.agent_id.clone(),
                    pose: start_node.pose,
                    tick: 0,
                });
            }
        }

        let mut registry = crate::urban::UrbanSegmentLockRegistry::new();
        let mut network_runtime = match &urban_state.deconfliction.mode {
            DeconflictionMode::SharedMemory => None,
            DeconflictionMode::NetworkProtocol { coordinator_id } => {
                Some(NetworkSegmentRuntime::new(
                    states.iter().map(|state| state.agent_id.clone()).collect(),
                    coordinator_id.clone(),
                    urban_state.deconfliction.right_of_way_policy.clone(),
                    urban_state.deconfliction.agent_priorities.clone(),
                ))
            }
        };
        let mut total_ticks = 0;
        let mut violation_count = 0u64;
        let mut deconflict_wait_events = 0u64;

        for tick in 0..=config.max_ticks {
            total_ticks = tick;
            if tick > 0 {
                if let Some(ref mut builder) = log_builder {
                    builder.push(swarm_replay::Event::TickStart { tick });
                }
            }

            let effective_blocked = crate::urban::effective_blocked_edges(
                &urban_state.map,
                &urban_state.temporary_obstacles,
                tick,
            );

            process_deconflicted_failures(
                &mut states,
                DeconflictedFailureContext {
                    tick,
                    failures: &config.failures,
                    registry: &mut registry,
                    map: &urban_state.map,
                    planner_mode,
                    effective_blocked: &effective_blocked,
                    network_runtime: network_runtime.as_mut(),
                    log_builder: log_builder.as_mut(),
                },
            );

            let mut requests = Vec::new();
            for (request_order, state) in states.iter_mut().enumerate() {
                if !state.active() || state.distance_on_segment != 0.0 {
                    continue;
                }
                let Some(edge_id) = state.current_edge_id().cloned() else {
                    continue;
                };
                if let Some(waiting_for) = state.waiting_for.clone() {
                    if is_segment_locked_by_other(
                        network_runtime.as_ref(),
                        &registry,
                        &waiting_for,
                        &state.agent_id,
                    ) {
                        deconflict_wait_events += 1;
                        if let Some(ref mut builder) = log_builder {
                            builder.push(swarm_replay::Event::UrbanDeconflictWait {
                                agent_id: state.agent_id.clone(),
                                tick,
                                edge_id: waiting_for,
                                reason: "segment still locked".to_owned(),
                            });
                        }
                        continue;
                    }
                    state.wait_ticks += tick.saturating_sub(state.wait_start_tick);
                    state.waiting_for = None;
                }
                requests.push(crate::urban::SegmentLockRequest {
                    agent_id: state.agent_id.clone(),
                    edge_id,
                    segment_index: state.segment_index,
                    request_order,
                });
            }

            let decisions = if let Some(network_runtime) = network_runtime.as_mut() {
                network_runtime.request_batch(requests, tick, log_builder.as_mut())
            } else {
                registry.request_batch(
                    requests,
                    tick,
                    &urban_state.deconfliction.right_of_way_policy,
                    &urban_state.deconfliction.agent_priorities,
                )
            };
            for (agent_id, decision) in decisions {
                let Some(state) = states.iter_mut().find(|state| state.agent_id == agent_id) else {
                    continue;
                };
                match decision {
                    crate::urban::SegmentLockDecision::Acquired(lock)
                    | crate::urban::SegmentLockDecision::AlreadyHeld(lock) => {
                        if let Some(ref mut builder) = log_builder {
                            push_segment_lock_acquired(
                                builder,
                                &lock,
                                urban_state.deconfliction.right_of_way_policy.clone(),
                                "segment reserved before entry",
                            );
                            if let Some(segment) = state.route.segments.get(state.segment_index) {
                                push_segment_entered(
                                    builder,
                                    &state.agent_id,
                                    tick,
                                    state.segment_index,
                                    segment,
                                );
                            }
                        }
                    }
                    crate::urban::SegmentLockDecision::Conflict(conflict) => {
                        if let Some(ref mut builder) = log_builder {
                            push_segment_conflict(builder, &conflict);
                        }
                        let edge_id = conflict.edge_id.clone();
                        match urban_state.deconfliction.locked_segment_policy {
                            UrbanBlockedPolicy::Wait => {
                                if state.waiting_for.is_none() {
                                    state.wait_start_tick = tick;
                                }
                                state.waiting_for = Some(edge_id.clone());
                                deconflict_wait_events += 1;
                                if let Some(ref mut builder) = log_builder {
                                    builder.push(swarm_replay::Event::UrbanDeconflictWait {
                                        agent_id: state.agent_id.clone(),
                                        tick,
                                        edge_id,
                                        reason: conflict.reason,
                                    });
                                }
                            }
                            UrbanBlockedPolicy::Replan => {
                                let current_from =
                                    state.route.segments[state.segment_index].from.clone();
                                let mut excluded_edges = effective_blocked.clone();
                                for locked_edge in locked_edges_except(
                                    network_runtime.as_ref(),
                                    &registry,
                                    &state.agent_id,
                                ) {
                                    excluded_edges.insert(locked_edge);
                                }
                                match try_replan(
                                    &urban_state.map,
                                    &current_from,
                                    &excluded_edges,
                                    planner_mode,
                                    state.segment_index,
                                    &state.route,
                                ) {
                                    Some(new_route) => {
                                        state.replan_count += 1;
                                        state.route = new_route;
                                        state.segment_index = 0;
                                        state.distance_on_segment = 0.0;
                                        state.waiting_for = None;
                                        if let Some(ref mut builder) = log_builder {
                                            builder.push(
                                                swarm_replay::Event::UrbanDeconflictReplan {
                                                    agent_id: state.agent_id.clone(),
                                                    tick,
                                                    edge_id,
                                                    edge_ids: state
                                                        .route
                                                        .segments
                                                        .iter()
                                                        .map(|segment| segment.edge_id.clone())
                                                        .collect(),
                                                    route_length_m: state.route.total_length_m,
                                                    reason: "alternate route around locked segment"
                                                        .to_owned(),
                                                },
                                            );
                                        }
                                    }
                                    None => {
                                        state.aborted = true;
                                        state.unresolved_blockages += 1;
                                        if let Some(ref mut builder) = log_builder {
                                            builder.push(
                                                swarm_replay::Event::UrbanDeconflictAbort {
                                                    agent_id: state.agent_id.clone(),
                                                    tick,
                                                    edge_id,
                                                    reason:
                                                        "no alternate route around locked segment"
                                                            .to_owned(),
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                            UrbanBlockedPolicy::Abort => {
                                state.aborted = true;
                                state.unresolved_blockages += 1;
                                if let Some(ref mut builder) = log_builder {
                                    builder.push(swarm_replay::Event::UrbanDeconflictAbort {
                                        agent_id: state.agent_id.clone(),
                                        tick,
                                        edge_id,
                                        reason: conflict.reason,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            for state in &mut states {
                if !state.active() || state.waiting_for.is_some() {
                    continue;
                }
                let Some(edge_id) = state.current_edge_id().cloned() else {
                    continue;
                };
                if is_segment_locked_by_other(
                    network_runtime.as_ref(),
                    &registry,
                    &edge_id,
                    &state.agent_id,
                ) {
                    continue;
                }
                if effective_blocked.contains(&edge_id) && state.distance_on_segment == 0.0 {
                    violation_count += 1;
                    if let Some(ref mut builder) = log_builder {
                        push_urban_violation_event(
                            builder,
                            &state.agent_id,
                            tick,
                            &state.route,
                            &swarm_types::UrbanViolation::BlockedEdge {
                                edge_id: edge_id.clone(),
                            },
                        );
                    }
                }

                let remaining = state.speed_m_per_tick;
                let Some(segment) = state.route.segments.get(state.segment_index) else {
                    continue;
                };
                let segment_remaining = (segment.length_m - state.distance_on_segment).max(0.0);
                if remaining + f64::EPSILON >= segment_remaining {
                    state.total_distance_travelled_m += segment_remaining;
                    state.distance_on_segment = segment.length_m;
                    if let Some(ref mut builder) = log_builder {
                        builder.push(swarm_replay::Event::UrbanSegmentCompleted {
                            agent_id: state.agent_id.clone(),
                            tick,
                            segment_index: state.segment_index,
                            edge_id: segment.edge_id.clone(),
                        });
                    }
                    let released_lock = if let Some(network_runtime) = network_runtime.as_mut() {
                        network_runtime.release(
                            &segment.edge_id,
                            &state.agent_id,
                            tick,
                            log_builder.as_mut(),
                        )
                    } else {
                        registry.release(&segment.edge_id, &state.agent_id, tick)
                    };
                    if let Some(lock) = released_lock {
                        if let Some(ref mut builder) = log_builder {
                            push_segment_lock_released(builder, &lock, tick);
                        }
                    }
                    state.segment_index += 1;
                    if state.segment_index == state.route.segments.len() {
                        state.completed = true;
                        state.completion_tick = Some(tick);
                        if let Some(ref mut builder) = log_builder {
                            builder.push(swarm_replay::Event::UrbanPatrolCompleted {
                                agent_id: state.agent_id.clone(),
                                tick,
                                route_length_m: state.route.total_length_m,
                                distance_travelled_m: state.total_distance_travelled_m,
                            });
                        }
                    } else {
                        state.distance_on_segment = 0.0;
                    }
                } else {
                    state.distance_on_segment += remaining;
                    state.total_distance_travelled_m += remaining;
                }

                if let Some(ref mut builder) = log_builder {
                    if let Some(pose) = current_urban_pose(
                        &urban_state.map,
                        &state.route,
                        state.segment_index,
                        state.distance_on_segment,
                        state.completed,
                    ) {
                        builder.push(swarm_replay::Event::PoseUpdated {
                            agent_id: state.agent_id.clone(),
                            pose,
                            tick,
                        });
                    }
                }
            }

            if states.iter().all(|state| !state.active()) {
                break;
            }
        }

        let completed_count = states.iter().filter(|state| state.completed).count();
        let aborted_count = states.iter().filter(|state| state.aborted).count();
        let recovered_failed_count = states
            .iter()
            .filter(|state| state.failed && state.handoff_target.is_some())
            .count();
        let effective_completed = completed_count + recovered_failed_count == states.len();
        let success = effective_completed && aborted_count == 0 && violation_count == 0;
        let total_distance_travelled: f64 = states
            .iter()
            .map(|state| state.total_distance_travelled_m)
            .sum();
        let total_route_length_m: f64 = states.iter().map(|state| state.route.total_length_m).sum();
        let route_eff = route_efficiency(total_route_length_m, total_distance_travelled);
        let completion_tick = states
            .iter()
            .filter_map(|state| state.completion_tick)
            .max();
        let replan_count: u64 = states.iter().map(|state| state.replan_count).sum();
        let wait_ticks: u64 = states.iter().map(|state| state.wait_ticks).sum();
        let unresolved_blockages: u64 = states.iter().map(|state| state.unresolved_blockages).sum();
        let conflict_count = network_runtime
            .as_ref()
            .map(NetworkSegmentRuntime::conflict_count)
            .unwrap_or_else(|| registry.conflict_history().len() as u64);
        let mut metrics = urban_patrol_metrics(
            scenario,
            total_ticks,
            success,
            true,
            initial_route_length_m,
            initial_route_risk,
            violation_count,
            effective_completed,
            completion_tick,
            total_distance_travelled,
            route_eff,
            None,
            replan_count,
            wait_ticks,
            0,
            0.0,
            unresolved_blockages,
        );
        metrics.task_completion_rate = if states.is_empty() {
            0.0
        } else {
            (completed_count + recovered_failed_count) as f64 / states.len() as f64
        };
        metrics.all_tasks_assigned = effective_completed;
        metrics.urban_deconflict_conflict_count = conflict_count;
        metrics.urban_deconflict_wait_ticks = wait_ticks + deconflict_wait_events;
        metrics.urban_deconflict_replan_count = replan_count;
        metrics.urban_deconflict_abort_count = aborted_count as u64;
        metrics.urban_avg_delay_per_agent_ticks = if states.is_empty() {
            0.0
        } else {
            metrics.urban_deconflict_wait_ticks as f64 / states.len() as f64
        };
        metrics.urban_segment_utilization = if total_ticks > 0 && !initial_route.segments.is_empty()
        {
            let route_capacity = total_ticks as f64 * initial_route.segments.len() as f64;
            (total_distance_travelled / initial_route_length_m).min(route_capacity) / route_capacity
        } else {
            0.0
        };
        finish_urban_run_metrics(metrics, log_builder)
    }
}

fn perimeter_length_m(waypoints: &[swarm_types::Pose]) -> f64 {
    waypoints
        .windows(2)
        .map(|pair| pair[0].distance_to(&pair[1]))
        .sum()
}

/// Attempt to replan the remaining route from `current_from`, avoiding
/// `effective_blocked` edges.
///
/// Iterates over the remaining waypoints in `route.segments[segment_index..]`,
/// trying each `to` node as the next reachable target. For each reachable
/// waypoint `w` at original index `i`, builds the candidate:
///   `path_to_w + route.segments[(segment_index + i + 1)..]`
/// Validates via M71 gate (judge_route) and effective_blocked check.
///
/// Returns `None` if no valid replan exists.
fn try_replan(
    map: &swarm_types::UrbanMap,
    current_from: &UrbanNodeId,
    effective_blocked: &HashSet<UrbanEdgeId>,
    planner: crate::urban::UrbanPlannerMode,
    segment_index: usize,
    route: &UrbanPlannedRoute,
) -> Option<UrbanPlannedRoute> {
    use std::collections::HashSet as HSet;
    let remaining = &route.segments[segment_index..];
    let mut tried_targets: HSet<UrbanNodeId> = HSet::new();

    for (idx, seg) in remaining.iter().enumerate() {
        let target = &seg.to;
        // Skip if we already tried this target or if it equals current position.
        if target == current_from || !tried_targets.insert(target.clone()) {
            continue;
        }
        let Ok(path) = crate::urban::plan_route_excluding(
            map,
            current_from,
            target,
            effective_blocked,
            planner,
        ) else {
            continue;
        };
        // Splice: path_to_target + remaining segments after this waypoint.
        let suffix = &route.segments[(segment_index + idx + 1)..];
        let new_segments: Vec<_> = path.segments.iter().chain(suffix.iter()).cloned().collect();
        // Reject empty replacement routes — means nothing was replanned.
        if new_segments.is_empty() {
            continue;
        }
        let total_length_m = new_segments.iter().map(|s| s.length_m).sum();
        let total_cost = new_segments.iter().map(|s| s.cost).sum();
        let candidate = swarm_types::UrbanPlannedRoute {
            segments: new_segments,
            total_length_m,
            total_cost,
        };
        // M71 gate: no static violations.
        if !crate::urban::judge_route(map, &candidate).is_empty() {
            continue;
        }
        // Effective blocked check.
        if candidate
            .segments
            .iter()
            .any(|s| effective_blocked.contains(&s.edge_id))
        {
            continue;
        }
        return Some(candidate);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_id(id: &str) -> AgentId {
        AgentId::from(id.to_owned())
    }

    fn edge_id(id: &str) -> UrbanEdgeId {
        UrbanEdgeId::from(id.to_owned())
    }

    fn request(
        agent_id: &AgentId,
        edge_id: &UrbanEdgeId,
        order: usize,
    ) -> crate::urban::SegmentLockRequest {
        crate::urban::SegmentLockRequest {
            agent_id: agent_id.clone(),
            edge_id: edge_id.clone(),
            segment_index: 0,
            request_order: order,
        }
    }

    #[test]
    fn network_segment_runtime_priority_matches_shared_memory_batch() {
        let agent_0 = agent_id("agent-0");
        let agent_1 = agent_id("agent-1");
        let edge_id = edge_id("road-n0-n1");
        let mut priorities = HashMap::new();
        priorities.insert(agent_0.clone(), 1);
        priorities.insert(agent_1.clone(), 9);
        let mut runtime = NetworkSegmentRuntime::new(
            vec![agent_0.clone(), agent_1.clone()],
            agent_id("coordinator-0"),
            UrbanRightOfWayPolicy::Priority,
            priorities,
        );

        let decisions = runtime.request_batch(
            vec![
                request(&agent_0, &edge_id, 0),
                request(&agent_1, &edge_id, 1),
            ],
            1,
            None,
        );

        let agent_0_decision = decisions
            .iter()
            .find(|(agent_id, _)| agent_id == &agent_0)
            .expect("agent-0 decision should exist");
        let agent_1_decision = decisions
            .iter()
            .find(|(agent_id, _)| agent_id == &agent_1)
            .expect("agent-1 decision should exist");
        assert!(matches!(
            agent_0_decision.1,
            crate::urban::SegmentLockDecision::Conflict(_)
        ));
        assert!(matches!(
            agent_1_decision.1,
            crate::urban::SegmentLockDecision::Acquired(_)
        ));
    }
}
