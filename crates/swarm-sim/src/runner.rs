use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use rand::SeedableRng;
use swarm_alloc::Allocator;
use swarm_comms::{
    ConnectivityModel, ConnectivitySnapshot, InMemAgentTransport, InMemNetwork, NetworkConfig,
};
use swarm_metrics::RunMetrics;
use swarm_runtime::{AgentNode, Coordinator, GridState, NodeTickOutput};
use swarm_types::{AgentId, Health, Role, Task, TaskId};

use crate::{Clock, Scenario};

#[derive(Clone, Debug)]
pub struct FailureEvent {
    pub agent_id: AgentId,
    pub at_tick: u64,
}

#[derive(Clone, Debug)]
pub struct DynamicTaskEvent {
    pub at_tick: u64,
    pub task: Task,
}

#[derive(Clone, Debug)]
pub struct PartitionEvent {
    pub at_tick: u64,
    pub until_tick: Option<u64>,
    pub agents: (AgentId, AgentId),
}

#[derive(Clone, Debug)]
pub struct RunConfig {
    pub max_ticks: u64,
    pub timeout_ticks: u64,
    pub max_unassigned_ticks: u64,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub latency_per_hop: u64,
    pub failures: Vec<FailureEvent>,
    pub dynamic_tasks: Vec<DynamicTaskEvent>,
    pub partition_events: Vec<PartitionEvent>,
    pub gossip_interval_ticks: u64,
    // v0.5: base station identifier for connectivity metrics
    pub base_id: Option<AgentId>,
    // v0.8: movement config
    pub enable_movement: bool,
    pub tick_duration_ms: u64,
    // v0.9: SAR grid state
    pub grid_state: Option<GridState>,
    // v0.10 Phase 1
    pub enable_cbba: bool,
}

pub struct ScenarioRunner;

impl ScenarioRunner {
    pub fn run(scenario: &Scenario, config: RunConfig) -> RunMetrics {
        use swarm_alloc::GreedyAllocator;
        Self::run_with(scenario, config, GreedyAllocator)
    }

    pub fn run_with<A: Allocator>(
        scenario: &Scenario,
        config: RunConfig,
        allocator: A,
    ) -> RunMetrics {
        Self::run_internal(scenario, config, allocator, None).0
    }

    /// Run a scenario with optional event logging.
    ///
    /// Returns `(RunMetrics, Option<EventLog>)`. The `EventLog` is `Some` when
    /// `enable_log` is `true`. Existing callers of `run_with` are unaffected.
    pub fn run_with_log<A: Allocator>(
        scenario: &Scenario,
        config: RunConfig,
        allocator: A,
    ) -> (RunMetrics, Option<swarm_replay::EventLog>) {
        let run_id = format!("{}_{}", scenario.name, scenario.seed);
        let builder = swarm_replay::EventLogBuilder::new(run_id, scenario.seed, &scenario.name);
        Self::run_internal(scenario, config, allocator, Some(builder))
    }

    fn run_internal<A: Allocator>(
        scenario: &Scenario,
        config: RunConfig,
        mut allocator: A,
        mut log_builder: Option<swarm_replay::EventLogBuilder>,
    ) -> (RunMetrics, Option<swarm_replay::EventLog>) {
        let bus = Rc::new(RefCell::new(InMemNetwork::new(NetworkConfig {
            packet_loss_rate: config.packet_loss_rate,
            latency_ticks: config.latency_ticks,
            latency_per_hop: config.latency_per_hop,
            seed: scenario.seed,
            partitions: HashSet::new(),
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
                    node.cbba = Some(swarm_alloc::CbbaAllocator::default());
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

        // v0.8 movement metrics
        let mut total_distance_travelled: f64 = 0.0;
        let mut agents_exhausted: u64 = 0;
        let mut time_to_first_exhaustion: Option<u64> = None;

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
        let base_pose = scenario
            .base_station
            .unwrap_or(swarm_types::Pose { x: 0.0, y: 0.0 });

        for _ in 0..config.max_ticks {
            clock.advance();
            let current_tick = u64::from(clock.now());
            total_ticks = current_tick;

            if let Some(ref mut builder) = log_builder {
                builder.push(swarm_replay::Event::TickStart { tick: current_tick });
            }

            for failure in config
                .failures
                .iter()
                .filter(|failure| failure.at_tick == current_tick)
            {
                crashed_agents.insert(failure.agent_id.clone());
                if let Some(ref mut builder) = log_builder {
                    builder.push(swarm_replay::Event::AgentFailed {
                        agent_id: failure.agent_id.clone(),
                        tick: current_tick,
                    });
                }
            }

            bus.borrow_mut().advance_tick();

            // Apply partition events
            for pe in &config.partition_events {
                if pe.at_tick == current_tick {
                    bus.borrow_mut()
                        .add_partition(pe.agents.0.clone(), pe.agents.1.clone());
                    partition_events += 1;
                    partitions_active = true;
                    if let Some(ref mut builder) = log_builder {
                        builder.push(swarm_replay::Event::PartitionAdded {
                            agent_a: pe.agents.0.clone(),
                            agent_b: pe.agents.1.clone(),
                            tick: current_tick,
                        });
                    }
                }
                if pe.until_tick == Some(current_tick) {
                    bus.borrow_mut()
                        .remove_partition(pe.agents.0.clone(), pe.agents.1.clone());
                    heal_tick = Some(current_tick);
                    if let Some(ref mut builder) = log_builder {
                        builder.push(swarm_replay::Event::PartitionRemoved {
                            agent_a: pe.agents.0.clone(),
                            agent_b: pe.agents.1.clone(),
                            tick: current_tick,
                        });
                    }
                }
            }

            let injected: Vec<Task> = config
                .dynamic_tasks
                .iter()
                .filter(|ev| ev.at_tick == current_tick)
                .map(|ev| ev.task.clone())
                .collect();
            tasks_injected += injected.len() as u64;

            // v0.5: Update connectivity snapshot on the network bus before heartbeats/gossip.
            // Include all non-crashed agents (not just alive) so that partition-induced
            // false failure detection does not permanently break mesh reachability after heal.
            {
                let first_alive = nodes.iter().find(|(_, id)| !crashed_agents.contains(id));
                if let Some((node, _)) = first_alive {
                    let agent_entries: Vec<(AgentId, swarm_types::Pose, f64, Health)> = node
                        .coordinator
                        .membership
                        .all_agents()
                        .filter(|(id, _)| !crashed_agents.contains(id))
                        .map(|(id, entry)| {
                            (id.clone(), entry.pose, entry.comms_range, Health::Alive)
                        })
                        .collect();
                    let snapshot = ConnectivitySnapshot {
                        agent_entries,
                        ground_nodes: scenario
                            .ground_nodes
                            .iter()
                            .map(|gn| (gn.id.clone(), gn.pose, gn.comms_range))
                            .collect(),
                        base_id: base_id.to_string(),
                        base_pose,
                    };
                    bus.borrow_mut().set_connectivity_snapshot(snapshot);
                }
            }

            // Phase 1: All alive agents send heartbeats (uses AgentNode method)
            for (node, agent_id) in &mut nodes {
                if crashed_agents.contains(agent_id) {
                    continue;
                }
                let _ = node.send_heartbeats(current_tick);
            }

            // Phase 2: All alive agents poll and process (uses AgentNode method)
            let mut tick_outputs: Vec<(AgentId, NodeTickOutput)> = Vec::new();
            for (node, agent_id) in &mut nodes {
                if crashed_agents.contains(agent_id) {
                    continue;
                }

                let output = match node.process_inbox_and_allocate(
                    current_tick,
                    &mut allocator,
                    injected.clone(),
                ) {
                    Ok(out) => out,
                    Err(_) => continue,
                };
                tick_outputs.push((agent_id.clone(), output));
            }

            // v0.5: Pose update — only teleport when movement is disabled.
            // When enable_movement=true, agents move gradually via apply_movement.
            if !config.enable_movement {
                for (node, agent_id) in &mut nodes {
                    if crashed_agents.contains(agent_id) {
                        continue;
                    }
                    let assigned_tasks: Vec<(AgentId, Option<swarm_types::Pose>)> = node
                        .coordinator
                        .registry
                        .tasks()
                        .filter(|t| t.assigned_to.as_ref() == Some(agent_id))
                        .map(|t| (agent_id.clone(), t.pose))
                        .collect();
                    for (_aid, pose) in assigned_tasks {
                        if let Some(p) = pose {
                            node.coordinator.membership.update_pose(agent_id, p);
                        }
                    }
                }
            }

            // v0.9: SAR scan logic
            if let Some(ref mut grid_state) = grid_state {
                for (node, agent_id) in &mut nodes {
                    if crashed_agents.contains(agent_id) {
                        continue;
                    }
                    let assigned_tasks: Vec<_> = node
                        .coordinator
                        .registry
                        .tasks()
                        .filter(|t| t.assigned_to.as_ref() == Some(agent_id))
                        .map(|t| (t.id.clone(), t.grid_cell))
                        .collect();
                    let mut scanned_task_ids = Vec::new();
                    for (task_id, grid_cell) in assigned_tasks {
                        if let Some((cell_x, cell_y)) = grid_cell {
                            if let Some(entry) = node.coordinator.membership.get(agent_id) {
                                let cell_pose = grid_state.grid.cell_center(cell_x, cell_y);
                                let dist = entry.pose.distance_to(&cell_pose);
                                let threshold = grid_state.grid.cell_size * 0.1;
                                if dist < threshold {
                                    let mut rng = rand::rngs::StdRng::seed_from_u64(
                                        scenario.seed.wrapping_add(current_tick),
                                    );
                                    grid_state.scan_cell(
                                        agent_id.clone(),
                                        cell_x,
                                        cell_y,
                                        &entry.role,
                                        current_tick,
                                        &mut rng,
                                    );
                                    scanned_task_ids.push(task_id);
                                }
                            }
                        }
                    }
                    // Release scanned tasks so agents can be reassigned to new cells
                    for task_id in scanned_task_ids {
                        node.coordinator.registry.release_task(&task_id);
                    }
                }
                coverage_over_time.push(grid_state.coverage_fraction());
            }

            // v0.5: Compute connectivity metrics for this tick
            {
                let first_alive = nodes.iter().find(|(_, id)| !crashed_agents.contains(id));
                if let Some((node, _)) = first_alive {
                    let agent_entries: Vec<(AgentId, swarm_types::Pose, f64, Health)> = node
                        .coordinator
                        .membership
                        .all_agents()
                        .filter(|(id, _)| !crashed_agents.contains(id))
                        .map(|(id, entry)| {
                            (id.clone(), entry.pose, entry.comms_range, Health::Alive)
                        })
                        .collect();
                    let snapshot = ConnectivitySnapshot {
                        agent_entries,
                        ground_nodes: scenario
                            .ground_nodes
                            .iter()
                            .map(|gn| (gn.id.clone(), gn.pose, gn.comms_range))
                            .collect(),
                        base_id: base_id.to_string(),
                        base_pose,
                    };
                    let reachability = ConnectivityModel::reachability_from_base(&snapshot);
                    let alive_agent_ids: Vec<AgentId> = node
                        .coordinator
                        .membership
                        .alive_agents()
                        .map(|(id, _)| id.clone())
                        .collect();
                    let availability =
                        ConnectivityModel::availability_fraction(&reachability, &alive_agent_ids);
                    availability_per_tick.push(availability);

                    let disconnected_count = alive_agent_ids.len()
                        - alive_agent_ids
                            .iter()
                            .filter(|id| reachability.contains_key(id.as_ref()))
                            .count();
                    disconnected_agents_max =
                        disconnected_agents_max.max(disconnected_count as u64);

                    let hop_sum: usize = alive_agent_ids
                        .iter()
                        .filter_map(|id| reachability.get(id.as_ref()))
                        .sum();
                    let reachable_count = alive_agent_ids
                        .iter()
                        .filter(|id| reachability.contains_key(id.as_ref()))
                        .count();
                    if reachable_count > 0 {
                        total_hop_count_sum += hop_sum as f64 / reachable_count as f64;
                        total_hop_count_ticks += 1;
                    }
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
            let first_id = nodes
                .iter()
                .find(|(_, id)| !crashed_agents.contains(id))
                .map(|(_, id)| id.clone());

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
            let all_failure_ticks_passed = config
                .failures
                .iter()
                .all(|failure| current_tick >= failure.at_tick);
            let all_dynamic_tasks_injected = config
                .dynamic_tasks
                .iter()
                .all(|ev| current_tick >= ev.at_tick);
            let all_partitions_resolved = config
                .partition_events
                .iter()
                .all(|pe| pe.until_tick.is_some_and(|u| current_tick >= u));
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

            let sar_complete = grid_state.as_ref().is_none_or(|g| g.all_targets_found());

            if all_tasks_assigned
                && max_task_unassigned_ticks <= config.max_unassigned_ticks
                && all_failure_ticks_passed
                && all_expected_failures_detected
                && all_dynamic_tasks_injected
                && post_partition_converged
                && sar_complete
            {
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
        let success = all_tasks_assigned
            && all_expected_failures_detected
            && max_task_unassigned_ticks <= config.max_unassigned_ticks;

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

        // Log final poses if event logging is enabled
        if let Some(ref mut builder) = log_builder {
            for (node, agent_id) in &nodes {
                if let Some(entry) = node.coordinator.membership.get(agent_id) {
                    builder.push(swarm_replay::Event::PoseUpdated {
                        agent_id: agent_id.clone(),
                        pose: entry.pose,
                        tick: total_ticks,
                    });
                }
            }
        }

        let event_log = log_builder.map(|b| b.build());

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
            },
            event_log,
        )
    }
}

fn update_unassigned_durations(
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

fn released_tasks_reassigned(coordinator: &Coordinator, released_tasks: &[TaskId]) -> bool {
    released_tasks.iter().all(|released_task| {
        coordinator
            .registry
            .tasks()
            .any(|task| &task.id == released_task && task.assigned_to.is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_alloc::{AllocationAgent, AllocationTask, Allocator};
    use swarm_types::{Agent, Capability, Health, Pose, Role, Task, TaskStatus};

    fn scenario(seed: u64, agent_count: usize, task_count: usize) -> Scenario {
        let agents = (0..agent_count)
            .map(|index| Agent {
                id: AgentId::from(format!("agent-{index}")),
                role: Role::Scout,
                health: Health::Alive,
                pose: Pose { x: 0.0, y: 0.0 },
                capabilities: Vec::new(),
                current_task: None,
                battery: 100.0,
                comms_range: f64::INFINITY,
                generation: 1,
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
            })
            .collect();
        let tasks = (0..task_count)
            .map(|index| Task {
                id: TaskId::from(format!("task-{index}")),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: 1,
                required_capabilities: vec![],
                required_role: None,
                preferred_role: None,
                expires_at: None,
                pose: None,
                grid_cell: None,
            })
            .collect();
        Scenario {
            name: "test".to_owned(),
            seed,
            agents,
            tasks,
            ground_nodes: vec![],
            base_station: None,
        }
    }

    fn config(failures: Vec<FailureEvent>) -> RunConfig {
        RunConfig {
            max_ticks: 50,
            timeout_ticks: 3,
            max_unassigned_ticks: 5,
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            latency_per_hop: 0,
            failures,
            dynamic_tasks: vec![],
            partition_events: vec![],
            gossip_interval_ticks: 999,
            base_id: None,
            enable_movement: false,
            tick_duration_ms: 100,
            grid_state: None,
            enable_cbba: false,
        }
    }

    #[test]
    fn runner_timeout_semantics_before_after_detection() {
        let scenario = scenario(0, 5, 8);
        let metrics = ScenarioRunner::run(
            &scenario,
            config(vec![FailureEvent {
                agent_id: AgentId::from("agent-0".to_owned()),
                at_tick: 2,
            }]),
        );

        assert!(metrics.success);
        assert_eq!(metrics.detection_time_ticks, Some(3));
        assert_eq!(metrics.reallocation_time_ticks, Some(0));
    }

    #[test]
    fn runner_failure_triggers_reallocation() {
        let scenario = scenario(1, 5, 8);
        let metrics = ScenarioRunner::run(
            &scenario,
            config(vec![FailureEvent {
                agent_id: AgentId::from("agent-0".to_owned()),
                at_tick: 2,
            }]),
        );

        assert!(metrics.success);
        assert!(metrics.all_tasks_assigned);
        assert!(metrics.max_task_unassigned_ticks <= 5);
    }

    #[test]
    fn runner_deterministic_same_seed() {
        let scenario = scenario(7, 5, 8);
        let config = config(vec![FailureEvent {
            agent_id: AgentId::from("agent-0".to_owned()),
            at_tick: 2,
        }]);

        let a = ScenarioRunner::run(&scenario, config.clone());
        let b = ScenarioRunner::run(&scenario, config);

        assert_eq!(a, b);
    }

    #[test]
    fn runner_no_failure_assigns_all_tasks() {
        let scenario = scenario(0, 5, 8);
        let metrics = ScenarioRunner::run(&scenario, config(Vec::new()));

        assert!(metrics.success);
        assert!(metrics.all_tasks_assigned);
    }

    #[test]
    fn runner_dynamic_task_appears_and_gets_assigned() {
        let s = scenario(0, 3, 0);
        let dynamic_task = Task {
            id: TaskId::from("dyn-0".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
            grid_cell: None,
        };
        let cfg = RunConfig {
            dynamic_tasks: vec![DynamicTaskEvent {
                at_tick: 2,
                task: dynamic_task,
            }],
            ..config(vec![])
        };
        let metrics = ScenarioRunner::run(&s, cfg);
        assert!(metrics.all_tasks_assigned);
        assert_eq!(metrics.tasks_injected, 1);
    }

    #[test]
    fn runner_expired_task_counted_in_metrics() {
        let s = scenario(0, 3, 0);
        let expiring_task = Task {
            id: TaskId::from("exp-0".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![Capability::from("missing".to_owned())],
            required_role: None,
            preferred_role: None,
            expires_at: Some(3),
            pose: None,
            grid_cell: None,
        };
        let cfg = RunConfig {
            dynamic_tasks: vec![DynamicTaskEvent {
                at_tick: 1,
                task: expiring_task,
            }],
            ..config(vec![])
        };
        let metrics = ScenarioRunner::run(&s, cfg);
        assert_eq!(metrics.tasks_expired, 1);
    }

    #[test]
    fn runner_greedy_deterministic_with_capabilities() {
        let mut s = scenario(5, 4, 2);
        s.agents[0].capabilities = vec![Capability::from("optical".to_owned())];
        s.tasks[0].required_capabilities = vec![Capability::from("optical".to_owned())];

        let cfg = config(vec![]);
        let a = ScenarioRunner::run(&s, cfg.clone());
        let b = ScenarioRunner::run(&s, cfg);

        assert_eq!(a, b);
    }

    #[test]
    fn runner_auction_deterministic() {
        use swarm_alloc::AuctionAllocator;
        let s = scenario(9, 5, 4);
        let cfg = config(vec![]);

        let a = ScenarioRunner::run_with(&s, cfg.clone(), AuctionAllocator::default());
        let b = ScenarioRunner::run_with(&s, cfg, AuctionAllocator::default());

        assert_eq!(a, b);
    }

    #[test]
    fn runner_capability_gate_task_stays_unassigned() {
        let s = scenario(0, 3, 0);
        let impossible_task = Task {
            id: TaskId::from("imp-0".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![Capability::from("unobtainium".to_owned())],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
            grid_cell: None,
        };
        let cfg = RunConfig {
            dynamic_tasks: vec![DynamicTaskEvent {
                at_tick: 1,
                task: impossible_task,
            }],
            ..config(vec![])
        };
        let metrics = ScenarioRunner::run(&s, cfg);
        assert!(!metrics.all_tasks_assigned);
    }

    #[test]
    fn runner_no_duplicate_ownership_invariant() {
        let s = scenario(0, 5, 5);
        let cfg = config(vec![]);
        ScenarioRunner::run(&s, cfg);
    }

    struct DuplicateAllocator;

    impl Allocator for DuplicateAllocator {
        fn allocate(
            &mut self,
            tasks: &[AllocationTask<'_>],
            agents: &[AllocationAgent],
        ) -> Vec<(TaskId, AgentId)> {
            if tasks.is_empty() || agents.is_empty() {
                return vec![];
            }
            let task_id = tasks[0].task.id.clone();
            let agent_id = agents[0].id.clone();
            vec![(task_id.clone(), agent_id.clone()), (task_id, agent_id)]
        }
    }

    #[test]
    fn runner_conflict_counter_in_metrics() {
        let s = scenario(0, 2, 1);
        let cfg = config(vec![]);
        let metrics = ScenarioRunner::run_with(&s, cfg, DuplicateAllocator);
        assert!(metrics.conflicting_assignments > 0);
    }

    #[test]
    fn allocate_unassigned_counts_duplicate_allocator_output() {
        let s = scenario(0, 2, 1);
        let cfg = config(vec![]);
        let metrics = ScenarioRunner::run_with(&s, cfg, DuplicateAllocator);
        assert!(metrics.conflicting_assignments > 0);
    }

    #[test]
    fn cbba_distributed_path_succeeds() {
        use swarm_alloc::CbbaAllocator;
        let s = scenario(0, 3, 2);
        let mut cfg = config(vec![]);
        cfg.enable_cbba = true;
        cfg.gossip_interval_ticks = 1;
        cfg.max_ticks = 30;
        let metrics = ScenarioRunner::run_with(&s, cfg, CbbaAllocator::default());
        // Distributed CBBA should converge and complete the mission
        assert!(metrics.success, "CBBA did not complete the mission");
    }
}
