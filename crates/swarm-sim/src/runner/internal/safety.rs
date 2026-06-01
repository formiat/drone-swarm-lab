use super::super::*;

pub(in crate::runner) fn record_safety_violations<T: Transport>(
    nodes: &mut [(AgentNode<T>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    scenario: &Scenario,
    safety_cfg: &swarm_safety::SafetyConfig,
    current_tick: u64,
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
) -> u64 {
    let all_agents: Vec<Agent> = nodes
        .iter()
        .filter(|(_, id)| !crashed_agents.contains(id))
        .map(|(node, id)| {
            node.coordinator
                .membership
                .get(id)
                .map(|entry| Agent {
                    id: id.clone(),
                    role: entry.role.clone(),
                    health: entry.health.clone(),
                    pose: entry.pose,
                    capabilities: entry.capabilities.clone(),
                    current_task: None,
                    battery: entry.battery,
                    comms_range: entry.comms_range,
                    generation: entry.generation,
                    speed: 0.0,
                    max_range: 0.0,
                    battery_drain_rate: 0.0,
                    battery_model: None,
                })
                .unwrap_or_else(|| {
                    scenario
                        .agents
                        .iter()
                        .find(|agent| &agent.id == id)
                        .cloned()
                        .expect("scenario agent should exist for runner membership")
                })
        })
        .collect();

    let mut safety_violations = 0;
    for (node, agent_id) in nodes {
        if crashed_agents.contains(agent_id) {
            continue;
        }
        if let Some(entry) = node.coordinator.membership.get(agent_id) {
            let agent = Agent {
                id: agent_id.clone(),
                role: entry.role.clone(),
                health: entry.health.clone(),
                pose: entry.pose,
                capabilities: entry.capabilities.clone(),
                current_task: None,
                battery: entry.battery,
                comms_range: entry.comms_range,
                generation: entry.generation,
                speed: 0.0,
                max_range: 0.0,
                battery_drain_rate: 0.0,
                battery_model: None,
            };
            let violations = swarm_safety::check_agent(safety_cfg, &agent, &all_agents);
            safety_violations += violations.len() as u64;
            if let Some(builder) = log_builder {
                for violation in &violations {
                    let violation_type = match violation.violation_type {
                        swarm_safety::ViolationType::NoFlyZoneEntered => {
                            swarm_replay::ViolationType::NoFly
                        }
                        swarm_safety::ViolationType::GeofenceExited => {
                            swarm_replay::ViolationType::Geofence
                        }
                        swarm_safety::ViolationType::SeparationBreached { .. } => {
                            swarm_replay::ViolationType::Separation
                        }
                    };
                    builder.push(swarm_replay::Event::SafetyViolation {
                        agent_id: agent_id.clone(),
                        violation_type,
                        tick: current_tick,
                    });
                }
            }
        }
    }
    safety_violations
}
