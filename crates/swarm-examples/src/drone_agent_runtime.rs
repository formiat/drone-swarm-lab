use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use swarm_comms::{
    AgentMissionState, DroneLinkConfig, DuplicateSuppressor, InternetLikeMock,
    InternetLikeMockProfile, MissionRejectReason, NullDroneLink, ProtocolRole, SerialDroneLink,
    SwarmMessage, SwarmMessageEnvelope, Transport, UdpDroneLink, SWARM_PROTOCOL_SCHEMA_VERSION,
};
use swarm_types::AgentId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GcsLostPolicy {
    pub kind: String,
    #[serde(default)]
    pub after_ticks: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AutonomyConfig {
    #[serde(default)]
    pub gcs_lost_policy: Option<GcsLostPolicy>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DroneAgentConfig {
    pub agent_id: AgentId,
    #[serde(default)]
    pub drone_link: DroneLinkConfig,
    #[serde(default)]
    pub autonomy: AutonomyConfig,
    #[serde(default = "default_max_ticks")]
    pub max_ticks: u64,
}

fn default_max_ticks() -> u64 {
    200
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct DroneAgentProtocolCounters {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub duplicates_dropped: u64,
    pub malformed_dropped: u64,
    pub send_errors: u64,
    pub mission_offers_seen: u64,
    pub state_requests_answered: u64,
    pub segment_grants_seen: u64,
    pub segment_denies_seen: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RunReport {
    pub agent_id: AgentId,
    pub drone_link_kind: String,
    pub ticks_run: u64,
    pub dry_run: bool,
    pub elapsed_ms: u128,
    pub protocol: DroneAgentProtocolCounters,
}

#[derive(Debug, thiserror::Error)]
pub enum DroneAgentRuntimeError {
    #[error("invalid bind_addr: {message}")]
    InvalidBindAddr { message: String },
    #[error("invalid peer addr: {message}")]
    InvalidPeerAddr { message: String },
    #[error("UDP bind error: {message}")]
    UdpBind { message: String },
    #[error("transport poll error: {message}")]
    Poll { message: String },
}

pub fn drone_link_kind(config: &DroneLinkConfig) -> &'static str {
    match config {
        DroneLinkConfig::Simulated => "simulated",
        DroneLinkConfig::Udp { .. } => "udp",
        DroneLinkConfig::InternetLikeMock { .. } => "internet_like_mock",
        DroneLinkConfig::Serial { .. } => "serial",
    }
}

pub fn run_agent(
    config: DroneAgentConfig,
    dry_run: bool,
) -> Result<RunReport, DroneAgentRuntimeError> {
    let kind = drone_link_kind(&config.drone_link).to_owned();
    let start = Instant::now();
    let max_ticks = config.max_ticks;
    let agent_id = config.agent_id.clone();
    let protocol = match config.drone_link.clone() {
        DroneLinkConfig::Simulated => {
            let transport = NullDroneLink {
                own_id: agent_id.clone(),
            };
            run_loop(
                transport,
                &agent_id,
                std::slice::from_ref(&agent_id),
                max_ticks,
                dry_run,
            )?
        }
        DroneLinkConfig::Udp { bind_addr, peers } => {
            let bind: SocketAddr =
                bind_addr
                    .parse()
                    .map_err(|error: std::net::AddrParseError| {
                        DroneAgentRuntimeError::InvalidBindAddr {
                            message: error.to_string(),
                        }
                    })?;
            let mut heartbeat_targets = Vec::new();
            let peer_map: HashMap<AgentId, SocketAddr> = peers
                .into_iter()
                .map(|(id, addr)| {
                    heartbeat_targets.push(id.clone());
                    addr.parse::<SocketAddr>()
                        .map(|socket| (id, socket))
                        .map_err(|error| DroneAgentRuntimeError::InvalidPeerAddr {
                            message: error.to_string(),
                        })
                })
                .collect::<Result<_, _>>()?;
            if heartbeat_targets.is_empty() {
                heartbeat_targets.push(agent_id.clone());
            }
            let transport =
                UdpDroneLink::bind(agent_id.clone(), bind, peer_map).map_err(|error| {
                    DroneAgentRuntimeError::UdpBind {
                        message: error.to_string(),
                    }
                })?;
            run_loop(transport, &agent_id, &heartbeat_targets, max_ticks, dry_run)?
        }
        DroneLinkConfig::InternetLikeMock { profile, seed } => {
            let transport = match profile {
                InternetLikeMockProfile::Lte => InternetLikeMock::with_lte_profile(seed),
                InternetLikeMockProfile::Satcom => InternetLikeMock::with_satcom_profile(seed),
            };
            run_internet_like_loop(transport, &agent_id, max_ticks, dry_run)?
        }
        DroneLinkConfig::Serial { .. } => {
            let transport = SerialDroneLink {
                own_id: agent_id.clone(),
            };
            run_loop(
                transport,
                &agent_id,
                std::slice::from_ref(&agent_id),
                max_ticks,
                dry_run,
            )?
        }
    };

    Ok(RunReport {
        agent_id,
        drone_link_kind: kind,
        ticks_run: max_ticks,
        dry_run,
        elapsed_ms: start.elapsed().as_millis(),
        protocol,
    })
}

pub fn run_loop<T>(
    mut transport: T,
    agent_id: &AgentId,
    heartbeat_targets: &[AgentId],
    max_ticks: u64,
    dry_run: bool,
) -> Result<DroneAgentProtocolCounters, DroneAgentRuntimeError>
where
    T: Transport,
    T::Error: std::fmt::Debug,
{
    let mut protocol = DroneAgentProtocol::new(agent_id.clone(), dry_run);
    for tick in 0..max_ticks {
        protocol.poll_incoming(&mut transport, tick)?;
        protocol.send_heartbeat(&mut transport, heartbeat_targets, tick);
        if tick == 0 || tick % 25 == 0 {
            protocol.send_presence(&mut transport, heartbeat_targets, tick);
        }
    }
    Ok(protocol.counters)
}

fn run_internet_like_loop(
    mut transport: InternetLikeMock,
    agent_id: &AgentId,
    max_ticks: u64,
    dry_run: bool,
) -> Result<DroneAgentProtocolCounters, DroneAgentRuntimeError> {
    let mut protocol = DroneAgentProtocol::new(agent_id.clone(), dry_run);
    for tick in 0..max_ticks {
        transport.advance_tick();
        protocol.poll_incoming(&mut transport, tick)?;
        protocol.send_heartbeat(&mut transport, std::slice::from_ref(agent_id), tick);
        if tick == 0 || tick % 25 == 0 {
            protocol.send_presence(&mut transport, std::slice::from_ref(agent_id), tick);
        }
    }
    Ok(protocol.counters)
}

struct DroneAgentProtocol {
    agent_id: AgentId,
    dry_run: bool,
    generation: u64,
    duplicate_suppressor: DuplicateSuppressor,
    envelope_counter: u64,
    counters: DroneAgentProtocolCounters,
}

impl DroneAgentProtocol {
    fn new(agent_id: AgentId, dry_run: bool) -> Self {
        Self {
            agent_id,
            dry_run,
            generation: 0,
            duplicate_suppressor: DuplicateSuppressor::with_default_window(),
            envelope_counter: 0,
            counters: DroneAgentProtocolCounters::default(),
        }
    }

    fn poll_incoming<T: Transport>(
        &mut self,
        transport: &mut T,
        tick: u64,
    ) -> Result<(), DroneAgentRuntimeError>
    where
        T::Error: std::fmt::Debug,
    {
        while let Some(raw) = transport
            .poll()
            .map_err(|error| DroneAgentRuntimeError::Poll {
                message: format!("{error:?}"),
            })?
        {
            let Some(envelope) = SwarmMessageEnvelope::from_raw_message(&raw) else {
                self.counters.malformed_dropped += 1;
                continue;
            };
            if self
                .duplicate_suppressor
                .is_duplicate(&envelope.envelope_id)
            {
                self.counters.duplicates_dropped += 1;
                continue;
            }
            self.counters.messages_received += 1;
            self.handle_envelope(transport, envelope, tick);
        }
        Ok(())
    }

    fn handle_envelope<T: Transport>(
        &mut self,
        transport: &mut T,
        envelope: SwarmMessageEnvelope,
        tick: u64,
    ) where
        T::Error: std::fmt::Debug,
    {
        match envelope.message {
            SwarmMessage::MissionOffer { offer_id, .. } => {
                self.counters.mission_offers_seen += 1;
                if self.dry_run {
                    let reply = SwarmMessage::MissionReject {
                        offer_id,
                        reason: MissionRejectReason::IncompatibleRole,
                    };
                    self.send_envelope(
                        transport,
                        envelope.from,
                        Some(envelope.envelope_id),
                        reply,
                        tick,
                    );
                }
            }
            SwarmMessage::StateRequest { session_id, .. } => {
                self.counters.state_requests_answered += 1;
                let reply = SwarmMessage::StateResponse {
                    mission_state: AgentMissionState::Idle,
                    active_leases: Vec::new(),
                    completed_resources: Vec::new(),
                    last_tick: tick,
                };
                self.send_envelope(transport, envelope.from, Some(session_id), reply, tick);
            }
            SwarmMessage::SegmentGrant { .. } => {
                self.counters.segment_grants_seen += 1;
            }
            SwarmMessage::SegmentDeny { .. } => {
                self.counters.segment_denies_seen += 1;
            }
            _ => {}
        }
    }

    fn send_heartbeat<T: Transport>(&mut self, transport: &mut T, targets: &[AgentId], tick: u64)
    where
        T::Error: std::fmt::Debug,
    {
        for target in targets {
            self.send_envelope(
                transport,
                target.clone(),
                None,
                SwarmMessage::Heartbeat {
                    tick,
                    generation: self.generation,
                    mission_state: AgentMissionState::Idle,
                },
                tick,
            );
        }
    }

    fn send_presence<T: Transport>(&mut self, transport: &mut T, targets: &[AgentId], tick: u64)
    where
        T::Error: std::fmt::Debug,
    {
        for target in targets {
            self.send_envelope(
                transport,
                target.clone(),
                None,
                SwarmMessage::Presence {
                    role: ProtocolRole::Observer,
                    capabilities: vec!["typed_protocol_loop".to_owned()],
                },
                tick,
            );
        }
    }

    fn send_envelope<T: Transport>(
        &mut self,
        transport: &mut T,
        to: AgentId,
        correlation_id: Option<String>,
        message: SwarmMessage,
        tick: u64,
    ) where
        T::Error: std::fmt::Debug,
    {
        self.envelope_counter += 1;
        let envelope = SwarmMessageEnvelope {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
            envelope_id: format!(
                "{}-{tick}-{}",
                self.agent_id.as_ref(),
                self.envelope_counter
            ),
            correlation_id,
            from: self.agent_id.clone(),
            to,
            sent_at: Utc::now(),
            ttl_ticks: 64,
            message,
        };
        match transport.send(envelope.into_raw_message()) {
            Ok(()) => self.counters.messages_sent += 1,
            Err(_) => self.counters.send_errors += 1,
        }
    }
}

impl From<Infallible> for DroneAgentRuntimeError {
    fn from(value: Infallible) -> Self {
        match value {}
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::rc::Rc;

    use swarm_comms::{
        InMemAgentTransport, InMemNetwork, NetworkConfig, RawMessage, SegmentDenyReason, Transport,
    };
    use swarm_types::UrbanEdgeId;

    use super::*;

    fn aid(value: &str) -> AgentId {
        AgentId::from(value.to_owned())
    }

    fn envelope(
        from: AgentId,
        to: AgentId,
        envelope_id: &str,
        message: SwarmMessage,
    ) -> RawMessage {
        SwarmMessageEnvelope {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
            envelope_id: envelope_id.to_owned(),
            correlation_id: None,
            from,
            to,
            sent_at: Utc::now(),
            ttl_ticks: 64,
            message,
        }
        .into_raw_message()
    }

    fn network() -> Rc<RefCell<InMemNetwork>> {
        Rc::new(RefCell::new(InMemNetwork::new(NetworkConfig {
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            latency_per_hop: 0,
            seed: 7,
            partitions: HashSet::new(),
            comms_jitter_ticks: 0,
        })))
    }

    #[test]
    fn typed_loop_sends_heartbeat_and_presence_envelopes() {
        let network = network();
        let agent_id = aid("agent-0");
        let peer_id = aid("peer-0");
        let transport = InMemAgentTransport::new(network.clone(), agent_id.clone());

        let counters = run_loop(
            transport,
            &agent_id,
            std::slice::from_ref(&peer_id),
            1,
            true,
        )
        .unwrap();
        let received = network.borrow_mut().drain_ready(&peer_id);

        assert_eq!(counters.messages_sent, 2);
        assert!(received.iter().any(|raw| {
            matches!(
                SwarmMessageEnvelope::from_raw_message(raw).map(|env| env.message),
                Some(SwarmMessage::Heartbeat { .. })
            )
        }));
        assert!(received.iter().any(|raw| {
            matches!(
                SwarmMessageEnvelope::from_raw_message(raw).map(|env| env.message),
                Some(SwarmMessage::Presence { .. })
            )
        }));
    }

    #[test]
    fn typed_loop_drops_duplicate_and_malformed_messages() {
        let network = network();
        let agent_id = aid("agent-0");
        let peer_id = aid("peer-0");
        network
            .borrow_mut()
            .send(envelope(
                peer_id.clone(),
                agent_id.clone(),
                "duplicate-1",
                SwarmMessage::SegmentDeny {
                    edge_id: UrbanEdgeId::from("edge-0".to_owned()),
                    to: agent_id.clone(),
                    holder: peer_id.clone(),
                    reason: SegmentDenyReason::AlreadyHeld,
                },
            ))
            .unwrap();
        network
            .borrow_mut()
            .send(envelope(
                peer_id.clone(),
                agent_id.clone(),
                "duplicate-1",
                SwarmMessage::SegmentDeny {
                    edge_id: UrbanEdgeId::from("edge-0".to_owned()),
                    to: agent_id.clone(),
                    holder: peer_id,
                    reason: SegmentDenyReason::AlreadyHeld,
                },
            ))
            .unwrap();
        network
            .borrow_mut()
            .send(RawMessage {
                from: aid("bad"),
                to: agent_id.clone(),
                payload: b"not-json".to_vec(),
            })
            .unwrap();
        let transport = InMemAgentTransport::new(network, agent_id.clone());

        let counters = run_loop(
            transport,
            &agent_id,
            std::slice::from_ref(&agent_id),
            1,
            true,
        )
        .unwrap();

        assert_eq!(counters.messages_received, 1);
        assert_eq!(counters.duplicates_dropped, 1);
        assert_eq!(counters.malformed_dropped, 1);
        assert_eq!(counters.segment_denies_seen, 1);
    }
}
