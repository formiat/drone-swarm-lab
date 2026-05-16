use std::collections::{HashMap, HashSet};

use swarm_alloc::{AllocationAgent, AllocationTask, Allocator, GreedyAllocator};
use swarm_comms::{InMemNetwork, NetworkConfig, RawMessage, Transport};
use swarm_metrics::RunMetrics;
use swarm_runtime::Coordinator;
use swarm_types::{AgentId, Task, TaskId};

use crate::{Clock, Scenario};

#[derive(Clone, Debug)]
pub struct FailureEvent {
    pub agent_id: AgentId,
    pub at_tick: u64,
}

#[derive(Clone, Debug)]
pub struct DynamicTaskEvent {
    pub at_tick: u64,
    pub task: Task,
}

#[derive(Clone, Debug)]
pub struct RunConfig {
    pub max_ticks: u64,
    pub timeout_ticks: u64,
    pub max_unassigned_ticks: u64,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub failures: Vec<FailureEvent>,
    pub dynamic_tasks: Vec<DynamicTaskEvent>,
}

pub struct ScenarioRunner;

impl ScenarioRunner {
    pub fn run(scenario: &Scenario, config: RunConfig) -> RunMetrics {
        Self::run_with(scenario, config, GreedyAllocator)
    }

    pub fn run_with<A: Allocator>(
        scenario: &Scenario,
        config: RunConfig,
        allocator: A,
    ) -> RunMetrics {
        let mut network = InMemNetwork::new(NetworkConfig {
            packet_loss_rate: config.packet_loss_rate,
            latency_ticks: config.latency_ticks,
            seed: scenario.seed,
        });
        let mut coordinator = Coordinator::new(
            scenario.agents.clone(),
            scenario.tasks.clone(),
            config.timeout_ticks,
        );
        let mut clock = Clock::new(1);
        let coordinator_id = AgentId::from("coordinator".to_owned());
        let failure_ticks: HashMap<AgentId, u64> = config
            .failures
            .iter()
            .map(|failure| (failure.agent_id.clone(), failure.at_tick))
            .collect();
        let mut crashed_agents = HashSet::new();
        let mut detected_agents = HashSet::new();
        let mut unassigned_durations: HashMap<TaskId, u64> = HashMap::new();
        let mut max_task_unassigned_ticks = 0;
        let mut detection_time_ticks = None;
        let mut detection_tick = None;
        let mut reallocation_time_ticks = None;
        let mut total_ticks = 0;
        let mut tasks_injected: u64 = 0;
        let mut tasks_expired: u64 = 0;
        let mut conflicting_assignments: u64 = 0;

        conflicting_assignments += allocate_unassigned(&mut coordinator, &allocator);

        for _ in 0..config.max_ticks {
            clock.advance();
            let current_tick = u64::from(clock.now());
            total_ticks = current_tick;

            for failure in config
                .failures
                .iter()
                .filter(|failure| failure.at_tick == current_tick)
            {
                crashed_agents.insert(failure.agent_id.clone());
            }

            let heartbeat_senders: Vec<_> = coordinator
                .membership
                .alive_agents()
                .map(|(agent_id, _)| agent_id.clone())
                .filter(|agent_id| !crashed_agents.contains(agent_id))
                .collect();

            for agent_id in heartbeat_senders {
                network
                    .send(RawMessage {
                        from: agent_id.clone(),
                        to: coordinator_id.clone(),
                        payload: agent_id.to_string().into_bytes(),
                    })
                    .expect("in-memory transport is infallible");
            }

            network.advance_tick();
            let heartbeat_senders = network
                .drain_ready(&coordinator_id)
                .into_iter()
                .filter_map(|message| String::from_utf8(message.payload).ok())
                .map(AgentId::from)
                .collect();

            let injected: Vec<Task> = config
                .dynamic_tasks
                .iter()
                .filter(|ev| ev.at_tick == current_tick)
                .map(|ev| ev.task.clone())
                .collect();
            tasks_injected += injected.len() as u64;

            let output = coordinator.process_tick(heartbeat_senders, current_tick, injected);

            tasks_expired += output.expired_task_ids.len() as u64;

            if detection_time_ticks.is_none() && !output.newly_failed.is_empty() {
                let first_failure_tick = output
                    .newly_failed
                    .iter()
                    .filter_map(|agent_id| failure_ticks.get(agent_id))
                    .min()
                    .copied()
                    .unwrap_or(current_tick);
                detection_time_ticks = Some(current_tick.saturating_sub(first_failure_tick));
                detection_tick = Some(current_tick);
            }
            detected_agents.extend(output.newly_failed.iter().cloned());

            max_task_unassigned_ticks = update_unassigned_durations(
                &coordinator,
                &mut unassigned_durations,
                max_task_unassigned_ticks,
            );

            if !output.released_tasks.is_empty()
                || !output.expired_task_ids.is_empty()
                || !coordinator.registry.unassigned().is_empty()
            {
                let conflicts = allocate_unassigned(&mut coordinator, &allocator);
                conflicting_assignments += conflicts;
                if let Some(detected_at) = detection_tick {
                    if reallocation_time_ticks.is_none()
                        && released_tasks_reassigned(&coordinator, &output.released_tasks)
                    {
                        reallocation_time_ticks = Some(current_tick.saturating_sub(detected_at));
                    }
                }
            }

            let all_expected_failures_detected = crashed_agents
                .iter()
                .all(|agent_id| detected_agents.contains(agent_id));
            let all_failure_ticks_passed = config
                .failures
                .iter()
                .all(|failure| current_tick >= failure.at_tick);
            let all_dynamic_tasks_injected = config
                .dynamic_tasks
                .iter()
                .all(|ev| current_tick >= ev.at_tick);
            if coordinator.registry.all_assigned_or_completed()
                && max_task_unassigned_ticks <= config.max_unassigned_ticks
                && all_failure_ticks_passed
                && all_expected_failures_detected
                && all_dynamic_tasks_injected
            {
                break;
            }
        }

        let all_expected_failures_detected = config
            .failures
            .iter()
            .all(|failure| detected_agents.contains(&failure.agent_id));
        let all_tasks_assigned = coordinator.registry.all_assigned_or_completed();
        let success = all_tasks_assigned
            && all_expected_failures_detected
            && max_task_unassigned_ticks <= config.max_unassigned_ticks;

        RunMetrics {
            seed: scenario.seed,
            total_ticks,
            messages_attempted: network.messages_attempted(),
            messages_dropped: network.messages_dropped(),
            detection_time_ticks,
            reallocation_time_ticks,
            max_task_unassigned_ticks,
            all_tasks_assigned,
            success,
            tasks_injected,
            tasks_expired,
            conflicting_assignments,
        }
    }
}

/// Returns the number of conflicting assignment decisions in this allocation round.
fn allocate_unassigned<A: Allocator>(coordinator: &mut Coordinator, allocator: &A) -> u64 {
    let tasks: Vec<Task> = coordinator
        .registry
        .unassigned()
        .into_iter()
        .cloned()
        .collect();
    let allocation_tasks: Vec<AllocationTask<'_>> =
        tasks.iter().map(|task| AllocationTask { task }).collect();

    let agents: Vec<AllocationAgent> = coordinator
        .membership
        .alive_agents()
        .map(|(id, entry)| AllocationAgent {
            id: id.clone(),
            pose: entry.pose,
            battery: entry.battery,
            capabilities: entry.capabilities.clone(),
            role: entry.role.clone(),
        })
        .collect();

    let decisions = allocator.allocate(&allocation_tasks, &agents);

    // Deduplication pass: allocator must not produce two decisions for the same task.
    let mut seen = HashSet::new();
    let mut conflicts: u64 = 0;
    for (task_id, agent_id) in decisions {
        if !seen.insert(task_id.clone()) {
            // Duplicate task_id from allocator output — first decision wins.
            conflicts += 1;
            continue;
        }
        if coordinator.registry.assign(&task_id, agent_id).is_err() {
            // Task became non-assignable between unassigned() and assign() calls.
            conflicts += 1;
        }
    }
    conflicts
}

fn update_unassigned_durations(
    coordinator: &Coordinator,
    durations: &mut HashMap<TaskId, u64>,
    current_max: u64,
) -> u64 {
    let unassigned: HashSet<_> = coordinator
        .registry
        .unassigned()
        .into_iter()
        .map(|task| task.id.clone())
        .collect();
    durations.retain(|task_id, _| unassigned.contains(task_id));

    let mut max_duration = current_max;
    for task_id in unassigned {
        let duration = durations.entry(task_id).or_insert(0);
        *duration += 1;
        max_duration = max_duration.max(*duration);
    }
    max_duration
}

fn released_tasks_reassigned(coordinator: &Coordinator, released_tasks: &[TaskId]) -> bool {
    released_tasks.iter().all(|released_task| {
        coordinator
            .registry
            .tasks()
            .any(|task| &task.id == released_task && task.assigned_to.is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_alloc::{AllocationAgent, AllocationTask, Allocator};
    use swarm_types::{Agent, Capability, Health, Pose, Role, Task, TaskStatus};

    fn scenario(seed: u64, agent_count: usize, task_count: usize) -> Scenario {
        let agents = (0..agent_count)
            .map(|index| Agent {
                id: AgentId::from(format!("agent-{index}")),
                role: Role::Scout,
                health: Health::Alive,
                pose: Pose { x: 0.0, y: 0.0 },
                capabilities: Vec::new(),
                current_task: None,
                battery: 100.0,
            })
            .collect();
        let tasks = (0..task_count)
            .map(|index| Task {
                id: TaskId::from(format!("task-{index}")),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: 1,
                required_capabilities: vec![],
                preferred_role: None,
                expires_at: None,
                pose: None,
            })
            .collect();
        Scenario {
            name: "test".to_owned(),
            seed,
            agents,
            tasks,
        }
    }

    fn config(failures: Vec<FailureEvent>) -> RunConfig {
        RunConfig {
            max_ticks: 50,
            timeout_ticks: 3,
            max_unassigned_ticks: 5,
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            failures,
            dynamic_tasks: vec![],
        }
    }

    #[test]
    fn runner_timeout_semantics_before_after_detection() {
        let scenario = scenario(0, 5, 8);
        let metrics = ScenarioRunner::run(
            &scenario,
            config(vec![FailureEvent {
                agent_id: AgentId::from("agent-0".to_owned()),
                at_tick: 2,
            }]),
        );

        assert!(metrics.success);
        assert_eq!(metrics.detection_time_ticks, Some(3));
        assert_eq!(metrics.reallocation_time_ticks, Some(0));
    }

    #[test]
    fn runner_failure_triggers_reallocation() {
        let scenario = scenario(1, 5, 8);
        let metrics = ScenarioRunner::run(
            &scenario,
            config(vec![FailureEvent {
                agent_id: AgentId::from("agent-0".to_owned()),
                at_tick: 2,
            }]),
        );

        assert!(metrics.success);
        assert!(metrics.all_tasks_assigned);
        assert!(metrics.max_task_unassigned_ticks <= 5);
    }

    #[test]
    fn runner_deterministic_same_seed() {
        let scenario = scenario(7, 5, 8);
        let config = config(vec![FailureEvent {
            agent_id: AgentId::from("agent-0".to_owned()),
            at_tick: 2,
        }]);

        let a = ScenarioRunner::run(&scenario, config.clone());
        let b = ScenarioRunner::run(&scenario, config);

        assert_eq!(a, b);
    }

    #[test]
    fn runner_no_failure_assigns_all_tasks() {
        let scenario = scenario(0, 5, 8);
        let metrics = ScenarioRunner::run(&scenario, config(Vec::new()));

        assert!(metrics.success);
        assert!(metrics.all_tasks_assigned);
    }

    #[test]
    fn runner_dynamic_task_appears_and_gets_assigned() {
        let s = scenario(0, 3, 0);
        let dynamic_task = Task {
            id: TaskId::from("dyn-0".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            preferred_role: None,
            expires_at: None,
            pose: None,
        };
        let cfg = RunConfig {
            dynamic_tasks: vec![DynamicTaskEvent {
                at_tick: 2,
                task: dynamic_task,
            }],
            ..config(vec![])
        };
        let metrics = ScenarioRunner::run(&s, cfg);
        assert!(metrics.all_tasks_assigned);
        assert_eq!(metrics.tasks_injected, 1);
    }

    #[test]
    fn runner_expired_task_counted_in_metrics() {
        let s = scenario(0, 3, 0);
        let expiring_task = Task {
            id: TaskId::from("exp-0".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![Capability::from("missing".to_owned())],
            preferred_role: None,
            expires_at: Some(3),
            pose: None,
        };
        let cfg = RunConfig {
            dynamic_tasks: vec![DynamicTaskEvent {
                at_tick: 1,
                task: expiring_task,
            }],
            ..config(vec![])
        };
        let metrics = ScenarioRunner::run(&s, cfg);
        assert_eq!(metrics.tasks_expired, 1);
    }

    #[test]
    fn runner_greedy_deterministic_with_capabilities() {
        let mut s = scenario(5, 4, 2);
        s.agents[0].capabilities = vec![Capability::from("optical".to_owned())];
        s.tasks[0].required_capabilities = vec![Capability::from("optical".to_owned())];

        let cfg = config(vec![]);
        let a = ScenarioRunner::run(&s, cfg.clone());
        let b = ScenarioRunner::run(&s, cfg);

        assert_eq!(a, b);
    }

    #[test]
    fn runner_auction_deterministic() {
        use swarm_alloc::AuctionAllocator;
        let s = scenario(9, 5, 4);
        let cfg = config(vec![]);

        let a = ScenarioRunner::run_with(&s, cfg.clone(), AuctionAllocator::default());
        let b = ScenarioRunner::run_with(&s, cfg, AuctionAllocator::default());

        assert_eq!(a, b);
    }

    #[test]
    fn runner_capability_gate_task_stays_unassigned() {
        let s = scenario(0, 3, 0);
        let impossible_task = Task {
            id: TaskId::from("imp-0".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![Capability::from("unobtainium".to_owned())],
            preferred_role: None,
            expires_at: None,
            pose: None,
        };
        let cfg = RunConfig {
            dynamic_tasks: vec![DynamicTaskEvent {
                at_tick: 1,
                task: impossible_task,
            }],
            ..config(vec![])
        };
        let metrics = ScenarioRunner::run(&s, cfg);
        assert!(!metrics.all_tasks_assigned);
    }

    #[test]
    fn runner_no_duplicate_ownership_invariant() {
        let s = scenario(0, 5, 5);
        let cfg = config(vec![]);
        ScenarioRunner::run(&s, cfg);
        // If there were duplicate ownership, task_registry.assign() would return Err,
        // which is counted as a conflict — not a panic. The test verifies the run completes.
    }

    /// Stub allocator that returns the same task_id twice to trigger conflict detection.
    struct DuplicateAllocator;

    impl Allocator for DuplicateAllocator {
        fn allocate(
            &self,
            tasks: &[AllocationTask<'_>],
            agents: &[AllocationAgent],
        ) -> Vec<(TaskId, AgentId)> {
            if tasks.is_empty() || agents.is_empty() {
                return vec![];
            }
            let task_id = tasks[0].task.id.clone();
            let agent_id = agents[0].id.clone();
            // Return same task_id twice — second is a conflict
            vec![(task_id.clone(), agent_id.clone()), (task_id, agent_id)]
        }
    }

    #[test]
    fn runner_conflict_counter_in_metrics() {
        let s = scenario(0, 2, 1);
        let cfg = config(vec![]);
        let metrics = ScenarioRunner::run_with(&s, cfg, DuplicateAllocator);
        assert!(metrics.conflicting_assignments > 0);
    }

    #[test]
    fn allocate_unassigned_counts_duplicate_allocator_output() {
        let s = scenario(0, 2, 1);
        let cfg = config(vec![]);
        let metrics = ScenarioRunner::run_with(&s, cfg, DuplicateAllocator);
        assert_eq!(metrics.conflicting_assignments, 1);
    }
}
