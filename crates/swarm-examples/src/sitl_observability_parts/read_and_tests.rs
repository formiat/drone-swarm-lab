pub fn read_sitl_event_log(path: impl AsRef<Path>) -> Result<SitlEventLog, SitlEventLogError> {
    let path = path.as_ref();
    let json = fs::read_to_string(path).map_err(|error| SitlEventLogError::Read {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    serde_json::from_str(&json).map_err(|error| SitlEventLogError::Deserialize {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_log() -> SitlEventLog {
        let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
            run_id: "sitl-agent-0".to_owned(),
            scenario_path: PathBuf::from("scenarios/sitl.waypoints.json"),
            scenario_name: "sitl_waypoints_0".to_owned(),
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            agent_id: "agent-0".to_owned(),
            connection_string: Some("udp:127.0.0.1:14550".to_owned()),
            mode: SitlEventLogMode::ConnectionExecute,
        });
        recorder.push_connection_opened();
        recorder.push_heartbeat_seen();
        recorder.push_mission_clear_sent();
        recorder.push_mission_count_sent(2);
        recorder.push_mission_item_requested(0);
        recorder.push_mission_item_sent(0, Some("wp-0".to_owned()));
        recorder.push_mission_item_requested(1);
        recorder.push_mission_item_sent(1, Some("wp-1".to_owned()));
        recorder.push_mission_ack_received("MAV_MISSION_ACCEPTED", true);
        recorder.push_command_sent("MAV_CMD_MISSION_START");
        recorder.push_command_ack_received("MAV_CMD_MISSION_START", "MAV_RESULT_ACCEPTED", true);
        recorder.push_current_seq_changed(1, Some("wp-1".to_owned()));
        recorder.push_waypoint_reached(1, Some("wp-1".to_owned()));
        recorder.push_task_completed(1, "wp-1");
        recorder.push_run_completed("completed");
        recorder.into_log()
    }

    #[test]
    fn event_log_roundtrips_with_snake_case_events() {
        let log = sample_log();
        let json = serde_json::to_string(&log).unwrap();

        assert!(json.contains(r#""type":"mission_item_requested""#));
        assert!(json.contains(r#""mode":"connection_execute""#));
        let restored: SitlEventLog = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, log);
    }

    #[test]
    fn writer_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("sitl-log.json");

        write_sitl_event_log(&path, &sample_log()).unwrap();

        let restored = read_sitl_event_log(path).unwrap();
        assert_eq!(restored.run_id, "sitl-agent-0");
    }

    #[test]
    fn summary_counts_upload_telemetry_and_completion_events() {
        let summary = summarize_sitl_event_log(&sample_log());

        assert_eq!(summary.mission_count_sent, 1);
        assert_eq!(summary.mission_item_requested, 2);
        assert_eq!(summary.mission_item_sent, 2);
        assert_eq!(summary.mission_ack_accepted, 1);
        assert_eq!(summary.waypoint_reached, 1);
        assert_eq!(summary.task_completed, 1);
        assert_eq!(summary.final_status, Some("completed".to_owned()));
    }

    #[test]
    fn summary_counts_failure_and_abort_events() {
        let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
            run_id: "sitl-failed".to_owned(),
            scenario_path: PathBuf::from("scenario.json"),
            scenario_name: "s".to_owned(),
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            agent_id: "agent-0".to_owned(),
            connection_string: None,
            mode: SitlEventLogMode::Mock,
        });
        recorder.push_abort_requested(Some("Accepted".to_owned()));
        recorder.push_disconnected("telemetry timeout");
        recorder.push_failure("disconnected", "telemetry timeout");

        let summary = summarize_sitl_event_log(recorder.log());

        assert_eq!(summary.abort_requested, 1);
        assert_eq!(summary.disconnected, 1);
        assert_eq!(summary.failures, 1);
        assert_eq!(summary.final_status, Some("disconnected".to_owned()));
    }

    #[test]
    fn reallocation_events_roundtrip_and_summarize() {
        let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
            run_id: "sitl-reallocation".to_owned(),
            scenario_path: PathBuf::from("scenario.json"),
            scenario_name: "s".to_owned(),
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            agent_id: "agent-0".to_owned(),
            connection_string: None,
            mode: SitlEventLogMode::Mock,
        });
        recorder.push_agent_lost("agent-1");
        recorder.push_task_released("task-0", "agent-1");
        recorder.push_task_reassigned("task-0", "agent-1", "agent-0", 0);
        recorder.push_survivor_mission_update_started(
            "agent-0",
            "mission_replacement",
            vec!["task-0".to_owned()],
        );
        recorder.push_survivor_mission_update_completed(
            "agent-0",
            "mission_replacement",
            vec!["task-0".to_owned()],
            1,
        );
        recorder.push_reallocation_completed("agent-1", 1, vec!["task-0".to_owned()], 0);
        let log = recorder.into_log();

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains(r#""type":"agent_lost""#));
        assert!(json.contains(r#""type":"task_reassigned""#));
        assert!(json.contains(r#""type":"survivor_mission_update_started""#));
        assert!(json.contains(r#""policy":"mission_replacement""#));
        assert!(json.contains(r#""type":"reallocation_completed""#));
        let restored: SitlEventLog = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, log);

        let summary = summarize_sitl_event_log(&restored);
        assert_eq!(summary.agent_lost, 1);
        assert_eq!(summary.task_released, 1);
        assert_eq!(summary.task_reassigned, 1);
        assert_eq!(summary.reallocation_completed, 1);
        assert_eq!(summary.tasks_recovered, 1);
        assert_eq!(summary.reallocation_latency_ticks, Some(0));
        assert_eq!(summary.survivor_mission_update_started, 1);
        assert_eq!(summary.survivor_mission_update_completed, 1);
        assert_eq!(summary.survivor_mission_updates, 1);
    }

    #[test]
    fn multi_agent_events_roundtrip_and_summarize() {
        let mut recorder = SitlEventRecorder::new(SitlEventLogMetadata {
            run_id: "sitl-multi".to_owned(),
            scenario_path: PathBuf::from("scenario.json"),
            scenario_name: "multi".to_owned(),
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            agent_id: "supervisor".to_owned(),
            connection_string: None,
            mode: SitlEventLogMode::ConnectionExecute,
        });
        recorder.push_multi_agent_run_started(2, "multi");
        recorder.push_multi_agent_agent_started("agent-0", "udp:127.0.0.1:14550", 1, 1);
        recorder.push_multi_agent_mission_count_sent("agent-0", 1);
        recorder.push_multi_agent_mission_item_sent("agent-0", 0, Some("wp-0".to_owned()));
        recorder.push_multi_agent_current_seq_changed("agent-0", 0, Some("wp-0".to_owned()));
        recorder.push_multi_agent_waypoint_reached("agent-0", 0, Some("wp-0".to_owned()));
        recorder.push_multi_agent_task_completed("agent-0", 0, "wp-0");
        recorder.push_multi_agent_failure("agent-1", "failed", "test failure");
        recorder.push_multi_agent_agent_finished("agent-0", "completed", 2);
        recorder.push_multi_agent_run_finished("completed");
        let log = recorder.into_log();

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains(r#""type":"multi_agent_run_started""#));
        assert!(json.contains(r#""type":"multi_agent_agent_started""#));
        assert!(json.contains(r#""type":"multi_agent_mission_count_sent""#));
        assert!(json.contains(r#""type":"multi_agent_mission_item_sent""#));
        assert!(json.contains(r#""type":"multi_agent_current_seq_changed""#));
        assert!(json.contains(r#""type":"multi_agent_waypoint_reached""#));
        assert!(json.contains(r#""type":"multi_agent_task_completed""#));
        assert!(json.contains(r#""type":"multi_agent_failure""#));
        assert!(json.contains(r#""type":"multi_agent_agent_finished""#));
        let restored: SitlEventLog = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, log);

        let summary = summarize_sitl_event_log(&restored);
        assert_eq!(summary.multi_agent_run_started, 1);
        assert_eq!(summary.multi_agent_agent_started, 1);
        assert_eq!(summary.multi_agent_agent_finished, 1);
        assert_eq!(summary.multi_agent_mission_count_sent, 1);
        assert_eq!(summary.multi_agent_mission_item_sent, 1);
        assert_eq!(summary.multi_agent_current_seq_changed, 1);
        assert_eq!(summary.multi_agent_waypoint_reached, 1);
        assert_eq!(summary.multi_agent_task_completed, 1);
        assert_eq!(summary.multi_agent_failures, 1);
        assert_eq!(summary.mission_count_sent, 1);
        assert_eq!(summary.mission_item_sent, 1);
        assert_eq!(summary.current_seq_changed, 1);
        assert_eq!(summary.waypoint_reached, 1);
        assert_eq!(summary.task_completed, 1);
        assert_eq!(summary.failures, 1);
        assert_eq!(summary.multi_agent_run_finished, 1);
        assert_eq!(summary.multi_agent_agent_count, Some(2));
        assert_eq!(summary.final_status.as_deref(), Some("completed"));
    }

    #[test]
    fn formatted_summary_is_compact_and_contains_counts() {
        let summary = summarize_sitl_event_log(&sample_log());
        let text = format_sitl_summary(&summary);

        assert!(text.contains("SITL run: sitl-agent-0"));
        assert!(text.contains("requested=2"));
        assert!(text.contains("waypoint_reached=1"));
        assert!(text.contains("Reallocation: agent_lost=0"));
        assert!(text.contains("survivor_mission_updates=0"));
        assert!(text.contains("agents_started=0"));
        assert!(text.contains("Multi-agent events: mission_count=0"));
        assert!(text.contains("final_status=completed"));
    }
}
