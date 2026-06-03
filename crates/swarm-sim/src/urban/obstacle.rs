use std::collections::HashSet;

use swarm_types::{UrbanEdgeId, UrbanMap, UrbanPlannedRoute, UrbanTemporaryObstacle};

/// Number of route segments the mock detector looks ahead for blocked edges.
pub const URBAN_BLOCKED_LOOKAHEAD_SEGMENTS: usize = 3;

/// Returns the set of edge IDs effectively blocked at `tick`.
///
/// Combines static map `blocked` flags with active temporary obstacles whose
/// severity is `Hard` or absent. `Soft` obstacles are advisory and are not
/// included in this set.
pub fn effective_blocked_edges(
    map: &UrbanMap,
    obstacles: &[UrbanTemporaryObstacle],
    tick: u64,
) -> HashSet<UrbanEdgeId> {
    let mut blocked: HashSet<UrbanEdgeId> = map
        .edges
        .iter()
        .filter(|e| e.blocked)
        .map(|e| e.id.clone())
        .collect();
    for obstacle in obstacles {
        if obstacle.is_active(tick) && obstacle.is_hard_block() {
            blocked.insert(obstacle.edge_id.clone());
        }
    }
    blocked
}

/// Returns the first `(segment_index, edge_id)` found blocked within `lookahead`
/// segments starting at `from_segment` in the given route.
///
/// Returns `None` if no blocked segment is found within the lookahead window.
pub fn detect_blocked_ahead(
    route: &UrbanPlannedRoute,
    from_segment: usize,
    blocked: &HashSet<UrbanEdgeId>,
    lookahead: usize,
) -> Option<(usize, UrbanEdgeId)> {
    let end = (from_segment + lookahead).min(route.segments.len());
    for idx in from_segment..end {
        let segment = &route.segments[idx];
        if blocked.contains(&segment.edge_id) {
            return Some((idx, segment.edge_id.clone()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use swarm_types::{
        ObstacleSeverity, UrbanEdge, UrbanNode, UrbanPlannedRoute, UrbanRouteSegment,
    };

    use super::*;

    fn make_edge_id(id: &str) -> UrbanEdgeId {
        UrbanEdgeId::from(id.to_owned())
    }

    fn make_node_id(id: &str) -> swarm_types::UrbanNodeId {
        swarm_types::UrbanNodeId::from(id.to_owned())
    }

    fn make_map(blocked_edge: Option<&str>) -> UrbanMap {
        UrbanMap {
            nodes: vec![
                UrbanNode {
                    id: make_node_id("n0"),
                    pose: swarm_types::Pose {
                        x: 0.0,
                        y: 0.0,
                        ..Default::default()
                    },
                },
                UrbanNode {
                    id: make_node_id("n1"),
                    pose: swarm_types::Pose {
                        x: 10.0,
                        y: 0.0,
                        ..Default::default()
                    },
                },
                UrbanNode {
                    id: make_node_id("n2"),
                    pose: swarm_types::Pose {
                        x: 20.0,
                        y: 0.0,
                        ..Default::default()
                    },
                },
            ],
            edges: vec![
                UrbanEdge {
                    id: make_edge_id("e0"),
                    from: make_node_id("n0"),
                    to: make_node_id("n1"),
                    cost: 10.0,
                    length_m: 10.0,
                    corridor_width_m: None,
                    blocked: blocked_edge == Some("e0"),
                },
                UrbanEdge {
                    id: make_edge_id("e1"),
                    from: make_node_id("n1"),
                    to: make_node_id("n2"),
                    cost: 10.0,
                    length_m: 10.0,
                    corridor_width_m: None,
                    blocked: blocked_edge == Some("e1"),
                },
            ],
            static_obstacles: vec![],
        }
    }

    fn make_route(edge_ids: &[&str]) -> UrbanPlannedRoute {
        let segments: Vec<UrbanRouteSegment> = edge_ids
            .iter()
            .map(|id| UrbanRouteSegment {
                edge_id: make_edge_id(id),
                from: make_node_id("n0"),
                to: make_node_id("n1"),
                length_m: 10.0,
                cost: 10.0,
            })
            .collect();
        let total_length_m = segments.iter().map(|s| s.length_m).sum();
        let total_cost = segments.iter().map(|s| s.cost).sum();
        UrbanPlannedRoute {
            segments,
            total_length_m,
            total_cost,
        }
    }

    fn obstacle(edge_id: &str, appears: u64, disappears: Option<u64>) -> UrbanTemporaryObstacle {
        UrbanTemporaryObstacle {
            edge_id: make_edge_id(edge_id),
            appears_at_tick: appears,
            disappears_at_tick: disappears,
            reason: None,
            severity: None,
        }
    }

    #[test]
    fn effective_blocked_edges_includes_static_blocked() {
        let map = make_map(Some("e0"));
        let result = effective_blocked_edges(&map, &[], 0);
        assert!(result.contains(&make_edge_id("e0")));
        assert!(!result.contains(&make_edge_id("e1")));
    }

    #[test]
    fn effective_blocked_edges_includes_active_obstacle() {
        let map = make_map(None);
        let obstacles = vec![obstacle("e1", 5, Some(10))];
        let result = effective_blocked_edges(&map, &obstacles, 7);
        assert!(result.contains(&make_edge_id("e1")));
    }

    #[test]
    fn effective_blocked_edges_excludes_inactive_obstacle() {
        let map = make_map(None);
        let obstacles = vec![obstacle("e1", 5, Some(10))];
        // before window
        assert!(!effective_blocked_edges(&map, &obstacles, 4).contains(&make_edge_id("e1")));
        // after window
        assert!(!effective_blocked_edges(&map, &obstacles, 10).contains(&make_edge_id("e1")));
    }

    #[test]
    fn detect_blocked_ahead_finds_blocked_within_lookahead() {
        let route = make_route(&["e0", "e1", "e0"]);
        let mut blocked = HashSet::new();
        blocked.insert(make_edge_id("e1"));
        let result = detect_blocked_ahead(&route, 0, &blocked, 3);
        assert_eq!(result, Some((1, make_edge_id("e1"))));
    }

    #[test]
    fn detect_blocked_ahead_returns_none_when_clear() {
        let route = make_route(&["e0", "e1"]);
        let blocked = HashSet::new();
        assert!(detect_blocked_ahead(&route, 0, &blocked, 3).is_none());
    }

    #[test]
    fn detect_blocked_ahead_respects_lookahead_limit() {
        let route = make_route(&["e0", "e0", "e1"]);
        let mut blocked = HashSet::new();
        blocked.insert(make_edge_id("e1"));
        // lookahead=2 should not see e1 at index 2
        assert!(detect_blocked_ahead(&route, 0, &blocked, 2).is_none());
        // lookahead=3 should see it
        assert!(detect_blocked_ahead(&route, 0, &blocked, 3).is_some());
    }

    #[test]
    fn judge_rejects_agent_on_hard_blocked_edge() {
        let map = make_map(None);

        // Hard obstacle (or absent severity) is included in the effective blocked set.
        let hard_obstacle = UrbanTemporaryObstacle {
            edge_id: make_edge_id("e0"),
            appears_at_tick: 0,
            disappears_at_tick: None,
            reason: None,
            severity: Some(ObstacleSeverity::Hard),
        };
        let blocked = effective_blocked_edges(&map, &[hard_obstacle], 5);
        assert!(
            blocked.contains(&make_edge_id("e0")),
            "hard obstacle must be in blocked set"
        );

        // Soft obstacle is advisory only — not in the blocked set.
        let soft_obstacle = UrbanTemporaryObstacle {
            edge_id: make_edge_id("e0"),
            appears_at_tick: 0,
            disappears_at_tick: None,
            reason: None,
            severity: Some(ObstacleSeverity::Soft),
        };
        let blocked_soft = effective_blocked_edges(&map, &[soft_obstacle], 5);
        assert!(
            !blocked_soft.contains(&make_edge_id("e0")),
            "soft obstacle must not be in blocked set"
        );

        // Absent severity defaults to Hard.
        let default_obstacle = UrbanTemporaryObstacle {
            edge_id: make_edge_id("e1"),
            appears_at_tick: 0,
            disappears_at_tick: None,
            reason: None,
            severity: None,
        };
        let blocked_default = effective_blocked_edges(&map, &[default_obstacle], 5);
        assert!(
            blocked_default.contains(&make_edge_id("e1")),
            "obstacle with absent severity must be treated as hard"
        );
    }
}
