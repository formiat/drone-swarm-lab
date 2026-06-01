#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_log::EventLogBuilder;
    use swarm_types::{AgentId, Pose, TaskId, UrbanEdgeId, UrbanNodeId};

    #[test]
    fn replay_reconstructs_state() {
        let mut builder = EventLogBuilder::new("test", 0, "scenario");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::MessageSent {
            from: AgentId::from("a0".to_owned()),
            to: AgentId::from("a1".to_owned()),
            tick: 1,
            payload_len: 10,
        });
        builder.push(Event::TaskAssigned {
            task_id: TaskId::from("t0".to_owned()),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 2,
        });
        builder.push(Event::AgentFailed {
            agent_id: AgentId::from("a1".to_owned()),
            tick: 3,
        });

        let log = builder.build();
        let state = replay(&log);

        assert_eq!(state.messages_sent, 1);
        assert_eq!(state.assigned_tasks.len(), 1);
        assert_eq!(state.failed_agents.len(), 1);
    }

    #[test]
    fn summarize_counts_events_correctly() {
        let mut builder = EventLogBuilder::new("test", 0, "s");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::TickStart { tick: 1 });
        builder.push(Event::TaskAssigned {
            task_id: TaskId::from("t0".to_owned()),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 0,
        });
        builder.push(Event::TaskCompleted {
            task_id: TaskId::from("t0".to_owned()),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 1,
        });
        builder.push(Event::AgentFailed {
            agent_id: AgentId::from("a1".to_owned()),
            tick: 1,
        });
        builder.push(Event::SarScan {
            agent_id: AgentId::from("a0".to_owned()),
            cell: (0, 0),
            tick: 0,
            detected: false,
        });
        builder.push(Event::SarDetection {
            agent_id: AgentId::from("a0".to_owned()),
            target_pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            tick: 1,
        });
        builder.push(Event::EdgeVisited {
            edge_id: "e0".to_owned(),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 1,
        });
        builder.push(Event::SafetyViolation {
            agent_id: AgentId::from("a0".to_owned()),
            violation_type: crate::event_log::ViolationType::NoFly,
            tick: 1,
        });
        builder.push(Event::CbbaConverged { tick: 1 });
        builder.push(Event::MessageSent {
            from: AgentId::from("a0".to_owned()),
            to: AgentId::from("a1".to_owned()),
            tick: 0,
            payload_len: 10,
        });
        builder.push(Event::MessageDropped {
            from: AgentId::from("a0".to_owned()),
            to: AgentId::from("a1".to_owned()),
            tick: 0,
            reason: crate::event_log::DropReason::PacketLoss,
        });

        let log = builder.build();
        let s = summarize(&log);
        assert_eq!(s.total_ticks, 1);
        assert_eq!(s.assignments, 1);
        assert_eq!(s.completions, 1);
        assert_eq!(s.failures, 1);
        assert_eq!(s.sar_scans, 1);
        assert_eq!(s.sar_detections, 1);
        assert_eq!(s.edges_visited, 1);
        assert_eq!(s.safety_violations, 1);
        assert_eq!(s.cbba_convergence_ticks, vec![1]);
        assert_eq!(s.messages_sent, 1);
        assert_eq!(s.messages_dropped, 1);
    }

    #[test]
    fn summarize_counts_urban_events() {
        let mut builder = EventLogBuilder::new("urban", 0, "urban_patrol_small_block");
        let agent_id = AgentId::from("agent-0".to_owned());
        let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
        let bus_id = swarm_types::UrbanBusId::from("bus-0".to_owned());
        builder.push(Event::UrbanRoutePlanned {
            agent_id: agent_id.clone(),
            tick: 0,
            edge_ids: vec![edge_id.clone()],
            route_length_m: 20.0,
        });
        builder.push(Event::UrbanSegmentEntered {
            agent_id: agent_id.clone(),
            tick: 0,
            segment_index: 0,
            edge_id: edge_id.clone(),
            from: UrbanNodeId::from("n0".to_owned()),
            to: UrbanNodeId::from("n1".to_owned()),
        });
        builder.push(Event::UrbanSegmentCompleted {
            agent_id: agent_id.clone(),
            tick: 10,
            segment_index: 0,
            edge_id: edge_id.clone(),
        });
        builder.push(Event::UrbanViolation {
            agent_id: agent_id.clone(),
            tick: 11,
            segment_index: Some(0),
            edge_id: Some(edge_id),
            obstacle_id: None,
            pose: Pose::default(),
            reason: "test".to_owned(),
        });
        builder.push(Event::UrbanPatrolCompleted {
            agent_id: agent_id.clone(),
            tick: 12,
            route_length_m: 20.0,
            distance_travelled_m: 20.0,
        });
        builder.push(Event::BusObserved {
            agent_id: agent_id.clone(),
            tick: 13,
            bus_id: bus_id.clone(),
            pose: Pose::default(),
            distance_m: 1.0,
            detector_seed: 9,
        });
        builder.push(Event::BusDetected {
            agent_id: agent_id.clone(),
            tick: 13,
            bus_id: bus_id.clone(),
            pose: Pose::default(),
            distance_m: 1.0,
            detector_seed: 9,
        });
        builder.push(Event::BusFalsePositive {
            agent_id: agent_id.clone(),
            tick: 14,
            pose: Pose::default(),
            detector_seed: 9,
        });
        builder.push(Event::UrbanSearchCompleted {
            agent_id: agent_id.clone(),
            tick: 13,
            detected: true,
            bus_id: Some(bus_id),
            reason: "detected".to_owned(),
            distance_travelled_m: 10.0,
        });
        builder.push(Event::UrbanSearchCompleted {
            agent_id,
            tick: 20,
            detected: false,
            bus_id: None,
            reason: "timeout".to_owned(),
            distance_travelled_m: 40.0,
        });

        let summary = summarize(&builder.build());
        assert_eq!(summary.urban_routes_planned, 1);
        assert_eq!(summary.urban_segments_entered, 1);
        assert_eq!(summary.urban_segments_completed, 1);
        assert_eq!(summary.urban_violations, 1);
        assert_eq!(summary.urban_patrol_completions, 1);
        assert_eq!(summary.urban_completion_ticks, vec![12]);
        assert_eq!(summary.bus_observations, 1);
        assert_eq!(summary.bus_detections, 1);
        assert_eq!(summary.bus_false_positives, 1);
        assert_eq!(summary.urban_search_completions, 2);
        assert_eq!(summary.urban_search_time_to_detection_ticks, vec![13]);
        assert_eq!(summary.urban_search_no_detection_count, 1);
    }

    #[test]
    fn timeline_output_is_deterministic() {
        let log = timeline_fixture();
        let timeline = format_timeline(&log, &ReplayTimelineFilter::default());

        assert!(timeline.contains("tick=00000 category=generic agent=- event=TickStart"));
        assert!(timeline.contains(
            "tick=00000 category=urban agent=agent-0 event=UrbanRoutePlanned edges=1 route_length_m=20.000"
        ));
        assert!(timeline.contains(
            "tick=00001 category=urban agent=agent-0 event=UrbanSegmentCompleted segment=0 edge=road-n0-n1"
        ));
        let first_urban = timeline.find("UrbanRoutePlanned").unwrap();
        let completed = timeline.find("UrbanSegmentCompleted").unwrap();
        assert!(first_urban < completed);
    }

    #[test]
    fn timeline_filter_by_agent() {
        let log = timeline_fixture();
        let timeline = format_timeline(
            &log,
            &ReplayTimelineFilter {
                agent_id: Some(AgentId::from("agent-1".to_owned())),
                category: None,
            },
        );

        assert!(timeline.contains("agent=agent-1"));
        assert!(timeline.contains("PoseUpdated"));
        assert!(!timeline.contains("agent=agent-0"));
    }

    #[test]
    fn timeline_filter_by_urban_category() {
        let log = timeline_fixture();
        let timeline = format_timeline(
            &log,
            &ReplayTimelineFilter {
                agent_id: None,
                category: Some(ReplayEventCategory::Urban),
            },
        );

        assert!(timeline.contains("category=urban"));
        assert!(timeline.contains("UrbanRoutePlanned"));
        assert!(!timeline.contains("PoseUpdated"));
        assert!(!timeline.contains("TickStart"));
    }

    #[test]
    fn timeline_reports_empty_filter() {
        let log = timeline_fixture();
        let timeline = format_timeline(
            &log,
            &ReplayTimelineFilter {
                agent_id: Some(AgentId::from("missing".to_owned())),
                category: Some(ReplayEventCategory::Urban),
            },
        );

        assert_eq!(timeline, "No timeline events matched the filter.\n");
    }

    #[test]
    fn snapshot_at_tick_reconstructs_poses() {
        let mut builder = EventLogBuilder::new("test", 0, "s");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::PoseUpdated {
            agent_id: AgentId::from("a0".to_owned()),
            pose: Pose {
                x: 1.0,
                y: 2.0,
                ..Default::default()
            },
            tick: 0,
        });
        builder.push(Event::TickStart { tick: 1 });
        builder.push(Event::PoseUpdated {
            agent_id: AgentId::from("a0".to_owned()),
            pose: Pose {
                x: 3.0,
                y: 4.0,
                ..Default::default()
            },
            tick: 1,
        });

        let log = builder.build();
        let snap0 = snapshot_at_tick(&log, 0);
        assert_eq!(snap0.agent_poses.len(), 1);
        assert_eq!(
            snap0.agent_poses[0].1,
            Pose {
                x: 1.0,
                y: 2.0,
                ..Default::default()
            }
        );

        let snap1 = snapshot_at_tick(&log, 1);
        assert_eq!(
            snap1.agent_poses[0].1,
            Pose {
                x: 3.0,
                y: 4.0,
                ..Default::default()
            }
        );
    }

    #[test]
    fn render_ascii_grid_basic() {
        let snapshot = ReplaySnapshot {
            tick: 0,
            agent_poses: vec![(
                AgentId::from("a0".to_owned()),
                Pose {
                    x: 5.0,
                    y: 5.0,
                    ..Default::default()
                },
            )],
            assigned_tasks: vec![],
            active_agents: vec![AgentId::from("a0".to_owned())],
            failed_agents: vec![],
        };
        let grid = render_ascii_grid(&snapshot, (0.0, 10.0, 0.0, 10.0), 5);
        assert!(grid.contains('A'));
        assert!(grid.contains('.'));
    }

    fn timeline_fixture() -> EventLog {
        let mut builder = EventLogBuilder::new("urban", 0, "urban_patrol_small_block");
        let agent_0 = AgentId::from("agent-0".to_owned());
        let agent_1 = AgentId::from("agent-1".to_owned());
        let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::UrbanRoutePlanned {
            agent_id: agent_0.clone(),
            tick: 0,
            edge_ids: vec![edge_id.clone()],
            route_length_m: 20.0,
        });
        builder.push(Event::PoseUpdated {
            agent_id: agent_1,
            pose: Pose {
                x: 10.0,
                y: 10.0,
                ..Default::default()
            },
            tick: 0,
        });
        builder.push(Event::UrbanSegmentCompleted {
            agent_id: agent_0,
            tick: 1,
            segment_index: 0,
            edge_id,
        });
        builder.build()
    }
}
