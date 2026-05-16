use std::collections::HashSet;

use swarm_alloc::{AllocationAgent, AllocationTask, Allocator};
use swarm_comms::{RawMessage, Transport};
use swarm_types::{AgentId, Task, TaskId};

use crate::Coordinator;

pub struct NodeTickOutput {
    pub newly_failed: Vec<AgentId>,
    pub released_tasks: Vec<TaskId>,
    pub expired_task_ids: Vec<TaskId>,
    pub conflicting_assignments: u64,
}

pub struct AgentNode<T> {
    pub coordinator: Coordinator,
    pub transport: T,
    pub own_id: AgentId,
    pub peer_ids: Vec<AgentId>,
}

impl<T: Transport> AgentNode<T> {
    pub fn new(
        own_id: AgentId,
        peer_ids: Vec<AgentId>,
        coordinator: Coordinator,
        transport: T,
    ) -> Self {
        Self {
            coordinator,
            transport,
            own_id,
            peer_ids,
        }
    }

    pub fn tick<A: Allocator>(
        &mut self,
        current_tick: u64,
        allocator: &A,
        injected: Vec<Task>,
    ) -> Result<NodeTickOutput, T::Error> {
        self.send_heartbeats()?;
        self.process_inbox_and_allocate(current_tick, allocator, injected)
    }

    pub fn send_heartbeats(&mut self) -> Result<(), T::Error> {
        let hb = RawMessage {
            from: self.own_id.clone(),
            to: AgentId::from("placeholder".to_owned()),
            payload: b"hb".to_vec(),
        };

        for peer_id in &self.peer_ids {
            let mut msg = hb.clone();
            msg.to = peer_id.clone();
            self.transport.send(msg)?;
        }
        Ok(())
    }

    pub fn process_inbox_and_allocate<A: Allocator>(
        &mut self,
        current_tick: u64,
        allocator: &A,
        injected: Vec<Task>,
    ) -> Result<NodeTickOutput, T::Error> {
        let mut heartbeat_senders = vec![self.own_id.clone()];

        loop {
            match self.transport.poll() {
                Ok(Some(msg)) => heartbeat_senders.push(msg.from),
                Ok(None) => break,
                Err(_) => break,
            }
        }

        let output = self
            .coordinator
            .process_tick(heartbeat_senders, current_tick, injected);

        let mut conflicting_assignments: u64 = 0;

        if !output.released_tasks.is_empty()
            || !output.expired_task_ids.is_empty()
            || !self.coordinator.registry.unassigned().is_empty()
        {
            conflicting_assignments += allocate_unassigned(&mut self.coordinator, allocator);
        }

        Ok(NodeTickOutput {
            newly_failed: output.newly_failed,
            released_tasks: output.released_tasks,
            expired_task_ids: output.expired_task_ids,
            conflicting_assignments,
        })
    }
}

fn allocate_unassigned<A: Allocator>(coordinator: &mut Coordinator, allocator: &A) -> u64 {
    let mut tasks: Vec<Task> = coordinator
        .registry
        .unassigned()
        .into_iter()
        .cloned()
        .collect();
    tasks.sort_by(|a, b| a.id.as_ref().cmp(b.id.as_ref()));
    let allocation_tasks: Vec<AllocationTask<'_>> =
        tasks.iter().map(|task| AllocationTask { task }).collect();

    let mut agents: Vec<AllocationAgent> = coordinator
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
    agents.sort_by(|a, b| a.id.as_ref().cmp(b.id.as_ref()));

    let decisions = allocator.allocate(&allocation_tasks, &agents);

    let mut seen = HashSet::new();
    let mut conflicts: u64 = 0;
    for (task_id, agent_id) in decisions {
        if !seen.insert(task_id.clone()) {
            conflicts += 1;
            continue;
        }
        if coordinator.registry.assign(&task_id, agent_id).is_err() {
            conflicts += 1;
        }
    }
    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use swarm_alloc::GreedyAllocator;
    use swarm_comms::{InMemAgentTransport, InMemNetwork, NetworkConfig};
    use swarm_types::{Agent, Health, Pose, Role, TaskStatus};

    fn agent_entry(id: &str) -> Agent {
        Agent {
            id: AgentId::from(id.to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose { x: 0.0, y: 0.0 },
            capabilities: vec![],
            current_task: None,
            battery: 100.0,
        }
    }

    fn task_entry(id: &str) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            preferred_role: None,
            expires_at: None,
            pose: None,
        }
    }

    fn make_bus() -> Rc<RefCell<InMemNetwork>> {
        Rc::new(RefCell::new(InMemNetwork::new(NetworkConfig {
            packet_loss_rate: 0.0,
            latency_ticks: 0,
            seed: 42,
        })))
    }

    #[test]
    fn node_tick_sends_heartbeats_to_peers() {
        let bus = make_bus();
        let transport = InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));

        let coordinator = Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        );
        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![AgentId::from("agent-1".to_owned())],
            coordinator,
            transport,
        );

        let allocator = GreedyAllocator;
        node.tick(1, &allocator, vec![]).unwrap();

        let msgs = bus
            .borrow_mut()
            .drain_ready(&AgentId::from("agent-1".to_owned()));
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].from, AgentId::from("agent-0".to_owned()));
    }

    #[test]
    fn node_tick_self_heartbeat_keeps_own_agent_alive() {
        let bus = make_bus();
        let transport = InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));

        let coordinator = Coordinator::new(vec![agent_entry("agent-0")], vec![], 3);
        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![],
            coordinator,
            transport,
        );

        let allocator = GreedyAllocator;
        for tick in 1..=6 {
            let output = node.tick(tick, &allocator, vec![]).unwrap();
            assert!(
                !output
                    .newly_failed
                    .contains(&AgentId::from("agent-0".to_owned())),
                "own agent should never be detected as failed (self-heartbeat)"
            );
        }
    }

    #[test]
    fn node_tick_detects_failure() {
        let bus = make_bus();
        let transport = InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));

        let coordinator = Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            3,
        );
        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![AgentId::from("agent-1".to_owned())],
            coordinator,
            transport,
        );

        let allocator = GreedyAllocator;
        let mut agent_1_failed = false;
        for tick in 1..=6 {
            let output = node.tick(tick, &allocator, vec![]).unwrap();
            if output
                .newly_failed
                .contains(&AgentId::from("agent-1".to_owned()))
            {
                agent_1_failed = true;
            }
        }
        assert!(
            agent_1_failed,
            "agent-1 should be detected as failed after timeout"
        );
    }

    #[test]
    fn node_tick_reallocates_after_failure() {
        let bus = make_bus();
        let transport = InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));

        let mut task = task_entry("task-0");
        task.assigned_to = Some(AgentId::from("agent-1".to_owned()));
        task.status = TaskStatus::Assigned;

        let coordinator = Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![task],
            3,
        );
        let mut node = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![AgentId::from("agent-1".to_owned())],
            coordinator,
            transport,
        );

        let allocator = GreedyAllocator;
        let mut reallocated = false;
        for tick in 1..=10 {
            let output = node.tick(tick, &allocator, vec![]).unwrap();
            if output
                .released_tasks
                .contains(&TaskId::from("task-0".to_owned()))
            {
                continue;
            }
            let assigned = node.coordinator.registry.tasks().any(|t| {
                t.id == TaskId::from("task-0".to_owned())
                    && t.assigned_to == Some(AgentId::from("agent-0".to_owned()))
            });
            if assigned {
                reallocated = true;
                break;
            }
        }
        assert!(
            reallocated,
            "task-0 should be reallocated to agent-0 after agent-1 failure"
        );
    }

    #[test]
    fn node_tick_same_output_inmem_vs_stub_transport() {
        use std::convert::Infallible;

        struct StubTransport {
            messages: Vec<RawMessage>,
            poll_idx: usize,
        }

        impl Transport for StubTransport {
            type Error = Infallible;

            fn send(&mut self, _msg: RawMessage) -> Result<(), Self::Error> {
                Ok(())
            }

            fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
                if self.poll_idx < self.messages.len() {
                    let msg = self.messages[self.poll_idx].clone();
                    self.poll_idx += 1;
                    Ok(Some(msg))
                } else {
                    Ok(None)
                }
            }
        }

        let coordinator_a = Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        );
        let coordinator_b = Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        );

        let bus = make_bus();
        let transport_a =
            InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));
        let mut node_a = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![AgentId::from("agent-1".to_owned())],
            coordinator_a,
            transport_a,
        );

        let stub_messages = vec![RawMessage {
            from: AgentId::from("agent-1".to_owned()),
            to: AgentId::from("agent-0".to_owned()),
            payload: b"hb".to_vec(),
        }];
        let transport_b = StubTransport {
            messages: stub_messages,
            poll_idx: 0,
        };
        let mut node_b = AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![AgentId::from("agent-1".to_owned())],
            coordinator_b,
            transport_b,
        );

        bus.borrow_mut().advance_tick();
        let allocator = GreedyAllocator;

        bus.borrow_mut()
            .send(RawMessage {
                from: AgentId::from("agent-1".to_owned()),
                to: AgentId::from("agent-0".to_owned()),
                payload: b"hb".to_vec(),
            })
            .unwrap();
        bus.borrow_mut().advance_tick();

        let out_a = node_a.tick(1, &allocator, vec![]).unwrap();
        let out_b = node_b.tick(1, &allocator, vec![]).unwrap();

        assert_eq!(out_a.newly_failed, out_b.newly_failed);
        assert_eq!(out_a.released_tasks, out_b.released_tasks);
        assert_eq!(out_a.expired_task_ids, out_b.expired_task_ids);
        assert_eq!(out_a.conflicting_assignments, out_b.conflicting_assignments);
    }

    #[test]
    fn deterministic_allocation_independent_of_insertion_order() {
        let allocator = GreedyAllocator;

        let agents_ordered: Vec<Agent> = vec![
            agent_entry("agent-0"),
            agent_entry("agent-1"),
            agent_entry("agent-2"),
        ];
        let agents_reversed: Vec<Agent> = vec![
            agent_entry("agent-2"),
            agent_entry("agent-1"),
            agent_entry("agent-0"),
        ];

        let tasks_ordered: Vec<Task> = vec![
            task_entry("task-0"),
            task_entry("task-1"),
            task_entry("task-2"),
            task_entry("task-3"),
            task_entry("task-4"),
        ];
        let tasks_reversed: Vec<Task> = vec![
            task_entry("task-4"),
            task_entry("task-3"),
            task_entry("task-2"),
            task_entry("task-1"),
            task_entry("task-0"),
        ];

        let mut coord_a = Coordinator::new(agents_ordered, tasks_ordered, 5);
        let mut coord_b = Coordinator::new(agents_reversed, tasks_reversed, 5);

        // Apply identical heartbeats
        let all_ids: Vec<AgentId> = vec![
            AgentId::from("agent-0".to_owned()),
            AgentId::from("agent-1".to_owned()),
            AgentId::from("agent-2".to_owned()),
        ];
        coord_a.process_tick(all_ids.clone(), 0, vec![]);
        coord_b.process_tick(all_ids, 0, vec![]);

        // Allocate with different HashMap internal order
        allocate_unassigned(&mut coord_a, &allocator);
        allocate_unassigned(&mut coord_b, &allocator);

        let map_a: HashMap<TaskId, AgentId> = coord_a
            .registry
            .tasks()
            .filter_map(|t| t.assigned_to.clone().map(|a| (t.id.clone(), a)))
            .collect();
        let map_b: HashMap<TaskId, AgentId> = coord_b
            .registry
            .tasks()
            .filter_map(|t| t.assigned_to.clone().map(|a| (t.id.clone(), a)))
            .collect();

        assert_eq!(
            map_a, map_b,
            "allocation must be deterministic regardless of HashMap insertion order"
        );
    }
}
