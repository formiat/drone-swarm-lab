use std::collections::{HashMap, HashSet};

use swarm_types::{AgentId, Pose, Task, TaskId};

use crate::allocator::{has_all_capabilities, has_required_role, AllocationAgent, AllocationTask};
use crate::Allocator;

/// Configuration for the Consensus-Based Bundle Algorithm.
pub struct CbbaConfig {
    pub max_bundle_size: usize,
    pub max_rounds: u32,
    pub score_weight_distance: f64,
    pub score_weight_battery: f64,
    // v0.15 retransmission
    pub retransmit_max_attempts: u32,
    pub retransmit_backoff_ticks: u64,
    pub retransmit_threshold_packet_loss: f64,
}

impl Default for CbbaConfig {
    fn default() -> Self {
        Self {
            max_bundle_size: 5,
            max_rounds: 20,
            score_weight_distance: 1.0,
            score_weight_battery: 0.5,
            retransmit_max_attempts: 3,
            retransmit_backoff_ticks: 2,
            retransmit_threshold_packet_loss: 0.1,
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
    // v0.15 bundle travel distance
    pub bundle_travel_distance: f64,
    // v0.15 retransmission
    pub packet_loss_rate: f64,
    force_rebroadcast: bool,
}

/// Greedy nearest-neighbour TSP ordering for task bundles.
pub fn order_bundle_tsp(agent_pose: Pose, bundle: &[TaskId], tasks: &[Task]) -> Vec<TaskId> {
    if bundle.len() <= 1 {
        return bundle.to_vec();
    }
    let mut ordered = Vec::new();
    let mut remaining: HashSet<TaskId> = bundle.iter().cloned().collect();
    let mut current_pos = agent_pose;

    while !remaining.is_empty() {
        let next = remaining
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
        remaining.remove(&next);
        ordered.push(next.clone());
        if let Some(task) = tasks.iter().find(|t| t.id == next) {
            current_pos = task.pose.unwrap_or(current_pos);
        }
    }
    ordered
}

/// Compute total travel distance for a bundle with TSP ordering.
pub fn bundle_travel_distance(agent_pose: Pose, ordered_bundle: &[TaskId], tasks: &[Task]) -> f64 {
    let mut total = 0.0;
    let mut current = agent_pose;
    for tid in ordered_bundle {
        if let Some(task) = tasks.iter().find(|t| &t.id == tid) {
            if let Some(pose) = task.pose {
                total += current.distance_to(&pose);
                current = pose;
            }
        }
    }
    total
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
            bundle_travel_distance: 0.0,
            packet_loss_rate: 0.0,
            force_rebroadcast: false,
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
                    Some((local_agent, local_bid)) => {
                        if *remote_bid > *local_bid {
                            // Remove task from losing agent's bundle
                            if let Some(local_bundle) = self.bundles.get_mut(local_agent) {
                                local_bundle.retain(|t| t != task_id);
                            }
                            // Add task to winning agent's bundle
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

        // Reset convergence when new unassigned tasks appear (agent failure, task release)
        if !tasks.is_empty() && self.converged {
            self.converged = false;
        }

        // Clear stale state for released tasks and dead agents
        let alive_ids: std::collections::HashSet<&AgentId> = agents.iter().map(|a| &a.id).collect();
        self.bundles
            .retain(|agent_id, _| alive_ids.contains(agent_id));
        self.winning_bids.retain(|task_id, (agent_id, _)| {
            alive_ids.contains(agent_id) && tasks.iter().any(|t| &t.task.id == task_id)
        });
        self.prev_winning_bids.retain(|task_id, (agent_id, _)| {
            alive_ids.contains(agent_id) && tasks.iter().any(|t| &t.task.id == task_id)
        });

        // Free bundle slots occupied by completed tasks (no longer in winning_bids).
        for bundle in self.bundles.values_mut() {
            bundle.retain(|task_id| self.winning_bids.contains_key(task_id));
        }

        self.build_bundles(agents, tasks);

        // v0.15: TSP-ordering of bundles after building
        let task_list: Vec<Task> = tasks.iter().map(|at| at.task.clone()).collect();
        for (agent_id, bundle) in self.bundles.iter_mut() {
            if let Some(agent) = agents.iter().find(|a| a.id == *agent_id) {
                *bundle = order_bundle_tsp(agent.pose, bundle, &task_list);
            }
        }

        // v0.15: Retransmission — periodic rebroadcast when packet loss is high
        if self.packet_loss_rate > self.config.retransmit_threshold_packet_loss
            && self
                .current_round
                .is_multiple_of(self.config.retransmit_backoff_ticks as u32)
        {
            self.force_rebroadcast = true;
            // Simulate retransmission overhead: increase messages_exchanged
            self.messages_exchanged += agents.len() as u64;
        }
        if self.force_rebroadcast {
            // Force convergence re-evaluation on next round
            self.prev_winning_bids.clear();
            self.force_rebroadcast = false;
        }

        self.check_convergence();

        // v0.15: Bundle travel distance metric
        self.bundle_travel_distance = 0.0;
        for (agent_id, bundle) in &self.bundles {
            if let Some(agent) = agents.iter().find(|a| a.id == *agent_id) {
                self.bundle_travel_distance +=
                    bundle_travel_distance(agent.pose, bundle, &task_list);
            }
        }

        self.current_assignments()
    }

    fn allocation_metrics(&self) -> (u64, bool, u64) {
        (
            self.current_round as u64,
            self.converged,
            self.messages_exchanged,
        )
    }

    fn is_distributed(&self) -> bool {
        true
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
            edge_id: None,
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

    #[test]
    fn cbba_reassigns_after_agent_removal() {
        let mut cbba = CbbaAllocator::default();
        let tasks: Vec<Task> = vec![task("t0", 1, 5.0, 0.0), task("t1", 1, 10.0, 0.0)];
        let agents = vec![agent("a0", 0.0, 0.0), agent("a1", 8.0, 0.0)];
        let atasks: Vec<AllocationTask<'_>> = tasks.iter().map(|t| at(t)).collect();

        // Round 1: a0 gets t0 (closer), a1 gets t1 (closer)
        let result = cbba.allocate(&atasks, &agents);
        assert_eq!(result.len(), 2);

        // Simulate a0 failing: call allocate with only a1
        let survivors = vec![agent("a1", 8.0, 0.0)];
        let result = cbba.allocate(&atasks, &survivors);
        // t0 should be reassigned to a1
        assert!(result.iter().any(|(task_id, agent_id)| {
            *task_id == TaskId::from("t0".to_owned()) && *agent_id == AgentId::from("a1".to_owned())
        }));
    }

    #[test]
    fn cbba_is_distributed() {
        assert!(CbbaAllocator::default().is_distributed());
    }

    /// Verifies that completed tasks (removed from `tasks` slice) have their
    /// bundle slots freed so new tasks can fill them.
    #[test]
    fn cbba_bundle_slots_freed_after_task_completion() {
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

        // Fill the bundle to capacity: two allocate calls add one task each.
        cbba.allocate(&atasks, &agents);
        cbba.allocate(&atasks, &agents);
        let bundle = &cbba.bundles[&AgentId::from("a0".to_owned())];
        assert_eq!(
            bundle.len(),
            2,
            "bundle should be full at max_bundle_size=2"
        );

        // Simulate t0 completing: remove it from the active task list.
        let remaining: Vec<Task> = tasks[1..].to_vec();
        let atasks2: Vec<AllocationTask<'_>> = remaining.iter().map(|t| at(t)).collect();

        // After the fix, the freed slot should allow t2 to be added.
        cbba.allocate(&atasks2, &agents);
        let bundle = &cbba.bundles[&AgentId::from("a0".to_owned())];
        assert!(
            bundle.contains(&TaskId::from("t2".to_owned())),
            "t2 should enter the bundle after the slot freed by completed t0; bundle: {:?}",
            bundle
        );
    }

    #[test]
    fn order_bundle_tsp_nearest_first() {
        let agent_pose = Pose { x: 0.0, y: 0.0 };
        let t_near = task("t_near", 1, 1.0, 0.0);
        let t_far = task("t_far", 1, 100.0, 0.0);
        let tasks = vec![t_far.clone(), t_near.clone()];
        let bundle = vec![t_far.id.clone(), t_near.id.clone()];
        let ordered = order_bundle_tsp(agent_pose, &bundle, &tasks);
        assert_eq!(ordered[0], t_near.id);
    }

    #[test]
    fn order_bundle_tsp_all_tasks_included() {
        let agent_pose = Pose { x: 0.0, y: 0.0 };
        let t1 = task("t1", 1, 10.0, 0.0);
        let t2 = task("t2", 1, 50.0, 0.0);
        let t3 = task("t3", 1, 30.0, 0.0);
        let tasks = vec![t1.clone(), t2.clone(), t3.clone()];
        let bundle = vec![t1.id.clone(), t2.id.clone(), t3.id.clone()];
        let ordered = order_bundle_tsp(agent_pose, &bundle, &tasks);
        assert_eq!(ordered.len(), 3);
        // All tasks must be present
        assert!(ordered.contains(&t1.id));
        assert!(ordered.contains(&t2.id));
        assert!(ordered.contains(&t3.id));
    }

    #[test]
    fn cbba_config_retransmit_defaults() {
        let config = CbbaConfig::default();
        assert_eq!(config.retransmit_max_attempts, 3);
        assert_eq!(config.retransmit_backoff_ticks, 2);
        assert!((config.retransmit_threshold_packet_loss - 0.1).abs() < 1e-6);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn cbba_retransmission_increases_messages() {
        let mut cbba = CbbaAllocator::default();
        cbba.packet_loss_rate = 0.2; // above default threshold 0.1
        let tasks: Vec<Task> = vec![task("t0", 1, 1.0, 0.0), task("t1", 1, 2.0, 0.0)];
        let agents = vec![agent("a0", 0.0, 0.0), agent("a1", 10.0, 0.0)];
        let atasks: Vec<AllocationTask<'_>> = tasks.iter().map(|t| at(t)).collect();

        let msg_before = cbba.messages_exchanged;
        cbba.allocate(&atasks, &agents);
        // With packet_loss=0.2 > threshold=0.1 and round=1, retransmission should fire
        // because 1 % retransmit_backoff_ticks(2) == 0? No, 1%2=1. Let's try round 2.
        cbba.allocate(&atasks, &agents);
        let msg_after = cbba.messages_exchanged;
        assert!(
            msg_after > msg_before,
            "retransmission should increase messages_exchanged: before={}, after={}",
            msg_before,
            msg_after
        );
    }

    #[test]
    fn cbba_tsp_reduces_travel_distance() {
        let agent_pose = Pose { x: 0.0, y: 0.0 };
        let tasks = vec![
            task("t_far", 1, 100.0, 0.0),
            task("t_near", 1, 1.0, 0.0),
            task("t_mid", 1, 50.0, 0.0),
        ];
        let bundle = vec![
            tasks[0].id.clone(),
            tasks[1].id.clone(),
            tasks[2].id.clone(),
        ];
        let original_dist = bundle_travel_distance(agent_pose, &bundle, &tasks);
        let ordered = order_bundle_tsp(agent_pose, &bundle, &tasks);
        let tsp_dist = bundle_travel_distance(agent_pose, &ordered, &tasks);
        assert!(
            tsp_dist <= original_dist,
            "TSP ordering should reduce travel distance: original={}, tsp={}",
            original_dist,
            tsp_dist
        );
        assert_eq!(ordered[0], tasks[1].id, "nearest task should be first");
    }
}
