use derive_more::{AsRef, Deref, DerefMut, Display, From, Into};
use serde::{Deserialize, Serialize};

use crate::pose::Pose;

/// Unique identifier for an inspection edge.
#[derive(
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct EdgeId(String);

/// A single edge to be inspected (e.g. power line segment, pipe segment).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InspectionEdge {
    pub id: EdgeId,
    pub from: Pose,
    pub to: Pose,
    pub length_m: f64,
    pub priority: u8,
}

/// Graph of edges representing linear infrastructure to inspect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InspectionGraph {
    pub edges: Vec<InspectionEdge>,
    pub depot: Pose,
}

impl InspectionGraph {
    /// Create a straight line along the X axis starting at (0,0).
    pub fn linear_route(n_segments: u32, segment_length_m: f64) -> Self {
        let mut edges = Vec::new();
        for i in 0..n_segments {
            let from = Pose {
                x: i as f64 * segment_length_m,
                y: 0.0,
                ..Default::default()
            };
            let to = Pose {
                x: (i + 1) as f64 * segment_length_m,
                y: 0.0,
                ..Default::default()
            };
            edges.push(InspectionEdge {
                id: EdgeId::from(format!("edge-{i}")),
                from,
                to,
                length_m: segment_length_m,
                priority: 1,
            });
        }
        Self {
            edges,
            depot: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
        }
    }

    /// Create a closed perimeter of a width x height grid.
    pub fn grid_perimeter(width: u32, height: u32, cell_size_m: f64) -> Self {
        let mut edges = Vec::new();
        let w = width as f64 * cell_size_m;
        let h = height as f64 * cell_size_m;

        // Bottom edge: (0,0) -> (w,0)
        for i in 0..width {
            let x0 = i as f64 * cell_size_m;
            let x1 = (i + 1) as f64 * cell_size_m;
            edges.push(InspectionEdge {
                id: EdgeId::from(format!("edge-bottom-{i}")),
                from: Pose {
                    x: x0,
                    y: 0.0,
                    ..Default::default()
                },
                to: Pose {
                    x: x1,
                    y: 0.0,
                    ..Default::default()
                },
                length_m: cell_size_m,
                priority: 1,
            });
        }

        // Right edge: (w,0) -> (w,h)
        for i in 0..height {
            let y0 = i as f64 * cell_size_m;
            let y1 = (i + 1) as f64 * cell_size_m;
            edges.push(InspectionEdge {
                id: EdgeId::from(format!("edge-right-{i}")),
                from: Pose {
                    x: w,
                    y: y0,
                    ..Default::default()
                },
                to: Pose {
                    x: w,
                    y: y1,
                    ..Default::default()
                },
                length_m: cell_size_m,
                priority: 1,
            });
        }

        // Top edge: (w,h) -> (0,h)
        for i in 0..width {
            let x0 = w - i as f64 * cell_size_m;
            let x1 = w - (i + 1) as f64 * cell_size_m;
            edges.push(InspectionEdge {
                id: EdgeId::from(format!("edge-top-{i}")),
                from: Pose {
                    x: x0,
                    y: h,
                    ..Default::default()
                },
                to: Pose {
                    x: x1,
                    y: h,
                    ..Default::default()
                },
                length_m: cell_size_m,
                priority: 1,
            });
        }

        // Left edge: (0,h) -> (0,0)
        for i in 0..height {
            let y0 = h - i as f64 * cell_size_m;
            let y1 = h - (i + 1) as f64 * cell_size_m;
            edges.push(InspectionEdge {
                id: EdgeId::from(format!("edge-left-{i}")),
                from: Pose {
                    x: 0.0,
                    y: y0,
                    ..Default::default()
                },
                to: Pose {
                    x: 0.0,
                    y: y1,
                    ..Default::default()
                },
                length_m: cell_size_m,
                priority: 1,
            });
        }

        Self {
            edges,
            depot: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
        }
    }

    /// Create a random geometric graph in a 100x100 square.
    /// Nodes are placed randomly; edges connect pairs within 30 m.
    pub fn random_graph(n_nodes: u32, seed: u64) -> Self {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(seed);
        let mut nodes = Vec::new();
        for _ in 0..n_nodes {
            nodes.push(Pose {
                x: rng.gen::<f64>() * 100.0,
                y: rng.gen::<f64>() * 100.0,
                ..Default::default()
            });
        }

        let mut edges = Vec::new();
        let threshold = 30.0;
        let mut edge_idx = 0;
        for i in 0..n_nodes {
            for j in (i + 1)..n_nodes {
                let dist = nodes[i as usize].distance_to(&nodes[j as usize]);
                if dist < threshold {
                    edges.push(InspectionEdge {
                        id: EdgeId::from(format!("edge-{edge_idx}")),
                        from: nodes[i as usize],
                        to: nodes[j as usize],
                        length_m: dist,
                        priority: 1,
                    });
                    edge_idx += 1;
                }
            }
        }

        // Ensure graph is non-empty: if no edges, create a minimal path.
        if edges.is_empty() && n_nodes >= 2 {
            for i in 0..(n_nodes - 1) {
                let dist = nodes[i as usize].distance_to(&nodes[(i + 1) as usize]);
                edges.push(InspectionEdge {
                    id: EdgeId::from(format!("edge-{i}")),
                    from: nodes[i as usize],
                    to: nodes[(i + 1) as usize],
                    length_m: dist,
                    priority: 1,
                });
            }
        }

        Self {
            edges,
            depot: nodes.first().copied().unwrap_or(Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_route_n_edges() {
        let graph = InspectionGraph::linear_route(10, 10.0);
        assert_eq!(graph.edges.len(), 10);
    }

    #[test]
    fn linear_route_total_length() {
        let graph = InspectionGraph::linear_route(10, 10.0);
        let total: f64 = graph.edges.iter().map(|e| e.length_m).sum();
        assert!((total - 100.0).abs() < 1e-6);
    }

    #[test]
    fn grid_perimeter_closed() {
        let graph = InspectionGraph::grid_perimeter(10, 10, 10.0);
        let last = graph.edges.last().unwrap();
        assert!((last.to.x - 0.0).abs() < 1e-6);
        assert!((last.to.y - 0.0).abs() < 1e-6);
    }

    #[test]
    fn grid_perimeter_count() {
        let graph = InspectionGraph::grid_perimeter(10, 10, 10.0);
        assert_eq!(graph.edges.len(), 2 * (10 + 10));
    }

    #[test]
    fn random_graph_no_panic() {
        for n in 2..=50 {
            let graph = InspectionGraph::random_graph(n, 42);
            assert!(!graph.edges.is_empty());
        }
    }
}
