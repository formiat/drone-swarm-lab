use crate::event_log::{Event, EventLog};

/// Summary of a replay log with mission-specific counts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplaySummary {
    pub total_ticks: u64,
    pub assignments: usize,
    pub completions: usize,
    pub conflicts: usize,
    pub failures: usize,
    pub safety_violations: usize,
    pub sar_scans: usize,
    pub sar_detections: usize,
    pub edges_visited: usize,
    pub cbba_convergence_ticks: Vec<u64>,
    pub messages_sent: u64,
    pub messages_dropped: u64,
    // v0.38 Wildfire v2
    pub zones_mapped: usize,
    pub hazard_updates: usize,
    pub observations: usize,
    pub priority_reallocation_requests: usize,
    pub priority_task_releases: usize,
    // M65 Urban Patrol v0
    pub urban_routes_planned: usize,
    pub urban_segments_entered: usize,
    pub urban_segments_completed: usize,
    pub urban_violations: usize,
    pub urban_patrol_completions: usize,
    pub urban_completion_ticks: Vec<u64>,
    // M66 Urban Search v1
    pub bus_observations: usize,
    pub bus_detections: usize,
    pub bus_false_positives: usize,
    pub urban_search_completions: usize,
    pub urban_search_time_to_detection_ticks: Vec<u64>,
    pub urban_search_no_detection_count: usize,
    // M87 Swarm Command Plane
    pub swarm_command_plan_dispatched_count: usize,
    pub swarm_agent_command_dispatched_count: usize,
    pub swarm_ownership_handoff_count: usize,
    pub swarm_sync_partial_failure_count: usize,
    pub swarm_supervisor_state_change_count: usize,
    // M88 Logical Swarm Topologies
    pub swarm_topology_configured_count: usize,
    pub swarm_command_route_selected_count: usize,
    pub swarm_command_route_blocked_count: usize,
    pub swarm_topology_degraded_count: usize,
    pub swarm_mothership_dependency_count: usize,
}

/// Summarize an event log into key metrics.
pub fn summarize(log: &EventLog) -> ReplaySummary {
    let mut summary = ReplaySummary::default();

    for event in &log.events {
        match event {
            Event::TickStart { tick } => {
                summary.total_ticks = summary.total_ticks.max(*tick);
            }
            Event::TaskAssigned { .. } => {
                summary.assignments += 1;
            }
            Event::TaskCompleted { .. } => {
                summary.completions += 1;
            }
            Event::AgentFailed { .. } => {
                summary.failures += 1;
            }
            Event::MessageSent { .. } => {
                summary.messages_sent += 1;
            }
            Event::MessageDropped { .. } => {
                summary.messages_dropped += 1;
            }
            Event::SafetyViolation { .. } => {
                summary.safety_violations += 1;
            }
            Event::SarScan { .. } => {
                summary.sar_scans += 1;
            }
            Event::SarDetection { .. } => {
                summary.sar_detections += 1;
            }
            Event::EdgeVisited { .. } => {
                summary.edges_visited += 1;
            }
            Event::CbbaConverged { tick } => {
                summary.cbba_convergence_ticks.push(*tick);
            }
            Event::AgentObservation { .. } => {
                summary.observations += 1;
            }
            Event::HazardMapUpdated { .. } => {
                summary.hazard_updates += 1;
            }
            Event::WildfirePriorityReallocationRequested { .. } => {
                summary.priority_reallocation_requests += 1;
            }
            Event::WildfirePriorityTaskReleased { .. } => {
                summary.priority_task_releases += 1;
            }
            Event::UrbanRoutePlanned { .. } => {
                summary.urban_routes_planned += 1;
            }
            Event::UrbanSegmentEntered { .. } => {
                summary.urban_segments_entered += 1;
            }
            Event::UrbanSegmentCompleted { .. } => {
                summary.urban_segments_completed += 1;
            }
            Event::UrbanViolation { .. } => {
                summary.urban_violations += 1;
            }
            Event::UrbanPatrolCompleted { tick, .. } => {
                summary.urban_patrol_completions += 1;
                summary.urban_completion_ticks.push(*tick);
            }
            Event::BusObserved { .. } => {
                summary.bus_observations += 1;
            }
            Event::BusDetected { tick, .. } => {
                summary.bus_detections += 1;
                summary.urban_search_time_to_detection_ticks.push(*tick);
            }
            Event::BusFalsePositive { .. } => {
                summary.bus_false_positives += 1;
            }
            Event::UrbanSearchCompleted { detected, .. } => {
                summary.urban_search_completions += 1;
                if !detected {
                    summary.urban_search_no_detection_count += 1;
                }
            }
            Event::SwarmCommandPlanDispatched { .. } => {
                summary.swarm_command_plan_dispatched_count += 1;
            }
            Event::SwarmAgentCommandDispatched { .. } => {
                summary.swarm_agent_command_dispatched_count += 1;
            }
            Event::SwarmOwnershipHandoff { .. } => {
                summary.swarm_ownership_handoff_count += 1;
            }
            Event::SwarmSyncCommandResult {
                failed_agent_ids,
                timed_out_agent_ids,
                ..
            } => {
                if !failed_agent_ids.is_empty() || !timed_out_agent_ids.is_empty() {
                    summary.swarm_sync_partial_failure_count += 1;
                }
            }
            Event::SwarmSupervisorStateChanged { .. } => {
                summary.swarm_supervisor_state_change_count += 1;
            }
            Event::SwarmTopologyConfigured { .. } => {
                summary.swarm_topology_configured_count += 1;
            }
            Event::SwarmCommandRouteSelected { .. } => {
                summary.swarm_command_route_selected_count += 1;
            }
            Event::SwarmCommandRouteBlocked { .. } => {
                summary.swarm_command_route_blocked_count += 1;
            }
            Event::SwarmTopologyDegraded { .. } => {
                summary.swarm_topology_degraded_count += 1;
            }
            Event::SwarmMothershipDependencyRecorded { .. } => {
                summary.swarm_mothership_dependency_count += 1;
            }
            _ => {}
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use swarm_types::AgentId;

    use super::*;

    #[test]
    fn swarm_command_plane_events_update_summary() {
        let log = EventLog {
            schema_version: "0.2".to_owned(),
            run_id: "m87".to_owned(),
            seed: 1,
            scenario_name: "swarm".to_owned(),
            events: vec![
                Event::SwarmCommandPlanDispatched {
                    tick: 1,
                    plan_id: "plan-1".to_owned(),
                    agent_count: 2,
                },
                Event::SwarmAgentCommandDispatched {
                    tick: 2,
                    plan_id: "plan-1".to_owned(),
                    agent_id: AgentId::from("agent-0".to_owned()),
                    command_count: 3,
                },
                Event::SwarmOwnershipHandoff {
                    tick: 3,
                    from_agent_id: AgentId::from("agent-0".to_owned()),
                    to_agent_id: AgentId::from("agent-1".to_owned()),
                    ownership_kind: "task".to_owned(),
                    resource_id: "wp-0".to_owned(),
                    reason: "replacement".to_owned(),
                },
                Event::SwarmSupervisorStateChanged {
                    tick: 4,
                    from: "active".to_owned(),
                    to: "degraded".to_owned(),
                    reason: "agent_lost".to_owned(),
                },
                Event::SwarmSyncCommandResult {
                    tick: 5,
                    kind: "takeoff_all".to_owned(),
                    succeeded_agent_ids: vec![AgentId::from("agent-0".to_owned())],
                    failed_agent_ids: vec![AgentId::from("agent-1".to_owned())],
                    timed_out_agent_ids: Vec::new(),
                    partial_success: true,
                },
                Event::SwarmTopologyConfigured {
                    tick: 6,
                    topology_kind: "mesh".to_owned(),
                    node_count: 3,
                    link_count: 2,
                },
                Event::SwarmCommandRouteSelected {
                    tick: 7,
                    route_id: "route:gcs:agent-0".to_owned(),
                    from_node_id: "gcs".to_owned(),
                    to_agent_id: AgentId::from("agent-0".to_owned()),
                    via_node_ids: vec!["gcs".to_owned(), "agent:agent-0".to_owned()],
                    degraded: false,
                },
                Event::SwarmCommandRouteBlocked {
                    tick: 8,
                    route_id: "route:gcs:agent-1".to_owned(),
                    from_node_id: "gcs".to_owned(),
                    to_agent_id: AgentId::from("agent-1".to_owned()),
                    reason: "mesh_partition_or_blocked_link".to_owned(),
                },
            ],
        };

        let summary = summarize(&log);

        assert_eq!(summary.swarm_command_plan_dispatched_count, 1);
        assert_eq!(summary.swarm_agent_command_dispatched_count, 1);
        assert_eq!(summary.swarm_ownership_handoff_count, 1);
        assert_eq!(summary.swarm_supervisor_state_change_count, 1);
        assert_eq!(summary.swarm_sync_partial_failure_count, 1);
        assert_eq!(summary.swarm_topology_configured_count, 1);
        assert_eq!(summary.swarm_command_route_selected_count, 1);
        assert_eq!(summary.swarm_command_route_blocked_count, 1);
    }
}
