use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::Infallible;
use std::rc::Rc;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use swarm_types::AgentId;

use crate::{RawMessage, Transport};

#[derive(Clone, Debug)]
pub struct NetworkConfig {
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub seed: u64,
    pub partitions: HashSet<(AgentId, AgentId)>,
}

pub struct InMemNetwork {
    in_flight: HashMap<AgentId, VecDeque<(u64, RawMessage)>>,
    config: NetworkConfig,
    rng: SmallRng,
    current_tick: u64,
    messages_attempted: u64,
    messages_dropped: u64,
    partitions: HashSet<(AgentId, AgentId)>,
}

impl InMemNetwork {
    pub fn new(config: NetworkConfig) -> Self {
        let partitions = config.partitions.clone();
        Self {
            rng: SmallRng::seed_from_u64(config.seed),
            config,
            in_flight: HashMap::new(),
            current_tick: 0,
            messages_attempted: 0,
            messages_dropped: 0,
            partitions,
        }
    }

    pub fn advance_tick(&mut self) {
        self.current_tick += 1;
    }

    pub fn drain_ready(&mut self, recipient: &AgentId) -> Vec<RawMessage> {
        let Some(queue) = self.in_flight.get_mut(recipient) else {
            return Vec::new();
        };

        let mut ready = Vec::new();
        let mut delayed = VecDeque::new();

        while let Some((delivery_tick, message)) = queue.pop_front() {
            if delivery_tick <= self.current_tick {
                ready.push(message);
            } else {
                delayed.push_back((delivery_tick, message));
            }
        }

        *queue = delayed;
        ready
    }

    pub fn messages_attempted(&self) -> u64 {
        self.messages_attempted
    }

    pub fn messages_dropped(&self) -> u64 {
        self.messages_dropped
    }

    pub fn add_partition(&mut self, a: AgentId, b: AgentId) {
        let pair = if a.as_ref() <= b.as_ref() {
            (a, b)
        } else {
            (b, a)
        };
        self.partitions.insert(pair);
    }

    pub fn remove_partition(&mut self, a: AgentId, b: AgentId) {
        let pair = if a.as_ref() <= b.as_ref() {
            (a, b)
        } else {
            (b, a)
        };
        self.partitions.remove(&pair);
    }
}

impl Transport for InMemNetwork {
    type Error = Infallible;

    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        self.messages_attempted += 1;

        let pair = if msg.from.as_ref() <= msg.to.as_ref() {
            (msg.from.clone(), msg.to.clone())
        } else {
            (msg.to.clone(), msg.from.clone())
        };
        if self.partitions.contains(&pair) {
            self.messages_dropped += 1;
            return Ok(());
        }

        let packet_loss_rate = self.config.packet_loss_rate.clamp(0.0, 1.0);
        if self.rng.gen::<f64>() < packet_loss_rate {
            self.messages_dropped += 1;
            return Ok(());
        }

        let delivery_tick = self.current_tick + self.config.latency_ticks;
        self.in_flight
            .entry(msg.to.clone())
            .or_default()
            .push_back((delivery_tick, msg));
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
        Ok(None)
    }
}

/// Per-agent Transport wrapper over a shared InMemNetwork.
///
/// Used by ScenarioRunner: one shared bus, one wrapper per agent.
pub struct InMemAgentTransport {
    bus: Rc<RefCell<InMemNetwork>>,
    own_id: AgentId,
    buffer: VecDeque<RawMessage>,
}

impl InMemAgentTransport {
    pub fn new(bus: Rc<RefCell<InMemNetwork>>, own_id: AgentId) -> Self {
        Self {
            bus,
            own_id,
            buffer: VecDeque::new(),
        }
    }
}

impl Transport for InMemAgentTransport {
    type Error = Infallible;

    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        self.bus.borrow_mut().send(msg)
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
        if let Some(msg) = self.buffer.pop_front() {
            return Ok(Some(msg));
        }
        let mut ready = self.bus.borrow_mut().drain_ready(&self.own_id);
        if ready.is_empty() {
            return Ok(None);
        }
        let first = ready.remove(0);
        self.buffer = ready.into();
        Ok(Some(first))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message() -> RawMessage {
        RawMessage {
            from: AgentId::from("agent-0".to_owned()),
            to: AgentId::from("agent-1".to_owned()),
            payload: b"ping".to_vec(),
        }
    }

    fn make_network_config(packet_loss_rate: f64, latency_ticks: u64, seed: u64) -> NetworkConfig {
        NetworkConfig {
            packet_loss_rate,
            latency_ticks,
            seed,
            partitions: HashSet::new(),
        }
    }

    #[test]
    fn inmem_send_recv_no_loss() {
        let mut network = InMemNetwork::new(make_network_config(0.0, 0, 7));
        let recipient = AgentId::from("agent-1".to_owned());

        network.send(message()).unwrap();

        let messages = network.drain_ready(&recipient);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].payload, b"ping");
    }

    #[test]
    fn inmem_packet_loss_100pct() {
        let mut network = InMemNetwork::new(make_network_config(1.0, 0, 7));
        let recipient = AgentId::from("agent-1".to_owned());

        network.send(message()).unwrap();

        assert!(network.drain_ready(&recipient).is_empty());
        assert_eq!(network.messages_attempted(), 1);
        assert_eq!(network.messages_dropped(), 1);
    }

    #[test]
    fn inmem_latency_delays_delivery() {
        let mut network = InMemNetwork::new(make_network_config(0.0, 2, 7));
        let recipient = AgentId::from("agent-1".to_owned());

        network.send(message()).unwrap();
        assert!(network.drain_ready(&recipient).is_empty());

        network.advance_tick();
        assert!(network.drain_ready(&recipient).is_empty());

        network.advance_tick();
        assert_eq!(network.drain_ready(&recipient).len(), 1);
    }

    #[test]
    fn inmem_deterministic_seed() {
        let config = make_network_config(0.5, 0, 123);
        let recipient = AgentId::from("agent-1".to_owned());
        let mut a = InMemNetwork::new(config.clone());
        let mut b = InMemNetwork::new(config);

        for _ in 0..100 {
            a.send(message()).unwrap();
            b.send(message()).unwrap();
        }

        assert_eq!(a.messages_dropped(), b.messages_dropped());
        assert_eq!(
            a.drain_ready(&recipient).len(),
            b.drain_ready(&recipient).len()
        );
    }

    #[test]
    fn inmem_message_counters() {
        let mut network = InMemNetwork::new(make_network_config(1.0, 0, 7));

        network.send(message()).unwrap();
        network.send(message()).unwrap();

        assert_eq!(network.messages_attempted(), 2);
        assert_eq!(network.messages_dropped(), 2);
    }

    #[test]
    fn inmem_agent_poll_receives_own_messages() {
        let bus = Rc::new(RefCell::new(InMemNetwork::new(make_network_config(
            0.0, 0, 7,
        ))));
        let mut transport =
            InMemAgentTransport::new(bus.clone(), AgentId::from("agent-1".to_owned()));

        let msg = RawMessage {
            from: AgentId::from("agent-0".to_owned()),
            to: AgentId::from("agent-1".to_owned()),
            payload: b"ping".to_vec(),
        };
        bus.borrow_mut().send(msg).unwrap();

        let received = transport.poll().unwrap();
        assert!(received.is_some());
        assert_eq!(received.unwrap().from, AgentId::from("agent-0".to_owned()));
    }

    #[test]
    fn inmem_agent_poll_ignores_other_agent_messages() {
        let bus = Rc::new(RefCell::new(InMemNetwork::new(make_network_config(
            0.0, 0, 7,
        ))));
        let mut transport_a1 =
            InMemAgentTransport::new(bus.clone(), AgentId::from("agent-1".to_owned()));
        let mut transport_a2 =
            InMemAgentTransport::new(bus.clone(), AgentId::from("agent-2".to_owned()));

        let msg = RawMessage {
            from: AgentId::from("agent-0".to_owned()),
            to: AgentId::from("agent-2".to_owned()),
            payload: b"ping".to_vec(),
        };
        bus.borrow_mut().send(msg).unwrap();

        assert!(transport_a1.poll().unwrap().is_none());
        let received = transport_a2.poll().unwrap();
        assert!(received.is_some());
    }

    #[test]
    fn partition_blocks_bidirectional_traffic() {
        let mut network = InMemNetwork::new(make_network_config(0.0, 0, 7));
        let a0 = AgentId::from("agent-0".to_owned());
        let a1 = AgentId::from("agent-1".to_owned());
        network.add_partition(a0.clone(), a1.clone());

        network
            .send(RawMessage {
                from: a0.clone(),
                to: a1.clone(),
                payload: b"hi".to_vec(),
            })
            .unwrap();
        assert!(network.drain_ready(&a1).is_empty());

        network
            .send(RawMessage {
                from: a1.clone(),
                to: a0.clone(),
                payload: b"hi".to_vec(),
            })
            .unwrap();
        assert!(network.drain_ready(&a0).is_empty());
    }

    #[test]
    fn partition_removal_restores_traffic() {
        let mut network = InMemNetwork::new(make_network_config(0.0, 0, 7));
        let a0 = AgentId::from("agent-0".to_owned());
        let a1 = AgentId::from("agent-1".to_owned());
        network.add_partition(a0.clone(), a1.clone());
        network.remove_partition(a0.clone(), a1.clone());

        network
            .send(RawMessage {
                from: a0.clone(),
                to: a1.clone(),
                payload: b"hi".to_vec(),
            })
            .unwrap();
        assert_eq!(network.drain_ready(&a1).len(), 1);
    }

    #[test]
    fn non_partitioned_pairs_unaffected() {
        let mut network = InMemNetwork::new(make_network_config(0.0, 0, 7));
        let a0 = AgentId::from("agent-0".to_owned());
        let a1 = AgentId::from("agent-1".to_owned());
        let a2 = AgentId::from("agent-2".to_owned());
        network.add_partition(a0.clone(), a1.clone());

        network
            .send(RawMessage {
                from: a0.clone(),
                to: a2.clone(),
                payload: b"hi".to_vec(),
            })
            .unwrap();
        assert_eq!(network.drain_ready(&a2).len(), 1);
    }
}
