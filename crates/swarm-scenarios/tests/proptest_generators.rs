use proptest::prelude::*;
use swarm_types::{Agent, AgentId, Capability, Health, Pose, Role, Task, TaskId, TaskStatus};

/// Generate a valid Agent for property-based testing.
pub fn agent_strategy() -> impl Strategy<Value = Agent> {
    (
        any::<u8>(),
        any::<u8>(),
        10.0f64..100.0f64,
        1.0f64..50.0f64,
        prop::collection::vec(
            prop::sample::select(vec![
                Capability::from("thermal".to_owned()),
                Capability::from("optical".to_owned()),
            ]),
            0..3,
        ),
    )
        .prop_map(
            |(idx, role_idx, battery, comms_range, capabilities)| Agent {
                id: AgentId::from(format!("agent-{}", idx)),
                role: match role_idx % 5 {
                    0 => Role::Scout,
                    1 => Role::Relay,
                    2 => Role::Mapper,
                    3 => Role::Inspector,
                    _ => Role::Carrier,
                },
                health: Health::Alive,
                pose: Pose {
                    x: (idx as f64) * 10.0,
                    y: (idx as f64) * 5.0,
                },
                capabilities,
                current_task: None,
                battery,
                comms_range,
                generation: 1,
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
            },
        )
}

/// Generate a valid Task for property-based testing.
pub fn task_strategy() -> impl Strategy<Value = Task> {
    (any::<u8>(), any::<u8>(), 1u8..10u8).prop_map(|(idx, role_idx, priority)| Task {
        id: TaskId::from(format!("task-{}", idx)),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority,
        required_capabilities: vec![],
        required_role: if role_idx % 4 == 0 {
            Some(Role::Relay)
        } else {
            None
        },
        preferred_role: None,
        expires_at: None,
        grid_cell: None,
        edge_id: None,
        pose: Some(Pose {
            x: (idx as f64) * 8.0,
            y: (idx as f64) * 4.0,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    proptest! {
        #[test]
        fn generated_agent_has_positive_battery(agent in agent_strategy()) {
            prop_assert!(agent.battery > 0.0);
            prop_assert!(agent.comms_range > 0.0);
        }

        #[test]
        fn generated_task_has_valid_priority(task in task_strategy()) {
            prop_assert!(task.priority >= 1);
            prop_assert!(task.priority <= 10);
        }
    }
}
