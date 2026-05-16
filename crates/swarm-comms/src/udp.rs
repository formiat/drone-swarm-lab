use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};

use swarm_types::AgentId;

use crate::{RawMessage, Transport};

#[derive(Debug, thiserror::Error)]
pub enum UdpTransportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("unknown peer: {0}")]
    UnknownPeer(AgentId),
}

pub struct UdpTransport {
    socket: UdpSocket,
    peers: HashMap<AgentId, SocketAddr>,
    recv_buf: Vec<u8>,
}

impl UdpTransport {
    pub fn new(
        bind_addr: SocketAddr,
        peers: HashMap<AgentId, SocketAddr>,
    ) -> Result<Self, UdpTransportError> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            peers,
            recv_buf: vec![0u8; 65535],
        })
    }
}

impl Transport for UdpTransport {
    type Error = UdpTransportError;

    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        let addr = self
            .peers
            .get(&msg.to)
            .ok_or_else(|| UdpTransportError::UnknownPeer(msg.to.clone()))?;
        let bytes = serde_json::to_vec(&msg)?;
        self.socket.send_to(&bytes, *addr)?;
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
        match self.socket.recv_from(&mut self.recv_buf) {
            Ok((n, _addr)) => {
                let msg: RawMessage = serde_json::from_slice(&self.recv_buf[..n])?;
                Ok(Some(msg))
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(UdpTransportError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn free_loopback_port() -> u16 {
        let sock = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        sock.local_addr().unwrap().port()
    }

    #[test]
    fn udp_send_recv_loopback() {
        let peer_id = AgentId::from("peer".to_owned());
        let sender_port = free_loopback_port();
        let recv_port = free_loopback_port();

        let mut sender = UdpTransport::new(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), sender_port),
            HashMap::from([(
                peer_id.clone(),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), recv_port),
            )]),
        )
        .unwrap();

        let mut recv = UdpTransport::new(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), recv_port),
            HashMap::new(),
        )
        .unwrap();

        let msg = RawMessage {
            from: AgentId::from("sender".to_owned()),
            to: peer_id.clone(),
            payload: b"hello".to_vec(),
        };
        sender.send(msg).unwrap();

        // Small delay for loopback delivery
        std::thread::sleep(std::time::Duration::from_millis(10));

        let received = recv.poll().unwrap();
        assert!(received.is_some());
        let r = received.unwrap();
        assert_eq!(r.from, AgentId::from("sender".to_owned()));
        assert_eq!(r.payload, b"hello");
    }

    #[test]
    fn udp_unknown_peer_returns_error() {
        let port = free_loopback_port();
        let mut transport = UdpTransport::new(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            HashMap::new(),
        )
        .unwrap();

        let msg = RawMessage {
            from: AgentId::from("sender".to_owned()),
            to: AgentId::from("unknown".to_owned()),
            payload: b"hi".to_vec(),
        };
        let result = transport.send(msg);
        assert!(matches!(result, Err(UdpTransportError::UnknownPeer(_))));
    }

    #[test]
    fn udp_poll_empty_returns_none() {
        let port = free_loopback_port();
        let mut transport = UdpTransport::new(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            HashMap::new(),
        )
        .unwrap();

        let result = transport.poll();
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn udp_multiple_messages_received() {
        let peer_id = AgentId::from("peer".to_owned());
        let sender_port = free_loopback_port();
        let recv_port = free_loopback_port();

        let mut sender = UdpTransport::new(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), sender_port),
            HashMap::from([(
                peer_id.clone(),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), recv_port),
            )]),
        )
        .unwrap();

        let mut recv = UdpTransport::new(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), recv_port),
            HashMap::new(),
        )
        .unwrap();

        for i in 0..3 {
            let msg = RawMessage {
                from: AgentId::from("sender".to_owned()),
                to: peer_id.clone(),
                payload: format!("msg{i}").into_bytes(),
            };
            sender.send(msg).unwrap();
        }

        std::thread::sleep(std::time::Duration::from_millis(10));

        let mut received = Vec::new();
        while let Ok(Some(msg)) = recv.poll() {
            received.push(String::from_utf8(msg.payload).unwrap());
        }
        assert_eq!(received.len(), 3);
        assert_eq!(received, vec!["msg0", "msg1", "msg2"]);
    }
}
