use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use swarm_replay::{Event, EventLog};
use swarm_types::{AgentId, Pose, UrbanEdgeId, UrbanNodeId, UrbanObstacleId};

/// Text-artifact route trace reconstructed from an Urban replay log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanRouteTrace {
    pub run_id: String,
    pub scenario_name: String,
    pub seed: u64,
    pub agents: Vec<UrbanAgentRouteTrace>,
    pub event_counts: UrbanEventCounts,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanAgentRouteTrace {
    pub agent_id: AgentId,
    pub planned_edge_ids: Vec<UrbanEdgeId>,
    pub route_length_m: f64,
    pub segments: Vec<UrbanTraceSegment>,
    pub pose_trace: Vec<UrbanPoseTracePoint>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanTraceSegment {
    pub segment_index: usize,
    pub edge_id: UrbanEdgeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<UrbanNodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<UrbanNodeId>,
    pub status: UrbanSegmentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entered_tick: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_tick: Option<u64>,
    #[serde(default)]
    pub violation_ticks: Vec<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrbanSegmentStatus {
    Planned,
    Entered,
    Completed,
    Violated,
    NotCompleted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanPoseTracePoint {
    pub tick: u64,
    pub pose: Pose,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanJudgeReport {
    pub run_id: String,
    pub scenario_name: String,
    pub violations: Vec<UrbanJudgeViolationRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanJudgeViolationRecord {
    pub agent_id: AgentId,
    pub tick: u64,
    pub violation_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id: Option<UrbanEdgeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obstacle_id: Option<UrbanObstacleId>,
    pub pose: Pose,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UrbanEventCounts {
    pub route_planned: u64,
    pub segment_entered: u64,
    pub segment_completed: u64,
    pub violation: u64,
    pub patrol_completed: u64,
    pub bus_observed: u64,
    pub bus_detected: u64,
    pub bus_false_positive: u64,
    pub search_completed: u64,
    pub pose_updated: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanSeparationSummary {
    pub threshold_m: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_separation_m: Option<f64>,
    pub separation_violation_count: u64,
    pub route_conflict_count: u64,
    pub conflicts: Vec<UrbanRouteConflict>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanRouteConflict {
    pub agent_a: AgentId,
    pub agent_b: AgentId,
    pub tick: u64,
    pub distance_m: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index_a: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id_a: Option<UrbanEdgeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index_b: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id_b: Option<UrbanEdgeId>,
}

/// Reconstruct a route trace from Urban events in a replay log.
pub fn build_urban_route_trace(log: &EventLog) -> UrbanRouteTrace {
    let mut agents = Vec::new();

    for event in &log.events {
        match event {
            Event::UrbanRoutePlanned {
                agent_id,
                edge_ids,
                route_length_m,
                ..
            } => {
                let agent = agent_trace_mut(&mut agents, agent_id);
                agent.planned_edge_ids = edge_ids.clone();
                agent.route_length_m = *route_length_m;
                for (segment_index, edge_id) in edge_ids.iter().enumerate() {
                    let segment = segment_mut(agent, segment_index, edge_id);
                    segment.status = UrbanSegmentStatus::Planned;
                }
            }
            Event::UrbanSegmentEntered {
                agent_id,
                tick,
                segment_index,
                edge_id,
                from,
                to,
            } => {
                let segment = segment_mut(
                    agent_trace_mut(&mut agents, agent_id),
                    *segment_index,
                    edge_id,
                );
                segment.from = Some(from.clone());
                segment.to = Some(to.clone());
                segment.entered_tick = Some(*tick);
            }
            Event::UrbanSegmentCompleted {
                agent_id,
                tick,
                segment_index,
                edge_id,
            } => {
                let segment = segment_mut(
                    agent_trace_mut(&mut agents, agent_id),
                    *segment_index,
                    edge_id,
                );
                segment.completed_tick = Some(*tick);
            }
            Event::UrbanViolation {
                agent_id,
                tick,
                segment_index,
                edge_id,
                ..
            } => {
                if let Some(segment) = find_violation_segment(
                    agent_trace_mut(&mut agents, agent_id),
                    *segment_index,
                    edge_id.as_ref(),
                ) {
                    segment.violation_ticks.push(*tick);
                }
            }
            Event::PoseUpdated {
                agent_id,
                tick,
                pose,
            } => {
                agent_trace_mut(&mut agents, agent_id)
                    .pose_trace
                    .push(UrbanPoseTracePoint {
                        tick: *tick,
                        pose: *pose,
                    });
            }
            Event::TickStart { .. }
            | Event::AgentFailed { .. }
            | Event::TaskAssigned { .. }
            | Event::TaskStarted { .. }
            | Event::TaskCompleted { .. }
            | Event::TaskExpired { .. }
            | Event::MessageSent { .. }
            | Event::MessageDropped { .. }
            | Event::PartitionAdded { .. }
            | Event::PartitionRemoved { .. }
            | Event::SarScan { .. }
            | Event::SarDetection { .. }
            | Event::EdgeVisited { .. }
            | Event::SafetyViolation { .. }
            | Event::CbbaConverged { .. }
            | Event::CbbaBundleUpdated { .. }
            | Event::AgentObservation { .. }
            | Event::HazardMapUpdated { .. }
            | Event::TaskPriorityUpdated { .. }
            | Event::UrbanPatrolCompleted { .. }
            | Event::BusObserved { .. }
            | Event::BusDetected { .. }
            | Event::BusFalsePositive { .. }
            | Event::UrbanSearchCompleted { .. } => {}
        }
    }

    for agent in &mut agents {
        agent.segments.sort_by_key(|segment| segment.segment_index);
        for segment in &mut agent.segments {
            segment.status = if !segment.violation_ticks.is_empty() {
                UrbanSegmentStatus::Violated
            } else if segment.completed_tick.is_some() {
                UrbanSegmentStatus::Completed
            } else if segment.entered_tick.is_some() {
                UrbanSegmentStatus::Entered
            } else {
                UrbanSegmentStatus::NotCompleted
            };
        }
        agent.pose_trace.sort_by_key(|point| point.tick);
    }
    agents.sort_by(|a, b| a.agent_id.as_ref().cmp(b.agent_id.as_ref()));

    UrbanRouteTrace {
        run_id: log.run_id.clone(),
        scenario_name: log.scenario_name.clone(),
        seed: log.seed,
        agents,
        event_counts: count_urban_events(log),
    }
}

/// Build a structured Urban judge report from replay violation events.
pub fn build_urban_judge_report(log: &EventLog) -> UrbanJudgeReport {
    let mut violations = Vec::new();
    for event in &log.events {
        if let Event::UrbanViolation {
            agent_id,
            tick,
            segment_index,
            edge_id,
            obstacle_id,
            pose,
            reason,
        } = event
        {
            violations.push(UrbanJudgeViolationRecord {
                agent_id: agent_id.clone(),
                tick: *tick,
                violation_type: classify_violation(reason),
                segment_index: *segment_index,
                edge_id: edge_id.clone(),
                obstacle_id: obstacle_id.clone(),
                pose: *pose,
                reason: reason.clone(),
            });
        }
    }
    violations.sort_by(|a, b| {
        a.tick
            .cmp(&b.tick)
            .then_with(|| a.agent_id.as_ref().cmp(b.agent_id.as_ref()))
            .then_with(|| a.segment_index.cmp(&b.segment_index))
            .then_with(|| optional_id(a.edge_id.as_ref()).cmp(&optional_id(b.edge_id.as_ref())))
    });
    UrbanJudgeReport {
        run_id: log.run_id.clone(),
        scenario_name: log.scenario_name.clone(),
        violations,
    }
}

/// Count Urban-related replay events.
pub fn count_urban_events(log: &EventLog) -> UrbanEventCounts {
    let mut counts = UrbanEventCounts::default();
    for event in &log.events {
        match event {
            Event::UrbanRoutePlanned { .. } => counts.route_planned += 1,
            Event::UrbanSegmentEntered { .. } => counts.segment_entered += 1,
            Event::UrbanSegmentCompleted { .. } => counts.segment_completed += 1,
            Event::UrbanViolation { .. } => counts.violation += 1,
            Event::UrbanPatrolCompleted { .. } => counts.patrol_completed += 1,
            Event::BusObserved { .. } => counts.bus_observed += 1,
            Event::BusDetected { .. } => counts.bus_detected += 1,
            Event::BusFalsePositive { .. } => counts.bus_false_positive += 1,
            Event::UrbanSearchCompleted { .. } => counts.search_completed += 1,
            Event::PoseUpdated { .. } => counts.pose_updated += 1,
            _ => {}
        }
    }
    counts
}

/// Measure pairwise separation conflicts from route trace pose samples.
pub fn measure_urban_separation(
    trace: &UrbanRouteTrace,
    threshold_m: f64,
) -> UrbanSeparationSummary {
    let mut min_separation_m: Option<f64> = None;
    let mut conflicts = Vec::new();
    let mut poses_by_tick: BTreeMap<u64, Vec<(&UrbanAgentRouteTrace, Pose)>> = BTreeMap::new();

    for agent in &trace.agents {
        for point in &agent.pose_trace {
            poses_by_tick
                .entry(point.tick)
                .or_default()
                .push((agent, point.pose));
        }
    }

    for (tick, mut poses) in poses_by_tick {
        poses.sort_by(|a, b| a.0.agent_id.as_ref().cmp(b.0.agent_id.as_ref()));
        for left_index in 0..poses.len() {
            for right_index in (left_index + 1)..poses.len() {
                let (agent_a, pose_a) = poses[left_index];
                let (agent_b, pose_b) = poses[right_index];
                let distance_m = pose_a.distance_to(&pose_b);
                min_separation_m = Some(
                    min_separation_m
                        .map(|current| current.min(distance_m))
                        .unwrap_or(distance_m),
                );
                if distance_m < threshold_m {
                    let segment_a = active_segment(agent_a, tick);
                    let segment_b = active_segment(agent_b, tick);
                    conflicts.push(UrbanRouteConflict {
                        agent_a: agent_a.agent_id.clone(),
                        agent_b: agent_b.agent_id.clone(),
                        tick,
                        distance_m,
                        segment_index_a: segment_a.map(|segment| segment.segment_index),
                        edge_id_a: segment_a.map(|segment| segment.edge_id.clone()),
                        segment_index_b: segment_b.map(|segment| segment.segment_index),
                        edge_id_b: segment_b.map(|segment| segment.edge_id.clone()),
                    });
                }
            }
        }
    }

    conflicts.sort_by(|a, b| {
        a.tick
            .cmp(&b.tick)
            .then_with(|| a.agent_a.as_ref().cmp(b.agent_a.as_ref()))
            .then_with(|| a.agent_b.as_ref().cmp(b.agent_b.as_ref()))
    });

    UrbanSeparationSummary {
        threshold_m,
        min_separation_m,
        separation_violation_count: conflicts.len() as u64,
        route_conflict_count: conflicts.len() as u64,
        conflicts,
    }
}

pub fn write_urban_route_trace_json<P: AsRef<Path>>(
    trace: &UrbanRouteTrace,
    path: P,
) -> io::Result<()> {
    write_json(trace, path)
}

pub fn write_urban_judge_report_json<P: AsRef<Path>>(
    report: &UrbanJudgeReport,
    path: P,
) -> io::Result<()> {
    write_json(report, path)
}

pub fn write_urban_route_trace_csv<P: AsRef<Path>>(
    trace: &UrbanRouteTrace,
    path: P,
) -> io::Result<()> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "run_id",
            "agent_id",
            "record_type",
            "tick",
            "segment_index",
            "edge_id",
            "from",
            "to",
            "status",
            "x",
            "y",
            "z",
        ])
        .map_err(csv_to_io)?;
    for agent in &trace.agents {
        for segment in &agent.segments {
            writer
                .write_record([
                    trace.run_id.as_str(),
                    agent.agent_id.as_ref(),
                    "segment",
                    segment
                        .entered_tick
                        .or(segment.completed_tick)
                        .map(|tick| tick.to_string())
                        .unwrap_or_default()
                        .as_str(),
                    segment.segment_index.to_string().as_str(),
                    segment.edge_id.as_ref(),
                    optional_id(segment.from.as_ref()).as_str(),
                    optional_id(segment.to.as_ref()).as_str(),
                    segment_status_name(segment.status),
                    "",
                    "",
                    "",
                ])
                .map_err(csv_to_io)?;
        }
        for point in &agent.pose_trace {
            writer
                .write_record([
                    trace.run_id.as_str(),
                    agent.agent_id.as_ref(),
                    "pose",
                    point.tick.to_string().as_str(),
                    "",
                    "",
                    "",
                    "",
                    "",
                    format!("{:.3}", point.pose.x).as_str(),
                    format!("{:.3}", point.pose.y).as_str(),
                    format!("{:.3}", point.pose.z).as_str(),
                ])
                .map_err(csv_to_io)?;
        }
    }
    write_csv_bytes(writer, path)
}

pub fn write_urban_judge_report_csv<P: AsRef<Path>>(
    report: &UrbanJudgeReport,
    path: P,
) -> io::Result<()> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "run_id",
            "agent_id",
            "tick",
            "violation_type",
            "segment_index",
            "edge_id",
            "obstacle_id",
            "x",
            "y",
            "z",
            "reason",
        ])
        .map_err(csv_to_io)?;
    for violation in &report.violations {
        writer
            .write_record([
                report.run_id.as_str(),
                violation.agent_id.as_ref(),
                violation.tick.to_string().as_str(),
                violation.violation_type.as_str(),
                violation
                    .segment_index
                    .map(|segment_index| segment_index.to_string())
                    .unwrap_or_default()
                    .as_str(),
                optional_id(violation.edge_id.as_ref()).as_str(),
                optional_id(violation.obstacle_id.as_ref()).as_str(),
                format!("{:.3}", violation.pose.x).as_str(),
                format!("{:.3}", violation.pose.y).as_str(),
                format!("{:.3}", violation.pose.z).as_str(),
                violation.reason.as_str(),
            ])
            .map_err(csv_to_io)?;
    }
    write_csv_bytes(writer, path)
}

fn agent_trace_mut<'a>(
    agents: &'a mut Vec<UrbanAgentRouteTrace>,
    agent_id: &AgentId,
) -> &'a mut UrbanAgentRouteTrace {
    if let Some(index) = agents.iter().position(|agent| &agent.agent_id == agent_id) {
        return &mut agents[index];
    }
    agents.push(UrbanAgentRouteTrace {
        agent_id: agent_id.clone(),
        planned_edge_ids: Vec::new(),
        route_length_m: 0.0,
        segments: Vec::new(),
        pose_trace: Vec::new(),
    });
    agents.last_mut().expect("agent trace was just inserted")
}

fn segment_mut<'a>(
    agent: &'a mut UrbanAgentRouteTrace,
    segment_index: usize,
    edge_id: &UrbanEdgeId,
) -> &'a mut UrbanTraceSegment {
    if let Some(index) = agent
        .segments
        .iter()
        .position(|segment| segment.segment_index == segment_index)
    {
        return &mut agent.segments[index];
    }
    agent.segments.push(UrbanTraceSegment {
        segment_index,
        edge_id: edge_id.clone(),
        from: None,
        to: None,
        status: UrbanSegmentStatus::Planned,
        entered_tick: None,
        completed_tick: None,
        violation_ticks: Vec::new(),
    });
    agent
        .segments
        .last_mut()
        .expect("segment trace was just inserted")
}

fn find_violation_segment<'a>(
    agent: &'a mut UrbanAgentRouteTrace,
    segment_index: Option<usize>,
    edge_id: Option<&UrbanEdgeId>,
) -> Option<&'a mut UrbanTraceSegment> {
    if let Some(segment_index) = segment_index {
        if let Some(index) = agent
            .segments
            .iter()
            .position(|segment| segment.segment_index == segment_index)
        {
            return Some(&mut agent.segments[index]);
        }
    }
    let edge_id = edge_id?;
    let index = agent
        .segments
        .iter()
        .position(|segment| &segment.edge_id == edge_id)?;
    Some(&mut agent.segments[index])
}

fn active_segment(agent: &UrbanAgentRouteTrace, tick: u64) -> Option<&UrbanTraceSegment> {
    agent
        .segments
        .iter()
        .filter(|segment| {
            segment.entered_tick.is_some_and(|entered| entered <= tick)
                && segment
                    .completed_tick
                    .is_none_or(|completed| tick <= completed)
        })
        .max_by_key(|segment| segment.entered_tick)
}

fn classify_violation(reason: &str) -> String {
    if reason.contains("ObstacleIntersection") {
        "obstacle_intersection".to_owned()
    } else if reason.contains("BlockedEdge") {
        "blocked_edge".to_owned()
    } else if reason.contains("MissingEdge") {
        "missing_edge".to_owned()
    } else {
        "unknown".to_owned()
    }
}

fn segment_status_name(status: UrbanSegmentStatus) -> &'static str {
    match status {
        UrbanSegmentStatus::Planned => "planned",
        UrbanSegmentStatus::Entered => "entered",
        UrbanSegmentStatus::Completed => "completed",
        UrbanSegmentStatus::Violated => "violated",
        UrbanSegmentStatus::NotCompleted => "not_completed",
    }
}

fn optional_id<T: ToString>(id: Option<&T>) -> String {
    id.map(ToString::to_string).unwrap_or_default()
}

fn write_json<T: Serialize, P: AsRef<Path>>(value: &T, path: P) -> io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    std::fs::write(path, json)
}

fn write_csv_bytes<P: AsRef<Path>>(writer: csv::Writer<Vec<u8>>, path: P) -> io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    std::fs::write(path, bytes)
}

fn csv_to_io(error: csv::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_replay::EventLogBuilder;
    use swarm_types::UrbanObstacleId;

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
}
