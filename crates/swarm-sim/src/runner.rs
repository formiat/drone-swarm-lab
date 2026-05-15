use std::collections::{HashMap, HashSet};

use swarm_alloc::{Allocator, GreedyAllocator};
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
pub struct RunConfig {
    pub max_ticks: u64,
    pub timeout_ticks: u64,
    pub max_unassigned_ticks: u64,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub failures: Vec<FailureEvent>,
}

pub struct ScenarioRunner;

impl ScenarioRunner {
    pub fn run(scenario: &Scenario, config: RunConfig) -> RunMetrics {
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
        let allocator = GreedyAllocator;
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

        allocate_unassigned(&mut coordinator, &allocator);

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

            let output = coordinator.process_tick(heartbeat_senders, current_tick);

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

            if !output.released_tasks.is_empty() || !coordinator.registry.unassigned().is_empty() {
                allocate_unassigned(&mut coordinator, &allocator);
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
            if coordinator.registry.all_assigned_or_completed()
                && max_task_unassigned_ticks <= config.max_unassigned_ticks
                && all_failure_ticks_passed
                && all_expected_failures_detected
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
        }
    }
}

fn allocate_unassigned(coordinator: &mut Coordinator, allocator: &GreedyAllocator) {
    let tasks: Vec<Task> = coordinator
        .registry
        .unassigned()
        .into_iter()
        .cloned()
        .collect();
    let task_refs: Vec<_> = tasks.iter().collect();
    let agents: Vec<_> = coordinator
        .membership
        .alive_agents()
        .map(|(agent_id, _)| agent_id.clone())
        .collect();
    let agent_refs: Vec<_> = agents.iter().collect();

    for (task_id, agent_id) in allocator.allocate(&task_refs, &agent_refs) {
        let _ = coordinator.registry.assign(&task_id, agent_id);
    }
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
    use swarm_types::{Agent, Health, Pose, Role, Task, TaskStatus};

    fn scenario(seed: u64, agent_count: usize, task_count: usize) -> Scenario {
        let agents = (0..agent_count)
            .map(|index| Agent {
                id: AgentId::from(format!("agent-{index}")),
                role: Role::Scout,
                health: Health::Alive,
                pose: Pose { x: 0.0, y: 0.0 },
                capabilities: Vec::new(),
                current_task: None,
            })
            .collect();
        let tasks = (0..task_count)
            .map(|index| Task {
                id: TaskId::from(format!("task-{index}")),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: 1,
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
}
