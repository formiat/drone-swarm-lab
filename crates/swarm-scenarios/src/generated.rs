use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use rand::{rngs::StdRng, Rng, SeedableRng};
use swarm_sim::{
    validate_scenario_suite, FailureEvent, GeoOrigin, PartitionEvent, RunConfig, Scenario,
    ScenarioGeneratorManifest, ScenarioGeneratorParameter, ScenarioSuite, ScenarioSuiteEntry,
    UrbanState, ValidationError, SCENARIO_GENERATOR_MANIFEST_SCHEMA_VERSION,
};
use swarm_types::{
    Aabb, Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskKind, TaskStatus,
    UrbanBlockedPolicy, UrbanBus, UrbanBusId, UrbanBusRoute, UrbanBusStop, UrbanDetectorConfig,
    UrbanEdge, UrbanEdgeId, UrbanMap, UrbanNode, UrbanNodeId, UrbanObstacleId,
    UrbanPerimeterPatrol, UrbanRouteLoop, UrbanSearchState, UrbanStaticObstacle,
    UrbanTemporaryObstacle,
};

pub const SYNTHETIC_URBAN_GENERATOR_NAME: &str = "synthetic-urban";
pub const SYNTHETIC_URBAN_GENERATOR_VERSION: &str = "0.1.0";

pub trait ScenarioGenerator {
    type Config;

    fn generate(
        &self,
        config: &Self::Config,
    ) -> Result<GeneratedScenarioSuite, ScenarioGenerationError>;
}

#[derive(Clone, Debug)]
pub struct GeneratedScenarioSuite {
    pub suite: ScenarioSuite,
    pub manifest: ScenarioGeneratorManifest,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScenarioGenerationError {
    InvalidConfig { field: String, message: String },
    ValidationFailed { errors: Vec<ValidationError> },
}

impl fmt::Display for ScenarioGenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field, message } => write!(f, "[{field}] {message}"),
            Self::ValidationFailed { errors } => {
                write!(f, "generated scenario suite failed validation")?;
                for error in errors {
                    write!(f, "; [{}] {}", error.field, error.message)?;
                }
                Ok(())
            }
        }
    }
}

impl Error for ScenarioGenerationError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntheticScenarioCategory {
    Tiny,
    Small,
    Medium,
    Stress,
    RegressionStable,
    Experimental,
}

impl SyntheticScenarioCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Stress => "stress",
            Self::RegressionStable => "regression-stable",
            Self::Experimental => "experimental",
        }
    }

    fn parse_value(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "tiny" => Some(Self::Tiny),
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "stress" => Some(Self::Stress),
            "regression-stable" | "regression_stable" | "regression" => {
                Some(Self::RegressionStable)
            }
            "experimental" => Some(Self::Experimental),
            _ => None,
        }
    }
}

impl FromStr for SyntheticScenarioCategory {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse_value(value).ok_or_else(|| format!("unsupported synthetic category: {value}"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntheticBusMode {
    None,
    Static,
    Route,
}

impl SyntheticBusMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Static => "static",
            Self::Route => "route",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntheticFailureType {
    None,
    AgentLost,
}

impl SyntheticFailureType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::AgentLost => "agent-lost",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntheticReplacementPolicy {
    None,
    Accept,
    Reject,
}

impl SyntheticReplacementPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Accept => "accept",
            Self::Reject => "reject",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyntheticFailureConfig {
    pub failure_type: SyntheticFailureType,
    pub agent_failure_tick: Option<u64>,
    pub partial_completion_target: Option<u64>,
    pub replacement_policy: SyntheticReplacementPolicy,
}

impl Default for SyntheticFailureConfig {
    fn default() -> Self {
        Self {
            failure_type: SyntheticFailureType::None,
            agent_failure_tick: None,
            partial_completion_target: None,
            replacement_policy: SyntheticReplacementPolicy::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyntheticPartitionConfig {
    pub at_tick: u64,
    pub until_tick: u64,
    pub heal_at_tick: u64,
    pub agent_a: usize,
    pub agent_b: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyntheticCommsConfig {
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub latency_per_hop: u64,
    pub comms_jitter_ticks: u64,
    pub partitions: Vec<SyntheticPartitionConfig>,
}

impl Default for SyntheticCommsConfig {
    fn default() -> Self {
        Self {
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            latency_per_hop: 0,
            comms_jitter_ticks: 0,
            partitions: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyntheticUrbanConfig {
    pub seed: u64,
    pub category: SyntheticScenarioCategory,
    pub rows: usize,
    pub cols: usize,
    pub agent_count: usize,
    pub static_obstacle_density: f64,
    pub blocked_edge_count: usize,
    pub bus_mode: SyntheticBusMode,
    pub perimeter: bool,
    pub max_ticks: u64,
    pub failure: SyntheticFailureConfig,
    pub comms: SyntheticCommsConfig,
}

impl Default for SyntheticUrbanConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            category: SyntheticScenarioCategory::Tiny,
            rows: 3,
            cols: 3,
            agent_count: 1,
            static_obstacle_density: 0.10,
            blocked_edge_count: 1,
            bus_mode: SyntheticBusMode::None,
            perimeter: true,
            max_ticks: 180,
            failure: SyntheticFailureConfig::default(),
            comms: SyntheticCommsConfig::default(),
        }
    }
}

pub struct SyntheticUrbanGenerator;

impl ScenarioGenerator for SyntheticUrbanGenerator {
    type Config = SyntheticUrbanConfig;

    fn generate(
        &self,
        config: &Self::Config,
    ) -> Result<GeneratedScenarioSuite, ScenarioGenerationError> {
        validate_config(config)?;

        let mut rng = StdRng::seed_from_u64(config.seed);
        let graph = build_grid_graph(config, &mut rng);
        let route_loop = UrbanRouteLoop {
            nodes: perimeter_route(config.rows, config.cols),
        };
        let temporary_obstacles = build_temporary_obstacles(config, &graph, &route_loop);
        let urban_state = UrbanState {
            map: graph.map,
            route_loop: route_loop.clone(),
            start_node: route_loop.nodes.first().cloned(),
            planner: "dijkstra".to_owned(),
            temporary_obstacles,
            blocked_route_policy: UrbanBlockedPolicy::Replan,
            perimeter_patrol: config
                .perimeter
                .then(|| perimeter_patrol(config.rows, config.cols)),
        };
        let urban_search_state = build_search_state(config, &urban_state.map, &route_loop);

        let agents = build_agents(config.agent_count, &urban_state);
        let tasks = build_tasks(&route_loop, &urban_state.map);
        let mission = if urban_search_state.is_some() {
            "urban-search"
        } else {
            "urban-patrol"
        };
        let profile = format!(
            "generated-{}-{}x{}-{}",
            config.category.as_str(),
            config.rows,
            config.cols,
            config.seed
        );
        let scenario = Scenario {
            name: format!("urban_generated_{}", profile.replace('-', "_")),
            seed: config.seed,
            agents,
            tasks,
            ground_nodes: vec![],
            base_station: Some(Pose::default()),
            geo_origin: Some(GeoOrigin {
                lat_deg: 47.397_742,
                lon_deg: 8.545_594,
                alt_m: 0.0,
            }),
        };
        let run_config = RunConfig {
            max_ticks: config.max_ticks,
            timeout_ticks: 3,
            max_unassigned_ticks: config.max_ticks,
            packet_loss_rate: config.comms.packet_loss_rate,
            latency_ticks: config.comms.latency_ticks,
            latency_per_hop: config.comms.latency_per_hop,
            comms_jitter_ticks: config.comms.comms_jitter_ticks,
            failures: build_failure_events(config),
            partition_events: build_partition_events(config),
            enable_movement: true,
            tick_duration_ms: 1000,
            urban_state: Some(urban_state),
            urban_search_state,
            ..Default::default()
        };
        let entry = ScenarioSuiteEntry {
            mission: mission.to_owned(),
            profile,
            scenario,
            run_config,
        };
        let manifest = build_manifest(config);
        let suite = ScenarioSuite {
            schema_version: "0.1".to_owned(),
            name: format!("Synthetic Urban {}", config.category.as_str()),
            description: "Deterministic generated Urban scenario suite".to_owned(),
            generator_manifest: Some(manifest.clone()),
            scenarios: vec![entry],
        };

        let validation_errors = validate_scenario_suite(&suite);
        if !validation_errors.is_empty() {
            return Err(ScenarioGenerationError::ValidationFailed {
                errors: validation_errors,
            });
        }

        Ok(GeneratedScenarioSuite { suite, manifest })
    }
}

pub struct SyntheticScenarioLibrary;

impl SyntheticScenarioLibrary {
    pub fn urban_tiny_regression(seed: u64) -> SyntheticUrbanConfig {
        SyntheticUrbanConfig {
            seed,
            category: SyntheticScenarioCategory::RegressionStable,
            rows: 3,
            cols: 3,
            agent_count: 1,
            static_obstacle_density: 0.10,
            blocked_edge_count: 1,
            bus_mode: SyntheticBusMode::None,
            perimeter: true,
            max_ticks: 180,
            failure: SyntheticFailureConfig::default(),
            comms: SyntheticCommsConfig::default(),
        }
    }

    pub fn urban_small_exploratory(seed: u64) -> SyntheticUrbanConfig {
        SyntheticUrbanConfig {
            seed,
            category: SyntheticScenarioCategory::Small,
            rows: 5,
            cols: 5,
            agent_count: 2,
            static_obstacle_density: 0.18,
            blocked_edge_count: 3,
            bus_mode: SyntheticBusMode::Route,
            perimeter: true,
            max_ticks: 300,
            failure: SyntheticFailureConfig {
                failure_type: SyntheticFailureType::AgentLost,
                agent_failure_tick: Some(45),
                partial_completion_target: Some(2),
                replacement_policy: SyntheticReplacementPolicy::Accept,
            },
            comms: SyntheticCommsConfig {
                packet_loss_rate: 0.02,
                latency_ticks: 1,
                latency_per_hop: 1,
                comms_jitter_ticks: 1,
                partitions: vec![SyntheticPartitionConfig {
                    at_tick: 30,
                    until_tick: 42,
                    heal_at_tick: 42,
                    agent_a: 0,
                    agent_b: 1,
                }],
            },
        }
    }

    pub fn urban_stress_manual(seed: u64) -> SyntheticUrbanConfig {
        SyntheticUrbanConfig {
            seed,
            category: SyntheticScenarioCategory::Stress,
            rows: 9,
            cols: 9,
            agent_count: 4,
            static_obstacle_density: 0.22,
            blocked_edge_count: 8,
            bus_mode: SyntheticBusMode::Route,
            perimeter: true,
            max_ticks: 700,
            failure: SyntheticFailureConfig {
                failure_type: SyntheticFailureType::AgentLost,
                agent_failure_tick: Some(120),
                partial_completion_target: Some(4),
                replacement_policy: SyntheticReplacementPolicy::Accept,
            },
            comms: SyntheticCommsConfig {
                packet_loss_rate: 0.05,
                latency_ticks: 2,
                latency_per_hop: 1,
                comms_jitter_ticks: 2,
                partitions: vec![],
            },
        }
    }

    pub fn urban_for_category(
        category: SyntheticScenarioCategory,
        seed: u64,
    ) -> SyntheticUrbanConfig {
        match category {
            SyntheticScenarioCategory::Tiny | SyntheticScenarioCategory::RegressionStable => {
                Self::urban_tiny_regression(seed)
            }
            SyntheticScenarioCategory::Small | SyntheticScenarioCategory::Experimental => {
                Self::urban_small_exploratory(seed)
            }
            SyntheticScenarioCategory::Medium => {
                let mut config = Self::urban_small_exploratory(seed);
                config.category = SyntheticScenarioCategory::Medium;
                config.rows = 7;
                config.cols = 7;
                config.agent_count = 3;
                config.blocked_edge_count = 5;
                config.max_ticks = 500;
                config
            }
            SyntheticScenarioCategory::Stress => Self::urban_stress_manual(seed),
        }
    }
}

struct GeneratedGraph {
    map: UrbanMap,
    edge_by_pair: HashMap<(String, String), UrbanEdgeId>,
}

fn validate_config(config: &SyntheticUrbanConfig) -> Result<(), ScenarioGenerationError> {
    if config.rows < 2 {
        return invalid("rows", "must be >= 2");
    }
    if config.cols < 2 {
        return invalid("cols", "must be >= 2");
    }
    if config.agent_count == 0 {
        return invalid("agent_count", "must be >= 1");
    }
    if !config.static_obstacle_density.is_finite()
        || !(0.0..=1.0).contains(&config.static_obstacle_density)
    {
        return invalid("static_obstacle_density", "must be finite and in 0.0..=1.0");
    }
    let max_blocked_edges = available_perimeter_route_edge_count(config.rows, config.cols);
    if config.blocked_edge_count > max_blocked_edges {
        return invalid(
            "blocked_edge_count",
            format!("must be <= available perimeter route edges ({max_blocked_edges})"),
        );
    }
    if config.max_ticks == 0 {
        return invalid("max_ticks", "must be > 0");
    }
    if !config.comms.packet_loss_rate.is_finite()
        || !(0.0..=1.0).contains(&config.comms.packet_loss_rate)
    {
        return invalid("comms.packet_loss_rate", "must be finite and in 0.0..=1.0");
    }
    for (index, partition) in config.comms.partitions.iter().enumerate() {
        if partition.at_tick >= partition.until_tick
            || partition.until_tick > partition.heal_at_tick
        {
            return invalid(
                format!("comms.partitions[{index}]"),
                "must satisfy at_tick < until_tick <= heal_at_tick",
            );
        }
        if partition.agent_a >= config.agent_count || partition.agent_b >= config.agent_count {
            return invalid(
                format!("comms.partitions[{index}]"),
                "agent indexes must reference configured agents",
            );
        }
    }
    if matches!(config.failure.failure_type, SyntheticFailureType::AgentLost)
        && config.failure.agent_failure_tick.is_none()
    {
        return invalid("failure.agent_failure_tick", "must be set for agent-lost");
    }

    Ok(())
}

fn invalid<T>(
    field: impl Into<String>,
    message: impl Into<String>,
) -> Result<T, ScenarioGenerationError> {
    Err(ScenarioGenerationError::InvalidConfig {
        field: field.into(),
        message: message.into(),
    })
}

fn build_grid_graph(config: &SyntheticUrbanConfig, rng: &mut StdRng) -> GeneratedGraph {
    let spacing = 10.0;
    let mut nodes = Vec::with_capacity(config.rows * config.cols);
    for row in 0..config.rows {
        for col in 0..config.cols {
            nodes.push(UrbanNode {
                id: node_id(row, col),
                pose: Pose {
                    x: col as f64 * spacing,
                    y: row as f64 * spacing,
                    z: 0.0,
                },
            });
        }
    }

    let mut edges = Vec::new();
    let mut edge_by_pair = HashMap::new();
    for row in 0..config.rows {
        for col in 0..config.cols {
            if col + 1 < config.cols {
                push_bidirectional_edge(row, col, row, col + 1, rng, &mut edges, &mut edge_by_pair);
            }
            if row + 1 < config.rows {
                push_bidirectional_edge(row, col, row + 1, col, rng, &mut edges, &mut edge_by_pair);
            }
        }
    }

    let static_obstacles = build_static_obstacles(config, spacing);

    GeneratedGraph {
        map: UrbanMap {
            nodes,
            edges,
            static_obstacles,
        },
        edge_by_pair,
    }
}

fn push_bidirectional_edge(
    from_row: usize,
    from_col: usize,
    to_row: usize,
    to_col: usize,
    rng: &mut StdRng,
    edges: &mut Vec<UrbanEdge>,
    edge_by_pair: &mut HashMap<(String, String), UrbanEdgeId>,
) {
    push_edge(from_row, from_col, to_row, to_col, rng, edges, edge_by_pair);
    push_edge(to_row, to_col, from_row, from_col, rng, edges, edge_by_pair);
}

fn push_edge(
    from_row: usize,
    from_col: usize,
    to_row: usize,
    to_col: usize,
    rng: &mut StdRng,
    edges: &mut Vec<UrbanEdge>,
    edge_by_pair: &mut HashMap<(String, String), UrbanEdgeId>,
) {
    let from = node_id(from_row, from_col);
    let to = node_id(to_row, to_col);
    let id = UrbanEdgeId::from(format!("road-{}-{}", *from, *to));
    edge_by_pair.insert(((*from).clone(), (*to).clone()), id.clone());
    edges.push(UrbanEdge {
        id,
        from,
        to,
        cost: 10.0,
        length_m: 10.0,
        corridor_width_m: Some(4.0 + rng.gen_range(0..=4) as f64),
        blocked: false,
    });
}

fn build_static_obstacles(config: &SyntheticUrbanConfig, spacing: f64) -> Vec<UrbanStaticObstacle> {
    let interior_rows = config.rows.saturating_sub(2);
    let interior_cols = config.cols.saturating_sub(2);
    let max_obstacles = interior_rows * interior_cols;
    let requested = ((max_obstacles as f64) * config.static_obstacle_density)
        .round()
        .clamp(0.0, max_obstacles as f64) as usize;

    let mut obstacles = Vec::with_capacity(requested);
    'rows: for row in 1..config.rows.saturating_sub(1) {
        for col in 1..config.cols.saturating_sub(1) {
            if obstacles.len() == requested {
                break 'rows;
            }
            let min_x = col as f64 * spacing + 2.0;
            let min_y = row as f64 * spacing + 2.0;
            obstacles.push(UrbanStaticObstacle {
                id: UrbanObstacleId::from(format!("building-r{row}-c{col}")),
                bounds: Aabb {
                    min_x,
                    min_y,
                    max_x: min_x + 3.0,
                    max_y: min_y + 3.0,
                },
                label: Some("building".to_owned()),
            });
        }
    }
    obstacles
}

fn build_temporary_obstacles(
    config: &SyntheticUrbanConfig,
    graph: &GeneratedGraph,
    route_loop: &UrbanRouteLoop,
) -> Vec<UrbanTemporaryObstacle> {
    route_loop
        .nodes
        .windows(2)
        .filter_map(|pair| {
            graph
                .edge_by_pair
                .get(&((*pair[0]).clone(), (*pair[1]).clone()))
                .cloned()
        })
        .take(config.blocked_edge_count)
        .enumerate()
        .map(|(index, edge_id)| UrbanTemporaryObstacle {
            edge_id,
            appears_at_tick: 1 + index as u64,
            disappears_at_tick: Some(6 + index as u64),
            reason: Some("synthetic-road-work".to_owned()),
            severity: Some("soft".to_owned()),
        })
        .collect()
}

fn build_search_state(
    config: &SyntheticUrbanConfig,
    map: &UrbanMap,
    route_loop: &UrbanRouteLoop,
) -> Option<UrbanSearchState> {
    match config.bus_mode {
        SyntheticBusMode::None => None,
        SyntheticBusMode::Static => {
            let node = route_loop.nodes.get(route_loop.nodes.len() / 2)?;
            let pose = map.node(node)?.pose;
            Some(UrbanSearchState {
                buses: vec![UrbanBus {
                    id: UrbanBusId::from("bus-0".to_owned()),
                    pose,
                    active_from_tick: Some(0),
                    active_until_tick: None,
                    route: None,
                }],
                detector: detector(config.seed),
            })
        }
        SyntheticBusMode::Route => {
            let stops = route_loop
                .nodes
                .iter()
                .take(route_loop.nodes.len().min(5))
                .enumerate()
                .map(|(index, node_id)| UrbanBusStop {
                    node_id: node_id.clone(),
                    arrival_tick: index as u64 * 8,
                })
                .collect();
            let pose = route_loop
                .nodes
                .first()
                .and_then(|node| map.node(node))
                .map(|node| node.pose)?;
            Some(UrbanSearchState {
                buses: vec![UrbanBus {
                    id: UrbanBusId::from("bus-0".to_owned()),
                    pose,
                    active_from_tick: None,
                    active_until_tick: None,
                    route: Some(UrbanBusRoute {
                        stops,
                        speed_m_per_tick: 1.25,
                    }),
                }],
                detector: detector(config.seed),
            })
        }
    }
}

fn detector(seed: u64) -> UrbanDetectorConfig {
    UrbanDetectorConfig {
        detection_range_m: 0.5,
        detection_probability: 1.0,
        false_positive_rate: 0.0,
        seed,
    }
}

fn build_agents(agent_count: usize, urban_state: &UrbanState) -> Vec<Agent> {
    let start_pose = urban_state
        .start_node
        .as_ref()
        .and_then(|node| urban_state.map.node(node))
        .map(|node| node.pose)
        .unwrap_or_default();
    (0..agent_count)
        .map(|index| Agent {
            id: AgentId::from(format!("agent-{index}")),
            role: Role::Scout,
            health: Health::Alive,
            pose: start_pose,
            capabilities: vec![],
            current_task: None,
            battery: 100.0,
            comms_range: 1000.0,
            generation: 1,
            speed: 2.0,
            max_range: 1000.0,
            battery_drain_rate: 0.0,
            battery_model: None,
        })
        .collect()
}

fn build_tasks(route_loop: &UrbanRouteLoop, map: &UrbanMap) -> Vec<Task> {
    route_loop
        .nodes
        .iter()
        .skip(1)
        .enumerate()
        .filter_map(|(index, node_id)| {
            let pose = map.node(node_id)?.pose;
            Some(Task {
                id: TaskId::from(format!("urban-waypoint-{index}-{}", **node_id)),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: 1,
                required_capabilities: vec![],
                required_role: None,
                preferred_role: Some(Role::Scout),
                expires_at: None,
                pose: Some(pose),
                grid_cell: None,
                edge_id: None,
                kind: Some(TaskKind::Waypoint),
            })
        })
        .collect()
}

fn build_failure_events(config: &SyntheticUrbanConfig) -> Vec<FailureEvent> {
    match (
        config.failure.failure_type,
        config.failure.agent_failure_tick,
    ) {
        (SyntheticFailureType::AgentLost, Some(at_tick)) => vec![FailureEvent {
            agent_id: AgentId::from("agent-0".to_owned()),
            at_tick,
        }],
        _ => vec![],
    }
}

fn build_partition_events(config: &SyntheticUrbanConfig) -> Vec<PartitionEvent> {
    config
        .comms
        .partitions
        .iter()
        .map(|partition| PartitionEvent {
            at_tick: partition.at_tick,
            until_tick: Some(partition.until_tick),
            heal_at_tick: Some(partition.heal_at_tick),
            agents: (
                AgentId::from(format!("agent-{}", partition.agent_a)),
                AgentId::from(format!("agent-{}", partition.agent_b)),
            ),
        })
        .collect()
}

fn build_manifest(config: &SyntheticUrbanConfig) -> ScenarioGeneratorManifest {
    let mut parameters = vec![
        parameter("rows", config.rows),
        parameter("cols", config.cols),
        parameter("agent_count", config.agent_count),
        parameter("static_obstacle_density", config.static_obstacle_density),
        parameter("blocked_edge_count", config.blocked_edge_count),
        parameter("bus_mode", config.bus_mode.as_str()),
        parameter("perimeter", config.perimeter),
        parameter("max_ticks", config.max_ticks),
        parameter("packet_loss_rate", config.comms.packet_loss_rate),
        parameter("latency_ticks", config.comms.latency_ticks),
        parameter("latency_per_hop", config.comms.latency_per_hop),
        parameter("comms_jitter_ticks", config.comms.comms_jitter_ticks),
        parameter("partition_count", config.comms.partitions.len()),
        parameter("failure_type", config.failure.failure_type.as_str()),
        parameter(
            "agent_failure_tick",
            config
                .failure
                .agent_failure_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "none".to_owned()),
        ),
        parameter(
            "partial_completion_target",
            config
                .failure
                .partial_completion_target
                .map(|target| target.to_string())
                .unwrap_or_else(|| "none".to_owned()),
        ),
        parameter(
            "replacement_policy",
            config.failure.replacement_policy.as_str(),
        ),
    ];
    parameters.sort_by(|a, b| a.key.cmp(&b.key));

    ScenarioGeneratorManifest {
        schema_version: SCENARIO_GENERATOR_MANIFEST_SCHEMA_VERSION.to_owned(),
        generator_name: SYNTHETIC_URBAN_GENERATOR_NAME.to_owned(),
        generator_version: SYNTHETIC_URBAN_GENERATOR_VERSION.to_owned(),
        seed: config.seed,
        category: config.category.as_str().to_owned(),
        parameters,
    }
}

fn parameter(key: impl Into<String>, value: impl ToString) -> ScenarioGeneratorParameter {
    ScenarioGeneratorParameter {
        key: key.into(),
        value: value.to_string(),
    }
}

fn perimeter_route(rows: usize, cols: usize) -> Vec<UrbanNodeId> {
    let mut nodes = Vec::new();
    for col in 0..cols {
        nodes.push(node_id(0, col));
    }
    for row in 1..rows {
        nodes.push(node_id(row, cols - 1));
    }
    if rows > 1 {
        for col in (0..cols - 1).rev() {
            nodes.push(node_id(rows - 1, col));
        }
    }
    if cols > 1 && rows > 2 {
        for row in (1..rows - 1).rev() {
            nodes.push(node_id(row, 0));
        }
    }
    if let Some(first) = nodes.first().cloned() {
        nodes.push(first);
    }
    nodes
}

fn available_perimeter_route_edge_count(rows: usize, cols: usize) -> usize {
    if rows < 2 || cols < 2 {
        return 0;
    }
    2 * rows + 2 * cols - 4
}

fn perimeter_patrol(rows: usize, cols: usize) -> UrbanPerimeterPatrol {
    let max_x = (cols.saturating_sub(1)) as f64 * 10.0;
    let max_y = (rows.saturating_sub(1)) as f64 * 10.0;
    UrbanPerimeterPatrol {
        polygon: vec![
            Pose {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Pose {
                x: max_x,
                y: 0.0,
                z: 0.0,
            },
            Pose {
                x: max_x,
                y: max_y,
                z: 0.0,
            },
            Pose {
                x: 0.0,
                y: max_y,
                z: 0.0,
            },
        ],
        spacing_m: 10.0,
    }
}

fn node_id(row: usize, col: usize) -> UrbanNodeId {
    UrbanNodeId::from(format!("n{row}_{col}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_sim::export_suite;

    #[test]
    fn generator_is_deterministic_for_same_seed() {
        let generator = SyntheticUrbanGenerator;
        let config = SyntheticScenarioLibrary::urban_tiny_regression(42);

        let left = generator.generate(&config).unwrap();
        let right = generator.generate(&config).unwrap();

        assert_eq!(
            export_suite(&left.suite).unwrap(),
            export_suite(&right.suite).unwrap()
        );
    }

    #[test]
    fn generator_records_reproducibility_manifest() {
        let generator = SyntheticUrbanGenerator;
        let generated = generator
            .generate(&SyntheticScenarioLibrary::urban_small_exploratory(7))
            .unwrap();

        assert_eq!(
            generated.manifest.schema_version,
            SCENARIO_GENERATOR_MANIFEST_SCHEMA_VERSION
        );
        assert_eq!(generated.manifest.generator_name, "synthetic-urban");
        assert_eq!(generated.manifest.seed, 7);
        assert_eq!(generated.manifest.category, "small");
        assert!(generated
            .manifest
            .parameters
            .iter()
            .any(|parameter| parameter.key == "bus_mode" && parameter.value == "route"));
        assert_eq!(
            generated.suite.generator_manifest.as_ref(),
            Some(&generated.manifest)
        );
    }

    #[test]
    fn generator_validates_map_route_and_obstacle_schedule() {
        let generator = SyntheticUrbanGenerator;
        let generated = generator
            .generate(&SyntheticScenarioLibrary::urban_tiny_regression(5))
            .unwrap();
        let urban_state = generated.suite.scenarios[0]
            .run_config
            .urban_state
            .as_ref()
            .unwrap();

        assert!(urban_state.map.validate().is_empty());
        assert!(urban_state
            .map
            .validate_route_loop(&urban_state.route_loop)
            .is_empty());
        assert!(urban_state
            .map
            .validate_temporary_obstacles(&urban_state.temporary_obstacles)
            .is_empty());
        assert_eq!(urban_state.temporary_obstacles.len(), 1);
    }

    #[test]
    fn different_seeds_change_generated_corridor_widths() {
        let generator = SyntheticUrbanGenerator;
        let left = generator
            .generate(&SyntheticScenarioLibrary::urban_tiny_regression(1))
            .unwrap();
        let right = generator
            .generate(&SyntheticScenarioLibrary::urban_tiny_regression(2))
            .unwrap();
        let left_edges = &left.suite.scenarios[0]
            .run_config
            .urban_state
            .as_ref()
            .unwrap()
            .map
            .edges;
        let right_edges = &right.suite.scenarios[0]
            .run_config
            .urban_state
            .as_ref()
            .unwrap()
            .map
            .edges;

        assert_ne!(
            left_edges
                .iter()
                .map(|edge| edge.corridor_width_m)
                .collect::<Vec<_>>(),
            right_edges
                .iter()
                .map(|edge| edge.corridor_width_m)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn invalid_config_is_rejected_before_generation() {
        let generator = SyntheticUrbanGenerator;
        let mut config = SyntheticScenarioLibrary::urban_tiny_regression(1);
        config.rows = 1;

        let error = generator.generate(&config).unwrap_err();

        assert!(matches!(
            error,
            ScenarioGenerationError::InvalidConfig { field, .. } if field == "rows"
        ));
    }

    #[test]
    fn blocked_edge_count_larger_than_route_edges_is_rejected() {
        let generator = SyntheticUrbanGenerator;
        let mut config = SyntheticScenarioLibrary::urban_tiny_regression(1);
        config.rows = 2;
        config.cols = 2;
        config.blocked_edge_count = 5;

        let error = generator.generate(&config).unwrap_err();

        assert!(matches!(
            error,
            ScenarioGenerationError::InvalidConfig { field, .. } if field == "blocked_edge_count"
        ));
    }
}
