use derive_more::{AsRef, Deref, DerefMut, From, Into};
use serde::{Deserialize, Serialize};

/// Unique identifier for a single command within a mission plan.
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, AsRef, Deref, DerefMut, From, Into,
)]
pub struct CommandId(String);

/// Unique identifier for a mission.
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, AsRef, Deref, DerefMut, From, Into,
)]
pub struct MissionId(String);

/// Identifier for a named route used in `follow_route` commands.
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, AsRef, Deref, DerefMut, From, Into,
)]
pub struct RouteId(String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_id_roundtrip() {
        let id = CommandId::from("cmd-1".to_owned());
        let json = serde_json::to_string(&id).unwrap();
        let back: CommandId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn mission_id_roundtrip() {
        let id = MissionId::from("m-42".to_owned());
        let json = serde_json::to_string(&id).unwrap();
        let back: MissionId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn route_id_roundtrip() {
        let id = RouteId::from("urban-patrol-loop".to_owned());
        let json = serde_json::to_string(&id).unwrap();
        let back: RouteId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }
}
