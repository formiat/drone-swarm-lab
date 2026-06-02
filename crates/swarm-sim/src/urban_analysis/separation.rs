use std::collections::BTreeMap;

use swarm_types::Pose;

use super::{
    UrbanAgentRouteTrace, UrbanRouteConflict, UrbanRouteTrace, UrbanSeparationSummary,
    UrbanTraceSegment,
};

/// Measure pairwise separation conflicts from route trace pose samples.
pub fn measure_urban_separation(
    trace: &UrbanRouteTrace,
    threshold_m: f64,
) -> UrbanSeparationSummary {
    let mut min_separation_m: Option<f64> = None;
    let mut conflicts = Vec::new();
    let mut poses_by_tick: BTreeMap<u64, Vec<(&UrbanAgentRouteTrace, Pose)>> = BTreeMap::new();

    for agent in &trace.agents {
        for point in &agent.pose_trace {
            poses_by_tick
                .entry(point.tick)
                .or_default()
                .push((agent, point.pose));
        }
    }

    for (tick, mut poses) in poses_by_tick {
        poses.sort_by(|a, b| a.0.agent_id.as_ref().cmp(b.0.agent_id.as_ref()));
        for left_index in 0..poses.len() {
            for right_index in (left_index + 1)..poses.len() {
                let (agent_a, pose_a) = poses[left_index];
                let (agent_b, pose_b) = poses[right_index];
                let distance_m = pose_a.distance_to(&pose_b);
                min_separation_m = Some(
                    min_separation_m
                        .map(|current| current.min(distance_m))
                        .unwrap_or(distance_m),
                );
                if distance_m < threshold_m {
                    let segment_a = active_segment(agent_a, tick);
                    let segment_b = active_segment(agent_b, tick);
                    conflicts.push(UrbanRouteConflict {
                        agent_a: agent_a.agent_id.clone(),
                        agent_b: agent_b.agent_id.clone(),
                        tick,
                        distance_m,
                        segment_index_a: segment_a.map(|segment| segment.segment_index),
                        edge_id_a: segment_a.map(|segment| segment.edge_id.clone()),
                        segment_index_b: segment_b.map(|segment| segment.segment_index),
                        edge_id_b: segment_b.map(|segment| segment.edge_id.clone()),
                    });
                }
            }
        }
    }

    conflicts.sort_by(|a, b| {
        a.tick
            .cmp(&b.tick)
            .then_with(|| a.agent_a.as_ref().cmp(b.agent_a.as_ref()))
            .then_with(|| a.agent_b.as_ref().cmp(b.agent_b.as_ref()))
    });

    UrbanSeparationSummary {
        threshold_m,
        min_separation_m,
        separation_violation_count: conflicts.len() as u64,
        route_conflict_count: conflicts.len() as u64,
        conflicts,
    }
}
fn active_segment(agent: &UrbanAgentRouteTrace, tick: u64) -> Option<&UrbanTraceSegment> {
    agent
        .segments
        .iter()
        .filter(|segment| {
            segment.entered_tick.is_some_and(|entered| entered <= tick)
                && segment
                    .completed_tick
                    .is_none_or(|completed| tick <= completed)
        })
        .max_by_key(|segment| segment.entered_tick)
}
