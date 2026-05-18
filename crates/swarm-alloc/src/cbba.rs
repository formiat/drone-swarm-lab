use std::collections::HashMap;

use swarm_types::{AgentId, Pose, Task, TaskId};

use crate::allocator::{has_all_capabilities, has_required_role, AllocationAgent, AllocationTask};
use crate::Allocator;

/// Configuration for the Consensus-Based Bundle Algorithm.
pub struct CbbaConfig {
    pub max_bundle_size: usize,
    pub max_rounds: u32,
    pub score_weight_distance: f64,
    pub score_weight_battery: f64,
}

impl Default for CbbaConfig {
    fn default() -> Self {
        Self {
            max_bundle_size: 5,
            max_rounds: 20,
            score_weight_distance: 1.0,
            score_weight_battery: 0.5,
        }
    }
}

/// Consensus-Based Bundle Algorithm — distributed auction.
///
/// Agents iteratively build task bundles and exchange winning bids
/// through gossip messages. Convergence is detected when winning bids
/// remain stable for two consecutive rounds.
pub struct CbbaAllocator {
    pub config: CbbaConfig,
    /// Per-agent task bundles: agent_id → ordered list of task_ids.
    pub bundles: HashMap<AgentId, Vec<TaskId>>,
    /// Global winning bids: task_id → (winner_agent_id, bid_value).
    pub winning_bids: HashMap<TaskId, (AgentId, f64)>,
    /// Winning bids from the previous round (for convergence detection).
    prev_winning_bids: HashMap<TaskId, (AgentId, f64)>,
    /// Current CBBA round number (one per simulation tick).
    pub current_round: u32,
    /// Whether CBBA has converged (bundles stable).
    pub converged: bool,
    /// Total CBBA messages exchanged.
    pub messages_exchanged: u64,
}

/// A set of remote CBBA bids: sender_agent → (task_id → (winner, bid_value)).
pub type RemoteBids = HashMap<AgentId, HashMap<TaskId, (AgentId, f64)>>;

impl CbbaAllocator {
    pub fn new(config: CbbaConfig) -> Self {
        Self {
            config,
            bundles: HashMap::new(),
            winning_bids: HashMap::new(),
            prev_winning_bids: HashMap::new(),
            current_round: 0,
            converged: false,
            messages_exchanged: 0,
        }
    }

    /// Compute the marginal score for assigning a task to an agent,
    /// given the agent's existing bundle.
    pub fn marginal_score(&self, agent: &AllocationAgent, task: &Task, bundle: &[TaskId]) -> f64 {
        let task_pose = task.pose.unwrap_or(Pose { x: 0.0, y: 0.0 });
        let dist = agent.pose.distance_to(&task_pose);
        let base = -self.config.score_weight_distance * dist
            + self.config.score_weight_battery * agent.battery;

        if !has_all_capabilities(agent, &task.required_capabilities)
            || !has_required_role(agent, &task.required_role)
            || agent.battery <= 0.0
        {
            return f64::NEG_INFINITY;
        }

        // Marginal penalty: task further from last bundle task costs more
        if let Some(_last_id) = bundle.last() {
            let position_penalty = bundle.len() as f64 * 0.1 * dist;
            return base - position_penalty;
        }
        base
    }

    /// Phase 1: Bundle Building — each agent locally adds tasks to its bundle.
    pub fn build_bundles(&mut self, agents: &[AllocationAgent], tasks: &[AllocationTask<'_>]) {
        if self.converged {
            return;
        }

        for agent in agents {
            let bundle = self.bundles.entry(agent.id.clone()).or_default().clone();

            if bundle.len() >= self.config.max_bundle_size {
                continue;
            }

            let mut best_score = f64::NEG_INFINITY;
            let mut best_task_id: Option<TaskId> = None;

            for at in tasks {
                let task_id = &at.task.id;
                if bundle.contains(task_id) {
                    continue;
                }

                let score = self.marginal_score(agent, at.task, &bundle);
                if score <= f64::NEG_INFINITY + 1.0 {
                    continue;
                }

                // If someone else has a higher bid, skip
                if let Some((winner, existing_bid)) = self.winning_bids.get(task_id) {
                    if winner != &agent.id && *existing_bid >= score {
                        continue;
                    }
                }

                if score > best_score {
                    best_score = score;
                    best_task_id = Some(task_id.clone());
                }
            }

            if let Some(task_id) = best_task_id {
                if best_score > f64::NEG_INFINITY {
                    let bundle = self.bundles.get_mut(&agent.id).unwrap();
                    bundle.push(task_id.clone());
                    self.winning_bids
                        .insert(task_id, (agent.id.clone(), best_score));
                }
            }
        }
    }

    /// Phase 2: Consensus — apply remote winning bids received via gossip.
    #[allow(clippy::type_complexity)]
    pub fn apply_remote_bids(
        &mut self,
        remote_bids: &[(AgentId, HashMap<TaskId, (AgentId, f64)>)],
    ) {
        if self.converged {
            return;
        }

        for (_sender, bids) in remote_bids {
            for (task_id, (remote_agent_id, remote_bid)) in bids {
                match self.winning_bids.get(task_id) {
                    None => {
                        self.winning_bids
                            .insert(task_id.clone(), (remote_agent_id.clone(), *remote_bid));
                    }
                    Some((_local_agent, local_bid)) => {
                        if *remote_bid > *local_bid {
                            if let Some(bundle) = self.bundles.get_mut(remote_agent_id) {
                                if !bundle.contains(task_id) {
                                    bundle.push(task_id.clone());
                                }
                            }
                            self.winning_bids
                                .insert(task_id.clone(), (remote_agent_id.clone(), *remote_bid));
                        }
                    }
                }
            }
        }
        self.messages_exchanged += remote_bids.len() as u64;
    }

    /// Check if winning bids have converged (stable for 2 rounds).
    pub fn check_convergence(&mut self) -> bool {
        if self.converged {
            return true;
        }
        if self.prev_winning_bids == self.winning_bids && !self.winning_bids.is_empty() {
            self.converged = true;
            return true;
        }
        self.prev_winning_bids = self.winning_bids.clone();
        if self.current_round >= self.config.max_rounds {
            self.converged = true;
            return true;
        }
        false
    }

    /// Return current assignment decisions from bundles.
    pub fn current_assignments(&self) -> Vec<(TaskId, AgentId)> {
        let mut assignments = Vec::new();
        for (agent_id, bundle) in &self.bundles {
            for task_id in bundle {
                assignments.push((task_id.clone(), agent_id.clone()));
            }
        }
        assignments
    }
}

impl Default for CbbaAllocator {
    fn default() -> Self {
        Self::new(CbbaConfig::default())
    }
}

/// Allocator trait implementation — delegates to run_round for benchmark harness.
impl Allocator for CbbaAllocator {
    fn allocate(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)> {
        self.current_round += 1;
        self.build_bundles(agents, tasks);
        self.check_convergence();
        self.current_assignments()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::{AllocationAgent, AllocationTask};
    use swarm_types::{Pose, Role, TaskStatus};

    fn task(id: &str, priority: u8, x: f64, y: f64) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(Pose { x, y }),
            grid_cell: None,
        }
    }

    fn agent(id: &str, x: f64, y: f64) -> AllocationAgent {
        AllocationAgent {
            id: AgentId::from(id.to_owned()),
            pose: Pose { x, y },
            battery: 100.0,
            capabilities: vec![],
            role: Role::Scout,
            comms_range: f64::INFINITY,
        }
    }

    fn at(t: &Task) -> AllocationTask<'_> {
        AllocationTask { task: t }
    }

    #[test]
    fn cbba_config_defaults() {
        let config = CbbaConfig::default();
        assert_eq!(config.max_bundle_size, 5);
        assert_eq!(config.max_rounds, 20);
    }

    #[test]
    fn cbba_score_distance() {
        let cbba = CbbaAllocator::default();
        let t_near = task("t0", 1, 1.0, 0.0);
        let t_far = task("t1", 1, 100.0, 0.0);
        let a = agent("a0", 0.0, 0.0);
        let near = cbba.marginal_score(&a, &t_near, &[]);
        let far = cbba.marginal_score(&a, &t_far, &[]);
        assert!(near > far);
    }

    #[test]
    fn cbba_bundle_position_penalty() {
        let cbba = CbbaAllocator::default();
        let t1 = task("t1", 1, 10.0, 0.0);
        let t2 = task("t2", 1, 50.0, 0.0);
        let a = agent("a0", 0.0, 0.0);
        let score_without_bundle = cbba.marginal_score(&a, &t2, &[]);
        let score_with_bundle = cbba.marginal_score(&a, &t2, std::slice::from_ref(&t1.id));
        assert!(score_without_bundle > score_with_bundle);
    }

    #[test]
    fn cbba_bundle_capped_by_max_size() {
        let config = CbbaConfig {
            max_bundle_size: 2,
            ..Default::default()
        };
        let mut cbba = CbbaAllocator::new(config);
        let tasks: Vec<Task> = vec![
            task("t0", 1, 1.0, 0.0),
            task("t1", 1, 2.0, 0.0),
            task("t2", 1, 3.0, 0.0),
        ];
        let agents = vec![agent("a0", 0.0, 0.0)];
        let atasks: Vec<AllocationTask<'_>> = tasks.iter().map(|t| at(t)).collect();

        // build_bundles adds at most one task per agent per call
        for _ in 0..3 {
            cbba.build_bundles(&agents, &atasks);
        }
        assert!(
            cbba.bundles[&AgentId::from("a0".to_owned())].len() <= 2,
            "bundle size capped at max_bundle_size"
        );
    }

    #[test]
    fn cbba_round_assignments_converge() {
        let mut cbba = CbbaAllocator::default();
        let tasks: Vec<Task> = vec![
            task("t0", 1, 5.0, 0.0),
            task("t1", 1, 10.0, 0.0),
            task("t2", 1, 50.0, 0.0),
        ];
        let agents = vec![agent("a0", 0.0, 0.0), agent("a1", 45.0, 0.0)];
        let atasks: Vec<AllocationTask<'_>> = tasks.iter().map(|t| at(t)).collect();

        // Run multiple rounds until convergence
        for _ in 0..10 {
            cbba.current_round += 1;
            cbba.build_bundles(&agents, &atasks);
            cbba.check_convergence();
        }
        // After multiple rounds with same state, bundles should be stable
        assert!(cbba.converged);
    }

    #[test]
    fn cbba_conflicting_bids_resolution() {
        let mut cbba = CbbaAllocator::default();
        let tasks: Vec<Task> = vec![task("t0", 1, 5.0, 0.0)];
        // a1 is closer to task-0 (at x=5) than a0 (at x=0)
        let agents = vec![agent("a0", 0.0, 0.0), agent("a1", 4.0, 0.0)];
        let atasks: Vec<AllocationTask<'_>> = tasks.iter().map(|t| at(t)).collect();

        cbba.build_bundles(&agents, &atasks);
        // a1 (distance=1, score=-1+50=49) beats a0 (distance=5, score=-5+50=45)
        let winner = &cbba.winning_bids[&TaskId::from("t0".to_owned())].0;
        assert_eq!(*winner, AgentId::from("a1".to_owned()));
    }
}
