use std::collections::HashSet;

use swarm_types::{UrbanBlockedPolicy, UrbanEdgeId, UrbanNodeId, UrbanPlannedRoute};

use super::{
    advance_urban_analysis_agent, current_urban_pose, finish_urban_run_metrics,
    push_segment_entered, push_urban_analysis_agent_started, push_urban_violation_event,
    route_efficiency, speed_m_per_tick, urban_analysis_agent_states, urban_patrol_metrics, Health,
    RunConfig, RunMetrics, Scenario, ScenarioRunner,
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

impl ScenarioRunner {
    pub(super) fn run_urban_patrol(
        scenario: &Scenario,
        config: RunConfig,
        mut log_builder: Option<swarm_replay::EventLogBuilder>,
    ) -> (RunMetrics, Option<swarm_replay::EventLog>) {
        let Some(urban_state) = config.urban_state.clone() else {
            unreachable!("run_urban_patrol is called only for urban_state runs");
        };
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
