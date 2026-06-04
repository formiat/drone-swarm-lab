use super::*;
use swarm_replay::{Event, EventLog, EventLogBuilder};
use swarm_types::{UrbanObstacleId, UrbanRightOfWayPolicy};

#[test]
fn route_trace_captures_planned_executed_segments_and_poses() {
    let log = urban_log_with_violation();
    let trace = build_urban_route_trace(&log);

    assert_eq!(trace.event_counts.route_planned, 1);
    assert_eq!(trace.event_counts.segment_completed, 1);
    assert_eq!(trace.event_counts.violation, 1);
    assert_eq!(trace.agents.len(), 1);
    let agent = &trace.agents[0];
    assert_eq!(agent.planned_edge_ids.len(), 2);
    assert_eq!(agent.segments.len(), 2);
    assert_eq!(agent.segments[0].status, UrbanSegmentStatus::Completed);
    assert_eq!(agent.segments[1].status, UrbanSegmentStatus::Violated);
    assert_eq!(agent.pose_trace.len(), 2);
}

#[test]
fn judge_report_serializes_structured_obstacle_id() {
    let log = urban_log_with_violation();
    let report = build_urban_judge_report(&log);

    assert_eq!(report.violations.len(), 1);
    let violation = &report.violations[0];
    assert_eq!(violation.violation_type, "obstacle_intersection");
    assert_eq!(
        violation.obstacle_id,
        Some(UrbanObstacleId::from("building-center".to_owned()))
    );
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("building-center"));
}

#[test]
fn route_trace_csv_has_stable_header() {
    let trace = build_urban_route_trace(&urban_log_with_violation());
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let path = tmp_dir.path().join("trace.csv");

    write_urban_route_trace_csv(&trace, &path).unwrap();

    let csv = std::fs::read_to_string(path).unwrap();
    assert!(csv.starts_with(
        "run_id,agent_id,record_type,tick,segment_index,edge_id,from,to,status,x,y,z"
    ));
    assert!(csv.contains("segment"));
    assert!(csv.contains("pose"));
}

#[test]
fn judge_report_csv_has_stable_header() {
    let report = build_urban_judge_report(&urban_log_with_violation());
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let path = tmp_dir.path().join("judge.csv");

    write_urban_judge_report_csv(&report, &path).unwrap();

    let csv = std::fs::read_to_string(path).unwrap();
    assert!(csv.starts_with(
        "run_id,agent_id,tick,violation_type,segment_index,edge_id,obstacle_id,x,y,z,reason"
    ));
    assert!(csv.contains("obstacle_intersection"));
}

#[test]
fn segment_ownership_report_reconstructs_lock_intervals() {
    let log = urban_log_with_segment_locks();
    let report = build_urban_segment_ownership_report(&log);

    assert_eq!(report.run_id, "urban-locks");
    assert_eq!(report.scenario_name, "urban_multi_agent_deconflict");
    assert_eq!(report.records.len(), 2);

    let released = &report.records[0];
    assert_eq!(released.agent_id, AgentId::from("agent-0".to_owned()));
    assert_eq!(released.edge_id, UrbanEdgeId::from("road-n0-n1".to_owned()));
    assert_eq!(released.acquired_tick, 1);
    assert_eq!(released.released_tick, Some(4));
    assert_eq!(released.held_ticks, Some(3));

    let active = &report.records[1];
    assert_eq!(active.agent_id, AgentId::from("agent-1".to_owned()));
    assert_eq!(active.edge_id, UrbanEdgeId::from("road-n1-n2".to_owned()));
    assert_eq!(active.acquired_tick, 2);
    assert_eq!(active.released_tick, None);
    assert_eq!(active.held_ticks, None);
}

#[test]
fn segment_ownership_csv_has_stable_header() {
    let report = build_urban_segment_ownership_report(&urban_log_with_segment_locks());
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let path = tmp_dir.path().join("ownership.csv");

    write_urban_segment_ownership_csv(&report, &path).unwrap();

    let csv = std::fs::read_to_string(path).unwrap();
    assert!(csv.starts_with(
        "run_id,scenario_name,edge_id,agent_id,acquired_tick,released_tick,held_ticks"
    ));
    assert!(csv.contains("road-n0-n1"));
    assert!(csv.contains("agent-0"));
}

#[test]
fn separation_summary_reports_two_agent_conflict() {
    let trace = two_agent_trace();
    let summary = measure_urban_separation(&trace, 2.0);

    assert_eq!(summary.min_separation_m, Some(1.0));
    assert_eq!(summary.separation_violation_count, 1);
    assert_eq!(summary.route_conflict_count, 1);
    assert_eq!(summary.conflicts[0].tick, 1);
    assert_eq!(
        summary.conflicts[0].edge_id_a,
        Some(UrbanEdgeId::from("road-n0-n1".to_owned()))
    );
}

#[test]
fn one_agent_separation_summary_is_empty() {
    let trace = build_urban_route_trace(&urban_log_with_violation());
    let summary = measure_urban_separation(&trace, 2.0);

    assert_eq!(summary.min_separation_m, None);
    assert_eq!(summary.separation_violation_count, 0);
    assert!(summary.conflicts.is_empty());
}

fn urban_log_with_violation() -> EventLog {
    let agent_id = AgentId::from("agent-0".to_owned());
    let edge_0 = UrbanEdgeId::from("road-n0-n1".to_owned());
    let edge_1 = UrbanEdgeId::from("road-n1-n2".to_owned());
    let mut builder = EventLogBuilder::new("urban-run", 0, "urban_patrol_small_block");
    builder.push(Event::UrbanRoutePlanned {
        agent_id: agent_id.clone(),
        tick: 0,
        edge_ids: vec![edge_0.clone(), edge_1.clone()],
        route_length_m: 40.0,
    });
    builder.push(Event::UrbanSegmentEntered {
        agent_id: agent_id.clone(),
        tick: 0,
        segment_index: 0,
        edge_id: edge_0.clone(),
        from: UrbanNodeId::from("n0".to_owned()),
        to: UrbanNodeId::from("n1".to_owned()),
    });
    builder.push(Event::PoseUpdated {
        agent_id: agent_id.clone(),
        pose: Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        },
        tick: 0,
    });
    builder.push(Event::UrbanSegmentCompleted {
        agent_id: agent_id.clone(),
        tick: 1,
        segment_index: 0,
        edge_id: edge_0,
    });
    builder.push(Event::UrbanSegmentEntered {
        agent_id: agent_id.clone(),
        tick: 1,
        segment_index: 1,
        edge_id: edge_1.clone(),
        from: UrbanNodeId::from("n1".to_owned()),
        to: UrbanNodeId::from("n2".to_owned()),
    });
    builder.push(Event::PoseUpdated {
        agent_id: agent_id.clone(),
        pose: Pose {
            x: 1.0,
            y: 0.0,
            ..Default::default()
        },
        tick: 1,
    });
    builder.push(Event::UrbanViolation {
        agent_id,
        tick: 2,
        segment_index: Some(1),
        edge_id: Some(edge_1),
        obstacle_id: Some(UrbanObstacleId::from("building-center".to_owned())),
        pose: Pose {
            x: 2.0,
            y: 0.0,
            ..Default::default()
        },
        reason: "ObstacleIntersection".to_owned(),
    });
    builder.build()
}

fn urban_log_with_segment_locks() -> EventLog {
    let mut builder = EventLogBuilder::new("urban-locks", 0, "urban_multi_agent_deconflict");
    builder.push(Event::UrbanSegmentLockAcquired {
        agent_id: AgentId::from("agent-0".to_owned()),
        tick: 1,
        edge_id: UrbanEdgeId::from("road-n0-n1".to_owned()),
        policy: UrbanRightOfWayPolicy::FirstCome,
        reason: "first reservation".to_owned(),
    });
    builder.push(Event::UrbanSegmentLockAcquired {
        agent_id: AgentId::from("agent-1".to_owned()),
        tick: 2,
        edge_id: UrbanEdgeId::from("road-n1-n2".to_owned()),
        policy: UrbanRightOfWayPolicy::FirstCome,
        reason: "second reservation".to_owned(),
    });
    builder.push(Event::UrbanSegmentLockReleased {
        agent_id: AgentId::from("agent-0".to_owned()),
        tick: 4,
        edge_id: UrbanEdgeId::from("road-n0-n1".to_owned()),
        held_ticks: 3,
    });
    builder.build()
}

fn two_agent_trace() -> UrbanRouteTrace {
    let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
    UrbanRouteTrace {
        run_id: "two-agent".to_owned(),
        scenario_name: "urban_multi_agent_small_block".to_owned(),
        seed: 0,
        event_counts: UrbanEventCounts::default(),
        agents: vec![
            UrbanAgentRouteTrace {
                agent_id: AgentId::from("agent-0".to_owned()),
                planned_edge_ids: vec![edge_id.clone()],
                route_length_m: 20.0,
                segments: vec![UrbanTraceSegment {
                    segment_index: 0,
                    edge_id: edge_id.clone(),
                    from: Some(UrbanNodeId::from("n0".to_owned())),
                    to: Some(UrbanNodeId::from("n1".to_owned())),
                    status: UrbanSegmentStatus::Entered,
                    entered_tick: Some(0),
                    completed_tick: None,
                    violation_ticks: vec![],
                }],
                pose_trace: vec![UrbanPoseTracePoint {
                    tick: 1,
                    pose: Pose {
                        x: 0.0,
                        y: 0.0,
                        ..Default::default()
                    },
                }],
            },
            UrbanAgentRouteTrace {
                agent_id: AgentId::from("agent-1".to_owned()),
                planned_edge_ids: vec![edge_id.clone()],
                route_length_m: 20.0,
                segments: vec![UrbanTraceSegment {
                    segment_index: 0,
                    edge_id,
                    from: Some(UrbanNodeId::from("n0".to_owned())),
                    to: Some(UrbanNodeId::from("n1".to_owned())),
                    status: UrbanSegmentStatus::Entered,
                    entered_tick: Some(0),
                    completed_tick: None,
                    violation_ticks: vec![],
                }],
                pose_trace: vec![UrbanPoseTracePoint {
                    tick: 1,
                    pose: Pose {
                        x: 1.0,
                        y: 0.0,
                        ..Default::default()
                    },
                }],
            },
        ],
    }
}
