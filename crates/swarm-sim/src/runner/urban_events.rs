use super::{
    current_urban_pose, offset_urban_analysis_pose, AgentId, UrbanBusId, UrbanMap,
    UrbanPlannedRoute, UrbanRouteSegment, UrbanViolation,
};
use crate::urban::{UrbanSegmentConflictRecord, UrbanSegmentLock};

pub(super) fn advance_search_segment(
    route: &UrbanPlannedRoute,
    segment_index: usize,
    tick: u64,
    agent_id: &AgentId,
    mut log_builder: Option<&mut swarm_replay::EventLogBuilder>,
) -> usize {
    let Some(segment) = route.segments.get(segment_index) else {
        return 0;
    };
    if let Some(ref mut builder) = log_builder {
        builder.push(swarm_replay::Event::UrbanSegmentCompleted {
            agent_id: agent_id.clone(),
            tick,
            segment_index,
            edge_id: segment.edge_id.clone(),
        });
    }

    let next_index = if segment_index + 1 == route.segments.len() {
        0
    } else {
        segment_index + 1
    };
    if let Some(next_segment) = route.segments.get(next_index) {
        if let Some(ref mut builder) = log_builder {
            push_segment_entered(builder, agent_id, tick, next_index, next_segment);
        }
    }
    next_index
}

pub(super) fn push_segment_entered(
    builder: &mut swarm_replay::EventLogBuilder,
    agent_id: &AgentId,
    tick: u64,
    segment_index: usize,
    segment: &UrbanRouteSegment,
) {
    builder.push(swarm_replay::Event::UrbanSegmentEntered {
        agent_id: agent_id.clone(),
        tick,
        segment_index,
        edge_id: segment.edge_id.clone(),
        from: segment.from.clone(),
        to: segment.to.clone(),
    });
}

pub(super) fn push_segment_lock_acquired(
    builder: &mut swarm_replay::EventLogBuilder,
    lock: &UrbanSegmentLock,
    policy: swarm_types::UrbanRightOfWayPolicy,
    reason: impl Into<String>,
) {
    builder.push(swarm_replay::Event::UrbanSegmentLockAcquired {
        agent_id: lock.holder_agent_id.clone(),
        tick: lock.acquired_at_tick,
        edge_id: lock.edge_id.clone(),
        policy,
        reason: reason.into(),
    });
}

pub(super) fn push_segment_lock_released(
    builder: &mut swarm_replay::EventLogBuilder,
    lock: &UrbanSegmentLock,
    tick: u64,
) {
    builder.push(swarm_replay::Event::UrbanSegmentLockReleased {
        agent_id: lock.holder_agent_id.clone(),
        tick,
        edge_id: lock.edge_id.clone(),
        held_ticks: tick.saturating_sub(lock.acquired_at_tick),
    });
}

pub(super) fn push_segment_conflict(
    builder: &mut swarm_replay::EventLogBuilder,
    conflict: &UrbanSegmentConflictRecord,
) {
    builder.push(swarm_replay::Event::UrbanSegmentConflict {
        tick: conflict.tick,
        edge_id: conflict.edge_id.clone(),
        holder_agent_id: conflict.holder_agent_id.clone(),
        requester_agent_id: conflict.requester_agent_id.clone(),
        policy: conflict.policy.clone(),
        reason: conflict.reason.clone(),
    });
}

pub(super) fn push_urban_violation_event(
    builder: &mut swarm_replay::EventLogBuilder,
    agent_id: &AgentId,
    tick: u64,
    route: &UrbanPlannedRoute,
    violation: &UrbanViolation,
) {
    let edge_id = match violation {
        UrbanViolation::MissingEdge { edge_id }
        | UrbanViolation::BlockedEdge { edge_id }
        | UrbanViolation::ObstacleIntersection { edge_id, .. } => Some(edge_id.clone()),
    };
    let obstacle_id = match violation {
        UrbanViolation::ObstacleIntersection { obstacle_id, .. } => Some(obstacle_id.clone()),
        UrbanViolation::MissingEdge { .. } | UrbanViolation::BlockedEdge { .. } => None,
    };
    let segment_index = edge_id.as_ref().and_then(|id| {
        route
            .segments
            .iter()
            .position(|segment| &segment.edge_id == id)
    });
    let pose = match violation {
        UrbanViolation::ObstacleIntersection { location, .. } => *location,
        UrbanViolation::MissingEdge { .. } | UrbanViolation::BlockedEdge { .. } => {
            swarm_types::Pose::default()
        }
    };
    builder.push(swarm_replay::Event::UrbanViolation {
        agent_id: agent_id.clone(),
        tick,
        segment_index,
        edge_id,
        obstacle_id,
        pose,
        reason: format!("{violation:?}"),
    });
}

pub(super) fn push_detection_events(
    builder: &mut swarm_replay::EventLogBuilder,
    agent_id: &AgentId,
    tick: u64,
    pose: swarm_types::Pose,
    detector_seed: u64,
    outcome: &crate::urban::UrbanDetectionOutcome,
) {
    for observation in &outcome.observations {
        builder.push(swarm_replay::Event::BusObserved {
            agent_id: agent_id.clone(),
            tick,
            bus_id: observation.bus_id.clone(),
            pose: observation.pose,
            distance_m: observation.distance_m,
            detector_seed,
        });
    }
    if let Some(detection) = &outcome.detection {
        builder.push(swarm_replay::Event::BusDetected {
            agent_id: agent_id.clone(),
            tick,
            bus_id: detection.bus_id.clone(),
            pose: detection.pose,
            distance_m: detection.distance_m,
            detector_seed,
        });
    }
    if outcome.false_positive {
        builder.push(swarm_replay::Event::BusFalsePositive {
            agent_id: agent_id.clone(),
            tick,
            pose,
            detector_seed,
        });
    }
}

pub(super) fn push_urban_search_completed(
    builder: &mut swarm_replay::EventLogBuilder,
    agent_id: &AgentId,
    tick: u64,
    detected: bool,
    bus_id: Option<UrbanBusId>,
    reason: &str,
    distance_travelled_m: f64,
) {
    builder.push(swarm_replay::Event::UrbanSearchCompleted {
        agent_id: agent_id.clone(),
        tick,
        detected,
        bus_id,
        reason: reason.to_owned(),
        distance_travelled_m,
    });
}

pub(super) fn push_urban_analysis_agent_started(
    builder: &mut swarm_replay::EventLogBuilder,
    state: &super::urban_helpers::UrbanAnalysisAgentState,
    map: &UrbanMap,
    route: &UrbanPlannedRoute,
) {
    builder.push(swarm_replay::Event::UrbanRoutePlanned {
        agent_id: state.agent_id.clone(),
        tick: 0,
        edge_ids: route
            .segments
            .iter()
            .map(|segment| segment.edge_id.clone())
            .collect(),
        route_length_m: route.total_length_m,
    });
    if let Some(pose) = current_urban_pose(map, route, 0, 0.0, false) {
        builder.push(swarm_replay::Event::PoseUpdated {
            agent_id: state.agent_id.clone(),
            pose: offset_urban_analysis_pose(pose, state),
            tick: 0,
        });
    }
    if let Some(first_segment) = route.segments.first() {
        push_segment_entered(builder, &state.agent_id, 0, 0, first_segment);
    } else {
        builder.push(swarm_replay::Event::UrbanPatrolCompleted {
            agent_id: state.agent_id.clone(),
            tick: 0,
            route_length_m: route.total_length_m,
            distance_travelled_m: 0.0,
        });
    }
}
