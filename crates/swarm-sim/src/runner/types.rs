use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use swarm_alloc::Allocator;
use swarm_comms::DroneLinkConfig;
use swarm_runtime::{AgentAutonomyConfig, GridState};
use swarm_safety::SafetyConfig;
use swarm_types::{
    AgentId, EdgeId, InspectionGraph, Task, TaskId, UrbanBlockedPolicy, UrbanMap, UrbanNodeId,
    UrbanPerimeterPatrol, UrbanRightOfWayPolicy, UrbanRouteLoop, UrbanSearchState,
    UrbanTemporaryObstacle,
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrbanMissionTemplate {
    PerimeterPatrol,
    BlockLoop,
    SearchUntilTarget,
    InspectionCorridorCandidate,
}

/// Runtime configuration for mission-level Urban segment ownership.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UrbanDeconflictionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub right_of_way_policy: UrbanRightOfWayPolicy,
    #[serde(default)]
    pub locked_segment_policy: UrbanBlockedPolicy,
    #[serde(default)]
    pub agent_priorities: HashMap<AgentId, u8>,
}

/// Runtime configuration for Urban road-graph foundation scenarios.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UrbanState {
    pub map: UrbanMap,
    pub route_loop: UrbanRouteLoop,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission_template: Option<UrbanMissionTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_node: Option<UrbanNodeId>,
    #[serde(default = "default_urban_planner")]
    pub planner: String,
    #[serde(default)]
    pub temporary_obstacles: Vec<UrbanTemporaryObstacle>,
    #[serde(default)]
    pub blocked_route_policy: UrbanBlockedPolicy,
    #[serde(default)]
    pub deconfliction: UrbanDeconflictionConfig,
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
    /// Optional SAR success threshold by found-target ratio; None keeps strict all-targets success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sar_success_threshold: Option<f64>,
    // v0.37 Realism Scenario Pack
    /// Realism profile name (light, medium, heavy).
    #[serde(default)]
    pub realism_profile: Option<String>,
    /// Penalty weight for assigning tasks outside an agent's communication range.
    #[serde(default)]
    pub comms_penalty_weight: f64,
    /// Wildfire priority threshold that forces release/reallocation when crossed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wildfire_priority_realloc_threshold: Option<u8>,
    /// Re-rank unfinished SAR tasks by posterior uncertainty after scan events.
    #[serde(default)]
    pub dynamic_belief_updates: bool,
    /// Single-vehicle parametric mission executed directly via MAVLink without
    /// task allocation or simulation-side routing. Absent for all other mission
    /// types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitive_mission: Option<PrimitiveMission>,
    /// Transport backend used by standalone agent processes.
    /// Defaults to `Simulated` (in-memory shared bus) so existing scenarios
    /// that omit this field continue to work without modification.
    #[serde(default)]
    pub drone_link: DroneLinkConfig,
    /// Per-agent failsafe and autonomous behaviour configuration.
    /// Defaults to conservative settings; existing scenarios without this field
    /// deserialize correctly.
    #[serde(default)]
    pub autonomy: AgentAutonomyConfig,
}

/// A minimal real-hardware mission expressed as a single parametric command
/// sequence. No task allocation or simulation is performed; the plan is
/// converted directly into MAVLink `MISSION_ITEM_INT` messages.
///
/// Positions are in local simulation coordinates (x = east m, y = north m,
/// z = altitude above takeoff point m). The upload layer converts to WGS84
/// using the scenario `geo_origin`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PrimitiveMission {
    /// Climb to `altitude_m`, loiter in place for `hold_seconds`, land.
    Hover { altitude_m: f64, hold_seconds: f32 },
    /// Climb to `altitude_m`, fly `turns` full circles of `radius_m`, land.
    Orbit {
        altitude_m: f64,
        turns: f32,
        radius_m: f32,
    },
    /// Climb to `altitude_m` and immediately descend and land.
    TakeoffLand { altitude_m: f64 },
    /// Climb to `altitude_m`, fly a closed square route of `side_m`, land.
    WaypointSquare { altitude_m: f64, side_m: f64 },
}

impl PrimitiveMission {
    /// Human-readable items for dry-run display.
    /// value: `(label, x, y, z, params)` where params is a short description.
    pub fn describe_items(&self) -> Vec<PrimitiveMissionItemDesc> {
        match self {
            Self::Hover {
                altitude_m,
                hold_seconds,
            } => vec![
                PrimitiveMissionItemDesc {
                    label: "loiter_time".to_owned(),
                    x: 0.0,
                    y: 0.0,
                    z: *altitude_m,
                    params: format!("hold_seconds={hold_seconds:.1}"),
                },
                PrimitiveMissionItemDesc {
                    label: "land".to_owned(),
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    params: String::new(),
                },
            ],
            Self::Orbit {
                altitude_m,
                turns,
                radius_m,
            } => vec![
                PrimitiveMissionItemDesc {
                    label: "loiter_turns".to_owned(),
                    x: 0.0,
                    y: 0.0,
                    z: *altitude_m,
                    params: format!("turns={turns:.1} radius_m={radius_m:.1}"),
                },
                PrimitiveMissionItemDesc {
                    label: "land".to_owned(),
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    params: String::new(),
                },
            ],
            Self::TakeoffLand { altitude_m } => vec![
                PrimitiveMissionItemDesc {
                    label: "waypoint".to_owned(),
                    x: 0.0,
                    y: 0.0,
                    z: *altitude_m,
                    params: "brief hover before land".to_owned(),
                },
                PrimitiveMissionItemDesc {
                    label: "land".to_owned(),
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    params: String::new(),
                },
            ],
            Self::WaypointSquare { altitude_m, side_m } => {
                let side = *side_m;
                let altitude = *altitude_m;
                vec![
                    PrimitiveMissionItemDesc {
                        label: "square_start".to_owned(),
                        x: 0.0,
                        y: 0.0,
                        z: altitude,
                        params: format!("side_m={side:.1}"),
                    },
                    PrimitiveMissionItemDesc {
                        label: "square_east".to_owned(),
                        x: side,
                        y: 0.0,
                        z: altitude,
                        params: format!("side_m={side:.1}"),
                    },
                    PrimitiveMissionItemDesc {
                        label: "square_north".to_owned(),
                        x: side,
                        y: side,
                        z: altitude,
                        params: format!("side_m={side:.1}"),
                    },
                    PrimitiveMissionItemDesc {
                        label: "square_west".to_owned(),
                        x: 0.0,
                        y: side,
                        z: altitude,
                        params: format!("side_m={side:.1}"),
                    },
                    PrimitiveMissionItemDesc {
                        label: "square_return".to_owned(),
                        x: 0.0,
                        y: 0.0,
                        z: altitude,
                        params: format!("side_m={side:.1}"),
                    },
                    PrimitiveMissionItemDesc {
                        label: "land".to_owned(),
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                        params: String::new(),
                    },
                ]
            }
        }
    }
}

/// One item in the dry-run summary of a `PrimitiveMission`.
#[derive(Clone, Debug, PartialEq)]
pub struct PrimitiveMissionItemDesc {
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub params: String,
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
    sar_success_threshold: Option<f64>,
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
        let target_total = gs.targets.len() as f64;
        let found_ratio = if target_total > 0.0 {
            gs.targets_found as f64 / target_total
        } else {
            1.0
        };
        let sar_goal_satisfied = sar_success_threshold
            .map(|threshold| found_ratio >= threshold)
            .unwrap_or_else(|| gs.all_targets_found());
        let sar_success = sar_goal_satisfied
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
