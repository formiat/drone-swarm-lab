use rand::Rng;
use serde::{Deserialize, Serialize};

use swarm_types::{AgentId, BeliefMap, CellState, HiddenTarget, Role, SearchGrid, SensorModel};

/// Mutable grid scan progress, target placement, and scan results.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GridState {
    pub grid: SearchGrid,
    pub cells: Vec<CellState>,
    pub targets: Vec<HiddenTarget>,
    pub sensor: SensorModel,
    #[serde(default)]
    pub targets_found: u32,
    #[serde(default)]
    pub first_find_tick: Option<u64>,
    #[serde(default)]
    pub scan_count: u32,
    // v0.14 SAR v2 belief map
    #[serde(default)]
    pub belief_map: Option<BeliefMap>,
}

impl GridState {
    pub fn new(grid: SearchGrid, targets: Vec<HiddenTarget>, sensor: SensorModel) -> Self {
        let cell_count = grid.total_cells() as usize;
        Self {
            grid,
            cells: vec![CellState::Unvisited; cell_count],
            targets,
            sensor,
            targets_found: 0,
            first_find_tick: None,
            scan_count: 0,
            belief_map: None,
        }
    }

    /// Enable SAR v2 belief tracking with the given prior probability.
    pub fn with_belief(mut self, prior: f64) -> Self {
        self.belief_map = Some(BeliefMap::new(&self.grid, prior));
        self
    }

    /// Scan a cell when an agent arrives at its center.
    /// Returns true if a target was found in this scan.
    #[allow(clippy::too_many_arguments)]
    pub fn scan_cell<R: Rng>(
        &mut self,
        agent_id: AgentId,
        cell_x: u32,
        cell_y: u32,
        role: &Role,
        current_tick: u64,
        agent_z: f64,
        rng: &mut R,
    ) -> bool {
        let cell_idx = self.grid.cell_index(cell_x, cell_y);
        if cell_idx >= self.cells.len() {
            return false;
        }

        // Idempotent: already visited or target found
        if !matches!(self.cells[cell_idx], CellState::Unvisited) {
            return matches!(self.cells[cell_idx], CellState::TargetFound { .. });
        }

        self.scan_count += 1;

        // Check if there's a target in this cell
        let target_here = self
            .targets
            .iter()
            .find(|t| t.cell_x == cell_x && t.cell_y == cell_y);

        let base_pod = self.sensor.probability(role);
        // v0.31: altitude penalty reduces PoD proportionally
        let altitude_penalty = if self.sensor.altitude_factor > 0.0 && agent_z > 0.0 {
            (1.0 - self.sensor.altitude_factor * agent_z).max(0.0)
        } else {
            1.0
        };
        let pod = base_pod * altitude_penalty;
        let roll: f64 = rng.gen();
        let detected = roll < pod;

        // SAR v2: update belief map with Bayes rule
        if let Some(ref mut bm) = self.belief_map {
            bm.update((cell_x, cell_y), detected, &self.sensor);
            if detected && target_here.is_none() {
                bm.false_positives += 1;
            }
            if bm.cells[cell_y as usize][cell_x as usize].scan_count > 1 {
                bm.confirmation_scans += 1;
            }
        }

        if let Some(target) = target_here {
            if detected {
                // Target found!
                self.cells[cell_idx] = CellState::TargetFound {
                    target_id: target.id.clone(),
                    found_by: agent_id.clone(),
                    found_at_tick: current_tick,
                };
                self.targets_found += 1;
                if self.first_find_tick.is_none() {
                    self.first_find_tick = Some(current_tick);
                }
                return true;
            }
        }

        // Scanned but no target found (or target missed due to PoD)
        self.cells[cell_idx] = CellState::Visited {
            scanned_by: vec![agent_id],
            scan_tick: current_tick,
        };
        false
    }

    pub fn coverage_fraction(&self) -> f64 {
        let visited = self
            .cells
            .iter()
            .filter(|c| !matches!(c, CellState::Unvisited))
            .count();
        visited as f64 / self.cells.len() as f64
    }

    pub fn all_targets_found(&self) -> bool {
        self.targets_found == self.targets.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn scan_finds_target_when_pod_is_one() {
        let grid = SearchGrid::new(5, 5, 10.0);
        let targets = vec![HiddenTarget {
            id: "t1".to_string(),
            cell_x: 2,
            cell_y: 2,
        }];
        let sensor = SensorModel::new(1.0, 1.0, 1.0);
        let mut state = GridState::new(grid, targets, sensor);
        let mut rng = StdRng::seed_from_u64(42);

        let found = state.scan_cell(
            AgentId::from("a1".to_owned()),
            2,
            2,
            &Role::Scout,
            10,
            0.0,
            &mut rng,
        );
        assert!(found);
        assert_eq!(state.targets_found, 1);
        assert_eq!(state.first_find_tick, Some(10));
    }

    #[test]
    fn scan_misses_target_when_pod_is_zero() {
        let grid = SearchGrid::new(5, 5, 10.0);
        let targets = vec![HiddenTarget {
            id: "t1".to_string(),
            cell_x: 2,
            cell_y: 2,
        }];
        let sensor = SensorModel::new(0.0, 0.0, 0.0);
        let mut state = GridState::new(grid.clone(), targets, sensor);
        let mut rng = StdRng::seed_from_u64(42);

        let found = state.scan_cell(
            AgentId::from("a1".to_owned()),
            2,
            2,
            &Role::Scout,
            10,
            0.0,
            &mut rng,
        );
        assert!(!found);
        assert_eq!(state.targets_found, 0);
        assert!(matches!(
            state.cells[grid.cell_index(2, 2)],
            CellState::Visited { .. }
        ));
    }

    #[test]
    fn scan_coverage_fraction() {
        let grid = SearchGrid::new(2, 2, 10.0);
        let targets = vec![];
        let sensor = SensorModel::new(1.0, 1.0, 1.0);
        let mut state = GridState::new(grid, targets, sensor);
        let mut rng = StdRng::seed_from_u64(42);

        state.scan_cell(
            AgentId::from("a1".to_owned()),
            0,
            0,
            &Role::Scout,
            1,
            0.0,
            &mut rng,
        );
        state.scan_cell(
            AgentId::from("a1".to_owned()),
            1,
            0,
            &Role::Scout,
            2,
            0.0,
            &mut rng,
        );

        assert_eq!(state.coverage_fraction(), 0.5);
    }

    #[test]
    fn scan_idempotent() {
        let grid = SearchGrid::new(5, 5, 10.0);
        let targets = vec![HiddenTarget {
            id: "t1".to_string(),
            cell_x: 2,
            cell_y: 2,
        }];
        let sensor = SensorModel::new(1.0, 1.0, 1.0);
        let mut state = GridState::new(grid, targets, sensor);
        let mut rng = StdRng::seed_from_u64(42);

        let found1 = state.scan_cell(
            AgentId::from("a1".to_owned()),
            2,
            2,
            &Role::Scout,
            10,
            0.0,
            &mut rng,
        );
        assert!(found1);
        assert_eq!(state.targets_found, 1);

        let found2 = state.scan_cell(
            AgentId::from("a2".to_owned()),
            2,
            2,
            &Role::Scout,
            11,
            0.0,
            &mut rng,
        );
        // Second scan should not change state (idempotent)
        assert!(found2); // returns true because cell is TargetFound
        assert_eq!(state.targets_found, 1); // but count doesn't increase
        assert_eq!(state.first_find_tick, Some(10));
    }

    #[test]
    fn scan_altitude_factor_reduces_pod() {
        let grid = SearchGrid::new(5, 5, 10.0);
        let targets = vec![HiddenTarget {
            id: "t1".to_string(),
            cell_x: 2,
            cell_y: 2,
        }];
        // altitude_factor = 0.1: at z=5, penalty = 1 - 0.1*5 = 0.5
        let mut sensor = SensorModel::new(1.0, 1.0, 1.0);
        sensor.altitude_factor = 0.1;
        let state = GridState::new(grid, targets, sensor);
        // With pod=1.0 and altitude penalty=0.5, effective_pod=0.5 — use many runs to verify
        let hits: u32 = (0..100)
            .map(|i| {
                let mut s = state.clone();
                let mut rng = rand::rngs::StdRng::seed_from_u64(i);
                s.scan_cell(
                    AgentId::from("a1".to_owned()),
                    2,
                    2,
                    &Role::Scout,
                    1,
                    5.0,
                    &mut rng,
                ) as u32
            })
            .sum();
        // With 50% effective PoD, expect roughly 50 hits out of 100 (within generous bounds)
        assert!(hits > 20 && hits < 80, "Expected ~50 hits, got {hits}");
    }

    #[test]
    fn scan_altitude_factor_zero_no_penalty() {
        let grid = SearchGrid::new(5, 5, 10.0);
        let targets = vec![HiddenTarget {
            id: "t1".to_string(),
            cell_x: 2,
            cell_y: 2,
        }];
        let sensor = SensorModel::new(1.0, 1.0, 1.0); // altitude_factor = 0.0
        let mut state = GridState::new(grid, targets, sensor);
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        // pod=1.0, no altitude penalty → always finds target
        let found = state.scan_cell(
            AgentId::from("a1".to_owned()),
            2,
            2,
            &Role::Scout,
            1,
            100.0,
            &mut rng,
        );
        assert!(found);
    }
}
