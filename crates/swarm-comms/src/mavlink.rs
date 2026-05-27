use std::collections::VecDeque;

use swarm_types::TaskStatus;

use crate::{RawMessage, Transport};

#[derive(Debug, thiserror::Error)]
pub enum MavlinkError {
    #[error("mavlink connection error: {0}")]
    Connection(String),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("transport not connected")]
    NotConnected,
    #[error("no pose on task")]
    NoPose,
}

/// A waypoint in local coordinate space (no MAVLink dependency).
#[derive(Debug, Clone)]
pub struct Waypoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub seq: u16,
}

/// Convert a Task to a Waypoint.
pub fn task_to_waypoint(task: &swarm_types::Task) -> Option<Waypoint> {
    task.pose.map(|pose| Waypoint {
        x: pose.x,
        y: pose.y,
        z: 0.0,
        seq: 0,
    })
}

/// Derive TaskStatus from a boolean acknowledgement flag (mock path).
pub fn waypoint_status_to_task_status(ack: bool) -> TaskStatus {
    if ack {
        TaskStatus::Completed
    } else {
        TaskStatus::InProgress
    }
}

/// Mock MAVLink transport for unit tests and --mock mode.
pub struct MockMavlinkTransport {
    sent: Vec<RawMessage>,
    inbox: VecDeque<RawMessage>,
    waypoints: Vec<Waypoint>,
}

impl MockMavlinkTransport {
    pub fn new() -> Self {
        Self {
            sent: Vec::new(),
            inbox: VecDeque::new(),
            waypoints: Vec::new(),
        }
    }

    pub fn sent_messages(&self) -> &[RawMessage] {
        &self.sent
    }

    pub fn push_incoming(&mut self, msg: RawMessage) {
        self.inbox.push_back(msg);
    }

    pub fn waypoints(&self) -> &[Waypoint] {
        &self.waypoints
    }

    pub fn send_waypoint(&mut self, wp: Waypoint) {
        self.waypoints.push(wp);
    }
}

impl Default for MockMavlinkTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for MockMavlinkTransport {
    type Error = MavlinkError;

    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        self.sent.push(msg);
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
        Ok(self.inbox.pop_front())
    }
}

/// Wraps a MAVLink connection for use with the swarm Transport trait.
/// Only available with feature "mavlink-transport".
#[cfg(feature = "mavlink-transport")]
pub struct MavlinkTransport {
    conn: mavlink::Connection<mavlink::dialects::common::MavMessage>,
    agent_id: swarm_types::AgentId,
    recv_buf: VecDeque<RawMessage>,
}

#[cfg(feature = "mavlink-transport")]
impl MavlinkTransport {
    pub fn new(
        connection_string: &str,
        agent_id: swarm_types::AgentId,
    ) -> Result<Self, MavlinkError> {
        let conn: mavlink::Connection<mavlink::dialects::common::MavMessage> =
            mavlink::connect(connection_string)
                .map_err(|e: std::io::Error| MavlinkError::Connection(e.to_string()))?;
        Ok(Self {
            conn,
            agent_id,
            recv_buf: VecDeque::new(),
        })
    }
}

#[cfg(feature = "mavlink-transport")]
impl Transport for MavlinkTransport {
    type Error = MavlinkError;

    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        use mavlink::MavConnection;
        let _bytes = serde_json::to_vec(&msg)?;
        self.conn
            .send_default(&mavlink::dialects::common::MavMessage::RAW_RPM(
                mavlink::dialects::common::RAW_RPM_DATA::default(),
            ))
            .map_err(|e: mavlink::error::MessageWriteError| {
                MavlinkError::Connection(e.to_string())
            })?;
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
        use mavlink::MavConnection;
        if let Some(msg) = self.recv_buf.pop_front() {
            return Ok(Some(msg));
        }
        match self.conn.try_recv() {
            Ok((_header, mav_msg)) => {
                let result = RawMessage {
                    from: self.agent_id.clone(),
                    to: self.agent_id.clone(),
                    payload: serde_json::to_vec(&format!("{mav_msg:?}"))?,
                };
                self.recv_buf.push_back(result);
                Ok(self.recv_buf.pop_front())
            }
            Err(e) => Err(MavlinkError::Connection(e.to_string())),
        }
    }
}

/// Convert a Task to a MAVLink mission item int message (requires mavlink feature).
#[allow(deprecated)]
#[cfg(feature = "mavlink-transport")]
pub fn task_to_mavlink_waypoint(
    task: &swarm_types::Task,
    seq: u16,
    target_system: u8,
    target_component: u8,
) -> Option<mavlink::dialects::common::MavMessage> {
    let pose = task.pose?;
    let lat = (pose.x * 1.0e-7) as i32;
    let lon = (pose.y * 1.0e-7) as i32;
    Some(mavlink::dialects::common::MavMessage::MISSION_ITEM_INT(
        mavlink::dialects::common::MISSION_ITEM_INT_DATA {
            param1: 0.0,
            param2: 0.0,
            param3: 0.0,
            param4: 0.0,
            x: lat,
            y: lon,
            z: 0.0,
            seq,
            command: mavlink::dialects::common::MavCmd::MAV_CMD_NAV_WAYPOINT,
            target_system,
            target_component,
            frame: mavlink::dialects::common::MavFrame::MAV_FRAME_GLOBAL,
            current: if seq == 0 { 1 } else { 0 },
            autocontinue: 1,
        },
    ))
}

/// Convert a MAVLink message to a TaskStatus (requires mavlink feature).
#[cfg(feature = "mavlink-transport")]
pub fn mavlink_status_to_task_status(
    msg: &mavlink::dialects::common::MavMessage,
) -> Option<TaskStatus> {
    match msg {
        mavlink::dialects::common::MavMessage::MISSION_ACK(ack) => {
            if ack.mavtype == mavlink::dialects::common::MavMissionResult::MAV_MISSION_ACCEPTED {
                Some(TaskStatus::Completed)
            } else {
                Some(TaskStatus::Failed)
            }
        }
        mavlink::dialects::common::MavMessage::HEARTBEAT(_) => Some(TaskStatus::InProgress),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::{AgentId, Task, TaskId, TaskStatus};

    #[test]
    fn mock_mavlink_send_poll_roundtrip() {
        let mut transport = MockMavlinkTransport::new();
        let msg = RawMessage {
            from: AgentId::from("agent-0".to_owned()),
            to: AgentId::from("sitl".to_owned()),
            payload: b"hello".to_vec(),
        };
        transport.send(msg.clone()).unwrap();
        assert_eq!(transport.sent_messages().len(), 1);
        assert_eq!(transport.sent_messages()[0].payload, b"hello");
    }

    #[test]
    fn mock_mavlink_poll_returns_pushed() {
        let mut transport = MockMavlinkTransport::new();
        let msg = RawMessage {
            from: AgentId::from("sitl".to_owned()),
            to: AgentId::from("agent-0".to_owned()),
            payload: b"ack".to_vec(),
        };
        transport.push_incoming(msg.clone());
        let polled = transport.poll().unwrap().unwrap();
        assert_eq!(polled.payload, b"ack");
    }

    #[test]
    fn mock_mavlink_poll_empty_returns_none() {
        let mut transport = MockMavlinkTransport::new();
        assert!(transport.poll().unwrap().is_none());
    }

    #[test]
    fn task_to_waypoint_with_pose() {
        let task = Task {
            id: TaskId::from("t1".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(swarm_types::Pose { x: 10.0, y: 20.0 , ..Default::default()}),
            grid_cell: None,
            edge_id: None,
            kind: None,
        };
        let wp = task_to_waypoint(&task).unwrap();
        assert!((wp.x - 10.0).abs() < 1e-6);
        assert!((wp.y - 20.0).abs() < 1e-6);
    }

    #[test]
    fn task_to_waypoint_no_pose() {
        let task = Task {
            id: TaskId::from("t1".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
            grid_cell: None,
            edge_id: None,
            kind: None,
        };
        assert!(task_to_waypoint(&task).is_none());
    }

    #[test]
    fn waypoint_status_to_task_status_completed() {
        assert_eq!(waypoint_status_to_task_status(true), TaskStatus::Completed);
    }

    #[test]
    fn waypoint_status_to_task_status_in_progress() {
        assert_eq!(
            waypoint_status_to_task_status(false),
            TaskStatus::InProgress
        );
    }
}
