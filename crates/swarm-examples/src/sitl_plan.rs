use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use swarm_safety::preflight::{SafetyValidationReport, ViolationSeverity};
use swarm_sim::{
    export_route_loop_to_waypoints, validate_scenario_suite, GeoOrigin, ScenarioSuite,
    ScenarioSuiteEntry, UrbanRouteExportOptions,
};

pub const DEFAULT_SITL_GEO_ORIGIN: GeoOrigin = GeoOrigin {
    lat_deg: 47.397_742,
    lon_deg: 8.545_594,
    alt_m: 0.0,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SitlMode {
    Mock,
    DryRun,
    Connection { addr: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SitlConnectionClass {
    LocalPx4SitlUdp,
    HardwareCandidate,
}

impl SitlConnectionClass {
    pub fn name(self) -> &'static str {
        match self {
            Self::LocalPx4SitlUdp => "local_px4_sitl_udp",
            Self::HardwareCandidate => "hardware_candidate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParsedSitlConnection<'a> {
    Udp { host: &'a str },
    Tcp,
    Serial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SitlCoordinateFrame {
    LocalSimulation,
}

impl SitlCoordinateFrame {
    pub fn name(self) -> &'static str {
        match self {
            Self::LocalSimulation => "local_simulation",
        }
    }

    pub fn from_name(name: &str) -> Result<Self, SitlError> {
        match name {
            "local_simulation" => Ok(Self::LocalSimulation),
            other => Err(SitlError::UnsupportedCoordinateFrame {
                frame: other.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SitlWaypointItem {
    pub seq: u16,
    pub task_id: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_index_on_segment: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SitlPlan {
    pub agent_id: String,
    pub scenario_path: PathBuf,
    pub suite_name: String,
    pub scenario_name: String,
    pub mission: String,
    pub profile: String,
    pub coordinate_frame: SitlCoordinateFrame,
    pub altitude_source: String,
    pub geo_origin: Option<GeoOrigin>,
    pub export_kind: String,
    pub planner_or_adapter: String,
    pub route_length_m: Option<f64>,
    pub segment_count: Option<usize>,
    pub waypoint_count: usize,
    pub waypoints: Vec<SitlWaypointItem>,
    pub safety_report: SafetyValidationReport,
}

#[derive(Debug, thiserror::Error)]
pub enum SitlError {
    #[error("invalid scenario {path:?}: {message}")]
    InvalidScenario { path: PathBuf, message: String },
    #[error("no pose tasks found in scenario '{scenario_name}'")]
    NoPoseTasks { scenario_name: String },
    #[error("SITL task subset for agent '{agent_id}' is empty")]
    EmptyTaskSubset { agent_id: String },
    #[error("SITL task '{task_id}' was not found for agent '{agent_id}'")]
    TaskNotFound { task_id: String, agent_id: String },
    #[error("SITL task '{task_id}' is missing pose for agent '{agent_id}'")]
    TaskMissingPose { task_id: String, agent_id: String },
    #[error(
        "feature missing: --connection requires feature '{feature}'. Build with: cargo build --bin sitl_agent --features {feature}"
    )]
    FeatureMissing { feature: &'static str },
    #[error(
        "bad connection string '{addr}': expected udpin:<host>:<port>, udpout:<host>:<port>, tcpout:<host>:<port>, tcpin:<host>:<port>, serial:<path>:<baud>, or legacy udp:/tcp: alias"
    )]
    BadConnectionString { addr: String },
    #[error(
        "hardware candidate connection '{addr}' classified as {class}; this path may target real hardware or a remote endpoint and requires --allow-hardware-candidate. Read docs/HARDWARE_READINESS.md before any hardware experiment"
    )]
    HardwareCandidateRequiresExplicitAllow { addr: String, class: &'static str },
    #[error(
        "connection option {option} requires --connection <addr> or --multi-agent-config <path>"
    )]
    ConnectionOptionRequiresConnection { option: &'static str },
    #[error("unsupported coordinate frame '{frame}'")]
    UnsupportedCoordinateFrame { frame: String },
    #[error("Urban route export does not support task subset filtering for agent '{agent_id}'")]
    UrbanRouteTaskSubsetUnsupported { agent_id: String },
    #[error("Urban route export failed: {message}")]
    UrbanRouteExport { message: String },
    #[error("connection failed: {message}")]
    ConnectionFailed { message: String },
    #[error("safety config read failed {path:?}: {message}")]
    SafetyConfigRead { path: PathBuf, message: String },
    #[error("safety config parse failed {path:?}: {message}")]
    SafetyConfigParse { path: PathBuf, message: String },
    #[error("safety config invalid {field}: {message}")]
    SafetyConfigInvalid { field: String, message: String },
    #[error("safety validation failed: {message}")]
    SafetyValidationFailed { message: String },
    #[error("preflight validation failed: {rule_ids}")]
    PreflightFailed { rule_ids: String },
    #[error("missing SITL mode: specify exactly one of --mock, --dry-run, or --connection <addr>")]
    MissingMode,
    #[error(
        "conflicting SITL modes: specify exactly one of --mock, --dry-run, or --connection <addr>"
    )]
    ConflictingModes,
    #[error("missing required argument: {name}")]
    MissingArgument { name: &'static str },
    #[error("unknown argument: {arg}")]
    UnknownArgument { arg: String },
    #[error("conflicting lifecycle modes: specify at most one of --upload-only or --execute")]
    ConflictingLifecycleModes,
    #[error("lifecycle option {option} requires --connection <addr>")]
    LifecycleOptionRequiresConnection { option: &'static str },
    #[error("lifecycle option {option} requires --execute")]
    LifecycleOptionRequiresExecute { option: &'static str },
    #[error("invalid duration for {name}: '{value}'")]
    InvalidDuration { name: &'static str, value: String },
    #[error("run report option {option} requires --connection <addr> --execute")]
    RunReportRequiresExecute { option: &'static str },
    #[error("run report write failed {path:?}: {message}")]
    RunReportWrite { path: PathBuf, message: String },
    #[error("dry-run artifact option {option} is only supported for --dry-run")]
    DryRunArtifactUnsupported { option: &'static str },
    #[error("dry-run artifact write failed {path:?}: {message}")]
    DryRunArtifactWrite { path: PathBuf, message: String },
    #[error("replay log option {option} is not supported for {mode}")]
    ReplayLogUnsupported {
        option: &'static str,
        mode: &'static str,
    },
    #[error("replay log write failed {path:?}: {message}")]
    ReplayLogWrite { path: PathBuf, message: String },
    #[error("replay summary write failed {path:?}: {message}")]
    ReplaySummaryWrite { path: PathBuf, message: String },
    #[error("multi-agent config read failed {path:?}: {message}")]
    MultiAgentConfigRead { path: PathBuf, message: String },
    #[error("multi-agent config parse failed {path:?}: {message}")]
    MultiAgentConfigParse { path: PathBuf, message: String },
    #[error("multi-agent config invalid: {message}")]
    MultiAgentConfigInvalid { message: String },
    #[error("multi-agent manifest write failed {path:?}: {message}")]
    MultiAgentManifestWrite { path: PathBuf, message: String },
    #[error("output path already exists {path:?}; use --force to overwrite")]
    OutputAlreadyExists { path: PathBuf },
}

#[derive(Clone, Debug, Serialize)]
pub struct SitlDryRunArtifact {
    pub schema_version: String,
    pub source_scenario_path: PathBuf,
    pub suite_name: String,
    pub scenario_name: String,
    pub mission: String,
    pub profile: String,
    pub agent_id: String,
    pub export_kind: String,
    pub planner_or_adapter: String,
    pub route_length_m: Option<f64>,
    pub segment_count: Option<usize>,
    pub waypoint_count: usize,
    pub start_waypoint: Option<SitlWaypointItem>,
    pub end_waypoint: Option<SitlWaypointItem>,
    pub start_global: Option<SitlGlobalWaypointSummary>,
    pub end_global: Option<SitlGlobalWaypointSummary>,
    pub altitude_source: String,
    pub geo_origin: Option<GeoOrigin>,
    pub effective_geo_origin: GeoOrigin,
    pub coordinate_frame: String,
    pub safety_report: SafetyValidationReport,
    pub command: Vec<String>,
    pub git_commit: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct SitlGlobalWaypointSummary {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub relative_alt_m: f64,
}

pub fn validate_connection_string(addr: &str) -> Result<(), SitlError> {
    parse_sitl_connection(addr).map(|_| ())
}

pub fn classify_connection_string(addr: &str) -> Result<SitlConnectionClass, SitlError> {
    let connection = parse_sitl_connection(addr)?;
    Ok(match connection {
        ParsedSitlConnection::Udp { host } if is_loopback_host(host) => {
            SitlConnectionClass::LocalPx4SitlUdp
        }
        ParsedSitlConnection::Udp { .. }
        | ParsedSitlConnection::Tcp
        | ParsedSitlConnection::Serial => SitlConnectionClass::HardwareCandidate,
    })
}

fn parse_sitl_connection(addr: &str) -> Result<ParsedSitlConnection<'_>, SitlError> {
    let addr = addr.trim();
    let Some((scheme, rest)) = addr.split_once(':') else {
        return bad_connection_string(addr);
    };

    match scheme {
        "udp" | "udpin" | "udpout" | "udpbcast" => {
            parse_host_port(addr, rest).map(|host| ParsedSitlConnection::Udp { host })
        }
        "tcp" | "tcpin" | "tcpout" => {
            parse_host_port(addr, rest).map(|_| ParsedSitlConnection::Tcp)
        }
        "serial" => validate_serial(addr, rest).map(|_| ParsedSitlConnection::Serial),
        _ => bad_connection_string(addr),
    }
}

fn parse_host_port<'a>(addr: &str, rest: &'a str) -> Result<&'a str, SitlError> {
    let Some((host, port)) = rest.rsplit_once(':') else {
        return bad_connection_string(addr);
    };
    if host.trim().is_empty() || port.trim().parse::<u16>().is_err() {
        return bad_connection_string(addr);
    }
    Ok(host)
}

fn validate_serial(addr: &str, rest: &str) -> Result<(), SitlError> {
    let Some((path, baud)) = rest.rsplit_once(':') else {
        return bad_connection_string(addr);
    };
    if path.trim().is_empty() || baud.trim().parse::<u32>().is_err() {
        return bad_connection_string(addr);
    }
    Ok(())
}

fn bad_connection_string<T>(addr: &str) -> Result<T, SitlError> {
    Err(SitlError::BadConnectionString {
        addr: addr.to_owned(),
    })
}

fn is_loopback_host(host: &str) -> bool {
    let host = host.trim();
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    host.parse::<IpAddr>()
        .map(|addr| addr.is_loopback())
        .unwrap_or(false)
}

pub fn load_sitl_plan(
    scenario_path: impl AsRef<Path>,
    agent_id: impl Into<String>,
) -> Result<SitlPlan, SitlError> {
    let scenario_path = scenario_path.as_ref();
    let suite = load_sitl_suite(scenario_path)?;
    build_sitl_plan(&suite, scenario_path, agent_id)
}

pub fn load_sitl_suite(scenario_path: impl AsRef<Path>) -> Result<ScenarioSuite, SitlError> {
    let scenario_path = scenario_path.as_ref();
    let scenario_path_string =
        scenario_path
            .to_str()
            .ok_or_else(|| SitlError::InvalidScenario {
                path: scenario_path.to_path_buf(),
                message: "scenario path is not valid UTF-8".to_owned(),
            })?;
    swarm_sim::load_scenario_suite(scenario_path_string).map_err(|error| {
        SitlError::InvalidScenario {
            path: scenario_path.to_path_buf(),
            message: error.to_string(),
        }
    })
}

pub fn first_sitl_entry(
    suite: &ScenarioSuite,
    scenario_path: impl AsRef<Path>,
) -> Result<&ScenarioSuiteEntry, SitlError> {
    suite
        .scenarios
        .first()
        .ok_or_else(|| SitlError::InvalidScenario {
            path: scenario_path.as_ref().to_path_buf(),
            message: "Scenario suite must contain at least one scenario".to_owned(),
        })
}

pub fn build_sitl_plan(
    suite: &ScenarioSuite,
    scenario_path: impl AsRef<Path>,
    agent_id: impl Into<String>,
) -> Result<SitlPlan, SitlError> {
    build_sitl_plan_with_task_filter(suite, scenario_path, agent_id, None)
}

pub fn build_sitl_plan_for_task_ids(
    suite: &ScenarioSuite,
    scenario_path: impl AsRef<Path>,
    agent_id: impl Into<String>,
    task_ids: &[String],
) -> Result<SitlPlan, SitlError> {
    build_sitl_plan_with_task_filter(suite, scenario_path, agent_id, Some(task_ids))
}

fn build_sitl_plan_with_task_filter(
    suite: &ScenarioSuite,
    scenario_path: impl AsRef<Path>,
    agent_id: impl Into<String>,
    task_ids: Option<&[String]>,
) -> Result<SitlPlan, SitlError> {
    let scenario_path = scenario_path.as_ref().to_path_buf();
    let entry = first_sitl_entry(suite, &scenario_path)?;
    let safety_report = check_preflight_or_err(entry)?;
    let validation_errors = validate_scenario_suite(suite);
    let agent_id = agent_id.into();

    if entry.mission == "urban-patrol" && entry.run_config.urban_state.is_some() {
        if task_ids.is_some() {
            return Err(SitlError::UrbanRouteTaskSubsetUnsupported { agent_id });
        }
        return build_urban_route_sitl_plan(
            &suite.name,
            entry,
            scenario_path,
            agent_id,
            validation_errors,
            safety_report,
        );
    }

    let task_ids: Option<HashSet<&str>> =
        task_ids.map(|ids| ids.iter().map(String::as_str).collect());

    let waypoints: Vec<SitlWaypointItem> = entry
        .scenario
        .tasks
        .iter()
        .filter(|task| {
            let task_id = task.id.to_string();
            task_ids
                .as_ref()
                .is_none_or(|ids| ids.contains(task_id.as_str()))
        })
        .filter_map(|task| {
            let pose = task.pose?;
            Some((task, pose))
        })
        .enumerate()
        .map(|(seq, (task, pose))| SitlWaypointItem {
            seq: seq as u16,
            task_id: task.id.to_string(),
            x: pose.x,
            y: pose.y,
            z: pose.z,
            source: "pose_task".to_owned(),
            edge_id: None,
            from_node_id: None,
            to_node_id: None,
            segment_index: None,
            point_index_on_segment: None,
        })
        .collect();

    if let Some(task_ids) = task_ids.as_ref() {
        if task_ids.is_empty() {
            return Err(SitlError::EmptyTaskSubset {
                agent_id: agent_id.clone(),
            });
        }
        for task_id in task_ids {
            let Some(task) = entry
                .scenario
                .tasks
                .iter()
                .find(|task| task.id.to_string() == *task_id)
            else {
                return Err(SitlError::TaskNotFound {
                    task_id: (*task_id).to_owned(),
                    agent_id: agent_id.clone(),
                });
            };
            if task.pose.is_none() {
                return Err(SitlError::TaskMissingPose {
                    task_id: (*task_id).to_owned(),
                    agent_id: agent_id.clone(),
                });
            }
        }
    }

    if waypoints.is_empty() {
        return Err(SitlError::NoPoseTasks {
            scenario_name: entry.scenario.name.clone(),
        });
    }

    if !validation_errors.is_empty() {
        let message = validation_errors
            .iter()
            .map(|error| {
                let field = &error.field;
                let message = &error.message;
                format!("{field}: {message}")
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(SitlError::InvalidScenario {
            path: scenario_path,
            message,
        });
    }

    Ok(SitlPlan {
        agent_id,
        scenario_path,
        suite_name: suite.name.clone(),
        scenario_name: entry.scenario.name.clone(),
        mission: entry.mission.clone(),
        profile: entry.profile.clone(),
        coordinate_frame: SitlCoordinateFrame::LocalSimulation,
        altitude_source: "pose.z (serde default 0.0 when omitted)".to_owned(),
        geo_origin: entry.scenario.geo_origin,
        export_kind: "pose_tasks".to_owned(),
        planner_or_adapter: "sitl_pose_task_extractor".to_owned(),
        route_length_m: None,
        segment_count: None,
        waypoint_count: waypoints.len(),
        waypoints,
        safety_report,
    })
}

fn build_urban_route_sitl_plan(
    suite_name: &str,
    entry: &ScenarioSuiteEntry,
    scenario_path: PathBuf,
    agent_id: String,
    validation_errors: Vec<swarm_sim::ValidationError>,
    safety_report: SafetyValidationReport,
) -> Result<SitlPlan, SitlError> {
    if !validation_errors.is_empty() {
        let message = validation_errors
            .iter()
            .map(|error| {
                let field = &error.field;
                let message = &error.message;
                format!("{field}: {message}")
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(SitlError::InvalidScenario {
            path: scenario_path,
            message,
        });
    }

    let urban_state = entry
        .run_config
        .urban_state
        .as_ref()
        .expect("checked by caller");
    let options = UrbanRouteExportOptions {
        planner: urban_state.planner.clone(),
        ..Default::default()
    };
    let export =
        export_route_loop_to_waypoints(&urban_state.map, &urban_state.route_loop, &options)
            .map_err(|error| SitlError::UrbanRouteExport {
                message: error.to_string(),
            })?;

    if export.waypoints.is_empty() {
        return Err(SitlError::NoPoseTasks {
            scenario_name: entry.scenario.name.clone(),
        });
    }

    let waypoints: Vec<SitlWaypointItem> = export
        .waypoints
        .iter()
        .map(|waypoint| SitlWaypointItem {
            seq: waypoint.seq,
            task_id: waypoint.task_id.clone(),
            x: waypoint.pose.x,
            y: waypoint.pose.y,
            z: waypoint.pose.z,
            source: "urban_route".to_owned(),
            edge_id: Some(waypoint.edge_id.to_string()),
            from_node_id: Some(waypoint.from_node_id.to_string()),
            to_node_id: Some(waypoint.to_node_id.to_string()),
            segment_index: Some(waypoint.segment_index),
            point_index_on_segment: Some(waypoint.point_index_on_segment),
        })
        .collect();

    Ok(SitlPlan {
        agent_id,
        scenario_path,
        suite_name: suite_name.to_owned(),
        scenario_name: entry.scenario.name.clone(),
        mission: entry.mission.clone(),
        profile: entry.profile.clone(),
        coordinate_frame: SitlCoordinateFrame::LocalSimulation,
        altitude_source: export.metadata.altitude_source.clone(),
        geo_origin: entry.scenario.geo_origin,
        export_kind: "urban_route".to_owned(),
        planner_or_adapter: format!("urban_route_export:{}", export.metadata.planner),
        route_length_m: Some(export.metadata.route_length_m),
        segment_count: Some(export.metadata.segment_count),
        waypoint_count: export.metadata.waypoint_count,
        waypoints,
        safety_report,
    })
}

pub fn check_preflight_or_err(
    entry: &ScenarioSuiteEntry,
) -> Result<SafetyValidationReport, SitlError> {
    let report = swarm_sim::preflight::run_preflight(entry);
    if !report.passed {
        let rule_ids = report
            .violations
            .iter()
            .filter(|violation| violation.severity == ViolationSeverity::Error)
            .map(|violation| violation.rule_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(SitlError::PreflightFailed { rule_ids });
    }
    Ok(report)
}

pub fn format_dry_run_plan(plan: &SitlPlan) -> String {
    let mut output = String::new();
    writeln!(output, "mode: dry-run").unwrap();
    writeln!(output, "agent_id: {}", plan.agent_id).unwrap();
    writeln!(output, "scenario_path: {}", plan.scenario_path.display()).unwrap();
    writeln!(output, "suite_name: {}", plan.suite_name).unwrap();
    writeln!(output, "scenario_name: {}", plan.scenario_name).unwrap();
    writeln!(output, "mission: {}", plan.mission).unwrap();
    writeln!(output, "profile: {}", plan.profile).unwrap();
    writeln!(output, "coordinate_frame: {}", plan.coordinate_frame.name()).unwrap();
    writeln!(output, "export_kind: {}", plan.export_kind).unwrap();
    writeln!(output, "planner_or_adapter: {}", plan.planner_or_adapter).unwrap();
    writeln!(output, "altitude_source: {}", plan.altitude_source).unwrap();
    writeln!(
        output,
        "geo_origin: {}",
        plan.geo_origin
            .map(format_geo_origin)
            .unwrap_or_else(|| "default_sitl_upload_origin".to_owned())
    )
    .unwrap();
    if let Some(route_length_m) = plan.route_length_m {
        writeln!(output, "route_length_m: {route_length_m:.3}").unwrap();
    }
    if let Some(segment_count) = plan.segment_count {
        writeln!(output, "segment_count: {segment_count}").unwrap();
    }
    writeln!(output, "waypoint_count: {}", plan.waypoint_count).unwrap();
    if plan.safety_report.passed {
        writeln!(output, "preflight_safety: passed").unwrap();
    } else {
        let rule_ids = preflight_error_rule_ids(&plan.safety_report);
        writeln!(output, "preflight_safety: failed rule_ids={rule_ids}").unwrap();
    }
    writeln!(
        output,
        "limitations: x/y are local simulation coordinates, not WGS84 latitude/longitude; dry-run does not upload to PX4"
    )
    .unwrap();
    writeln!(output, "waypoints:").unwrap();
    for waypoint in &plan.waypoints {
        writeln!(
            output,
            "  seq={} task_id={} x={:.3} y={:.3} z={:.3}",
            waypoint.seq, waypoint.task_id, waypoint.x, waypoint.y, waypoint.z
        )
        .unwrap();
        if waypoint.source == "urban_route" {
            writeln!(
                output,
                "    source={} edge_id={} from={} to={} segment_index={} point_index_on_segment={}",
                waypoint.source,
                waypoint.edge_id.as_deref().unwrap_or("-"),
                waypoint.from_node_id.as_deref().unwrap_or("-"),
                waypoint.to_node_id.as_deref().unwrap_or("-"),
                waypoint
                    .segment_index
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                waypoint
                    .point_index_on_segment
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned())
            )
            .unwrap();
        }
    }
    output
}

pub fn format_geo_origin(origin: GeoOrigin) -> String {
    format!(
        "lat_deg={:.7} lon_deg={:.7} alt_m={:.3}",
        origin.lat_deg, origin.lon_deg, origin.alt_m
    )
}

pub fn dry_run_artifact(plan: &SitlPlan, command: Vec<String>) -> SitlDryRunArtifact {
    let effective_geo_origin = plan.geo_origin.unwrap_or(DEFAULT_SITL_GEO_ORIGIN);
    SitlDryRunArtifact {
        schema_version: "sitl_dry_run_artifact.v1".to_owned(),
        source_scenario_path: plan.scenario_path.clone(),
        suite_name: plan.suite_name.clone(),
        scenario_name: plan.scenario_name.clone(),
        mission: plan.mission.clone(),
        profile: plan.profile.clone(),
        agent_id: plan.agent_id.clone(),
        export_kind: plan.export_kind.clone(),
        planner_or_adapter: plan.planner_or_adapter.clone(),
        route_length_m: plan.route_length_m,
        segment_count: plan.segment_count,
        waypoint_count: plan.waypoint_count,
        start_waypoint: plan.waypoints.first().cloned(),
        end_waypoint: plan.waypoints.last().cloned(),
        start_global: plan
            .waypoints
            .first()
            .map(|waypoint| global_waypoint_summary(waypoint, effective_geo_origin)),
        end_global: plan
            .waypoints
            .last()
            .map(|waypoint| global_waypoint_summary(waypoint, effective_geo_origin)),
        altitude_source: plan.altitude_source.clone(),
        geo_origin: plan.geo_origin,
        effective_geo_origin,
        coordinate_frame: plan.coordinate_frame.name().to_owned(),
        safety_report: plan.safety_report.clone(),
        command,
        git_commit: option_env!("GIT_COMMIT").map(str::to_owned),
    }
}

fn preflight_error_rule_ids(report: &SafetyValidationReport) -> String {
    report
        .violations
        .iter()
        .filter(|violation| violation.severity == ViolationSeverity::Error)
        .map(|violation| violation.rule_id.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

pub fn global_waypoint_summary(
    waypoint: &SitlWaypointItem,
    origin: GeoOrigin,
) -> SitlGlobalWaypointSummary {
    let lat_deg = origin.lat_deg + waypoint.y / 111_320.0;
    let meters_per_lon_degree = 111_320.0 * origin.lat_deg.to_radians().cos();
    let lon_deg = origin.lon_deg + waypoint.x / meters_per_lon_degree;
    SitlGlobalWaypointSummary {
        lat_deg,
        lon_deg,
        relative_alt_m: waypoint.z,
    }
}

pub fn write_dry_run_artifact(
    path: impl AsRef<Path>,
    plan: &SitlPlan,
    command: Vec<String>,
) -> Result<(), SitlError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| SitlError::DryRunArtifactWrite {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    let artifact = dry_run_artifact(plan, command);
    let content = serde_json::to_string_pretty(&artifact).map_err(|error| {
        SitlError::DryRunArtifactWrite {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    fs::write(path, content).map_err(|error| SitlError::DryRunArtifactWrite {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_sim::{GeoOrigin, RunConfig, Scenario, ScenarioSuiteEntry, UrbanState};
    use swarm_types::{
        Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskKind, TaskStatus, UrbanBlockedPolicy,
        UrbanEdge, UrbanEdgeId, UrbanMap, UrbanNode, UrbanNodeId, UrbanRouteLoop,
    };

    fn agent() -> Agent {
        Agent {
            id: AgentId::from("agent-0".to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose::default(),
            capabilities: vec![],
            current_task: None,
            battery: 100.0,
            comms_range: 1000.0,
            generation: 1,
            speed: 0.0,
            max_range: 1000.0,
            battery_drain_rate: 0.0,
            battery_model: None,
        }
    }

    fn task(id: &str, pose: Option<Pose>) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose,
            grid_cell: None,
            edge_id: None,
            kind: None,
        }
    }

    fn suite(tasks: Vec<Task>) -> ScenarioSuite {
        ScenarioSuite {
            schema_version: "0.1".to_owned(),
            name: "SITL Waypoints".to_owned(),
            description: "test suite".to_owned(),
            scenarios: vec![ScenarioSuiteEntry {
                mission: "sitl".to_owned(),
                profile: "waypoints".to_owned(),
                scenario: Scenario {
                    name: "sitl_waypoints_0".to_owned(),
                    seed: 0,
                    agents: vec![agent()],
                    tasks,
                    ground_nodes: vec![],
                    base_station: None,
                    geo_origin: None,
                },
                run_config: RunConfig {
                    max_ticks: 50,
                    ..Default::default()
                },
            }],
        }
    }

    fn urban_suite(geo_origin: Option<GeoOrigin>) -> ScenarioSuite {
        let n0 = UrbanNodeId::from("n0".to_owned());
        let n1 = UrbanNodeId::from("n1".to_owned());
        ScenarioSuite {
            schema_version: "0.1".to_owned(),
            name: "Urban Patrol Small Block".to_owned(),
            description: "test suite".to_owned(),
            scenarios: vec![ScenarioSuiteEntry {
                mission: "urban-patrol".to_owned(),
                profile: "patrol-small-block".to_owned(),
                scenario: Scenario {
                    name: "urban_patrol_small_block".to_owned(),
                    seed: 0,
                    agents: vec![agent()],
                    tasks: vec![Task {
                        id: TaskId::from("urban-waypoint-n1".to_owned()),
                        status: TaskStatus::Unassigned,
                        assigned_to: None,
                        priority: 1,
                        required_capabilities: vec![],
                        required_role: None,
                        preferred_role: Some(Role::Scout),
                        expires_at: None,
                        pose: Some(Pose {
                            x: 20.0,
                            y: 0.0,
                            ..Default::default()
                        }),
                        grid_cell: None,
                        edge_id: None,
                        kind: Some(TaskKind::Waypoint),
                    }],
                    ground_nodes: vec![],
                    base_station: None,
                    geo_origin,
                },
                run_config: RunConfig {
                    max_ticks: 50,
                    urban_state: Some(UrbanState {
                        map: UrbanMap {
                            nodes: vec![
                                UrbanNode {
                                    id: n0.clone(),
                                    pose: Pose {
                                        x: 0.0,
                                        y: 0.0,
                                        ..Default::default()
                                    },
                                },
                                UrbanNode {
                                    id: n1.clone(),
                                    pose: Pose {
                                        x: 20.0,
                                        y: 0.0,
                                        ..Default::default()
                                    },
                                },
                            ],
                            edges: vec![
                                UrbanEdge {
                                    id: UrbanEdgeId::from("road-n0-n1".to_owned()),
                                    from: n0.clone(),
                                    to: n1.clone(),
                                    cost: 20.0,
                                    length_m: 20.0,
                                    corridor_width_m: Some(6.0),
                                    blocked: false,
                                },
                                UrbanEdge {
                                    id: UrbanEdgeId::from("road-n1-n0".to_owned()),
                                    from: n1.clone(),
                                    to: n0.clone(),
                                    cost: 20.0,
                                    length_m: 20.0,
                                    corridor_width_m: Some(6.0),
                                    blocked: false,
                                },
                            ],
                            static_obstacles: vec![],
                        },
                        route_loop: UrbanRouteLoop {
                            nodes: vec![n0, n1],
                        },
                        start_node: Some(UrbanNodeId::from("n0".to_owned())),
                        planner: "dijkstra".to_owned(),
                        temporary_obstacles: vec![],
                        blocked_route_policy: UrbanBlockedPolicy::default(),
                        perimeter_patrol: None,
                    }),
                    ..Default::default()
                },
            }],
        }
    }

    #[test]
    fn helper_extracts_pose_tasks_with_sequential_ids() {
        let suite = suite(vec![
            task(
                "wp-0",
                Some(Pose {
                    x: 10.0,
                    y: 20.0,
                    z: 3.0,
                }),
            ),
            task("no-pose", None),
            task(
                "wp-1",
                Some(Pose {
                    x: 30.0,
                    y: 40.0,
                    z: 5.0,
                }),
            ),
        ]);
        let plan = build_sitl_plan(&suite, "scenario.json", "agent-0").unwrap();

        assert_eq!(plan.waypoints.len(), 2);
        assert_eq!(plan.waypoints[0].seq, 0);
        assert_eq!(plan.waypoints[0].task_id, "wp-0");
        assert_eq!(plan.waypoints[0].z, 3.0);
        assert_eq!(plan.export_kind, "pose_tasks");
        assert_eq!(plan.planner_or_adapter, "sitl_pose_task_extractor");
        assert_eq!(plan.waypoint_count, 2);
        assert_eq!(plan.geo_origin, None);
        assert_eq!(plan.waypoints[1].seq, 1);
        assert_eq!(plan.waypoints[1].task_id, "wp-1");
        assert_eq!(plan.waypoints[1].x, 30.0);
    }

    #[test]
    fn geo_origin_absent_uses_sitl_default() {
        let suite = suite(vec![task(
            "wp-0",
            Some(Pose {
                x: 10.0,
                y: 20.0,
                z: 3.0,
            }),
        )]);
        let plan = build_sitl_plan(&suite, "scenario.json", "agent-0").unwrap();
        let output = format_dry_run_plan(&plan);

        assert_eq!(plan.geo_origin, None);
        assert!(output.contains("geo_origin: default_sitl_upload_origin"));
    }

    #[test]
    fn urban_patrol_plan_uses_route_export() {
        let origin = GeoOrigin {
            lat_deg: 47.397_742,
            lon_deg: 8.545_594,
            alt_m: 0.0,
        };
        let suite = urban_suite(Some(origin));
        let plan = build_sitl_plan(&suite, "urban.json", "agent-0").unwrap();

        assert_eq!(plan.export_kind, "urban_route");
        assert_eq!(plan.geo_origin, Some(origin));
        assert_eq!(plan.route_length_m, Some(40.0));
        assert_eq!(plan.segment_count, Some(2));
        assert_eq!(plan.waypoint_count, 2);
        assert_eq!(plan.waypoints[0].task_id, "urban-route-0-road-n0-n1-1");
        assert_eq!(plan.waypoints[0].source, "urban_route");
        assert_eq!(plan.waypoints[0].edge_id.as_deref(), Some("road-n0-n1"));
        assert_eq!(plan.waypoints[0].z, 5.0);
    }

    #[test]
    fn urban_route_task_subset_is_rejected() {
        let suite = urban_suite(None);
        let task_ids = vec!["urban-waypoint-n1".to_owned()];
        let error =
            build_sitl_plan_for_task_ids(&suite, "urban.json", "agent-0", &task_ids).unwrap_err();

        assert!(matches!(
            error,
            SitlError::UrbanRouteTaskSubsetUnsupported { .. }
        ));
    }

    #[test]
    fn geo_origin_overrides_default_in_dry_run() {
        let default_plan = build_sitl_plan(&urban_suite(None), "urban.json", "agent-0").unwrap();
        let custom_origin = GeoOrigin {
            lat_deg: 40.0,
            lon_deg: -73.0,
            alt_m: 12.0,
        };
        let custom_plan =
            build_sitl_plan(&urban_suite(Some(custom_origin)), "urban.json", "agent-0").unwrap();

        assert_eq!(default_plan.waypoints[0].x, custom_plan.waypoints[0].x);
        assert_eq!(default_plan.waypoints[0].y, custom_plan.waypoints[0].y);

        let default_artifact = dry_run_artifact(&default_plan, vec![]);
        let custom_artifact = dry_run_artifact(&custom_plan, vec![]);

        assert_eq!(
            default_artifact.effective_geo_origin,
            DEFAULT_SITL_GEO_ORIGIN
        );
        assert_eq!(custom_artifact.effective_geo_origin, custom_origin);
        assert_ne!(
            default_artifact.start_global.unwrap().lat_deg,
            custom_artifact.start_global.unwrap().lat_deg
        );
        assert_ne!(
            default_artifact.start_global.unwrap().lon_deg,
            custom_artifact.start_global.unwrap().lon_deg
        );
    }

    #[test]
    fn build_sitl_plan_for_task_ids_filters_subset() {
        let suite = suite(vec![
            task(
                "wp-0",
                Some(Pose {
                    x: 10.0,
                    y: 20.0,
                    z: 3.0,
                }),
            ),
            task(
                "wp-1",
                Some(Pose {
                    x: 30.0,
                    y: 40.0,
                    z: 5.0,
                }),
            ),
        ]);
        let task_ids = vec!["wp-1".to_owned()];
        let plan =
            build_sitl_plan_for_task_ids(&suite, "scenario.json", "agent-0", &task_ids).unwrap();

        assert_eq!(plan.waypoints.len(), 1);
        assert_eq!(plan.waypoints[0].seq, 0);
        assert_eq!(plan.waypoints[0].task_id, "wp-1");
    }

    #[test]
    fn build_sitl_plan_for_task_ids_preserves_scenario_order() {
        let suite = suite(vec![
            task(
                "wp-0",
                Some(Pose {
                    x: 10.0,
                    y: 20.0,
                    z: 3.0,
                }),
            ),
            task(
                "wp-1",
                Some(Pose {
                    x: 30.0,
                    y: 40.0,
                    z: 5.0,
                }),
            ),
        ]);
        let task_ids = vec!["wp-1".to_owned(), "wp-0".to_owned()];
        let plan =
            build_sitl_plan_for_task_ids(&suite, "scenario.json", "agent-0", &task_ids).unwrap();

        assert_eq!(plan.waypoints[0].task_id, "wp-0");
        assert_eq!(plan.waypoints[1].task_id, "wp-1");
    }

    #[test]
    fn build_sitl_plan_for_task_ids_rejects_empty_subset() {
        let suite = suite(vec![task(
            "wp-0",
            Some(Pose {
                x: 10.0,
                y: 20.0,
                z: 0.0,
            }),
        )]);
        let task_ids = Vec::new();
        let error = build_sitl_plan_for_task_ids(&suite, "scenario.json", "agent-0", &task_ids)
            .unwrap_err();

        assert!(matches!(error, SitlError::EmptyTaskSubset { .. }));
    }

    #[test]
    fn build_sitl_plan_legacy_path_still_returns_all_pose_tasks() {
        let suite = suite(vec![
            task(
                "wp-0",
                Some(Pose {
                    x: 10.0,
                    y: 20.0,
                    z: 3.0,
                }),
            ),
            task(
                "wp-1",
                Some(Pose {
                    x: 30.0,
                    y: 40.0,
                    z: 5.0,
                }),
            ),
        ]);
        let plan = build_sitl_plan(&suite, "scenario.json", "agent-0").unwrap();

        assert_eq!(plan.waypoints.len(), 2);
        assert_eq!(plan.waypoints[0].task_id, "wp-0");
        assert_eq!(plan.waypoints[1].task_id, "wp-1");
    }

    #[test]
    fn helper_returns_no_pose_tasks_error() {
        let suite = suite(vec![task("no-pose", None)]);
        let error = build_sitl_plan(&suite, "scenario.json", "agent-0").unwrap_err();

        assert!(matches!(error, SitlError::NoPoseTasks { .. }));
    }

    #[test]
    fn helper_returns_invalid_scenario_error() {
        let suite = ScenarioSuite {
            schema_version: "0.1".to_owned(),
            name: String::new(),
            description: "test suite".to_owned(),
            scenarios: vec![],
        };
        let error = build_sitl_plan(&suite, "scenario.json", "agent-0").unwrap_err();

        assert!(matches!(error, SitlError::InvalidScenario { .. }));
    }

    #[test]
    fn dry_run_format_contains_contract_fields() {
        let suite = suite(vec![task(
            "wp-0",
            Some(Pose {
                x: 10.0,
                y: 20.0,
                z: 0.0,
            }),
        )]);
        let plan = build_sitl_plan(&suite, "scenario.json", "agent-0").unwrap();
        let output = format_dry_run_plan(&plan);

        assert!(output.contains("mode: dry-run"));
        assert!(output.contains("agent_id: agent-0"));
        assert!(output.contains("scenario_path: scenario.json"));
        assert!(output.contains("suite_name: SITL Waypoints"));
        assert!(output.contains("scenario_name: sitl_waypoints_0"));
        assert!(output.contains("mission: sitl"));
        assert!(output.contains("profile: waypoints"));
        assert!(output.contains("coordinate_frame: local_simulation"));
        assert!(output.contains("altitude_source: pose.z"));
        assert!(output.contains("seq=0 task_id=wp-0 x=10.000 y=20.000 z=0.000"));
    }

    #[test]
    fn unsupported_coordinate_frame_is_typed_error() {
        let error = SitlCoordinateFrame::from_name("global").unwrap_err();

        assert!(matches!(
            error,
            SitlError::UnsupportedCoordinateFrame { frame } if frame == "global"
        ));
    }

    #[test]
    fn bad_connection_string_is_typed_error() {
        let error = validate_connection_string("not-a-connection").unwrap_err();

        assert!(matches!(error, SitlError::BadConnectionString { .. }));
    }

    #[test]
    fn udp_connection_requires_host_and_port() {
        for addr in [
            "udp:",
            "udp:127.0.0.1",
            "udp::14550",
            "udp:127.0.0.1:notaport",
        ] {
            let error = validate_connection_string(addr).unwrap_err();
            assert!(matches!(error, SitlError::BadConnectionString { .. }));
        }
    }

    #[test]
    fn tcp_connection_requires_host_and_port() {
        for addr in [
            "tcp:",
            "tcp:localhost",
            "tcp::5760",
            "tcp:localhost:notaport",
        ] {
            let error = validate_connection_string(addr).unwrap_err();
            assert!(matches!(error, SitlError::BadConnectionString { .. }));
        }
    }

    #[test]
    fn serial_connection_requires_path_and_baud() {
        for addr in [
            "serial:",
            "serial:/dev/ttyUSB0",
            "serial::57600",
            "serial:/dev/ttyUSB0:fast",
        ] {
            let error = validate_connection_string(addr).unwrap_err();
            assert!(matches!(error, SitlError::BadConnectionString { .. }));
        }
    }

    #[test]
    fn supported_connection_strings_are_valid() {
        validate_connection_string("udp:127.0.0.1:14550").unwrap();
        validate_connection_string("udpin:127.0.0.1:14550").unwrap();
        validate_connection_string("udpout:127.0.0.1:14550").unwrap();
        validate_connection_string("tcp:localhost:5760").unwrap();
        validate_connection_string("tcpout:localhost:5760").unwrap();
        validate_connection_string("serial:/dev/ttyUSB0:57600").unwrap();
    }

    #[test]
    fn sitl_connection_class_loopback_udp_is_local_px4_sitl() {
        for addr in [
            "udp:127.0.0.1:14550",
            "udpin:127.0.0.1:14550",
            "udpout:127.0.0.1:14550",
            "udp:localhost:14550",
            "udp:[::1]:14550",
        ] {
            assert_eq!(
                classify_connection_string(addr).unwrap(),
                SitlConnectionClass::LocalPx4SitlUdp
            );
        }
    }

    #[test]
    fn sitl_connection_class_remote_udp_is_hardware_candidate() {
        for addr in [
            "udp:192.168.1.10:14550",
            "udp:10.0.0.5:14550",
            "udpin:0.0.0.0:14550",
            "udpin:192.168.1.10:14550",
        ] {
            assert_eq!(
                classify_connection_string(addr).unwrap(),
                SitlConnectionClass::HardwareCandidate
            );
        }
    }

    #[test]
    fn sitl_connection_class_tcp_is_hardware_candidate() {
        for addr in ["tcp:localhost:5760", "tcp:192.168.1.10:5760"] {
            assert_eq!(
                classify_connection_string(addr).unwrap(),
                SitlConnectionClass::HardwareCandidate
            );
        }
    }

    #[test]
    fn sitl_connection_class_serial_is_hardware_candidate() {
        assert_eq!(
            classify_connection_string("serial:/dev/ttyUSB0:57600").unwrap(),
            SitlConnectionClass::HardwareCandidate
        );
    }

    #[test]
    fn sitl_connection_class_rejects_malformed_connection_strings() {
        for addr in ["not-a-connection", "udp:127.0.0.1", "serial:/dev/ttyUSB0"] {
            let error = classify_connection_string(addr).unwrap_err();
            assert!(matches!(error, SitlError::BadConnectionString { .. }));
        }
    }
}
