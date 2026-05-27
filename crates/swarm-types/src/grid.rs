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
            ..Default::default()
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
    // v0.14 — sensor model v2 for Bayesian belief update
    #[serde(default = "default_detection_probability")]
    pub detection_probability: f64, // P(detect | target present)
    #[serde(default = "default_false_positive_rate")]
    pub false_positive_rate: f64, // P(detect | no target)
}

fn default_detection_probability() -> f64 {
    0.5
}

fn default_false_positive_rate() -> f64 {
    0.1
}

impl SensorModel {
    pub fn new(scout_pod: f64, thermal_pod: f64, relay_pod: f64) -> Self {
        Self {
            scout_pod,
            thermal_pod,
            relay_pod,
            detection_probability: 0.5,
            false_positive_rate: 0.1,
        }
    }

    pub fn new_v2(
        scout_pod: f64,
        thermal_pod: f64,
        relay_pod: f64,
        detection_probability: f64,
        false_positive_rate: f64,
    ) -> Self {
        Self {
            scout_pod,
            thermal_pod,
            relay_pod,
            detection_probability,
            false_positive_rate,
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

/// Belief cell tracking probabilistic state of a grid cell.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BeliefCell {
    pub prior: f64,
    pub posterior: f64,
    pub scan_count: u32,
    pub last_scan_tick: Option<u64>,
}

/// Probabilistic belief map for SAR v2.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BeliefMap {
    pub grid: SearchGrid,
    pub cells: Vec<Vec<BeliefCell>>,
    pub false_positives: u32,
    pub confirmation_scans: u32,
}

impl BeliefMap {
    pub fn new(grid: &SearchGrid, prior: f64) -> Self {
        let cells = (0..grid.height)
            .map(|_| {
                (0..grid.width)
                    .map(|_| BeliefCell {
                        prior,
                        posterior: prior,
                        scan_count: 0,
                        last_scan_tick: None,
                    })
                    .collect()
            })
            .collect();
        Self {
            grid: grid.clone(),
            cells,
            false_positives: 0,
            confirmation_scans: 0,
        }
    }

    pub fn update(&mut self, cell: (u32, u32), detection: bool, sensor: &SensorModel) {
        let (x, y) = cell;
        let bc = &mut self.cells[y as usize][x as usize];
        bc.scan_count += 1;

        let p_d_given_t = sensor.detection_probability; // P(detection | target)
        let p_d_given_not_t = sensor.false_positive_rate; // P(detection | no target)
        let p_t = bc.posterior;
        let p_not_t = 1.0 - p_t;

        let p_d = p_d_given_t * p_t + p_d_given_not_t * p_not_t;
        if p_d > 0.0 && p_d < 1.0 {
            bc.posterior = if detection {
                p_d_given_t * p_t / p_d
            } else {
                (1.0 - p_d_given_t) * p_t / (1.0 - p_d)
            };
        }
        bc.posterior = bc.posterior.clamp(0.0, 1.0);
    }

    pub fn entropy(&self, cell: (u32, u32)) -> f64 {
        let (x, y) = cell;
        let p = self.cells[y as usize][x as usize].posterior;
        if p <= 0.0 || p >= 1.0 {
            return 0.0;
        }
        -p * p.log2() - (1.0 - p) * (1.0 - p).log2()
    }

    pub fn highest_uncertainty_cells(&self, n: usize) -> Vec<(u32, u32)> {
        let mut all: Vec<((u32, u32), f64)> = (0..self.grid.height)
            .flat_map(|y| (0..self.grid.width).map(move |x| ((x, y), self.entropy((x, y)))))
            .collect();
        all.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        all.into_iter().take(n).map(|(c, _)| c).collect()
    }

    pub fn mean_entropy(&self) -> f64 {
        let total: f64 = (0..self.grid.height)
            .flat_map(|y| (0..self.grid.width).map(move |x| self.entropy((x, y))))
            .sum();
        total / (self.grid.width * self.grid.height) as f64
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
        assert!(grid.cell_at_pose(&Pose { x: -1.0, y: 0.0 , ..Default::default()}).is_none());
        assert!(grid.cell_at_pose(&Pose { x: 0.0, y: -1.0 , ..Default::default()}).is_none());
        assert!(grid.cell_at_pose(&Pose { x: 51.0, y: 0.0 , ..Default::default()}).is_none());
        assert!(grid.cell_at_pose(&Pose { x: 0.0, y: 51.0 , ..Default::default()}).is_none());
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

    #[test]
    fn sensor_model_v2_fields() {
        let sensor = SensorModel::new_v2(0.6, 0.95, 0.2, 0.7, 0.15);
        assert_eq!(sensor.detection_probability, 0.7);
        assert_eq!(sensor.false_positive_rate, 0.15);
    }

    #[test]
    fn sensor_model_v2_serde_roundtrip() {
        let sensor = SensorModel::new_v2(0.6, 0.95, 0.2, 0.7, 0.15);
        let json = serde_json::to_string(&sensor).unwrap();
        let parsed: SensorModel = serde_json::from_str(&json).unwrap();
        assert_eq!(sensor, parsed);
    }

    #[test]
    fn belief_map_update_bayes_correct() {
        let grid = SearchGrid::new(5, 5, 10.0);
        let sensor = SensorModel::new_v2(0.6, 0.95, 0.2, 0.8, 0.1);
        let mut bm = BeliefMap::new(&grid, 0.2);

        // Detection: posterior should increase
        bm.update((2, 2), true, &sensor);
        assert!(bm.cells[2][2].posterior > 0.2);

        // No detection: posterior should decrease
        let p_before = bm.cells[2][2].posterior;
        bm.update((2, 2), false, &sensor);
        assert!(bm.cells[2][2].posterior < p_before);
    }

    #[test]
    fn belief_map_entropy_zero_at_extremes() {
        let grid = SearchGrid::new(5, 5, 10.0);
        let mut bm = BeliefMap::new(&grid, 0.5);
        bm.cells[0][0].posterior = 0.0;
        bm.cells[1][1].posterior = 1.0;
        assert_eq!(bm.entropy((0, 0)), 0.0);
        assert_eq!(bm.entropy((1, 1)), 0.0);
    }

    #[test]
    fn belief_map_entropy_max_at_half() {
        let grid = SearchGrid::new(5, 5, 10.0);
        let mut bm = BeliefMap::new(&grid, 0.5);
        bm.cells[0][0].posterior = 0.5;
        let h = bm.entropy((0, 0));
        assert!(h > 0.9, "entropy at 0.5 should be ~1.0, got {}", h);
    }

    #[test]
    fn belief_map_posterior_clamped() {
        let grid = SearchGrid::new(3, 3, 10.0);
        let sensor = SensorModel::new_v2(0.6, 0.95, 0.2, 1.0, 0.0);
        let mut bm = BeliefMap::new(&grid, 0.5);

        // Many detections should not push posterior above 1.0
        for _ in 0..20 {
            bm.update((1, 1), true, &sensor);
        }
        assert!(bm.cells[1][1].posterior <= 1.0);
        assert!(bm.cells[1][1].posterior >= 0.0);
    }

    #[test]
    fn belief_map_highest_uncertainty_cells() {
        let grid = SearchGrid::new(3, 3, 10.0);
        let mut bm = BeliefMap::new(&grid, 0.01); // low prior so entropy is low initially
        bm.cells[0][0].posterior = 0.99; // low entropy
        bm.cells[1][1].posterior = 0.5; // high entropy (max ~1.0)
        bm.cells[2][2].posterior = 0.1; // medium entropy

        let top = bm.highest_uncertainty_cells(2);
        // (1,1) has max entropy, should be first; second can be any remaining cell with entropy > 0
        assert_eq!(top[0], (1, 1));
        assert!(top.len() == 2);
    }
}
