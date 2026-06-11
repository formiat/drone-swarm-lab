use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::*;
use swarm_comms::{
    InMemAgentTransport, InMemNetwork, NetworkConfig, SegmentDenyReason, SwarmMessage,
    SwarmMessageEnvelope, Transport, SWARM_PROTOCOL_SCHEMA_VERSION,
};
use swarm_types::{
    Aabb, Pose, UrbanBus, UrbanBusId, UrbanBusRoute, UrbanBusStop, UrbanDetectorConfig, UrbanEdge,
    UrbanEdgeId, UrbanGeoPoint, UrbanMap, UrbanNode, UrbanNodeId, UrbanPlannedRoute,
    UrbanRouteLoop, UrbanRouteSegment, UrbanSearchState, UrbanStaticObstacle, UrbanViolation,
};

fn node(id: &str, x: f64, y: f64) -> UrbanNode {
    UrbanNode {
        id: UrbanNodeId::from(id.to_owned()),
        pose: Pose {
            x,
            y,
            ..Default::default()
        },
        geo: None,
    }
}

fn edge(id: &str, from: &str, to: &str, cost: f64) -> UrbanEdge {
    edge_with_width(id, from, to, cost, 4.0)
}

fn edge_with_width(id: &str, from: &str, to: &str, cost: f64, width: f64) -> UrbanEdge {
    UrbanEdge {
        id: UrbanEdgeId::from(id.to_owned()),
        from: UrbanNodeId::from(from.to_owned()),
        to: UrbanNodeId::from(to.to_owned()),
        cost,
        length_m: cost,
        corridor_width_m: Some(width),
        blocked: false,
    }
}

fn block_map() -> UrbanMap {
    UrbanMap {
        nodes: vec![
            node("n0", 0.0, 0.0),
            node("n1", 10.0, 0.0),
            node("n2", 10.0, 10.0),
            node("n3", 0.0, 10.0),
        ],
        edges: vec![
            edge("e01", "n0", "n1", 10.0),
            edge("e12", "n1", "n2", 10.0),
            edge("e23", "n2", "n3", 10.0),
            edge("e30", "n3", "n0", 10.0),
            edge("e02", "n0", "n2", 25.0),
        ],
        static_obstacles: vec![],
    }
}

fn geo_block_map() -> UrbanMap {
    let mut map = block_map();
    for (index, node) in map.nodes.iter_mut().enumerate() {
        node.geo = Some(UrbanGeoPoint {
            lat_deg: 47.0 + index as f64 * 0.0001,
            lon_deg: 8.0 + index as f64 * 0.0001,
            alt_m: 5.0,
        });
    }
    map
}

fn corridor_delta_map() -> UrbanMap {
    UrbanMap {
        nodes: vec![
            node("start", 0.0, 0.0),
            node("goal", 20.0, 0.0),
            node("safe-a", 0.0, 10.0),
            node("safe-b", 20.0, 10.0),
        ],
        edges: vec![
            edge_with_width("narrow-shortcut", "start", "goal", 20.0, 1.5),
            edge_with_width("safe-north-a", "start", "safe-a", 10.0, 8.0),
            edge_with_width("safe-north-b", "safe-a", "safe-b", 20.0, 8.0),
            edge_with_width("safe-north-c", "safe-b", "goal", 10.0, 8.0),
        ],
        static_obstacles: vec![UrbanStaticObstacle {
            id: swarm_types::UrbanObstacleId::from("building-near-shortcut".to_owned()),
            bounds: Aabb {
                min_x: 9.0,
                min_y: 2.0,
                max_x: 11.0,
                max_y: 4.0,
            },
            label: Some("building".to_owned()),
        }],
    }
}

fn search_state(
    bus_pose: Pose,
    range: f64,
    probability: f64,
    false_positive: f64,
) -> UrbanSearchState {
    UrbanSearchState {
        buses: vec![UrbanBus {
            id: UrbanBusId::from("bus-0".to_owned()),
            pose: bus_pose,
            active_from_tick: None,
            active_until_tick: None,
            route: None,
        }],
        detector: UrbanDetectorConfig {
            detection_range_m: range,
            detection_probability: probability,
            false_positive_rate: false_positive,
            seed: 11,
        },
    }
}

fn agent_id(id: &str) -> swarm_types::AgentId {
    swarm_types::AgentId::from(id.to_owned())
}

fn coordinator_network(
    agent_ids: &[&str],
    lease_ticks: u64,
) -> (
    SegmentCoordinator<InMemAgentTransport>,
    HashMap<swarm_types::AgentId, InMemAgentTransport>,
) {
    let bus = Rc::new(RefCell::new(InMemNetwork::new(NetworkConfig {
        packet_loss_rate: 0.0,
        latency_ticks: 0,
        latency_per_hop: 0,
        seed: 95,
        partitions: HashSet::new(),
        comms_jitter_ticks: 0,
    })));
    let coordinator_id = agent_id("coordinator-0");
    let coordinator = SegmentCoordinator::new(
        coordinator_id.clone(),
        InMemAgentTransport::new(bus.clone(), coordinator_id),
        swarm_types::UrbanRightOfWayPolicy::FirstCome,
        HashMap::new(),
    )
    .with_default_lease_ticks(lease_ticks);
    let transports = agent_ids
        .iter()
        .map(|id| {
            let agent_id = agent_id(id);
            (
                agent_id.clone(),
                InMemAgentTransport::new(bus.clone(), agent_id),
            )
        })
        .collect();
    (coordinator, transports)
}

fn envelope(
    from: &swarm_types::AgentId,
    to: &swarm_types::AgentId,
    tick: u64,
    message: SwarmMessage,
) -> SwarmMessageEnvelope {
    SwarmMessageEnvelope {
        schema_version: SWARM_PROTOCOL_SCHEMA_VERSION.to_owned(),
        envelope_id: format!("env-{}-{tick}", from.as_ref()),
        correlation_id: None,
        from: from.clone(),
        to: to.clone(),
        sent_at: chrono::Utc::now(),
        ttl_ticks: 10,
        message,
    }
}

fn send_reserve(
    transports: &mut HashMap<swarm_types::AgentId, InMemAgentTransport>,
    from: &swarm_types::AgentId,
    tick: u64,
    edge_id: UrbanEdgeId,
) {
    let coordinator_id = agent_id("coordinator-0");
    transports
        .get_mut(from)
        .expect("agent transport should exist")
        .send(
            envelope(
                from,
                &coordinator_id,
                tick,
                SwarmMessage::SegmentReserve {
                    edge_id,
                    segment_index: 0,
                    requester: from.clone(),
                    request_tick: tick,
                },
            )
            .into_raw_message(),
        )
        .unwrap();
}

fn poll_segment_message(
    transports: &mut HashMap<swarm_types::AgentId, InMemAgentTransport>,
    agent_id: &swarm_types::AgentId,
) -> SwarmMessage {
    let raw = transports
        .get_mut(agent_id)
        .expect("agent transport should exist")
        .poll()
        .unwrap()
        .expect("response should be delivered");
    SwarmMessageEnvelope::from_raw_message(&raw)
        .expect("valid swarm envelope")
        .message
}

#[test]
fn segment_coordinator_grants_first_request() {
    let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
    let agent_0 = agent_id("agent-0");
    let (mut coordinator, mut transports) = coordinator_network(&["agent-0"], 30);

    send_reserve(&mut transports, &agent_0, 1, edge_id.clone());
    let events = coordinator.handle_incoming(1).unwrap();

    assert_eq!(
        events,
        vec![CoordinatorEvent::GrantSent {
            edge_id: edge_id.clone(),
            to: agent_0.clone(),
        }]
    );
    assert!(matches!(
        poll_segment_message(&mut transports, &agent_0),
        SwarmMessage::SegmentGrant { edge_id: granted, to, .. }
            if granted == edge_id && to == agent_0
    ));
}

#[test]
fn segment_coordinator_denies_concurrent_to_held_segment() {
    let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
    let agent_0 = agent_id("agent-0");
    let agent_1 = agent_id("agent-1");
    let (mut coordinator, mut transports) = coordinator_network(&["agent-0", "agent-1"], 30);

    send_reserve(&mut transports, &agent_0, 1, edge_id.clone());
    coordinator.handle_incoming(1).unwrap();
    let _ = poll_segment_message(&mut transports, &agent_0);
    send_reserve(&mut transports, &agent_1, 2, edge_id.clone());
    let events = coordinator.handle_incoming(2).unwrap();

    assert_eq!(
        events,
        vec![CoordinatorEvent::DenySent {
            edge_id: edge_id.clone(),
            to: agent_1.clone(),
            reason: SegmentDenyReason::AlreadyHeld,
        }]
    );
    assert!(matches!(
        poll_segment_message(&mut transports, &agent_1),
        SwarmMessage::SegmentDeny { edge_id: denied, to, holder, reason }
            if denied == edge_id
                && to == agent_1
                && holder == agent_0
                && reason == SegmentDenyReason::AlreadyHeld
    ));
}

#[test]
fn segment_coordinator_grants_after_release() {
    let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
    let agent_0 = agent_id("agent-0");
    let agent_1 = agent_id("agent-1");
    let (mut coordinator, mut transports) = coordinator_network(&["agent-0", "agent-1"], 30);

    send_reserve(&mut transports, &agent_0, 1, edge_id.clone());
    coordinator.handle_incoming(1).unwrap();
    let lease = match poll_segment_message(&mut transports, &agent_0) {
        SwarmMessage::SegmentGrant { lease, .. } => lease,
        message => panic!("expected segment grant, got {message:?}"),
    };
    transports
        .get_mut(&agent_0)
        .unwrap()
        .send(
            envelope(
                &agent_0,
                &agent_id("coordinator-0"),
                2,
                SwarmMessage::SegmentRelease {
                    edge_id: edge_id.clone(),
                    lease_id: lease.lease_id,
                },
            )
            .into_raw_message(),
        )
        .unwrap();
    assert_eq!(
        coordinator.handle_incoming(2).unwrap(),
        vec![CoordinatorEvent::Released {
            edge_id: edge_id.clone(),
            agent_id: agent_0,
        }]
    );

    send_reserve(&mut transports, &agent_1, 3, edge_id.clone());
    assert_eq!(
        coordinator.handle_incoming(3).unwrap(),
        vec![CoordinatorEvent::GrantSent {
            edge_id: edge_id.clone(),
            to: agent_1.clone(),
        }]
    );
}

#[test]
fn segment_lease_expiry_frees_segment() {
    let edge_id = UrbanEdgeId::from("road-n0-n1".to_owned());
    let agent_0 = agent_id("agent-0");
    let agent_1 = agent_id("agent-1");
    let (mut coordinator, mut transports) = coordinator_network(&["agent-0", "agent-1"], 1);

    send_reserve(&mut transports, &agent_0, 1, edge_id.clone());
    coordinator.handle_incoming(1).unwrap();
    let _ = poll_segment_message(&mut transports, &agent_0);

    send_reserve(&mut transports, &agent_1, 3, edge_id.clone());
    let events = coordinator.handle_incoming(3).unwrap();

    assert_eq!(
        events,
        vec![
            CoordinatorEvent::LeaseExpired {
                edge_id: edge_id.clone(),
                agent_id: agent_0,
            },
            CoordinatorEvent::GrantSent {
                edge_id,
                to: agent_1,
            },
        ]
    );
}

#[test]
fn urban_operational_evidence_serde_roundtrip() {
    let log = swarm_replay::EventLog {
        schema_version: swarm_replay::event_log::EVENT_LOG_SCHEMA_VERSION.to_owned(),
        run_id: "urban-network-run".to_owned(),
        seed: 95,
        scenario_name: "urban_perimeter_patrol.network".to_owned(),
        events: vec![
            swarm_replay::Event::UrbanRoutePlanned {
                agent_id: agent_id("agent-0"),
                tick: 0,
                edge_ids: vec![UrbanEdgeId::from("road-n0-n1".to_owned())],
                route_length_m: 20.0,
            },
            swarm_replay::Event::UrbanSegmentCompleted {
                agent_id: agent_id("agent-0"),
                tick: 10,
                segment_index: 0,
                edge_id: UrbanEdgeId::from("road-n0-n1".to_owned()),
            },
            swarm_replay::Event::SwarmProtocolMessage {
                tick: 0,
                from: agent_id("agent-0"),
                to: agent_id("coordinator-0"),
                envelope_id: "env-0".to_owned(),
                kind: "segment_reserve".to_owned(),
            },
        ],
    };

    let evidence = build_urban_operational_evidence_from_replay(
        &log,
        "abc123",
        swarm_comms::DeconflictionMode::NetworkProtocol {
            coordinator_id: agent_id("coordinator-0"),
        },
    )
    .expect("urban replay should produce evidence");
    let pack = UrbanOperationalEvidencePack::new(vec![evidence]);
    let json = serde_json::to_string(&pack).unwrap();
    let decoded: UrbanOperationalEvidencePack = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, pack);
    assert_eq!(
        decoded.evidence[0].schema_version,
        URBAN_OPERATIONAL_EVIDENCE_SCHEMA_VERSION
    );
    assert_eq!(decoded.evidence[0].sector_assignments.len(), 1);
}

#[test]
fn agent_failure_triggers_handoff_to_reserve() {
    let log = urban_evidence_log(
        "urban_perimeter_patrol_network_failure",
        vec![
            route_planned_event("agent-0", "road-n0-n1"),
            swarm_replay::Event::AgentFailed {
                agent_id: agent_id("agent-0"),
                tick: 4,
            },
            swarm_replay::Event::SwarmOwnershipHandoff {
                tick: 5,
                from_agent_id: agent_id("agent-0"),
                to_agent_id: agent_id("agent-1"),
                ownership_kind: "urban_segment".to_owned(),
                resource_id: "road-n0-n1".to_owned(),
                reason: "agent_failed".to_owned(),
            },
        ],
    );

    let evidence = build_urban_operational_evidence_from_replay(
        &log,
        "abc123",
        swarm_comms::DeconflictionMode::NetworkProtocol {
            coordinator_id: agent_id("coordinator-0"),
        },
    )
    .expect("handoff replay should produce evidence");

    assert_eq!(evidence.handoff_events.len(), 1);
    assert_eq!(evidence.handoff_events[0].1, agent_id("agent-0"));
    assert_eq!(evidence.handoff_events[0].2, agent_id("agent-1"));
    assert_eq!(evidence.handoff_events[0].3, "road-n0-n1");
}

#[test]
fn search_detection_triggers_sector_handoff() {
    let log = urban_evidence_log(
        "urban_search_until_detection_network",
        vec![
            route_planned_event("agent-0", "search-sector-0"),
            swarm_replay::Event::BusDetected {
                agent_id: agent_id("agent-0"),
                tick: 8,
                bus_id: UrbanBusId::from("bus-0".to_owned()),
                pose: Pose {
                    x: 10.0,
                    y: 0.0,
                    ..Default::default()
                },
                distance_m: 1.0,
                detector_seed: 11,
            },
            swarm_replay::Event::SwarmOwnershipHandoff {
                tick: 9,
                from_agent_id: agent_id("agent-0"),
                to_agent_id: agent_id("agent-1"),
                ownership_kind: "search_sector".to_owned(),
                resource_id: "search-sector-0".to_owned(),
                reason: "mocked_detection".to_owned(),
            },
        ],
    );

    let evidence = build_urban_operational_evidence_from_replay(
        &log,
        "abc123",
        swarm_comms::DeconflictionMode::NetworkProtocol {
            coordinator_id: agent_id("coordinator-0"),
        },
    )
    .expect("search replay should produce evidence");

    assert_eq!(evidence.mission_family, "urban-search-until-detection");
    assert_eq!(evidence.handoff_events[0].3, "search-sector-0");
}

#[test]
fn checkpoint_wait_on_coordinator_unavailable() {
    let log = urban_evidence_log(
        "urban_perimeter_patrol_network_partition",
        vec![
            route_planned_event("agent-0", "road-n0-n1"),
            swarm_replay::Event::CommandSuppressed {
                tick: 7,
                resource_id: "checkpoint-n1".to_owned(),
                reason: "ambiguous authority: coordinator unavailable".to_owned(),
            },
        ],
    );

    let evidence = build_urban_operational_evidence_from_replay(
        &log,
        "abc123",
        swarm_comms::DeconflictionMode::NetworkProtocol {
            coordinator_id: agent_id("coordinator-0"),
        },
    )
    .expect("checkpoint replay should produce evidence");

    assert!(evidence.degraded_outcomes.iter().any(|outcome| {
        outcome == "command_suppressed:checkpoint-n1:ambiguous authority: coordinator unavailable"
    }));
}

#[test]
fn no_safe_route_produces_explicit_degraded_outcome() {
    let log = urban_evidence_log(
        "urban_blocked_route_recovery_network",
        vec![
            route_planned_event("agent-0", "road-n0-n1"),
            swarm_replay::Event::UrbanNoRouteAvailable {
                agent_id: agent_id("agent-0"),
                tick: 6,
                from: UrbanNodeId::from("n0".to_owned()),
                to: UrbanNodeId::from("n2".to_owned()),
                reason: "all paths blocked".to_owned(),
            },
        ],
    );

    let evidence = build_urban_operational_evidence_from_replay(
        &log,
        "abc123",
        swarm_comms::DeconflictionMode::NetworkProtocol {
            coordinator_id: agent_id("coordinator-0"),
        },
    )
    .expect("blocked-route replay should produce evidence");

    assert_eq!(evidence.mission_family, "urban-blocked-route-recovery");
    assert!(evidence
        .degraded_outcomes
        .contains(&"no_route_available:all paths blocked".to_owned()));
}

fn urban_evidence_log(
    scenario_name: &str,
    events: Vec<swarm_replay::Event>,
) -> swarm_replay::EventLog {
    swarm_replay::EventLog {
        schema_version: swarm_replay::event_log::EVENT_LOG_SCHEMA_VERSION.to_owned(),
        run_id: format!("{scenario_name}-run"),
        seed: 95,
        scenario_name: scenario_name.to_owned(),
        events,
    }
}

fn route_planned_event(agent_id: &str, edge_id: &str) -> swarm_replay::Event {
    swarm_replay::Event::UrbanRoutePlanned {
        agent_id: self::agent_id(agent_id),
        tick: 0,
        edge_ids: vec![UrbanEdgeId::from(edge_id.to_owned())],
        route_length_m: 20.0,
    }
}

#[test]
fn urban_dijkstra_returns_shortest_route() {
    let route = plan_route(
        &block_map(),
        &UrbanNodeId::from("n0".to_owned()),
        &UrbanNodeId::from("n2".to_owned()),
    )
    .unwrap();
    let ids: Vec<_> = route
        .segments
        .iter()
        .map(|segment| &segment.edge_id)
        .collect();
    assert_eq!(
        ids,
        vec![
            &UrbanEdgeId::from("e01".to_owned()),
            &UrbanEdgeId::from("e12".to_owned())
        ]
    );
    assert_eq!(route.total_length_m, 20.0);
}

#[test]
fn urban_dijkstra_tie_breaking_is_deterministic() {
    let mut map = block_map();
    map.edges.push(edge("e03", "n0", "n3", 10.0));
    let route = plan_route(
        &map,
        &UrbanNodeId::from("n0".to_owned()),
        &UrbanNodeId::from("n2".to_owned()),
    )
    .unwrap();
    let ids: Vec<_> = route
        .segments
        .iter()
        .map(|segment| &segment.edge_id)
        .collect();
    assert_eq!(
        ids,
        vec![
            &UrbanEdgeId::from("e01".to_owned()),
            &UrbanEdgeId::from("e12".to_owned())
        ]
    );
}

#[test]
fn urban_planner_mode_rejects_unknown_value() {
    let err = UrbanPlannerMode::parse("shortest-and-magic").unwrap_err();
    assert!(matches!(err, UrbanRouteError::InvalidInput { field, .. } if field == "planner"));
}

#[test]
fn corridor_aware_route_prefers_wider_lower_risk_detour() {
    let map = corridor_delta_map();
    let from = UrbanNodeId::from("start".to_owned());
    let to = UrbanNodeId::from("goal".to_owned());
    let dijkstra = plan_route_with_mode(&map, &from, &to, UrbanPlannerMode::Dijkstra).unwrap();
    let corridor = plan_route_with_mode(&map, &from, &to, UrbanPlannerMode::CorridorAware).unwrap();

    assert_eq!(dijkstra.segments.len(), 1);
    assert_eq!(
        dijkstra.segments[0].edge_id,
        UrbanEdgeId::from("narrow-shortcut".to_owned())
    );
    assert_eq!(
        corridor
            .segments
            .iter()
            .map(|segment| segment.edge_id.as_ref())
            .collect::<Vec<_>>(),
        vec!["safe-north-a", "safe-north-b", "safe-north-c"]
    );
    assert!(corridor.total_length_m > dijkstra.total_length_m);
    assert!(route_risk_score(&map, &corridor) < route_risk_score(&map, &dijkstra));
    assert!(judge_route(&map, &corridor).is_empty());
}

#[test]
fn corridor_aware_handles_missing_width_without_panic() {
    let mut map = corridor_delta_map();
    map.edges[0].corridor_width_m = None;
    let route = plan_route_with_mode(
        &map,
        &UrbanNodeId::from("start".to_owned()),
        &UrbanNodeId::from("goal".to_owned()),
        UrbanPlannerMode::CorridorAware,
    )
    .unwrap();
    assert!(!route.segments.is_empty());
    assert!(route_risk_score(&map, &route).is_finite());
}

#[test]
fn urban_route_loop_expands_segments() {
    let route = expand_route_loop(
        &block_map(),
        &UrbanRouteLoop {
            nodes: vec![
                UrbanNodeId::from("n0".to_owned()),
                UrbanNodeId::from("n1".to_owned()),
                UrbanNodeId::from("n2".to_owned()),
                UrbanNodeId::from("n3".to_owned()),
                UrbanNodeId::from("n0".to_owned()),
            ],
        },
    )
    .unwrap();
    assert_eq!(route.segments.len(), 4);
    assert_eq!(route.total_length_m, 40.0);
}

#[test]
fn urban_route_missing_node_is_error() {
    let err = plan_route(
        &block_map(),
        &UrbanNodeId::from("missing".to_owned()),
        &UrbanNodeId::from("n2".to_owned()),
    )
    .unwrap_err();
    assert!(matches!(err, UrbanRouteError::InvalidInput { .. }));
}

#[test]
fn urban_route_avoids_blocked_edge() {
    let mut map = block_map();
    map.edges
        .iter_mut()
        .find(|edge| edge.id == UrbanEdgeId::from("e01".to_owned()))
        .unwrap()
        .blocked = true;
    map.edges.push(edge("e03", "n0", "n3", 10.0));
    map.edges.push(edge("e32", "n3", "n2", 10.0));
    let route = plan_route(
        &map,
        &UrbanNodeId::from("n0".to_owned()),
        &UrbanNodeId::from("n2".to_owned()),
    )
    .unwrap();
    assert_eq!(
        route.segments[0].edge_id,
        UrbanEdgeId::from("e03".to_owned())
    );
}

#[test]
fn urban_route_reports_no_route() {
    let mut map = block_map();
    map.edges.clear();
    map.edges.push(edge("isolated", "n0", "n1", 1.0));
    let err = plan_route(
        &map,
        &UrbanNodeId::from("n2".to_owned()),
        &UrbanNodeId::from("n0".to_owned()),
    )
    .unwrap_err();
    assert!(matches!(err, UrbanRouteError::NoRoute { .. }));
}

#[test]
fn urban_route_exports_ordered_waypoints() {
    let export = export_route_loop_to_waypoints(
        &block_map(),
        &UrbanRouteLoop {
            nodes: vec![
                UrbanNodeId::from("n0".to_owned()),
                UrbanNodeId::from("n1".to_owned()),
                UrbanNodeId::from("n2".to_owned()),
                UrbanNodeId::from("n3".to_owned()),
                UrbanNodeId::from("n0".to_owned()),
            ],
        },
        &UrbanRouteExportOptions {
            max_spacing_m: 100.0,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(export.waypoints.len(), 4);
    assert_eq!(export.waypoints[0].seq, 0);
    assert_eq!(
        export.waypoints[0].edge_id,
        UrbanEdgeId::from("e01".to_owned())
    );
    assert_eq!(export.waypoints[0].pose.x, 10.0);
    assert_eq!(export.waypoints[0].pose.y, 0.0);
    assert_eq!(export.waypoints[1].seq, 1);
    assert_eq!(
        export.waypoints[1].edge_id,
        UrbanEdgeId::from("e12".to_owned())
    );
    assert_eq!(export.waypoints[1].pose.x, 10.0);
    assert_eq!(export.waypoints[1].pose.y, 10.0);
    assert_eq!(
        export.waypoints[2].edge_id,
        UrbanEdgeId::from("e23".to_owned())
    );
    assert_eq!(
        export.waypoints[3].edge_id,
        UrbanEdgeId::from("e30".to_owned())
    );
}

#[test]
fn urban_route_exports_wgs84_node_geo_without_densification() {
    let export = export_planned_route_to_waypoints(
        &geo_block_map(),
        UrbanPlannedRoute {
            segments: vec![
                UrbanRouteSegment {
                    edge_id: UrbanEdgeId::from("e01".to_owned()),
                    from: UrbanNodeId::from("n0".to_owned()),
                    to: UrbanNodeId::from("n1".to_owned()),
                    length_m: 10.0,
                    cost: 10.0,
                },
                UrbanRouteSegment {
                    edge_id: UrbanEdgeId::from("e12".to_owned()),
                    from: UrbanNodeId::from("n1".to_owned()),
                    to: UrbanNodeId::from("n2".to_owned()),
                    length_m: 10.0,
                    cost: 10.0,
                },
            ],
            total_length_m: 20.0,
            total_cost: 20.0,
        },
        &UrbanRouteExportOptions {
            max_spacing_m: 1.0,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        export.metadata.coordinate_mode,
        UrbanCoordinateMode::Wgs84NodeGeo
    );
    assert_eq!(export.waypoints.len(), 2);
    assert_eq!(export.waypoints[0].geo.unwrap().lat_deg, 47.0001);
    assert_eq!(export.waypoints[1].geo.unwrap().lon_deg, 8.0002);
}

#[test]
fn urban_geojson_import_preserves_geo_and_computes_local_pose() {
    let map = import_urban_map_from_geojson_str(
        r#"{
          "type": "FeatureCollection",
          "features": [
            {"type":"Feature","properties":{"id":"n0"},"geometry":{"type":"Point","coordinates":[8.0,47.0,5.0]}},
            {"type":"Feature","properties":{"id":"n1"},"geometry":{"type":"Point","coordinates":[8.0001,47.0001,5.0]}},
            {"type":"Feature","properties":{"id":"e01","from":"n0","to":"n1","corridor_width_m":6.0},"geometry":{"type":"LineString","coordinates":[[8.0,47.0],[8.0001,47.0001]]}}
          ]
        }"#,
        &UrbanGeoJsonImportOptions::default(),
    )
    .unwrap();

    assert_eq!(map.nodes.len(), 2);
    assert_eq!(map.edges.len(), 1);
    assert_eq!(map.nodes[0].pose.x, 0.0);
    assert_eq!(map.nodes[0].pose.y, 0.0);
    assert!(map.nodes[1].pose.x > 0.0);
    assert!(map.nodes[1].pose.y > 0.0);
    assert_eq!(map.nodes[1].geo.unwrap().lat_deg, 47.0001);
}

#[test]
fn urban_geojson_import_rejects_unsupported_geometry() {
    let err = import_urban_map_from_geojson_str(
        r#"{
          "type": "FeatureCollection",
          "features": [
            {"type":"Feature","properties":{"id":"building"},"geometry":{"type":"Polygon","coordinates":[]}}
          ]
        }"#,
        &UrbanGeoJsonImportOptions::default(),
    )
    .unwrap_err();

    assert!(
        matches!(err, UrbanGeoJsonImportError::Invalid { field, .. } if field == "features[0].geometry.type")
    );
}

#[test]
fn urban_route_export_stable_ids() {
    let route_loop = UrbanRouteLoop {
        nodes: vec![
            UrbanNodeId::from("n0".to_owned()),
            UrbanNodeId::from("n1".to_owned()),
            UrbanNodeId::from("n2".to_owned()),
        ],
    };
    let options = UrbanRouteExportOptions {
        max_spacing_m: 100.0,
        ..Default::default()
    };

    let first = export_route_loop_to_waypoints(&block_map(), &route_loop, &options).unwrap();
    let second = export_route_loop_to_waypoints(&block_map(), &route_loop, &options).unwrap();

    assert_eq!(first.waypoints, second.waypoints);
    assert_eq!(first.waypoints[0].task_id, "urban-route-0-e01-1");
    assert_eq!(first.waypoints[1].task_id, "urban-route-1-e12-1");
}

#[test]
fn urban_route_altitude_explicit() {
    let export = export_route_loop_to_waypoints(
        &block_map(),
        &UrbanRouteLoop {
            nodes: vec![
                UrbanNodeId::from("n0".to_owned()),
                UrbanNodeId::from("n1".to_owned()),
            ],
        },
        &UrbanRouteExportOptions {
            default_altitude_m: 17.5,
            max_spacing_m: 100.0,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(export.metadata.altitude_m, 17.5);
    assert_eq!(export.waypoints[0].pose.z, 17.5);
    assert_eq!(
        export.metadata.altitude_source,
        "urban_route_export.default_altitude_m"
    );
}

#[test]
fn urban_route_export_densifies_long_edges() {
    let route = plan_route(
        &block_map(),
        &UrbanNodeId::from("n0".to_owned()),
        &UrbanNodeId::from("n1".to_owned()),
    )
    .unwrap();
    let export = export_planned_route_to_waypoints(
        &block_map(),
        route,
        &UrbanRouteExportOptions {
            max_spacing_m: 4.0,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(export.waypoints.len(), 3);
    assert!((export.waypoints[0].pose.x - 10.0 / 3.0).abs() < 1e-9);
    assert!((export.waypoints[1].pose.x - 20.0 / 3.0).abs() < 1e-9);
    assert_eq!(export.waypoints[2].pose.x, 10.0);
    assert_eq!(export.waypoints[2].point_index_on_segment, 3);
}

#[test]
fn urban_route_export_rejects_bad_spacing() {
    let err = export_route_loop_to_waypoints(
        &block_map(),
        &UrbanRouteLoop {
            nodes: vec![
                UrbanNodeId::from("n0".to_owned()),
                UrbanNodeId::from("n1".to_owned()),
            ],
        },
        &UrbanRouteExportOptions {
            max_spacing_m: 0.0,
            ..Default::default()
        },
    )
    .unwrap_err();

    assert!(matches!(
        err,
        UrbanRouteExportError::InvalidOption { field, .. } if field == "max_spacing_m"
    ));
}

#[test]
fn urban_judge_reports_blocked_edge_violation() {
    let mut map = block_map();
    map.edges[0].blocked = true;
    let route = UrbanPlannedRoute {
        segments: vec![UrbanRouteSegment {
            edge_id: UrbanEdgeId::from("e01".to_owned()),
            from: UrbanNodeId::from("n0".to_owned()),
            to: UrbanNodeId::from("n1".to_owned()),
            length_m: 10.0,
            cost: 10.0,
        }],
        total_length_m: 10.0,
        total_cost: 10.0,
    };
    assert!(matches!(
        judge_route(&map, &route).as_slice(),
        [UrbanViolation::BlockedEdge { .. }]
    ));
}

#[test]
fn urban_judge_reports_aabb_intersection() {
    let mut map = block_map();
    map.static_obstacles.push(UrbanStaticObstacle {
        id: swarm_types::UrbanObstacleId::from("building".to_owned()),
        bounds: Aabb {
            min_x: 4.0,
            min_y: -1.0,
            max_x: 6.0,
            max_y: 1.0,
        },
        label: Some("building".to_owned()),
    });
    let route = plan_route(
        &map,
        &UrbanNodeId::from("n0".to_owned()),
        &UrbanNodeId::from("n1".to_owned()),
    )
    .unwrap();
    assert!(matches!(
        judge_route(&map, &route).as_slice(),
        [UrbanViolation::ObstacleIntersection { .. }]
    ));
}

#[test]
fn urban_pose_along_segment_interpolates_and_clamps() {
    let map = block_map();
    let segment = UrbanRouteSegment {
        edge_id: UrbanEdgeId::from("e01".to_owned()),
        from: UrbanNodeId::from("n0".to_owned()),
        to: UrbanNodeId::from("n1".to_owned()),
        length_m: 10.0,
        cost: 10.0,
    };

    let halfway = pose_along_segment(&map, &segment, 5.0).unwrap();
    assert_eq!(halfway.x, 5.0);
    assert_eq!(halfway.y, 0.0);

    let clamped = pose_along_segment(&map, &segment, 50.0).unwrap();
    assert_eq!(clamped.x, 10.0);
    assert_eq!(clamped.y, 0.0);
}

fn square_perimeter() -> Vec<Pose> {
    vec![
        Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        },
        Pose {
            x: 20.0,
            y: 0.0,
            ..Default::default()
        },
        Pose {
            x: 20.0,
            y: 20.0,
            ..Default::default()
        },
        Pose {
            x: 0.0,
            y: 20.0,
            ..Default::default()
        },
    ]
}

#[test]
fn perimeter_waypoints_square_correct_count() {
    let waypoints = perimeter_waypoints(&square_perimeter(), 10.0).unwrap();

    assert_eq!(waypoints.len(), 9);
    assert_eq!(waypoints[0].x, 0.0);
    assert_eq!(waypoints[1].x, 10.0);
    assert_eq!(waypoints[2].x, 20.0);
}

#[test]
fn perimeter_waypoints_is_deterministic() {
    let first = perimeter_waypoints(&square_perimeter(), 10.0).unwrap();
    let second = perimeter_waypoints(&square_perimeter(), 10.0).unwrap();

    assert_eq!(first, second);
}

#[test]
fn perimeter_waypoints_closed_route() {
    let waypoints = perimeter_waypoints(&square_perimeter(), 10.0).unwrap();

    assert_eq!(waypoints.first(), waypoints.last());
}

#[test]
fn perimeter_waypoints_rejects_invalid_spacing() {
    let err = perimeter_waypoints(&square_perimeter(), 0.0).unwrap_err();

    assert!(matches!(
        err,
        UrbanRouteError::InvalidInput { field, .. } if field == "perimeter.spacing_m"
    ));
}

#[test]
fn detector_detects_in_range_bus_with_probability_one() {
    let state = search_state(
        Pose {
            x: 1.0,
            y: 0.0,
            ..Default::default()
        },
        2.0,
        1.0,
        0.0,
    );

    let outcome = detect_buses(&block_map(), Pose::default(), 0, 42, &state);

    assert_eq!(outcome.observations.len(), 1);
    assert!(outcome.detection.is_some());
    assert!(!outcome.false_positive);
}

#[test]
fn detector_ignores_out_of_range_bus() {
    let state = search_state(
        Pose {
            x: 10.0,
            y: 0.0,
            ..Default::default()
        },
        2.0,
        1.0,
        0.0,
    );

    let outcome = detect_buses(&block_map(), Pose::default(), 0, 42, &state);

    assert!(outcome.observations.is_empty());
    assert!(outcome.detection.is_none());
    assert!(!outcome.false_positive);
}

#[test]
fn detector_probability_zero_never_detects_real_bus() {
    let state = search_state(
        Pose {
            x: 1.0,
            y: 0.0,
            ..Default::default()
        },
        2.0,
        0.0,
        0.0,
    );

    let outcome = detect_buses(&block_map(), Pose::default(), 0, 42, &state);

    assert_eq!(outcome.observations.len(), 1);
    assert!(outcome.detection.is_none());
    assert!(!outcome.false_positive);
}

#[test]
fn detector_false_positive_is_seed_controlled() {
    let state = search_state(
        Pose {
            x: 10.0,
            y: 0.0,
            ..Default::default()
        },
        2.0,
        0.0,
        1.0,
    );

    let outcome = detect_buses(&block_map(), Pose::default(), 0, 42, &state);

    assert!(outcome.observations.is_empty());
    assert!(outcome.detection.is_none());
    assert!(outcome.false_positive);
}

#[test]
fn detector_respects_bus_active_window() {
    let mut state = search_state(
        Pose {
            x: 1.0,
            y: 0.0,
            ..Default::default()
        },
        2.0,
        1.0,
        0.0,
    );
    state.buses[0].active_from_tick = Some(5);
    state.buses[0].active_until_tick = Some(10);

    assert!(detect_buses(&block_map(), Pose::default(), 4, 42, &state)
        .observations
        .is_empty());
    assert!(detect_buses(&block_map(), Pose::default(), 5, 42, &state)
        .detection
        .is_some());
    assert!(detect_buses(&block_map(), Pose::default(), 11, 42, &state)
        .observations
        .is_empty());
}

#[test]
fn detect_buses_finds_moving_bus_when_in_range() {
    let mut state = search_state(Pose::default(), 0.5, 1.0, 0.0);
    state.buses[0].pose = Pose {
        x: 100.0,
        y: 100.0,
        ..Default::default()
    };
    state.buses[0].route = Some(UrbanBusRoute {
        stops: vec![
            UrbanBusStop {
                node_id: UrbanNodeId::from("n0".to_owned()),
                arrival_tick: 0,
            },
            UrbanBusStop {
                node_id: UrbanNodeId::from("n1".to_owned()),
                arrival_tick: 10,
            },
        ],
        speed_m_per_tick: 1.0,
    });

    let outcome = detect_buses(
        &block_map(),
        Pose {
            x: 5.0,
            y: 0.0,
            ..Default::default()
        },
        5,
        42,
        &state,
    );

    assert_eq!(outcome.observations.len(), 1);
    assert!(outcome.detection.is_some());
    assert_eq!(outcome.observations[0].pose.x, 5.0);
}

#[test]
fn detect_buses_misses_moving_bus_out_of_range() {
    let mut state = search_state(Pose::default(), 0.5, 1.0, 0.0);
    state.buses[0].route = Some(UrbanBusRoute {
        stops: vec![
            UrbanBusStop {
                node_id: UrbanNodeId::from("n0".to_owned()),
                arrival_tick: 0,
            },
            UrbanBusStop {
                node_id: UrbanNodeId::from("n1".to_owned()),
                arrival_tick: 10,
            },
        ],
        speed_m_per_tick: 1.0,
    });

    let outcome = detect_buses(
        &block_map(),
        Pose {
            x: 0.0,
            y: 10.0,
            ..Default::default()
        },
        5,
        42,
        &state,
    );

    assert!(outcome.observations.is_empty());
    assert!(outcome.detection.is_none());
}

#[test]
fn detect_buses_records_sampled_moving_pose() {
    let mut state = search_state(
        Pose {
            x: 99.0,
            y: 99.0,
            ..Default::default()
        },
        1.0,
        1.0,
        0.0,
    );
    state.buses[0].route = Some(UrbanBusRoute {
        stops: vec![
            UrbanBusStop {
                node_id: UrbanNodeId::from("n0".to_owned()),
                arrival_tick: 0,
            },
            UrbanBusStop {
                node_id: UrbanNodeId::from("n1".to_owned()),
                arrival_tick: 10,
            },
        ],
        speed_m_per_tick: 1.0,
    });

    let outcome = detect_buses(
        &block_map(),
        Pose {
            x: 5.0,
            y: 0.0,
            ..Default::default()
        },
        5,
        42,
        &state,
    );

    assert_eq!(outcome.observations[0].pose.x, 5.0);
    assert_eq!(outcome.observations[0].pose.y, 0.0);
}

#[test]
fn plan_route_excluding_finds_alternate_path() {
    // Map: n0 -e01-> n1 -e12-> n2, plus shortcut n0 -e02-> n2
    let map = UrbanMap {
        nodes: vec![
            node("n0", 0.0, 0.0),
            node("n1", 10.0, 0.0),
            node("n2", 20.0, 0.0),
        ],
        edges: vec![
            edge("e01", "n0", "n1", 10.0),
            edge("e12", "n1", "n2", 10.0),
            edge("e02", "n0", "n2", 25.0),
        ],
        static_obstacles: vec![],
    };
    let from = UrbanNodeId::from("n0".to_owned());
    let to = UrbanNodeId::from("n2".to_owned());
    let mut extra_blocked = HashSet::new();
    extra_blocked.insert(UrbanEdgeId::from("e01".to_owned()));

    let route = plan_route_excluding(&map, &from, &to, &extra_blocked, UrbanPlannerMode::Dijkstra)
        .expect("alternate route exists");
    // Must not contain the excluded edge
    assert!(!route
        .segments
        .iter()
        .any(|s| s.edge_id == UrbanEdgeId::from("e01".to_owned())));
    // Must arrive at n2
    assert_eq!(
        route.segments.last().unwrap().to,
        UrbanNodeId::from("n2".to_owned())
    );
}

#[test]
fn plan_route_excluding_returns_no_route_if_all_blocked() {
    let map = UrbanMap {
        nodes: vec![node("n0", 0.0, 0.0), node("n1", 10.0, 0.0)],
        edges: vec![edge("e01", "n0", "n1", 10.0)],
        static_obstacles: vec![],
    };
    let from = UrbanNodeId::from("n0".to_owned());
    let to = UrbanNodeId::from("n1".to_owned());
    let mut extra_blocked = HashSet::new();
    extra_blocked.insert(UrbanEdgeId::from("e01".to_owned()));

    let result = plan_route_excluding(&map, &from, &to, &extra_blocked, UrbanPlannerMode::Dijkstra);
    assert!(matches!(result, Err(UrbanRouteError::NoRoute { .. })));
}

// Urban bridge tests for M80 Mission Command IR.

fn simple_map() -> UrbanMap {
    UrbanMap {
        nodes: vec![
            node("n0", 0.0, 0.0),
            node("n1", 10.0, 0.0),
            node("n2", 10.0, 10.0),
        ],
        edges: vec![edge("e01", "n0", "n1", 1.0), edge("e12", "n1", "n2", 1.0)],
        static_obstacles: vec![],
    }
}

fn simple_route() -> UrbanPlannedRoute {
    UrbanPlannedRoute {
        segments: vec![
            UrbanRouteSegment {
                edge_id: UrbanEdgeId::from("e01".to_owned()),
                from: UrbanNodeId::from("n0".to_owned()),
                to: UrbanNodeId::from("n1".to_owned()),
                length_m: 10.0,
                cost: 1.0,
            },
            UrbanRouteSegment {
                edge_id: UrbanEdgeId::from("e12".to_owned()),
                from: UrbanNodeId::from("n1".to_owned()),
                to: UrbanNodeId::from("n2".to_owned()),
                length_m: 10.0,
                cost: 1.0,
            },
        ],
        total_length_m: 20.0,
        total_cost: 2.0,
    }
}

#[test]
fn urban_route_to_follow_route_non_empty() {
    use swarm_mission_ir::{MissionCommand, RouteId};

    let map = simple_map();
    let route = simple_route();
    let route_id = RouteId::from("test-route".to_owned());

    let cmd = urban_route_to_follow_route(&map, &route, route_id, 5.0);
    assert!(cmd.is_some(), "expected Some from non-empty route");

    if let Some(MissionCommand::FollowRoute { waypoints, .. }) = cmd {
        assert_eq!(waypoints.len(), 2, "expected one waypoint per segment");
    } else {
        panic!("expected MissionCommand::FollowRoute");
    }
}

#[test]
fn urban_route_to_follow_route_empty_route_returns_none() {
    use swarm_mission_ir::RouteId;

    let map = simple_map();
    let empty = UrbanPlannedRoute {
        segments: vec![],
        total_length_m: 0.0,
        total_cost: 0.0,
    };
    let cmd = urban_route_to_follow_route(&map, &empty, RouteId::from("r".to_owned()), 5.0);
    assert!(cmd.is_none());
}

#[test]
fn urban_route_to_follow_route_altitude_in_waypoints() {
    use swarm_mission_ir::{MissionCommand, Position, RouteId};

    let map = simple_map();
    let route = simple_route();
    let altitude = 8.5_f64;
    let cmd = urban_route_to_follow_route(&map, &route, RouteId::from("r".to_owned()), altitude);

    if let Some(MissionCommand::FollowRoute { waypoints, .. }) = cmd {
        for wp in &waypoints {
            if let Position::Local(loc) = wp.position {
                assert!(
                    (loc.z_m - altitude).abs() < 1e-10,
                    "waypoint altitude {z} != {altitude}",
                    z = loc.z_m
                );
            } else {
                panic!("expected local position");
            }
        }
    } else {
        panic!("expected FollowRoute");
    }
}

#[test]
fn urban_route_to_follow_route_node_positions_correct() {
    use swarm_mission_ir::{MissionCommand, Position, RouteId};

    let map = simple_map();
    let route = simple_route();
    let cmd = urban_route_to_follow_route(&map, &route, RouteId::from("r".to_owned()), 5.0);

    if let Some(MissionCommand::FollowRoute { waypoints, .. }) = cmd {
        // first segment "to" node is n1 at (10.0, 0.0)
        if let Position::Local(loc) = waypoints[0].position {
            assert!((loc.x_m - 10.0).abs() < 1e-10);
            assert!((loc.y_m - 0.0).abs() < 1e-10);
        }
        // second segment "to" node is n2 at (10.0, 10.0)
        if let Position::Local(loc) = waypoints[1].position {
            assert!((loc.x_m - 10.0).abs() < 1e-10);
            assert!((loc.y_m - 10.0).abs() < 1e-10);
        }
    } else {
        panic!("expected FollowRoute");
    }
}
