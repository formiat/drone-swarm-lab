use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use swarm_sim::ScenarioSuite;

use crate::sitl_plan::{
    build_sitl_plan_for_task_ids, first_sitl_entry, validate_connection_string, SitlError,
    SitlWaypointItem,
};

pub const MULTI_AGENT_SITL_CONFIG_SCHEMA_VERSION: &str = "multi_sitl.v1";
pub const MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION: &str = "multi_sitl_manifest.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiAgentSitlConfig {
    pub schema_version: String,
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
            waypoint_count: plan.waypoints.len(),
            waypoints: plan.waypoints,
            standalone_command,
        });
    }

    Ok(MultiAgentSitlManifest {
        schema_version: MULTI_AGENT_SITL_MANIFEST_SCHEMA_VERSION.to_owned(),
        scenario_path: scenario_path.to_path_buf(),
        scenario_name: entry.scenario.name.clone(),
        mission: entry.mission.clone(),
        profile: entry.profile.clone(),
        agents_count: agents.len(),
        agents,
        ownership_summary: ownership_summary(suite, config)?,
    })
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
            agents: vec![
                MultiAgentSitlAgentConfig {
                    agent_id: "agent-0".to_owned(),
                    system_id: 1,
                    component_id: 1,
                    connection_string: "udp:127.0.0.1:14550".to_owned(),
                    start_delay_ms: 0,
                    lifecycle: MultiAgentLifecycle::UploadOnly,
                    task_ids: vec!["wp-0".to_owned()],
                },
                MultiAgentSitlAgentConfig {
                    agent_id: "agent-1".to_owned(),
                    system_id: 2,
                    component_id: 1,
                    connection_string: "udp:127.0.0.1:14560".to_owned(),
                    start_delay_ms: 10,
                    lifecycle: MultiAgentLifecycle::Execute,
                    task_ids: vec!["wp-1".to_owned()],
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
