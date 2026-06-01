#![allow(unused_imports)]
#![allow(clippy::module_inception)]
use super::*;
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "mavlink-transport")]
    use mavlink::dialects::common;
    #[cfg(feature = "mavlink-transport")]
    use std::collections::VecDeque;
    #[cfg(feature = "mavlink-transport")]
    use std::path::PathBuf;

    #[test]
    fn connection_string_validation_accepts_mavlink_udp_and_legacy_alias() {
        validate_connection_string("udpin:0.0.0.0:14550").unwrap();
        validate_connection_string("udpout:127.0.0.1:14550").unwrap();
        validate_connection_string("udp:127.0.0.1:14550").unwrap();
    }

    #[test]
    fn connection_string_validation_rejects_unknown_scheme() {
        let error = validate_connection_string("bad").unwrap_err();
        assert!(matches!(error, SitlError::BadConnectionString { .. }));
    }

    #[cfg(feature = "mavlink-transport")]
    struct FakeTelemetryRuntime {
        events: VecDeque<swarm_comms::MavlinkTelemetryEvent>,
        now: Duration,
        poll_step: Duration,
        abort_attempts: usize,
        abort_result: swarm_comms::AbortCommandResult,
    }

    #[cfg(feature = "mavlink-transport")]
    impl FakeTelemetryRuntime {
        fn new(events: impl IntoIterator<Item = swarm_comms::MavlinkTelemetryEvent>) -> Self {
            Self {
                events: events.into_iter().collect(),
                now: Duration::ZERO,
                poll_step: Duration::ZERO,
                abort_attempts: 0,
                abort_result: swarm_comms::AbortCommandResult::Accepted,
            }
        }

        fn with_poll_step(mut self, poll_step: Duration) -> Self {
            self.poll_step = poll_step;
            self
        }
    }

    #[cfg(feature = "mavlink-transport")]
    impl SitlTelemetryRuntime for FakeTelemetryRuntime {
        fn poll_telemetry_event(
            &mut self,
        ) -> Result<Option<swarm_comms::MavlinkTelemetryEvent>, swarm_comms::MavlinkTelemetryError>
        {
            self.now += self.poll_step;
            Ok(self.events.pop_front())
        }

        fn abort_mission(
            &mut self,
            _options: &swarm_comms::MissionLifecycleOptions,
        ) -> swarm_comms::AbortCommandResult {
            self.abort_attempts += 1;
            self.abort_result.clone()
        }

        fn sleep(&mut self, duration: Duration) {
            self.now += duration;
        }

        fn elapsed(&self) -> Duration {
            self.now
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn test_plan() -> SitlPlan {
        SitlPlan {
            agent_id: "agent-0".to_owned(),
            scenario_path: PathBuf::from("scenario.json"),
            suite_name: "SITL Waypoints".to_owned(),
            scenario_name: "sitl_waypoints_test".to_owned(),
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            coordinate_frame: crate::sitl_plan::SitlCoordinateFrame::LocalSimulation,
            altitude_source: "pose.z".to_owned(),
            waypoints: vec![
                crate::sitl_plan::SitlWaypointItem {
                    seq: 0,
                    task_id: "wp-0".to_owned(),
                    x: 10.0,
                    y: 20.0,
                    z: 3.0,
                },
                crate::sitl_plan::SitlWaypointItem {
                    seq: 1,
                    task_id: "wp-1".to_owned(),
                    x: 30.0,
                    y: 40.0,
                    z: 4.0,
                },
            ],
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_args(telemetry_timeout: Duration, no_progress_timeout: Duration) -> LifecycleArgs {
        LifecycleArgs {
            mode: LifecycleMode::Execute,
            no_arm: false,
            abort_after: None,
            timeout: Duration::from_millis(1),
            telemetry_timeout,
            no_progress_timeout,
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn completed_progress_report() -> crate::sitl_progress::SitlMissionProgressReport {
        crate::sitl_progress::SitlMissionProgressReport {
            final_status: crate::sitl_progress::SitlMissionFinalStatus::Completed,
            total_tasks: 2,
            completed_count: 2,
            failed_count: 0,
            current_task_id: Some("wp-1".to_owned()),
            failure_reason: None,
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn test_waypoints() -> Vec<Waypoint> {
        test_plan()
            .waypoints
            .iter()
            .map(|waypoint| Waypoint {
                x: waypoint.x,
                y: waypoint.y,
                z: waypoint.z,
                seq: waypoint.seq,
            })
            .collect()
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn sitl_mavlink_observer_records_task_id_for_mission_item_sent() {
        let plan = test_plan();
        let task_id_by_seq = sitl_task_ids_by_seq(&plan);
        let mut recorder = new_sitl_event_recorder(
            &plan,
            Some("udp:127.0.0.1:14550"),
            SitlEventLogMode::ConnectionUploadOnly,
        );
        let mut observer = SitlMavlinkObserver {
            recorder: &mut recorder,
            task_id_by_seq: &task_id_by_seq,
        };

        swarm_comms::MavlinkMissionObserver::on_event(
            &mut observer,
            swarm_comms::MavlinkMissionEvent::MissionItemSent { seq: 1 },
        );
        swarm_comms::MavlinkMissionObserver::on_event(
            &mut observer,
            swarm_comms::MavlinkMissionEvent::MissionItemSent { seq: 99 },
        );

        let mission_item_events: Vec<_> = recorder
            .log()
            .events
            .iter()
            .filter_map(|event| match event {
                crate::sitl_observability::SitlEvent::MissionItemSent { seq, task_id, .. } => {
                    Some((*seq, task_id.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            mission_item_events,
            vec![(1, Some("wp-1".to_owned())), (99, None)]
        );
    }

    #[cfg(feature = "mavlink-transport")]
    fn mission_start_success() -> SitlMissionStartReport {
        SitlMissionStartReport {
            uploaded_count: 2,
            armed: true,
            took_off: true,
            started: true,
            post_start_heartbeat: true,
            abort_result: None,
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn fake_execution_failure(
        final_status: SitlRunFinalStatus,
        error: &str,
    ) -> SitlExecutionFailure {
        SitlExecutionFailure {
            final_status,
            mission_item_count: 2,
            completed_count: 0,
            failed_count: 2,
            error: error.to_owned(),
            abort_result: None,
        }
    }

    #[cfg(feature = "mavlink-transport")]
    struct FakeGoldenPathDriver {
        start_result: Result<SitlMissionStartReport, SitlExecutionFailure>,
        telemetry_result:
            Result<crate::sitl_progress::SitlMissionProgressReport, SitlExecutionFailure>,
        upload_calls: usize,
        telemetry_calls: usize,
        last_upload_waypoint_count: Option<usize>,
        last_telemetry_mission_item_count: Option<usize>,
    }

    #[cfg(feature = "mavlink-transport")]
    impl FakeGoldenPathDriver {
        fn new(
            start_result: Result<SitlMissionStartReport, SitlExecutionFailure>,
            telemetry_result: Result<
                crate::sitl_progress::SitlMissionProgressReport,
                SitlExecutionFailure,
            >,
        ) -> Self {
            Self {
                start_result,
                telemetry_result,
                upload_calls: 0,
                telemetry_calls: 0,
                last_upload_waypoint_count: None,
                last_telemetry_mission_item_count: None,
            }
        }
    }

    #[cfg(feature = "mavlink-transport")]
    impl SitlGoldenPathDriver for FakeGoldenPathDriver {
        fn upload_and_start_mission(
            &mut self,
            waypoints: &[Waypoint],
            _upload_options: swarm_comms::MissionUploadOptions,
            _lifecycle_options: swarm_comms::MissionLifecycleOptions,
        ) -> Result<SitlMissionStartReport, SitlExecutionFailure> {
            self.upload_calls += 1;
            self.last_upload_waypoint_count = Some(waypoints.len());
            self.start_result.clone()
        }

        fn run_telemetry_progress(
            &mut self,
            _plan: &SitlPlan,
            _lifecycle: &LifecycleArgs,
            _lifecycle_options: &swarm_comms::MissionLifecycleOptions,
            mission_item_count: usize,
        ) -> Result<crate::sitl_progress::SitlMissionProgressReport, SitlExecutionFailure> {
            self.telemetry_calls += 1;
            self.last_telemetry_mission_item_count = Some(mission_item_count);
            self.telemetry_result.clone()
        }
    }

    #[cfg(feature = "mavlink-transport")]
    fn test_golden_path_run<'a>(
        plan: &'a SitlPlan,
        waypoints: &'a [Waypoint],
        lifecycle: &'a LifecycleArgs,
        run_report: Option<&'a str>,
    ) -> SitlGoldenPathRun<'a> {
        SitlGoldenPathRun {
            plan,
            waypoints,
            connection_string: "udp:127.0.0.1:14550",
            upload_options: swarm_comms::MissionUploadOptions::default(),
            lifecycle_options: swarm_comms::MissionLifecycleOptions::default(),
            lifecycle,
            run_report,
        }
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_golden_path_driver_success_writes_completed_report() {
        let plan = test_plan();
        let waypoints = test_waypoints();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let report_dir = tempfile::tempdir().unwrap();
        let report_path = report_dir.path().join("nested").join("report.json");
        let mut driver =
            FakeGoldenPathDriver::new(Ok(mission_start_success()), Ok(completed_progress_report()));

        run_golden_path_with_driver(
            &mut driver,
            test_golden_path_run(&plan, &waypoints, &lifecycle, report_path.to_str()),
        )
        .unwrap();

        assert_eq!(driver.upload_calls, 1);
        assert_eq!(driver.telemetry_calls, 1);
        assert_eq!(driver.last_upload_waypoint_count, Some(2));
        assert_eq!(driver.last_telemetry_mission_item_count, Some(2));
        let json = std::fs::read_to_string(report_path).unwrap();
        let report: SitlRunReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.final_status, SitlRunFinalStatus::Completed);
        assert_eq!(report.completed_count, 2);
        assert_eq!(report.failed_count, 0);
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_golden_path_driver_upload_failure_writes_error_report() {
        let plan = test_plan();
        let waypoints = test_waypoints();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let report_dir = tempfile::tempdir().unwrap();
        let report_path = report_dir.path().join("report.json");
        let failure =
            fake_execution_failure(SitlRunFinalStatus::Rejected, "mission rejected by vehicle");
        let mut driver = FakeGoldenPathDriver::new(Err(failure), Ok(completed_progress_report()));

        let error = run_golden_path_with_driver(
            &mut driver,
            test_golden_path_run(&plan, &waypoints, &lifecycle, report_path.to_str()),
        )
        .unwrap_err();

        assert!(matches!(error, SitlError::ConnectionFailed { .. }));
        assert_eq!(driver.upload_calls, 1);
        assert_eq!(driver.telemetry_calls, 0);
        let json = std::fs::read_to_string(report_path).unwrap();
        let report: SitlRunReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.final_status, SitlRunFinalStatus::Rejected);
        assert_eq!(report.error, Some("mission rejected by vehicle".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_golden_path_driver_lifecycle_abort_writes_aborted_report() {
        let plan = test_plan();
        let waypoints = test_waypoints();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let report_dir = tempfile::tempdir().unwrap();
        let report_path = report_dir.path().join("report.json");
        let mut start = mission_start_success();
        start.abort_result = Some(swarm_comms::AbortCommandResult::Accepted);
        let mut driver = FakeGoldenPathDriver::new(Ok(start), Ok(completed_progress_report()));

        let error = run_golden_path_with_driver(
            &mut driver,
            test_golden_path_run(&plan, &waypoints, &lifecycle, report_path.to_str()),
        )
        .unwrap_err();

        assert!(matches!(error, SitlError::ConnectionFailed { .. }));
        assert_eq!(driver.upload_calls, 1);
        assert_eq!(driver.telemetry_calls, 0);
        let json = std::fs::read_to_string(report_path).unwrap();
        let report: SitlRunReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.final_status, SitlRunFinalStatus::Aborted);
        assert_eq!(report.abort_result, Some("Accepted".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_golden_path_driver_telemetry_failure_writes_error_report() {
        let plan = test_plan();
        let waypoints = test_waypoints();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let report_dir = tempfile::tempdir().unwrap();
        let report_path = report_dir.path().join("report.json");
        let mut failure = fake_execution_failure(
            SitlRunFinalStatus::TimedOutNoProgress,
            "no mission progress before 60s",
        );
        failure.completed_count = 1;
        failure.failed_count = 1;
        failure.abort_result = Some("Accepted".to_owned());
        let mut driver = FakeGoldenPathDriver::new(Ok(mission_start_success()), Err(failure));

        let error = run_golden_path_with_driver(
            &mut driver,
            test_golden_path_run(&plan, &waypoints, &lifecycle, report_path.to_str()),
        )
        .unwrap_err();

        assert!(matches!(error, SitlError::ConnectionFailed { .. }));
        assert_eq!(driver.upload_calls, 1);
        assert_eq!(driver.telemetry_calls, 1);
        let json = std::fs::read_to_string(report_path).unwrap();
        let report: SitlRunReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.final_status, SitlRunFinalStatus::TimedOutNoProgress);
        assert_eq!(report.completed_count, 1);
        assert_eq!(report.failed_count, 1);
        assert_eq!(report.abort_result, Some("Accepted".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_golden_path_summary_builds_success_report() {
        let plan = test_plan();

        let report = success_run_report(&plan, "udp:127.0.0.1:14550", &completed_progress_report());

        assert_eq!(report.scenario_name, "sitl_waypoints_test");
        assert_eq!(report.agent_id, "agent-0");
        assert_eq!(report.mission_item_count, 2);
        assert_eq!(report.completed_count, 2);
        assert_eq!(report.failed_count, 0);
        assert_eq!(report.final_status, SitlRunFinalStatus::Completed);
        assert_eq!(report.error, None);
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn fake_failure_summary_builds_error_report() {
        let plan = test_plan();
        let failure = SitlExecutionFailure {
            final_status: SitlRunFinalStatus::Disconnected,
            mission_item_count: 2,
            completed_count: 1,
            failed_count: 1,
            error: "telemetry disconnected".to_owned(),
            abort_result: Some("Accepted".to_owned()),
        };

        let report = failure_run_report(&plan, "udp:127.0.0.1:14550", &failure);

        assert_eq!(report.final_status, SitlRunFinalStatus::Disconnected);
        assert_eq!(report.completed_count, 1);
        assert_eq!(report.failed_count, 1);
        assert_eq!(report.error, Some("telemetry disconnected".to_owned()));
        assert_eq!(report.abort_result, Some("Accepted".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn upload_failure_maps_to_failed_report_failure() {
        let error = swarm_comms::MavlinkFlightError::MissionUpload(
            swarm_comms::MavlinkMissionError::MissionRequestTimeout { expected_seq: 1 },
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(failure.mission_item_count, 2);
        assert_eq!(failure.completed_count, 0);
        assert!(failure.error.contains("mission upload failed"));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_failure_maps_to_failed_report_failure() {
        let error = swarm_comms::MavlinkFlightError::Lifecycle(
            swarm_comms::MavlinkLifecycleError::ReadFailed("ack read failed".to_owned()),
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(failure.mission_item_count, 2);
        assert!(failure.error.contains("mission lifecycle failed"));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_command_ack_timeout_keeps_abort_result() {
        let error = swarm_comms::MavlinkFlightError::Lifecycle(
            swarm_comms::MavlinkLifecycleError::CommandAckTimeout {
                command: common::MavCmd::MAV_CMD_NAV_TAKEOFF,
                abort_result: Some(swarm_comms::AbortCommandResult::Accepted),
            },
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(failure.abort_result, Some("Accepted".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_command_rejected_keeps_abort_failure() {
        let error = swarm_comms::MavlinkFlightError::Lifecycle(
            swarm_comms::MavlinkLifecycleError::CommandRejected {
                command: common::MavCmd::MAV_CMD_MISSION_START,
                result: common::MavResult::MAV_RESULT_DENIED,
                abort_result: Some(swarm_comms::AbortCommandResult::Failed(
                    "rtl write failed".to_owned(),
                )),
            },
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(
            failure.abort_result,
            Some("Failed(\"rtl write failed\")".to_owned())
        );
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_post_start_timeout_keeps_abort_result() {
        let error = swarm_comms::MavlinkFlightError::Lifecycle(
            swarm_comms::MavlinkLifecycleError::PostStartHeartbeatTimeout {
                abort_result: swarm_comms::AbortCommandResult::Accepted,
            },
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(failure.abort_result, Some("Accepted".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn lifecycle_abort_failed_keeps_abort_result() {
        let error = swarm_comms::MavlinkFlightError::Lifecycle(
            swarm_comms::MavlinkLifecycleError::AbortFailed {
                abort_result: swarm_comms::AbortCommandResult::AckTimeout,
            },
        );

        let failure = flight_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::Failed);
        assert_eq!(failure.abort_result, Some("AckTimeout".to_owned()));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn telemetry_failure_keeps_progress_counts_and_abort_result() {
        let progress_report = crate::sitl_progress::SitlMissionProgressReport {
            final_status: crate::sitl_progress::SitlMissionFinalStatus::TimedOutNoProgress,
            total_tasks: 2,
            completed_count: 1,
            failed_count: 1,
            current_task_id: Some("wp-1".to_owned()),
            failure_reason: Some("no mission progress before 60s".to_owned()),
        };
        let error = SitlTelemetryLoopError::Failed {
            report: progress_report,
            abort_result: swarm_comms::AbortCommandResult::Accepted,
        };

        let failure = telemetry_error_to_execution_failure(2, error);

        assert_eq!(failure.final_status, SitlRunFinalStatus::TimedOutNoProgress);
        assert_eq!(failure.completed_count, 1);
        assert_eq!(failure.failed_count, 1);
        assert_eq!(failure.abort_result, Some("Accepted".to_owned()));
        assert_eq!(failure.error, "no mission progress before 60s");
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn report_write_failure_returns_typed_error() {
        let dir = tempfile::tempdir().unwrap();
        let plan = test_plan();
        let report = success_run_report(&plan, "udp:127.0.0.1:14550", &completed_progress_report());

        let error = write_run_report_if_requested(dir.path().to_str(), &report).unwrap_err();

        match error {
            SitlError::RunReportWrite { path, message } => {
                assert_eq!(path, dir.path());
                assert!(!message.is_empty());
            }
            error => panic!("expected run report write error, got {error:?}"),
        }
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn telemetry_loop_recorder_tracks_terminal_waypoint_completion() {
        let plan = test_plan();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let lifecycle_options = swarm_comms::MissionLifecycleOptions::default();
        let mut runtime = FakeTelemetryRuntime::new([
            swarm_comms::MavlinkTelemetryEvent::Heartbeat,
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 1 },
        ]);
        let mut recorder = new_sitl_event_recorder(
            &plan,
            Some("udp:127.0.0.1:14550"),
            SitlEventLogMode::ConnectionExecute,
        );

        let report = run_telemetry_progress_loop_with_runtime(
            &mut runtime,
            &plan,
            &lifecycle,
            &lifecycle_options,
            Some(&mut recorder),
        )
        .unwrap();

        assert_eq!(report.completed_count, 2);
        let summary = crate::sitl_observability::summarize_sitl_event_log(recorder.log());
        assert_eq!(summary.waypoint_reached, 2);
        assert_eq!(summary.task_completed, 2);
        let final_waypoint_task_id =
            recorder
                .log()
                .events
                .iter()
                .rev()
                .find_map(|event| match event {
                    crate::sitl_observability::SitlEvent::WaypointReached {
                        seq, task_id, ..
                    } if *seq == 1 => Some(task_id.clone()),
                    _ => None,
                });
        assert_eq!(final_waypoint_task_id, Some(Some("wp-1".to_owned())));
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn telemetry_loop_recorder_logs_current_seq_only_on_change() {
        let plan = test_plan();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(30));
        let lifecycle_options = swarm_comms::MissionLifecycleOptions::default();
        let mut runtime = FakeTelemetryRuntime::new([
            swarm_comms::MavlinkTelemetryEvent::MissionCurrent { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::MissionCurrent { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::MissionCurrent { seq: 1 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 1 },
        ]);
        let mut recorder = new_sitl_event_recorder(
            &plan,
            Some("udp:127.0.0.1:14550"),
            SitlEventLogMode::ConnectionExecute,
        );

        run_telemetry_progress_loop_with_runtime(
            &mut runtime,
            &plan,
            &lifecycle,
            &lifecycle_options,
            Some(&mut recorder),
        )
        .unwrap();

        let summary = crate::sitl_observability::summarize_sitl_event_log(recorder.log());
        assert_eq!(summary.current_seq_changed, 2);
        let current_seq_events: Vec<_> = recorder
            .log()
            .events
            .iter()
            .filter_map(|event| match event {
                crate::sitl_observability::SitlEvent::CurrentSeqChanged {
                    seq, task_id, ..
                } => Some((*seq, task_id.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            current_seq_events,
            vec![(0, Some("wp-0".to_owned())), (1, Some("wp-1".to_owned()))]
        );
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn telemetry_loop_duplicate_reached_does_not_reset_no_progress_timeout() {
        let plan = test_plan();
        let lifecycle = lifecycle_args(Duration::from_secs(30), Duration::from_secs(3));
        let lifecycle_options = swarm_comms::MissionLifecycleOptions::default();
        let mut runtime = FakeTelemetryRuntime::new([
            swarm_comms::MavlinkTelemetryEvent::Heartbeat,
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
            swarm_comms::MavlinkTelemetryEvent::WaypointReached { seq: 0 },
        ])
        .with_poll_step(Duration::from_secs(1));

        let error = run_telemetry_progress_loop_with_runtime(
            &mut runtime,
            &plan,
            &lifecycle,
            &lifecycle_options,
            None,
        )
        .unwrap_err();

        let SitlTelemetryLoopError::Failed {
            report,
            abort_result,
        } = error
        else {
            panic!("expected telemetry loop failure");
        };
        assert_eq!(
            report.final_status,
            crate::sitl_progress::SitlMissionFinalStatus::TimedOutNoProgress
        );
        assert_eq!(report.completed_count, 1);
        assert_eq!(report.failed_count, 1);
        assert_eq!(abort_result, swarm_comms::AbortCommandResult::Accepted);
        assert_eq!(runtime.abort_attempts, 1);
    }

    #[test]
    #[cfg(feature = "mavlink-transport")]
    fn telemetry_loop_disconnect_timeout_attempts_abort() {
        let plan = test_plan();
        let lifecycle = lifecycle_args(Duration::from_millis(20), Duration::from_secs(30));
        let lifecycle_options = swarm_comms::MissionLifecycleOptions::default();
        let mut runtime = FakeTelemetryRuntime::new([]);

        let error = run_telemetry_progress_loop_with_runtime(
            &mut runtime,
            &plan,
            &lifecycle,
            &lifecycle_options,
            None,
        )
        .unwrap_err();

        let SitlTelemetryLoopError::Failed {
            report,
            abort_result,
        } = error
        else {
            panic!("expected telemetry loop failure");
        };
        assert_eq!(
            report.final_status,
            crate::sitl_progress::SitlMissionFinalStatus::Disconnected
        );
        assert_eq!(report.completed_count, 0);
        assert_eq!(report.failed_count, 2);
        assert_eq!(abort_result, swarm_comms::AbortCommandResult::Accepted);
        assert_eq!(runtime.abort_attempts, 1);
    }
}
