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

/// Axis-aligned bounding box in 2D.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Aabb {
    pub fn contains(&self, pose: &Pose) -> bool {
        pose.x >= self.min_x && pose.x <= self.max_x && pose.y >= self.min_y && pose.y <= self.max_y
    }

    pub fn center(&self) -> Pose {
        Pose {
            x: (self.min_x + self.max_x) / 2.0,
            y: (self.min_y + self.max_y) / 2.0,
        }
    }

    pub fn area(&self) -> f64 {
        (self.max_x - self.min_x) * (self.max_y - self.min_y)
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
