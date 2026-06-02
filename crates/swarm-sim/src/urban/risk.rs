use swarm_types::{UrbanEdge, UrbanMap, UrbanPlannedRoute};

use super::geometry::segment_aabb_clearance;

const CORRIDOR_NEUTRAL_WIDTH_M: f64 = 6.0;
const CLEARANCE_NEUTRAL_M: f64 = 8.0;

/// Compute an additive route risk proxy from corridor width and obstacle clearance.
pub fn route_risk_score(map: &UrbanMap, route: &UrbanPlannedRoute) -> f64 {
    route
        .segments
        .iter()
        .filter_map(|segment| map.edge(&segment.edge_id))
        .map(|edge| edge_risk_score(map, edge))
        .sum()
}

pub(super) fn edge_risk_score(map: &UrbanMap, edge: &UrbanEdge) -> f64 {
    let width_penalty = match edge.corridor_width_m {
        Some(width) if width > 0.0 => CORRIDOR_NEUTRAL_WIDTH_M / width,
        Some(_) => CORRIDOR_NEUTRAL_WIDTH_M,
        None => 1.0,
    };
    let clearance_penalty = edge_clearance_m(map, edge)
        .map(|clearance| ((CLEARANCE_NEUTRAL_M - clearance).max(0.0)) / CLEARANCE_NEUTRAL_M)
        .unwrap_or(0.0);
    edge.length_m * (width_penalty + clearance_penalty)
}

fn edge_clearance_m(map: &UrbanMap, edge: &UrbanEdge) -> Option<f64> {
    let from = map.node(&edge.from).map(|node| node.pose)?;
    let to = map.node(&edge.to).map(|node| node.pose)?;
    map.static_obstacles
        .iter()
        .map(|obstacle| segment_aabb_clearance(from, to, obstacle.bounds))
        .min_by(|left, right| left.total_cmp(right))
}
