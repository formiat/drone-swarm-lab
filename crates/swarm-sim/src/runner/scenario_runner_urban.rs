use super::*;
impl ScenarioRunner {
    pub(super) fn run_urban_patrol(
        scenario: &Scenario,
        config: RunConfig,
        mut log_builder: Option<swarm_replay::EventLogBuilder>,
    ) -> (RunMetrics, Option<swarm_replay::EventLog>) {
        let Some(urban_state) = config.urban_state.clone() else {
            unreachable!("run_urban_patrol is called only for urban_state runs");
        };
        let route = match crate::urban::expand_route_loop_with_planner_name(
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
                    route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &route),
                    0,
                    false,
                    None,
                    0.0,
                    0.0,
                    Some("urban_patrol_no_alive_agent".to_owned()),
                ),
                log_builder.map(|builder| builder.build()),
            );
        };
        let agent_id = agent.id.clone();
        let start_node = match crate::urban::route_start_node(
            &urban_state.map,
            &urban_state.route_loop,
            &route,
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
                        route.total_length_m,
                        crate::urban::route_risk_score(&urban_state.map, &route),
                        0,
                        false,
                        None,
                        0.0,
                        0.0,
                        Some(format!("urban_patrol_invalid_start: {error}")),
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
                    route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &route),
                    0,
                    false,
                    None,
                    0.0,
                    0.0,
                    Some(format!(
                        "urban_patrol_invalid_start: agent '{}' starts {:.3}m from start_node '{}'",
                        agent.id, start_pose_distance, start_node.id
                    )),
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
                edge_ids: route
                    .segments
                    .iter()
                    .map(|segment| segment.edge_id.clone())
                    .collect(),
                route_length_m: route.total_length_m,
            });
            builder.push(swarm_replay::Event::PoseUpdated {
                agent_id: agent_id.clone(),
                pose: start_node.pose,
                tick: 0,
            });
            for state in &analysis_agent_states {
                push_urban_analysis_agent_started(builder, state, &urban_state.map, &route);
            }
        }

        let static_violations = crate::urban::judge_route(&urban_state.map, &route);
        if !static_violations.is_empty() {
            if let Some(ref mut builder) = log_builder {
                for violation in &static_violations {
                    push_urban_violation_event(builder, &agent_id, 0, &route, violation);
                }
            }
            return (
                urban_patrol_metrics(
                    scenario,
                    0,
                    false,
                    true,
                    route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &route),
                    static_violations.len() as u64,
                    false,
                    None,
                    0.0,
                    0.0,
                    None,
                ),
                log_builder.map(|builder| builder.build()),
            );
        }

        let speed_m_per_tick = speed_m_per_tick(agent, config.tick_duration_ms);
        let mut total_ticks = 0;
        let mut completed = route.segments.is_empty();
        let mut completion_tick = completed.then_some(0);
        let mut total_distance_travelled = 0.0;
        let mut segment_index = 0usize;
        let mut distance_on_segment = 0.0;

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

        for tick in 1..=config.max_ticks {
            if completed {
                break;
            }
            total_ticks = tick;
            if let Some(ref mut builder) = log_builder {
                builder.push(swarm_replay::Event::TickStart { tick });
            }

            let mut remaining = speed_m_per_tick;
            while remaining > 0.0 && segment_index < route.segments.len() {
                let segment = &route.segments[segment_index];
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
                        break;
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

        let route_efficiency = route_efficiency(route.total_length_m, total_distance_travelled);
        let route_risk_score = crate::urban::route_risk_score(&urban_state.map, &route);

        let metrics = urban_patrol_metrics(
            scenario,
            total_ticks,
            completed,
            true,
            route.total_length_m,
            route_risk_score,
            0,
            completed,
            completion_tick,
            total_distance_travelled,
            route_efficiency,
            None,
        );
        finish_urban_run_metrics(metrics, log_builder)
    }

    pub(super) fn run_urban_search(
        scenario: &Scenario,
        config: RunConfig,
        mut log_builder: Option<swarm_replay::EventLogBuilder>,
    ) -> (RunMetrics, Option<swarm_replay::EventLog>) {
        let Some(search_state) = config.urban_search_state.clone() else {
            unreachable!("run_urban_search is called only for urban_search_state runs");
        };
        let Some(urban_state) = config.urban_state.clone() else {
            return (
                urban_search_metrics(
                    scenario,
                    0,
                    false,
                    false,
                    0.0,
                    0.0,
                    1,
                    None,
                    0,
                    0.0,
                    0.0,
                    Some("urban_search_missing_urban_state".to_owned()),
                ),
                log_builder.map(|builder| builder.build()),
            );
        };
        let route = match crate::urban::expand_route_loop_with_planner_name(
            &urban_state.map,
            &urban_state.route_loop,
            &urban_state.planner,
        ) {
            Ok(route) => route,
            Err(error) => {
                return (
                    urban_search_metrics(
                        scenario,
                        0,
                        false,
                        false,
                        0.0,
                        0.0,
                        1,
                        None,
                        0,
                        0.0,
                        0.0,
                        Some(error.to_string()),
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
                urban_search_metrics(
                    scenario,
                    0,
                    false,
                    true,
                    route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &route),
                    0,
                    None,
                    0,
                    0.0,
                    0.0,
                    Some("urban_search_no_alive_agent".to_owned()),
                ),
                log_builder.map(|builder| builder.build()),
            );
        };
        let agent_id = agent.id.clone();
        let start_node = match crate::urban::route_start_node(
            &urban_state.map,
            &urban_state.route_loop,
            &route,
            urban_state.start_node.as_ref(),
        ) {
            Ok(start_node) => start_node,
            Err(error) => {
                return (
                    urban_search_metrics(
                        scenario,
                        0,
                        false,
                        true,
                        route.total_length_m,
                        crate::urban::route_risk_score(&urban_state.map, &route),
                        0,
                        None,
                        0,
                        0.0,
                        0.0,
                        Some(format!("urban_search_invalid_start: {error}")),
                    ),
                    log_builder.map(|builder| builder.build()),
                );
            }
        };
        let start_pose_distance = agent.pose.distance_to(&start_node.pose);
        if start_pose_distance > crate::urban::URBAN_START_POSE_TOLERANCE_M {
            return (
                urban_search_metrics(
                    scenario,
                    0,
                    false,
                    true,
                    route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &route),
                    0,
                    None,
                    0,
                    0.0,
                    0.0,
                    Some(format!(
                        "urban_search_invalid_start: agent '{}' starts {:.3}m from start_node '{}'",
                        agent.id, start_pose_distance, start_node.id
                    )),
                ),
                log_builder.map(|builder| builder.build()),
            );
        }

        if let Some(ref mut builder) = log_builder {
            builder.push(swarm_replay::Event::UrbanRoutePlanned {
                agent_id: agent_id.clone(),
                tick: 0,
                edge_ids: route
                    .segments
                    .iter()
                    .map(|segment| segment.edge_id.clone())
                    .collect(),
                route_length_m: route.total_length_m,
            });
            builder.push(swarm_replay::Event::PoseUpdated {
                agent_id: agent_id.clone(),
                pose: start_node.pose,
                tick: 0,
            });
        }

        let static_violations = crate::urban::judge_route(&urban_state.map, &route);
        if !static_violations.is_empty() {
            if let Some(ref mut builder) = log_builder {
                for violation in &static_violations {
                    push_urban_violation_event(builder, &agent_id, 0, &route, violation);
                }
                push_urban_search_completed(builder, &agent_id, 0, false, None, "violation", 0.0);
            }
            return (
                urban_search_metrics(
                    scenario,
                    0,
                    false,
                    true,
                    route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &route),
                    static_violations.len() as u64,
                    None,
                    0,
                    0.0,
                    0.0,
                    None,
                ),
                log_builder.map(|builder| builder.build()),
            );
        }

        let mut false_positive_count = 0;
        let outcome = crate::urban::detect_buses(start_node.pose, 0, scenario.seed, &search_state);
        if let Some(ref mut builder) = log_builder {
            push_detection_events(
                builder,
                &agent_id,
                0,
                start_node.pose,
                search_state.detector.seed,
                &outcome,
            );
        }
        false_positive_count += u64::from(outcome.false_positive);
        if let Some(detection) = outcome.detection {
            if let Some(ref mut builder) = log_builder {
                push_urban_search_completed(
                    builder,
                    &agent_id,
                    0,
                    true,
                    Some(detection.bus_id.clone()),
                    "detected",
                    0.0,
                );
            }
            return (
                urban_search_metrics(
                    scenario,
                    0,
                    true,
                    true,
                    route.total_length_m,
                    crate::urban::route_risk_score(&urban_state.map, &route),
                    0,
                    Some(0),
                    false_positive_count,
                    0.0,
                    0.0,
                    None,
                ),
                log_builder.map(|builder| builder.build()),
            );
        }

        if let Some(first_segment) = route.segments.first() {
            if let Some(ref mut builder) = log_builder {
                push_segment_entered(builder, &agent_id, 0, 0, first_segment);
            }
        }

        let speed_m_per_tick = speed_m_per_tick(agent, config.tick_duration_ms);
        let mut total_ticks = 0;
        let mut total_distance_travelled = 0.0;
        let mut segment_index = 0usize;
        let mut distance_on_segment = 0.0;

        for tick in 1..=config.max_ticks {
            total_ticks = tick;
            if let Some(ref mut builder) = log_builder {
                builder.push(swarm_replay::Event::TickStart { tick });
            }

            let mut remaining = speed_m_per_tick;
            let mut transitions = 0usize;
            while remaining > 0.0
                && !route.segments.is_empty()
                && transitions <= route.segments.len()
            {
                let segment = &route.segments[segment_index];
                let segment_remaining = (segment.length_m - distance_on_segment).max(0.0);
                if segment_remaining <= f64::EPSILON {
                    segment_index = advance_search_segment(
                        &route,
                        segment_index,
                        tick,
                        &agent_id,
                        log_builder.as_mut(),
                    );
                    distance_on_segment = 0.0;
                    transitions += 1;
                    continue;
                }

                if remaining + f64::EPSILON >= segment_remaining {
                    total_distance_travelled += segment_remaining;
                    remaining -= segment_remaining;
                    distance_on_segment = 0.0;
                    segment_index = advance_search_segment(
                        &route,
                        segment_index,
                        tick,
                        &agent_id,
                        log_builder.as_mut(),
                    );
                    transitions += 1;
                } else {
                    distance_on_segment += remaining;
                    total_distance_travelled += remaining;
                    remaining = 0.0;
                }
            }

            let pose = current_urban_pose(
                &urban_state.map,
                &route,
                segment_index,
                distance_on_segment,
                false,
            )
            .unwrap_or(start_node.pose);
            if let Some(ref mut builder) = log_builder {
                builder.push(swarm_replay::Event::PoseUpdated {
                    agent_id: agent_id.clone(),
                    pose,
                    tick,
                });
            }

            let outcome = crate::urban::detect_buses(pose, tick, scenario.seed, &search_state);
            if let Some(ref mut builder) = log_builder {
                push_detection_events(
                    builder,
                    &agent_id,
                    tick,
                    pose,
                    search_state.detector.seed,
                    &outcome,
                );
            }
            false_positive_count += u64::from(outcome.false_positive);

            if let Some(detection) = outcome.detection {
                if let Some(ref mut builder) = log_builder {
                    push_urban_search_completed(
                        builder,
                        &agent_id,
                        tick,
                        true,
                        Some(detection.bus_id.clone()),
                        "detected",
                        total_distance_travelled,
                    );
                }
                return (
                    urban_search_metrics(
                        scenario,
                        total_ticks,
                        true,
                        true,
                        route.total_length_m,
                        crate::urban::route_risk_score(&urban_state.map, &route),
                        0,
                        Some(tick),
                        false_positive_count,
                        total_distance_travelled,
                        route_efficiency(route.total_length_m, total_distance_travelled),
                        None,
                    ),
                    log_builder.map(|builder| builder.build()),
                );
            }
        }

        if let Some(ref mut builder) = log_builder {
            push_urban_search_completed(
                builder,
                &agent_id,
                total_ticks,
                false,
                None,
                "timeout",
                total_distance_travelled,
            );
        }

        (
            urban_search_metrics(
                scenario,
                total_ticks,
                false,
                true,
                route.total_length_m,
                crate::urban::route_risk_score(&urban_state.map, &route),
                0,
                None,
                false_positive_count,
                total_distance_travelled,
                route_efficiency(route.total_length_m, total_distance_travelled),
                None,
            ),
            log_builder.map(|builder| builder.build()),
        )
    }
}
