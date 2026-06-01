#![allow(unused_imports)]
use super::*;
use super::*;
use crate::sitl_multi_agent::{build_multi_agent_manifest, MultiAgentSitlConfig};

pub(super) fn fixture_suite() -> swarm_sim::ScenarioSuite {
    serde_json::from_str(
        r#"{
  "schema_version": "0.1",
  "name": "Supervisor Unit Fixture",
  "description": "in-memory supervisor unit fixture",
  "scenarios": [
    {
      "mission": "sitl",
      "profile": "unit",
      "scenario": {
        "name": "supervisor_unit",
        "seed": 0,
        "agents": [
          {
            "id": "agent-0",
            "role": "scout",
            "health": "alive",
            "pose": { "x": 0.0, "y": 0.0 },
            "capabilities": [],
            "current_task": null,
            "battery": 100.0,
            "comms_range": 1000.0,
            "generation": 1,
            "speed": 0.0,
            "max_range": 1000.0,
            "battery_drain_rate": 0.0
          },
          {
            "id": "agent-1",
            "role": "scout",
            "health": "alive",
            "pose": { "x": 1.0, "y": 1.0 },
            "capabilities": [],
            "current_task": null,
            "battery": 100.0,
            "comms_range": 1000.0,
            "generation": 1,
            "speed": 0.0,
            "max_range": 1000.0,
            "battery_drain_rate": 0.0
          }
        ],
        "tasks": [
          {
            "id": "wp-0",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 10.0, "y": 20.0, "z": 3.5 },
            "grid_cell": null
          },
          {
            "id": "wp-1",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 30.0, "y": 40.0, "z": 4.5 },
            "grid_cell": null
          }
        ],
        "ground_nodes": [],
        "base_station": null
      },
      "run_config": { "max_ticks": 10, "timeout_ticks": 1 }
    }
  ]
}"#,
    )
    .unwrap()
}

pub(super) fn fixture_config() -> MultiAgentSitlConfig {
    serde_json::from_str(
        r#"{
  "schema_version": "multi_sitl.v1",
  "agents": [
    {
      "agent_id": "agent-0",
      "system_id": 1,
      "component_id": 1,
      "connection_string": "udp:127.0.0.1:14550",
      "start_delay_ms": 0,
      "lifecycle": "upload_only",
      "task_ids": ["wp-0"]
    },
    {
      "agent_id": "agent-1",
      "system_id": 2,
      "component_id": 1,
      "connection_string": "udp:127.0.0.1:14560",
      "start_delay_ms": 0,
      "lifecycle": "execute",
      "task_ids": ["wp-1"]
    }
  ]
}"#,
    )
    .unwrap()
}

pub(super) fn fixture_execute_config() -> MultiAgentSitlConfig {
    serde_json::from_str(
        r#"{
  "schema_version": "multi_sitl.v1",
  "agents": [
    {
      "agent_id": "agent-0",
      "system_id": 1,
      "component_id": 1,
      "connection_string": "udp:127.0.0.1:14550",
      "start_delay_ms": 0,
      "lifecycle": "execute",
      "task_ids": ["wp-0"]
    },
    {
      "agent_id": "agent-1",
      "system_id": 2,
      "component_id": 1,
      "connection_string": "udp:127.0.0.1:14560",
      "start_delay_ms": 0,
      "lifecycle": "execute",
      "task_ids": ["wp-1"]
    }
  ]
}"#,
    )
    .unwrap()
}

pub(super) fn fixture_manifest() -> MultiAgentSitlManifest {
    let suite = fixture_suite();
    let config = fixture_config();
    build_multi_agent_manifest(
        &suite,
        "inline-scenario.json",
        "inline-config.json",
        &config,
    )
    .unwrap()
}

pub(super) fn fixture_execute_manifest() -> MultiAgentSitlManifest {
    let suite = fixture_suite();
    let config = fixture_execute_config();
    build_multi_agent_manifest(
        &suite,
        "inline-scenario.json",
        "inline-config.json",
        &config,
    )
    .unwrap()
}

pub(super) fn fixture_nonlexical_suite() -> swarm_sim::ScenarioSuite {
    serde_json::from_str(
        r#"{
  "schema_version": "0.1",
  "name": "Supervisor Nonlexical Unit Fixture",
  "description": "in-memory supervisor unit fixture with nonlexical task ids",
  "scenarios": [
    {
      "mission": "sitl",
      "profile": "unit",
      "scenario": {
        "name": "supervisor_nonlexical_unit",
        "seed": 0,
        "agents": [
          {
            "id": "agent-0",
            "role": "scout",
            "health": "alive",
            "pose": { "x": 0.0, "y": 0.0 },
            "capabilities": [],
            "current_task": null,
            "battery": 100.0,
            "comms_range": 1000.0,
            "generation": 1,
            "speed": 0.0,
            "max_range": 1000.0,
            "battery_drain_rate": 0.0
          },
          {
            "id": "agent-1",
            "role": "scout",
            "health": "alive",
            "pose": { "x": 1.0, "y": 1.0 },
            "capabilities": [],
            "current_task": null,
            "battery": 100.0,
            "comms_range": 1000.0,
            "generation": 1,
            "speed": 0.0,
            "max_range": 1000.0,
            "battery_drain_rate": 0.0
          }
        ],
        "tasks": [
          {
            "id": "wp-2",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 10.0, "y": 20.0, "z": 3.5 },
            "grid_cell": null
          },
          {
            "id": "wp-10",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 20.0, "y": 30.0, "z": 4.0 },
            "grid_cell": null
          },
          {
            "id": "wp-1",
            "status": "unassigned",
            "assigned_to": null,
            "priority": 1,
            "required_capabilities": [],
            "required_role": null,
            "preferred_role": null,
            "expires_at": null,
            "pose": { "x": 30.0, "y": 40.0, "z": 4.5 },
            "grid_cell": null
          }
        ],
        "ground_nodes": [],
        "base_station": null
      },
      "run_config": { "max_ticks": 10, "timeout_ticks": 1 }
    }
  ]
}"#,
    )
    .unwrap()
}

pub(super) fn fixture_nonlexical_execute_config() -> MultiAgentSitlConfig {
    serde_json::from_str(
        r#"{
  "schema_version": "multi_sitl.v1",
  "agents": [
    {
      "agent_id": "agent-0",
      "system_id": 1,
      "component_id": 1,
      "connection_string": "udp:127.0.0.1:14550",
      "start_delay_ms": 0,
      "lifecycle": "execute",
      "task_ids": ["wp-2", "wp-10"]
    },
    {
      "agent_id": "agent-1",
      "system_id": 2,
      "component_id": 1,
      "connection_string": "udp:127.0.0.1:14560",
      "start_delay_ms": 0,
      "lifecycle": "execute",
      "task_ids": ["wp-1"]
    }
  ]
}"#,
    )
    .unwrap()
}

pub(super) fn fixture_nonlexical_execute_manifest() -> MultiAgentSitlManifest {
    let suite = fixture_nonlexical_suite();
    let config = fixture_nonlexical_execute_config();
    build_multi_agent_manifest(
        &suite,
        "nonlexical-scenario.json",
        "inline-config.json",
        &config,
    )
    .unwrap()
}

pub(super) fn fixture_live_config() -> SupervisorLiveConfig {
    SupervisorLiveConfig {
        scenario_path: "inline-scenario.json".to_owned(),
        config_path: "inline-config.json".to_owned(),
        safety_config_path: None,
        replay_log: None,
        run_report: None,
        lifecycle: SitlConnectionLifecycle::default(),
        allow_hardware_candidate: false,
        reupload_on_failure: false,
        run_id: Some("unit-live-run".to_owned()),
    }
}

pub(super) struct FakeAgentController {
    agent_id: String,
    lifecycle: MultiAgentLifecycle,
    fail_upload: bool,
    fail_start: bool,
    heartbeat_until_tick: Option<u64>,
    uploaded: bool,
    started: bool,
    waypoint_count: usize,
    poll_ticks: Vec<u64>,
}

impl FakeAgentController {
    pub(super) fn alive(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            lifecycle: MultiAgentLifecycle::Execute,
            fail_upload: false,
            fail_start: false,
            heartbeat_until_tick: None,
            uploaded: false,
            started: false,
            waypoint_count: 0,
            poll_ticks: Vec::new(),
        }
    }

    pub(super) fn stops_at(agent_id: impl Into<String>, tick: u64) -> Self {
        Self {
            heartbeat_until_tick: Some(tick),
            ..Self::alive(agent_id)
        }
    }

    pub(super) fn with_upload_failure(mut self) -> Self {
        self.fail_upload = true;
        self
    }

    pub(super) fn with_start_failure(mut self) -> Self {
        self.fail_start = true;
        self
    }
}

impl AgentController for FakeAgentController {
    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn lifecycle(&self) -> MultiAgentLifecycle {
        self.lifecycle
    }

    fn upload(&mut self, waypoints: &[SitlWaypointItem]) -> Result<AgentStep, SitlError> {
        if self.fail_upload {
            return Err(SitlError::ConnectionFailed {
                message: format!("fake upload failure for {}", self.agent_id),
            });
        }
        self.uploaded = true;
        self.waypoint_count = waypoints.len();
        Ok(AgentStep {
            agent_id: self.agent_id.clone(),
            waypoint_count: self.waypoint_count,
        })
    }

    fn start(&mut self) -> Result<AgentStep, SitlError> {
        if self.fail_start {
            return Err(SitlError::ConnectionFailed {
                message: format!("fake start failure for {}", self.agent_id),
            });
        }
        self.started = true;
        Ok(AgentStep {
            agent_id: self.agent_id.clone(),
            waypoint_count: self.waypoint_count,
        })
    }

    fn poll(&mut self, tick: u64) -> Result<AgentProgress, SitlError> {
        self.poll_ticks.push(tick);
        let heartbeat_seen = self
            .heartbeat_until_tick
            .is_none_or(|heartbeat_until_tick| tick < heartbeat_until_tick);
        Ok(AgentProgress {
            agent_id: self.agent_id.clone(),
            heartbeat_seen,
        })
    }

    fn abort(&mut self, _reason: &str) -> Result<AgentStep, SitlError> {
        Ok(AgentStep {
            agent_id: self.agent_id.clone(),
            waypoint_count: self.waypoint_count,
        })
    }
}

pub(super) struct FakeLiveAgentController {
    run: LiveAgentRun,
    start_delay_ms: u64,
    mission_waypoints: Vec<SitlWaypointItem>,
    started: bool,
    poll_count: usize,
    finish_after_polls: usize,
}

impl FakeLiveAgentController {
    pub(super) fn completed(agent: &MultiAgentSitlManifestAgent) -> Self {
        Self {
            run: LiveAgentRun {
                agent_id: agent.agent_id.clone(),
                connection_string: agent.connection_string.clone(),
                system_id: agent.system_id,
                component_id: agent.component_id,
                lifecycle: agent.lifecycle,
                mission_item_count: agent.waypoint_count,
                completed_task_count: agent.waypoint_count,
                completed_waypoints: completed_waypoints_from_items(&agent.waypoints),
                completed_task_ids: agent.task_ids.clone(),
                final_status: "completed".to_owned(),
                error: None,
            },
            start_delay_ms: agent.start_delay_ms,
            mission_waypoints: agent.waypoints.clone(),
            started: false,
            poll_count: 0,
            finish_after_polls: 0,
        }
    }

    pub(super) fn failed(agent: &MultiAgentSitlManifestAgent, completed_task_count: usize) -> Self {
        Self {
            run: LiveAgentRun {
                agent_id: agent.agent_id.clone(),
                connection_string: agent.connection_string.clone(),
                system_id: agent.system_id,
                component_id: agent.component_id,
                lifecycle: agent.lifecycle,
                mission_item_count: agent.waypoint_count,
                completed_task_count,
                completed_waypoints: completed_waypoints_from_items(
                    &agent.waypoints[..completed_task_count.min(agent.waypoints.len())],
                ),
                completed_task_ids: agent
                    .task_ids
                    .iter()
                    .take(completed_task_count)
                    .cloned()
                    .collect(),
                final_status: "failed".to_owned(),
                error: Some("fake live failure".to_owned()),
            },
            start_delay_ms: agent.start_delay_ms,
            mission_waypoints: agent.waypoints.clone(),
            started: false,
            poll_count: 0,
            finish_after_polls: 0,
        }
    }

    pub(super) fn completed_after_polls(agent: &MultiAgentSitlManifestAgent, polls: usize) -> Self {
        Self {
            finish_after_polls: polls,
            ..Self::completed(agent)
        }
    }

    pub(super) fn failed_after_polls(
        agent: &MultiAgentSitlManifestAgent,
        completed_task_count: usize,
        polls: usize,
    ) -> Self {
        Self {
            finish_after_polls: polls,
            ..Self::failed(agent, completed_task_count)
        }
    }
}

impl LiveAgentController for FakeLiveAgentController {
    fn agent_id(&self) -> &str {
        &self.run.agent_id
    }

    fn start_delay_ms(&self) -> u64 {
        self.start_delay_ms
    }

    fn mission_waypoints(&self) -> &[SitlWaypointItem] {
        &self.mission_waypoints
    }

    fn replace_mission(&mut self, plan: &MissionReplacementPlan) -> Result<(), SitlError> {
        if plan.target_agent_id != self.run.agent_id {
            return Err(SitlError::MultiAgentConfigInvalid {
                message: format!(
                    "fake live replacement target '{}' does not match '{}'",
                    plan.target_agent_id, self.run.agent_id
                ),
            });
        }
        self.mission_waypoints = plan.waypoints.clone();
        self.run.mission_item_count = plan.waypoints.len();
        if self.run.final_status == "completed" {
            self.run.completed_task_count = plan.waypoints.len();
            self.run.completed_waypoints = completed_waypoints_from_items(&plan.waypoints);
            self.run.completed_task_ids = plan.task_ids.clone();
        }
        Ok(())
    }

    fn run(&mut self) -> Result<LiveAgentRun, SitlError> {
        Ok(self.run.clone())
    }

    fn start(&mut self) -> Result<(), SitlError> {
        self.started = true;
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<LiveAgentRun>, SitlError> {
        if !self.started {
            return Ok(Some(LiveAgentRun {
                final_status: "failed".to_owned(),
                error: Some("fake live controller polled before start".to_owned()),
                ..self.run.clone()
            }));
        }
        self.poll_count += 1;
        if self.poll_count > self.finish_after_polls {
            Ok(Some(self.run.clone()))
        } else {
            Ok(None)
        }
    }

    fn completed_task_count(&self) -> usize {
        self.completed_waypoints().len()
    }

    fn completed_waypoints(&self) -> Vec<CompletedWaypoint> {
        if self.poll_count > self.finish_after_polls {
            self.run.completed_waypoints.clone()
        } else {
            Vec::new()
        }
    }

    fn completed_task_ids(&self) -> Vec<String> {
        task_ids_from_completed_waypoints(&self.completed_waypoints())
    }
}

pub(super) fn fake_controllers() -> Vec<FakeAgentController> {
    vec![
        FakeAgentController::alive("agent-0"),
        FakeAgentController::alive("agent-1"),
    ]
}

pub(super) fn run_fake_supervisor(
    controllers: Vec<FakeAgentController>,
    own_id: &str,
) -> Result<SupervisorMetrics, SitlError> {
    run_fake_supervisor_with_ticks(controllers, own_id, 1, 6)
}

pub(super) fn run_fake_supervisor_with_ticks(
    controllers: Vec<FakeAgentController>,
    own_id: &str,
    timeout_ticks: u64,
    max_ticks: u64,
) -> Result<SupervisorMetrics, SitlError> {
    let suite = fixture_suite();
    let manifest = fixture_manifest();
    let entry = first_sitl_entry(&suite, "inline-scenario.json").unwrap();
    let loop_config = SupervisorLoopConfig {
        replay_log: None,
        run_id: None,
        timeout_ticks,
        max_ticks,
        own_id: own_id.to_owned(),
        mode_label: "Fake",
    };
    run_supervisor_with_controllers(entry, &manifest, controllers, &loop_config)
}
