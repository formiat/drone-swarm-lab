use swarm_alloc::GreedyAllocator;
use swarm_safety::{SafetyConfig, SeparationConstraint};
use swarm_sim::{RunConfig, Scenario, ScenarioRunner};
use swarm_types::{Agent, AgentId, Health, Pose, Role, Task, TaskId, TaskStatus};

fn make_agent(id: &str, x: f64, y: f64) -> Agent {
    Agent {
        id: AgentId::from(id.to_owned()),
        role: Role::Scout,
        health: Health::Alive,
        pose: Pose {
            x,
            y,
            ..Default::default()
        },
        capabilities: vec![],
        current_task: None,
        battery: 100.0,
        comms_range: 1000.0,
        generation: 1,
        speed: 0.0,
        max_range: 1000.0,
        battery_drain_rate: 0.0,
        battery_model: None,
    }
}

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

#[test]
fn safety_nofly_tasks_not_assigned() {
    let scenario = Scenario {
        name: "safety_nofly_test".to_owned(),
        seed: 0,
        agents: vec![make_agent("a0", 20.0, 20.0)], // outside no-fly 0-10
        tasks: vec![make_task("t0", 5.0, 5.0)],     // inside no-fly 0-10
        ground_nodes: vec![],
        base_station: None,
        geo_origin: None,
    };

    let config = RunConfig {
        max_ticks: 20,
        timeout_ticks: 3,
        max_unassigned_ticks: 10,
        packet_loss_rate: 0.0,
        latency_ticks: 0,
        latency_per_hop: 0,
        failures: vec![],
        dynamic_tasks: vec![],
        partition_events: vec![],
        gossip_interval_ticks: 999,
        base_id: None,
        enable_movement: false,
        tick_duration_ms: 100,
        grid_state: None,
        enable_cbba: false,
        safety_config: Some(SafetyConfig {
            geofence: None,
            no_fly_zones: vec![swarm_safety::NoFlyZone {
                bounds: swarm_safety::Aabb {
                    min_x: 0.0,
                    max_x: 10.0,
                    min_y: 0.0,
                    max_y: 10.0,
                },
                active_from_tick: None,
                active_until_tick: None,
            }],
            separation: None,
            ..Default::default()
        }),
        ..Default::default()
    };

    let metrics = ScenarioRunner::run_with(&scenario, config, GreedyAllocator);
    assert!(
        !metrics.all_tasks_assigned,
        "task in no-fly should not be assigned"
    );
    assert_eq!(metrics.safety_violations, 0, "no violations expected");
}

#[test]
fn safety_violations_counted() {
    let scenario = Scenario {
        name: "safety_violation_test".to_owned(),
        seed: 0,
        agents: vec![make_agent("a0", 150.0, 150.0)], // outside geofence 0-100
        tasks: vec![make_task("t0", 150.0, 150.0)],   // same position, also outside
        ground_nodes: vec![],
        base_station: None,
        geo_origin: None,
    };

    let config = RunConfig {
        max_ticks: 10,
        timeout_ticks: 3,
        max_unassigned_ticks: 10,
        packet_loss_rate: 0.0,
        latency_ticks: 0,
        latency_per_hop: 0,
        failures: vec![],
        dynamic_tasks: vec![],
        partition_events: vec![],
        gossip_interval_ticks: 999,
        base_id: None,
        enable_movement: false,
        tick_duration_ms: 100,
        grid_state: None,
        enable_cbba: false,
        safety_config: Some(SafetyConfig {
            geofence: Some(swarm_safety::Geofence {
                bounds: swarm_safety::Aabb {
                    min_x: 0.0,
                    max_x: 100.0,
                    min_y: 0.0,
                    max_y: 100.0,
                },
            }),
            no_fly_zones: vec![],
            separation: None,
            ..Default::default()
        }),
        ..Default::default()
    };

    let metrics = ScenarioRunner::run_with(&scenario, config, GreedyAllocator);
    assert!(
        metrics.safety_violations > 0,
        "expected geofence violations, got {}",
        metrics.safety_violations
    );
}

#[test]
fn safety_separation_no_panic() {
    let scenario = Scenario {
        name: "safety_separation_test".to_owned(),
        seed: 0,
        agents: vec![
            make_agent("a0", 0.0, 0.0),
            make_agent("a1", 2.0, 0.0), // distance 2 < min_distance 5
        ],
        tasks: vec![make_task("t0", 0.0, 0.0), make_task("t1", 2.0, 0.0)],
        ground_nodes: vec![],
        base_station: None,
        geo_origin: None,
    };

    let config = RunConfig {
        max_ticks: 10,
        timeout_ticks: 3,
        max_unassigned_ticks: 10,
        packet_loss_rate: 0.0,
        latency_ticks: 0,
        latency_per_hop: 0,
        failures: vec![],
        dynamic_tasks: vec![],
        partition_events: vec![],
        gossip_interval_ticks: 999,
        base_id: None,
        enable_movement: false,
        tick_duration_ms: 100,
        grid_state: None,
        enable_cbba: false,
        safety_config: Some(SafetyConfig {
            geofence: None,
            no_fly_zones: vec![],
            separation: Some(SeparationConstraint {
                min_distance_m: 5.0,
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let metrics = ScenarioRunner::run_with(&scenario, config, GreedyAllocator);
    assert!(
        metrics.safety_violations > 0,
        "expected separation violations"
    );
}
