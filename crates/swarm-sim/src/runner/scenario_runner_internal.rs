use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use swarm_alloc::Allocator;
use swarm_comms::{InMemAgentTransport, InMemNetwork, NetworkConfig};
use swarm_metrics::RunMetrics;
use swarm_runtime::{AgentNode, Coordinator};
use swarm_types::{AdapterRegistry, AgentId, Role, Task, TaskId};

use super::{
    compute_mission_success, compute_urban_foundation_metrics,
    internal::{
        advance_tick, all_dynamic_tasks_injected, all_failure_ticks_passed,
        all_partitions_resolved, apply_environment_effects, apply_partition_events,
        assemble_final_metrics, connectivity_metrics_tick, first_active_agent_id,
        process_alive_nodes, process_wildfire_mapping_tick, record_agent_failures,
        record_inspection_edge_visits, record_safety_violations, record_sar_scans,
        record_tick_start, send_alive_heartbeats, should_stop_tick, tasks_injected_at_tick,
        teleport_assigned_tasks_when_movement_disabled, update_connectivity_snapshot,
        update_view_divergence, MetricsInput, MissionStopSnapshot,
    },
    released_tasks_reassigned, update_unassigned_durations, RunConfig, SafetyAllocator,
    ScenarioRunner,
};
use crate::{Clock, Scenario};

impl ScenarioRunner {
    pub(super) fn run_internal<A: Allocator>(
        scenario: &Scenario,
        config: RunConfig,
        allocator: A,
        mut log_builder: Option<swarm_replay::EventLogBuilder>,
    ) -> (RunMetrics, Option<swarm_replay::EventLog>) {
        if config.urban_search_state.is_some() {
            return Self::run_urban_search(scenario, config, log_builder);
        }
        if config.urban_state.is_some() {
            return Self::run_urban_patrol(scenario, config, log_builder);
        }

        let mut inspection_state = config.inspection_state;
        let urban_state = config.urban_state.clone();
        let (
            urban_route_planned,
            urban_route_length_m,
            urban_route_risk_score,
            urban_violation_count,
        ) = compute_urban_foundation_metrics(&urban_state);
        let urban_route_completed = false;
        let mut allocator = SafetyAllocator {
            inner: allocator,
            safety_config: config.safety_config.clone(),
        };
        let bus = Rc::new(RefCell::new(InMemNetwork::new(NetworkConfig {
            packet_loss_rate: config.packet_loss_rate,
            latency_ticks: config.latency_ticks,
            latency_per_hop: config.latency_per_hop,
            seed: scenario.seed,
            partitions: HashSet::new(),
            comms_jitter_ticks: config.comms_jitter_ticks,
        })));

        let agent_ids: Vec<AgentId> = scenario.agents.iter().map(|a| a.id.clone()).collect();

        let mut nodes: Vec<(AgentNode<InMemAgentTransport>, AgentId)> = scenario
            .agents
            .iter()
            .map(|agent| {
                let peer_ids: Vec<AgentId> = agent_ids
                    .iter()
                    .filter(|id| *id != &agent.id)
                    .cloned()
                    .collect();
                let transport = InMemAgentTransport::new(bus.clone(), agent.id.clone());
                let coordinator = Coordinator::new(
                    scenario.agents.clone(),
                    scenario.tasks.clone(),
                    config.timeout_ticks,
                );
                let mut node = AgentNode::new(agent.id.clone(), peer_ids, coordinator, transport);
                node.gossip_interval_ticks = config.gossip_interval_ticks;
                node.config.enable_movement = config.enable_movement;
                node.config.tick_duration_ms = config.tick_duration_ms;
                if config.enable_cbba {
                    #[allow(clippy::field_reassign_with_default)]
                    {
                        let mut cbba = swarm_alloc::CbbaAllocator::default();
                        cbba.packet_loss_rate = config.packet_loss_rate;
                        node.cbba = Some(cbba);
                    }
                }
                (node, agent.id.clone())
            })
            .collect();

        let mut clock = Clock::new(1);
        let failure_ticks: HashMap<AgentId, u64> = config
            .failures
            .iter()
            .map(|failure| (failure.agent_id.clone(), failure.at_tick))
            .collect();
        let mut crashed_agents: HashSet<AgentId> = HashSet::new();
        let mut detected_agents: HashSet<AgentId> = HashSet::new();
        let mut unassigned_durations: HashMap<TaskId, u64> = HashMap::new();
        let mut max_task_unassigned_ticks = 0;
        let mut detection_time_ticks = None;
        let mut detection_tick = None;
        let mut reallocation_time_ticks = None;
        let mut total_ticks = 0;
        let mut tasks_injected: u64 = 0;
        let mut tasks_expired: u64 = 0;
        let mut conflicting_assignments: u64 = 0;
        let mut stale_messages_discarded: u64 = 0;
        let mut partition_events: u64 = 0;
        let mut partitions_active: bool = false;
        let mut convergence_ticks: Option<u64> = None;
        let mut heal_tick: Option<u64> = None;
        let mut max_view_divergence: u64 = 0;

        // v0.16 inspection metrics
        let mut revisit_count: u64 = 0;

        // v0.8 movement metrics
        let mut total_distance_travelled: f64 = 0.0;
        let mut time_to_first_exhaustion: Option<u64> = None;

        // v0.13 safety metrics
        let mut safety_violations: u64 = 0;

        // v0.15 CBBA convergence tick tracking
        let mut cbba_convergence_tick: Option<u64> = None;

        // v0.33 Adapter registry for mission-semantic completion checks
        let adapter_registry = AdapterRegistry::new();

        // v0.30 Wildfire Mapping metrics
        let mut wildfire_state = config.wildfire_state;
        let mut priority_updates: u64 = 0;
        // v0.38 Wildfire v2 metrics
        let mut high_priority_zones_mapped: u64 = 0;
        let mut time_to_map_first_high_risk: Option<u64> = None;
        let mut threat_level_over_time: Vec<f64> = Vec::new();
        let mut zone_observations: u64 = 0;

        // v0.9 SAR metrics
        let mut coverage_over_time: Vec<f64> = Vec::new();
        let mut grid_state = config.grid_state;

        // v0.5 connectivity metrics
        let mut availability_per_tick: Vec<f64> = Vec::new();
        let mut disconnected_agents_max: u64 = 0;
        let mut relay_reallocation_ticks: Option<u64> = None;
        let mut relay_detection_tick: Option<u64> = None;
        let mut total_hop_count_sum: f64 = 0.0;
        let mut total_hop_count_ticks: u64 = 0;
        let base_id = config
            .base_id
            .clone()
            .unwrap_or_else(|| AgentId::from("base".to_owned()));
        let base_pose = scenario.base_station.unwrap_or(swarm_types::Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        });

        for _ in 0..config.max_ticks {
            let current_tick = advance_tick(&mut clock);
            total_ticks = current_tick;

            record_tick_start(&mut log_builder, current_tick);

            record_agent_failures(
                &config.failures,
                current_tick,
                &mut crashed_agents,
                &mut log_builder,
            );

            bus.borrow_mut().advance_tick();

            let partition_tick = apply_partition_events(
                &config.partition_events,
                current_tick,
                &bus,
                &mut nodes,
                &mut log_builder,
            );
            partition_events += partition_tick.partition_events;
            partitions_active |= partition_tick.partitions_active;
            if partition_tick.heal_tick.is_some() {
                heal_tick = partition_tick.heal_tick;
            }

            let injected = tasks_injected_at_tick(&config.dynamic_tasks, current_tick);
            tasks_injected += injected.len() as u64;

            // v0.5: Update connectivity snapshot on the network bus before heartbeats/gossip.
            // Include all non-crashed agents (not just alive) so that partition-induced
            // false failure detection does not permanently break mesh reachability after heal.
            update_connectivity_snapshot(
                &nodes,
                &crashed_agents,
                &bus,
                scenario,
                &base_id,
                base_pose,
            );

            send_alive_heartbeats(&mut nodes, &crashed_agents, current_tick);
            let tick_outputs = process_alive_nodes(
                &mut nodes,
                &crashed_agents,
                current_tick,
                &mut allocator,
                &injected,
            );

            // v0.5: Pose update — only teleport when movement is disabled.
            // When enable_movement=true, agents move gradually via apply_movement.
            teleport_assigned_tasks_when_movement_disabled(
                &mut nodes,
                &crashed_agents,
                config.enable_movement,
            );

            // v0.31: Wind drift and pose noise (applied after movement to own agent's view)
            apply_environment_effects(
                &mut nodes,
                &crashed_agents,
                config.wind,
                config.pose_noise_m,
                config.tick_duration_ms,
                scenario.seed,
                current_tick,
            );

            // v0.13: Safety checks after movement/teleport
            if let Some(ref safety_cfg) = config.safety_config {
                safety_violations += record_safety_violations(
                    &mut nodes,
                    &crashed_agents,
                    scenario,
                    safety_cfg,
                    current_tick,
                    &mut log_builder,
                );
            }

            // v0.15: Track CBBA convergence tick
            if cbba_convergence_tick.is_none()
                && nodes
                    .iter()
                    .filter(|(_, id)| !crashed_agents.contains(id))
                    .all(|(n, _)| n.cbba.as_ref().is_none_or(|c| c.converged))
            {
                cbba_convergence_tick = Some(current_tick);
                if let Some(ref mut builder) = log_builder {
                    builder.push(swarm_replay::Event::CbbaConverged { tick: current_tick });
                }
            }

            if let Some(ref mut inspection_state) = inspection_state {
                revisit_count += record_inspection_edge_visits(
                    &mut nodes,
                    &crashed_agents,
                    inspection_state,
                    current_tick,
                    &mut log_builder,
                );
            }

            if let Some(ref mut grid_state) = grid_state {
                record_sar_scans(
                    &mut nodes,
                    &crashed_agents,
                    grid_state,
                    scenario.seed,
                    current_tick,
                    &mut log_builder,
                );
                coverage_over_time.push(grid_state.coverage_fraction());
            }

            if let Some(ref mut wildfire_state) = wildfire_state {
                let wildfire_tick = process_wildfire_mapping_tick(
                    &mut nodes,
                    &crashed_agents,
                    wildfire_state,
                    config.wind,
                    current_tick,
                    time_to_map_first_high_risk.is_some(),
                    &mut log_builder,
                );
                priority_updates += wildfire_tick.priority_updates;
                high_priority_zones_mapped += wildfire_tick.high_priority_zones_mapped;
                if time_to_map_first_high_risk.is_none() {
                    time_to_map_first_high_risk = wildfire_tick.time_to_map_first_high_risk;
                }
                zone_observations += wildfire_tick.zone_observations;
                threat_level_over_time.push(wildfire_tick.avg_threat_level);
            }

            // v0.5: Compute connectivity metrics for this tick
            if let Some(connectivity) =
                connectivity_metrics_tick(&nodes, &crashed_agents, scenario, &base_id, base_pose)
            {
                availability_per_tick.push(connectivity.availability);
                disconnected_agents_max =
                    disconnected_agents_max.max(connectivity.disconnected_agents);
                if let Some(average_hop_count) = connectivity.average_hop_count {
                    total_hop_count_sum += average_hop_count;
                    total_hop_count_ticks += 1;
                }
            }

            // Aggregate outputs across all agents
            for (_agent_id, output) in &tick_outputs {
                conflicting_assignments += output.conflicting_assignments;
                stale_messages_discarded += output.discarded_messages;

                if detection_time_ticks.is_none() && !output.newly_failed.is_empty() {
                    let first_failure_tick = output
                        .newly_failed
                        .iter()
                        .filter_map(|agent_id| failure_ticks.get(agent_id))
                        .min()
                        .copied()
                        .unwrap_or(current_tick);
                    detection_time_ticks = Some(current_tick.saturating_sub(first_failure_tick));
                    detection_tick = Some(current_tick);
                }
                detected_agents.extend(output.newly_failed.iter().cloned());

                // v0.8: aggregate movement metrics
                for (_agent_id, distance) in &output.distance_travelled {
                    total_distance_travelled += distance;
                }
                if time_to_first_exhaustion.is_none()
                    && output.newly_failed.iter().any(|id| {
                        nodes
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
                    time_to_first_exhaustion = Some(current_tick);
                }
            }

            // Use first non-crashed agent's coordinator for state checks
            let first_id = first_active_agent_id(&nodes, &crashed_agents);

            update_view_divergence(
                &nodes,
                &crashed_agents,
                current_tick,
                heal_tick,
                &mut max_view_divergence,
                &mut convergence_ticks,
            );

            // Count expired tasks from first agent only (replicated state)
            if let Some(ref target_id) = first_id {
                if let Some((_, output)) = tick_outputs.iter().find(|(id, _)| id == target_id) {
                    tasks_expired += output.expired_task_ids.len() as u64;
                    if let Some(ref mut builder) = log_builder {
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
                if let Some((node, _)) = nodes.iter().find(|(_, id)| id == target_id) {
                    max_task_unassigned_ticks = update_unassigned_durations(
                        &node.coordinator,
                        &mut unassigned_durations,
                        max_task_unassigned_ticks,
                    );

                    if let Some(detected_at) = detection_tick {
                        if reallocation_time_ticks.is_none() {
                            let target_output = tick_outputs
                                .iter()
                                .find(|(id, _)| id == target_id)
                                .map(|(_, out)| &out.released_tasks);
                            if let Some(released) = target_output {
                                if released_tasks_reassigned(&node.coordinator, released) {
                                    reallocation_time_ticks =
                                        Some(current_tick.saturating_sub(detected_at));
                                }
                            }
                        }
                    }

                    // v0.5: Track relay reallocation
                    if relay_reallocation_ticks.is_none() {
                        // Check if any relay agent was detected as failed this tick
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
                            relay_detection_tick = Some(current_tick);
                        }

                        if let Some(det_at) = relay_detection_tick {
                            // Check if all relay tasks are assigned to alive agents
                            let all_relay_tasks_reassigned = node
                                .coordinator
                                .registry
                                .tasks()
                                .filter(|t| t.required_role == Some(Role::Relay))
                                .all(|t| {
                                    t.assigned_to.as_ref().is_some_and(|aid| {
                                        node.coordinator.membership.is_alive(aid)
                                    })
                                });
                            if all_relay_tasks_reassigned {
                                relay_reallocation_ticks =
                                    Some(current_tick.saturating_sub(det_at));
                            }
                        }
                    }
                }
            }

            let all_expected_failures_detected = crashed_agents
                .iter()
                .all(|agent_id| detected_agents.contains(agent_id));
            let all_failure_ticks_passed = all_failure_ticks_passed(&config.failures, current_tick);
            let all_dynamic_tasks_injected =
                all_dynamic_tasks_injected(&config.dynamic_tasks, current_tick);
            let all_partitions_resolved =
                all_partitions_resolved(&config.partition_events, current_tick);
            // Don't break early while partitions are still pending
            let post_partition_converged = if all_partitions_resolved {
                convergence_ticks.is_some() || max_view_divergence == 0
            } else {
                // Partitions are pending — keep running
                false
            };
            let all_tasks_assigned = nodes
                .iter()
                .find(|(_, id)| !crashed_agents.contains(id))
                .is_some_and(|(node, _)| node.coordinator.registry.all_assigned_or_completed());

            // v0.33 adapter-driven completion checks — use live tasks from the registry so
            // that statuses (Assigned, Completed) reflect the current simulation state rather
            // than the initial snapshot stored in scenario.tasks.
            let live_tasks: Vec<Task> = nodes
                .iter()
                .find(|(_, id)| !crashed_agents.contains(id))
                .map(|(node, _)| node.coordinator.registry.tasks().cloned().collect())
                .unwrap_or_default();
            let run_state =
                Self::build_run_state(&grid_state, &inspection_state, &wildfire_state, &live_tasks);
            let adapter_complete =
                Self::adapter_driven_complete(&live_tasks, &run_state, &adapter_registry);

            // Legacy mission-specific checks preserved as fallback
            let sar_complete = grid_state.as_ref().is_none_or(|g| g.all_targets_found());

            let inspection_complete = inspection_state
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
                max_task_unassigned_ticks,
                config.max_unassigned_ticks,
            ) {
                break;
            }
        }

        let all_expected_failures_detected = config
            .failures
            .iter()
            .all(|failure| detected_agents.contains(&failure.agent_id));
        let all_tasks_assigned = nodes
            .iter()
            .find(|(_, id)| !crashed_agents.contains(id))
            .is_some_and(|(node, _)| node.coordinator.registry.all_assigned_or_completed());

        // v0.35: Recompute adapter_complete after loop for final success determination.
        let final_live_tasks: Vec<Task> = nodes
            .iter()
            .find(|(_, id)| !crashed_agents.contains(id))
            .map(|(node, _)| node.coordinator.registry.tasks().cloned().collect())
            .unwrap_or_default();
        let final_run_state = Self::build_run_state(
            &grid_state,
            &inspection_state,
            &wildfire_state,
            &final_live_tasks,
        );
        let adapter_complete =
            Self::adapter_driven_complete(&final_live_tasks, &final_run_state, &adapter_registry);

        // v0.35: Mission-specific success semantics
        let (success, unsupported_reason) = compute_mission_success(
            config.max_unassigned_ticks,
            &config.strategy_name,
            config.wildfire_success_threshold,
            config.inspection_coverage_threshold,
            all_tasks_assigned,
            all_expected_failures_detected,
            max_task_unassigned_ticks,
            &grid_state,
            &inspection_state,
            &wildfire_state,
            &urban_state,
            urban_route_planned,
            urban_violation_count,
            urban_route_completed,
            adapter_complete,
        );

        let msgs_attempted = bus.borrow().messages_attempted();
        let msgs_dropped = bus.borrow().messages_dropped();
        let bytes_sent = bus.borrow().bytes_sent();
        drop(bus);

        assemble_final_metrics(MetricsInput {
            msgs_attempted,
            msgs_dropped,
            bytes_sent,
            nodes,
            crashed_agents,
            grid_state,
            inspection_state,
            wildfire_state,
            seed: scenario.seed,
            total_ticks,
            total_distance_travelled,
            detection_time_ticks,
            reallocation_time_ticks,
            max_task_unassigned_ticks,
            all_tasks_assigned,
            success,
            tasks_injected,
            tasks_expired,
            conflicting_assignments,
            partition_events,
            partitions_active,
            stale_messages_discarded,
            convergence_ticks,
            max_view_divergence,
            relay_reallocation_ticks,
            disconnected_agents_max,
            cbba_convergence_tick,
            safety_violations,
            revisit_count,
            priority_updates,
            high_priority_zones_mapped,
            time_to_map_first_high_risk,
            zone_observations,
            time_to_first_exhaustion,
            coverage_over_time,
            threat_level_over_time,
            availability_per_tick,
            total_hop_count_sum,
            total_hop_count_ticks,
            base_pose,
            realism_profile: config.realism_profile,
            wind: config.wind,
            urban_route_planned,
            urban_route_length_m,
            urban_route_risk_score,
            urban_violation_count,
            urban_route_completed,
            unsupported_reason,
            log_builder,
        })
    }
}
