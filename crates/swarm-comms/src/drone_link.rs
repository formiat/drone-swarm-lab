//! Transport implementations for drone-to-drone and drone-to-GCS communication.
//!
//! Provides four pluggable [`Transport`] implementations:
//! - [`UdpDroneLink`]       — real UDP unicast between processes
//! - [`InternetLikeMock`]   — in-process simulated LTE / satcom channel
//! - [`SerialDroneLink`]    — placeholder for future serial-port support
//! - [`NullDroneLink`]      — no-op transport for unit tests
//!
//! [`DroneLinkConfig`] selects the backend in scenario / agent configs.

use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::net::{SocketAddr, UdpSocket};

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use swarm_types::AgentId;

use crate::{RawMessage, Transport};

/// Maximum UDP datagram payload: 65 535 − 8 (UDP) − 20 (IPv4) = 65 507 bytes.
pub const UDP_MAX_PAYLOAD: usize = 65_507;

// ─── UdpDroneLinkError ────────────────────────────────────────────────────────

/// Errors produced by [`UdpDroneLink`].
#[derive(Debug, thiserror::Error)]
pub enum UdpDroneLinkError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unknown peer: {0}")]
    UnknownPeer(AgentId),
    #[error("payload too large: {0} bytes (max 65507)")]
    PayloadTooLarge(usize),
}

// ─── UdpDroneLink ─────────────────────────────────────────────────────────────

/// UDP unicast transport between agent processes.
///
/// Non-blocking: `send()` uses `sendto` without waiting for an ACK; `poll()`
/// uses `recv_from` in non-blocking mode.
pub struct UdpDroneLink {
    socket: UdpSocket,
    own_id: AgentId,
    /// key: `AgentId`
    peers: HashMap<AgentId, SocketAddr>,
    recv_buffer: VecDeque<RawMessage>,
}

impl UdpDroneLink {
    /// Bind to `bind_addr` and register known `peers`.
    pub fn bind(
        own_id: AgentId,
        bind_addr: SocketAddr,
        peers: HashMap<AgentId, SocketAddr>,
    ) -> Result<Self, UdpDroneLinkError> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            own_id,
            peers,
            recv_buffer: VecDeque::new(),
        })
    }

    /// Returns the agent identifier this link is bound to.
    pub fn local_id(&self) -> &AgentId {
        &self.own_id
    }

    /// Returns the local socket address (useful for test port discovery).
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }
}

impl Transport for UdpDroneLink {
    type Error = UdpDroneLinkError;

    /// Serialise and send `msg` via UDP unicast.
    ///
    /// Returns [`UdpDroneLinkError::UnknownPeer`] if the recipient is absent
    /// from the peer table, or [`UdpDroneLinkError::PayloadTooLarge`] if
    /// `msg.payload` exceeds 65 507 bytes.
    fn send(&mut self, msg: RawMessage) -> Result<(), UdpDroneLinkError> {
        let addr = self
            .peers
            .get(&msg.to)
            .copied()
            .ok_or_else(|| UdpDroneLinkError::UnknownPeer(msg.to.clone()))?;
        if msg.payload.len() > UDP_MAX_PAYLOAD {
            return Err(UdpDroneLinkError::PayloadTooLarge(msg.payload.len()));
        }
        let bytes = serde_json::to_vec(&msg).expect("RawMessage serialisation must not fail");
        self.socket.send_to(&bytes, addr)?;
        Ok(())
    }

    /// Non-blocking receive; returns `None` when no datagram is ready.
    fn poll(&mut self) -> Result<Option<RawMessage>, UdpDroneLinkError> {
        if let Some(msg) = self.recv_buffer.pop_front() {
            return Ok(Some(msg));
        }
        let mut buf = vec![0u8; UDP_MAX_PAYLOAD + 128];
        match self.socket.recv_from(&mut buf) {
            Ok((len, _addr)) => match serde_json::from_slice::<RawMessage>(&buf[..len]) {
                Ok(msg) => Ok(Some(msg)),
                Err(_) => Ok(None), // malformed datagram — discard silently
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(UdpDroneLinkError::Io(e)),
        }
    }
}

// ─── InternetLikeMock ────────────────────────────────────────────────────────

/// Named channel profiles for [`InternetLikeMock`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InternetLikeMockProfile {
    Lte,
    Satcom,
}

/// Simulates internet-like channel characteristics in-process.
///
/// Models high baseline latency, variable per-packet jitter, burst drops, and
/// occasional packet reordering. Useful for testing cellular/LTE scenarios.
///
/// # Design note
/// Uses an internal pending queue rather than `InMemNetwork` because
/// `InMemNetwork::drain_ready()` requires an `own_id` parameter that was not
/// part of the planned `Transport` API. The behaviour is equivalent.
pub struct InternetLikeMock {
    /// In-transit messages: `(delivery_tick, msg)`.
    pending: VecDeque<(u64, RawMessage)>,
    current_tick: u64,
    base_latency_ticks: u64,
    jitter_ticks: u64,
    packet_loss_rate: f64,
    reorder_probability: f64,
    burst_drop_probability: f64,
    in_burst: bool,
    rng: SmallRng,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InternetLikeMockProfileSummary {
    pub base_latency_ticks: u64,
    pub jitter_ticks: u64,
    pub packet_loss_rate: f64,
    pub reorder_probability: f64,
    pub burst_drop_probability: f64,
}

impl InternetLikeMock {
    /// LTE profile: ~1 tick base latency with jitter, 3 % packet loss.
    pub fn with_lte_profile(seed: u64) -> Self {
        Self {
            pending: VecDeque::new(),
            current_tick: 0,
            base_latency_ticks: 1,
            jitter_ticks: 2,
            packet_loss_rate: 0.03,
            reorder_probability: 0.05,
            burst_drop_probability: 0.0, // LTE has memoryless loss, no bursts
            in_burst: false,
            rng: SmallRng::seed_from_u64(seed),
        }
    }

    /// Satcom profile: ~6 tick base latency with jitter, 8 % packet loss.
    pub fn with_satcom_profile(seed: u64) -> Self {
        Self {
            pending: VecDeque::new(),
            current_tick: 0,
            base_latency_ticks: 6,
            jitter_ticks: 4,
            packet_loss_rate: 0.08,
            reorder_probability: 0.08,
            burst_drop_probability: 0.03, // satellite links have bursty loss
            in_burst: false,
            rng: SmallRng::seed_from_u64(seed),
        }
    }

    /// Advance simulation time by one tick, making delayed messages deliverable.
    pub fn advance_tick(&mut self) {
        self.current_tick += 1;
    }

    /// Current simulation tick.
    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    pub fn profile_summary(&self) -> InternetLikeMockProfileSummary {
        InternetLikeMockProfileSummary {
            base_latency_ticks: self.base_latency_ticks,
            jitter_ticks: self.jitter_ticks,
            packet_loss_rate: self.packet_loss_rate,
            reorder_probability: self.reorder_probability,
            burst_drop_probability: self.burst_drop_probability,
        }
    }
}

impl Transport for InternetLikeMock {
    type Error = Infallible;

    fn send(&mut self, msg: RawMessage) -> Result<(), Infallible> {
        // Burst drop logic (only active when burst_drop_probability > 0)
        if self.burst_drop_probability > 0.0 {
            if self.in_burst {
                // Exit burst with 50 % probability per message
                if self.rng.gen::<f64>() < 0.5 {
                    self.in_burst = false;
                }
                return Ok(());
            }
            if self.rng.gen::<f64>() < self.burst_drop_probability {
                self.in_burst = true;
                return Ok(());
            }
        }

        // Normal random packet loss
        if self.rng.gen::<f64>() < self.packet_loss_rate {
            return Ok(());
        }

        // Compute delivery tick: base + uniform jitter in [0, 2*jitter_ticks]
        let jitter = if self.jitter_ticks > 0 {
            self.rng.gen::<u64>() % (self.jitter_ticks * 2 + 1)
        } else {
            0
        };
        let delivery_tick = self.current_tick + self.base_latency_ticks + jitter;

        // Reorder: with small probability insert at the front of the queue so
        // the message is returned before earlier-queued messages with the same
        // delivery_tick (queue-level reordering, not timing-level).
        if self.reorder_probability > 0.0
            && !self.pending.is_empty()
            && self.rng.gen::<f64>() < self.reorder_probability
        {
            self.pending.push_front((delivery_tick, msg));
        } else {
            self.pending.push_back((delivery_tick, msg));
        }
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, Infallible> {
        // Find and return the first message whose delivery tick has passed.
        let pos = self
            .pending
            .iter()
            .position(|(tick, _)| *tick <= self.current_tick);
        if let Some(idx) = pos {
            let (_, msg) = self.pending.remove(idx).unwrap();
            return Ok(Some(msg));
        }
        Ok(None)
    }
}

// ─── SerialDroneLink ─────────────────────────────────────────────────────────

/// Errors produced by [`SerialDroneLink`].
#[derive(Debug, thiserror::Error)]
pub enum SerialDroneLinkError {
    #[error("serial transport not yet implemented")]
    NotImplemented,
}

/// Serial-port transport placeholder; all operations return
/// [`SerialDroneLinkError::NotImplemented`].
pub struct SerialDroneLink {
    pub own_id: AgentId,
}

impl Transport for SerialDroneLink {
    type Error = SerialDroneLinkError;

    fn send(&mut self, _: RawMessage) -> Result<(), SerialDroneLinkError> {
        Err(SerialDroneLinkError::NotImplemented)
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, SerialDroneLinkError> {
        Err(SerialDroneLinkError::NotImplemented)
    }
}

// ─── NullDroneLink ────────────────────────────────────────────────────────────

/// No-op transport for unit tests: drops all outgoing messages and never
/// returns incoming ones.
pub struct NullDroneLink {
    pub own_id: AgentId,
}

impl Transport for NullDroneLink {
    type Error = Infallible;

    fn send(&mut self, _: RawMessage) -> Result<(), Infallible> {
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, Infallible> {
        Ok(None)
    }
}

// ─── DroneLinkConfig ─────────────────────────────────────────────────────────

/// Selects the transport backend for a scenario or standalone agent process.
///
/// Defaults to [`DroneLinkConfig::Simulated`] so existing scenarios that do
/// not specify this field continue to work without modification.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DroneLinkConfig {
    /// In-memory shared bus (backward-compatible default for all existing scenarios).
    #[default]
    Simulated,
    /// UDP unicast between processes on localhost or LAN.
    Udp {
        bind_addr: String,
        /// key: `AgentId`
        peers: HashMap<AgentId, String>,
    },
    /// Simulated internet-like channel (high latency, variable loss).
    InternetLikeMock {
        profile: InternetLikeMockProfile,
        seed: u64,
    },
    /// Serial-port placeholder; returns `NotImplemented` for all operations.
    Serial { path: String, baud: u32 },
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn aid(s: &str) -> AgentId {
        AgentId::from(s.to_owned())
    }

    fn msg(from: &str, to: &str) -> RawMessage {
        RawMessage {
            from: aid(from),
            to: aid(to),
            payload: b"ping".to_vec(),
        }
    }

    // ── NullDroneLink ─────────────────────────────────────────────────────

    #[test]
    fn null_drone_link_send_drops_silently() {
        let mut link = NullDroneLink { own_id: aid("a") };
        link.send(msg("a", "b")).unwrap();
    }

    #[test]
    fn null_drone_link_poll_returns_none() {
        let mut link = NullDroneLink { own_id: aid("a") };
        assert!(link.poll().unwrap().is_none());
    }

    // ── SerialDroneLink ───────────────────────────────────────────────────

    #[test]
    fn serial_drone_link_returns_not_implemented() {
        let mut link = SerialDroneLink { own_id: aid("a") };
        assert!(matches!(
            link.send(msg("a", "b")),
            Err(SerialDroneLinkError::NotImplemented)
        ));
        assert!(matches!(
            link.poll(),
            Err(SerialDroneLinkError::NotImplemented)
        ));
    }

    // ── UdpDroneLink ──────────────────────────────────────────────────────

    #[test]
    fn udp_drone_link_loopback_roundtrip() {
        // Bind link_b first on an OS-assigned port; read its address.
        // Then bind link_a pointing to link_b, and link_b pointing to link_a.
        let temp_b = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr_b = temp_b.local_addr().unwrap();
        let temp_a = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr_a = temp_a.local_addr().unwrap();
        drop(temp_a);
        drop(temp_b);

        let mut peers_a: HashMap<AgentId, SocketAddr> = HashMap::new();
        peers_a.insert(aid("agent-b"), addr_b);
        let mut link_a = UdpDroneLink::bind(aid("agent-a"), addr_a, peers_a).unwrap();

        let mut peers_b: HashMap<AgentId, SocketAddr> = HashMap::new();
        peers_b.insert(aid("agent-a"), addr_a);
        let mut link_b = UdpDroneLink::bind(aid("agent-b"), addr_b, peers_b).unwrap();

        // Send from a to b
        let m = RawMessage {
            from: aid("agent-a"),
            to: aid("agent-b"),
            payload: b"hello".to_vec(),
        };
        link_a.send(m).unwrap();

        // Poll b with a short busy-wait (non-blocking socket)
        let start = std::time::Instant::now();
        let received = loop {
            if let Some(msg) = link_b.poll().unwrap() {
                break msg;
            }
            if start.elapsed() > std::time::Duration::from_secs(2) {
                panic!("timed out waiting for UDP message");
            }
            std::thread::yield_now();
        };

        assert_eq!(received.payload, b"hello");
        assert_eq!(received.from, aid("agent-a"));
    }

    #[test]
    fn udp_drone_link_unknown_peer_returns_error() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = socket.local_addr().unwrap();
        drop(socket);
        let mut link = UdpDroneLink::bind(aid("agent-a"), addr, HashMap::new()).unwrap();
        let result = link.send(RawMessage {
            from: aid("agent-a"),
            to: aid("unknown"),
            payload: vec![],
        });
        assert!(
            matches!(result, Err(UdpDroneLinkError::UnknownPeer(_))),
            "expected UnknownPeer, got {result:?}"
        );
    }

    #[test]
    fn udp_drone_link_payload_too_large_returns_error() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = socket.local_addr().unwrap();
        let peer_addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        drop(socket);

        let mut peers = HashMap::new();
        peers.insert(aid("peer"), peer_addr);
        let mut link = UdpDroneLink::bind(aid("agent-a"), addr, peers).unwrap();

        let oversized = RawMessage {
            from: aid("agent-a"),
            to: aid("peer"),
            payload: vec![0u8; UDP_MAX_PAYLOAD + 1],
        };
        let result = link.send(oversized);
        assert!(
            matches!(result, Err(UdpDroneLinkError::PayloadTooLarge(_))),
            "expected PayloadTooLarge, got {result:?}"
        );
    }

    // ── InternetLikeMock ──────────────────────────────────────────────────

    #[test]
    fn internet_like_mock_lte_delivers_messages_with_latency() {
        let mut mock = InternetLikeMock::with_lte_profile(1);
        let n = 100u32;

        for _ in 0..n {
            mock.send(msg("a", "b")).unwrap();
        }

        // At tick 0 no messages should be ready (base_latency_ticks = 1 → delivery ≥ 1)
        let ready_at_0 = {
            let mut count = 0u32;
            while mock.poll().unwrap().is_some() {
                count += 1;
            }
            count
        };
        assert_eq!(ready_at_0, 0, "messages must not be immediately available");

        // Advance past max delivery_tick = 1 + 2*2 = 5
        for _ in 0..10 {
            mock.advance_tick();
        }

        let mut delivered = 0u32;
        while mock.poll().unwrap().is_some() {
            delivered += 1;
        }
        // Allow for ~3 % loss; at least 90/100 should be delivered
        assert!(
            delivered >= 85,
            "expected ≥85 of 100 delivered, got {delivered}"
        );
    }

    #[test]
    fn internet_like_mock_lte_drops_approximately_3pct() {
        let mut mock = InternetLikeMock::with_lte_profile(42);
        let n = 1000u32;

        for _ in 0..n {
            mock.send(msg("a", "b")).unwrap();
        }

        // Advance well past max delivery_tick
        for _ in 0..20 {
            mock.advance_tick();
        }

        let mut received = 0u32;
        while mock.poll().unwrap().is_some() {
            received += 1;
        }

        let dropped = n - received;
        // Expect ~30 drops (3 %).  Wide tolerance to tolerate RNG variance.
        assert!(
            dropped <= 150,
            "too many drops: {dropped}/1000 (expected ~30)"
        );
        assert!(
            dropped >= 5,
            "too few drops: {dropped}/1000 with 3% loss rate"
        );
    }

    #[test]
    fn internet_like_mock_satcom_profile_summary_matches_docs() {
        let mock = InternetLikeMock::with_satcom_profile(7);
        let summary = mock.profile_summary();

        assert_eq!(summary.base_latency_ticks, 6);
        assert_eq!(summary.jitter_ticks, 4);
        assert_eq!(summary.packet_loss_rate, 0.08);
        assert_eq!(summary.reorder_probability, 0.08);
        assert_eq!(summary.burst_drop_probability, 0.03);
    }

    // ── DroneLinkConfig serde ─────────────────────────────────────────────

    #[test]
    fn drone_link_config_serde_roundtrip_all_variants() {
        let mut peers = HashMap::new();
        peers.insert(aid("agent-1"), "127.0.0.1:7002".to_owned());

        let configs = vec![
            DroneLinkConfig::Simulated,
            DroneLinkConfig::Udp {
                bind_addr: "127.0.0.1:7001".to_owned(),
                peers,
            },
            DroneLinkConfig::InternetLikeMock {
                profile: InternetLikeMockProfile::Lte,
                seed: 42,
            },
            DroneLinkConfig::InternetLikeMock {
                profile: InternetLikeMockProfile::Satcom,
                seed: 7,
            },
            DroneLinkConfig::Serial {
                path: "/dev/ttyUSB0".to_owned(),
                baud: 57600,
            },
        ];

        for config in &configs {
            let json = serde_json::to_string(config).unwrap();
            let back: DroneLinkConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(config, &back, "roundtrip failed for: {json}");
        }
    }

    #[test]
    fn drone_link_config_default_is_simulated() {
        assert_eq!(DroneLinkConfig::default(), DroneLinkConfig::Simulated);
    }
}
