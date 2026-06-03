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
            _ => {}
        }
    }

    summary
}
