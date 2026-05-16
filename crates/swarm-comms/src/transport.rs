use serde::{Deserialize, Serialize};
use swarm_types::AgentId;

/// A raw, untyped message passed through the transport layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawMessage {
    pub from: AgentId,
    pub to: AgentId,
    pub payload: Vec<u8>,
}

/// Pluggable point-to-point message transport between agents.
///
/// Implementations may be in-memory, UDP-backed, or MAVLink-backed.
pub trait Transport {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Enqueue a message for delivery to its recipient.
    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error>;

    /// Poll for the next incoming message; returns `None` if the inbox is empty.
    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_message_serde_roundtrip() {
        let msg = RawMessage {
            from: AgentId::from("agent-0".to_owned()),
            to: AgentId::from("agent-1".to_owned()),
            payload: b"hb".to_vec(),
        };
        let bytes = serde_json::to_vec(&msg).unwrap();
        let back: RawMessage = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.from, msg.from);
        assert_eq!(back.to, msg.to);
        assert_eq!(back.payload, msg.payload);
    }
}
