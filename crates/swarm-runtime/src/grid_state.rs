use rand::Rng;

use swarm_types::{AgentId, CellState, HiddenTarget, Role, SearchGrid, SensorModel};

/// Mutable grid scan progress, target placement, and scan results.
#[derive(Clone, Debug)]
pub struct GridState {
    pub grid: SearchGrid,
    pub cells: Vec<CellState>,
    pub targets: Vec<HiddenTarget>,
    pub sensor: SensorModel,
    pub targets_found: u32,
    pub first_find_tick: Option<u64>,
    pub scan_count: u32,
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
        }
    }

    /// Scan a cell when an agent arrives at its center.
    /// Returns true if a target was found in this scan.
    pub fn scan_cell<R: Rng>(
        &mut self,
        agent_id: AgentId,
        cell_x: u32,
        cell_y: u32,
        role: &Role,
        current_tick: u64,
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

        if let Some(target) = target_here {
            let pod = self.sensor.probability(role);
            let roll: f64 = rng.gen();
            if roll < pod {
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
            &mut rng,
        );
        state.scan_cell(
            AgentId::from("a1".to_owned()),
            1,
            0,
            &Role::Scout,
            2,
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
            &mut rng,
        );
        // Second scan should not change state (idempotent)
        assert!(found2); // returns true because cell is TargetFound
        assert_eq!(state.targets_found, 1); // but count doesn't increase
        assert_eq!(state.first_find_tick, Some(10));
    }
}
