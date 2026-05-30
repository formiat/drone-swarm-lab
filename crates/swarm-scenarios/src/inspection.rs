use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use swarm_sim::{InspectionState, RunConfig, Scenario};
use swarm_types::{
    Agent, AgentId, Health, InspectionGraph, Role, Task, TaskId, TaskKind, TaskStatus,
};

pub struct InspectionConfig {
    pub graph: InspectionGraph,
    pub agent_count: u32,
    pub battery_constraint: f64,
    pub require_role: Option<Role>,
    pub seed: u64,
    pub max_ticks: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InspectionProfile {
    Linear,
    Perimeter,
    Random,
}

impl InspectionProfile {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "linear" => Some(Self::Linear),
            "perimeter" => Some(Self::Perimeter),
            "random" => Some(Self::Random),
            _ => None,
        }
    }

    pub fn config(&self, seed: u64) -> InspectionConfig {
        match self {
            Self::Linear => InspectionConfig {
                graph: InspectionGraph::linear_route(10, 10.0),
                agent_count: 3,
                battery_constraint: 0.0,
                require_role: None,
                seed,
                max_ticks: 500,
            },
            Self::Perimeter => InspectionConfig {
                graph: InspectionGraph::grid_perimeter(10, 10, 10.0),
                agent_count: 4,
                battery_constraint: 0.3,
                require_role: None,
                seed,
                max_ticks: 500,
            },
            Self::Random => InspectionConfig {
                graph: InspectionGraph::random_graph(15, seed),
                agent_count: 5,
                battery_constraint: 0.0,
                require_role: None,
                seed,
                max_ticks: 500,
            },
        }
    }
}

pub struct InspectionStandardProfiles;

impl InspectionStandardProfiles {
    pub fn profile_names() -> Vec<&'static str> {
        vec!["linear", "perimeter", "random"]
    }
}

pub fn build_inspection_scenario(config: &InspectionConfig) -> (Scenario, RunConfig) {
    let mut rng = StdRng::seed_from_u64(config.seed);

    let agents: Vec<Agent> = (0..config.agent_count)
        .map(|i| {
            let role = config.require_role.clone().unwrap_or(Role::Scout);
            let _x = rng.gen::<f64>() * 20.0;
            let _y = rng.gen::<f64>() * 20.0;
            let (battery, battery_drain_rate, max_range) = if config.battery_constraint > 0.0 {
                let total_length: f64 = config.graph.edges.iter().map(|e| e.length_m).sum();
                let battery = config.battery_constraint * 100.0;
                let required_range = total_length * 2.0;
                let drain_rate = battery / required_range;
                (battery, drain_rate, required_range)
            } else {
                (100.0, 0.0, f64::INFINITY)
            };
            Agent {
                id: AgentId::from(format!("agent-{i}")),
                role,
                health: Health::Alive,
                pose: config.graph.depot,
                capabilities: vec![],
                current_task: None,
                battery,
                comms_range: 50.0,
                generation: 1,
                speed: 2.0,
                max_range,
                battery_drain_rate,
                battery_model: None,
            }
        })
        .collect();

    let tasks: Vec<Task> = config
        .graph
        .edges
        .iter()
        .map(|edge| Task {
            id: TaskId::from(format!("edge-{}", edge.id)),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: edge.priority,
            required_capabilities: vec![],
            required_role: config.require_role.clone(),
            preferred_role: config.require_role.clone(),
            expires_at: None,
            pose: Some(edge.to),
            grid_cell: None,
            edge_id: Some(edge.id.clone()),
            kind: Some(TaskKind::InspectionEdge),
        })
        .collect();

    let scenario = Scenario {
        name: "inspection".to_owned(),
        seed: config.seed,
        agents,
        tasks,
        ground_nodes: vec![],
        base_station: Some(config.graph.depot),
    };

    let run_config = RunConfig {
        max_ticks: config.max_ticks,
        timeout_ticks: 3,
        max_unassigned_ticks: config.max_ticks,
        packet_loss_rate: 0.0,
        latency_ticks: 0,
        latency_per_hop: 0,
        failures: vec![],
        dynamic_tasks: vec![],
        partition_events: vec![],
        gossip_interval_ticks: 999,
        base_id: None,
        enable_movement: true,
        tick_duration_ms: 1000,
        grid_state: None,
        enable_cbba: false,
        inspection_state: Some(InspectionState::new(config.graph.clone())),
        ..Default::default()
    };

    (scenario, run_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_inspection_scenario_tasks_match_edges() {
        let config = InspectionConfig {
            graph: InspectionGraph::linear_route(5, 10.0),
            agent_count: 2,
            battery_constraint: 0.0,
            require_role: None,
            seed: 42,
            max_ticks: 100,
        };
        let (scenario, _) = build_inspection_scenario(&config);
        assert_eq!(scenario.tasks.len(), 5);
    }

    #[test]
    fn build_inspection_scenario_edge_id_set() {
        let config = InspectionConfig {
            graph: InspectionGraph::linear_route(5, 10.0),
            agent_count: 2,
            battery_constraint: 0.0,
            require_role: None,
            seed: 42,
            max_ticks: 100,
        };
        let (scenario, _) = build_inspection_scenario(&config);
        let ids: Vec<_> = scenario
            .tasks
            .iter()
            .filter_map(|t| t.edge_id.clone())
            .collect();
        assert_eq!(ids.len(), 5);
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 5);
    }

    #[test]
    fn inspection_linear_3_agents_coverage_above_90() {
        let config = InspectionConfig {
            graph: InspectionGraph::linear_route(10, 10.0),
            agent_count: 3,
            battery_constraint: 0.0,
            require_role: None,
            seed: 42,
            max_ticks: 500,
        };
        let (scenario, run_config) = build_inspection_scenario(&config);
        let metrics = swarm_sim::ScenarioRunner::run(&scenario, run_config);
        eprintln!("linear metrics: exhausted={}, coverage={}, missed={}, revisits={}, total_ticks={}, final_battery_min={}, avg_distance_travelled={}, all_tasks_assigned={}",
            metrics.agents_exhausted, metrics.edge_coverage_rate, metrics.missed_edges, metrics.revisit_count, metrics.total_ticks, metrics.final_battery_min, metrics.avg_distance_travelled, metrics.all_tasks_assigned);
        assert!(metrics.success, "inspection linear should succeed");
        assert!(
            metrics.edge_coverage_rate > 0.9,
            "coverage should be > 0.9, got {}",
            metrics.edge_coverage_rate
        );
    }

    #[test]
    fn inspection_perimeter_battery_constraint_no_exhaustion() {
        let config = InspectionConfig {
            graph: InspectionGraph::grid_perimeter(5, 5, 5.0),
            agent_count: 4,
            battery_constraint: 0.3,
            require_role: None,
            seed: 42,
            max_ticks: 500,
        };
        let (scenario, run_config) = build_inspection_scenario(&config);
        let metrics = swarm_sim::ScenarioRunner::run(&scenario, run_config);
        eprintln!("perimeter metrics: exhausted={}, coverage={}, missed={}, revisits={}, total_ticks={}, final_battery_min={}, avg_distance_travelled={}, all_tasks_assigned={}",
            metrics.agents_exhausted, metrics.edge_coverage_rate, metrics.missed_edges, metrics.revisit_count, metrics.total_ticks, metrics.final_battery_min, metrics.avg_distance_travelled, metrics.all_tasks_assigned);
        assert_eq!(metrics.agents_exhausted, 0, "no agents should be exhausted");
        // The perimeter profile is intentionally battery-constrained; this unit
        // test guards the no-exhaustion behavior and a stable seed-42 coverage
        // floor, while broader coverage thresholds live in the regression suite.
        assert!(
            metrics.edge_coverage_rate >= 0.65,
            "coverage should stay >= 0.65, got {}",
            metrics.edge_coverage_rate
        );
    }
}
