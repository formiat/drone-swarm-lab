use crate::event_log::{Event, EventLog};
use swarm_types::{AgentId, Pose, TaskId};

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
        | Event::UrbanSearchCompleted { .. }
        | Event::UrbanEdgeBlocked { .. }
        | Event::UrbanEdgeUnblocked { .. }
        | Event::UrbanObstacleDetected { .. }
        | Event::UrbanPolicyDecision { .. }
        | Event::UrbanRouteReplanned { .. }
        | Event::UrbanWaitStarted { .. }
        | Event::UrbanWaitCompleted { .. }
        | Event::UrbanNoRouteAvailable { .. }
        | Event::UrbanSegmentLockAcquired { .. }
        | Event::UrbanSegmentLockReleased { .. }
        | Event::UrbanSegmentConflict { .. }
        | Event::UrbanDeconflictWait { .. }
        | Event::UrbanDeconflictReplan { .. }
        | Event::UrbanDeconflictAbort { .. }
        | Event::UrbanSegmentCoordinatorEvent { .. } => ReplayEventCategory::Urban,
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
        | Event::TaskPriorityUpdated { .. }
        | Event::WildfirePriorityReallocationRequested { .. }
        | Event::WildfirePriorityTaskReleased { .. }
        | Event::SwarmCommandPlanDispatched { .. }
        | Event::SwarmAgentCommandDispatched { .. }
        | Event::SwarmOwnershipAcquired { .. }
        | Event::SwarmOwnershipReleased { .. }
        | Event::SwarmOwnershipHandoff { .. }
        | Event::SwarmSupervisorStateChanged { .. }
        | Event::SwarmSyncCommandIssued { .. }
        | Event::SwarmSyncCommandResult { .. }
        | Event::SwarmTopologyConfigured { .. }
        | Event::SwarmCommandRouteSelected { .. }
        | Event::SwarmCommandRouteBlocked { .. }
        | Event::SwarmTopologyDegraded { .. }
        | Event::SwarmMothershipDependencyRecorded { .. }
        | Event::SwarmProtocolMessage { .. }
        | Event::LeaseGranted { .. }
        | Event::LeaseExpired { .. }
        | Event::OwnershipConflict { .. }
        | Event::AgentGcsLost { .. }
        | Event::AgentGcsReconnected { .. }
        | Event::AgentContinuingUnderLease { .. }
        | Event::AgentLeaseExpiredDuringGcsLoss { .. }
        | Event::AgentNeighborLost { .. }
        | Event::AgentStateReconciled { .. }
        | Event::PartitionDetected { .. }
        | Event::PartitionHealed { .. }
        | Event::SupervisorDegradedDecision { .. }
        | Event::SupervisorReconciled { .. }
        | Event::CommandSuppressed { .. } => ReplayEventCategory::Generic,
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
        | Event::UrbanSearchCompleted { agent_id, .. }
        | Event::UrbanEdgeBlocked { agent_id, .. }
        | Event::UrbanEdgeUnblocked { agent_id, .. }
        | Event::UrbanObstacleDetected { agent_id, .. }
        | Event::UrbanPolicyDecision { agent_id, .. }
        | Event::UrbanRouteReplanned { agent_id, .. }
        | Event::UrbanWaitStarted { agent_id, .. }
        | Event::UrbanWaitCompleted { agent_id, .. }
        | Event::UrbanNoRouteAvailable { agent_id, .. }
        | Event::UrbanSegmentLockAcquired { agent_id, .. }
        | Event::UrbanSegmentLockReleased { agent_id, .. }
        | Event::UrbanDeconflictWait { agent_id, .. }
        | Event::UrbanDeconflictReplan { agent_id, .. }
        | Event::UrbanDeconflictAbort { agent_id, .. }
        | Event::UrbanSegmentCoordinatorEvent { agent_id, .. }
        | Event::SwarmAgentCommandDispatched { agent_id, .. }
        | Event::SwarmOwnershipAcquired { agent_id, .. }
        | Event::SwarmOwnershipReleased { agent_id, .. } => Some(agent_id.clone()),
        Event::UrbanSegmentConflict {
            requester_agent_id, ..
        } => Some(requester_agent_id.clone()),
        Event::SwarmOwnershipHandoff { to_agent_id, .. } => Some(to_agent_id.clone()),
        Event::MessageSent { from, .. }
        | Event::MessageDropped { from, .. }
        | Event::PartitionAdded { agent_a: from, .. }
        | Event::PartitionRemoved { agent_a: from, .. } => Some(from.clone()),
        Event::TickStart { .. }
        | Event::TaskExpired { .. }
        | Event::CbbaConverged { .. }
        | Event::HazardMapUpdated { .. }
        | Event::TaskPriorityUpdated { .. }
        | Event::WildfirePriorityReallocationRequested { .. }
        | Event::SwarmCommandPlanDispatched { .. }
        | Event::SwarmSupervisorStateChanged { .. }
        | Event::SwarmSyncCommandIssued { .. }
        | Event::SwarmSyncCommandResult { .. }
        | Event::SwarmTopologyConfigured { .. }
        | Event::SwarmTopologyDegraded { .. } => None,
        Event::SwarmCommandRouteSelected { to_agent_id, .. }
        | Event::SwarmCommandRouteBlocked { to_agent_id, .. } => Some(to_agent_id.clone()),
        Event::SwarmMothershipDependencyRecorded { child_agent_id, .. } => {
            Some(child_agent_id.clone())
        }
        Event::WildfirePriorityTaskReleased {
            previous_agent_id, ..
        } => previous_agent_id.clone(),
        Event::SwarmProtocolMessage { from, .. } => Some(from.clone()),
        Event::LeaseGranted { holder, .. } => Some(holder.clone()),
        Event::LeaseExpired { .. } => None,
        Event::OwnershipConflict { claimant_a, .. } => Some(claimant_a.clone()),
        Event::AgentGcsLost { agent_id, .. }
        | Event::AgentGcsReconnected { agent_id, .. }
        | Event::AgentContinuingUnderLease { agent_id, .. }
        | Event::AgentLeaseExpiredDuringGcsLoss { agent_id, .. }
        | Event::AgentNeighborLost { agent_id, .. }
        | Event::AgentStateReconciled { agent_id, .. } => Some(agent_id.clone()),
        Event::PartitionDetected { group_a, .. } => group_a.first().cloned(),
        Event::PartitionHealed { .. }
        | Event::SupervisorDegradedDecision { .. }
        | Event::CommandSuppressed { .. } => None,
        Event::SupervisorReconciled { result_summary, .. } => result_summary
            .conflicts
            .first()
            .map(|conflict| conflict.holder_a.clone()),
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
        | Event::WildfirePriorityReallocationRequested { tick, .. }
        | Event::WildfirePriorityTaskReleased { tick, .. }
        | Event::UrbanRoutePlanned { tick, .. }
        | Event::UrbanSegmentEntered { tick, .. }
        | Event::UrbanSegmentCompleted { tick, .. }
        | Event::UrbanViolation { tick, .. }
        | Event::UrbanPatrolCompleted { tick, .. }
        | Event::BusObserved { tick, .. }
        | Event::BusDetected { tick, .. }
        | Event::BusFalsePositive { tick, .. }
        | Event::UrbanSearchCompleted { tick, .. }
        | Event::UrbanEdgeBlocked { tick, .. }
        | Event::UrbanEdgeUnblocked { tick, .. }
        | Event::UrbanObstacleDetected { tick, .. }
        | Event::UrbanPolicyDecision { tick, .. }
        | Event::UrbanRouteReplanned { tick, .. }
        | Event::UrbanWaitStarted { tick, .. }
        | Event::UrbanWaitCompleted { tick, .. }
        | Event::UrbanNoRouteAvailable { tick, .. }
        | Event::UrbanSegmentLockAcquired { tick, .. }
        | Event::UrbanSegmentLockReleased { tick, .. }
        | Event::UrbanSegmentConflict { tick, .. }
        | Event::UrbanDeconflictWait { tick, .. }
        | Event::UrbanDeconflictReplan { tick, .. }
        | Event::UrbanDeconflictAbort { tick, .. }
        | Event::UrbanSegmentCoordinatorEvent { tick, .. }
        | Event::SwarmCommandPlanDispatched { tick, .. }
        | Event::SwarmAgentCommandDispatched { tick, .. }
        | Event::SwarmOwnershipAcquired { tick, .. }
        | Event::SwarmOwnershipReleased { tick, .. }
        | Event::SwarmOwnershipHandoff { tick, .. }
        | Event::SwarmSupervisorStateChanged { tick, .. }
        | Event::SwarmSyncCommandIssued { tick, .. }
        | Event::SwarmSyncCommandResult { tick, .. }
        | Event::SwarmTopologyConfigured { tick, .. }
        | Event::SwarmCommandRouteSelected { tick, .. }
        | Event::SwarmCommandRouteBlocked { tick, .. }
        | Event::SwarmTopologyDegraded { tick, .. }
        | Event::SwarmMothershipDependencyRecorded { tick, .. }
        | Event::SwarmProtocolMessage { tick, .. }
        | Event::LeaseGranted { tick, .. }
        | Event::LeaseExpired { tick, .. }
        | Event::OwnershipConflict { tick, .. }
        | Event::AgentGcsLost { tick, .. }
        | Event::AgentGcsReconnected { tick, .. }
        | Event::AgentContinuingUnderLease { tick, .. }
        | Event::AgentLeaseExpiredDuringGcsLoss { tick, .. }
        | Event::AgentNeighborLost { tick, .. }
        | Event::AgentStateReconciled { tick, .. }
        | Event::PartitionDetected { tick, .. }
        | Event::PartitionHealed { tick, .. }
        | Event::SupervisorDegradedDecision { tick, .. }
        | Event::SupervisorReconciled { tick, .. }
        | Event::CommandSuppressed { tick, .. } => *tick,
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
        Event::WildfirePriorityReallocationRequested { .. } => {
            "WildfirePriorityReallocationRequested"
        }
        Event::WildfirePriorityTaskReleased { .. } => "WildfirePriorityTaskReleased",
        Event::UrbanRoutePlanned { .. } => "UrbanRoutePlanned",
        Event::UrbanSegmentEntered { .. } => "UrbanSegmentEntered",
        Event::UrbanSegmentCompleted { .. } => "UrbanSegmentCompleted",
        Event::UrbanViolation { .. } => "UrbanViolation",
        Event::UrbanPatrolCompleted { .. } => "UrbanPatrolCompleted",
        Event::BusObserved { .. } => "BusObserved",
        Event::BusDetected { .. } => "BusDetected",
        Event::BusFalsePositive { .. } => "BusFalsePositive",
        Event::UrbanSearchCompleted { .. } => "UrbanSearchCompleted",
        Event::UrbanEdgeBlocked { .. } => "UrbanEdgeBlocked",
        Event::UrbanEdgeUnblocked { .. } => "UrbanEdgeUnblocked",
        Event::UrbanObstacleDetected { .. } => "UrbanObstacleDetected",
        Event::UrbanPolicyDecision { .. } => "UrbanPolicyDecision",
        Event::UrbanRouteReplanned { .. } => "UrbanRouteReplanned",
        Event::UrbanWaitStarted { .. } => "UrbanWaitStarted",
        Event::UrbanWaitCompleted { .. } => "UrbanWaitCompleted",
        Event::UrbanNoRouteAvailable { .. } => "UrbanNoRouteAvailable",
        Event::UrbanSegmentLockAcquired { .. } => "UrbanSegmentLockAcquired",
        Event::UrbanSegmentLockReleased { .. } => "UrbanSegmentLockReleased",
        Event::UrbanSegmentConflict { .. } => "UrbanSegmentConflict",
        Event::UrbanDeconflictWait { .. } => "UrbanDeconflictWait",
        Event::UrbanDeconflictReplan { .. } => "UrbanDeconflictReplan",
        Event::UrbanDeconflictAbort { .. } => "UrbanDeconflictAbort",
        Event::UrbanSegmentCoordinatorEvent { .. } => "UrbanSegmentCoordinatorEvent",
        Event::SwarmCommandPlanDispatched { .. } => "SwarmCommandPlanDispatched",
        Event::SwarmAgentCommandDispatched { .. } => "SwarmAgentCommandDispatched",
        Event::SwarmOwnershipAcquired { .. } => "SwarmOwnershipAcquired",
        Event::SwarmOwnershipReleased { .. } => "SwarmOwnershipReleased",
        Event::SwarmOwnershipHandoff { .. } => "SwarmOwnershipHandoff",
        Event::SwarmSupervisorStateChanged { .. } => "SwarmSupervisorStateChanged",
        Event::SwarmSyncCommandIssued { .. } => "SwarmSyncCommandIssued",
        Event::SwarmSyncCommandResult { .. } => "SwarmSyncCommandResult",
        Event::SwarmTopologyConfigured { .. } => "SwarmTopologyConfigured",
        Event::SwarmCommandRouteSelected { .. } => "SwarmCommandRouteSelected",
        Event::SwarmCommandRouteBlocked { .. } => "SwarmCommandRouteBlocked",
        Event::SwarmTopologyDegraded { .. } => "SwarmTopologyDegraded",
        Event::SwarmMothershipDependencyRecorded { .. } => "SwarmMothershipDependencyRecorded",
        Event::SwarmProtocolMessage { .. } => "SwarmProtocolMessage",
        Event::LeaseGranted { .. } => "LeaseGranted",
        Event::LeaseExpired { .. } => "LeaseExpired",
        Event::OwnershipConflict { .. } => "OwnershipConflict",
        Event::AgentGcsLost { .. } => "AgentGcsLost",
        Event::AgentGcsReconnected { .. } => "AgentGcsReconnected",
        Event::AgentContinuingUnderLease { .. } => "AgentContinuingUnderLease",
        Event::AgentLeaseExpiredDuringGcsLoss { .. } => "AgentLeaseExpiredDuringGcsLoss",
        Event::AgentNeighborLost { .. } => "AgentNeighborLost",
        Event::AgentStateReconciled { .. } => "AgentStateReconciled",
        Event::PartitionDetected { .. } => "PartitionDetected",
        Event::PartitionHealed { .. } => "PartitionHealed",
        Event::SupervisorDegradedDecision { .. } => "SupervisorDegradedDecision",
        Event::SupervisorReconciled { .. } => "SupervisorReconciled",
        Event::CommandSuppressed { .. } => "CommandSuppressed",
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
        Event::CbbaBundleUpdated {
            bundle_size,
            conflict_count,
            ..
        } => format!("bundle_size={bundle_size} conflict_count={conflict_count}"),
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
        Event::WildfirePriorityReallocationRequested {
            task_id,
            old_priority,
            new_priority,
            previous_agent_id,
            ..
        } => format!(
            "task={task_id} old_priority={old_priority} new_priority={new_priority} previous_agent={}",
            previous_agent_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "none".to_owned())
        ),
        Event::WildfirePriorityTaskReleased {
            task_id,
            old_priority,
            new_priority,
            previous_agent_id,
            ..
        } => format!(
            "task={task_id} old_priority={old_priority} new_priority={new_priority} previous_agent={}",
            previous_agent_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "none".to_owned())
        ),
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
        Event::UrbanEdgeBlocked { edge_id, reason, .. } => format!(
            "edge={edge_id} reason={}",
            optional_display(reason.as_ref())
        ),
        Event::UrbanEdgeUnblocked { edge_id, .. } => format!("edge={edge_id}"),
        Event::UrbanObstacleDetected {
            edge_id,
            lookahead_segments,
            ..
        } => format!("edge={edge_id} lookahead_segments={lookahead_segments}"),
        Event::UrbanPolicyDecision {
            edge_id, policy, ..
        } => format!("edge={edge_id} policy={policy}"),
        Event::UrbanRouteReplanned {
            edge_ids,
            route_length_m,
            ..
        } => format!(
            "edges={} route_length_m={route_length_m:.3}",
            edge_ids.len()
        ),
        Event::UrbanWaitStarted { edge_id, .. } => format!("edge={edge_id}"),
        Event::UrbanWaitCompleted {
            edge_id,
            waited_ticks,
            ..
        } => format!("edge={edge_id} waited_ticks={waited_ticks}"),
        Event::UrbanNoRouteAvailable {
            from,
            to,
            reason,
            ..
        } => format!("from={from} to={to} reason={reason}"),
        Event::UrbanSegmentLockAcquired {
            edge_id,
            policy,
            reason,
            ..
        } => format!("edge={edge_id} policy={policy:?} reason={reason}"),
        Event::UrbanSegmentLockReleased {
            edge_id,
            held_ticks,
            ..
        } => format!("edge={edge_id} held_ticks={held_ticks}"),
        Event::UrbanSegmentConflict {
            edge_id,
            holder_agent_id,
            requester_agent_id,
            policy,
            reason,
            ..
        } => format!(
            "edge={edge_id} holder={holder_agent_id} requester={requester_agent_id} policy={policy:?} reason={reason}"
        ),
        Event::UrbanDeconflictWait {
            edge_id, reason, ..
        } => format!("edge={edge_id} reason={reason}"),
        Event::UrbanDeconflictReplan {
            edge_id,
            edge_ids,
            route_length_m,
            reason,
            ..
        } => format!(
            "edge={edge_id} replacement_edges={} route_length_m={route_length_m:.3} reason={reason}",
            edge_ids.len()
        ),
        Event::UrbanDeconflictAbort {
            edge_id, reason, ..
        } => format!("edge={edge_id} reason={reason}"),
        Event::UrbanSegmentCoordinatorEvent {
            edge_id,
            event,
            reason,
            ..
        } => format!(
            "edge={edge_id} event={event} reason={}",
            optional_display(reason.as_ref())
        ),
        Event::SwarmCommandPlanDispatched {
            plan_id,
            agent_count,
            ..
        } => format!("plan={plan_id} agent_count={agent_count}"),
        Event::SwarmAgentCommandDispatched {
            plan_id,
            agent_id,
            command_count,
            ..
        } => format!("plan={plan_id} agent={agent_id} command_count={command_count}"),
        Event::SwarmOwnershipAcquired {
            agent_id,
            ownership_kind,
            resource_id,
            reason,
            ..
        } => format!(
            "agent={agent_id} kind={ownership_kind} resource={resource_id} reason={reason}"
        ),
        Event::SwarmOwnershipReleased {
            agent_id,
            ownership_kind,
            resource_id,
            reason,
            ..
        } => format!(
            "agent={agent_id} kind={ownership_kind} resource={resource_id} reason={reason}"
        ),
        Event::SwarmOwnershipHandoff {
            from_agent_id,
            to_agent_id,
            ownership_kind,
            resource_id,
            reason,
            ..
        } => format!(
            "from={from_agent_id} to={to_agent_id} kind={ownership_kind} resource={resource_id} reason={reason}"
        ),
        Event::SwarmSupervisorStateChanged {
            from, to, reason, ..
        } => format!("from={from} to={to} reason={reason}"),
        Event::SwarmSyncCommandIssued {
            kind, agent_ids, ..
        } => format!("kind={kind} agents={}", agent_ids.len()),
        Event::SwarmSyncCommandResult {
            kind,
            succeeded_agent_ids,
            failed_agent_ids,
            timed_out_agent_ids,
            partial_success,
            ..
        } => format!(
            "kind={kind} succeeded={} failed={} timed_out={} partial_success={partial_success}",
            succeeded_agent_ids.len(),
            failed_agent_ids.len(),
            timed_out_agent_ids.len()
        ),
        Event::SwarmTopologyConfigured {
            topology_kind,
            node_count,
            link_count,
            ..
        } => format!("topology={topology_kind} nodes={node_count} links={link_count}"),
        Event::SwarmCommandRouteSelected {
            route_id,
            from_node_id,
            to_agent_id,
            via_node_ids,
            degraded,
            ..
        } => format!(
            "route={route_id} from={from_node_id} to={to_agent_id} hops={} degraded={degraded}",
            via_node_ids.len()
        ),
        Event::SwarmCommandRouteBlocked {
            route_id,
            from_node_id,
            to_agent_id,
            reason,
            ..
        } => format!("route={route_id} from={from_node_id} to={to_agent_id} reason={reason}"),
        Event::SwarmTopologyDegraded {
            topology_kind,
            affected_agent_ids,
            reason,
            ..
        } => format!(
            "topology={topology_kind} affected_agents={} reason={reason}",
            affected_agent_ids.len()
        ),
        Event::SwarmMothershipDependencyRecorded {
            parent_agent_id,
            child_agent_id,
            dependency_kind,
            ..
        } => format!(
            "parent={parent_agent_id} child={child_agent_id} dependency={dependency_kind}"
        ),
        Event::SwarmProtocolMessage {
            from,
            to,
            envelope_id,
            kind,
            ..
        } => format!("from={from} to={to} envelope={envelope_id} kind={kind}"),
        Event::LeaseGranted {
            lease_id,
            holder,
            resource_id,
            expires_at_tick,
            ..
        } => format!(
            "lease={lease_id} holder={holder} resource={resource_id} expires_at_tick={expires_at_tick}"
        ),
        Event::LeaseExpired {
            lease_id,
            resource_id,
            ..
        } => format!("lease={lease_id} resource={resource_id}"),
        Event::OwnershipConflict {
            resource_id,
            claimant_a,
            claimant_b,
            ..
        } => format!("resource={resource_id} claimant_a={claimant_a} claimant_b={claimant_b}"),
        Event::AgentGcsLost {
            agent_id, policy, ..
        } => format!("agent={agent_id} policy={policy}"),
        Event::AgentGcsReconnected {
            agent_id,
            gcs_lost_ticks,
            ..
        } => format!("agent={agent_id} gcs_lost_ticks={gcs_lost_ticks}"),
        Event::AgentContinuingUnderLease {
            agent_id, lease_id, ..
        } => format!("agent={agent_id} lease={lease_id}"),
        Event::AgentLeaseExpiredDuringGcsLoss {
            agent_id,
            lease_id,
            policy_applied,
            ..
        } => format!("agent={agent_id} lease={lease_id} policy={policy_applied}"),
        Event::AgentNeighborLost {
            agent_id,
            lost_neighbor_id,
            ..
        } => format!("agent={agent_id} lost_neighbor={lost_neighbor_id}"),
        Event::AgentStateReconciled {
            agent_id,
            gcs_lost_ticks,
            policy_applied,
            ..
        } => format!("agent={agent_id} gcs_lost_ticks={gcs_lost_ticks} policy={policy_applied}"),
        Event::PartitionDetected {
            group_a, group_b, ..
        } => format!("group_a={} group_b={}", group_a.len(), group_b.len()),
        Event::PartitionHealed { .. } => "reconciled=true".to_owned(),
        Event::SupervisorDegradedDecision {
            condition,
            decision,
            resources,
            ..
        } => format!(
            "condition={condition:?} decision={decision:?} resources={}",
            resources.len()
        ),
        Event::SupervisorReconciled {
            result_summary, ..
        } => format!(
            "accepted={} rejected={} conflicts={}",
            result_summary.accepted.len(),
            result_summary.rejected.len(),
            result_summary.conflicts.len()
        ),
        Event::CommandSuppressed {
            resource_id, reason, ..
        } => format!("resource={resource_id} reason={reason}"),
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
