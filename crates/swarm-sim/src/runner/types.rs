use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use swarm_alloc::Allocator;
use swarm_runtime::GridState;
use swarm_safety::SafetyConfig;
use swarm_types::{
    AgentId, EdgeId, InspectionGraph, Task, TaskId, UrbanBlockedPolicy, UrbanMap, UrbanNodeId,
    UrbanPerimeterPatrol, UrbanRouteLoop, UrbanSearchState, UrbanTemporaryObstacle,
};

/// Tracks coverage of inspection edges during a run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InspectionState {
    pub graph: InspectionGraph,
    pub covered: HashSet<EdgeId>,
    pub visit_counts: HashMap<EdgeId, u32>,
}

impl InspectionState {
    pub fn new(graph: InspectionGraph) -> Self {
        Self {
            graph,
            covered: HashSet::new(),
            visit_counts: HashMap::new(),
        }
    }
}

/// Wrapper that filters out tasks in no-fly zones before delegating to the inner allocator.
pub(super) struct SafetyAllocator<A> {
    pub(super) inner: A,
    pub(super) safety_config: Option<swarm_safety::SafetyConfig>,
}

impl<A: Allocator> Allocator for SafetyAllocator<A> {
    fn allocate(
        &mut self,
        tasks: &[swarm_alloc::AllocationTask<'_>],
        agents: &[swarm_alloc::AllocationAgent],
    ) -> Vec<(TaskId, AgentId)> {
        let filtered_tasks: Vec<swarm_alloc::AllocationTask<'_>> = match &self.safety_config {
            Some(config) => tasks
                .iter()
                .filter(|at| {
                    let task_pose = match at.task.pose {
                        Some(p) => p,
                        None => return true,
                    };
                    !config
                        .no_fly_zones
                        .iter()
                        .any(|nf| nf.bounds.contains(&task_pose))
                })
                .cloned()
                .collect(),
            None => tasks.to_vec(),
        };
        self.inner.allocate(&filtered_tasks, agents)
    }

    fn allocate_with_connectivity(
        &mut self,
        tasks: &[swarm_alloc::AllocationTask<'_>],
        agents: &[swarm_alloc::AllocationAgent],
        connectivity: &swarm_alloc::ConnectivityContext,
    ) -> Vec<(TaskId, AgentId)> {
        let filtered_tasks: Vec<swarm_alloc::AllocationTask<'_>> = match &self.safety_config {
            Some(config) => tasks
                .iter()
                .filter(|at| {
                    let task_pose = match at.task.pose {
                        Some(p) => p,
                        None => return true,
                    };
                    !config
                        .no_fly_zones
                        .iter()
                        .any(|nf| nf.bounds.contains(&task_pose))
                })
                .cloned()
                .collect(),
            None => tasks.to_vec(),
        };
        self.inner
            .allocate_with_connectivity(&filtered_tasks, agents, connectivity)
    }

    fn allocation_metrics(&self) -> (u64, bool, u64) {
        self.inner.allocation_metrics()
    }

    fn is_distributed(&self) -> bool {
        self.inner.is_distributed()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FailureEvent {
    pub agent_id: AgentId,
    pub at_tick: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DynamicTaskEvent {
    pub at_tick: u64,
    pub task: Task,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartitionEvent {
    pub at_tick: u64,
    pub until_tick: Option<u64>,
    #[serde(default)]
    pub heal_at_tick: Option<u64>,
    pub agents: (AgentId, AgentId),
}

/// Runtime state for wildfire mapping missions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WildfireState {
    pub zones: Vec<WildfireZone>,
    pub mapped_zone_ids: std::collections::HashSet<String>,
    pub update_interval_ticks: u64,
    pub enable_dynamic_threat: bool,
    // v0.38 Wildfire v2
    #[serde(default)]
    pub enable_zone_expansion: bool,
    #[serde(default)]
    pub enable_spatial_spread: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WildfireZone {
    pub id: String,
    pub threat_level: f64,
    pub priority: u8,
}

/// Runtime configuration for Urban road-graph foundation scenarios.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UrbanState {
    pub map: UrbanMap,
    pub route_loop: UrbanRouteLoop,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_node: Option<UrbanNodeId>,
    #[serde(default = "default_urban_planner")]
    pub planner: String,
    #[serde(default)]
    pub temporary_obstacles: Vec<UrbanTemporaryObstacle>,
    #[serde(default)]
    pub blocked_route_policy: UrbanBlockedPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perimeter_patrol: Option<UrbanPerimeterPatrol>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunConfig {
    pub max_ticks: u64,
    #[serde(default)]
    pub timeout_ticks: u64,
    #[serde(default = "default_max_unassigned")]
    pub max_unassigned_ticks: u64,
    #[serde(default)]
    pub packet_loss_rate: f64,
    #[serde(default)]
    pub latency_ticks: u64,
    #[serde(default)]
    pub latency_per_hop: u64,
    #[serde(default)]
    pub comms_jitter_ticks: u64,
    #[serde(default)]
    pub failures: Vec<FailureEvent>,
    #[serde(default)]
    pub dynamic_tasks: Vec<DynamicTaskEvent>,
    #[serde(default)]
    pub partition_events: Vec<PartitionEvent>,
    #[serde(default = "default_gossip_interval")]
    pub gossip_interval_ticks: u64,
    #[serde(default)]
    pub base_id: Option<AgentId>,
    #[serde(default)]
    pub enable_movement: bool,
    #[serde(default = "default_tick_duration")]
    pub tick_duration_ms: u64,
    #[serde(default)]
    pub grid_state: Option<GridState>,
    #[serde(default)]
    pub enable_cbba: bool,
    #[serde(default)]
    pub safety_config: Option<SafetyConfig>,
    #[serde(default)]
    pub inspection_state: Option<InspectionState>,
    #[serde(default)]
    pub wildfire_state: Option<WildfireState>,
    #[serde(default)]
    pub urban_state: Option<UrbanState>,
    #[serde(default)]
    pub urban_search_state: Option<UrbanSearchState>,
    /// Wind drift per tick as (vx, vy, vz) in m/tick. Applied after movement.
    #[serde(default)]
    pub wind: Option<(f64, f64, f64)>,
    /// Gaussian pose noise radius in metres. Applied after movement.
    #[serde(default)]
    pub pose_noise_m: f64,
    // v0.35 Dynamic Mission Correctness
    /// Strategy name for support matrix detection.
    #[serde(default)]
    pub strategy_name: Option<String>,
    /// Success threshold for wildfire (fraction of zones that must be mapped).
    #[serde(default = "default_wildfire_threshold")]
    pub wildfire_success_threshold: f64,
    /// Success threshold for inspection (fraction of edges that must be covered).
    #[serde(default = "default_inspection_threshold")]
    pub inspection_coverage_threshold: f64,
    // v0.37 Realism Scenario Pack
    /// Realism profile name (light, medium, heavy).
    #[serde(default)]
    pub realism_profile: Option<String>,
}

fn default_max_unassigned() -> u64 {
    10
}

fn default_gossip_interval() -> u64 {
    999
}

fn default_tick_duration() -> u64 {
    100
}

fn default_wildfire_threshold() -> f64 {
    0.8
}

fn default_inspection_threshold() -> f64 {
    0.8
}

fn default_urban_planner() -> String {
    "dijkstra".to_owned()
}

/// Compute mission-specific success and detect unsupported configurations.
#[allow(clippy::too_many_arguments)]
pub(super) fn compute_mission_success(
    max_unassigned_ticks_config: u64,
    strategy_name: &Option<String>,
    wildfire_success_threshold: f64,
    inspection_coverage_threshold: f64,
    all_tasks_assigned: bool,
    all_expected_failures_detected: bool,
    max_task_unassigned_ticks: u64,
    grid_state: &Option<swarm_runtime::GridState>,
    inspection_state: &Option<InspectionState>,
    wildfire_state: &Option<WildfireState>,
    urban_state: &Option<UrbanState>,
    urban_route_planned: bool,
    urban_violation_count: u64,
    urban_route_completed: bool,
    _adapter_complete: bool,
) -> (bool, Option<String>) {
    let base_success = all_tasks_assigned
        && all_expected_failures_detected
        && max_task_unassigned_ticks <= max_unassigned_ticks_config;

    // SAR mission
    if let Some(ref gs) = grid_state {
        let sar_success = gs.all_targets_found()
            && all_expected_failures_detected
            && max_task_unassigned_ticks <= max_unassigned_ticks_config;

        // Detect unsupported strategies for SAR.
        // cbba: delayed reconvergence manifests as unassigned tasks after agent loss.
        // centralized: static pre-plan assigns all tasks upfront but cannot adapt to
        // dynamic belief updates, so the check does not require !all_tasks_assigned.
        if let Some(ref strategy) = strategy_name {
            if !sar_success {
                match strategy.as_str() {
                    "cbba" if !all_tasks_assigned => {
                        return (false, Some("delayed_reconvergence".to_owned()));
                    }
                    "centralized" => {
                        return (false, Some("static_pre_plan".to_owned()));
                    }
                    _ => {}
                }
            }
        }
        return (sar_success, None);
    }

    // Inspection mission
    if let Some(ref is) = inspection_state {
        let total = is.graph.edges.len() as f64;
        let covered = is.covered.len() as f64;
        let coverage_ratio = if total > 0.0 { covered / total } else { 1.0 };
        let inspection_success = coverage_ratio >= inspection_coverage_threshold
            && all_expected_failures_detected
            && max_task_unassigned_ticks <= max_unassigned_ticks_config;
        return (inspection_success, None);
    }

    // Wildfire mission
    if let Some(ref ws) = wildfire_state {
        let total = ws.zones.len() as f64;
        let mapped = ws.mapped_zone_ids.len() as f64;
        let mapped_ratio = if total > 0.0 { mapped / total } else { 1.0 };
        let wildfire_success = mapped_ratio >= wildfire_success_threshold
            && all_expected_failures_detected
            && max_task_unassigned_ticks <= max_unassigned_ticks_config;
        return (wildfire_success, None);
    }

    if urban_state.is_some() {
        let urban_success =
            urban_route_planned && urban_violation_count == 0 && urban_route_completed;
        return (urban_success, None);
    }

    // Default (coverage and other missions)
    (base_success, None)
}

pub struct ScenarioRunner;
