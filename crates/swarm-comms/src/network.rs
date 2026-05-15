use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use swarm_types::AgentId;

use crate::{RawMessage, Transport};

#[derive(Clone, Debug)]
pub struct NetworkConfig {
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub seed: u64,
}

pub struct InMemNetwork {
    in_flight: HashMap<AgentId, VecDeque<(u64, RawMessage)>>,
    config: NetworkConfig,
    rng: SmallRng,
    current_tick: u64,
    messages_attempted: u64,
    messages_dropped: u64,
}

impl InMemNetwork {
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            rng: SmallRng::seed_from_u64(config.seed),
            config,
            in_flight: HashMap::new(),
            current_tick: 0,
            messages_attempted: 0,
            messages_dropped: 0,
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
}

impl Transport for InMemNetwork {
    type Error = Infallible;

    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        self.messages_attempted += 1;

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

    #[test]
    fn inmem_send_recv_no_loss() {
        let mut network = InMemNetwork::new(NetworkConfig {
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            seed: 7,
        });
        let recipient = AgentId::from("agent-1".to_owned());

        network.send(message()).unwrap();

        let messages = network.drain_ready(&recipient);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].payload, b"ping");
    }

    #[test]
    fn inmem_packet_loss_100pct() {
        let mut network = InMemNetwork::new(NetworkConfig {
            packet_loss_rate: 1.0,
            latency_ticks: 0,
            seed: 7,
        });
        let recipient = AgentId::from("agent-1".to_owned());

        network.send(message()).unwrap();

        assert!(network.drain_ready(&recipient).is_empty());
        assert_eq!(network.messages_attempted(), 1);
        assert_eq!(network.messages_dropped(), 1);
    }

    #[test]
    fn inmem_latency_delays_delivery() {
        let mut network = InMemNetwork::new(NetworkConfig {
            packet_loss_rate: 0.0,
            latency_ticks: 2,
            seed: 7,
        });
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
        let config = NetworkConfig {
            packet_loss_rate: 0.5,
            latency_ticks: 0,
            seed: 123,
        };
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
        let mut network = InMemNetwork::new(NetworkConfig {
            packet_loss_rate: 1.0,
            latency_ticks: 0,
            seed: 7,
        });

        network.send(message()).unwrap();
        network.send(message()).unwrap();

        assert_eq!(network.messages_attempted(), 2);
        assert_eq!(network.messages_dropped(), 2);
    }
}
