use std::collections::{HashMap, HashSet, VecDeque};

use swarm_types::{AgentId, Health, Pose};

/// A snapshot of the network topology used for connectivity analysis.
#[derive(Clone, Debug)]
pub struct ConnectivitySnapshot {
    pub agent_entries: Vec<(AgentId, Pose, f64, Health)>, // id, pose, comms_range, health
    pub ground_nodes: Vec<(String, Pose, f64)>,           // id, pose, comms_range
    pub base_id: String,
    pub base_pose: Pose,
}

/// Models mesh connectivity using range-based links and BFS reachability.
pub struct ConnectivityModel;

impl ConnectivityModel {
    /// Returns true if two nodes are within direct communication range.
    pub fn direct_link(a: &Pose, range_a: f64, b: &Pose, range_b: f64) -> bool {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let distance = (dx * dx + dy * dy).sqrt();
        let min_range = range_a.min(range_b);
        // INFINITY means always in range
        if min_range.is_infinite() {
            return true;
        }
        distance <= min_range
    }

    /// Build an adjacency list representing the connectivity graph.
    pub fn build_adjacency(snapshot: &ConnectivitySnapshot) -> HashMap<String, Vec<String>> {
        let mut nodes: Vec<(String, Pose, f64)> = Vec::new();

        // Add base station
        nodes.push((snapshot.base_id.clone(), snapshot.base_pose, f64::INFINITY));

        // Add alive agents
        for (id, pose, range, health) in &snapshot.agent_entries {
            if *health == Health::Alive {
                nodes.push((id.to_string(), *pose, *range));
            }
        }

        // Add ground nodes
        for (id, pose, range) in &snapshot.ground_nodes {
            nodes.push((id.clone(), *pose, *range));
        }

        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                let (id_a, pose_a, range_a) = &nodes[i];
                let (id_b, pose_b, range_b) = &nodes[j];
                if Self::direct_link(pose_a, *range_a, pose_b, *range_b) {
                    adjacency
                        .entry(id_a.clone())
                        .or_default()
                        .push(id_b.clone());
                    adjacency
                        .entry(id_b.clone())
                        .or_default()
                        .push(id_a.clone());
                }
            }
        }

        adjacency
    }

    /// BFS from base to all reachable nodes. Returns map of node_id -> hop_count.
    pub fn reachability_from_base(snapshot: &ConnectivitySnapshot) -> HashMap<String, usize> {
        let adjacency = Self::build_adjacency(snapshot);
        let mut visited = HashMap::new();
        let mut queue = VecDeque::new();

        queue.push_back((snapshot.base_id.clone(), 0usize));
        visited.insert(snapshot.base_id.clone(), 0usize);

        while let Some((current_id, hops)) = queue.pop_front() {
            if let Some(neighbors) = adjacency.get(&current_id) {
                for neighbor in neighbors {
                    if !visited.contains_key(neighbor) {
                        visited.insert(neighbor.clone(), hops + 1);
                        queue.push_back((neighbor.clone(), hops + 1));
                    }
                }
            }
        }

        visited
    }

    /// Fraction of agents reachable from base.
    pub fn availability_fraction(
        reachability: &HashMap<String, usize>,
        agent_ids: &[AgentId],
    ) -> f64 {
        if agent_ids.is_empty() {
            return 1.0;
        }
        let reachable_count = agent_ids
            .iter()
            .filter(|id| reachability.contains_key(id.as_ref()))
            .count();
        reachable_count as f64 / agent_ids.len() as f64
    }

    /// BFS from `from_id` to `to_id`. Returns hop count if reachable.
    pub fn hop_count_between(
        snapshot: &ConnectivitySnapshot,
        from_id: &str,
        to_id: &str,
    ) -> Option<usize> {
        let adjacency = Self::build_adjacency(snapshot);
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((from_id.to_owned(), 0usize));
        visited.insert(from_id.to_owned());

        while let Some((current_id, hops)) = queue.pop_front() {
            if current_id == to_id {
                return Some(hops);
            }
            if let Some(neighbors) = adjacency.get(&current_id) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        visited.insert(neighbor.clone());
                        queue.push_back((neighbor.clone(), hops + 1));
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pose(x: f64, y: f64) -> Pose {
        Pose { x, y , ..Default::default()}
    }

    fn snapshot_with_agents(agents: Vec<(AgentId, Pose, f64, Health)>) -> ConnectivitySnapshot {
        ConnectivitySnapshot {
            agent_entries: agents,
            ground_nodes: vec![],
            base_id: "base".to_owned(),
            base_pose: pose(0.0, 0.0),
        }
    }

    #[test]
    fn direct_link_within_range() {
        let a = pose(0.0, 0.0);
        let b = pose(3.0, 4.0); // distance = 5
        assert!(ConnectivityModel::direct_link(&a, 10.0, &b, 10.0));
    }

    #[test]
    fn direct_link_beyond_range() {
        let a = pose(0.0, 0.0);
        let b = pose(10.0, 0.0); // distance = 10
        assert!(!ConnectivityModel::direct_link(&a, 5.0, &b, 5.0));
    }

    #[test]
    fn direct_link_infinity_always_true() {
        let a = pose(0.0, 0.0);
        let b = pose(1000.0, 1000.0);
        assert!(ConnectivityModel::direct_link(
            &a,
            f64::INFINITY,
            &b,
            f64::INFINITY
        ));
    }

    #[test]
    fn mesh_reachability_via_relay() {
        // Base at (0,0), relay at (5,0), scout at (10,0)
        // Range = 6 for all
        let agents = vec![
            (
                AgentId::from("relay".to_owned()),
                pose(5.0, 0.0),
                6.0,
                Health::Alive,
            ),
            (
                AgentId::from("scout".to_owned()),
                pose(10.0, 0.0),
                6.0,
                Health::Alive,
            ),
        ];
        let snapshot = snapshot_with_agents(agents);
        let reachability = ConnectivityModel::reachability_from_base(&snapshot);

        assert!(reachability.contains_key("base"));
        assert!(reachability.contains_key("relay"));
        assert!(reachability.contains_key("scout"));
        assert_eq!(reachability["scout"], 2); // base -> relay -> scout
    }

    #[test]
    fn mesh_unreachable_without_relay() {
        // Base at (0,0), scout at (10,0)
        // Range = 5 for all -> scout unreachable
        let agents = vec![(
            AgentId::from("scout".to_owned()),
            pose(10.0, 0.0),
            5.0,
            Health::Alive,
        )];
        let snapshot = snapshot_with_agents(agents);
        let reachability = ConnectivityModel::reachability_from_base(&snapshot);

        assert!(reachability.contains_key("base"));
        assert!(!reachability.contains_key("scout"));
    }

    #[test]
    fn hop_count_two_hops() {
        // Base -> relay1 -> relay2 -> scout
        let agents = vec![
            (
                AgentId::from("r1".to_owned()),
                pose(5.0, 0.0),
                6.0,
                Health::Alive,
            ),
            (
                AgentId::from("r2".to_owned()),
                pose(10.0, 0.0),
                6.0,
                Health::Alive,
            ),
            (
                AgentId::from("scout".to_owned()),
                pose(15.0, 0.0),
                6.0,
                Health::Alive,
            ),
        ];
        let snapshot = snapshot_with_agents(agents);
        let reachability = ConnectivityModel::reachability_from_base(&snapshot);

        assert_eq!(reachability["r1"], 1);
        assert_eq!(reachability["r2"], 2);
        assert_eq!(reachability["scout"], 3);
    }

    #[test]
    fn availability_all_reachable() {
        let agents = vec![
            (
                AgentId::from("a1".to_owned()),
                pose(3.0, 0.0),
                10.0,
                Health::Alive,
            ),
            (
                AgentId::from("a2".to_owned()),
                pose(6.0, 0.0),
                10.0,
                Health::Alive,
            ),
        ];
        let snapshot = snapshot_with_agents(agents);
        let reachability = ConnectivityModel::reachability_from_base(&snapshot);
        let agent_ids: Vec<AgentId> = vec![
            AgentId::from("a1".to_owned()),
            AgentId::from("a2".to_owned()),
        ];
        assert_eq!(
            ConnectivityModel::availability_fraction(&reachability, &agent_ids),
            1.0
        );
    }

    #[test]
    fn availability_half_reachable() {
        let agents = vec![
            (
                AgentId::from("a1".to_owned()),
                pose(3.0, 0.0),
                10.0,
                Health::Alive,
            ),
            (
                AgentId::from("a2".to_owned()),
                pose(100.0, 0.0),
                5.0,
                Health::Alive,
            ),
        ];
        let snapshot = snapshot_with_agents(agents);
        let reachability = ConnectivityModel::reachability_from_base(&snapshot);
        let agent_ids: Vec<AgentId> = vec![
            AgentId::from("a1".to_owned()),
            AgentId::from("a2".to_owned()),
        ];
        assert_eq!(
            ConnectivityModel::availability_fraction(&reachability, &agent_ids),
            0.5
        );
    }

    #[test]
    fn dead_agent_excluded_from_graph() {
        let agents = vec![
            (
                AgentId::from("relay".to_owned()),
                pose(5.0, 0.0),
                6.0,
                Health::Dead,
            ),
            (
                AgentId::from("scout".to_owned()),
                pose(10.0, 0.0),
                6.0,
                Health::Alive,
            ),
        ];
        let snapshot = snapshot_with_agents(agents);
        let reachability = ConnectivityModel::reachability_from_base(&snapshot);

        assert!(!reachability.contains_key("relay"));
        assert!(!reachability.contains_key("scout")); // unreachable because relay is dead
    }

    #[test]
    fn ground_nodes_participate_in_mesh() {
        let agents = vec![(
            AgentId::from("scout".to_owned()),
            pose(10.0, 0.0),
            6.0,
            Health::Alive,
        )];
        let snapshot = ConnectivitySnapshot {
            agent_entries: agents,
            ground_nodes: vec![("gn1".to_owned(), pose(5.0, 0.0), 6.0)],
            base_id: "base".to_owned(),
            base_pose: pose(0.0, 0.0),
        };
        let reachability = ConnectivityModel::reachability_from_base(&snapshot);

        assert!(reachability.contains_key("gn1"));
        assert!(reachability.contains_key("scout"));
    }
}
