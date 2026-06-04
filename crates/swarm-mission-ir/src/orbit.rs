use serde::{Deserialize, Serialize};

/// Direction of travel for an orbit command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrbitDirection {
    Clockwise,
    CounterClockwise,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_direction_roundtrip() {
        for dir in [OrbitDirection::Clockwise, OrbitDirection::CounterClockwise] {
            let json = serde_json::to_string(&dir).unwrap();
            let back: OrbitDirection = serde_json::from_str(&json).unwrap();
            assert_eq!(dir, back);
        }
    }
}
