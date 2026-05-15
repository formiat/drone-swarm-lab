use swarm_types::AgentId;

/// A raw, untyped message passed through the transport layer.
#[derive(Clone, Debug)]
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
