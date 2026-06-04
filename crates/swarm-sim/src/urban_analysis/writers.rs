use std::io;
use std::path::Path;

use serde::Serialize;

use super::{UrbanJudgeReport, UrbanRouteTrace, UrbanSegmentOwnershipReport, UrbanSegmentStatus};

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

pub fn write_urban_segment_ownership_json<P: AsRef<Path>>(
    report: &UrbanSegmentOwnershipReport,
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

pub fn write_urban_segment_ownership_csv<P: AsRef<Path>>(
    report: &UrbanSegmentOwnershipReport,
    path: P,
) -> io::Result<()> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer
        .write_record([
            "run_id",
            "scenario_name",
            "edge_id",
            "agent_id",
            "acquired_tick",
            "released_tick",
            "held_ticks",
        ])
        .map_err(csv_to_io)?;
    for record in &report.records {
        writer
            .write_record([
                report.run_id.as_str(),
                report.scenario_name.as_str(),
                record.edge_id.as_ref(),
                record.agent_id.as_ref(),
                record.acquired_tick.to_string().as_str(),
                record
                    .released_tick
                    .map(|tick| tick.to_string())
                    .unwrap_or_default()
                    .as_str(),
                record
                    .held_ticks
                    .map(|ticks| ticks.to_string())
                    .unwrap_or_default()
                    .as_str(),
            ])
            .map_err(csv_to_io)?;
    }
    write_csv_bytes(writer, path)
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
