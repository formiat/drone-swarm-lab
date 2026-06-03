use swarm_replay::{Event, EventLog};
use swarm_types::{AgentId, UrbanEdgeId};

use super::{
    count_urban_events, UrbanAgentRouteTrace, UrbanPoseTracePoint, UrbanRouteTrace,
    UrbanSegmentStatus, UrbanTraceSegment,
};

/// Reconstruct a route trace from Urban events in a replay log.
pub fn build_urban_route_trace(log: &EventLog) -> UrbanRouteTrace {
    let mut agents = Vec::new();

    for event in &log.events {
        match event {
            Event::UrbanRoutePlanned {
                agent_id,
                edge_ids,
                route_length_m,
                ..
            } => {
                let agent = agent_trace_mut(&mut agents, agent_id);
                agent.planned_edge_ids = edge_ids.clone();
                agent.route_length_m = *route_length_m;
                for (segment_index, edge_id) in edge_ids.iter().enumerate() {
                    let segment = segment_mut(agent, segment_index, edge_id);
                    segment.status = UrbanSegmentStatus::Planned;
                }
            }
            Event::UrbanSegmentEntered {
                agent_id,
                tick,
                segment_index,
                edge_id,
                from,
                to,
            } => {
                let segment = segment_mut(
                    agent_trace_mut(&mut agents, agent_id),
                    *segment_index,
                    edge_id,
                );
                segment.from = Some(from.clone());
                segment.to = Some(to.clone());
                segment.entered_tick = Some(*tick);
            }
            Event::UrbanSegmentCompleted {
                agent_id,
                tick,
                segment_index,
                edge_id,
            } => {
                let segment = segment_mut(
                    agent_trace_mut(&mut agents, agent_id),
                    *segment_index,
                    edge_id,
                );
                segment.completed_tick = Some(*tick);
            }
            Event::UrbanViolation {
                agent_id,
                tick,
                segment_index,
                edge_id,
                ..
            } => {
                if let Some(segment) = find_violation_segment(
                    agent_trace_mut(&mut agents, agent_id),
                    *segment_index,
                    edge_id.as_ref(),
                ) {
                    segment.violation_ticks.push(*tick);
                }
            }
            Event::PoseUpdated {
                agent_id,
                tick,
                pose,
            } => {
                agent_trace_mut(&mut agents, agent_id)
                    .pose_trace
                    .push(UrbanPoseTracePoint {
                        tick: *tick,
                        pose: *pose,
                    });
            }
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
            | Event::UrbanNoRouteAvailable { .. } => {}
        }
    }

    for agent in &mut agents {
        agent.segments.sort_by_key(|segment| segment.segment_index);
        for segment in &mut agent.segments {
            segment.status = if !segment.violation_ticks.is_empty() {
                UrbanSegmentStatus::Violated
            } else if segment.completed_tick.is_some() {
                UrbanSegmentStatus::Completed
            } else if segment.entered_tick.is_some() {
                UrbanSegmentStatus::Entered
            } else {
                UrbanSegmentStatus::NotCompleted
            };
        }
        agent.pose_trace.sort_by_key(|point| point.tick);
    }
    agents.sort_by(|a, b| a.agent_id.as_ref().cmp(b.agent_id.as_ref()));

    UrbanRouteTrace {
        run_id: log.run_id.clone(),
        scenario_name: log.scenario_name.clone(),
        seed: log.seed,
        agents,
        event_counts: count_urban_events(log),
    }
}
fn agent_trace_mut<'a>(
    agents: &'a mut Vec<UrbanAgentRouteTrace>,
    agent_id: &AgentId,
) -> &'a mut UrbanAgentRouteTrace {
    if let Some(index) = agents.iter().position(|agent| &agent.agent_id == agent_id) {
        return &mut agents[index];
    }
    agents.push(UrbanAgentRouteTrace {
        agent_id: agent_id.clone(),
        planned_edge_ids: Vec::new(),
        route_length_m: 0.0,
        segments: Vec::new(),
        pose_trace: Vec::new(),
    });
    agents.last_mut().expect("agent trace was just inserted")
}

fn segment_mut<'a>(
    agent: &'a mut UrbanAgentRouteTrace,
    segment_index: usize,
    edge_id: &UrbanEdgeId,
) -> &'a mut UrbanTraceSegment {
    if let Some(index) = agent
        .segments
        .iter()
        .position(|segment| segment.segment_index == segment_index)
    {
        return &mut agent.segments[index];
    }
    agent.segments.push(UrbanTraceSegment {
        segment_index,
        edge_id: edge_id.clone(),
        from: None,
        to: None,
        status: UrbanSegmentStatus::Planned,
        entered_tick: None,
        completed_tick: None,
        violation_ticks: Vec::new(),
    });
    agent
        .segments
        .last_mut()
        .expect("segment trace was just inserted")
}

fn find_violation_segment<'a>(
    agent: &'a mut UrbanAgentRouteTrace,
    segment_index: Option<usize>,
    edge_id: Option<&UrbanEdgeId>,
) -> Option<&'a mut UrbanTraceSegment> {
    if let Some(segment_index) = segment_index {
        if let Some(index) = agent
            .segments
            .iter()
            .position(|segment| segment.segment_index == segment_index)
        {
            return Some(&mut agent.segments[index]);
        }
    }
    let edge_id = edge_id?;
    let index = agent
        .segments
        .iter()
        .position(|segment| &segment.edge_id == edge_id)?;
    Some(&mut agent.segments[index])
}
