use swarm_alloc::GreedyAllocator;
use swarm_scenarios::{build_partition_scenario, PartitionConfig};
use swarm_sim::ScenarioRunner;
use swarm_types::{Agent, AgentId, Capability, Health, Pose, Role, Task, TaskId, TaskStatus};

fn main() {
    let agent_ids: Vec<AgentId> = (0..6)
        .map(|i| AgentId::from(format!("agent-{i}")))
        .collect();

    let agents: Vec<Agent> = agent_ids
        .iter()
        .map(|id| Agent {
            id: id.clone(),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose { x: 0.0, y: 0.0 },
            capabilities: vec![Capability::from("basic".to_owned())],
            current_task: None,
            battery: 100.0,
            comms_range: f64::INFINITY,
            generation: 1,
            speed: 5.0,
            max_range: 500.0,
            battery_drain_rate: 0.2,
        })
        .collect();

    let tasks: Vec<Task> = (0..8)
        .map(|i| Task {
            id: TaskId::from(format!("task-{i}")),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![Capability::from("basic".to_owned())],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
            grid_cell: None,
        })
        .collect();

    let config = PartitionConfig {
        seed: 42,
        agents,
        tasks,
        timeout_ticks: 5,
        max_ticks: 120,
        gossip_interval_ticks: 3,
        partition_start_tick: 10,
        partition_heal_tick: 30,
        group_a: agent_ids[0..3].to_vec(),
        group_b: agent_ids[3..6].to_vec(),
    };

    let (scenario, run_config) = build_partition_scenario(&config);
    let metrics = ScenarioRunner::run_with(&scenario, run_config, GreedyAllocator);

    assert!(
        metrics.success,
        "scenario should succeed: all tasks assigned after heal"
    );
    assert!(
        metrics.partitions_active,
        "partition should have been active"
    );
    assert!(
        metrics.max_view_divergence > 0,
        "views should diverge during partition"
    );
    assert!(
        metrics.convergence_ticks.is_some(),
        "maps should converge after heal"
    );

    println!("PASS: partition scenario converged");
    println!("  partitions_active: {}", metrics.partitions_active);
    println!("  partition_events: {}", metrics.partition_events);
    println!("  max_view_divergence: {}", metrics.max_view_divergence);
    println!("  convergence_ticks: {:?}", metrics.convergence_ticks);
    println!("  stale_discarded: {}", metrics.stale_messages_discarded);
    println!("  success: {}", metrics.success);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_process_partition_scenario_converges() {
        let agent_ids: Vec<AgentId> = (0..6)
            .map(|i| AgentId::from(format!("agent-{i}")))
            .collect();

        let agents: Vec<Agent> = agent_ids
            .iter()
            .map(|id| Agent {
                id: id.clone(),
                role: Role::Scout,
                health: Health::Alive,
                pose: Pose { x: 0.0, y: 0.0 },
                capabilities: vec![Capability::from("basic".to_owned())],
                current_task: None,
                battery: 100.0,
                comms_range: f64::INFINITY,
                generation: 1,
                speed: 5.0,
                max_range: 500.0,
                battery_drain_rate: 0.2,
            })
            .collect();

        let tasks: Vec<Task> = (0..8)
            .map(|i| Task {
                id: TaskId::from(format!("task-{i}")),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: 1,
                required_capabilities: vec![Capability::from("basic".to_owned())],
                required_role: None,
                preferred_role: None,
                expires_at: None,
                grid_cell: None,
                pose: None,
            })
            .collect();

        let config = PartitionConfig {
            seed: 42,
            agents,
            tasks,
            timeout_ticks: 5,
            max_ticks: 120,
            gossip_interval_ticks: 3,
            partition_start_tick: 10,
            partition_heal_tick: 30,
            group_a: agent_ids[0..3].to_vec(),
            group_b: agent_ids[3..6].to_vec(),
        };

        let (scenario, run_config) = build_partition_scenario(&config);
        let metrics = ScenarioRunner::run_with(&scenario, run_config, GreedyAllocator);

        assert!(metrics.success);
        assert!(metrics.partitions_active);
        assert!(metrics.max_view_divergence > 0);
        assert!(metrics.convergence_ticks.is_some());
    }
}
