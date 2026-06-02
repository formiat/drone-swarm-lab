use std::collections::{HashMap, HashSet};

use super::{
    push_segment_entered, Agent, AgentId, Coordinator, Health, Scenario, TaskId, UrbanMap,
    UrbanPlannedRoute, UrbanState,
};

pub(super) fn compute_urban_foundation_metrics(
    urban_state: &Option<UrbanState>,
) -> (bool, f64, f64, u64) {
    let Some(urban_state) = urban_state else {
        return (false, 0.0, 0.0, 0);
    };
    match crate::urban::expand_route_loop_with_planner_name(
        &urban_state.map,
        &urban_state.route_loop,
        &urban_state.planner,
    ) {
        Ok(route) => {
            let violations = crate::urban::judge_route(&urban_state.map, &route);
            (
                true,
                route.total_length_m,
                crate::urban::route_risk_score(&urban_state.map, &route),
                violations.len() as u64,
            )
        }
        Err(_) => (false, 0.0, 0.0, 1),
    }
}

pub(super) fn speed_m_per_tick(agent: &Agent, tick_duration_ms: u64) -> f64 {
    let tick_seconds = tick_duration_ms as f64 / 1000.0;
    if tick_seconds.is_finite() && tick_seconds > 0.0 && agent.speed.is_finite() {
        (agent.speed * tick_seconds).max(0.0)
    } else {
        0.0
    }
}

pub(super) fn route_efficiency(route_length_m: f64, distance_travelled_m: f64) -> f64 {
    if distance_travelled_m > 0.0 {
        route_length_m / distance_travelled_m
    } else {
        0.0
    }
}

pub(super) fn current_urban_pose(
    map: &UrbanMap,
    route: &UrbanPlannedRoute,
    segment_index: usize,
    distance_on_segment: f64,
    completed: bool,
) -> Option<swarm_types::Pose> {
    if completed {
        return route
            .segments
            .last()
            .and_then(|segment| map.node(&segment.to).map(|node| node.pose));
    }
    route.segments.get(segment_index).and_then(|segment| {
        crate::urban::pose_along_segment(map, segment, distance_on_segment).ok()
    })
}

pub(super) struct UrbanAnalysisAgentState {
    pub(super) agent_id: AgentId,
    pub(super) offset: swarm_types::Pose,
    pub(super) speed_m_per_tick: f64,
    pub(super) segment_index: usize,
    pub(super) distance_on_segment: f64,
    pub(super) completed: bool,
    pub(super) total_distance_travelled_m: f64,
}

pub(super) fn urban_analysis_agent_states(
    scenario: &Scenario,
    primary_agent_id: &AgentId,
    start_pose: swarm_types::Pose,
    tick_duration_ms: u64,
) -> Vec<UrbanAnalysisAgentState> {
    scenario
        .agents
        .iter()
        .filter(|agent| agent.health == Health::Alive && &agent.id != primary_agent_id)
        .map(|agent| UrbanAnalysisAgentState {
            agent_id: agent.id.clone(),
            offset: swarm_types::Pose {
                x: agent.pose.x - start_pose.x,
                y: agent.pose.y - start_pose.y,
                z: agent.pose.z - start_pose.z,
            },
            speed_m_per_tick: speed_m_per_tick(agent, tick_duration_ms),
            segment_index: 0,
            distance_on_segment: 0.0,
            completed: false,
            total_distance_travelled_m: 0.0,
        })
        .collect()
}

pub(super) fn advance_urban_analysis_agent(
    builder: &mut swarm_replay::EventLogBuilder,
    state: &mut UrbanAnalysisAgentState,
    map: &UrbanMap,
    route: &UrbanPlannedRoute,
    tick: u64,
) {
    if state.completed {
        return;
    }

    let mut remaining = state.speed_m_per_tick;
    while remaining > 0.0 && state.segment_index < route.segments.len() {
        let segment = &route.segments[state.segment_index];
        let segment_remaining = (segment.length_m - state.distance_on_segment).max(0.0);
        if remaining + f64::EPSILON >= segment_remaining {
            state.total_distance_travelled_m += segment_remaining;
            remaining -= segment_remaining;
            state.distance_on_segment = segment.length_m;
            builder.push(swarm_replay::Event::UrbanSegmentCompleted {
                agent_id: state.agent_id.clone(),
                tick,
                segment_index: state.segment_index,
                edge_id: segment.edge_id.clone(),
            });
            state.segment_index += 1;
            if state.segment_index == route.segments.len() {
                state.completed = true;
                builder.push(swarm_replay::Event::UrbanPatrolCompleted {
                    agent_id: state.agent_id.clone(),
                    tick,
                    route_length_m: route.total_length_m,
                    distance_travelled_m: state.total_distance_travelled_m,
                });
                break;
            }
            state.distance_on_segment = 0.0;
            push_segment_entered(
                builder,
                &state.agent_id,
                tick,
                state.segment_index,
                &route.segments[state.segment_index],
            );
        } else {
            state.distance_on_segment += remaining;
            state.total_distance_travelled_m += remaining;
            remaining = 0.0;
        }
    }

    if let Some(pose) = current_urban_pose(
        map,
        route,
        state.segment_index,
        state.distance_on_segment,
        state.completed,
    ) {
        builder.push(swarm_replay::Event::PoseUpdated {
            agent_id: state.agent_id.clone(),
            pose: offset_urban_analysis_pose(pose, state),
            tick,
        });
    }
}

pub(super) fn offset_urban_analysis_pose(
    pose: swarm_types::Pose,
    state: &UrbanAnalysisAgentState,
) -> swarm_types::Pose {
    swarm_types::Pose {
        x: pose.x + state.offset.x,
        y: pose.y + state.offset.y,
        z: pose.z + state.offset.z,
    }
}

pub(super) fn update_unassigned_durations(
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

pub(super) fn released_tasks_reassigned(
    coordinator: &Coordinator,
    released_tasks: &[TaskId],
) -> bool {
    released_tasks.iter().all(|released_task| {
        coordinator
            .registry
            .tasks()
            .any(|task| &task.id == released_task && task.assigned_to.is_some())
    })
}
