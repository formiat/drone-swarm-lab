#[cfg(all(test, feature = "mavlink-transport"))]
mod mission_upload_tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct FakeMissionConnection {
        incoming: VecDeque<(CommonHeader, CommonMessage)>,
        sent: Vec<CommonMessage>,
    }

    impl FakeMissionConnection {
        fn with_incoming(messages: impl IntoIterator<Item = CommonMessage>) -> Self {
            Self {
                incoming: messages
                    .into_iter()
                    .map(|message| {
                        (
                            CommonHeader {
                                system_id: 1,
                                component_id: 1,
                                sequence: 0,
                            },
                            message,
                        )
                    })
                    .collect(),
                sent: Vec::new(),
            }
        }

        fn sent(&self) -> &[CommonMessage] {
            &self.sent
        }
    }

    impl MavlinkVehicleConnection for FakeMissionConnection {
        fn send_message(&mut self, msg: CommonMessage) -> Result<(), MavlinkMissionError> {
            self.sent.push(msg);
            Ok(())
        }

        fn try_recv_message(
            &mut self,
        ) -> Result<Option<(CommonHeader, CommonMessage)>, MavlinkMissionError> {
            Ok(self.incoming.pop_front())
        }
    }

    fn options() -> MissionUploadOptions {
        MissionUploadOptions {
            timeout: Duration::from_millis(1),
            ..MissionUploadOptions::default()
        }
    }

    fn waypoint(seq: u16) -> Waypoint {
        Waypoint {
            x: 10.0 + f64::from(seq),
            y: 20.0 + f64::from(seq),
            z: 30.0,
            seq,
        }
    }

    fn heartbeat() -> CommonMessage {
        CommonMessage::HEARTBEAT(common::HEARTBEAT_DATA::default())
    }

    fn request_int(seq: u16) -> CommonMessage {
        CommonMessage::MISSION_REQUEST_INT(common::MISSION_REQUEST_INT_DATA {
            seq,
            target_system: 255,
            target_component: 0,
        })
    }

    #[allow(deprecated)]
    fn request(seq: u16) -> CommonMessage {
        CommonMessage::MISSION_REQUEST(common::MISSION_REQUEST_DATA {
            seq,
            target_system: 255,
            target_component: 0,
        })
    }

    fn ack(result: common::MavMissionResult) -> CommonMessage {
        CommonMessage::MISSION_ACK(common::MISSION_ACK_DATA {
            target_system: 255,
            target_component: 0,
            mavtype: result,
        })
    }

    fn command_ack(command: common::MavCmd, result: common::MavResult) -> CommonMessage {
        CommonMessage::COMMAND_ACK(common::COMMAND_ACK_DATA { command, result })
    }

    fn lifecycle_options() -> MissionLifecycleOptions {
        MissionLifecycleOptions {
            timeout: Duration::from_millis(1),
            ..MissionLifecycleOptions::default()
        }
    }

    fn assert_command(
        message: &CommonMessage,
        command: common::MavCmd,
    ) -> &common::COMMAND_LONG_DATA {
        let CommonMessage::COMMAND_LONG(data) = message else {
            panic!("expected COMMAND_LONG");
        };
        assert_eq!(data.command, command);
        data
    }

    fn command_long_count(conn: &FakeMissionConnection) -> usize {
        conn.sent()
            .iter()
            .filter(|message| matches!(message, CommonMessage::COMMAND_LONG(_)))
            .count()
    }

    #[derive(Default)]
    struct RecordingObserver {
        events: Vec<MavlinkMissionEvent>,
    }

    impl MavlinkMissionObserver for RecordingObserver {
        fn on_event(&mut self, event: MavlinkMissionEvent) {
            self.events.push(event);
        }
    }

    fn mission_current(seq: u16) -> CommonMessage {
        CommonMessage::MISSION_CURRENT(common::MISSION_CURRENT_DATA { seq })
    }

    fn waypoint_reached(seq: u16) -> CommonMessage {
        CommonMessage::MISSION_ITEM_REACHED(common::MISSION_ITEM_REACHED_DATA { seq })
    }

    fn unrelated_message() -> CommonMessage {
        CommonMessage::RAW_RPM(common::RAW_RPM_DATA::default())
    }

    #[test]
    fn telemetry_parser_maps_heartbeat() {
        assert_eq!(
            mavlink_message_to_telemetry_event(&heartbeat()),
            Some(MavlinkTelemetryEvent::Heartbeat)
        );
    }

    #[test]
    fn telemetry_parser_maps_mission_current() {
        assert_eq!(
            mavlink_message_to_telemetry_event(&mission_current(7)),
            Some(MavlinkTelemetryEvent::MissionCurrent { seq: 7 })
        );
    }

    #[test]
    fn telemetry_parser_maps_waypoint_reached() {
        assert_eq!(
            mavlink_message_to_telemetry_event(&waypoint_reached(3)),
            Some(MavlinkTelemetryEvent::WaypointReached { seq: 3 })
        );
    }

    #[test]
    fn telemetry_parser_maps_mission_ack_results() {
        assert_eq!(
            mavlink_message_to_telemetry_event(&ack(
                common::MavMissionResult::MAV_MISSION_ACCEPTED
            )),
            Some(MavlinkTelemetryEvent::MissionComplete)
        );
        assert_eq!(
            mavlink_message_to_telemetry_event(&ack(
                common::MavMissionResult::MAV_MISSION_INVALID_SEQUENCE
            )),
            Some(MavlinkTelemetryEvent::MissionRejected {
                reason: "MAV_MISSION_INVALID_SEQUENCE".to_owned(),
            })
        );
    }

    #[test]
    fn telemetry_poll_ignores_unrelated_messages() {
        let mut conn =
            FakeMissionConnection::with_incoming([unrelated_message(), mission_current(2)]);

        let event = poll_telemetry_event_with_connection(&mut conn).unwrap();

        assert_eq!(
            event,
            Some(MavlinkTelemetryEvent::MissionCurrent { seq: 2 })
        );
    }

    #[test]
    fn telemetry_poll_returns_none_without_event() {
        let mut conn = FakeMissionConnection::with_incoming([unrelated_message()]);

        let event = poll_telemetry_event_with_connection(&mut conn).unwrap();

        assert_eq!(event, None);
    }

    #[test]
    fn telemetry_wait_times_out_without_event() {
        let mut conn = FakeMissionConnection::default();

        let error = wait_next_telemetry_event_with_connection(&mut conn, Duration::from_millis(1))
            .unwrap_err();

        assert_eq!(
            error,
            MavlinkTelemetryError::Timeout {
                timeout: Duration::from_millis(1),
            }
        );
    }

    #[test]
    fn mission_upload_happy_path_uses_request_int() {
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            request_int(0),
            request_int(1),
            ack(common::MavMissionResult::MAV_MISSION_ACCEPTED),
        ]);

        let report =
            upload_mission_with_connection(&mut conn, &[waypoint(0), waypoint(1)], &options())
                .unwrap();

        assert_eq!(report.uploaded_count, 2);
        assert_eq!(conn.sent().len(), 4);
        assert!(matches!(
            &conn.sent()[0],
            CommonMessage::MISSION_CLEAR_ALL(_)
        ));
        assert!(matches!(
            &conn.sent()[1],
            CommonMessage::MISSION_COUNT(count) if count.count == 2
        ));
        assert!(matches!(
            &conn.sent()[2],
            CommonMessage::MISSION_ITEM_INT(item) if item.seq == 0
        ));
        assert!(matches!(
            &conn.sent()[3],
            CommonMessage::MISSION_ITEM_INT(item) if item.seq == 1
        ));
    }

    #[test]
    fn mission_upload_observer_records_handshake_events() {
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            request_int(0),
            request_int(1),
            ack(common::MavMissionResult::MAV_MISSION_ACCEPTED),
        ]);
        let mut observer = RecordingObserver::default();

        upload_mission_with_connection_observed(
            &mut conn,
            &[waypoint(0), waypoint(1)],
            &options(),
            &mut observer,
        )
        .unwrap();

        assert_eq!(
            observer.events,
            vec![
                MavlinkMissionEvent::HeartbeatSeen,
                MavlinkMissionEvent::MissionClearSent,
                MavlinkMissionEvent::MissionCountSent { count: 2 },
                MavlinkMissionEvent::MissionItemRequested { seq: 0 },
                MavlinkMissionEvent::MissionItemSent { seq: 0 },
                MavlinkMissionEvent::MissionItemRequested { seq: 1 },
                MavlinkMissionEvent::MissionItemSent { seq: 1 },
                MavlinkMissionEvent::MissionAckReceived {
                    result: "MAV_MISSION_ACCEPTED".to_owned(),
                    accepted: true,
                },
            ]
        );
    }

    #[test]
    fn mission_upload_accepts_legacy_request_fallback() {
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            request(0),
            ack(common::MavMissionResult::MAV_MISSION_ACCEPTED),
        ]);

        upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap();

        assert!(matches!(
            &conn.sent()[1],
            CommonMessage::MISSION_COUNT(count) if count.count == 1
        ));
        assert!(matches!(
            &conn.sent()[2],
            CommonMessage::MISSION_ITEM_INT(item) if item.seq == 0
        ));
    }

    #[test]
    fn mission_upload_rejects_wrong_request_sequence() {
        let mut conn = FakeMissionConnection::with_incoming([heartbeat(), request_int(7)]);

        let error =
            upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap_err();

        assert_eq!(
            error,
            MavlinkMissionError::UnexpectedRequestSeq {
                expected: 0,
                actual: 7,
            }
        );
    }

    #[test]
    fn mission_upload_reports_rejected_ack() {
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            request_int(0),
            ack(common::MavMissionResult::MAV_MISSION_INVALID_SEQUENCE),
        ]);

        let error =
            upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap_err();

        assert_eq!(
            error,
            MavlinkMissionError::MissionRejected(
                common::MavMissionResult::MAV_MISSION_INVALID_SEQUENCE
            )
        );
    }

    #[test]
    fn mission_upload_times_out_without_heartbeat() {
        let mut conn = FakeMissionConnection::default();

        let error =
            upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap_err();

        assert_eq!(error, MavlinkMissionError::HeartbeatTimeout);
    }

    #[test]
    fn mission_upload_times_out_without_request() {
        let mut conn = FakeMissionConnection::with_incoming([heartbeat()]);

        let error =
            upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap_err();

        assert_eq!(
            error,
            MavlinkMissionError::MissionRequestTimeout { expected_seq: 0 }
        );
    }

    #[test]
    fn mission_upload_times_out_without_final_ack() {
        let mut conn = FakeMissionConnection::with_incoming([heartbeat(), request_int(0)]);

        let error =
            upload_mission_with_connection(&mut conn, &[waypoint(0)], &options()).unwrap_err();

        assert_eq!(error, MavlinkMissionError::MissionAckTimeout);
    }

    #[test]
    fn mission_upload_can_skip_clear_existing() {
        let options = MissionUploadOptions {
            clear_existing: false,
            ..options()
        };
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            request_int(0),
            ack(common::MavMissionResult::MAV_MISSION_ACCEPTED),
        ]);

        let report = upload_mission_with_connection(&mut conn, &[waypoint(0)], &options).unwrap();

        assert!(!report.cleared_existing);
        assert!(matches!(
            &conn.sent()[0],
            CommonMessage::MISSION_COUNT(count) if count.count == 1
        ));
        assert!(matches!(
            &conn.sent()[1],
            CommonMessage::MISSION_ITEM_INT(item) if item.seq == 0
        ));
    }

    #[test]
    fn mission_upload_rejects_empty_mission() {
        let mut conn = FakeMissionConnection::default();

        let error = upload_mission_with_connection(&mut conn, &[], &options()).unwrap_err();

        assert_eq!(error, MavlinkMissionError::EmptyMission);
    }

    #[test]
    #[allow(deprecated)]
    fn waypoint_conversion_uses_home_origin_and_relative_altitude() {
        let options = MissionUploadOptions {
            home_origin: MissionHomeOrigin {
                lat_deg: 47.0,
                lon_deg: 8.0,
                alt_m: 5.0,
            },
            ..options()
        };
        let message = waypoint_to_mission_item_int(
            &Waypoint {
                x: 100.0,
                y: 111.32,
                z: 25.0,
                seq: 3,
            },
            &options,
        )
        .unwrap();

        let CommonMessage::MISSION_ITEM_INT(item) = message else {
            panic!("expected MISSION_ITEM_INT");
        };

        assert_eq!(item.seq, 3);
        assert_eq!(item.target_system, 1);
        assert_eq!(item.target_component, 1);
        assert_eq!(
            item.frame,
            common::MavFrame::MAV_FRAME_GLOBAL_RELATIVE_ALT_INT
        );
        assert!((f64::from(item.x) / 10_000_000.0 - 47.001).abs() < 0.000_001);
        assert!(f64::from(item.y) / 10_000_000.0 > 8.001);
        assert!((item.z - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn waypoint_conversion_rejects_unsupported_frame() {
        let options = MissionUploadOptions {
            frame: MissionFrame::LocalNed,
            ..options()
        };

        let error = waypoint_to_mission_item_int(&waypoint(0), &options).unwrap_err();

        assert_eq!(error, MavlinkMissionError::UnsupportedFrame);
    }

    #[test]
    fn command_helpers_build_expected_command_long_messages() {
        let arm = arm_command(1, 2);
        let arm = assert_command(&arm, common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM);
        assert_eq!(arm.target_system, 1);
        assert_eq!(arm.target_component, 2);
        assert_eq!(arm.param1, 1.0);

        let disarm = disarm_command(1, 2);
        let disarm = assert_command(&disarm, common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM);
        assert_eq!(disarm.param1, 0.0);

        let takeoff = takeoff_command(1, 2, 12.5);
        let takeoff = assert_command(&takeoff, common::MavCmd::MAV_CMD_NAV_TAKEOFF);
        assert!((takeoff.param7 - 12.5).abs() < f32::EPSILON);

        let start = start_mission_command(1, 2);
        assert_command(&start, common::MavCmd::MAV_CMD_MISSION_START);

        let abort = abort_command(1, 2);
        assert_command(&abort, common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH);
    }

    #[test]
    fn wait_command_ack_accepts_matching_ack_and_ignores_unrelated_messages() {
        let mut conn = FakeMissionConnection::with_incoming([
            heartbeat(),
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
        ]);

        wait_command_ack(
            &mut conn,
            common::MavCmd::MAV_CMD_NAV_TAKEOFF,
            Duration::from_millis(1),
        )
        .unwrap();
    }

    #[test]
    fn wait_command_ack_reports_rejected_result() {
        let mut conn = FakeMissionConnection::with_incoming([command_ack(
            common::MavCmd::MAV_CMD_NAV_TAKEOFF,
            common::MavResult::MAV_RESULT_DENIED,
        )]);

        let error = wait_command_ack(
            &mut conn,
            common::MavCmd::MAV_CMD_NAV_TAKEOFF,
            Duration::from_millis(1),
        )
        .unwrap_err();

        assert_eq!(
            error,
            MavlinkLifecycleError::CommandRejected {
                command: common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                result: common::MavResult::MAV_RESULT_DENIED,
                abort_result: None,
            }
        );
    }

    #[test]
    fn wait_command_ack_times_out_without_matching_ack() {
        let mut conn = FakeMissionConnection::default();

        let error = wait_command_ack(
            &mut conn,
            common::MavCmd::MAV_CMD_NAV_TAKEOFF,
            Duration::from_millis(1),
        )
        .unwrap_err();

        assert_eq!(
            error,
            MavlinkLifecycleError::CommandAckTimeout {
                command: common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                abort_result: None,
            }
        );
    }

    #[test]
    fn lifecycle_happy_path_sends_arm_takeoff_start() {
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_MISSION_START,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            heartbeat(),
        ]);

        let report =
            execute_uploaded_mission_with_connection(&mut conn, &lifecycle_options()).unwrap();

        assert!(report.armed);
        assert!(report.took_off);
        assert!(report.started);
        assert!(report.post_start_heartbeat);
        assert_eq!(command_long_count(&conn), 3);
        assert_command(
            &conn.sent()[0],
            common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
        );
        assert_command(&conn.sent()[1], common::MavCmd::MAV_CMD_NAV_TAKEOFF);
        assert_command(&conn.sent()[2], common::MavCmd::MAV_CMD_MISSION_START);
    }

    #[test]
    fn lifecycle_no_arm_skips_arm_command() {
        let mut options = lifecycle_options();
        options.no_arm = true;
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_MISSION_START,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            heartbeat(),
        ]);

        let report = execute_uploaded_mission_with_connection(&mut conn, &options).unwrap();

        assert!(!report.armed);
        assert!(report.took_off);
        assert_eq!(command_long_count(&conn), 2);
        assert_command(&conn.sent()[0], common::MavCmd::MAV_CMD_NAV_TAKEOFF);
        assert_command(&conn.sent()[1], common::MavCmd::MAV_CMD_MISSION_START);
    }

    #[test]
    fn lifecycle_arm_failure_sends_no_takeoff_or_abort() {
        let mut conn = FakeMissionConnection::with_incoming([command_ack(
            common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
            common::MavResult::MAV_RESULT_DENIED,
        )]);

        let error =
            execute_uploaded_mission_with_connection(&mut conn, &lifecycle_options()).unwrap_err();

        assert!(matches!(
            error,
            MavlinkLifecycleError::CommandRejected {
                command: common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                result: common::MavResult::MAV_RESULT_DENIED,
                abort_result: None,
            }
        ));
        assert_eq!(command_long_count(&conn), 1);
    }

    #[test]
    fn lifecycle_takeoff_failure_sends_abort() {
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_DENIED,
            ),
        ]);

        let error =
            execute_uploaded_mission_with_connection(&mut conn, &lifecycle_options()).unwrap_err();

        assert!(matches!(
            error,
            MavlinkLifecycleError::CommandRejected {
                command: common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                result: common::MavResult::MAV_RESULT_DENIED,
                abort_result: Some(AbortCommandResult::AckTimeout),
            }
        ));
        assert_eq!(command_long_count(&conn), 3);
        assert_command(
            &conn.sent()[2],
            common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
        );
    }

    #[test]
    fn lifecycle_start_failure_sends_abort() {
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_MISSION_START,
                common::MavResult::MAV_RESULT_FAILED,
            ),
        ]);

        let error =
            execute_uploaded_mission_with_connection(&mut conn, &lifecycle_options()).unwrap_err();

        assert!(matches!(
            error,
            MavlinkLifecycleError::CommandRejected {
                command: common::MavCmd::MAV_CMD_MISSION_START,
                result: common::MavResult::MAV_RESULT_FAILED,
                abort_result: Some(AbortCommandResult::AckTimeout),
            }
        ));
        assert_eq!(command_long_count(&conn), 4);
        assert_command(
            &conn.sent()[3],
            common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
        );
    }

    #[test]
    fn lifecycle_abort_after_sends_abort_after_successful_start() {
        let mut options = lifecycle_options();
        options.abort_after = Some(Duration::ZERO);
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_MISSION_START,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            heartbeat(),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
        ]);

        let report = execute_uploaded_mission_with_connection(&mut conn, &options).unwrap();

        assert_eq!(report.abort_result, Some(AbortCommandResult::Accepted));
        assert_eq!(command_long_count(&conn), 4);
        assert_command(
            &conn.sent()[3],
            common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
        );
    }

    #[test]
    fn upload_failure_in_execute_workflow_sends_no_lifecycle_commands() {
        let mut conn = FakeMissionConnection::with_incoming([heartbeat(), request_int(7)]);

        let error = upload_and_execute_mission_with_connection(
            &mut conn,
            &[waypoint(0)],
            &options(),
            &lifecycle_options(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            MavlinkFlightError::MissionUpload(MavlinkMissionError::UnexpectedRequestSeq {
                expected: 0,
                actual: 7,
            })
        ));
        assert_eq!(command_long_count(&conn), 0);
    }

    #[test]
    fn lifecycle_post_start_heartbeat_timeout_sends_abort() {
        let mut conn = FakeMissionConnection::with_incoming([
            command_ack(
                common::MavCmd::MAV_CMD_COMPONENT_ARM_DISARM,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
            command_ack(
                common::MavCmd::MAV_CMD_MISSION_START,
                common::MavResult::MAV_RESULT_ACCEPTED,
            ),
        ]);

        let error =
            execute_uploaded_mission_with_connection(&mut conn, &lifecycle_options()).unwrap_err();

        assert_eq!(
            error,
            MavlinkLifecycleError::PostStartHeartbeatTimeout {
                abort_result: AbortCommandResult::AckTimeout,
            }
        );
        assert_eq!(command_long_count(&conn), 4);
        assert_command(
            &conn.sent()[3],
            common::MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
        );
    }
}
