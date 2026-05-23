use rand::rngs::StdRng;
use rand::Rng;
use rand::SeedableRng;

use swarm_sim::{RunConfig, Scenario};
use swarm_types::{
    Agent, AgentId, Capability, Health, HiddenTarget, Pose, Role, SearchGrid, SensorModel, Task,
    TaskId, TaskStatus,
};

pub struct SarScenarioConfig {
    pub grid: SearchGrid,
    pub target_count: u32,
    pub scout_count: u32,
    pub thermal_count: u32,
    pub relay_count: u32,
    pub sensor: SensorModel,
    pub enable_movement: bool,
    pub tick_duration_ms: u64,
    pub max_ticks: u64,
    pub seed: u64,
    pub prior: f64, // v0.14 SAR v2: prior probability for BeliefMap
}

#[derive(Clone, Debug, PartialEq)]
pub enum SarProfile {
    Ideal,
    Standard,
    Challenging,
    BatteryConstrained,
    Uncertain, // v0.14 SAR v2
    Noisy,     // v0.14 SAR v2
}

impl SarProfile {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ideal" => Some(Self::Ideal),
            "standard" => Some(Self::Standard),
            "challenging" => Some(Self::Challenging),
            "battery-constrained" | "batteryconstrained" => Some(Self::BatteryConstrained),
            "uncertain" => Some(Self::Uncertain),
            "noisy" => Some(Self::Noisy),
            _ => None,
        }
    }

    pub fn config(&self, seed: u64) -> SarScenarioConfig {
        match self {
            Self::Ideal => SarScenarioConfig {
                grid: SearchGrid::new(6, 6, 10.0),
                target_count: 2,
                scout_count: 3,
                thermal_count: 1,
                relay_count: 1,
                sensor: SensorModel::new_v2(0.6, 0.95, 0.2, 0.6, 0.05),
                enable_movement: true,
                tick_duration_ms: 1000,
                max_ticks: 300,
                seed,
                prior: 0.05,
            },
            Self::Standard => SarScenarioConfig {
                grid: SearchGrid::new(8, 8, 10.0),
                target_count: 3,
                scout_count: 3,
                thermal_count: 2,
                relay_count: 2,
                sensor: SensorModel::new_v2(0.4, 0.85, 0.15, 0.5, 0.1),
                enable_movement: true,
                tick_duration_ms: 1000,
                max_ticks: 400,
                seed,
                prior: 0.05,
            },
            Self::Challenging => SarScenarioConfig {
                grid: SearchGrid::new(10, 10, 10.0),
                target_count: 5,
                scout_count: 4,
                thermal_count: 2,
                relay_count: 2,
                sensor: SensorModel::new_v2(0.3, 0.75, 0.1, 0.4, 0.15),
                enable_movement: true,
                tick_duration_ms: 1000,
                max_ticks: 500,
                seed,
                prior: 0.05,
            },
            Self::BatteryConstrained => SarScenarioConfig {
                grid: SearchGrid::new(6, 6, 10.0),
                target_count: 3,
                scout_count: 3,
                thermal_count: 1,
                relay_count: 1,
                sensor: SensorModel::new_v2(0.5, 0.9, 0.15, 0.5, 0.1),
                enable_movement: true,
                tick_duration_ms: 1000,
                max_ticks: 300,
                seed,
                prior: 0.05,
            },
            Self::Uncertain => SarScenarioConfig {
                grid: SearchGrid::new(6, 6, 10.0),
                target_count: 2,
                scout_count: 3,
                thermal_count: 1,
                relay_count: 1,
                sensor: SensorModel::new_v2(0.4, 0.7, 0.15, 0.5, 0.2),
                enable_movement: true,
                tick_duration_ms: 1000,
                max_ticks: 400,
                seed,
                prior: 0.05,
            },
            Self::Noisy => SarScenarioConfig {
                grid: SearchGrid::new(6, 6, 10.0),
                target_count: 2,
                scout_count: 3,
                thermal_count: 1,
                relay_count: 1,
                sensor: SensorModel::new_v2(0.3, 0.6, 0.1, 0.4, 0.4),
                enable_movement: true,
                tick_duration_ms: 1000,
                max_ticks: 500,
                seed,
                prior: 0.05,
            },
        }
    }
}

pub struct SarStandardProfiles;

impl SarStandardProfiles {
    pub fn profile_names() -> Vec<&'static str> {
        vec![
            "ideal",
            "standard",
            "challenging",
            "battery-constrained",
            "uncertain",
            "noisy",
        ]
    }
}

/// Calculate SAR task priority based on belief entropy.
/// For v0.14: static priority using initial prior entropy.
pub fn sar_task_priority(prior: f64) -> u8 {
    let entropy = if prior <= 0.0 || prior >= 1.0 {
        0.0
    } else {
        -prior * prior.log2() - (1.0 - prior) * (1.0 - prior).log2()
    };
    let raw = entropy * prior * 20.0;
    raw.clamp(1.0, 10.0) as u8
}

pub fn build_sar_scenario(config: &SarScenarioConfig) -> (Scenario, RunConfig) {
    let mut rng = StdRng::seed_from_u64(config.seed);

    // Place targets randomly in cells
    let mut targets = Vec::new();
    let total_cells = config.grid.total_cells();
    for i in 0..config.target_count {
        let cell_idx = rng.gen_range(0..total_cells);
        let cell_x = cell_idx % config.grid.width;
        let cell_y = cell_idx / config.grid.width;
        targets.push(HiddenTarget {
            id: format!("target-{i}"),
            cell_x,
            cell_y,
        });
    }

    // Create one task per cell (scanning the cell)
    let mut tasks = Vec::new();
    for y in 0..config.grid.height {
        for x in 0..config.grid.width {
            let cell_idx = y * config.grid.width + x;
            tasks.push(Task {
                id: TaskId::from(format!("cell-{cell_idx}")),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: sar_task_priority(config.prior),
                required_capabilities: vec![],
                required_role: None,
                preferred_role: None,
                expires_at: None,
                pose: Some(config.grid.cell_center(x, y)),
                grid_cell: Some((x, y)),
                edge_id: None,
            });
        }
    }

    // Create agents with different roles
    let mut agents = Vec::new();
    let total_agents = config.scout_count + config.thermal_count + config.relay_count;

    for i in 0..total_agents {
        let role = if i < config.scout_count {
            Role::Scout
        } else if i < config.scout_count + config.thermal_count {
            Role::Thermal
        } else {
            Role::Relay
        };

        let (speed, comms_range, capabilities) = match role {
            Role::Scout => (5.0, 15.0, vec![]),
            Role::Thermal => (4.0, 12.0, vec![Capability::from("thermal".to_owned())]),
            Role::Relay => (3.0, 20.0, vec![]),
            _ => (5.0, 15.0, vec![]),
        };

        let x = rng.gen_range(0.0..(config.grid.width as f64 * config.grid.cell_size));
        let y = rng.gen_range(0.0..(config.grid.height as f64 * config.grid.cell_size));

        agents.push(Agent {
            id: AgentId::from(format!("agent-{i}")),
            role,
            health: Health::Alive,
            pose: Pose { x, y },
            capabilities,
            current_task: None,
            battery: 100.0,
            comms_range,
            generation: 1,
            speed,
            max_range: 500.0,
            battery_drain_rate: 0.0,
        });
    }

    let grid_state =
        swarm_runtime::GridState::new(config.grid.clone(), targets.clone(), config.sensor.clone())
            .with_belief(config.prior);

    let scenario = Scenario {
        name: "sar_v2".to_owned(),
        seed: config.seed,
        agents,
        tasks,
        ground_nodes: vec![],
        base_station: None,
    };

    let run_config = RunConfig {
        max_ticks: config.max_ticks,
        timeout_ticks: 3,
        max_unassigned_ticks: 10,
        packet_loss_rate: 0.0,
        latency_ticks: 0,
        latency_per_hop: 0,
        failures: vec![],
        dynamic_tasks: vec![],
        partition_events: vec![],
        gossip_interval_ticks: 3,
        base_id: None,
        enable_movement: config.enable_movement,
        tick_duration_ms: config.tick_duration_ms,
        grid_state: Some(grid_state),
        enable_cbba: false,
        ..Default::default()
    };

    (scenario, run_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sar_scenario_creates_correct_agent_count() {
        let config = SarScenarioConfig {
            grid: SearchGrid::new(5, 5, 10.0),
            target_count: 2,
            scout_count: 3,
            thermal_count: 1,
            relay_count: 1,
            sensor: SensorModel::new(0.3, 0.8, 0.1),
            enable_movement: true,
            tick_duration_ms: 1000,
            max_ticks: 500,
            seed: 42,
            prior: 0.05,
        };
        let (scenario, _) = build_sar_scenario(&config);
        assert_eq!(scenario.agents.len(), 5);
    }

    #[test]
    fn sar_scenario_targets_within_grid() {
        let config = SarScenarioConfig {
            grid: SearchGrid::new(5, 5, 10.0),
            target_count: 3,
            scout_count: 2,
            thermal_count: 1,
            relay_count: 1,
            sensor: SensorModel::new(0.3, 0.8, 0.1),
            enable_movement: true,
            tick_duration_ms: 1000,
            max_ticks: 500,
            seed: 42,
            prior: 0.05,
        };
        let (_, run_config) = build_sar_scenario(&config);
        let grid_state = run_config.grid_state.unwrap();
        for target in &grid_state.targets {
            assert!(target.cell_x < config.grid.width);
            assert!(target.cell_y < config.grid.height);
        }
    }

    #[test]
    fn sar_ideal_profile_params() {
        let config = SarProfile::Ideal.config(42);
        assert_eq!(config.grid.width, 6);
        assert_eq!(config.grid.height, 6);
        assert_eq!(config.target_count, 2);
        assert_eq!(config.scout_count, 3);
        assert_eq!(config.thermal_count, 1);
        assert_eq!(config.relay_count, 1);
        assert_eq!(config.sensor.scout_pod, 0.6);
        assert_eq!(config.sensor.thermal_pod, 0.95);
        assert_eq!(config.max_ticks, 300);
    }

    #[test]
    fn sar_battery_constrained_profile_params() {
        let config = SarProfile::BatteryConstrained.config(42);
        assert_eq!(config.grid.width, 6);
        assert_eq!(config.grid.height, 6);
        assert_eq!(config.max_ticks, 300);
    }

    #[test]
    fn sar_profile_from_str_roundtrip() {
        assert_eq!(SarProfile::from_str("ideal"), Some(SarProfile::Ideal));
        assert_eq!(SarProfile::from_str("standard"), Some(SarProfile::Standard));
        assert_eq!(
            SarProfile::from_str("challenging"),
            Some(SarProfile::Challenging)
        );
        assert_eq!(
            SarProfile::from_str("battery-constrained"),
            Some(SarProfile::BatteryConstrained)
        );
        assert_eq!(
            SarProfile::from_str("batteryconstrained"),
            Some(SarProfile::BatteryConstrained)
        );
        assert_eq!(SarProfile::from_str("unknown"), None);
    }

    #[test]
    fn sar_task_priority_range() {
        let p = sar_task_priority(0.05);
        assert!((1..=10).contains(&p));
    }

    #[test]
    fn sar_task_priority_high_entropy_wins() {
        let p_half = sar_task_priority(0.5);
        let p_extreme = sar_task_priority(0.99);
        assert!(
            p_half > p_extreme,
            "entropy at 0.5 should give higher priority"
        );
    }

    #[test]
    fn sar_standard_profiles_names() {
        let names = SarStandardProfiles::profile_names();
        assert_eq!(names.len(), 6);
        assert!(names.contains(&"ideal"));
        assert!(names.contains(&"standard"));
        assert!(names.contains(&"challenging"));
        assert!(names.contains(&"battery-constrained"));
        assert!(names.contains(&"uncertain"));
        assert!(names.contains(&"noisy"));
    }

    #[test]
    fn sar_scenario_one_task_per_cell() {
        let config = SarScenarioConfig {
            grid: SearchGrid::new(4, 3, 10.0),
            target_count: 1,
            scout_count: 2,
            thermal_count: 0,
            relay_count: 0,
            sensor: SensorModel::new(0.3, 0.8, 0.1),
            enable_movement: true,
            tick_duration_ms: 1000,
            max_ticks: 500,
            seed: 42,
            prior: 0.05,
        };
        let (scenario, _) = build_sar_scenario(&config);
        assert_eq!(scenario.tasks.len(), 12); // 4 * 3
    }
}
