use swarm_types::{UrbanMap, UrbanPlannedRoute, UrbanViolation};

use super::geometry::{midpoint, segment_intersects_aabb};

/// Judge static validity of a planned Urban route.
pub fn judge_route(map: &UrbanMap, route: &UrbanPlannedRoute) -> Vec<UrbanViolation> {
    let mut violations = Vec::new();
    for segment in &route.segments {
        let Some(edge) = map.edge(&segment.edge_id) else {
            violations.push(UrbanViolation::MissingEdge {
                edge_id: segment.edge_id.clone(),
            });
            continue;
        };
        if edge.blocked {
            violations.push(UrbanViolation::BlockedEdge {
                edge_id: edge.id.clone(),
            });
        }
        let Some(from) = map.node(&segment.from).map(|node| node.pose) else {
            continue;
        };
        let Some(to) = map.node(&segment.to).map(|node| node.pose) else {
            continue;
        };
        for obstacle in &map.static_obstacles {
            if obstacle.bounds.contains(&from)
                || obstacle.bounds.contains(&to)
                || segment_intersects_aabb(from, to, obstacle.bounds)
            {
                violations.push(UrbanViolation::ObstacleIntersection {
                    edge_id: edge.id.clone(),
                    obstacle_id: obstacle.id.clone(),
                    location: midpoint(from, to),
                });
            }
        }
    }
    violations
}
