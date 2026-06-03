use super::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use swarm_alloc::GreedyAllocator;
use swarm_comms::{InMemAgentTransport, InMemNetwork, NetworkConfig, RawMessage, Transport};
use swarm_types::{Agent, AgentId, Capability, Health, Pose, Role, Task, TaskId, TaskStatus};

use crate::{Coordinator, RuntimeMessage};

fn agent_entry(id: &str) -> Agent {
    Agent {
        id: AgentId::from(id.to_owned()),
        role: Role::Scout,
        health: Health::Alive,
        pose: Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        },
        capabilities: vec![],
        current_task: None,
        battery: 100.0,
        comms_range: f64::INFINITY,
        generation: 1,
        speed: 0.0,
        max_range: 0.0,
        battery_drain_rate: 0.0,
        battery_model: None,
    }
}

fn task_entry(id: &str) -> Task {
    Task {
        id: TaskId::from(id.to_owned()),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority: 1,
        required_capabilities: vec![],
        required_role: None,
        preferred_role: None,
        expires_at: None,
        pose: None,
        grid_cell: None,
        edge_id: None,
        kind: None,
    }
}

fn task_owner(node: &AgentNode<InMemAgentTransport>, task_id: &str) -> Option<AgentId> {
    node.coordinator
        .registry
        .tasks()
        .find(|task| task.id == TaskId::from(task_id.to_owned()))
        .and_then(|task| task.assigned_to.clone())
}

fn assert_unique_task_ownership(node: &AgentNode<InMemAgentTransport>) {
    let mut seen = HashSet::new();
    for task in node.coordinator.registry.tasks() {
        if task.assigned_to.is_some() {
            assert!(seen.insert(task.id.clone()), "duplicate task {}", task.id);
        }
    }
}

fn make_network_config() -> NetworkConfig {
    NetworkConfig {
        packet_loss_rate: 0.0,
        latency_ticks: 0,
        latency_per_hop: 0,
        seed: 42,
        partitions: HashSet::new(),
        comms_jitter_ticks: 0,
    }
}

fn make_bus() -> Rc<RefCell<InMemNetwork>> {
    Rc::new(RefCell::new(InMemNetwork::new(make_network_config())))
}

fn make_hb_msg(from: &str, to: &str, tick: u64, gen: u64) -> RawMessage {
    RawMessage {
        from: AgentId::from(from.to_owned()),
        to: AgentId::from(to.to_owned()),
        payload: RuntimeMessage::heartbeat(tick, gen),
    }
}

#[test]
fn dispatch_heartbeat_updates_membership() {
    let bus = make_bus();
    let transport = InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));
    bus.borrow_mut().advance_tick();
    bus.borrow_mut()
        .send(make_hb_msg("agent-1", "agent-0", 5, 1))
        .unwrap();
    bus.borrow_mut().advance_tick();

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![AgentId::from("agent-1".to_owned())],
        Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        ),
        transport,
    );
    node.gossip_interval_ticks = 999;

    let mut allocator = GreedyAllocator::default();
    node.tick(1, &mut allocator, vec![]).unwrap();

    let entry = node
        .coordinator
        .membership
        .get(&AgentId::from("agent-1".to_owned()))
        .unwrap();
    assert_eq!(entry.last_heartbeat_tick, 5);
}

#[test]
fn dispatch_gossip_does_not_affect_heartbeat_senders() {
    let bus = make_bus();
    let transport = InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));

    let agent_0 = AgentId::from("agent-0".to_owned());
    let agent_1 = AgentId::from("agent-1".to_owned());
    bus.borrow_mut().advance_tick();

    // Send gossip (not heartbeat) from agent-1
    let gossip_payload = RuntimeMessage::gossip(HashMap::new(), {
        let mut m = HashMap::new();
        m.insert(agent_1.clone(), 1);
        m
    });
    bus.borrow_mut()
        .send(RawMessage {
            from: agent_1.clone(),
            to: agent_0.clone(),
            payload: gossip_payload,
        })
        .unwrap();
    bus.borrow_mut().advance_tick();

    let mut node = AgentNode::new(
        agent_0.clone(),
        vec![agent_1],
        Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        ),
        transport,
    );
    node.gossip_interval_ticks = 999;

    let mut allocator = GreedyAllocator::default();
    let out = node.tick(1, &mut allocator, vec![]).unwrap();

    // Gossip-only message should NOT count as heartbeat
    assert!(out.newly_failed.is_empty());
}

#[test]
fn dispatch_unknown_payload_is_discarded() {
    let bus = make_bus();
    let transport = InMemAgentTransport::new(bus.clone(), AgentId::from("agent-0".to_owned()));
    bus.borrow_mut().advance_tick();
    bus.borrow_mut()
        .send(RawMessage {
            from: AgentId::from("agent-X".to_owned()),
            to: AgentId::from("agent-0".to_owned()),
            payload: b"garbage".to_vec(),
        })
        .unwrap();
    bus.borrow_mut().advance_tick();

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![AgentId::from("agent-1".to_owned())],
        Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        ),
        transport,
    );
    node.gossip_interval_ticks = 999;

    let mut allocator = GreedyAllocator::default();
    let out = node.tick(1, &mut allocator, vec![]).unwrap();
    assert_eq!(out.discarded_messages, 1);
}

#[test]
fn gossip_merge_unassigned_task_from_remote() {
    let task = task_entry("task-0");
    let mut coord = Coordinator::new(
        vec![
            agent_entry("agent-0"),
            agent_entry("agent-1"),
            agent_entry("agent-2"),
        ],
        vec![task],
        5,
    );
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-0".to_owned()), 0, 1);
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![],
        coord,
        InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
    );

    let gossip = RuntimeMessage::Gossip {
        assignments: HashMap::from([(
            TaskId::from("task-0".to_owned()),
            AgentId::from("agent-1".to_owned()),
        )]),
        generations: HashMap::from([
            (AgentId::from("agent-0".to_owned()), 1),
            (AgentId::from("agent-1".to_owned()), 1),
        ]),
    };
    node.apply_gossip_buffer(&[gossip]);

    let t = node
        .coordinator
        .registry
        .tasks()
        .find(|t| t.id == TaskId::from("task-0".to_owned()))
        .unwrap();
    assert_eq!(t.assigned_to, Some(AgentId::from("agent-1".to_owned())));
}

#[test]
fn gossip_merge_higher_generation_overrides_local() {
    let mut task = task_entry("task-0");
    task.status = TaskStatus::Assigned;
    task.assigned_to = Some(AgentId::from("agent-1".to_owned()));

    let mut coord = Coordinator::new(
        vec![
            agent_entry("agent-0"),
            agent_entry("agent-1"),
            agent_entry("agent-2"),
        ],
        vec![task],
        5,
    );
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-2".to_owned()), 0, 3);

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![],
        coord,
        InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
    );

    let gossip = RuntimeMessage::Gossip {
        assignments: HashMap::from([(
            TaskId::from("task-0".to_owned()),
            AgentId::from("agent-2".to_owned()),
        )]),
        generations: HashMap::from([
            (AgentId::from("agent-1".to_owned()), 1),
            (AgentId::from("agent-2".to_owned()), 3),
        ]),
    };
    node.apply_gossip_buffer(&[gossip]);

    let t = node
        .coordinator
        .registry
        .tasks()
        .find(|t| t.id == TaskId::from("task-0".to_owned()))
        .unwrap();
    assert_eq!(t.assigned_to, Some(AgentId::from("agent-2".to_owned())));
}

#[test]
fn gossip_merge_equal_generation_max_agentid_wins() {
    let mut task = task_entry("task-0");
    task.status = TaskStatus::Assigned;
    task.assigned_to = Some(AgentId::from("agent-1".to_owned()));

    let mut coord = Coordinator::new(
        vec![
            agent_entry("agent-0"),
            agent_entry("agent-1"),
            agent_entry("agent-2"),
        ],
        vec![task],
        5,
    );
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-2".to_owned()), 0, 1);

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![],
        coord,
        InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
    );

    let gossip = RuntimeMessage::Gossip {
        assignments: HashMap::from([(
            TaskId::from("task-0".to_owned()),
            AgentId::from("agent-2".to_owned()),
        )]),
        generations: HashMap::from([
            (AgentId::from("agent-1".to_owned()), 1),
            (AgentId::from("agent-2".to_owned()), 1),
        ]),
    };
    node.apply_gossip_buffer(&[gossip]);

    // agent-2 > agent-1 lexicographically, so remote wins
    let t = node
        .coordinator
        .registry
        .tasks()
        .find(|t| t.id == TaskId::from("task-0".to_owned()))
        .unwrap();
    assert_eq!(t.assigned_to, Some(AgentId::from("agent-2".to_owned())));
}

#[test]
fn gossip_merge_lower_generation_is_ignored() {
    let mut task = task_entry("task-0");
    task.status = TaskStatus::Assigned;
    task.assigned_to = Some(AgentId::from("agent-2".to_owned()));

    let mut coord = Coordinator::new(
        vec![
            agent_entry("agent-0"),
            agent_entry("agent-1"),
            agent_entry("agent-2"),
        ],
        vec![task],
        5,
    );
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-2".to_owned()), 0, 3);

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![],
        coord,
        InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
    );

    let gossip = RuntimeMessage::Gossip {
        assignments: HashMap::from([(
            TaskId::from("task-0".to_owned()),
            AgentId::from("agent-1".to_owned()),
        )]),
        generations: HashMap::from([(AgentId::from("agent-1".to_owned()), 1)]),
    };
    node.apply_gossip_buffer(&[gossip]);

    let t = node
        .coordinator
        .registry
        .tasks()
        .find(|t| t.id == TaskId::from("task-0".to_owned()))
        .unwrap();
    // Local owner agent-2 has gen=3 > remote gen=1, so local wins
    assert_eq!(t.assigned_to, Some(AgentId::from("agent-2".to_owned())));
}

#[test]
fn gossip_merge_same_owner_no_op() {
    let mut task = task_entry("task-0");
    task.status = TaskStatus::Assigned;
    task.assigned_to = Some(AgentId::from("agent-1".to_owned()));

    let mut coord = Coordinator::new(
        vec![agent_entry("agent-0"), agent_entry("agent-1")],
        vec![task],
        5,
    );
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![],
        coord,
        InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
    );

    let gossip = RuntimeMessage::Gossip {
        assignments: HashMap::from([(
            TaskId::from("task-0".to_owned()),
            AgentId::from("agent-1".to_owned()),
        )]),
        generations: HashMap::from([(AgentId::from("agent-1".to_owned()), 1)]),
    };
    let (merged, _) = node.apply_gossip_buffer(&[gossip]);
    assert_eq!(merged, 0);
}

#[test]
fn gossip_merge_updates_membership_generations() {
    let coord = Coordinator::new(
        vec![agent_entry("agent-0"), agent_entry("agent-1")],
        vec![],
        5,
    );

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![],
        coord,
        InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
    );

    let gossip = RuntimeMessage::Gossip {
        assignments: HashMap::new(),
        generations: HashMap::from([
            (AgentId::from("agent-0".to_owned()), 1),
            (AgentId::from("agent-1".to_owned()), 5),
        ]),
    };
    node.apply_gossip_buffer(&[gossip]);

    let gen = node
        .coordinator
        .membership
        .generation_of(&AgentId::from("agent-1".to_owned()));
    assert_eq!(gen, 5);
}

#[test]
fn duplicate_assignment_returns_err_not_panics() {
    let task_id = TaskId::from("task-0".to_owned());
    let mut coord = Coordinator::new(
        vec![agent_entry("agent-0"), agent_entry("agent-1")],
        vec![task_entry("task-0")],
        5,
    );

    coord
        .registry
        .assign(&task_id, AgentId::from("agent-0".to_owned()))
        .unwrap();
    let result = coord
        .registry
        .assign(&task_id, AgentId::from("agent-1".to_owned()));
    assert!(result.is_err());
}

#[test]
fn reallocation_recovers_failed_agent_tasks_by_survivor() {
    let lost_agent = AgentId::from("agent-1".to_owned());
    let survivor = AgentId::from("agent-2".to_owned());
    let task_0 = TaskId::from("task-0".to_owned());
    let task_1 = TaskId::from("task-1".to_owned());
    let mut coord = Coordinator::new(
        vec![
            agent_entry("agent-0"),
            agent_entry("agent-1"),
            agent_entry("agent-2"),
        ],
        vec![
            task_entry("task-0"),
            task_entry("task-1"),
            task_entry("task-2"),
        ],
        3,
    );
    coord.registry.assign(&task_0, lost_agent.clone()).unwrap();
    coord.registry.assign(&task_1, lost_agent.clone()).unwrap();
    coord.membership.record_heartbeat(&survivor, 4, 1);

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![lost_agent.clone(), survivor],
        coord,
        InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
    );
    node.gossip_interval_ticks = 999;

    let mut allocator = GreedyAllocator::default();
    let out = node
        .process_inbox_and_allocate(4, &mut allocator, vec![])
        .unwrap();

    assert_eq!(out.newly_failed, vec![lost_agent.clone()]);
    assert_eq!(out.released_tasks.len(), 2);
    assert_eq!(out.reassignment_count, 2);
    assert_eq!(out.tasks_recovered, vec![task_0.clone(), task_1.clone()]);
    assert_eq!(out.reallocation_latency_ticks, Some(0));
    assert_eq!(out.failure_releases[0].failed_agent_id, lost_agent);
    assert!(task_owner(&node, "task-0").is_some());
    assert!(task_owner(&node, "task-1").is_some());
    assert_unique_task_ownership(&node);
}

#[test]
fn reallocation_unassignable_released_task_is_not_counted_as_recovered() {
    let lost_agent = AgentId::from("agent-1".to_owned());
    let task_id = TaskId::from("task-0".to_owned());
    let mut task = task_entry("task-0");
    task.required_capabilities = vec![Capability::from("thermal".to_owned())];
    let mut coord = Coordinator::new(
        vec![agent_entry("agent-0"), agent_entry("agent-1")],
        vec![task],
        3,
    );
    coord.registry.assign(&task_id, lost_agent.clone()).unwrap();

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![lost_agent],
        coord,
        InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
    );
    node.gossip_interval_ticks = 999;

    let mut allocator = GreedyAllocator::default();
    let out = node
        .process_inbox_and_allocate(4, &mut allocator, vec![])
        .unwrap();

    assert_eq!(out.released_tasks, vec![task_id.clone()]);
    assert!(out.tasks_recovered.is_empty());
    assert_eq!(out.reassignment_count, 0);
    assert_eq!(out.reallocation_latency_ticks, None);
    assert_eq!(task_owner(&node, "task-0"), None);
}

#[test]
fn reordered_gossip_messages_produce_same_result() {
    let mut task = task_entry("task-0");
    task.status = TaskStatus::Assigned;
    task.assigned_to = Some(AgentId::from("agent-1".to_owned()));

    let make_node = || {
        let mut coord = Coordinator::new(
            vec![
                agent_entry("agent-0"),
                agent_entry("agent-1"),
                agent_entry("agent-2"),
            ],
            vec![task.clone()],
            5,
        );
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);
        coord
            .membership
            .record_heartbeat(&AgentId::from("agent-2".to_owned()), 0, 3);
        AgentNode::new(
            AgentId::from("agent-0".to_owned()),
            vec![],
            coord,
            InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
        )
    };

    let g1 = RuntimeMessage::Gossip {
        assignments: HashMap::from([(
            TaskId::from("task-0".to_owned()),
            AgentId::from("agent-2".to_owned()),
        )]),
        generations: HashMap::from([
            (AgentId::from("agent-1".to_owned()), 1),
            (AgentId::from("agent-2".to_owned()), 3),
        ]),
    };
    let g2 = RuntimeMessage::Gossip {
        assignments: HashMap::new(),
        generations: HashMap::from([(AgentId::from("agent-2".to_owned()), 3)]),
    };

    let mut node_a = make_node();
    node_a.apply_gossip_buffer(&[g1.clone(), g2.clone()]);

    let mut node_b = make_node();
    node_b.apply_gossip_buffer(&[g2, g1]);

    let owner_a = node_a
        .coordinator
        .registry
        .tasks()
        .find(|t| t.id == TaskId::from("task-0".to_owned()))
        .unwrap()
        .assigned_to
        .clone();
    let owner_b = node_b
        .coordinator
        .registry
        .tasks()
        .find(|t| t.id == TaskId::from("task-0".to_owned()))
        .unwrap()
        .assigned_to
        .clone();
    assert_eq!(owner_a, owner_b);
}

#[test]
fn gossip_merge_ignores_dead_remote_owner() {
    let mut task = task_entry("task-0");
    task.status = TaskStatus::Unassigned;
    task.assigned_to = None;

    let mut coord = Coordinator::new(
        vec![agent_entry("agent-0"), agent_entry("agent-1")],
        vec![task],
        5,
    );
    coord
        .membership
        .mark_dead(&AgentId::from("agent-1".to_owned()));

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![],
        coord,
        InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
    );

    let gossip = RuntimeMessage::Gossip {
        assignments: HashMap::from([(
            TaskId::from("task-0".to_owned()),
            AgentId::from("agent-1".to_owned()),
        )]),
        generations: HashMap::from([(AgentId::from("agent-1".to_owned()), 1)]),
    };
    let (merged, stale) = node.apply_gossip_buffer(&[gossip]);
    assert_eq!(merged, 0);
    assert!(stale > 0);
}

#[test]
fn gossip_merge_preserves_unrelated_tasks() {
    let mut task0 = task_entry("task-0");
    task0.status = TaskStatus::Assigned;
    task0.assigned_to = Some(AgentId::from("agent-1".to_owned()));

    let mut task1 = task_entry("task-1");
    task1.status = TaskStatus::Assigned;
    task1.assigned_to = Some(AgentId::from("agent-1".to_owned()));

    let mut coord = Coordinator::new(
        vec![
            agent_entry("agent-0"),
            agent_entry("agent-1"),
            agent_entry("agent-2"),
        ],
        vec![task0, task1],
        5,
    );
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-1".to_owned()), 0, 1);
    coord
        .membership
        .record_heartbeat(&AgentId::from("agent-2".to_owned()), 0, 3);

    let mut node = AgentNode::new(
        AgentId::from("agent-0".to_owned()),
        vec![],
        coord,
        InMemAgentTransport::new(make_bus(), AgentId::from("agent-0".to_owned())),
    );

    // Gossip claims agent-2 (gen=3) owns task-0. Should override agent-1 (gen=1).
    // But task-1 should remain assigned to agent-1.
    let gossip = RuntimeMessage::Gossip {
        assignments: HashMap::from([(
            TaskId::from("task-0".to_owned()),
            AgentId::from("agent-2".to_owned()),
        )]),
        generations: HashMap::from([
            (AgentId::from("agent-1".to_owned()), 1),
            (AgentId::from("agent-2".to_owned()), 3),
        ]),
    };
    node.apply_gossip_buffer(&[gossip]);

    let t0 = node
        .coordinator
        .registry
        .tasks()
        .find(|t| t.id == TaskId::from("task-0".to_owned()))
        .unwrap();
    assert_eq!(t0.assigned_to, Some(AgentId::from("agent-2".to_owned())));

    let t1 = node
        .coordinator
        .registry
        .tasks()
        .find(|t| t.id == TaskId::from("task-1".to_owned()))
        .unwrap();
    assert_eq!(
        t1.assigned_to,
        Some(AgentId::from("agent-1".to_owned())),
        "unrelated task-1 should remain assigned to agent-1"
    );
}
