use serde::{Deserialize, Serialize};
use swarm_types::AgentId;

/// A raw, untyped message passed through the transport layer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawMessage {
    pub from: AgentId,
    pub to: AgentId,
    pub payload: Vec<u8>,
}

/// Typed command-plane envelope that can be carried by test transports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub envelope_id: String,
    pub route_id: String,
    pub from: AgentId,
    pub to: AgentId,
    pub payload: Vec<u8>,
    pub topology_kind: String,
}

impl CommandEnvelope {
    pub fn into_raw_message(self) -> RawMessage {
        RawMessage {
            from: self.from,
            to: self.to,
            payload: self.payload,
        }
    }
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

    #[test]
    fn command_envelope_serializes_deterministically() {
        let envelope = CommandEnvelope {
            envelope_id: "env-1".to_owned(),
            route_id: "route:gcs:agent-0".to_owned(),
            from: AgentId::from("gcs".to_owned()),
            to: AgentId::from("agent-0".to_owned()),
            payload: b"command".to_vec(),
            topology_kind: "centralized_gcs".to_owned(),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let back: CommandEnvelope = serde_json::from_str(&json).unwrap();
        let raw = back.clone().into_raw_message();

        assert_eq!(back, envelope);
        assert_eq!(raw.from, AgentId::from("gcs".to_owned()));
        assert_eq!(raw.to, AgentId::from("agent-0".to_owned()));
        assert_eq!(raw.payload, b"command".to_vec());
    }
}
