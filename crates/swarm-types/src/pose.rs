use serde::{Deserialize, Serialize};

/// 3D position in simulation space (metres or arbitrary units).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub z: f64,
}

impl Pose {
    /// Euclidean distance to another pose in 3D space.
    pub fn distance_to(&self, other: &Pose) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Horizontal (XY) distance — used for legacy 2D calculations.
    pub fn distance_to_2d(&self, other: &Pose) -> f64 {
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
            ..Default::default()
        }
    }

    pub fn area(&self) -> f64 {
        (self.max_x - self.min_x) * (self.max_y - self.min_y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pose_z_defaults_to_zero_on_deserialize() {
        let json = r#"{"x": 1.0, "y": 2.0}"#;
        let pose: Pose = serde_json::from_str(json).unwrap();
        assert_eq!(pose.x, 1.0);
        assert_eq!(pose.y, 2.0);
        assert_eq!(pose.z, 0.0);
    }

    #[test]
    fn pose_z_roundtrip() {
        let p = Pose {
            x: 1.0,
            y: 2.0,
            z: 3.5,
        };
        let json = serde_json::to_string(&p).unwrap();
        let parsed: Pose = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.z, 3.5);
    }

    #[test]
    fn distance_to_3d() {
        let a = Pose {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let b = Pose {
            x: 3.0,
            y: 4.0,
            z: 0.0,
        };
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn distance_to_2d_ignores_z() {
        let a = Pose {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let b = Pose {
            x: 3.0,
            y: 4.0,
            z: 100.0,
        };
        assert!((a.distance_to_2d(&b) - 5.0).abs() < 1e-9);
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
