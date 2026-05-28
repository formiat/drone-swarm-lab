use std::collections::HashSet;

use swarm_types::{Agent, Pose, Task, TaskId};

/// Compute total Euclidean travel distance for a route starting at `start`
/// and visiting `tasks` in order.
pub fn route_cost(start: Pose, tasks: &[&Task]) -> f64 {
    let mut total = 0.0;
    let mut current = start;
    for task in tasks {
        if let Some(pose) = task.pose {
            total += current.distance_to(&pose);
            current = pose;
        }
    }
    total
}

/// Planner that orders tasks into a feasible route for an agent.
pub trait RoutePlanner: Send + Sync {
    /// Return an ordered list of `TaskId`s for the agent to visit.
    fn order(&self, start: Pose, tasks: &[Task], agent: &Agent) -> Vec<TaskId>;

    /// Check whether the agent can execute all tasks and return to `start`
    /// with the configured battery reserve.
    fn is_feasible(&self, start: Pose, tasks: &[Task], agent: &Agent) -> bool;
}

/// Greedy nearest-neighbour TSP ordering.
///
/// Builds a route by repeatedly visiting the closest unvisited task.
pub struct NearestNeighbourPlanner;

impl RoutePlanner for NearestNeighbourPlanner {
    fn order(&self, start: Pose, tasks: &[Task], _agent: &Agent) -> Vec<TaskId> {
        if tasks.len() <= 1 {
            return tasks.iter().map(|t| t.id.clone()).collect();
        }

        let mut ordered = Vec::new();
        let mut remaining: HashSet<TaskId> = tasks.iter().map(|t| t.id.clone()).collect();
        let mut current_pos = start;

        while !remaining.is_empty() {
            let next_id = remaining
                .iter()
                .min_by(|a, b| {
                    let ta = tasks.iter().find(|t| &t.id == *a);
                    let tb = tasks.iter().find(|t| &t.id == *b);
                    let da = ta
                        .and_then(|t| t.pose)
                        .map(|p| current_pos.distance_to(&p))
                        .unwrap_or(0.0);
                    let db = tb
                        .and_then(|t| t.pose)
                        .map(|p| current_pos.distance_to(&p))
                        .unwrap_or(0.0);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .cloned()
                .unwrap();

            remaining.remove(&next_id);
            ordered.push(next_id.clone());

            if let Some(task) = tasks.iter().find(|t| t.id == next_id) {
                current_pos = task.pose.unwrap_or(current_pos);
            }
        }

        ordered
    }

    fn is_feasible(&self, _start: Pose, _tasks: &[Task], _agent: &Agent) -> bool {
        // Nearest-neighbour does not perform feasibility checks.
        true
    }
}

/// 2-opt local search for TSP route improvement.
///
/// Starts from a nearest-neighbour route and iteratively swaps two edges
/// whenever the swap reduces total travel distance.
pub struct TwoOptPlanner {
    /// Maximum number of complete passes over the route.
    pub max_iterations: usize,
}

impl Default for TwoOptPlanner {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
        }
    }
}

impl RoutePlanner for TwoOptPlanner {
    fn order(&self, start: Pose, tasks: &[Task], agent: &Agent) -> Vec<TaskId> {
        if tasks.len() <= 2 {
            return NearestNeighbourPlanner.order(start, tasks, agent);
        }

        // Build a lookup: task_id -> Task for fast pose retrieval.
        let task_by_id: std::collections::HashMap<TaskId, &Task> =
            tasks.iter().map(|t| (t.id.clone(), t)).collect();

        // Start from NN ordering.
        let mut route: Vec<TaskId> = NearestNeighbourPlanner.order(start, tasks, agent);

        // Helper: compute total route cost including return to start.
        let cost = |r: &[TaskId]| -> f64 {
            let task_refs: Vec<&Task> = r.iter().map(|id| *task_by_id.get(id).unwrap()).collect();
            let mut total = route_cost(start, &task_refs);
            // Add return distance to start if the last task has a pose.
            if let Some(last_id) = r.last() {
                if let Some(last_task) = task_by_id.get(last_id) {
                    if let Some(pose) = last_task.pose {
                        total += pose.distance_to(&start);
                    }
                }
            }
            total
        };

        let n = route.len();
        let mut improved = true;
        let mut iterations = 0;

        while improved && iterations < self.max_iterations {
            improved = false;
            iterations += 1;

            for i in 0..n {
                for j in i + 2..n {
                    // Reverse the segment route[i+1 ..= j].
                    let mut new_route = route.clone();
                    new_route[i + 1..=j].reverse();

                    if cost(&new_route) < cost(&route) {
                        route = new_route;
                        improved = true;
                    }
                }
            }
        }

        route
    }

    fn is_feasible(&self, _start: Pose, _tasks: &[Task], _agent: &Agent) -> bool {
        true
    }
}

/// Battery-aware planner that wraps an inner planner and drops tasks
/// from the end of the route until the route becomes feasible.
pub struct BatteryAwarePlanner {
    /// Minimum fraction of battery that must remain after returning to start.
    pub reserve_fraction: f64,
    /// Inner planner that produces the initial ordering.
    pub inner: Box<dyn RoutePlanner>,
}

impl Default for BatteryAwarePlanner {
    fn default() -> Self {
        Self {
            reserve_fraction: 0.2,
            inner: Box::new(NearestNeighbourPlanner),
        }
    }
}

impl RoutePlanner for BatteryAwarePlanner {
    fn order(&self, start: Pose, tasks: &[Task], agent: &Agent) -> Vec<TaskId> {
        let mut ordered = self.inner.order(start, tasks, agent);
        // Build lookup from task id to task for ordered subset feasibility checks.
        let task_by_id: std::collections::HashMap<TaskId, &Task> =
            tasks.iter().map(|t| (t.id.clone(), t)).collect();
        let mut ordered_tasks: Vec<Task> = ordered
            .iter()
            .filter_map(|id| task_by_id.get(id).cloned().cloned())
            .collect();
        // Drop from the END of the ordered route until feasible.
        while !ordered_tasks.is_empty() && !self.is_feasible(start, &ordered_tasks, agent) {
            ordered_tasks.pop();
            ordered.pop();
        }
        ordered
    }

    fn is_feasible(&self, start: Pose, tasks: &[Task], agent: &Agent) -> bool {
        if tasks.is_empty() {
            return true;
        }

        let reserve = if let Some(ref model) = agent.battery_model {
            model.reserve_fraction
        } else {
            self.reserve_fraction
        };

        let required_battery = compute_route_battery_drain(start, tasks, agent);
        required_battery <= agent.battery * (1.0 - reserve)
    }
}

/// Compute total battery drain for a route starting at `start`, visiting `tasks`
/// in order, and returning to `start`.
/// Uses `battery_model` v2 when available, otherwise falls back to legacy
/// `battery_drain_rate`.
fn compute_route_battery_drain(start: Pose, tasks: &[Task], agent: &Agent) -> f64 {
    if let Some(ref model) = agent.battery_model {
        let mut current = start;
        let mut total_drain = 0.0;
        for task in tasks {
            if let Some(pose) = task.pose {
                let horizontal = current.distance_to_2d(&pose);
                let vertical = (current.z - pose.z).abs();
                total_drain += horizontal * model.cruise_drain_per_meter
                    + vertical * model.climb_drain_per_meter;
                current = pose;
            }
        }
        // Return to start.
        let horizontal = current.distance_to_2d(&start);
        let vertical = (current.z - start.z).abs();
        total_drain +=
            horizontal * model.cruise_drain_per_meter + vertical * model.climb_drain_per_meter;
        total_drain
    } else {
        let task_refs: Vec<&Task> = tasks.iter().collect();
        let mut total_distance = route_cost(start, &task_refs);
        if let Some(last) = tasks.last() {
            if let Some(pose) = last.pose {
                total_distance += pose.distance_to(&start);
            }
        }
        total_distance * agent.battery_drain_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::{AgentId, Health, Pose, Role, TaskId, TaskStatus};

    fn make_task(id: &str, x: f64, y: f64) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(Pose {
                x,
                y,
                ..Default::default()
            }),
            grid_cell: None,
            edge_id: None,
            kind: None,
        }
    }

    fn make_agent(battery: f64, drain_rate: f64) -> Agent {
        Agent {
            id: AgentId::from("a0".to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            capabilities: vec![],
            current_task: None,
            battery,
            comms_range: f64::INFINITY,
            generation: 1,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: drain_rate,
            battery_model: None,
        }
    }

    #[test]
    fn nn_orders_nearest_first() {
        let tasks = vec![make_task("far", 100.0, 0.0), make_task("near", 1.0, 0.0)];
        let agent = make_agent(100.0, 0.0);
        let ordered = NearestNeighbourPlanner.order(
            Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            &tasks,
            &agent,
        );
        assert_eq!(ordered[0], TaskId::from("near".to_owned()));
        assert_eq!(ordered[1], TaskId::from("far".to_owned()));
    }

    #[test]
    fn nn_returns_all_tasks() {
        let tasks: Vec<Task> = (0..5)
            .map(|i| make_task(&format!("t{i}"), i as f64, 0.0))
            .collect();
        let agent = make_agent(100.0, 0.0);
        let ordered = NearestNeighbourPlanner.order(
            Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            &tasks,
            &agent,
        );
        assert_eq!(ordered.len(), 5);
        let unique: HashSet<_> = ordered.iter().collect();
        assert_eq!(unique.len(), 5);
    }

    #[test]
    fn two_opt_does_not_worsen_route() {
        let tasks: Vec<Task> = (0..8)
            .map(|i| make_task(&format!("t{i}"), (i * 7) as f64, (i * 3) as f64))
            .collect();
        let agent = make_agent(100.0, 0.0);
        let start = Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        };

        let nn = NearestNeighbourPlanner.order(start, &tasks, &agent);
        let two = TwoOptPlanner::default().order(start, &tasks, &agent);

        let nn_refs: Vec<&Task> = nn
            .iter()
            .map(|id| tasks.iter().find(|t| t.id == *id).unwrap())
            .collect();
        let two_refs: Vec<&Task> = two
            .iter()
            .map(|id| tasks.iter().find(|t| t.id == *id).unwrap())
            .collect();

        assert_eq!(two.len(), tasks.len(), "2-opt must return all tasks");
        assert!(
            route_cost(start, &two_refs) <= route_cost(start, &nn_refs),
            "2-opt should not worsen route cost"
        );
    }

    #[test]
    fn battery_aware_rejects_infeasible() {
        // Agent with 10% battery, drain 1% per meter.
        // 3 tasks at distance 10 each → total ~30m + return ~10m = 40m
        // Required battery = 40 * 1.0 = 40 > 10 * 0.8 = 8 → infeasible.
        let tasks = vec![
            make_task("t0", 10.0, 0.0),
            make_task("t1", 20.0, 0.0),
            make_task("t2", 30.0, 0.0),
        ];
        let agent = make_agent(10.0, 1.0);
        let planner = BatteryAwarePlanner {
            reserve_fraction: 0.2,
            inner: Box::new(NearestNeighbourPlanner),
        };
        let start = Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        };

        assert!(!planner.is_feasible(start, &tasks, &agent));
        let ordered = planner.order(start, &tasks, &agent);
        assert!(
            ordered.len() < tasks.len(),
            "battery-aware should drop tasks from infeasible bundle"
        );
    }

    #[test]
    fn battery_aware_accepts_feasible() {
        let tasks = vec![make_task("t0", 1.0, 0.0)];
        let agent = make_agent(100.0, 0.1);
        let planner = BatteryAwarePlanner::default();
        let start = Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        };

        assert!(planner.is_feasible(start, &tasks, &agent));
        let ordered = planner.order(start, &tasks, &agent);
        assert_eq!(ordered.len(), 1);
    }

    #[test]
    fn route_cost_empty_is_zero() {
        let cost = route_cost(
            Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            &[],
        );
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn route_cost_single_task() {
        let t = make_task("t0", 5.0, 0.0);
        let cost = route_cost(
            Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            &[&t],
        );
        assert!((cost - 5.0).abs() < 1e-6);
    }

    #[test]
    fn two_opt_is_permutation() {
        let tasks: Vec<Task> = (0..6)
            .map(|i| make_task(&format!("t{i}"), i as f64 * 5.0, i as f64 * 3.0))
            .collect();
        let agent = make_agent(100.0, 0.0);
        let start = Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        };

        let ordered = TwoOptPlanner::default().order(start, &tasks, &agent);
        assert_eq!(ordered.len(), tasks.len());

        let input_ids: HashSet<_> = tasks.iter().map(|t| &t.id).collect();
        let output_ids: HashSet<_> = ordered.iter().collect();
        assert_eq!(input_ids, output_ids);
    }

    #[test]
    fn battery_aware_order_drops_tasks_on_ordered_subset() {
        // Tasks ordered as t0(5), t1(10), t2(20).
        // With battery 30 and drain 1.0/m, reserve 0.2 -> budget = 24.
        // NN orders: t0, t1, t2 (nearest first from origin).
        // Full route: 0->5->10->20->0 = 40m, infeasible.
        // Dropping only t2: 0->5->10->0 = 20m, feasible.
        // This proves feasibility is checked on the current ordered subset.
        let tasks = vec![
            make_task("t0", 5.0, 0.0),
            make_task("t1", 10.0, 0.0),
            make_task("t2", 20.0, 0.0),
        ];
        let agent = make_agent(30.0, 1.0);
        let planner = BatteryAwarePlanner {
            reserve_fraction: 0.2,
            inner: Box::new(NearestNeighbourPlanner),
        };
        let start = Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        };

        let ordered = planner.order(start, &tasks, &agent);
        assert_eq!(
            ordered,
            vec![TaskId::from("t0".to_owned()), TaskId::from("t1".to_owned())]
        );
    }

    #[test]
    fn battery_aware_order_returns_empty_when_first_task_infeasible() {
        let tasks = vec![make_task("t0", 20.0, 0.0)];
        let agent = make_agent(30.0, 1.0);
        let planner = BatteryAwarePlanner {
            reserve_fraction: 0.2,
            inner: Box::new(NearestNeighbourPlanner),
        };
        let start = Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        };

        let ordered = planner.order(start, &tasks, &agent);
        assert!(
            ordered.is_empty(),
            "Single task route requires 40m battery drain, above the 24m budget"
        );
    }

    #[test]
    fn battery_aware_v2_feasibility_uses_model() {
        let tasks = vec![make_task("t0", 10.0, 0.0)];
        let agent = Agent {
            id: AgentId::from("a0".to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            capabilities: vec![],
            current_task: None,
            battery: 5.0,
            comms_range: f64::INFINITY,
            generation: 1,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
            battery_model: Some(swarm_types::BatteryModel {
                hover_drain_per_tick: 0.0,
                climb_drain_per_meter: 0.0,
                cruise_drain_per_meter: 0.1,
                reserve_fraction: 0.2,
            }),
        };
        let planner = BatteryAwarePlanner {
            reserve_fraction: 0.2,
            inner: Box::new(NearestNeighbourPlanner),
        };
        let start = Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        };

        // 10m + return 10m = 20m * 0.1 = 2.0 drain.
        // Battery 5.0, reserve 0.2 → budget = 4.0.
        // 2.0 <= 4.0 → feasible.
        assert!(planner.is_feasible(start, &tasks, &agent));

        // With cruise_drain_per_meter = 1.0:
        // 20m * 1.0 = 20.0 drain > 4.0 budget → infeasible.
        let mut agent_high_drain = agent.clone();
        agent_high_drain.battery_model = Some(swarm_types::BatteryModel {
            hover_drain_per_tick: 0.0,
            climb_drain_per_meter: 0.0,
            cruise_drain_per_meter: 1.0,
            reserve_fraction: 0.2,
        });
        assert!(!planner.is_feasible(start, &tasks, &agent_high_drain));
    }

    #[test]
    fn battery_aware_v2_order_drops_with_model() {
        let tasks = vec![
            make_task("t0", 10.0, 0.0),
            make_task("t1", 20.0, 0.0),
            make_task("t2", 30.0, 0.0),
        ];
        let agent = Agent {
            id: AgentId::from("a0".to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose {
                x: 0.0,
                y: 0.0,
                ..Default::default()
            },
            capabilities: vec![],
            current_task: None,
            battery: 5.0,
            comms_range: f64::INFINITY,
            generation: 1,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
            battery_model: Some(swarm_types::BatteryModel {
                hover_drain_per_tick: 0.0,
                climb_drain_per_meter: 0.0,
                cruise_drain_per_meter: 1.0,
                reserve_fraction: 0.2,
            }),
        };
        let planner = BatteryAwarePlanner {
            reserve_fraction: 0.2,
            inner: Box::new(NearestNeighbourPlanner),
        };
        let start = Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        };

        let ordered = planner.order(start, &tasks, &agent);
        // Budget = 5.0 * 0.8 = 4.0.
        // NN order: t0(10), t1(20), t2(30).
        // Route: 0→10→20→30→0 = 60m. Drain = 60. Infeasible.
        // Drop t2: 0→10→20→0 = 40m. Drain = 40. Infeasible.
        // Drop t1: 0→10→0 = 20m. Drain = 20. Infeasible.
        // Drop t0: 0. Feasible.
        assert!(ordered.is_empty());
    }

    #[test]
    fn route_cost_includes_z() {
        let start = Pose {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let task = Task {
            id: TaskId::from("t0".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(Pose {
                x: 3.0,
                y: 4.0,
                z: 5.0,
            }),
            grid_cell: None,
            edge_id: None,
            kind: None,
        };
        let cost = route_cost(start, &[&task]);
        let expected = (3.0f64 * 3.0 + 4.0 * 4.0 + 5.0 * 5.0).sqrt();
        assert!(
            (cost - expected).abs() < 1e-6,
            "route_cost should include z"
        );
    }
}
