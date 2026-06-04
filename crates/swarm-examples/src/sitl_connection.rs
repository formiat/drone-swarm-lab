use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use swarm_comms::Waypoint;
use swarm_sim::ScenarioSuiteEntry;

use crate::sitl_plan::{SitlError, SitlPlan, SitlWaypointItem};
use crate::sitl_safety::{
    load_sitl_safety_config, validate_pre_upload_safety, validate_pre_upload_safety_for_task_ids,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SitlConnectionLifecycle {
    pub timeout: Duration,
    pub telemetry_timeout: Duration,
    pub no_progress_timeout: Duration,
    pub no_arm: bool,
    pub abort_after: Option<Duration>,
}

impl Default for SitlConnectionLifecycle {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(2),
            telemetry_timeout: Duration::from_secs(10),
            no_progress_timeout: Duration::from_secs(60),
            no_arm: false,
            abort_after: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SitlSafetyGate {
    pub safety_config_path: Option<String>,
}

impl SitlSafetyGate {
    pub fn new(safety_config_path: Option<String>) -> Self {
        Self { safety_config_path }
    }

    pub fn validate_agent_task_subset(
        &self,
        entry: &ScenarioSuiteEntry,
        agent_id: &str,
        task_ids: &[String],
    ) -> Result<(), SitlError> {
        let config = load_sitl_safety_config(self.safety_config_path.as_deref().map(Path::new))?;
        if task_ids.is_empty() {
            validate_pre_upload_safety(entry, agent_id, &config)
        } else {
            validate_pre_upload_safety_for_task_ids(entry, agent_id, &config, task_ids)
        }
    }
}

pub fn waypoints_from_sitl_items(items: &[SitlWaypointItem]) -> Vec<Waypoint> {
    items
        .iter()
        .map(|waypoint| Waypoint {
            x: waypoint.x,
            y: waypoint.y,
            z: waypoint.z,
            seq: waypoint.seq,
        })
        .collect()
}

pub fn task_ids_by_seq_from_plan(plan: &SitlPlan) -> BTreeMap<u16, String> {
    task_ids_by_seq_from_items(&plan.waypoints)
}

pub fn task_ids_by_seq_from_items(items: &[SitlWaypointItem]) -> BTreeMap<u16, String> {
    items
        .iter()
        .map(|waypoint| (waypoint.seq, waypoint.task_id.clone()))
        .collect()
}

pub fn default_takeoff_altitude(items: &[SitlWaypointItem]) -> f32 {
    items
        .first()
        .map(|waypoint| (waypoint.z as f32).max(2.5))
        .unwrap_or(2.5)
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

    fn task(id: &str, pose: Pose) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(pose),
            grid_cell: None,
            edge_id: None,
            kind: None,
        }
    }

    fn entry() -> ScenarioSuiteEntry {
        ScenarioSuiteEntry {
            mission: "sitl".to_owned(),
            profile: "unit".to_owned(),
            scenario: Scenario {
                name: "safety_gate_unit".to_owned(),
                seed: 0,
                agents: vec![agent("agent-0"), agent("agent-1")],
                tasks: vec![
                    task(
                        "safe",
                        Pose {
                            x: 1.0,
                            y: 1.0,
                            z: 3.0,
                        },
                    ),
                    task(
                        "unsafe",
                        Pose {
                            x: 1500.0,
                            y: 1500.0,
                            z: 3.0,
                        },
                    ),
                ],
                ground_nodes: vec![],
                base_station: None,
                geo_origin: None,
            },
            run_config: RunConfig::default(),
        }
    }

    fn sitl_waypoint(seq: u16, task_id: &str, x: f64, y: f64, z: f64) -> SitlWaypointItem {
        SitlWaypointItem {
            seq,
            task_id: task_id.to_owned(),
            x,
            y,
            z,
            geo: None,
            source: "pose_task".to_owned(),
            edge_id: None,
            from_node_id: None,
            to_node_id: None,
            segment_index: None,
            point_index_on_segment: None,
        }
    }

    #[test]
    fn safety_gate_allows_safe_selected_subset() {
        let gate = SitlSafetyGate::new(None);
        let task_ids = vec!["safe".to_owned()];

        gate.validate_agent_task_subset(&entry(), "agent-0", &task_ids)
            .unwrap();
    }

    #[test]
    fn safety_gate_rejects_unsafe_selected_subset() {
        let gate = SitlSafetyGate::new(None);
        let task_ids = vec!["unsafe".to_owned()];

        let error = gate
            .validate_agent_task_subset(&entry(), "agent-1", &task_ids)
            .unwrap_err();

        assert!(error.to_string().contains("safety validation failed"));
        assert!(error.to_string().contains("task_id=unsafe"));
    }

    #[test]
    fn waypoint_helpers_keep_task_seq_mapping() {
        let plan = SitlPlan {
            agent_id: "agent-0".to_owned(),
            scenario_path: "scenario.json".into(),
            suite_name: "suite".to_owned(),
            scenario_name: "scenario".to_owned(),
            mission: "sitl".to_owned(),
            profile: "unit".to_owned(),
            coordinate_frame: crate::sitl_plan::SitlCoordinateFrame::LocalSimulation,
            coordinate_mode: "local_with_origin".to_owned(),
            altitude_source: "pose.z".to_owned(),
            geo_origin: None,
            export_kind: "pose_tasks".to_owned(),
            planner_or_adapter: "sitl_pose_task_extractor".to_owned(),
            route_length_m: None,
            segment_count: None,
            waypoint_count: 1,
            waypoints: vec![sitl_waypoint(7, "wp-7", 1.0, 2.0, 3.0)],
            safety_report: swarm_safety::preflight::SafetyValidationReport::ok(),
            urban_mission_template: None,
            urban_blocked_route_policy: None,
            urban_mock_perception: None,
            primitive_mission: None,
        };

        let waypoints = waypoints_from_sitl_items(&plan.waypoints);
        let task_ids = task_ids_by_seq_from_plan(&plan);

        assert_eq!(waypoints[0].seq, 7);
        assert_eq!(task_ids.get(&7).map(String::as_str), Some("wp-7"));
        assert_eq!(default_takeoff_altitude(&plan.waypoints), 3.0);
    }

    #[test]
    fn takeoff_altitude_matches_single_agent_floor_contract() {
        let waypoints = vec![
            sitl_waypoint(0, "wp-0", 0.0, 0.0, 0.0),
            sitl_waypoint(1, "wp-1", 0.0, 0.0, 5.0),
        ];

        assert_eq!(default_takeoff_altitude(&waypoints), 2.5);
    }
}
