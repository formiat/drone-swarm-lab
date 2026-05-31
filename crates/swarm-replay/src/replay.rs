use crate::event_log::{Event, EventLog};
use swarm_types::{AgentId, Pose, TaskId};

/// Minimal replay state that reconstructs the system from an event log.
///
/// The replay engine does not re-run the simulation; it reconstructs
/// the final state by applying events in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplayState {
    pub failed_agents: Vec<(AgentId, u64)>,
    pub assigned_tasks: Vec<(TaskId, AgentId, u64)>,
    pub messages_sent: u64,
    pub messages_dropped: u64,
    pub partition_events: u64,
    pub final_poses: Vec<(AgentId, swarm_types::Pose)>,
}

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

/// Snapshot of the system at a specific tick.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplaySnapshot {
    pub tick: u64,
    pub agent_poses: Vec<(AgentId, Pose)>,
    pub assigned_tasks: Vec<(TaskId, AgentId)>,
    pub active_agents: Vec<AgentId>,
    pub failed_agents: Vec<AgentId>,
}

/// Replay event category for timeline filtering.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReplayEventCategory {
    Generic,
    Urban,
}

impl ReplayEventCategory {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "generic" => Some(Self::Generic),
            "urban" => Some(Self::Urban),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Urban => "urban",
        }
    }
}

/// Filters applied to text timeline output.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct ReplayTimelineFilter {
    pub agent_id: Option<AgentId>,
    pub category: Option<ReplayEventCategory>,
}

/// One formatted replay timeline item.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReplayTimelineItem {
    pub tick: u64,
    pub category: ReplayEventCategory,
    pub agent_id: Option<AgentId>,
    pub event_name: &'static str,
    pub details: String,
}

/// Build deterministic replay timeline items from an event log.
pub fn timeline_items(log: &EventLog, filter: &ReplayTimelineFilter) -> Vec<ReplayTimelineItem> {
    log.events
        .iter()
        .filter_map(|event| {
            let category = event_category(event);
            if filter.category.is_some_and(|wanted| wanted != category) {
                return None;
            }
            let agent_id = event_agent_id(event);
            if let Some(wanted_agent_id) = &filter.agent_id {
                if agent_id.as_ref() != Some(wanted_agent_id) {
                    return None;
                }
            }
            Some(ReplayTimelineItem {
                tick: event_tick(event),
                category,
                agent_id,
                event_name: event_name(event),
                details: event_details(event),
            })
        })
        .collect()
}

/// Format a deterministic, line-oriented replay timeline.
pub fn format_timeline(log: &EventLog, filter: &ReplayTimelineFilter) -> String {
    let lines: Vec<String> = timeline_items(log, filter)
        .into_iter()
        .map(|item| {
            let agent = item
                .agent_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".to_owned());
            format!(
                "tick={tick:05} category={category} agent={agent} event={event} {details}",
                tick = item.tick,
                category = item.category.as_str(),
                event = item.event_name,
                details = item.details
            )
        })
        .collect();
    if lines.is_empty() {
        "No timeline events matched the filter.\n".to_owned()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

/// Replay an event log and produce the final reconstructed state.
pub fn replay(log: &EventLog) -> ReplayState {
    let mut state = ReplayState::default();

    for event in &log.events {
        match event {
            Event::AgentFailed { agent_id, tick } => {
                state.failed_agents.push((agent_id.clone(), *tick));
            }
            Event::TaskAssigned {
                task_id,
                agent_id,
                tick,
            } => {
                state
                    .assigned_tasks
                    .push((task_id.clone(), agent_id.clone(), *tick));
            }
            Event::MessageSent { .. } => {
                state.messages_sent += 1;
            }
            Event::MessageDropped { .. } => {
                state.messages_dropped += 1;
            }
            Event::PartitionAdded { .. } | Event::PartitionRemoved { .. } => {
                state.partition_events += 1;
            }
            Event::PoseUpdated { agent_id, pose, .. } => {
                // Overwrite previous pose for this agent
                if let Some(entry) = state.final_poses.iter_mut().find(|(id, _)| id == agent_id) {
                    entry.1 = *pose;
                } else {
                    state.final_poses.push((agent_id.clone(), *pose));
                }
            }
            Event::TickStart { .. }
            | Event::TaskStarted { .. }
            | Event::TaskCompleted { .. }
            | Event::TaskExpired { .. }
            | Event::SarScan { .. }
            | Event::SarDetection { .. }
            | Event::EdgeVisited { .. }
            | Event::SafetyViolation { .. }
            | Event::CbbaConverged { .. }
            | Event::CbbaBundleUpdated { .. }
            | Event::AgentObservation { .. }
            | Event::HazardMapUpdated { .. }
            | Event::TaskPriorityUpdated { .. }
            | Event::UrbanRoutePlanned { .. }
            | Event::UrbanSegmentEntered { .. }
            | Event::UrbanSegmentCompleted { .. }
            | Event::UrbanViolation { .. }
            | Event::UrbanPatrolCompleted { .. }
            | Event::BusObserved { .. }
            | Event::BusDetected { .. }
            | Event::BusFalsePositive { .. }
            | Event::UrbanSearchCompleted { .. } => {}
        }
    }

    state
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

/// Build a snapshot of the system state at the given tick.
pub fn snapshot_at_tick(log: &EventLog, target_tick: u64) -> ReplaySnapshot {
    let mut snapshot = ReplaySnapshot {
        tick: target_tick,
        ..Default::default()
    };

    for event in &log.events {
        match event {
            Event::TickStart { tick } => {
                if *tick > target_tick {
                    break;
                }
            }
            Event::AgentFailed { agent_id, tick } => {
                if *tick > target_tick {
                    continue;
                }
                if !snapshot.failed_agents.contains(agent_id) {
                    snapshot.failed_agents.push(agent_id.clone());
                }
            }
            Event::TaskAssigned {
                task_id,
                agent_id,
                tick,
            } => {
                if *tick > target_tick {
                    continue;
                }
                // Overwrite any previous assignment for this task
                if let Some(entry) = snapshot
                    .assigned_tasks
                    .iter_mut()
                    .find(|(tid, _)| tid == task_id)
                {
                    entry.1 = agent_id.clone();
                } else {
                    snapshot
                        .assigned_tasks
                        .push((task_id.clone(), agent_id.clone()));
                }
            }
            Event::PoseUpdated {
                agent_id,
                pose,
                tick,
                ..
            } => {
                if *tick > target_tick {
                    continue;
                }
                if let Some(entry) = snapshot
                    .agent_poses
                    .iter_mut()
                    .find(|(id, _)| id == agent_id)
                {
                    entry.1 = *pose;
                } else {
                    snapshot.agent_poses.push((agent_id.clone(), *pose));
                }
            }
            _ => {}
        }
    }

    // active agents = all agents in poses that are not failed
    for (agent_id, _) in &snapshot.agent_poses {
        if !snapshot.failed_agents.contains(agent_id) && !snapshot.active_agents.contains(agent_id)
        {
            snapshot.active_agents.push(agent_id.clone());
        }
    }

    snapshot
}

fn event_category(event: &Event) -> ReplayEventCategory {
    match event {
        Event::UrbanRoutePlanned { .. }
        | Event::UrbanSegmentEntered { .. }
        | Event::UrbanSegmentCompleted { .. }
        | Event::UrbanViolation { .. }
        | Event::UrbanPatrolCompleted { .. }
        | Event::BusObserved { .. }
        | Event::BusDetected { .. }
        | Event::BusFalsePositive { .. }
        | Event::UrbanSearchCompleted { .. } => ReplayEventCategory::Urban,
        Event::TickStart { .. }
        | Event::AgentFailed { .. }
        | Event::TaskAssigned { .. }
        | Event::TaskStarted { .. }
        | Event::TaskCompleted { .. }
        | Event::TaskExpired { .. }
        | Event::MessageSent { .. }
        | Event::MessageDropped { .. }
        | Event::PartitionAdded { .. }
        | Event::PartitionRemoved { .. }
        | Event::PoseUpdated { .. }
        | Event::SarScan { .. }
        | Event::SarDetection { .. }
        | Event::EdgeVisited { .. }
        | Event::SafetyViolation { .. }
        | Event::CbbaConverged { .. }
        | Event::CbbaBundleUpdated { .. }
        | Event::AgentObservation { .. }
        | Event::HazardMapUpdated { .. }
        | Event::TaskPriorityUpdated { .. } => ReplayEventCategory::Generic,
    }
}

fn event_agent_id(event: &Event) -> Option<AgentId> {
    match event {
        Event::AgentFailed { agent_id, .. }
        | Event::TaskAssigned { agent_id, .. }
        | Event::TaskStarted { agent_id, .. }
        | Event::TaskCompleted { agent_id, .. }
        | Event::PoseUpdated { agent_id, .. }
        | Event::SarScan { agent_id, .. }
        | Event::SarDetection { agent_id, .. }
        | Event::EdgeVisited { agent_id, .. }
        | Event::SafetyViolation { agent_id, .. }
        | Event::CbbaBundleUpdated { agent_id, .. }
        | Event::AgentObservation { agent_id, .. }
        | Event::UrbanRoutePlanned { agent_id, .. }
        | Event::UrbanSegmentEntered { agent_id, .. }
        | Event::UrbanSegmentCompleted { agent_id, .. }
        | Event::UrbanViolation { agent_id, .. }
        | Event::UrbanPatrolCompleted { agent_id, .. }
        | Event::BusObserved { agent_id, .. }
        | Event::BusDetected { agent_id, .. }
        | Event::BusFalsePositive { agent_id, .. }
        | Event::UrbanSearchCompleted { agent_id, .. } => Some(agent_id.clone()),
        Event::MessageSent { from, .. }
        | Event::MessageDropped { from, .. }
        | Event::PartitionAdded { agent_a: from, .. }
        | Event::PartitionRemoved { agent_a: from, .. } => Some(from.clone()),
        Event::TickStart { .. }
        | Event::TaskExpired { .. }
        | Event::CbbaConverged { .. }
        | Event::HazardMapUpdated { .. }
        | Event::TaskPriorityUpdated { .. } => None,
    }
}

fn event_tick(event: &Event) -> u64 {
    match event {
        Event::TickStart { tick }
        | Event::AgentFailed { tick, .. }
        | Event::TaskAssigned { tick, .. }
        | Event::TaskStarted { tick, .. }
        | Event::TaskCompleted { tick, .. }
        | Event::TaskExpired { tick, .. }
        | Event::MessageSent { tick, .. }
        | Event::MessageDropped { tick, .. }
        | Event::PartitionAdded { tick, .. }
        | Event::PartitionRemoved { tick, .. }
        | Event::PoseUpdated { tick, .. }
        | Event::SarScan { tick, .. }
        | Event::SarDetection { tick, .. }
        | Event::EdgeVisited { tick, .. }
        | Event::SafetyViolation { tick, .. }
        | Event::CbbaConverged { tick }
        | Event::CbbaBundleUpdated { tick, .. }
        | Event::AgentObservation { tick, .. }
        | Event::HazardMapUpdated { tick, .. }
        | Event::TaskPriorityUpdated { tick, .. }
        | Event::UrbanRoutePlanned { tick, .. }
        | Event::UrbanSegmentEntered { tick, .. }
        | Event::UrbanSegmentCompleted { tick, .. }
        | Event::UrbanViolation { tick, .. }
        | Event::UrbanPatrolCompleted { tick, .. }
        | Event::BusObserved { tick, .. }
        | Event::BusDetected { tick, .. }
        | Event::BusFalsePositive { tick, .. }
        | Event::UrbanSearchCompleted { tick, .. } => *tick,
    }
}

fn event_name(event: &Event) -> &'static str {
    match event {
        Event::TickStart { .. } => "TickStart",
        Event::AgentFailed { .. } => "AgentFailed",
        Event::TaskAssigned { .. } => "TaskAssigned",
        Event::TaskStarted { .. } => "TaskStarted",
        Event::TaskCompleted { .. } => "TaskCompleted",
        Event::TaskExpired { .. } => "TaskExpired",
        Event::MessageSent { .. } => "MessageSent",
        Event::MessageDropped { .. } => "MessageDropped",
        Event::PartitionAdded { .. } => "PartitionAdded",
        Event::PartitionRemoved { .. } => "PartitionRemoved",
        Event::PoseUpdated { .. } => "PoseUpdated",
        Event::SarScan { .. } => "SarScan",
        Event::SarDetection { .. } => "SarDetection",
        Event::EdgeVisited { .. } => "EdgeVisited",
        Event::SafetyViolation { .. } => "SafetyViolation",
        Event::CbbaConverged { .. } => "CbbaConverged",
        Event::CbbaBundleUpdated { .. } => "CbbaBundleUpdated",
        Event::AgentObservation { .. } => "AgentObservation",
        Event::HazardMapUpdated { .. } => "HazardMapUpdated",
        Event::TaskPriorityUpdated { .. } => "TaskPriorityUpdated",
        Event::UrbanRoutePlanned { .. } => "UrbanRoutePlanned",
        Event::UrbanSegmentEntered { .. } => "UrbanSegmentEntered",
        Event::UrbanSegmentCompleted { .. } => "UrbanSegmentCompleted",
        Event::UrbanViolation { .. } => "UrbanViolation",
        Event::UrbanPatrolCompleted { .. } => "UrbanPatrolCompleted",
        Event::BusObserved { .. } => "BusObserved",
        Event::BusDetected { .. } => "BusDetected",
        Event::BusFalsePositive { .. } => "BusFalsePositive",
        Event::UrbanSearchCompleted { .. } => "UrbanSearchCompleted",
    }
}

fn event_details(event: &Event) -> String {
    match event {
        Event::TickStart { .. } => String::new(),
        Event::AgentFailed { agent_id, .. } => format!("failed_agent={agent_id}"),
        Event::TaskAssigned {
            task_id, agent_id, ..
        } => format!("task={task_id} assigned_to={agent_id}"),
        Event::TaskStarted {
            task_id, agent_id, ..
        } => format!("task={task_id} started_by={agent_id}"),
        Event::TaskCompleted {
            task_id, agent_id, ..
        } => format!("task={task_id} completed_by={agent_id}"),
        Event::TaskExpired { task_id, .. } => format!("task={task_id}"),
        Event::MessageSent {
            from,
            to,
            payload_len,
            ..
        } => format!("from={from} to={to} payload_len={payload_len}"),
        Event::MessageDropped {
            from, to, reason, ..
        } => format!("from={from} to={to} reason={reason:?}"),
        Event::PartitionAdded {
            agent_a, agent_b, ..
        } => format!("agent_a={agent_a} agent_b={agent_b}"),
        Event::PartitionRemoved {
            agent_a, agent_b, ..
        } => format!("agent_a={agent_a} agent_b={agent_b}"),
        Event::PoseUpdated { pose, .. } => format_pose(*pose),
        Event::SarScan { cell, detected, .. } => {
            format!("cell=({}, {}) detected={detected}", cell.0, cell.1)
        }
        Event::SarDetection { target_pose, .. } => format!("target_{}", format_pose(*target_pose)),
        Event::EdgeVisited { edge_id, .. } => format!("edge={edge_id}"),
        Event::SafetyViolation {
            violation_type, ..
        } => format!("violation_type={violation_type:?}"),
        Event::CbbaConverged { .. } => "converged=true".to_owned(),
        Event::CbbaBundleUpdated { bundle_size, .. } => format!("bundle_size={bundle_size}"),
        Event::AgentObservation { zone_id, .. } => format!("zone={zone_id}"),
        Event::HazardMapUpdated {
            zone_id,
            new_threat_level,
            new_priority,
            ..
        } => format!(
            "zone={zone_id} threat={new_threat_level:.3} priority={new_priority}"
        ),
        Event::TaskPriorityUpdated {
            task_id,
            old_priority,
            new_priority,
            ..
        } => format!("task={task_id} old_priority={old_priority} new_priority={new_priority}"),
        Event::UrbanRoutePlanned {
            edge_ids,
            route_length_m,
            ..
        } => format!("edges={} route_length_m={route_length_m:.3}", edge_ids.len()),
        Event::UrbanSegmentEntered {
            segment_index,
            edge_id,
            from,
            to,
            ..
        } => format!("segment={segment_index} edge={edge_id} from={from} to={to}"),
        Event::UrbanSegmentCompleted {
            segment_index,
            edge_id,
            ..
        } => format!("segment={segment_index} edge={edge_id}"),
        Event::UrbanViolation {
            segment_index,
            edge_id,
            obstacle_id,
            pose,
            reason,
            ..
        } => format!(
            "segment={segment} edge={edge} obstacle={obstacle} {pose} reason={reason}",
            segment = optional_usize(*segment_index),
            edge = optional_display(edge_id.as_ref()),
            obstacle = optional_display(obstacle_id.as_ref()),
            pose = format_pose(*pose)
        ),
        Event::UrbanPatrolCompleted {
            route_length_m,
            distance_travelled_m,
            ..
        } => format!(
            "route_length_m={route_length_m:.3} distance_travelled_m={distance_travelled_m:.3}"
        ),
        Event::BusObserved {
            bus_id,
            distance_m,
            detector_seed,
            ..
        } => format!("bus={bus_id} distance_m={distance_m:.3} detector_seed={detector_seed}"),
        Event::BusDetected {
            bus_id,
            distance_m,
            detector_seed,
            ..
        } => format!("bus={bus_id} distance_m={distance_m:.3} detector_seed={detector_seed}"),
        Event::BusFalsePositive { detector_seed, .. } => {
            format!("detector_seed={detector_seed}")
        }
        Event::UrbanSearchCompleted {
            detected,
            bus_id,
            reason,
            distance_travelled_m,
            ..
        } => format!(
            "detected={detected} bus={bus} reason={reason} distance_travelled_m={distance_travelled_m:.3}",
            bus = optional_display(bus_id.as_ref())
        ),
    }
}

fn optional_display<T: ToString>(value: Option<&T>) -> String {
    value
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".to_owned())
}

fn optional_usize(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_owned())
}

fn format_pose(pose: Pose) -> String {
    format!("pose=({:.3},{:.3},{:.3})", pose.x, pose.y, pose.z)
}

/// Render an ASCII grid snapshot.
///
/// Agents are rendered as 'A', failed agents as 'X', empty cells as '.'.
/// When multiple agents occupy the same cell, the count is shown.
pub fn render_ascii_grid(
    snapshot: &ReplaySnapshot,
    grid_bounds: (f64, f64, f64, f64),
    grid_size: usize,
) -> String {
    let (min_x, max_x, min_y, max_y) = grid_bounds;
    let cell_w = (max_x - min_x) / grid_size as f64;
    let cell_h = (max_y - min_y) / grid_size as f64;

    // Count agents per cell
    let mut grid: Vec<Vec<u32>> = vec![vec![0; grid_size]; grid_size];
    let mut failed_grid: Vec<Vec<u32>> = vec![vec![0; grid_size]; grid_size];

    for (agent_id, pose) in &snapshot.agent_poses {
        let gx = ((pose.x - min_x) / cell_w).clamp(0.0, grid_size as f64 - 1.0) as usize;
        let gy = ((pose.y - min_y) / cell_h).clamp(0.0, grid_size as f64 - 1.0) as usize;
        if snapshot.failed_agents.contains(agent_id) {
            failed_grid[gy][gx] += 1;
        } else {
            grid[gy][gx] += 1;
        }
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "Tick {}  ({}x{} grid)",
        snapshot.tick, grid_size, grid_size
    ));
    lines.push("-".repeat(grid_size));

    for gy in (0..grid_size).rev() {
        let mut row = String::new();
        for gx in 0..grid_size {
            let active = grid[gy][gx];
            let failed = failed_grid[gy][gx];
            let ch = if active > 0 && failed > 0 {
                '*'
            } else if active > 1 {
                char::from_digit(active.min(9), 10).unwrap_or('A')
            } else if active == 1 {
                'A'
            } else if failed > 1 {
                char::from_digit(failed.min(9), 10).unwrap_or('X')
            } else if failed == 1 {
                'X'
            } else {
                '.'
            };
            row.push(ch);
        }
        lines.push(row);
    }

    lines.push("-".repeat(grid_size));
    lines.push("Legend: A=active agent  X=failed agent  *=mixed  .=empty".to_owned());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_log::EventLogBuilder;
    use swarm_types::{AgentId, Pose, TaskId, UrbanEdgeId, UrbanNodeId};

    #[test]
    fn replay_reconstructs_state() {
        let mut builder = EventLogBuilder::new("test", 0, "scenario");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::MessageSent {
            from: AgentId::from("a0".to_owned()),
            to: AgentId::from("a1".to_owned()),
            tick: 1,
            payload_len: 10,
        });
        builder.push(Event::TaskAssigned {
            task_id: TaskId::from("t0".to_owned()),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 2,
        });
        builder.push(Event::AgentFailed {
            agent_id: AgentId::from("a1".to_owned()),
            tick: 3,
        });

        let log = builder.build();
        let state = replay(&log);

        assert_eq!(state.messages_sent, 1);
        assert_eq!(state.assigned_tasks.len(), 1);
        assert_eq!(state.failed_agents.len(), 1);
    }

    #[test]
    fn summarize_counts_events_correctly() {
        let mut builder = EventLogBuilder::new("test", 0, "s");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::TickStart { tick: 1 });
        builder.push(Event::TaskAssigned {
            task_id: TaskId::from("t0".to_owned()),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 0,
        });
        builder.push(Event::TaskCompleted {
            task_id: TaskId::from("t0".to_owned()),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 1,
        });
        builder.push(Event::AgentFailed {
            agent_id: AgentId::from("a1".to_owned()),
            tick: 1,
        });
        builder.push(Event::SarScan {
            agent_id: AgentId::from("a0".to_owned()),
            cell: (0, 0),
            tick: 0,
            detected: false,
        });
        builder.push(Event::SarDetection {
            agent_id: AgentId::from("a0".to_owned()),
            target_pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            tick: 1,
        });
        builder.push(Event::EdgeVisited {
            edge_id: "e0".to_owned(),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 1,
        });
        builder.push(Event::SafetyViolation {
            agent_id: AgentId::from("a0".to_owned()),
            violation_type: crate::event_log::ViolationType::NoFly,
            tick: 1,
        });
        builder.push(Event::CbbaConverged { tick: 1 });
        builder.push(Event::MessageSent {
            from: AgentId::from("a0".to_owned()),
            to: AgentId::from("a1".to_owned()),
            tick: 0,
            payload_len: 10,
        });
        builder.push(Event::MessageDropped {
            from: AgentId::from("a0".to_owned()),
            to: AgentId::from("a1".to_owned()),
            tick: 0,
            reason: crate::event_log::DropReason::PacketLoss,
        });

        let log = builder.build();
        let s = summarize(&log);
        assert_eq!(s.total_ticks, 1);
        assert_eq!(s.assignments, 1);
        assert_eq!(s.completions, 1);
        assert_eq!(s.failures, 1);
        assert_eq!(s.sar_scans, 1);
        assert_eq!(s.sar_detections, 1);
        assert_eq!(s.edges_visited, 1);
        assert_eq!(s.safety_violations, 1);
        assert_eq!(s.cbba_convergence_ticks, vec![1]);
        assert_eq!(s.messages_sent, 1);
        assert_eq!(s.messages_dropped, 1);
    }

    #[test]
    fn summarize_counts_urban_events() {
        let mut builder = EventLogBuilder::new("urban", 0, "urban_patrol_small_block");
        let agent_id = AgentId::from("agent-0".to_owned());
        let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
        let bus_id = swarm_types::UrbanBusId::from("bus-0".to_owned());
        builder.push(Event::UrbanRoutePlanned {
            agent_id: agent_id.clone(),
            tick: 0,
            edge_ids: vec![edge_id.clone()],
            route_length_m: 20.0,
        });
        builder.push(Event::UrbanSegmentEntered {
            agent_id: agent_id.clone(),
            tick: 0,
            segment_index: 0,
            edge_id: edge_id.clone(),
            from: UrbanNodeId::from("n0".to_owned()),
            to: UrbanNodeId::from("n1".to_owned()),
        });
        builder.push(Event::UrbanSegmentCompleted {
            agent_id: agent_id.clone(),
            tick: 10,
            segment_index: 0,
            edge_id: edge_id.clone(),
        });
        builder.push(Event::UrbanViolation {
            agent_id: agent_id.clone(),
            tick: 11,
            segment_index: Some(0),
            edge_id: Some(edge_id),
            obstacle_id: None,
            pose: Pose::default(),
            reason: "test".to_owned(),
        });
        builder.push(Event::UrbanPatrolCompleted {
            agent_id: agent_id.clone(),
            tick: 12,
            route_length_m: 20.0,
            distance_travelled_m: 20.0,
        });
        builder.push(Event::BusObserved {
            agent_id: agent_id.clone(),
            tick: 13,
            bus_id: bus_id.clone(),
            pose: Pose::default(),
            distance_m: 1.0,
            detector_seed: 9,
        });
        builder.push(Event::BusDetected {
            agent_id: agent_id.clone(),
            tick: 13,
            bus_id: bus_id.clone(),
            pose: Pose::default(),
            distance_m: 1.0,
            detector_seed: 9,
        });
        builder.push(Event::BusFalsePositive {
            agent_id: agent_id.clone(),
            tick: 14,
            pose: Pose::default(),
            detector_seed: 9,
        });
        builder.push(Event::UrbanSearchCompleted {
            agent_id: agent_id.clone(),
            tick: 13,
            detected: true,
            bus_id: Some(bus_id),
            reason: "detected".to_owned(),
            distance_travelled_m: 10.0,
        });
        builder.push(Event::UrbanSearchCompleted {
            agent_id,
            tick: 20,
            detected: false,
            bus_id: None,
            reason: "timeout".to_owned(),
            distance_travelled_m: 40.0,
        });

        let summary = summarize(&builder.build());
        assert_eq!(summary.urban_routes_planned, 1);
        assert_eq!(summary.urban_segments_entered, 1);
        assert_eq!(summary.urban_segments_completed, 1);
        assert_eq!(summary.urban_violations, 1);
        assert_eq!(summary.urban_patrol_completions, 1);
        assert_eq!(summary.urban_completion_ticks, vec![12]);
        assert_eq!(summary.bus_observations, 1);
        assert_eq!(summary.bus_detections, 1);
        assert_eq!(summary.bus_false_positives, 1);
        assert_eq!(summary.urban_search_completions, 2);
        assert_eq!(summary.urban_search_time_to_detection_ticks, vec![13]);
        assert_eq!(summary.urban_search_no_detection_count, 1);
    }

    #[test]
    fn timeline_output_is_deterministic() {
        let log = timeline_fixture();
        let timeline = format_timeline(&log, &ReplayTimelineFilter::default());

        assert!(timeline.contains("tick=00000 category=generic agent=- event=TickStart"));
        assert!(timeline.contains(
            "tick=00000 category=urban agent=agent-0 event=UrbanRoutePlanned edges=1 route_length_m=20.000"
        ));
        assert!(timeline.contains(
            "tick=00001 category=urban agent=agent-0 event=UrbanSegmentCompleted segment=0 edge=road-n0-n1"
        ));
        let first_urban = timeline.find("UrbanRoutePlanned").unwrap();
        let completed = timeline.find("UrbanSegmentCompleted").unwrap();
        assert!(first_urban < completed);
    }

    #[test]
    fn timeline_filter_by_agent() {
        let log = timeline_fixture();
        let timeline = format_timeline(
            &log,
            &ReplayTimelineFilter {
                agent_id: Some(AgentId::from("agent-1".to_owned())),
                category: None,
            },
        );

        assert!(timeline.contains("agent=agent-1"));
        assert!(timeline.contains("PoseUpdated"));
        assert!(!timeline.contains("agent=agent-0"));
    }

    #[test]
    fn timeline_filter_by_urban_category() {
        let log = timeline_fixture();
        let timeline = format_timeline(
            &log,
            &ReplayTimelineFilter {
                agent_id: None,
                category: Some(ReplayEventCategory::Urban),
            },
        );

        assert!(timeline.contains("category=urban"));
        assert!(timeline.contains("UrbanRoutePlanned"));
        assert!(!timeline.contains("PoseUpdated"));
        assert!(!timeline.contains("TickStart"));
    }

    #[test]
    fn timeline_reports_empty_filter() {
        let log = timeline_fixture();
        let timeline = format_timeline(
            &log,
            &ReplayTimelineFilter {
                agent_id: Some(AgentId::from("missing".to_owned())),
                category: Some(ReplayEventCategory::Urban),
            },
        );

        assert_eq!(timeline, "No timeline events matched the filter.\n");
    }

    #[test]
    fn snapshot_at_tick_reconstructs_poses() {
        let mut builder = EventLogBuilder::new("test", 0, "s");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::PoseUpdated {
            agent_id: AgentId::from("a0".to_owned()),
            pose: Pose {
                x: 1.0,
                y: 2.0,
                ..Default::default()
            },
            tick: 0,
        });
        builder.push(Event::TickStart { tick: 1 });
        builder.push(Event::PoseUpdated {
            agent_id: AgentId::from("a0".to_owned()),
            pose: Pose {
                x: 3.0,
                y: 4.0,
                ..Default::default()
            },
            tick: 1,
        });

        let log = builder.build();
        let snap0 = snapshot_at_tick(&log, 0);
        assert_eq!(snap0.agent_poses.len(), 1);
        assert_eq!(
            snap0.agent_poses[0].1,
            Pose {
                x: 1.0,
                y: 2.0,
                ..Default::default()
            }
        );

        let snap1 = snapshot_at_tick(&log, 1);
        assert_eq!(
            snap1.agent_poses[0].1,
            Pose {
                x: 3.0,
                y: 4.0,
                ..Default::default()
            }
        );
    }

    #[test]
    fn render_ascii_grid_basic() {
        let snapshot = ReplaySnapshot {
            tick: 0,
            agent_poses: vec![(
                AgentId::from("a0".to_owned()),
                Pose {
                    x: 5.0,
                    y: 5.0,
                    ..Default::default()
                },
            )],
            assigned_tasks: vec![],
            active_agents: vec![AgentId::from("a0".to_owned())],
            failed_agents: vec![],
        };
        let grid = render_ascii_grid(&snapshot, (0.0, 10.0, 0.0, 10.0), 5);
        assert!(grid.contains('A'));
        assert!(grid.contains('.'));
    }

    fn timeline_fixture() -> EventLog {
        let mut builder = EventLogBuilder::new("urban", 0, "urban_patrol_small_block");
        let agent_0 = AgentId::from("agent-0".to_owned());
        let agent_1 = AgentId::from("agent-1".to_owned());
        let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::UrbanRoutePlanned {
            agent_id: agent_0.clone(),
            tick: 0,
            edge_ids: vec![edge_id.clone()],
            route_length_m: 20.0,
        });
        builder.push(Event::PoseUpdated {
            agent_id: agent_1,
            pose: Pose {
                x: 10.0,
                y: 10.0,
                ..Default::default()
            },
            tick: 0,
        });
        builder.push(Event::UrbanSegmentCompleted {
            agent_id: agent_0,
            tick: 1,
            segment_index: 0,
            edge_id,
        });
        builder.build()
    }
}
