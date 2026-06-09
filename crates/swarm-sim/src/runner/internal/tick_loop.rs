use swarm_comms::AgentMissionState;

use super::super::*;
use super::{
    all_dynamic_tasks_injected, all_failure_ticks_passed, all_partitions_resolved,
    apply_environment_effects, apply_partition_events, connectivity_metrics_tick,
    handle_node_failures, handle_partition_activation, handle_partition_heal, process_alive_nodes,
    process_wildfire_mapping_tick, record_agent_failures, record_inspection_edge_visits,
    record_safety_violations, record_sar_scans, record_tick_start, send_alive_heartbeats,
    send_gcs_heartbeats, should_stop_tick, tasks_injected_at_tick,
    teleport_assigned_tasks_when_movement_disabled, update_connectivity_snapshot,
    MissionStopSnapshot, TickLoopState,
};
use std::collections::{HashMap, HashSet};
use swarm_types::Role;

pub(in crate::runner) fn advance_tick(clock: &mut Clock) -> u64 {
    clock.advance();
    u64::from(clock.now())
}

pub(in crate::runner) fn first_active_agent_id<T: Transport>(
    nodes: &[(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
) -> Option<AgentId> {
    nodes
        .iter()
        .find(|(_, id)| !crashed_agents.contains(id))
        .map(|(_, id)| id.clone())
}

pub(in crate::runner) fn update_view_divergence<T: Transport>(
    nodes: &[(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    current_tick: u64,
    heal_tick: Option<u64>,
    max_view_divergence: &mut u64,
    convergence_ticks: &mut Option<u64>,
) {
    let maps: Vec<HashMap<TaskId, AgentId>> = nodes
        .iter()
        .filter(|(_, id)| !crashed_agents.contains(id))
        .map(|(node, _)| {
            node.coordinator
                .registry
                .tasks()
                .filter_map(|task| {
                    task.assigned_to
                        .clone()
                        .map(|agent_id| (task.id.clone(), agent_id))
                })
                .collect::<HashMap<_, _>>()
        })
        .collect();
    if maps.is_empty() {
        return;
    }

    let reference = &maps[0];
    let diverged = maps.iter().filter(|map| *map != reference).count() as u64;
    *max_view_divergence = (*max_view_divergence).max(diverged);

    if let Some(heal_at) = heal_tick {
        if current_tick > heal_at && diverged == 0 && convergence_ticks.is_none() {
            *convergence_ticks = Some(current_tick - heal_at);
        }
    }
}

pub(in crate::runner) fn run_tick_loop<A: Allocator>(
    state: &mut TickLoopState<A>,
    scenario: &Scenario,
    config: &RunConfig,
) {
    for _ in 0..config.max_ticks {
        let current_tick = advance_tick(&mut state.clock);
        state.total_ticks = current_tick;

        record_tick_start(&mut state.log_builder, current_tick);

        let crashed_before_tick = state.crashed_agents.clone();
        record_agent_failures(
            &config.failures,
            current_tick,
            &mut state.crashed_agents,
            &mut state.log_builder,
        );
        let newly_failed_agents = state
            .crashed_agents
            .difference(&crashed_before_tick)
            .cloned()
            .collect::<HashSet<_>>();

        state.bus.borrow_mut().advance_tick();

        let partition_tick = apply_partition_events(
            &config.partition_events,
            current_tick,
            &state.bus,
            &mut state.nodes,
            &mut state.log_builder,
        );
        state.partition_events += partition_tick.partition_events;
        state.partitions_active |= partition_tick.partitions_active;
        if partition_tick.heal_tick.is_some() {
            state.heal_tick = partition_tick.heal_tick;
        }
        for (agent_a, agent_b) in &partition_tick.added_pairs {
            state
                .active_partition_pairs
                .insert((agent_a.clone(), agent_b.clone()));
            handle_partition_activation(
                &state.nodes,
                &state.crashed_agents,
                &state.active_partition_pairs,
                &(agent_a.clone(), agent_b.clone()),
                current_tick,
                &mut state.degraded_decision_log,
                &mut state.partition_reports,
                &mut state.log_builder,
            );
        }
        for (agent_a, agent_b) in &partition_tick.healed_pairs {
            state
                .active_partition_pairs
                .remove(&(agent_a.clone(), agent_b.clone()));
            handle_partition_heal(
                &mut state.nodes,
                &state.crashed_agents,
                current_tick,
                &(agent_a.clone(), agent_b.clone()),
                config.tick_duration_ms,
                &mut state.degraded_decision_log,
                &mut state.partition_reports,
                &mut state.reconciliation_reports,
                &mut state.log_builder,
            );
        }
        handle_node_failures(
            &mut state.nodes,
            &newly_failed_agents,
            current_tick,
            &mut state.degraded_decision_log,
        );

        let injected = tasks_injected_at_tick(&config.dynamic_tasks, current_tick);
        state.tasks_injected += injected.len() as u64;

        update_connectivity_snapshot(
            &state.nodes,
            &state.crashed_agents,
            &state.bus,
            scenario,
            &state.base_id,
            state.base_pose,
        );

        // M93: inject GCS heartbeat from base_id; partition events will block it when needed
        send_gcs_heartbeats(
            &state.bus,
            &state.nodes,
            &state.crashed_agents,
            &state.base_id,
            current_tick,
        );
        send_alive_heartbeats(&mut state.nodes, &state.crashed_agents, current_tick);
        let tick_outputs = process_alive_nodes(
            &mut state.nodes,
            &state.crashed_agents,
            current_tick,
            &mut state.allocator,
            &injected,
        );
        let first_id = first_active_agent_id(&state.nodes, &state.crashed_agents);

        if let Some(ref target_id) = first_id {
            if let Some((_, output)) = tick_outputs.iter().find(|(id, _)| id == target_id) {
                if let Some(ref mut builder) = state.log_builder {
                    for assignment in &output.reassigned_tasks {
                        builder.push(swarm_replay::Event::TaskAssigned {
                            task_id: assignment.task_id.clone(),
                            agent_id: assignment.agent_id.clone(),
                            tick: current_tick,
                        });
                    }
                }
            }
        }

        // M93: process autonomy FSM events from all agents
        for (agent_id, output) in &tick_outputs {
            if output.gcs_lost_this_tick {
                state.gcs_lost_count += 1;
                if output
                    .gcs_lost_policy_name
                    .as_deref()
                    .is_some_and(|p| p == "return_to_launch")
                {
                    state.failsafe_rtl_count += 1;
                }
                if let Some(ref mut builder) = state.log_builder {
                    builder.push(swarm_replay::Event::AgentGcsLost {
                        agent_id: agent_id.clone(),
                        tick: current_tick,
                        policy: output.gcs_lost_policy_name.clone().unwrap_or_default(),
                    });
                }
            }
            if output.gcs_reconnected_this_tick {
                if let Some(ref mut builder) = state.log_builder {
                    builder.push(swarm_replay::Event::AgentGcsReconnected {
                        agent_id: agent_id.clone(),
                        tick: current_tick,
                        gcs_lost_ticks: output.gcs_recovered_lost_ticks,
                    });
                }
                if let Some(ref report) = output.reconcile_report {
                    if let Some(ref mut builder) = state.log_builder {
                        builder.push(swarm_replay::Event::AgentStateReconciled {
                            agent_id: agent_id.clone(),
                            tick: current_tick,
                            gcs_lost_ticks: report.gcs_lost_ticks,
                            policy_applied: report.policy_applied.clone(),
                            active_lease_count: report.active_leases_at_reconnect.len() as u64,
                            mission_state_name: format!("{:?}", report.mission_state_at_reconnect),
                        });
                    }
                }
            }
            if let Some(ref lease_id) = output.continuing_under_lease_this_tick {
                if let Some(ref mut builder) = state.log_builder {
                    builder.push(swarm_replay::Event::AgentContinuingUnderLease {
                        agent_id: agent_id.clone(),
                        lease_id: lease_id.clone(),
                        tick: current_tick,
                    });
                }
            }
            if let Some((ref lease_id, ref policy)) = output.lease_expired_in_gcs_loss {
                state.lease_expired_during_gcs_loss_count += 1;
                if let Some(ref mut builder) = state.log_builder {
                    builder.push(swarm_replay::Event::AgentLeaseExpiredDuringGcsLoss {
                        agent_id: agent_id.clone(),
                        lease_id: lease_id.clone(),
                        policy_applied: policy.clone(),
                        tick: current_tick,
                    });
                }
            }
            for lost_id in &output.neighbors_lost_this_tick {
                state.neighbor_lost_count += 1;
                if let Some(ref mut builder) = state.log_builder {
                    builder.push(swarm_replay::Event::AgentNeighborLost {
                        agent_id: agent_id.clone(),
                        lost_neighbor_id: lost_id.clone(),
                        tick: current_tick,
                    });
                }
            }
        }
        // M93: accumulate ticks spent in GCS-degraded states
        for (node, agent_id) in &state.nodes {
            if state.crashed_agents.contains(agent_id) {
                continue;
            }
            match &node.mission_state {
                AgentMissionState::GcsLost { .. }
                | AgentMissionState::ContinuingUnderLease { .. } => {
                    state.gcs_lost_total_ticks += 1;
                }
                _ => {}
            }
        }

        teleport_assigned_tasks_when_movement_disabled(
            &mut state.nodes,
            &state.crashed_agents,
            config.enable_movement,
        );

        apply_environment_effects(
            &mut state.nodes,
            &state.crashed_agents,
            config.wind,
            config.pose_noise_m,
            config.tick_duration_ms,
            scenario.seed,
            current_tick,
        );

        if let Some(ref safety_cfg) = config.safety_config {
            state.safety_violations += record_safety_violations(
                &mut state.nodes,
                &state.crashed_agents,
                scenario,
                safety_cfg,
                current_tick,
                &mut state.log_builder,
            );
        }

        if state.cbba_convergence_tick.is_none()
            && state
                .nodes
                .iter()
                .filter(|(_, id)| !state.crashed_agents.contains(id))
                .all(|(n, _)| n.cbba.as_ref().is_none_or(|c| c.converged))
        {
            state.cbba_convergence_tick = Some(current_tick);
            if let Some(ref mut builder) = state.log_builder {
                builder.push(swarm_replay::Event::CbbaConverged { tick: current_tick });
            }
        }

        if let Some(ref mut inspection_state) = state.inspection_state {
            state.revisit_count += record_inspection_edge_visits(
                &mut state.nodes,
                &state.crashed_agents,
                inspection_state,
                current_tick,
                &mut state.log_builder,
            );
        }

        if let Some(ref mut grid_state) = state.grid_state {
            record_sar_scans(
                &mut state.nodes,
                &state.crashed_agents,
                grid_state,
                scenario.seed,
                current_tick,
                config.dynamic_belief_updates,
                &mut state.log_builder,
            );
            state
                .coverage_over_time
                .push(grid_state.coverage_fraction());
        }

        if let Some(ref mut wildfire_state) = state.wildfire_state {
            let wildfire_tick = process_wildfire_mapping_tick(
                &mut state.nodes,
                &state.crashed_agents,
                wildfire_state,
                config.wind,
                config.wildfire_priority_realloc_threshold,
                current_tick,
                state.time_to_map_first_high_risk.is_some(),
                &mut state.log_builder,
            );
            state.priority_updates += wildfire_tick.priority_updates;
            state.high_priority_zones_mapped += wildfire_tick.high_priority_zones_mapped;
            if state.time_to_map_first_high_risk.is_none() {
                state.time_to_map_first_high_risk = wildfire_tick.time_to_map_first_high_risk;
            }
            state.zone_observations += wildfire_tick.zone_observations;
            state
                .threat_level_over_time
                .push(wildfire_tick.avg_threat_level);
            for request in wildfire_tick.priority_reallocation_requests {
                let mut released_previous_agent = None;
                for (node, agent_id) in &mut state.nodes {
                    if state.crashed_agents.contains(agent_id) {
                        continue;
                    }
                    if let Some(previous_agent_id) =
                        node.coordinator.registry.release_task(&request.task_id)
                    {
                        released_previous_agent.get_or_insert(previous_agent_id);
                    }
                }
                if let (Some(builder), Some(previous_agent_id)) =
                    (&mut state.log_builder, released_previous_agent)
                {
                    builder.push(swarm_replay::Event::WildfirePriorityTaskReleased {
                        task_id: request.task_id,
                        old_priority: request.old_priority,
                        new_priority: request.new_priority,
                        previous_agent_id: Some(previous_agent_id),
                        tick: current_tick,
                    });
                }
            }
        }

        if let Some(connectivity) = connectivity_metrics_tick(
            &state.nodes,
            &state.crashed_agents,
            scenario,
            &state.base_id,
            state.base_pose,
        ) {
            state.availability_per_tick.push(connectivity.availability);
            state.disconnected_agents_max = state
                .disconnected_agents_max
                .max(connectivity.disconnected_agents);
            if let Some(average_hop_count) = connectivity.average_hop_count {
                state.total_hop_count_sum += average_hop_count;
                state.total_hop_count_ticks += 1;
            }
        }

        for (_agent_id, output) in &tick_outputs {
            state.conflicting_assignments += output.conflicting_assignments;
            state.stale_messages_discarded += output.discarded_messages;

            if state.detection_time_ticks.is_none() && !output.newly_failed.is_empty() {
                let first_failure_tick = output
                    .newly_failed
                    .iter()
                    .filter_map(|agent_id| state.failure_ticks.get(agent_id))
                    .min()
                    .copied()
                    .unwrap_or(current_tick);
                state.detection_time_ticks = Some(current_tick.saturating_sub(first_failure_tick));
                state.detection_tick = Some(current_tick);
            }
            state
                .detected_agents
                .extend(output.newly_failed.iter().cloned());

            for (_agent_id, distance) in &output.distance_travelled {
                state.total_distance_travelled += distance;
            }
            if state.time_to_first_exhaustion.is_none()
                && output.newly_failed.iter().any(|id| {
                    state
                        .nodes
                        .iter()
                        .find(|(n, _)| &n.own_id == id)
                        .is_some_and(|(n, _)| {
                            n.coordinator
                                .membership
                                .get(id)
                                .is_some_and(|e| e.battery <= 0.0)
                        })
                })
            {
                state.time_to_first_exhaustion = Some(current_tick);
            }
        }

        if let Some(ref mut builder) = state.log_builder {
            for (node, agent_id) in &state.nodes {
                if state.crashed_agents.contains(agent_id) {
                    continue;
                }
                if let Some(cbba) = node.cbba.as_ref() {
                    builder.push(swarm_replay::Event::CbbaBundleUpdated {
                        agent_id: agent_id.clone(),
                        bundle_size: cbba.bundles.get(agent_id).map(Vec::len).unwrap_or_default(),
                        conflict_count: tick_outputs
                            .iter()
                            .find(|(output_agent_id, _)| output_agent_id == agent_id)
                            .map(|(_, output)| output.conflicting_assignments)
                            .unwrap_or_default(),
                        tick: current_tick,
                    });
                }
            }
        }

        update_view_divergence(
            &state.nodes,
            &state.crashed_agents,
            current_tick,
            state.heal_tick,
            &mut state.max_view_divergence,
            &mut state.convergence_ticks,
        );

        if let Some(ref target_id) = first_id {
            if let Some((_, output)) = tick_outputs.iter().find(|(id, _)| id == target_id) {
                state.tasks_expired += output.expired_task_ids.len() as u64;
                if let Some(ref mut builder) = state.log_builder {
                    for task_id in &output.expired_task_ids {
                        builder.push(swarm_replay::Event::TaskExpired {
                            task_id: task_id.clone(),
                            tick: current_tick,
                        });
                    }
                }
            }
        }

        if let Some(ref target_id) = first_id {
            if let Some((node, _)) = state.nodes.iter().find(|(_, id)| id == target_id) {
                state.max_task_unassigned_ticks = update_unassigned_durations(
                    &node.coordinator,
                    &mut state.unassigned_durations,
                    state.max_task_unassigned_ticks,
                );

                if let Some(detected_at) = state.detection_tick {
                    if state.reallocation_time_ticks.is_none() {
                        let target_output = tick_outputs
                            .iter()
                            .find(|(id, _)| id == target_id)
                            .map(|(_, out)| &out.released_tasks);
                        if let Some(released) = target_output {
                            if released_tasks_reassigned(&node.coordinator, released) {
                                state.reallocation_time_ticks =
                                    Some(current_tick.saturating_sub(detected_at));
                            }
                        }
                    }
                }

                if state.relay_reallocation_ticks.is_none() {
                    let relay_failed_this_tick: Vec<AgentId> = tick_outputs
                        .iter()
                        .flat_map(|(_, out)| out.newly_failed.iter().cloned())
                        .filter(|failed_id| {
                            node.coordinator
                                .membership
                                .get(failed_id)
                                .is_some_and(|e| e.role == Role::Relay)
                        })
                        .collect();
                    if !relay_failed_this_tick.is_empty() {
                        state.relay_detection_tick = Some(current_tick);
                    }

                    if let Some(det_at) = state.relay_detection_tick {
                        let all_relay_tasks_reassigned = node
                            .coordinator
                            .registry
                            .tasks()
                            .filter(|t| t.required_role == Some(Role::Relay))
                            .all(|t| {
                                t.assigned_to
                                    .as_ref()
                                    .is_some_and(|aid| node.coordinator.membership.is_alive(aid))
                            });
                        if all_relay_tasks_reassigned {
                            state.relay_reallocation_ticks =
                                Some(current_tick.saturating_sub(det_at));
                        }
                    }
                }
            }
        }

        let all_expected_failures_detected = state
            .crashed_agents
            .iter()
            .all(|agent_id| state.detected_agents.contains(agent_id));
        let all_failure_ticks_passed = all_failure_ticks_passed(&config.failures, current_tick);
        let all_dynamic_tasks_injected =
            all_dynamic_tasks_injected(&config.dynamic_tasks, current_tick);
        let all_partitions_resolved =
            all_partitions_resolved(&config.partition_events, current_tick);
        let post_partition_converged = if all_partitions_resolved {
            state.convergence_ticks.is_some() || state.max_view_divergence == 0
        } else {
            false
        };
        let all_tasks_assigned = state
            .nodes
            .iter()
            .find(|(_, id)| !state.crashed_agents.contains(id))
            .is_some_and(|(node, _)| node.coordinator.registry.all_assigned_or_completed());

        let live_tasks: Vec<Task> = state
            .nodes
            .iter()
            .find(|(_, id)| !state.crashed_agents.contains(id))
            .map(|(node, _)| node.coordinator.registry.tasks().cloned().collect())
            .unwrap_or_default();
        let run_state = ScenarioRunner::build_run_state(
            &state.grid_state,
            &state.inspection_state,
            &state.wildfire_state,
            &live_tasks,
        );
        let adapter_complete = ScenarioRunner::adapter_driven_complete(
            &live_tasks,
            &run_state,
            &state.adapter_registry,
        );

        let sar_complete = state
            .grid_state
            .as_ref()
            .is_none_or(|g| g.all_targets_found());
        let inspection_complete = state
            .inspection_state
            .as_ref()
            .is_none_or(|s| s.covered.len() == s.graph.edges.len());

        if should_stop_tick(
            MissionStopSnapshot {
                all_tasks_assigned,
                all_failure_ticks_passed,
                all_expected_failures_detected,
                all_dynamic_tasks_injected,
                post_partition_converged,
                sar_complete,
                inspection_complete,
                adapter_complete,
            },
            state.max_task_unassigned_ticks,
            config.max_unassigned_ticks,
        ) {
            break;
        }
    }
}
