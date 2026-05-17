use serde::{Deserialize, Serialize};

use crate::agent::AgentId;
use crate::pose::Pose;

/// Discrete search area divided into cells.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SearchGrid {
    pub width: u32,     // cells in x
    pub height: u32,    // cells in y
    pub cell_size: f64, // meters per cell
}

impl SearchGrid {
    pub fn new(width: u32, height: u32, cell_size: f64) -> Self {
        Self {
            width,
            height,
            cell_size,
        }
    }

    pub fn cell_center(&self, x: u32, y: u32) -> Pose {
        Pose {
            x: (x as f64 + 0.5) * self.cell_size,
            y: (y as f64 + 0.5) * self.cell_size,
        }
    }

    pub fn cell_at_pose(&self, pose: &Pose) -> Option<(u32, u32)> {
        let x = (pose.x / self.cell_size).floor() as i32;
        let y = (pose.y / self.cell_size).floor() as i32;
        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
            Some((x as u32, y as u32))
        } else {
            None
        }
    }

    pub fn total_cells(&self) -> u32 {
        self.width * self.height
    }

    pub fn cell_index(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }
}

/// State of a single grid cell.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CellState {
    Unvisited,
    Visited {
        scanned_by: Vec<AgentId>,
        scan_tick: u64,
    },
    TargetFound {
        target_id: String,
        found_by: AgentId,
        found_at_tick: u64,
    },
}

/// Hidden target placed on the grid.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HiddenTarget {
    pub id: String,
    pub cell_x: u32,
    pub cell_y: u32,
}

impl HiddenTarget {
    pub fn pose(&self, grid: &SearchGrid) -> Pose {
        grid.cell_center(self.cell_x, self.cell_y)
    }
}

/// Probability-of-Detection model based on agent role.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SensorModel {
    pub scout_pod: f64,   // base PoD for Scout role
    pub thermal_pod: f64, // elevated PoD for Thermal role
    pub relay_pod: f64,   // reduced PoD for Relay (if they scan at all)
}

impl SensorModel {
    pub fn new(scout_pod: f64, thermal_pod: f64, relay_pod: f64) -> Self {
        Self {
            scout_pod,
            thermal_pod,
            relay_pod,
        }
    }

    pub fn probability(&self, role: &crate::agent::Role) -> f64 {
        match role {
            crate::agent::Role::Scout => self.scout_pod,
            crate::agent::Role::Thermal => self.thermal_pod,
            _ => self.relay_pod,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Role;

    #[test]
    fn search_grid_cell_count() {
        let grid = SearchGrid::new(10, 10, 5.0);
        assert_eq!(grid.total_cells(), 100);
    }

    #[test]
    fn cell_center_roundtrip() {
        let grid = SearchGrid::new(10, 10, 5.0);
        let pose = grid.cell_center(3, 4);
        let (x, y) = grid.cell_at_pose(&pose).unwrap();
        assert_eq!(x, 3);
        assert_eq!(y, 4);
    }

    #[test]
    fn cell_at_pose_out_of_bounds() {
        let grid = SearchGrid::new(5, 5, 10.0);
        assert!(grid.cell_at_pose(&Pose { x: -1.0, y: 0.0 }).is_none());
        assert!(grid.cell_at_pose(&Pose { x: 0.0, y: -1.0 }).is_none());
        assert!(grid.cell_at_pose(&Pose { x: 51.0, y: 0.0 }).is_none());
        assert!(grid.cell_at_pose(&Pose { x: 0.0, y: 51.0 }).is_none());
    }

    #[test]
    fn sensor_model_scout_vs_thermal() {
        let sensor = SensorModel::new(0.3, 0.8, 0.1);
        assert_eq!(sensor.probability(&Role::Scout), 0.3);
        assert_eq!(sensor.probability(&Role::Thermal), 0.8);
        assert_eq!(sensor.probability(&Role::Relay), 0.1);
    }

    #[test]
    fn hidden_target_pose() {
        let grid = SearchGrid::new(10, 10, 5.0);
        let target = HiddenTarget {
            id: "t1".to_string(),
            cell_x: 2,
            cell_y: 3,
        };
        let pose = target.pose(&grid);
        assert_eq!(pose.x, 12.5);
        assert_eq!(pose.y, 17.5);
    }
}
