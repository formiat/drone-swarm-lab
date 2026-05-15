use derive_more::{AsMut, AsRef, Deref, DerefMut, Display, From, Into};
use serde::{Deserialize, Serialize};

use crate::agent::AgentId;

/// Unique identifier for a swarm message.
#[derive(
    AsMut,
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct MessageId(String);

/// A typed message exchanged between two agents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message<P> {
    pub id: MessageId,
    pub from: AgentId,
    pub to: AgentId,
    pub payload: P,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_id_newtype_roundtrip() {
        let id = MessageId::from("msg-42".to_owned());
        assert_eq!(*id, "msg-42");
    }
}
