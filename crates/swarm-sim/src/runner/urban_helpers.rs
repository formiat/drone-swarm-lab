use super::*;
pub(super) fn compute_urban_foundation_metrics(
    urban_state: &Option<UrbanState>,
) -> (bool, f64, f64, u64) {
    let Some(urban_state) = urban_state else {
        return (false, 0.0, 0.0, 0);
    };
    match crate::urban::expand_route_loop_with_planner_name(
        &urban_state.map,
        &urban_state.route_loop,
        &urban_state.planner,
    ) {
        Ok(route) => {
            let violations = crate::urban::judge_route(&urban_state.map, &route);
            (
                true,
                route.total_length_m,
                crate::urban::route_risk_score(&urban_state.map, &route),
                violations.len() as u64,
            )
        }
        Err(_) => (false, 0.0, 0.0, 1),
    }
}

pub(super) fn speed_m_per_tick(agent: &Agent, tick_duration_ms: u64) -> f64 {
    let tick_seconds = tick_duration_ms as f64 / 1000.0;
    if tick_seconds.is_finite() && tick_seconds > 0.0 && agent.speed.is_finite() {
        (agent.speed * tick_seconds).max(0.0)
    } else {
        0.0
    }
}

pub(super) fn route_efficiency(route_length_m: f64, distance_travelled_m: f64) -> f64 {
    if distance_travelled_m > 0.0 {
        route_length_m / distance_travelled_m
    } else {
        0.0
    }
}

pub(super) fn advance_search_segment(
    route: &UrbanPlannedRoute,
    segment_index: usize,
    tick: u64,
    agent_id: &AgentId,
    mut log_builder: Option<&mut swarm_replay::EventLogBuilder>,
) -> usize {
    let Some(segment) = route.segments.get(segment_index) else {
        return 0;
    };
    if let Some(ref mut builder) = log_builder {
        builder.push(swarm_replay::Event::UrbanSegmentCompleted {
            agent_id: agent_id.clone(),
            tick,
            segment_index,
            edge_id: segment.edge_id.clone(),
        });
    }

    let next_index = if segment_index + 1 == route.segments.len() {
        0
    } else {
        segment_index + 1
    };
    if let Some(next_segment) = route.segments.get(next_index) {
        if let Some(ref mut builder) = log_builder {
            push_segment_entered(builder, agent_id, tick, next_index, next_segment);
        }
    }
    next_index
}

pub(super) fn push_segment_entered(
    builder: &mut swarm_replay::EventLogBuilder,
    agent_id: &AgentId,
    tick: u64,
    segment_index: usize,
    segment: &UrbanRouteSegment,
) {
    builder.push(swarm_replay::Event::UrbanSegmentEntered {
        agent_id: agent_id.clone(),
        tick,
        segment_index,
        edge_id: segment.edge_id.clone(),
        from: segment.from.clone(),
        to: segment.to.clone(),
    });
}

pub(super) fn push_urban_violation_event(
    builder: &mut swarm_replay::EventLogBuilder,
    agent_id: &AgentId,
    tick: u64,
    route: &UrbanPlannedRoute,
    violation: &UrbanViolation,
) {
    let edge_id = match violation {
        UrbanViolation::MissingEdge { edge_id }
        | UrbanViolation::BlockedEdge { edge_id }
        | UrbanViolation::ObstacleIntersection { edge_id, .. } => Some(edge_id.clone()),
    };
    let obstacle_id = match violation {
        UrbanViolation::ObstacleIntersection { obstacle_id, .. } => Some(obstacle_id.clone()),
        UrbanViolation::MissingEdge { .. } | UrbanViolation::BlockedEdge { .. } => None,
    };
    let segment_index = edge_id.as_ref().and_then(|id| {
        route
            .segments
            .iter()
            .position(|segment| &segment.edge_id == id)
    });
    let pose = match violation {
        UrbanViolation::ObstacleIntersection { location, .. } => *location,
        UrbanViolation::MissingEdge { .. } | UrbanViolation::BlockedEdge { .. } => {
            swarm_types::Pose::default()
        }
    };
    builder.push(swarm_replay::Event::UrbanViolation {
        agent_id: agent_id.clone(),
        tick,
        segment_index,
        edge_id,
        obstacle_id,
        pose,
        reason: format!("{violation:?}"),
    });
}

pub(super) fn push_detection_events(
    builder: &mut swarm_replay::EventLogBuilder,
    agent_id: &AgentId,
    tick: u64,
    pose: swarm_types::Pose,
    detector_seed: u64,
    outcome: &crate::urban::UrbanDetectionOutcome,
) {
    for observation in &outcome.observations {
        builder.push(swarm_replay::Event::BusObserved {
            agent_id: agent_id.clone(),
            tick,
            bus_id: observation.bus_id.clone(),
            pose: observation.pose,
            distance_m: observation.distance_m,
            detector_seed,
        });
    }
    if let Some(detection) = &outcome.detection {
        builder.push(swarm_replay::Event::BusDetected {
            agent_id: agent_id.clone(),
            tick,
            bus_id: detection.bus_id.clone(),
            pose: detection.pose,
            distance_m: detection.distance_m,
            detector_seed,
        });
    }
    if outcome.false_positive {
        builder.push(swarm_replay::Event::BusFalsePositive {
            agent_id: agent_id.clone(),
            tick,
            pose,
            detector_seed,
        });
    }
}

pub(super) fn push_urban_search_completed(
    builder: &mut swarm_replay::EventLogBuilder,
    agent_id: &AgentId,
    tick: u64,
    detected: bool,
    bus_id: Option<UrbanBusId>,
    reason: &str,
    distance_travelled_m: f64,
) {
    builder.push(swarm_replay::Event::UrbanSearchCompleted {
        agent_id: agent_id.clone(),
        tick,
        detected,
        bus_id,
        reason: reason.to_owned(),
        distance_travelled_m,
    });
}

pub(super) fn current_urban_pose(
    map: &UrbanMap,
    route: &UrbanPlannedRoute,
    segment_index: usize,
    distance_on_segment: f64,
    completed: bool,
) -> Option<swarm_types::Pose> {
    if completed {
        return route
            .segments
            .last()
            .and_then(|segment| map.node(&segment.to).map(|node| node.pose));
    }
    route.segments.get(segment_index).and_then(|segment| {
        crate::urban::pose_along_segment(map, segment, distance_on_segment).ok()
    })
}

pub(super) struct UrbanAnalysisAgentState {
    agent_id: AgentId,
    offset: swarm_types::Pose,
    speed_m_per_tick: f64,
    segment_index: usize,
    distance_on_segment: f64,
    completed: bool,
    total_distance_travelled_m: f64,
}

pub(super) fn urban_analysis_agent_states(
    scenario: &Scenario,
    primary_agent_id: &AgentId,
    start_pose: swarm_types::Pose,
    tick_duration_ms: u64,
) -> Vec<UrbanAnalysisAgentState> {
    scenario
        .agents
        .iter()
        .filter(|agent| agent.health == Health::Alive && &agent.id != primary_agent_id)
        .map(|agent| UrbanAnalysisAgentState {
            agent_id: agent.id.clone(),
            offset: swarm_types::Pose {
                x: agent.pose.x - start_pose.x,
                y: agent.pose.y - start_pose.y,
                z: agent.pose.z - start_pose.z,
            },
            speed_m_per_tick: speed_m_per_tick(agent, tick_duration_ms),
            segment_index: 0,
            distance_on_segment: 0.0,
            completed: false,
            total_distance_travelled_m: 0.0,
        })
        .collect()
}

pub(super) fn push_urban_analysis_agent_started(
    builder: &mut swarm_replay::EventLogBuilder,
    state: &UrbanAnalysisAgentState,
    map: &UrbanMap,
    route: &UrbanPlannedRoute,
) {
    builder.push(swarm_replay::Event::UrbanRoutePlanned {
        agent_id: state.agent_id.clone(),
        tick: 0,
        edge_ids: route
            .segments
            .iter()
            .map(|segment| segment.edge_id.clone())
            .collect(),
        route_length_m: route.total_length_m,
    });
    if let Some(pose) = current_urban_pose(map, route, 0, 0.0, false) {
        builder.push(swarm_replay::Event::PoseUpdated {
            agent_id: state.agent_id.clone(),
            pose: offset_urban_analysis_pose(pose, state),
            tick: 0,
        });
    }
    if let Some(first_segment) = route.segments.first() {
        push_segment_entered(builder, &state.agent_id, 0, 0, first_segment);
    } else {
        builder.push(swarm_replay::Event::UrbanPatrolCompleted {
            agent_id: state.agent_id.clone(),
            tick: 0,
            route_length_m: route.total_length_m,
            distance_travelled_m: 0.0,
        });
    }
}

pub(super) fn advance_urban_analysis_agent(
    builder: &mut swarm_replay::EventLogBuilder,
    state: &mut UrbanAnalysisAgentState,
    map: &UrbanMap,
    route: &UrbanPlannedRoute,
    tick: u64,
) {
    if state.completed {
        return;
    }

    let mut remaining = state.speed_m_per_tick;
    while remaining > 0.0 && state.segment_index < route.segments.len() {
        let segment = &route.segments[state.segment_index];
        let segment_remaining = (segment.length_m - state.distance_on_segment).max(0.0);
        if remaining + f64::EPSILON >= segment_remaining {
            state.total_distance_travelled_m += segment_remaining;
            remaining -= segment_remaining;
            state.distance_on_segment = segment.length_m;
            builder.push(swarm_replay::Event::UrbanSegmentCompleted {
                agent_id: state.agent_id.clone(),
                tick,
                segment_index: state.segment_index,
                edge_id: segment.edge_id.clone(),
            });
            state.segment_index += 1;
            if state.segment_index == route.segments.len() {
                state.completed = true;
                builder.push(swarm_replay::Event::UrbanPatrolCompleted {
                    agent_id: state.agent_id.clone(),
                    tick,
                    route_length_m: route.total_length_m,
                    distance_travelled_m: state.total_distance_travelled_m,
                });
                break;
            }
            state.distance_on_segment = 0.0;
            push_segment_entered(
                builder,
                &state.agent_id,
                tick,
                state.segment_index,
                &route.segments[state.segment_index],
            );
        } else {
            state.distance_on_segment += remaining;
            state.total_distance_travelled_m += remaining;
            remaining = 0.0;
        }
    }

    if let Some(pose) = current_urban_pose(
        map,
        route,
        state.segment_index,
        state.distance_on_segment,
        state.completed,
    ) {
        builder.push(swarm_replay::Event::PoseUpdated {
            agent_id: state.agent_id.clone(),
            pose: offset_urban_analysis_pose(pose, state),
            tick,
        });
    }
}

pub(super) fn offset_urban_analysis_pose(
    pose: swarm_types::Pose,
    state: &UrbanAnalysisAgentState,
) -> swarm_types::Pose {
    swarm_types::Pose {
        x: pose.x + state.offset.x,
        y: pose.y + state.offset.y,
        z: pose.z + state.offset.z,
    }
}

pub(super) fn finish_urban_run_metrics(
    mut metrics: RunMetrics,
    log_builder: Option<swarm_replay::EventLogBuilder>,
) -> (RunMetrics, Option<swarm_replay::EventLog>) {
    let event_log = log_builder.map(|builder| builder.build());
    if let Some(log) = &event_log {
        let trace = crate::urban_analysis::build_urban_route_trace(log);
        let separation = crate::urban_analysis::measure_urban_separation(
            &trace,
            crate::urban_analysis::URBAN_ANALYSIS_DEFAULT_SEPARATION_THRESHOLD_M,
        );
        metrics.urban_min_agent_separation_m = separation.min_separation_m;
        metrics.urban_separation_violation_count = separation.separation_violation_count;
        metrics.urban_route_conflict_count = separation.route_conflict_count;
    }
    (metrics, event_log)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn urban_patrol_metrics(
    scenario: &Scenario,
    total_ticks: u64,
    success: bool,
    urban_route_planned: bool,
    urban_route_length_m: f64,
    urban_route_risk_score: f64,
    urban_violation_count: u64,
    urban_patrol_completed: bool,
    urban_time_to_complete_loop: Option<u64>,
    urban_distance_travelled_m: f64,
    urban_route_efficiency: f64,
    unsupported_reason: Option<String>,
) -> RunMetrics {
    let agent_count = scenario.agents.len() as f64;
    let battery_min = scenario
        .agents
        .iter()
        .map(|agent| agent.battery)
        .fold(f64::INFINITY, f64::min);
    let battery_min = if battery_min.is_finite() {
        battery_min
    } else {
        0.0
    };
    let battery_avg = if agent_count > 0.0 {
        scenario
            .agents
            .iter()
            .map(|agent| agent.battery)
            .sum::<f64>()
            / agent_count
    } else {
        0.0
    };
    let coverage_progress = if urban_route_length_m > 0.0 {
        (urban_distance_travelled_m / urban_route_length_m).clamp(0.0, 1.0)
    } else if urban_patrol_completed {
        1.0
    } else {
        0.0
    };

    RunMetrics {
        seed: scenario.seed,
        total_ticks,
        messages_attempted: 0,
        messages_dropped: 0,
        detection_time_ticks: None,
        reallocation_time_ticks: None,
        max_task_unassigned_ticks: 0,
        all_tasks_assigned: urban_patrol_completed,
        success,
        tasks_injected: 0,
        tasks_expired: 0,
        conflicting_assignments: 0,
        partition_events: 0,
        partitions_active: false,
        stale_messages_discarded: 0,
        convergence_ticks: None,
        max_view_divergence: 0,
        network_availability: 1.0,
        relay_reallocation_ticks: None,
        avg_hop_count: 0.0,
        disconnected_agents_max: 0,
        coverage_progress,
        bytes_sent: 0,
        stale_state_age_ticks: 0,
        battery_margin_min: battery_min,
        battery_margin_avg: battery_avg,
        final_battery_min: battery_min,
        avg_distance_travelled: urban_distance_travelled_m,
        agents_exhausted: 0,
        total_distance_travelled: urban_distance_travelled_m,
        mission_completion_ticks: urban_time_to_complete_loop.unwrap_or(total_ticks),
        time_to_first_exhaustion: None,
        time_to_find: None,
        coverage_over_time: vec![coverage_progress],
        probability_of_detection: 0.0,
        targets_found: 0,
        targets_total: 0,
        scan_count: 0,
        cbba_rounds_to_convergence: 0,
        cbba_converged: false,
        cbba_messages: 0,
        safety_violations: 0,
        belief_entropy_final: 0.0,
        false_positives: 0,
        confirmation_scans: 0,
        cbba_convergence_tick: None,
        bundle_travel_distance: urban_distance_travelled_m,
        edge_coverage_rate: 0.0,
        missed_edges: 0,
        revisit_count: 0,
        route_efficiency: urban_route_efficiency,
        avg_route_length: urban_route_length_m,
        avg_wasted_travel: 0.0,
        avg_return_reserve: 0.0,
        infeasible_routes: 0,
        hazard_zones_mapped: 0,
        priority_updates: 0,
        final_avg_threat_level: 0.0,
        high_priority_zones_mapped: 0,
        time_to_map_first_high_risk: None,
        threat_level_over_time: vec![],
        zone_observations: 0,
        unsupported_reason,
        realism_profile: None,
        wind: None,
        urban_route_length_m,
        urban_route_risk_score,
        urban_route_planned,
        urban_violation_count,
        urban_route_completed: urban_patrol_completed,
        urban_patrol_completed,
        urban_time_to_complete_loop,
        urban_distance_travelled_m,
        urban_route_efficiency,
        urban_replan_count: 0,
        bus_detected: false,
        time_to_detect_bus: None,
        false_positive_count: 0,
        distance_before_detection: 0.0,
        search_success_without_violation: false,
        urban_min_agent_separation_m: None,
        urban_separation_violation_count: 0,
        urban_route_conflict_count: 0,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn urban_search_metrics(
    scenario: &Scenario,
    total_ticks: u64,
    bus_detected: bool,
    urban_route_planned: bool,
    urban_route_length_m: f64,
    urban_route_risk_score: f64,
    urban_violation_count: u64,
    time_to_detect_bus: Option<u64>,
    false_positive_count: u64,
    urban_distance_travelled_m: f64,
    urban_route_efficiency: f64,
    unsupported_reason: Option<String>,
) -> RunMetrics {
    let search_success_without_violation =
        bus_detected && urban_violation_count == 0 && unsupported_reason.is_none();
    let mut metrics = urban_patrol_metrics(
        scenario,
        total_ticks,
        search_success_without_violation,
        urban_route_planned,
        urban_route_length_m,
        urban_route_risk_score,
        urban_violation_count,
        false,
        None,
        urban_distance_travelled_m,
        urban_route_efficiency,
        unsupported_reason,
    );
    metrics.all_tasks_assigned = bus_detected;
    metrics.bus_detected = bus_detected;
    metrics.time_to_detect_bus = time_to_detect_bus;
    metrics.false_positive_count = false_positive_count;
    metrics.distance_before_detection = if bus_detected {
        urban_distance_travelled_m
    } else {
        0.0
    };
    metrics.search_success_without_violation = search_success_without_violation;
    metrics
}

pub(super) fn update_unassigned_durations(
    coordinator: &Coordinator,
    durations: &mut HashMap<TaskId, u64>,
    current_max: u64,
) -> u64 {
    let unassigned: HashSet<_> = coordinator
        .registry
        .unassigned()
        .into_iter()
        .map(|task| task.id.clone())
        .collect();
    durations.retain(|task_id, _| unassigned.contains(task_id));

    let mut max_duration = current_max;
    for task_id in unassigned {
        let duration = durations.entry(task_id).or_insert(0);
        *duration += 1;
        max_duration = max_duration.max(*duration);
    }
    max_duration
}

pub(super) fn released_tasks_reassigned(
    coordinator: &Coordinator,
    released_tasks: &[TaskId],
) -> bool {
    released_tasks.iter().all(|released_task| {
        coordinator
            .registry
            .tasks()
            .any(|task| &task.id == released_task && task.assigned_to.is_some())
    })
}
