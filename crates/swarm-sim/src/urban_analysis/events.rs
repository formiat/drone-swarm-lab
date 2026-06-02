use swarm_replay::{Event, EventLog};

use super::UrbanEventCounts;

/// Count Urban-related replay events.
pub fn count_urban_events(log: &EventLog) -> UrbanEventCounts {
    let mut counts = UrbanEventCounts::default();
    for event in &log.events {
        match event {
            Event::UrbanRoutePlanned { .. } => counts.route_planned += 1,
            Event::UrbanSegmentEntered { .. } => counts.segment_entered += 1,
            Event::UrbanSegmentCompleted { .. } => counts.segment_completed += 1,
            Event::UrbanViolation { .. } => counts.violation += 1,
            Event::UrbanPatrolCompleted { .. } => counts.patrol_completed += 1,
            Event::BusObserved { .. } => counts.bus_observed += 1,
            Event::BusDetected { .. } => counts.bus_detected += 1,
            Event::BusFalsePositive { .. } => counts.bus_false_positive += 1,
            Event::UrbanSearchCompleted { .. } => counts.search_completed += 1,
            Event::PoseUpdated { .. } => counts.pose_updated += 1,
            _ => {}
        }
    }
    counts
}
