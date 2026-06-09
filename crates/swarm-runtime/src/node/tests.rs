use super::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use swarm_alloc::GreedyAllocator;
use swarm_comms::{
    AgentMissionState, InMemAgentTransport, InMemNetwork, LeaseId, NetworkConfig, RawMessage,
    Transport,
};
use swarm_types::{Agent, AgentId, Capability, Health, Pose, Role, Task, TaskId, TaskStatus};

use crate::autonomy::{
    AgentAutonomyConfig, GcsLostPolicy, MothershipLostPolicy, NeighborLostPolicy,
};
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

// ── M93: Agent Autonomy FSM tests ────────────────────────────────────────────

fn make_gcs_node(
    own: &str,
    gcs: &str,
    bus: Rc<RefCell<InMemNetwork>>,
    policy: GcsLostPolicy,
    heartbeat_timeout: u64,
    peers: Vec<&str>,
) -> AgentNode<InMemAgentTransport> {
    let own_id = AgentId::from(own.to_owned());
    let peer_ids: Vec<AgentId> = peers
        .iter()
        .map(|p| AgentId::from((*p).to_owned()))
        .collect();
    let all_agents: Vec<_> = std::iter::once(own)
        .chain(peers.iter().copied())
        .map(agent_entry)
        .collect();
    let transport = InMemAgentTransport::new(bus, own_id.clone());
    let mut node = AgentNode::new(
        own_id,
        peer_ids,
        Coordinator::new(all_agents, vec![], 5),
        transport,
    );
    node.gossip_interval_ticks = 999;
    node.gcs_id = Some(AgentId::from(gcs.to_owned()));
    node.autonomy = AgentAutonomyConfig {
        gcs_lost_policy: policy,
        gcs_heartbeat_timeout_ticks: heartbeat_timeout,
        ..AgentAutonomyConfig::default()
    };
    node
}

fn inject_gcs_hb(bus: &Rc<RefCell<InMemNetwork>>, gcs: &str, agent: &str, tick: u64) {
    bus.borrow_mut().advance_tick();
    bus.borrow_mut()
        .send(RawMessage {
            from: AgentId::from(gcs.to_owned()),
            to: AgentId::from(agent.to_owned()),
            payload: RuntimeMessage::heartbeat(tick, 1),
        })
        .unwrap();
}

fn skip_tick(bus: &Rc<RefCell<InMemNetwork>>) {
    bus.borrow_mut().advance_tick();
}

#[test]
fn gcs_lost_rtl_engages_after_threshold() {
    let bus = make_bus();
    let mut node = make_gcs_node(
        "agent-0",
        "gcs",
        bus.clone(),
        GcsLostPolicy::ReturnToLaunch { after_ticks: 3 },
        3,
        vec![],
    );
    let mut allocator = GreedyAllocator::default();

    // Ticks 1-2: GCS heartbeat present
    for tick in 1u64..=2 {
        inject_gcs_hb(&bus, "gcs", "agent-0", tick);
        let out = node
            .process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
        assert!(!out.gcs_lost_this_tick, "should not fire at tick {tick}");
    }

    // Ticks 3-4: no GCS HB — ticks_since = 1, 2 < 3
    for tick in 3u64..=4 {
        skip_tick(&bus);
        let out = node
            .process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
        assert!(
            !out.gcs_lost_this_tick,
            "should not fire before threshold at tick {tick}"
        );
    }

    // Tick 5: ticks_since = 5 - 2 = 3 >= 3 → fires
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(5, &mut allocator, vec![])
        .unwrap();
    assert!(out.gcs_lost_this_tick, "RTL should engage at tick 5");
    assert_eq!(
        out.gcs_lost_policy_name.as_deref(),
        Some("return_to_launch")
    );
}

#[test]
fn gcs_lost_continue_does_not_abort_before_threshold() {
    let bus = make_bus();
    let mut node = make_gcs_node(
        "agent-0",
        "gcs",
        bus.clone(),
        GcsLostPolicy::HoverInPlace {
            max_gcs_lost_ticks: 10,
        },
        10,
        vec![],
    );
    let mut allocator = GreedyAllocator::default();

    // Tick 1: GCS heartbeat
    inject_gcs_hb(&bus, "gcs", "agent-0", 1);
    node.process_inbox_and_allocate(1, &mut allocator, vec![])
        .unwrap();

    // Ticks 2-9: no GCS HB — ticks_since = 1..8 < 10
    for tick in 2u64..=9 {
        skip_tick(&bus);
        let out = node
            .process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
        assert!(!out.gcs_lost_this_tick, "should not fire at tick {tick}");
    }
}

#[test]
fn gcs_lost_abort_immediate_triggers_on_first_tick() {
    let bus = make_bus();
    let mut node = make_gcs_node(
        "agent-0",
        "gcs",
        bus.clone(),
        GcsLostPolicy::AbortImmediate,
        1,
        vec![],
    );
    let mut allocator = GreedyAllocator::default();

    // Tick 1: GCS HB present → no loss
    inject_gcs_hb(&bus, "gcs", "agent-0", 1);
    let out = node
        .process_inbox_and_allocate(1, &mut allocator, vec![])
        .unwrap();
    assert!(!out.gcs_lost_this_tick);

    // Tick 2: no GCS HB → ticks_since = 1 >= 1 → fires
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(2, &mut allocator, vec![])
        .unwrap();
    assert!(out.gcs_lost_this_tick);
    assert_eq!(out.gcs_lost_policy_name.as_deref(), Some("abort_immediate"));
}

#[test]
fn continuing_under_lease_stays_active_while_lease_valid() {
    let bus = make_bus();
    let mut node = make_gcs_node(
        "agent-0",
        "gcs",
        bus.clone(),
        GcsLostPolicy::ReturnToLaunch { after_ticks: 3 },
        3,
        vec![],
    );
    // Active lease that expires at tick 100
    node.active_leases
        .push(active_lease_record("lease-1", "lease-1", 0, 100));
    let mut allocator = GreedyAllocator::default();

    // Ticks 1-2: GCS HB
    for tick in 1u64..=2 {
        inject_gcs_hb(&bus, "gcs", "agent-0", tick);
        node.process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
    }

    // Ticks 3-4: no GCS HB, threshold not yet reached
    for tick in 3u64..=4 {
        skip_tick(&bus);
        let out = node
            .process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
        assert!(!out.gcs_lost_this_tick);
        assert!(out.continuing_under_lease_this_tick.is_none());
    }

    // Tick 5: ticks_since = 3 >= 3, but lease is valid → ContinuingUnderLease
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(5, &mut allocator, vec![])
        .unwrap();
    assert!(
        !out.gcs_lost_this_tick,
        "should NOT enter GcsLost while lease valid"
    );
    assert!(
        out.continuing_under_lease_this_tick.is_some(),
        "should emit ContinuingUnderLease event"
    );
    assert!(matches!(
        node.mission_state,
        AgentMissionState::ContinuingUnderLease { .. }
    ));

    // Ticks 6-9: still in ContinuingUnderLease, lease still valid
    for tick in 6u64..=9 {
        skip_tick(&bus);
        let out = node
            .process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
        assert!(
            !out.gcs_lost_this_tick,
            "should stay in ContinuingUnderLease at tick {tick}"
        );
        assert!(
            matches!(
                node.mission_state,
                AgentMissionState::ContinuingUnderLease { .. }
            ),
            "mission state should remain ContinuingUnderLease at tick {tick}"
        );
    }
}

#[test]
fn lease_expiry_during_gcs_loss_applies_policy() {
    let bus = make_bus();
    let mut node = make_gcs_node(
        "agent-0",
        "gcs",
        bus.clone(),
        GcsLostPolicy::ReturnToLaunch { after_ticks: 3 },
        3,
        vec![],
    );
    // Lease expires at tick 8
    node.active_leases
        .push(active_lease_record("lease-short", "lease-short", 0, 8));
    let mut allocator = GreedyAllocator::default();

    // Ticks 1-2: GCS HB
    for tick in 1u64..=2 {
        inject_gcs_hb(&bus, "gcs", "agent-0", tick);
        node.process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
    }

    // Tick 5: ContinuingUnderLease (lease valid, 5 < 8)
    for tick in 3u64..=5 {
        skip_tick(&bus);
        node.process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
    }
    assert!(matches!(
        node.mission_state,
        AgentMissionState::ContinuingUnderLease { .. }
    ));

    // Ticks 6-7: lease still valid (6 < 8, 7 < 8)
    for tick in 6u64..=7 {
        skip_tick(&bus);
        let out = node
            .process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
        assert!(
            out.lease_expired_in_gcs_loss.is_none(),
            "lease still valid at tick {tick}"
        );
    }

    // Tick 8: lease expiry — 8 < 8 = false → triggers transition
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(8, &mut allocator, vec![])
        .unwrap();
    assert!(
        out.lease_expired_in_gcs_loss.is_some(),
        "lease expiry event should fire at tick 8"
    );
    let (lid, policy) = out.lease_expired_in_gcs_loss.unwrap();
    assert_eq!(lid, "lease-short");
    assert_eq!(policy, "return_to_launch");
    assert!(matches!(
        node.mission_state,
        AgentMissionState::GcsLost { .. }
    ));
}

#[test]
fn gcs_reconnect_emits_state_reconcile_report() {
    let bus = make_bus();
    let mut node = make_gcs_node(
        "agent-0",
        "gcs",
        bus.clone(),
        GcsLostPolicy::ReturnToLaunch { after_ticks: 3 },
        3,
        vec![],
    );
    let mut allocator = GreedyAllocator::default();

    // Ticks 1-2: GCS HB
    for tick in 1u64..=2 {
        inject_gcs_hb(&bus, "gcs", "agent-0", tick);
        node.process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
    }

    // Tick 5: GCS lost (ticks_since = 3 >= 3)
    for tick in 3u64..=5 {
        skip_tick(&bus);
        node.process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
    }
    assert!(matches!(
        node.mission_state,
        AgentMissionState::GcsLost { .. }
    ));

    // Tick 6: GCS reconnects → should emit reconcile report
    inject_gcs_hb(&bus, "gcs", "agent-0", 6);
    let out = node
        .process_inbox_and_allocate(6, &mut allocator, vec![])
        .unwrap();
    assert!(
        out.gcs_reconnected_this_tick,
        "GCS should be detected as reconnected"
    );
    assert!(
        out.reconcile_report.is_some(),
        "reconcile report must be present"
    );
    let report = out.reconcile_report.unwrap();
    assert_eq!(*report.agent_id, "agent-0");
    assert!(
        report.gcs_lost_ticks > 0,
        "gcs_lost_ticks should be non-zero"
    );
    assert_eq!(report.policy_applied, "return_to_launch");
    assert!(matches!(node.mission_state, AgentMissionState::Idle));
}

#[test]
fn state_reconcile_report_contains_active_leases() {
    let bus = make_bus();
    let mut node = make_gcs_node(
        "agent-0",
        "gcs",
        bus.clone(),
        GcsLostPolicy::ReturnToLaunch { after_ticks: 3 },
        3,
        vec![],
    );
    // Active lease that outlasts the GCS loss period
    node.active_leases
        .push(active_lease_record("lease-active", "lease-active", 0, 100));
    let mut allocator = GreedyAllocator::default();

    // Ticks 1-2: GCS HB
    for tick in 1u64..=2 {
        inject_gcs_hb(&bus, "gcs", "agent-0", tick);
        node.process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
    }

    // Ticks 3-5: GCS lost; lease still valid so enters ContinuingUnderLease
    for tick in 3u64..=5 {
        skip_tick(&bus);
        node.process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
    }

    // Tick 6: GCS reconnects
    inject_gcs_hb(&bus, "gcs", "agent-0", 6);
    let out = node
        .process_inbox_and_allocate(6, &mut allocator, vec![])
        .unwrap();
    assert!(out.gcs_reconnected_this_tick);
    let report = out.reconcile_report.expect("reconcile report must exist");
    assert_eq!(
        report.active_leases_at_reconnect.len(),
        1,
        "report should list the active lease"
    );
    assert_eq!(
        *report.active_leases_at_reconnect[0].lease_id,
        "lease-active"
    );
}

#[test]
fn neighbor_lost_detected_after_timeout() {
    let bus = make_bus();
    let own_id = AgentId::from("agent-0".to_owned());
    let peer_id = AgentId::from("agent-1".to_owned());
    let transport = InMemAgentTransport::new(bus.clone(), own_id.clone());
    let mut node = AgentNode::new(
        own_id,
        vec![peer_id.clone()],
        Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        ),
        transport,
    );
    node.gossip_interval_ticks = 999;
    node.autonomy = AgentAutonomyConfig {
        peer_heartbeat_timeout_ticks: 3,
        ..AgentAutonomyConfig::default()
    };
    let mut allocator = GreedyAllocator::default();

    // Ticks 1-2: ticks_since_peer = current_tick (no HB ever) = 1, 2 < 3
    for tick in 1u64..=2 {
        skip_tick(&bus);
        let out = node
            .process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
        assert!(
            out.neighbors_lost_this_tick.is_empty(),
            "should not detect neighbor loss at tick {tick}"
        );
    }

    // Tick 3: ticks_since_peer = 3 >= 3 → detection
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(3, &mut allocator, vec![])
        .unwrap();
    assert_eq!(
        out.neighbors_lost_this_tick,
        vec![peer_id.clone()],
        "neighbor loss should fire at tick 3"
    );

    // Tick 4: already in lost_peers_detected → no re-fire
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(4, &mut allocator, vec![])
        .unwrap();
    assert!(
        out.neighbors_lost_this_tick.is_empty(),
        "neighbor loss should not fire twice"
    );
}

#[test]
fn mothership_lost_wait_at_staging_aborts_after_timeout() {
    let bus = make_bus();
    let own_id = AgentId::from("agent-0".to_owned());
    let ms_id = AgentId::from("mothership".to_owned());
    let transport = InMemAgentTransport::new(bus.clone(), own_id.clone());
    let mut node = AgentNode::new(
        own_id,
        vec![],
        Coordinator::new(vec![agent_entry("agent-0")], vec![], 5),
        transport,
    );
    node.gossip_interval_ticks = 999;
    node.mothership_id = Some(ms_id.clone());
    node.autonomy = AgentAutonomyConfig {
        mothership_lost_policy: MothershipLostPolicy::WaitAtStaging { max_ticks: 4 },
        ..AgentAutonomyConfig::default()
    };
    let mut allocator = GreedyAllocator::default();

    // Tick 1: mothership HB present → no loss
    bus.borrow_mut().advance_tick();
    bus.borrow_mut()
        .send(RawMessage {
            from: ms_id.clone(),
            to: AgentId::from("agent-0".to_owned()),
            payload: RuntimeMessage::heartbeat(1, 1),
        })
        .unwrap();
    let out = node
        .process_inbox_and_allocate(1, &mut allocator, vec![])
        .unwrap();
    assert!(!out.mothership_lost_this_tick, "no loss at tick 1");

    // Ticks 2-4: no mothership HB, ticks_since = 1..3 < 4
    for tick in 2u64..=4 {
        skip_tick(&bus);
        let out = node
            .process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
        assert!(
            !out.mothership_lost_this_tick,
            "should not fire before max_ticks at tick {tick}"
        );
        assert!(
            !matches!(node.mission_state, AgentMissionState::Aborting { .. }),
            "mission state should not be Aborting at tick {tick}"
        );
    }

    // Tick 5: ticks_since = 4 >= 4 → WaitAtStaging limit reached → Aborting
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(5, &mut allocator, vec![])
        .unwrap();
    assert!(
        out.mothership_lost_this_tick,
        "mothership_lost should fire at tick 5"
    );
    assert!(
        matches!(
            node.mission_state,
            AgentMissionState::Aborting { ref reason } if reason == "mothership_lost"
        ),
        "mission state should be Aborting(mothership_lost)"
    );

    // Tick 6: already Aborting → no re-fire
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(6, &mut allocator, vec![])
        .unwrap();
    assert!(
        !out.mothership_lost_this_tick,
        "mothership_lost should not fire twice"
    );
}

#[test]
fn gcs_reconnect_restores_pre_loss_mission_state() {
    let bus = make_bus();
    let mut node = make_gcs_node(
        "agent-0",
        "gcs",
        bus.clone(),
        GcsLostPolicy::ReturnToLaunch { after_ticks: 3 },
        3,
        vec![],
    );
    node.mission_state = AgentMissionState::WaitingForMission;
    let mut allocator = GreedyAllocator::default();

    for tick in 1u64..=2 {
        inject_gcs_hb(&bus, "gcs", "agent-0", tick);
        node.process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
    }

    for tick in 3u64..=5 {
        skip_tick(&bus);
        node.process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
    }
    assert!(matches!(
        node.mission_state,
        AgentMissionState::GcsLost { .. }
    ));

    inject_gcs_hb(&bus, "gcs", "agent-0", 6);
    let out = node
        .process_inbox_and_allocate(6, &mut allocator, vec![])
        .unwrap();
    assert!(out.gcs_reconnected_this_tick);
    assert!(
        matches!(node.mission_state, AgentMissionState::WaitingForMission),
        "agent should restore pre-loss state instead of resetting to Idle"
    );
}

#[test]
fn neighbor_lost_continue_policy_does_not_abort() {
    let bus = make_bus();
    let own_id = AgentId::from("agent-0".to_owned());
    let peer_id = AgentId::from("agent-1".to_owned());
    let transport = InMemAgentTransport::new(bus.clone(), own_id.clone());
    let mut node = AgentNode::new(
        own_id,
        vec![peer_id.clone()],
        Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        ),
        transport,
    );
    node.gossip_interval_ticks = 999;
    node.autonomy = AgentAutonomyConfig {
        neighbor_lost_policy: NeighborLostPolicy::ReleaseLocksAndContinue,
        peer_heartbeat_timeout_ticks: 2,
        ..AgentAutonomyConfig::default()
    };
    let mut allocator = GreedyAllocator::default();

    // Ticks 1: ticks_since_peer = 1 < 2 → no detection yet
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(1, &mut allocator, vec![])
        .unwrap();
    assert!(out.neighbors_lost_this_tick.is_empty());

    // Tick 2: ticks_since_peer = 2 >= 2 → peer declared lost
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(2, &mut allocator, vec![])
        .unwrap();
    assert_eq!(
        out.neighbors_lost_this_tick,
        vec![peer_id],
        "peer should be declared lost at tick 2"
    );
    // ReleaseLocksAndContinue must NOT transition the FSM to Aborting
    assert!(
        !matches!(node.mission_state, AgentMissionState::Aborting { .. }),
        "ReleaseLocksAndContinue must NOT transition to Aborting"
    );
    assert!(
        matches!(node.mission_state, AgentMissionState::Idle),
        "mission state should remain Idle under ReleaseLocksAndContinue"
    );
}

#[test]
fn neighbor_lost_releases_segment_locks() {
    // Verifies that ReleaseLocksAndContinue emits the neighbor-lost event (which
    // signals the planner layer to release segment locks held by the lost peer) and
    // does not abort the local mission.
    // NOTE (M93 Non-Goal): releasing segment locks held by a *different* agent
    // requires a shared cross-agent lease registry, which is out of scope for M93.
    // This test verifies the FSM-level observable: event fires, no abort.
    let bus = make_bus();
    let own_id = AgentId::from("agent-0".to_owned());
    let peer_id = AgentId::from("agent-1".to_owned());
    let transport = InMemAgentTransport::new(bus.clone(), own_id.clone());
    let mut node = AgentNode::new(
        own_id,
        vec![peer_id.clone()],
        Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        ),
        transport,
    );
    node.gossip_interval_ticks = 999;
    node.autonomy = AgentAutonomyConfig {
        neighbor_lost_policy: NeighborLostPolicy::ReleaseLocksAndContinue,
        peer_heartbeat_timeout_ticks: 3,
        ..AgentAutonomyConfig::default()
    };
    let mut allocator = GreedyAllocator::default();

    // Ticks 1-2: peer absent, ticks_since = 1, 2 < 3 → no event yet
    for tick in 1u64..=2 {
        skip_tick(&bus);
        let out = node
            .process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
        assert!(
            out.neighbors_lost_this_tick.is_empty(),
            "no event before timeout at tick {tick}"
        );
    }

    // Tick 3: ticks_since = 3 >= 3 → event fires
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(3, &mut allocator, vec![])
        .unwrap();
    assert_eq!(
        out.neighbors_lost_this_tick,
        vec![peer_id.clone()],
        "neighbor-lost event must fire at threshold tick"
    );
    assert!(
        !matches!(node.mission_state, AgentMissionState::Aborting { .. }),
        "ReleaseLocksAndContinue must NOT abort the local mission"
    );

    // Tick 4: already detected — must not re-fire
    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(4, &mut allocator, vec![])
        .unwrap();
    assert!(
        out.neighbors_lost_this_tick.is_empty(),
        "neighbor-lost event must not fire twice for the same peer"
    );
}

#[test]
fn neighbor_lost_wait_for_reconnect_holds_then_aborts() {
    let bus = make_bus();
    let own_id = AgentId::from("agent-0".to_owned());
    let peer_id = AgentId::from("agent-1".to_owned());
    let transport = InMemAgentTransport::new(bus.clone(), own_id.clone());
    let mut node = AgentNode::new(
        own_id,
        vec![peer_id.clone()],
        Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        ),
        transport,
    );
    node.gossip_interval_ticks = 999;
    node.mission_state = AgentMissionState::WaitingForMission;
    node.autonomy = AgentAutonomyConfig {
        neighbor_lost_policy: NeighborLostPolicy::WaitForReconnect { max_ticks: 3 },
        peer_heartbeat_timeout_ticks: 2,
        ..AgentAutonomyConfig::default()
    };
    let mut allocator = GreedyAllocator::default();

    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(1, &mut allocator, vec![])
        .unwrap();
    assert!(out.neighbors_lost_this_tick.is_empty());

    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(2, &mut allocator, vec![])
        .unwrap();
    assert_eq!(out.neighbors_lost_this_tick, vec![peer_id.clone()]);
    assert!(matches!(
        node.mission_state,
        AgentMissionState::WaitingForNeighborReconnect {
            ref neighbor_id,
            since_tick: 2,
            until_tick: 5,
        } if *neighbor_id == peer_id
    ));

    for tick in 3u64..=4 {
        skip_tick(&bus);
        let out = node
            .process_inbox_and_allocate(tick, &mut allocator, vec![])
            .unwrap();
        assert!(out.neighbors_lost_this_tick.is_empty());
        assert!(matches!(
            node.mission_state,
            AgentMissionState::WaitingForNeighborReconnect { .. }
        ));
    }

    skip_tick(&bus);
    let out = node
        .process_inbox_and_allocate(5, &mut allocator, vec![])
        .unwrap();
    assert!(out.neighbors_lost_this_tick.is_empty());
    assert!(matches!(
        node.mission_state,
        AgentMissionState::Aborting { ref reason }
            if reason == "neighbor_reconnect_timeout:agent-1"
    ));
}

#[test]
fn neighbor_lost_wait_for_reconnect_restores_state_on_reconnect() {
    let bus = make_bus();
    let own_id = AgentId::from("agent-0".to_owned());
    let peer_id = AgentId::from("agent-1".to_owned());
    let transport = InMemAgentTransport::new(bus.clone(), own_id.clone());
    let mut node = AgentNode::new(
        own_id,
        vec![peer_id.clone()],
        Coordinator::new(
            vec![agent_entry("agent-0"), agent_entry("agent-1")],
            vec![],
            5,
        ),
        transport,
    );
    node.gossip_interval_ticks = 999;
    node.mission_state = AgentMissionState::WaitingForMission;
    node.autonomy = AgentAutonomyConfig {
        neighbor_lost_policy: NeighborLostPolicy::WaitForReconnect { max_ticks: 4 },
        peer_heartbeat_timeout_ticks: 2,
        ..AgentAutonomyConfig::default()
    };
    let mut allocator = GreedyAllocator::default();

    skip_tick(&bus);
    node.process_inbox_and_allocate(1, &mut allocator, vec![])
        .unwrap();
    skip_tick(&bus);
    node.process_inbox_and_allocate(2, &mut allocator, vec![])
        .unwrap();
    assert!(matches!(
        node.mission_state,
        AgentMissionState::WaitingForNeighborReconnect { .. }
    ));

    bus.borrow_mut().advance_tick();
    bus.borrow_mut()
        .send(RawMessage {
            from: peer_id.clone(),
            to: AgentId::from("agent-0".to_owned()),
            payload: RuntimeMessage::heartbeat(3, 1),
        })
        .unwrap();
    let out = node
        .process_inbox_and_allocate(3, &mut allocator, vec![])
        .unwrap();
    assert!(out.neighbors_lost_this_tick.is_empty());
    assert!(matches!(
        node.mission_state,
        AgentMissionState::WaitingForMission
    ));
}
fn active_lease_record(
    lease_id: &str,
    resource_id: &str,
    granted_tick: u64,
    expiry_tick: u64,
) -> ActiveLeaseRecord {
    ActiveLeaseRecord {
        lease_id: LeaseId::from(lease_id.to_owned()),
        resource_id: resource_id.to_owned(),
        resource_kind: "task".to_owned(),
        granted_tick,
        expiry_tick,
    }
}
