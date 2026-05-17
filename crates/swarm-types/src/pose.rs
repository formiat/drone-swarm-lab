use serde::{Deserialize, Serialize};

/// 2D position in simulation space (metres or arbitrary units).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub x: f64,
    pub y: f64,
}

impl Pose {
    /// Euclidean distance to another pose.
    pub fn distance_to(&self, other: &Pose) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// 2D velocity vector.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Velocity {
    pub vx: f64,
    pub vy: f64,
}

impl Velocity {
    /// Scalar speed (magnitude of velocity vector).
    pub fn speed(&self) -> f64 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }
}
