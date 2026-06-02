use std::collections::HashSet;

use swarm_alloc::{route_cost, BatteryAwarePlanner, NearestNeighbourPlanner, RoutePlanner};
use swarm_comms::InMemAgentTransport;
use swarm_metrics::RunMetrics;
use swarm_runtime::{AgentNode, GridState};
use swarm_types::{AgentId, Pose, Task};

use super::super::{InspectionState, WildfireState};
use super::events::record_final_poses;

/// All accumulated simulation state required to assemble the final RunMetrics.
///
/// Constructed at the end of `run_internal` after the tick loop completes,
/// containing every scalar/vec accumulator and the owned simulation state
/// needed for the metrics computation phase.
pub(in crate::runner) struct MetricsInput {
    /// Values extracted from the network bus before it is dropped.
    pub msgs_attempted: u64,
    pub msgs_dropped: u64,
    pub bytes_sent: u64,
    /// Owned simulation state consumed by metrics computation.
    pub nodes: Vec<(AgentNode<InMemAgentTransport>, AgentId)>,
    pub crashed_agents: HashSet<AgentId>,
    pub grid_state: Option<GridState>,
    pub inspection_state: Option<InspectionState>,
    pub wildfire_state: Option<WildfireState>,
    /// Scenario seed forwarded directly into RunMetrics.
    pub seed: u64,
    pub total_ticks: u64,
    pub total_distance_travelled: f64,
    pub detection_time_ticks: Option<u64>,
    pub reallocation_time_ticks: Option<u64>,
    pub max_task_unassigned_ticks: u64,
    pub all_tasks_assigned: bool,
    pub success: bool,
    pub tasks_injected: u64,
    pub tasks_expired: u64,
    pub conflicting_assignments: u64,
    pub partition_events: u64,
    pub partitions_active: bool,
    pub stale_messages_discarded: u64,
    pub convergence_ticks: Option<u64>,
    pub max_view_divergence: u64,
    pub relay_reallocation_ticks: Option<u64>,
    pub disconnected_agents_max: u64,
    pub cbba_convergence_tick: Option<u64>,
    pub safety_violations: u64,
    pub revisit_count: u64,
    pub priority_updates: u64,
    pub high_priority_zones_mapped: u64,
    pub time_to_map_first_high_risk: Option<u64>,
    pub zone_observations: u64,
    pub time_to_first_exhaustion: Option<u64>,
    /// value: `Vec<f64>` coverage fraction per tick
    pub coverage_over_time: Vec<f64>,
    pub threat_level_over_time: Vec<f64>,
    /// value: `Vec<f64>` network availability fraction per tick
    pub availability_per_tick: Vec<f64>,
    pub total_hop_count_sum: f64,
    pub total_hop_count_ticks: u64,
    pub base_pose: Pose,
    /// Forwarded from RunConfig.
    pub realism_profile: Option<String>,
    pub wind: Option<(f64, f64, f64)>,
    /// Urban foundation metrics computed before the tick loop.
    pub urban_route_planned: bool,
    pub urban_route_length_m: f64,
    pub urban_route_risk_score: f64,
    pub urban_violation_count: u64,
    pub urban_route_completed: bool,
    /// Computed after the loop by compute_mission_success.
    pub unsupported_reason: Option<String>,
    /// Event log builder consumed here to produce the final event log.
    pub log_builder: Option<swarm_replay::EventLogBuilder>,
}

/// Assemble the final RunMetrics and EventLog from accumulated loop state.
///
/// Extracted from the post-loop section of `ScenarioRunner::run_internal`.
/// Contains all pure computation: stale-state aggregation, planner quality
/// metrics, CBBA convergence metrics, and the RunMetrics construction.
pub(in crate::runner) fn assemble_final_metrics(
    input: MetricsInput,
) -> (RunMetrics, Option<swarm_replay::EventLog>) {
    let network_availability = if input.availability_per_tick.is_empty() {
        1.0
    } else {
        input.availability_per_tick.iter().sum::<f64>() / input.availability_per_tick.len() as f64
    };
    let avg_hop_count = if input.total_hop_count_ticks > 0 {
        input.total_hop_count_sum / input.total_hop_count_ticks as f64
    } else {
        0.0
    };

    // v0.6: Compute new metrics from final state
    let mut agents_exhausted: u64 = 0;
    let (stale_state_age_ticks, final_battery_min, battery_margin_avg) = if let Some((node, _)) =
        input
            .nodes
            .iter()
            .find(|(_, id)| !input.crashed_agents.contains(id))
    {
        let mut max_stale_age: u64 = 0;
        let mut battery_sum: f64 = 0.0;
        let mut battery_count: u64 = 0;
        let mut battery_min = f64::MAX;
        let mut exhausted_count: u64 = 0;
        for (_agent_id, entry) in node.coordinator.membership.all_agents() {
            let stale_age = input.total_ticks.saturating_sub(entry.last_heartbeat_tick);
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

    let avg_distance_travelled = if !input.nodes.is_empty() {
        input.total_distance_travelled / input.nodes.len() as f64
    } else {
        0.0
    };

    // v0.6: coverage_progress as fraction of tasks with assigned agents
    let coverage_progress = if let Some((node, _)) = input
        .nodes
        .iter()
        .find(|(_, id)| !input.crashed_agents.contains(id))
    {
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

    let mut log_builder = input.log_builder;
    record_final_poses(&input.nodes, input.total_ticks, &mut log_builder);

    // v0.16: Compute inspection metrics
    let (edge_coverage_rate, missed_edges, route_efficiency) =
        if let Some(ref inspection_state) = input.inspection_state {
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
            let efficiency = if input.total_distance_travelled > 0.0 {
                sum_covered_lengths / input.total_distance_travelled
            } else {
                0.0
            };
            (coverage_rate, missed, efficiency)
        } else {
            (0.0, 0, 0.0)
        };

    let event_log = log_builder.map(|b| b.build());

    let bundle_travel_distance: f64 = input
        .nodes
        .iter()
        .filter_map(|(n, _)| n.cbba.as_ref().map(|c| c.bundle_travel_distance))
        .sum();

    // v0.34: Compute meaningful planner metrics from final agent state.
    let (avg_wasted_travel, avg_return_reserve, infeasible_routes) = if let Some((node, _)) = input
        .nodes
        .iter()
        .find(|(_, id)| !input.crashed_agents.contains(id))
    {
        let mut wasted_travel_sum = 0.0;
        let mut return_reserve_sum = 0.0;
        let mut return_reserve_count = 0u64;
        let mut infeasible_count = 0u64;
        let battery_planner = BatteryAwarePlanner::default();
        let nn_planner = NearestNeighbourPlanner;
        let task_list: Vec<Task> = node.coordinator.registry.tasks().cloned().collect();

        for (agent_id, entry) in node.coordinator.membership.all_agents() {
            if input.crashed_agents.contains(agent_id) {
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
            let return_dist = entry.pose.distance_to(&input.base_pose);
            let return_drain = if let Some(ref model) = entry.battery_model {
                let horizontal = entry.pose.distance_to_2d(&input.base_pose);
                let vertical = (entry.pose.z - input.base_pose.z).abs();
                horizontal * model.cruise_drain_per_meter + vertical * model.climb_drain_per_meter
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
            seed: input.seed,
            total_ticks: input.total_ticks,
            messages_attempted: input.msgs_attempted,
            messages_dropped: input.msgs_dropped,
            detection_time_ticks: input.detection_time_ticks,
            reallocation_time_ticks: input.reallocation_time_ticks,
            max_task_unassigned_ticks: input.max_task_unassigned_ticks,
            all_tasks_assigned: input.all_tasks_assigned,
            success: input.success,
            tasks_injected: input.tasks_injected,
            tasks_expired: input.tasks_expired,
            conflicting_assignments: input.conflicting_assignments,
            partition_events: input.partition_events,
            partitions_active: input.partitions_active,
            stale_messages_discarded: input.stale_messages_discarded,
            convergence_ticks: input.convergence_ticks,
            max_view_divergence: input.max_view_divergence,
            network_availability,
            relay_reallocation_ticks: input.relay_reallocation_ticks,
            avg_hop_count,
            disconnected_agents_max: input.disconnected_agents_max,
            coverage_progress,
            bytes_sent: input.bytes_sent,
            stale_state_age_ticks,
            battery_margin_min: final_battery_min,
            battery_margin_avg,
            // v0.8
            final_battery_min,
            avg_distance_travelled,
            agents_exhausted,
            total_distance_travelled: input.total_distance_travelled,
            mission_completion_ticks: input.total_ticks,
            time_to_first_exhaustion: input.time_to_first_exhaustion,
            // v0.9 SAR
            time_to_find: input.grid_state.as_ref().and_then(|g| g.first_find_tick),
            coverage_over_time: input.coverage_over_time,
            probability_of_detection: input.grid_state.as_ref().map_or(0.0, |g| {
                if g.targets.is_empty() {
                    0.0
                } else {
                    g.targets_found as f64 / g.targets.len() as f64
                }
            }),
            targets_found: input.grid_state.as_ref().map_or(0, |g| g.targets_found),
            targets_total: input
                .grid_state
                .as_ref()
                .map_or(0, |g| g.targets.len() as u32),
            scan_count: input.grid_state.as_ref().map_or(0, |g| g.scan_count),
            // v0.10 CBBA
            cbba_rounds_to_convergence: input
                .nodes
                .iter()
                .filter_map(|(n, _)| n.cbba.as_ref().map(|c| c.current_round as u64))
                .max()
                .unwrap_or(0),
            cbba_converged: input
                .nodes
                .iter()
                .all(|(n, _)| n.cbba.as_ref().is_none_or(|c| c.converged)),
            cbba_messages: input
                .nodes
                .iter()
                .filter_map(|(n, _)| n.cbba.as_ref().map(|c| c.messages_exchanged))
                .sum(),
            // v0.15 CBBA bundle travel
            bundle_travel_distance,
            // v0.15 CBBA convergence tick
            cbba_convergence_tick: input.cbba_convergence_tick,
            // v0.13 Safety
            safety_violations: input.safety_violations,
            // v0.14 SAR v2 belief metrics
            belief_entropy_final: input
                .grid_state
                .as_ref()
                .and_then(|g| g.belief_map.as_ref().map(|bm| bm.mean_entropy()))
                .unwrap_or(0.0),
            false_positives: input
                .grid_state
                .as_ref()
                .and_then(|g| g.belief_map.as_ref().map(|bm| bm.false_positives))
                .unwrap_or(0),
            confirmation_scans: input
                .grid_state
                .as_ref()
                .and_then(|g| g.belief_map.as_ref().map(|bm| bm.confirmation_scans))
                .unwrap_or(0),
            // v0.16 Inspection metrics
            edge_coverage_rate,
            missed_edges,
            revisit_count: input.revisit_count,
            route_efficiency,
            // v0.28 Planner Quality metrics
            avg_route_length: bundle_travel_distance,
            avg_wasted_travel,
            avg_return_reserve,
            infeasible_routes,
            // v0.30 Wildfire Mapping metrics
            hazard_zones_mapped: input
                .wildfire_state
                .as_ref()
                .map_or(0, |w| w.mapped_zone_ids.len() as u64),
            priority_updates: input.priority_updates,
            final_avg_threat_level: input.wildfire_state.as_ref().map_or(0.0, |w| {
                if w.zones.is_empty() {
                    0.0
                } else {
                    w.zones.iter().map(|z| z.threat_level).sum::<f64>() / w.zones.len() as f64
                }
            }),
            // v0.38 Wildfire v2
            high_priority_zones_mapped: input.high_priority_zones_mapped,
            time_to_map_first_high_risk: input.time_to_map_first_high_risk,
            threat_level_over_time: input.threat_level_over_time,
            zone_observations: input.zone_observations,
            // v0.35 Dynamic Mission Correctness
            unsupported_reason: input.unsupported_reason,
            // v0.37 Realism Scenario Pack
            realism_profile: input.realism_profile,
            wind: input.wind,
            // v0.64 Urban Foundations
            urban_route_length_m: input.urban_route_length_m,
            urban_route_risk_score: input.urban_route_risk_score,
            urban_route_planned: input.urban_route_planned,
            urban_violation_count: input.urban_violation_count,
            urban_route_completed: input.urban_route_completed,
            urban_patrol_completed: input.urban_route_completed,
            urban_time_to_complete_loop: input.urban_route_completed.then_some(input.total_ticks),
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
