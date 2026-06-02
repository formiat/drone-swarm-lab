use swarm_alloc::Allocator;
use swarm_metrics::RunMetrics;
use swarm_types::Task;

use super::{
    compute_mission_success, compute_urban_foundation_metrics,
    internal::{assemble_final_metrics, run_tick_loop, MetricsInput, TickLoopState},
    RunConfig, ScenarioRunner,
};
use crate::Scenario;

impl ScenarioRunner {
    pub(super) fn run_internal<A: Allocator>(
        scenario: &Scenario,
        config: RunConfig,
        allocator: A,
        log_builder: Option<swarm_replay::EventLogBuilder>,
    ) -> (RunMetrics, Option<swarm_replay::EventLog>) {
        if config.urban_search_state.is_some() {
            return Self::run_urban_search(scenario, config, log_builder);
        }
        if config.urban_state.is_some() {
            return Self::run_urban_patrol(scenario, config, log_builder);
        }

        let urban_state = config.urban_state.clone();
        let (
            urban_route_planned,
            urban_route_length_m,
            urban_route_risk_score,
            urban_violation_count,
        ) = compute_urban_foundation_metrics(&urban_state);
        let urban_route_completed = false;
        let mut state = TickLoopState::new(scenario, &config, allocator, log_builder);
        run_tick_loop(&mut state, scenario, &config);

        let all_expected_failures_detected = config
            .failures
            .iter()
            .all(|failure| state.detected_agents.contains(&failure.agent_id));
        let all_tasks_assigned = state
            .nodes
            .iter()
            .find(|(_, id)| !state.crashed_agents.contains(id))
            .is_some_and(|(node, _)| node.coordinator.registry.all_assigned_or_completed());

        // v0.35: Recompute adapter_complete after loop for final success determination.
        let final_live_tasks: Vec<Task> = state
            .nodes
            .iter()
            .find(|(_, id)| !state.crashed_agents.contains(id))
            .map(|(node, _)| node.coordinator.registry.tasks().cloned().collect())
            .unwrap_or_default();
        let final_run_state = Self::build_run_state(
            &state.grid_state,
            &state.inspection_state,
            &state.wildfire_state,
            &final_live_tasks,
        );
        let adapter_complete = Self::adapter_driven_complete(
            &final_live_tasks,
            &final_run_state,
            &state.adapter_registry,
        );

        // v0.35: Mission-specific success semantics
        let (success, unsupported_reason) = compute_mission_success(
            config.max_unassigned_ticks,
            &config.strategy_name,
            config.wildfire_success_threshold,
            config.inspection_coverage_threshold,
            all_tasks_assigned,
            all_expected_failures_detected,
            state.max_task_unassigned_ticks,
            &state.grid_state,
            &state.inspection_state,
            &state.wildfire_state,
            &urban_state,
            urban_route_planned,
            urban_violation_count,
            urban_route_completed,
            adapter_complete,
        );

        let msgs_attempted = state.bus.borrow().messages_attempted();
        let msgs_dropped = state.bus.borrow().messages_dropped();
        let bytes_sent = state.bus.borrow().bytes_sent();
        drop(state.bus);

        assemble_final_metrics(MetricsInput {
            msgs_attempted,
            msgs_dropped,
            bytes_sent,
            nodes: state.nodes,
            crashed_agents: state.crashed_agents,
            grid_state: state.grid_state,
            inspection_state: state.inspection_state,
            wildfire_state: state.wildfire_state,
            seed: scenario.seed,
            total_ticks: state.total_ticks,
            total_distance_travelled: state.total_distance_travelled,
            detection_time_ticks: state.detection_time_ticks,
            reallocation_time_ticks: state.reallocation_time_ticks,
            max_task_unassigned_ticks: state.max_task_unassigned_ticks,
            all_tasks_assigned,
            success,
            tasks_injected: state.tasks_injected,
            tasks_expired: state.tasks_expired,
            conflicting_assignments: state.conflicting_assignments,
            partition_events: state.partition_events,
            partitions_active: state.partitions_active,
            stale_messages_discarded: state.stale_messages_discarded,
            convergence_ticks: state.convergence_ticks,
            max_view_divergence: state.max_view_divergence,
            relay_reallocation_ticks: state.relay_reallocation_ticks,
            disconnected_agents_max: state.disconnected_agents_max,
            cbba_convergence_tick: state.cbba_convergence_tick,
            safety_violations: state.safety_violations,
            revisit_count: state.revisit_count,
            priority_updates: state.priority_updates,
            high_priority_zones_mapped: state.high_priority_zones_mapped,
            time_to_map_first_high_risk: state.time_to_map_first_high_risk,
            zone_observations: state.zone_observations,
            time_to_first_exhaustion: state.time_to_first_exhaustion,
            coverage_over_time: state.coverage_over_time,
            threat_level_over_time: state.threat_level_over_time,
            availability_per_tick: state.availability_per_tick,
            total_hop_count_sum: state.total_hop_count_sum,
            total_hop_count_ticks: state.total_hop_count_ticks,
            base_pose: state.base_pose,
            realism_profile: config.realism_profile,
            wind: config.wind,
            urban_route_planned,
            urban_route_length_m,
            urban_route_risk_score,
            urban_violation_count,
            urban_route_completed,
            unsupported_reason,
            log_builder: state.log_builder,
        })
    }
}
