use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use swarm_alloc::{
    route_cost, Allocator, BatteryAwarePlanner, NearestNeighbourPlanner, RoutePlanner,
};
use swarm_comms::{InMemAgentTransport, InMemNetwork, NetworkConfig};
use swarm_metrics::RunMetrics;
use swarm_runtime::{AgentNode, Coordinator};
use swarm_types::{AdapterRegistry, AgentId, Role, Task, TaskId};

use super::{
    compute_mission_success, compute_urban_foundation_metrics,
    internal::{
        advance_tick, all_dynamic_tasks_injected, all_failure_ticks_passed,
        all_partitions_resolved, apply_environment_effects, apply_partition_events,
        connectivity_metrics_tick, first_active_agent_id, process_alive_nodes,
        process_wildfire_mapping_tick, record_agent_failures, record_final_poses,
        record_inspection_edge_visits, record_safety_violations, record_sar_scans,
        record_tick_start, send_alive_heartbeats, should_stop_tick, tasks_injected_at_tick,
        teleport_assigned_tasks_when_movement_disabled, update_connectivity_snapshot,
        MissionStopSnapshot,
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
        let mut agents_exhausted: u64 = 0;
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

            // Track view divergence and convergence
            let maps: Vec<HashMap<TaskId, AgentId>> = nodes
                .iter()
                .filter(|(_, id)| !crashed_agents.contains(id))
                .map(|(node, _)| {
                    node.coordinator
                        .registry
                        .tasks()
                        .filter_map(|t| t.assigned_to.clone().map(|a| (t.id.clone(), a)))
                        .collect::<HashMap<_, _>>()
                })
                .collect();
            if !maps.is_empty() {
                let reference = &maps[0];
                let diverged = maps.iter().filter(|m| *m != reference).count() as u64;
                max_view_divergence = max_view_divergence.max(diverged);

                if let Some(heal_at) = heal_tick {
                    if current_tick > heal_at && diverged == 0 && convergence_ticks.is_none() {
                        convergence_ticks = Some(current_tick - heal_at);
                    }
                }
            }

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

        let network_availability = if availability_per_tick.is_empty() {
            1.0
        } else {
            availability_per_tick.iter().sum::<f64>() / availability_per_tick.len() as f64
        };
        let avg_hop_count = if total_hop_count_ticks > 0 {
            total_hop_count_sum / total_hop_count_ticks as f64
        } else {
            0.0
        };

        // v0.6: Compute new metrics from final state
        let (stale_state_age_ticks, final_battery_min, battery_margin_avg) =
            if let Some((node, _)) = nodes.iter().find(|(_, id)| !crashed_agents.contains(id)) {
                let mut max_stale_age: u64 = 0;
                let mut battery_sum: f64 = 0.0;
                let mut battery_count: u64 = 0;
                let mut battery_min = f64::MAX;
                let mut exhausted_count: u64 = 0;
                for (_agent_id, entry) in node.coordinator.membership.all_agents() {
                    let stale_age = total_ticks.saturating_sub(entry.last_heartbeat_tick);
                    max_stale_age = max_stale_age.max(stale_age);
                    battery_sum += entry.battery;
                    battery_count += 1;
                    battery_min = battery_min.min(entry.battery);
                    if entry.battery <= 0.0 {
                        exhausted_count += 1;
                    }
                }
                let battery_avg = if battery_count > 0 {
                    battery_sum / battery_count as f64
                } else {
                    0.0
                };
                let final_min = if battery_count > 0 { battery_min } else { 0.0 };
                agents_exhausted = exhausted_count;
                (max_stale_age, final_min, battery_avg)
            } else {
                (0, 0.0, 0.0)
            };

        let avg_distance_travelled = if !nodes.is_empty() {
            total_distance_travelled / nodes.len() as f64
        } else {
            0.0
        };

        // v0.6: coverage_progress as fraction of tasks with assigned agents
        let coverage_progress =
            if let Some((node, _)) = nodes.iter().find(|(_, id)| !crashed_agents.contains(id)) {
                let total_tasks = node.coordinator.registry.tasks().count() as f64;
                let assigned_tasks = node
                    .coordinator
                    .registry
                    .tasks()
                    .filter(|t| t.assigned_to.is_some())
                    .count() as f64;
                if total_tasks > 0.0 {
                    assigned_tasks / total_tasks
                } else {
                    1.0
                }
            } else {
                0.0
            };

        record_final_poses(&nodes, total_ticks, &mut log_builder);

        // v0.16: Compute inspection metrics
        let (edge_coverage_rate, missed_edges, route_efficiency) =
            if let Some(ref inspection_state) = inspection_state {
                let total_edges = inspection_state.graph.edges.len() as u64;
                let covered = inspection_state.covered.len() as u64;
                let missed = total_edges.saturating_sub(covered);
                let coverage_rate = if total_edges > 0 {
                    covered as f64 / total_edges as f64
                } else {
                    0.0
                };
                let sum_covered_lengths: f64 = inspection_state
                    .graph
                    .edges
                    .iter()
                    .filter(|e| inspection_state.covered.contains(&e.id))
                    .map(|e| e.length_m)
                    .sum();
                let efficiency = if total_distance_travelled > 0.0 {
                    sum_covered_lengths / total_distance_travelled
                } else {
                    0.0
                };
                (coverage_rate, missed, efficiency)
            } else {
                (0.0, 0, 0.0)
            };

        let event_log = log_builder.map(|b| b.build());

        let bundle_travel_distance: f64 = nodes
            .iter()
            .filter_map(|(n, _)| n.cbba.as_ref().map(|c| c.bundle_travel_distance))
            .sum();

        // v0.34: Compute meaningful planner metrics from final agent state.
        let (avg_wasted_travel, avg_return_reserve, infeasible_routes) =
            if let Some((node, _)) = nodes.iter().find(|(_, id)| !crashed_agents.contains(id)) {
                let mut wasted_travel_sum = 0.0;
                let mut return_reserve_sum = 0.0;
                let mut return_reserve_count = 0u64;
                let mut infeasible_count = 0u64;
                let battery_planner = BatteryAwarePlanner::default();
                let nn_planner = NearestNeighbourPlanner;
                let task_list: Vec<Task> = node.coordinator.registry.tasks().cloned().collect();

                for (agent_id, entry) in node.coordinator.membership.all_agents() {
                    if crashed_agents.contains(agent_id) {
                        continue;
                    }
                    let assigned_tasks: Vec<Task> = task_list
                        .iter()
                        .filter(|t| t.assigned_to.as_ref() == Some(agent_id))
                        .cloned()
                        .collect();

                    // Wasted travel: compare CBBA bundle distance to NN optimal for same tasks.
                    if let Some(ref cbba) = node.cbba {
                        if let Some(bundle) = cbba.bundles.get(agent_id) {
                            let bundle_tasks: Vec<&Task> = bundle
                                .iter()
                                .filter_map(|tid| task_list.iter().find(|t| t.id == *tid))
                                .collect();
                            let actual_cost = route_cost(entry.pose, &bundle_tasks);
                            let nn_ordered = nn_planner.order(
                                entry.pose,
                                &assigned_tasks,
                                &swarm_types::Agent {
                                    id: agent_id.clone(),
                                    role: entry.role.clone(),
                                    health: swarm_types::Health::Alive,
                                    pose: entry.pose,
                                    capabilities: entry.capabilities.clone(),
                                    current_task: None,
                                    battery: entry.battery,
                                    comms_range: entry.comms_range,
                                    generation: entry.generation,
                                    speed: entry.speed,
                                    max_range: entry.max_range,
                                    battery_drain_rate: entry.battery_drain_rate,
                                    battery_model: entry.battery_model.clone(),
                                },
                            );
                            let nn_tasks: Vec<&Task> = nn_ordered
                                .iter()
                                .filter_map(|tid| task_list.iter().find(|t| t.id == *tid))
                                .collect();
                            let nn_cost = route_cost(entry.pose, &nn_tasks);
                            if actual_cost > nn_cost {
                                wasted_travel_sum += actual_cost - nn_cost;
                            }
                        }
                    }

                    // Return reserve: battery minus battery needed to return to base.
                    let return_dist = entry.pose.distance_to(&base_pose);
                    let return_drain = if let Some(ref model) = entry.battery_model {
                        let horizontal = entry.pose.distance_to_2d(&base_pose);
                        let vertical = (entry.pose.z - base_pose.z).abs();
                        horizontal * model.cruise_drain_per_meter
                            + vertical * model.climb_drain_per_meter
                    } else {
                        return_dist * entry.battery_drain_rate
                    };
                    let reserve = entry.battery - return_drain;
                    return_reserve_sum += reserve.max(0.0);
                    return_reserve_count += 1;

                    // Infeasible routes: check if assigned tasks are feasible.
                    if !assigned_tasks.is_empty() {
                        let agent_full = swarm_types::Agent {
                            id: agent_id.clone(),
                            role: entry.role.clone(),
                            health: swarm_types::Health::Alive,
                            pose: entry.pose,
                            capabilities: entry.capabilities.clone(),
                            current_task: None,
                            battery: entry.battery,
                            comms_range: entry.comms_range,
                            generation: entry.generation,
                            speed: entry.speed,
                            max_range: entry.max_range,
                            battery_drain_rate: entry.battery_drain_rate,
                            battery_model: entry.battery_model.clone(),
                        };
                        if !battery_planner.is_feasible(entry.pose, &assigned_tasks, &agent_full) {
                            infeasible_count += 1;
                        }
                    }
                }

                let avg_wasted = wasted_travel_sum;
                let avg_reserve = if return_reserve_count > 0 {
                    return_reserve_sum / return_reserve_count as f64
                } else {
                    0.0
                };
                (avg_wasted, avg_reserve, infeasible_count)
            } else {
                (0.0, 0.0, 0)
            };

        (
            RunMetrics {
                seed: scenario.seed,
                total_ticks,
                messages_attempted: msgs_attempted,
                messages_dropped: msgs_dropped,
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
                network_availability,
                relay_reallocation_ticks,
                avg_hop_count,
                disconnected_agents_max,
                coverage_progress,
                bytes_sent,
                stale_state_age_ticks,
                battery_margin_min: final_battery_min,
                battery_margin_avg,
                // v0.8
                final_battery_min,
                avg_distance_travelled,
                agents_exhausted,
                total_distance_travelled,
                mission_completion_ticks: total_ticks,
                time_to_first_exhaustion,
                // v0.9 SAR
                time_to_find: grid_state.as_ref().and_then(|g| g.first_find_tick),
                coverage_over_time,
                probability_of_detection: grid_state.as_ref().map_or(0.0, |g| {
                    if g.targets.is_empty() {
                        0.0
                    } else {
                        g.targets_found as f64 / g.targets.len() as f64
                    }
                }),
                targets_found: grid_state.as_ref().map_or(0, |g| g.targets_found),
                targets_total: grid_state.as_ref().map_or(0, |g| g.targets.len() as u32),
                scan_count: grid_state.as_ref().map_or(0, |g| g.scan_count),
                // v0.10 CBBA
                cbba_rounds_to_convergence: nodes
                    .iter()
                    .filter_map(|(n, _)| n.cbba.as_ref().map(|c| c.current_round as u64))
                    .max()
                    .unwrap_or(0),
                cbba_converged: nodes
                    .iter()
                    .all(|(n, _)| n.cbba.as_ref().is_none_or(|c| c.converged)),
                cbba_messages: nodes
                    .iter()
                    .filter_map(|(n, _)| n.cbba.as_ref().map(|c| c.messages_exchanged))
                    .sum(),
                // v0.15 CBBA bundle travel
                bundle_travel_distance,
                // v0.15 CBBA convergence tick
                cbba_convergence_tick,
                // v0.13 Safety
                safety_violations,
                // v0.14 SAR v2 belief metrics
                belief_entropy_final: grid_state
                    .as_ref()
                    .and_then(|g| g.belief_map.as_ref().map(|bm| bm.mean_entropy()))
                    .unwrap_or(0.0),
                false_positives: grid_state
                    .as_ref()
                    .and_then(|g| g.belief_map.as_ref().map(|bm| bm.false_positives))
                    .unwrap_or(0),
                confirmation_scans: grid_state
                    .as_ref()
                    .and_then(|g| g.belief_map.as_ref().map(|bm| bm.confirmation_scans))
                    .unwrap_or(0),
                // v0.16 Inspection metrics
                edge_coverage_rate,
                missed_edges,
                revisit_count,
                route_efficiency,
                // v0.28 Planner Quality metrics
                avg_route_length: bundle_travel_distance,
                avg_wasted_travel,
                avg_return_reserve,
                infeasible_routes,
                // v0.30 Wildfire Mapping metrics
                hazard_zones_mapped: wildfire_state
                    .as_ref()
                    .map_or(0, |w| w.mapped_zone_ids.len() as u64),
                priority_updates,
                final_avg_threat_level: wildfire_state.as_ref().map_or(0.0, |w| {
                    if w.zones.is_empty() {
                        0.0
                    } else {
                        w.zones.iter().map(|z| z.threat_level).sum::<f64>() / w.zones.len() as f64
                    }
                }),
                // v0.38 Wildfire v2
                high_priority_zones_mapped,
                time_to_map_first_high_risk,
                threat_level_over_time,
                zone_observations,
                // v0.35 Dynamic Mission Correctness
                unsupported_reason,
                // v0.37 Realism Scenario Pack
                realism_profile: config.realism_profile.clone(),
                wind: config.wind,
                // v0.64 Urban Foundations
                urban_route_length_m,
                urban_route_risk_score,
                urban_route_planned,
                urban_violation_count,
                urban_route_completed,
                urban_patrol_completed: urban_route_completed,
                urban_time_to_complete_loop: urban_route_completed.then_some(total_ticks),
                urban_distance_travelled_m: 0.0,
                urban_route_efficiency: 0.0,
                urban_replan_count: 0,
                bus_detected: false,
                time_to_detect_bus: None,
                false_positive_count: 0,
                distance_before_detection: 0.0,
                search_success_without_violation: false,
                urban_min_agent_separation_m: None,
                urban_separation_violation_count: 0,
                urban_route_conflict_count: 0,
            },
            event_log,
        )
    }
}
