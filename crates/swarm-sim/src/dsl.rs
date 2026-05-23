use serde::{Deserialize, Serialize};

use crate::{RunConfig, Scenario};

/// A suite of scenarios with metadata for batch benchmarking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioSuite {
    pub name: String,
    pub description: String,
    pub scenarios: Vec<ScenarioSuiteEntry>,
}

/// A single entry in a scenario suite.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioSuiteEntry {
    pub mission: String,
    pub profile: String,
    pub scenario: Scenario,
    pub run_config: RunConfig,
}

/// Load a `ScenarioSuite` from a JSON file.
pub fn load_scenario_suite(path: &str) -> Result<ScenarioSuite, Box<dyn std::error::Error>> {
    let json = std::fs::read_to_string(path)?;
    let suite: ScenarioSuite = serde_json::from_str(&json)?;
    Ok(suite)
}

/// Serialize a single entry to pretty-printed JSON.
pub fn export_entry(entry: &ScenarioSuiteEntry) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(entry)
}

/// Serialize a full suite to pretty-printed JSON.
pub fn export_suite(suite: &ScenarioSuite) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(suite)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::Scenario;
    use swarm_types::{Agent, Health, Pose, Role, Task, TaskStatus};

    fn make_minimal_entry() -> ScenarioSuiteEntry {
        ScenarioSuiteEntry {
            mission: "coverage".to_owned(),
            profile: "ideal".to_owned(),
            scenario: Scenario {
                name: "test".to_owned(),
                seed: 0,
                agents: vec![Agent {
                    id: swarm_types::AgentId::from("agent-0".to_owned()),
                    role: Role::Scout,
                    health: Health::Alive,
                    pose: Pose { x: 0.0, y: 0.0 },
                    capabilities: vec![],
                    current_task: None,
                    battery: 100.0,
                    comms_range: 1000.0,
                    generation: 1,
                    speed: 0.0,
                    max_range: 0.0,
                    battery_drain_rate: 0.0,
                }],
                tasks: vec![Task {
                    id: swarm_types::TaskId::from("task-0".to_owned()),
                    status: TaskStatus::Unassigned,
                    assigned_to: None,
                    priority: 1,
                    required_capabilities: vec![],
                    required_role: None,
                    preferred_role: None,
                    expires_at: None,
                    pose: None,
                    grid_cell: None,
                    edge_id: None,
                }],
                ground_nodes: vec![],
                base_station: None,
            },
            run_config: RunConfig {
                max_ticks: 50,
                timeout_ticks: 3,
                max_unassigned_ticks: 10,
                packet_loss_rate: 0.0,
                latency_ticks: 0,
                latency_per_hop: 0,
                failures: vec![],
                dynamic_tasks: vec![],
                partition_events: vec![],
                gossip_interval_ticks: 999,
                base_id: None,
                enable_movement: false,
                tick_duration_ms: 100,
                grid_state: None,
                enable_cbba: false,
                ..Default::default()
            },
        }
    }

    #[test]
    fn scenario_suite_entry_json_roundtrip() {
        let entry = make_minimal_entry();
        let json = serde_json::to_string_pretty(&entry).unwrap();
        let parsed: ScenarioSuiteEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mission, "coverage");
        assert_eq!(parsed.profile, "ideal");
        assert_eq!(parsed.scenario.name, "test");
        assert_eq!(parsed.run_config.max_ticks, 50);
    }

    #[test]
    fn scenario_suite_entry_json_contains_mission_and_profile() {
        let entry = make_minimal_entry();
        let json = serde_json::to_string_pretty(&entry).unwrap();
        assert!(json.contains("\"mission\""));
        assert!(json.contains("\"profile\""));
        assert!(json.contains("\"coverage\""));
        assert!(json.contains("\"ideal\""));
    }

    #[test]
    fn scenario_suite_load_from_file() {
        let suite = ScenarioSuite {
            name: "Test Suite".to_owned(),
            description: "A test suite".to_owned(),
            scenarios: vec![make_minimal_entry()],
        };
        let json = serde_json::to_string_pretty(&suite).unwrap();
        let tmp = "/tmp/test_scenario_suite.json";
        std::fs::write(tmp, &json).unwrap();
        let loaded = load_scenario_suite(tmp).unwrap();
        assert_eq!(loaded.name, "Test Suite");
        assert_eq!(loaded.scenarios.len(), 1);
        assert_eq!(loaded.scenarios[0].mission, "coverage");
        std::fs::remove_file(tmp).unwrap();
    }

    #[test]
    fn scenario_json_roundtrip() {
        let entry = make_minimal_entry();
        let json = serde_json::to_string_pretty(&entry.scenario).unwrap();
        let parsed: Scenario = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.seed, 0);
        assert_eq!(parsed.agents.len(), 1);
        assert_eq!(parsed.tasks.len(), 1);
    }

    #[test]
    fn run_config_json_roundtrip() {
        let entry = make_minimal_entry();
        let json = serde_json::to_string_pretty(&entry.run_config).unwrap();
        let parsed: RunConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_ticks, 50);
        assert_eq!(parsed.timeout_ticks, 3);
        assert_eq!(parsed.max_unassigned_ticks, 10);
        assert!(parsed.failures.is_empty());
    }

    #[test]
    fn run_config_json_defaults_work() {
        let json = r#"{"max_ticks": 30}"#;
        let parsed: RunConfig = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.max_ticks, 30);
        assert_eq!(parsed.timeout_ticks, 0);
        assert_eq!(parsed.max_unassigned_ticks, 10);
        assert_eq!(parsed.gossip_interval_ticks, 999);
        assert_eq!(parsed.tick_duration_ms, 100);
        assert!(!parsed.enable_cbba);
    }

    #[test]
    fn scenario_suite_entry_integration_export() {
        let entry = make_minimal_entry();
        let json = export_entry(&entry).unwrap();
        assert!(!json.is_empty());
        let suite = ScenarioSuite {
            name: "Export Suite".to_owned(),
            description: "Suite for export test".to_owned(),
            scenarios: vec![entry],
        };
        let suite_json = export_suite(&suite).unwrap();
        assert!(suite_json.contains("Export Suite"));
    }

    #[test]
    fn load_coverage_example_scenario() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scenarios/coverage.ideal.json"
        );
        let suite = load_scenario_suite(path).unwrap();
        assert_eq!(suite.name, "Coverage Quick Bench");
        assert_eq!(suite.scenarios.len(), 1);
        let entry = &suite.scenarios[0];
        assert_eq!(entry.mission, "coverage");
        assert_eq!(entry.profile, "ideal-no-failures");
        assert_eq!(entry.scenario.agents.len(), 5);
        assert_eq!(entry.scenario.tasks.len(), 3);
    }

    #[test]
    fn load_emergency_mesh_example_scenario() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scenarios/emergency-mesh.ideal.json"
        );
        let suite = load_scenario_suite(path).unwrap();
        assert_eq!(suite.name, "Emergency Mesh Quick Bench");
        let entry = &suite.scenarios[0];
        assert_eq!(entry.mission, "emergency-mesh");
        assert_eq!(entry.profile, "ideal");
        assert_eq!(entry.scenario.ground_nodes.len(), 1);
        assert_eq!(
            entry.run_config.base_id,
            Some(swarm_types::AgentId::from("base".to_owned()))
        );
    }

    #[test]
    fn load_sar_example_scenario() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scenarios/sar.ideal.json"
        );
        let suite = load_scenario_suite(path).unwrap();
        assert_eq!(suite.name, "SAR Quick Bench");
        let entry = &suite.scenarios[0];
        assert_eq!(entry.mission, "sar");
        assert!(entry.run_config.enable_movement);
        assert!(entry.run_config.grid_state.is_some());
        let gs = entry.run_config.grid_state.as_ref().unwrap();
        assert_eq!(gs.targets.len(), 2);
        assert_eq!(gs.grid.width, 6);
        assert_eq!(gs.grid.height, 6);
    }
}
