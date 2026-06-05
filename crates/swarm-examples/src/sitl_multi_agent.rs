use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use swarm_command_plane::{
    build_swarm_command_plan, AgentCommandAssignment, PartialSuccessPolicy, SwarmAbortPolicy,
    SwarmCommandArtifactSummary, SwarmCommandFanoutInput, SwarmCommandPlan, SwarmCommandRole,
    SwarmOwnershipKind, SwarmOwnershipRecord, SwarmOwnershipRef, SwarmOwnershipStatus,
    SwarmTopologyConfig, SwarmTopologyKind, SynchronizedCommandKind, SynchronizedCommandWindow,
};
use swarm_comms::{MavlinkCommonPlanOptions, MavlinkCoordinateOrigin};
use swarm_mission_ir::{
    AltitudeReference, CommandId, CompletionTolerance, CoordinateFrame, LocalPosition,
    MissionCommand, MissionCommandEntry, MissionCommandPlan, MissionId, MissionWaypoint, Position,
    RouteId, TerminalState, TimeoutAction, TimeoutPolicy,
};
use swarm_sim::ScenarioSuite;
use swarm_types::AgentId;

use crate::sitl_plan::{
    build_sitl_plan_for_task_ids, first_sitl_entry, validate_connection_string, SitlError,
    SitlWaypointItem, DEFAULT_SITL_GEO_ORIGIN,
};

pub const MULTI_AGENT_SITL_CONFIG_SCHEMA_VERSION: &str = "multi_sitl.v1";
pub const MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION: &str = "multi_sitl_manifest.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiAgentSitlConfig {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<SwarmTopologyConfig>,
    pub agents: Vec<MultiAgentSitlAgentConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiAgentSitlAgentConfig {
    pub agent_id: String,
    pub system_id: u8,
    pub component_id: u8,
    pub connection_string: String,
    pub start_delay_ms: u64,
    pub lifecycle: MultiAgentLifecycle,
    pub task_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_role: Option<SwarmCommandRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abort_policy: Option<SwarmAbortPolicy>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiAgentLifecycle {
    UploadOnly,
    Execute,
}

impl MultiAgentLifecycle {
    pub fn cli_flag(self) -> &'static str {
        match self {
            Self::UploadOnly => "--upload-only",
            Self::Execute => "--execute",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiAgentSitlManifest {
    pub schema_version: String,
    pub scenario_path: PathBuf,
    pub scenario_name: String,
    pub mission: String,
    pub profile: String,
    pub agents_count: usize,
    pub agents: Vec<MultiAgentSitlManifestAgent>,
    pub ownership_summary: TaskOwnershipSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_plane: Option<SwarmCommandArtifactSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<MultiAgentSitlTopologySummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_plane_artifact: Option<SwarmCommandPlan>,
    #[serde(default)]
    pub artifact_metadata: SitlArtifactMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiAgentSitlTopologySummary {
    pub kind: SwarmTopologyKind,
    pub node_count: usize,
    pub link_count: usize,
    pub route_count: usize,
    pub degraded_route_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitlArtifactMetadata {
    pub command: Vec<String>,
    pub git_commit: Option<String>,
    pub build_profile: String,
    pub run_id: Option<String>,
    pub scenario_snapshot_path: Option<PathBuf>,
    pub config_snapshot_path: Option<PathBuf>,
    pub command_path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiAgentSitlManifestAgent {
    pub agent_id: String,
    pub system_id: u8,
    pub component_id: u8,
    pub connection_string: String,
    pub start_delay_ms: u64,
    pub lifecycle: MultiAgentLifecycle,
    pub task_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_role: Option<SwarmCommandRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abort_policy: Option<SwarmAbortPolicy>,
    pub waypoint_count: usize,
    pub waypoints: Vec<SitlWaypointItem>,
    pub standalone_command: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskOwnershipSummary {
    pub total_pose_tasks: usize,
    pub assigned_task_count: usize,
    pub unassigned_pose_tasks: Vec<String>,
    pub duplicate_task_ids: Vec<String>,
}

pub fn load_multi_agent_config(
    config_path: impl AsRef<Path>,
) -> Result<MultiAgentSitlConfig, SitlError> {
    let config_path = config_path.as_ref();
    let json =
        std::fs::read_to_string(config_path).map_err(|error| SitlError::MultiAgentConfigRead {
            path: config_path.to_path_buf(),
            message: error.to_string(),
        })?;
    serde_json::from_str(&json).map_err(|error| SitlError::MultiAgentConfigParse {
        path: config_path.to_path_buf(),
        message: error.to_string(),
    })
}

pub fn agent_config<'a>(
    config: &'a MultiAgentSitlConfig,
    agent_id: &str,
) -> Result<&'a MultiAgentSitlAgentConfig, SitlError> {
    config
        .agents
        .iter()
        .find(|agent| agent.agent_id == agent_id)
        .ok_or_else(|| SitlError::MultiAgentConfigInvalid {
            message: format!("agent_id '{agent_id}' is not present in multi-agent config"),
        })
}

pub fn build_multi_agent_manifest(
    suite: &ScenarioSuite,
    scenario_path: impl AsRef<Path>,
    config_path: impl AsRef<Path>,
    config: &MultiAgentSitlConfig,
) -> Result<MultiAgentSitlManifest, SitlError> {
    validate_multi_agent_config(suite, config)?;

    let scenario_path = scenario_path.as_ref();
    let config_path = config_path.as_ref();
    let entry = first_sitl_entry(suite, scenario_path)?;
    let mut agents = Vec::with_capacity(config.agents.len());

    for agent in &config.agents {
        let plan =
            build_sitl_plan_for_task_ids(suite, scenario_path, &agent.agent_id, &agent.task_ids)?;
        let standalone_command = standalone_sitl_agent_command(scenario_path, config_path, agent);
        agents.push(MultiAgentSitlManifestAgent {
            agent_id: agent.agent_id.clone(),
            system_id: agent.system_id,
            component_id: agent.component_id,
            connection_string: agent.connection_string.clone(),
            start_delay_ms: agent.start_delay_ms,
            lifecycle: agent.lifecycle,
            task_ids: agent.task_ids.clone(),
            command_role: agent.command_role.clone(),
            abort_policy: agent.abort_policy.clone(),
            waypoint_count: plan.waypoints.len(),
            waypoints: plan.waypoints,
            standalone_command,
        });
    }

    let ownership_summary = ownership_summary(suite, config)?;
    let command_plane_artifact = build_manifest_command_plane(entry, config, &agents)?;
    let command_plane = Some(command_plane_artifact.summary.clone());
    let topology = topology_summary(&command_plane_artifact);

    Ok(MultiAgentSitlManifest {
        schema_version: MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION.to_owned(),
        scenario_path: scenario_path.to_path_buf(),
        scenario_name: entry.scenario.name.clone(),
        mission: entry.mission.clone(),
        profile: entry.profile.clone(),
        agents_count: agents.len(),
        agents,
        ownership_summary,
        command_plane,
        topology,
        command_plane_artifact: Some(command_plane_artifact),
        artifact_metadata: SitlArtifactMetadata::default(),
    })
}

fn topology_summary(plan: &SwarmCommandPlan) -> Option<MultiAgentSitlTopologySummary> {
    let topology = plan.topology.as_ref()?;
    Some(MultiAgentSitlTopologySummary {
        kind: topology.kind.clone(),
        node_count: topology.nodes.len(),
        link_count: topology.links.len(),
        route_count: plan.command_routes.len(),
        degraded_route_count: plan
            .command_routes
            .iter()
            .filter(|route| route.degraded)
            .count(),
    })
}

fn build_manifest_command_plane(
    entry: &swarm_sim::ScenarioSuiteEntry,
    config: &MultiAgentSitlConfig,
    agents: &[MultiAgentSitlManifestAgent],
) -> Result<SwarmCommandPlan, SitlError> {
    let plan_id = format!(
        "{}:{}:{}",
        entry.scenario.name, entry.mission, entry.profile
    );
    let assignments = agents
        .iter()
        .map(|agent| AgentCommandAssignment {
            agent_id: AgentId::from(agent.agent_id.clone()),
            role: agent
                .command_role
                .clone()
                .unwrap_or(SwarmCommandRole::Scout),
            command_plan: manifest_agent_command_plan(&plan_id, agent),
            abort_policy: agent
                .abort_policy
                .clone()
                .unwrap_or(SwarmAbortPolicy::AbortMission),
            ownership_refs: agent
                .task_ids
                .iter()
                .map(|task_id| SwarmOwnershipRef {
                    kind: SwarmOwnershipKind::Task,
                    resource_id: task_id.clone(),
                })
                .collect(),
        })
        .collect();
    let ownership = agents
        .iter()
        .flat_map(|agent| {
            agent.task_ids.iter().map(|task_id| SwarmOwnershipRecord {
                agent_id: AgentId::from(agent.agent_id.clone()),
                kind: SwarmOwnershipKind::Task,
                resource_id: task_id.clone(),
                status: SwarmOwnershipStatus::Active,
                tick: 0,
                reason: "manifest_assignment".to_owned(),
            })
        })
        .collect();
    build_swarm_command_plan(SwarmCommandFanoutInput {
        plan_id,
        assignments,
        ownership,
        global_abort_policy: SwarmAbortPolicy::AbortMission,
        sync_operations: sync_operations_for_config(config),
        topology: config.topology.clone(),
        mavlink_options: MavlinkCommonPlanOptions {
            home_origin: Some(MavlinkCoordinateOrigin {
                lat_deg: DEFAULT_SITL_GEO_ORIGIN.lat_deg,
                lon_deg: DEFAULT_SITL_GEO_ORIGIN.lon_deg,
                alt_m: DEFAULT_SITL_GEO_ORIGIN.alt_m,
            }),
            ..Default::default()
        },
    })
    .map_err(|error| SitlError::MultiAgentConfigInvalid {
        message: format!("command-plane build failed: {error}"),
    })
}

fn manifest_agent_command_plan(
    plan_id: &str,
    agent: &MultiAgentSitlManifestAgent,
) -> MissionCommandPlan {
    let route_id_value = format!("{plan_id}:{}", agent.agent_id);
    let route_id = RouteId::from(route_id_value.clone());
    let waypoints: Vec<MissionWaypoint> = agent
        .waypoints
        .iter()
        .map(|waypoint| MissionWaypoint {
            position: Position::Local(LocalPosition {
                x_m: waypoint.x,
                y_m: waypoint.y,
                z_m: waypoint.z,
            }),
            acceptance_radius_m: None,
        })
        .collect();
    let commands = if waypoints.is_empty() {
        Vec::new()
    } else {
        vec![MissionCommandEntry {
            command_id: CommandId::from(format!("{plan_id}:{}:follow-route", agent.agent_id)),
            command: MissionCommand::FollowRoute {
                route_id: route_id.clone(),
                waypoints,
            },
            source_task_id: None,
            source_route_id: Some(route_id_value),
            source_agent_id: Some(agent.agent_id.clone()),
        }]
    };
    MissionCommandPlan {
        schema_version: MissionCommandPlan::SCHEMA_VERSION.to_owned(),
        mission_id: MissionId::from(format!("{plan_id}:{}", agent.agent_id)),
        coordinate_frame: CoordinateFrame::LocalNed,
        altitude_reference: AltitudeReference::RelativeHome,
        timeout_policy: TimeoutPolicy {
            command_timeout_secs: 5.0,
            completion_timeout_secs: 120.0,
            on_timeout: TimeoutAction::Abort,
        },
        expected_terminal_state: TerminalState::Landed,
        completion_tolerance: CompletionTolerance {
            position_m: 1.0,
            altitude_m: 0.5,
        },
        commands,
    }
}

fn sync_operations_for_config(config: &MultiAgentSitlConfig) -> Vec<SynchronizedCommandWindow> {
    let agent_ids: Vec<AgentId> = config
        .agents
        .iter()
        .map(|agent| AgentId::from(agent.agent_id.clone()))
        .collect();
    [
        SynchronizedCommandKind::ArmAll,
        SynchronizedCommandKind::TakeoffAll,
        SynchronizedCommandKind::StartAll,
        SynchronizedCommandKind::AbortAll,
    ]
    .into_iter()
    .map(|kind| SynchronizedCommandWindow {
        kind,
        agent_ids: agent_ids.clone(),
        timeout_ms: 5_000,
        partial_success_policy: PartialSuccessPolicy::RequireAll,
    })
    .collect()
}

fn validate_multi_agent_config(
    suite: &ScenarioSuite,
    config: &MultiAgentSitlConfig,
) -> Result<(), SitlError> {
    if config.schema_version != MULTI_AGENT_SITL_CONFIG_SCHEMA_VERSION {
        return invalid_config(format!(
            "unsupported schema_version '{}' (expected {MULTI_AGENT_SITL_CONFIG_SCHEMA_VERSION})",
            config.schema_version
        ));
    }
    if config.agents.is_empty() {
        return invalid_config("agents must not be empty");
    }

    let entry = first_sitl_entry(suite, "multi-agent config")?;
    let scenario_agent_ids: HashSet<String> = entry
        .scenario
        .agents
        .iter()
        .map(|agent| agent.id.to_string())
        .collect();
    let pose_task_ids: HashSet<String> = entry
        .scenario
        .tasks
        .iter()
        .filter(|task| task.pose.is_some())
        .map(|task| task.id.to_string())
        .collect();
    let all_task_ids: HashSet<String> = entry
        .scenario
        .tasks
        .iter()
        .map(|task| task.id.to_string())
        .collect();

    let mut seen_agents = HashSet::new();
    let mut task_owners: HashMap<String, Vec<String>> = HashMap::new();

    for agent in &config.agents {
        if agent.agent_id.trim().is_empty() {
            return invalid_config("agent_id must not be empty");
        }
        if !seen_agents.insert(agent.agent_id.clone()) {
            return invalid_config(format!("duplicate agent_id '{}'", agent.agent_id));
        }
        if !scenario_agent_ids.contains(&agent.agent_id) {
            return invalid_config(format!(
                "agent_id '{}' is not present in scenario agents",
                agent.agent_id
            ));
        }
        if agent.system_id == 0 {
            return invalid_config(format!(
                "agent '{}' has invalid system_id 0",
                agent.agent_id
            ));
        }
        validate_connection_string(&agent.connection_string)?;
        if agent.task_ids.is_empty() {
            return invalid_config(format!(
                "agent '{}' task_ids must not be empty",
                agent.agent_id
            ));
        }

        let mut seen_task_ids = HashSet::new();
        for task_id in &agent.task_ids {
            if !seen_task_ids.insert(task_id.clone()) {
                return invalid_config(format!(
                    "agent '{}' contains duplicate task_id '{}'",
                    agent.agent_id, task_id
                ));
            }
            if !all_task_ids.contains(task_id) {
                return invalid_config(format!(
                    "task_id '{task_id}' assigned to '{}' is not present in scenario tasks",
                    agent.agent_id
                ));
            }
            if !pose_task_ids.contains(task_id) {
                return invalid_config(format!(
                    "task_id '{task_id}' assigned to '{}' does not have pose",
                    agent.agent_id
                ));
            }
            task_owners
                .entry(task_id.clone())
                .or_default()
                .push(agent.agent_id.clone());
        }
    }

    for (task_id, owners) in task_owners {
        if owners.len() > 1 {
            return invalid_config(format!(
                "duplicate ownership task_id={task_id} agents={}",
                owners.join(",")
            ));
        }
    }

    Ok(())
}

fn ownership_summary(
    suite: &ScenarioSuite,
    config: &MultiAgentSitlConfig,
) -> Result<TaskOwnershipSummary, SitlError> {
    let entry = first_sitl_entry(suite, "multi-agent config")?;
    let pose_task_ids: Vec<String> = entry
        .scenario
        .tasks
        .iter()
        .filter(|task| task.pose.is_some())
        .map(|task| task.id.to_string())
        .collect();
    let assigned: HashSet<String> = config
        .agents
        .iter()
        .flat_map(|agent| agent.task_ids.iter().cloned())
        .collect();
    let unassigned_pose_tasks = pose_task_ids
        .iter()
        .filter(|task_id| !assigned.contains(*task_id))
        .cloned()
        .collect();

    Ok(TaskOwnershipSummary {
        total_pose_tasks: pose_task_ids.len(),
        assigned_task_count: assigned.len(),
        unassigned_pose_tasks,
        duplicate_task_ids: Vec::new(),
    })
}

fn standalone_sitl_agent_command(
    scenario_path: &Path,
    config_path: &Path,
    agent: &MultiAgentSitlAgentConfig,
) -> Vec<String> {
    vec![
        "cargo".to_owned(),
        "run".to_owned(),
        "-p".to_owned(),
        "swarm-examples".to_owned(),
        "--bin".to_owned(),
        "sitl_agent".to_owned(),
        "--features".to_owned(),
        "mavlink-transport".to_owned(),
        "--".to_owned(),
        "--scenario".to_owned(),
        scenario_path.display().to_string(),
        "--agent-id".to_owned(),
        agent.agent_id.clone(),
        "--multi-agent-config".to_owned(),
        config_path.display().to_string(),
        "--connection".to_owned(),
        agent.connection_string.clone(),
        agent.lifecycle.cli_flag().to_owned(),
    ]
}

fn invalid_config<T>(message: impl Into<String>) -> Result<T, SitlError> {
    Err(SitlError::MultiAgentConfigInvalid {
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_sim::{RunConfig, Scenario, ScenarioSuiteEntry};
    use swarm_types::{Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskStatus};

    fn agent(id: &str) -> Agent {
        Agent {
            id: AgentId::from(id.to_owned()),
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

    fn suite() -> ScenarioSuite {
        ScenarioSuite {
            schema_version: "0.1".to_owned(),
            name: "SITL Waypoints".to_owned(),
            description: "multi sitl test suite".to_owned(),
            generator_manifest: None,
            scenarios: vec![ScenarioSuiteEntry {
                mission: "sitl".to_owned(),
                profile: "waypoints".to_owned(),
                scenario: Scenario {
                    name: "sitl_waypoints_multi".to_owned(),
                    seed: 0,
                    agents: vec![agent("agent-0"), agent("agent-1")],
                    tasks: vec![
                        task(
                            "wp-0",
                            Some(Pose {
                                x: 1.0,
                                y: 2.0,
                                z: 3.0,
                            }),
                        ),
                        task(
                            "wp-1",
                            Some(Pose {
                                x: 4.0,
                                y: 5.0,
                                z: 6.0,
                            }),
                        ),
                        task("no-pose", None),
                    ],
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

    fn config() -> MultiAgentSitlConfig {
        MultiAgentSitlConfig {
            schema_version: MULTI_AGENT_SITL_CONFIG_SCHEMA_VERSION.to_owned(),
            topology: None,
            agents: vec![
                MultiAgentSitlAgentConfig {
                    agent_id: "agent-0".to_owned(),
                    system_id: 1,
                    component_id: 1,
                    connection_string: "udp:127.0.0.1:14550".to_owned(),
                    start_delay_ms: 0,
                    lifecycle: MultiAgentLifecycle::UploadOnly,
                    task_ids: vec!["wp-0".to_owned()],
                    command_role: None,
                    abort_policy: None,
                },
                MultiAgentSitlAgentConfig {
                    agent_id: "agent-1".to_owned(),
                    system_id: 2,
                    component_id: 1,
                    connection_string: "udp:127.0.0.1:14560".to_owned(),
                    start_delay_ms: 10,
                    lifecycle: MultiAgentLifecycle::Execute,
                    task_ids: vec!["wp-1".to_owned()],
                    command_role: None,
                    abort_policy: None,
                },
            ],
        }
    }

    #[test]
    fn multi_agent_config_parse_test() {
        let json = r#"{
          "schema_version": "multi_sitl.v1",
          "agents": [{
            "agent_id": "agent-0",
            "system_id": 1,
            "component_id": 1,
            "connection_string": "udp:127.0.0.1:14550",
            "start_delay_ms": 0,
            "lifecycle": "upload_only",
            "task_ids": ["wp-0"]
          }]
        }"#;
        let parsed: MultiAgentSitlConfig = serde_json::from_str(json).unwrap();

        assert_eq!(parsed.agents[0].lifecycle, MultiAgentLifecycle::UploadOnly);
        assert!(serde_json::to_string(&parsed)
            .unwrap()
            .contains("upload_only"));
    }

    #[test]
    fn agent_connection_config_parse_test() {
        let config = config();
        let agent = &config.agents[1];

        assert_eq!(agent.agent_id, "agent-1");
        assert_eq!(agent.system_id, 2);
        assert_eq!(agent.component_id, 1);
        assert_eq!(agent.connection_string, "udp:127.0.0.1:14560");
        assert_eq!(agent.start_delay_ms, 10);
    }

    #[test]
    fn multi_agent_config_rejects_empty_agents() {
        let config = MultiAgentSitlConfig {
            schema_version: MULTI_AGENT_SITL_CONFIG_SCHEMA_VERSION.to_owned(),
            topology: None,
            agents: vec![],
        };
        let error = build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &config)
            .unwrap_err();

        assert!(error.to_string().contains("agents must not be empty"));
    }

    #[test]
    fn multi_agent_config_rejects_duplicate_agent_id() {
        let mut config = config();
        config.agents[1].agent_id = "agent-0".to_owned();
        let error = build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &config)
            .unwrap_err();

        assert!(error.to_string().contains("duplicate agent_id"));
    }

    #[test]
    fn multi_agent_config_rejects_bad_connection_string() {
        let mut config = config();
        config.agents[0].connection_string = "bad".to_owned();
        let error = build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &config)
            .unwrap_err();

        assert!(matches!(error, SitlError::BadConnectionString { .. }));
    }

    #[test]
    fn multi_agent_config_rejects_invalid_system_id_zero() {
        let mut config = config();
        config.agents[0].system_id = 0;
        let error = build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &config)
            .unwrap_err();

        assert!(error.to_string().contains("invalid system_id 0"));
    }

    #[test]
    fn task_split_test() {
        let manifest =
            build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &config()).unwrap();

        assert_eq!(manifest.agents[0].waypoints[0].task_id, "wp-0");
        assert_eq!(manifest.agents[0].waypoints[0].seq, 0);
        assert_eq!(manifest.agents[1].waypoints[0].task_id, "wp-1");
        assert_eq!(manifest.agents[1].waypoints[0].seq, 0);
    }

    #[test]
    fn duplicate_ownership_rejection_test() {
        let mut config = config();
        config.agents[1].task_ids = vec!["wp-0".to_owned()];
        let error = build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &config)
            .unwrap_err();

        assert!(error.to_string().contains("duplicate ownership"));
        assert!(error.to_string().contains("task_id=wp-0"));
    }

    #[test]
    fn unknown_task_id_rejection_test() {
        let mut config = config();
        config.agents[0].task_ids = vec!["missing".to_owned()];
        let error = build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &config)
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("is not present in scenario tasks"));
    }

    #[test]
    fn task_without_pose_rejection_test() {
        let mut config = config();
        config.agents[0].task_ids = vec!["no-pose".to_owned()];
        let error = build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &config)
            .unwrap_err();

        assert!(error.to_string().contains("does not have pose"));
    }

    #[test]
    fn multi_agent_dry_run_manifest_test() {
        let manifest =
            build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &config()).unwrap();

        assert_eq!(manifest.schema_version, "multi_sitl_manifest.v1");
        assert_eq!(manifest.agents_count, 2);
        assert_eq!(manifest.ownership_summary.total_pose_tasks, 2);
        assert_eq!(manifest.ownership_summary.assigned_task_count, 2);
        assert_eq!(manifest.agents[0].connection_string, "udp:127.0.0.1:14550");
        assert_eq!(manifest.agents[0].standalone_command[0], "cargo");
    }

    #[test]
    fn sitl_topology_fixture_embeds_command_routes() {
        let mut config: MultiAgentSitlConfig = serde_json::from_str(include_str!(
            "../../../scenarios/sitl.multi-agent.topology.mesh-partition.json"
        ))
        .unwrap();
        config.agents[0].task_ids = vec!["wp-0".to_owned()];
        config.agents[1].task_ids = vec!["wp-1".to_owned()];

        let manifest =
            build_multi_agent_manifest(&suite(), "scenario.json", "topology.json", &config)
                .unwrap();

        let topology = manifest.topology.as_ref().unwrap();
        let command_plane = manifest.command_plane.as_ref().unwrap();
        let artifact = manifest.command_plane_artifact.as_ref().unwrap();

        assert_eq!(topology.kind, SwarmTopologyKind::Mesh);
        assert_eq!(command_plane.topology_kind, Some(SwarmTopologyKind::Mesh));
        assert_eq!(topology.route_count, 2);
        assert_eq!(artifact.command_routes.len(), 2);
        assert!(artifact.command_routes.iter().any(|route| !route.allowed));
    }

    #[test]
    fn unassigned_pose_tasks_are_reported_test() {
        let mut invalid_config = config();
        invalid_config.agents[1].task_ids = vec![];
        let error =
            build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &invalid_config)
                .unwrap_err();
        assert!(error.to_string().contains("task_ids must not be empty"));

        let base_config = config();
        let config = MultiAgentSitlConfig {
            schema_version: MULTI_AGENT_SITL_CONFIG_SCHEMA_VERSION.to_owned(),
            topology: None,
            agents: vec![base_config.agents[0].clone()],
        };
        let manifest =
            build_multi_agent_manifest(&suite(), "scenario.json", "multi.json", &config).unwrap();
        assert_eq!(
            manifest.ownership_summary.unassigned_pose_tasks,
            vec!["wp-1".to_owned()]
        );
    }
}
