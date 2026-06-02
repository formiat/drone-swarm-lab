use super::*;
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
    #[cfg(feature = "mavlink-transport")]
    fn raw_mavlink_transport_send_is_explicitly_unsupported() {
        let msg = RawMessage {
            from: AgentId::from("agent-0".to_owned()),
            to: AgentId::from("sitl".to_owned()),
            payload: b"not-a-mission-upload".to_vec(),
        };

        let error = reject_raw_transport_send(msg).unwrap_err();

        assert!(matches!(error, MavlinkError::UnsupportedRawTransportSend));
        assert!(error.to_string().contains("mission upload/lifecycle APIs"));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn mavlink_connection_string_legacy_aliases_are_normalized() {
        assert_eq!(
            normalize_mavlink_connection_string("udp:127.0.0.1:14550").as_ref(),
            "udpin:127.0.0.1:14550"
        );
        assert_eq!(
            normalize_mavlink_connection_string("tcp:127.0.0.1:5760").as_ref(),
            "tcpout:127.0.0.1:5760"
        );
        assert_eq!(
            normalize_mavlink_connection_string("udpin:0.0.0.0:14550").as_ref(),
            "udpin:0.0.0.0:14550"
        );
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
            pose: Some(swarm_types::Pose {
                x: 10.0,
                y: 20.0,
                z: 3.0,
            }),
            grid_cell: None,
            edge_id: None,
            kind: None,
        };
        let wp = task_to_waypoint(&task).unwrap();
        assert!((wp.x - 10.0).abs() < 1e-6);
        assert!((wp.y - 20.0).abs() < 1e-6);
        assert!((wp.z - 3.0).abs() < 1e-6);
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
