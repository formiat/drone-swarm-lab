use std::collections::HashSet;

use swarm_comms::InMemAgentTransport;
use swarm_runtime::AgentNode;
use swarm_types::{AgentId, TaskKind};

use crate::runner::WildfireState;

#[derive(Debug, Clone, Default, PartialEq)]
pub(in crate::runner) struct WildfireTickMetrics {
    pub priority_updates: u64,
    pub high_priority_zones_mapped: u64,
    pub time_to_map_first_high_risk: Option<u64>,
    pub zone_observations: u64,
    pub avg_threat_level: f64,
}

pub(in crate::runner) fn process_wildfire_mapping_tick(
    nodes: &mut [(AgentNode<InMemAgentTransport>, AgentId)],
    crashed_agents: &HashSet<AgentId>,
    wildfire_state: &mut WildfireState,
    wind: Option<(f64, f64, f64)>,
    current_tick: u64,
    first_high_risk_already_mapped: bool,
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
) -> WildfireTickMetrics {
    let mut metrics = WildfireTickMetrics::default();

    for (node, agent_id) in &mut *nodes {
        if crashed_agents.contains(agent_id) {
            continue;
        }
        let assigned_tasks: Vec<_> = node
            .coordinator
            .registry
            .tasks()
            .filter(|task| task.assigned_to.as_ref() == Some(agent_id))
            .filter(|task| task.kind == Some(TaskKind::MappingZone))
            .cloned()
            .collect();
        for task in assigned_tasks {
            if let Some(entry) = node.coordinator.membership.get(agent_id) {
                let task_pose = task.pose.unwrap_or(entry.pose);
                let dist = entry.pose.distance_to(&task_pose);
                let threshold = 1.0;
                if dist < threshold {
                    let zone_id = task.id.to_string();
                    metrics.zone_observations += 1;
                    if wildfire_state.mapped_zone_ids.insert(zone_id.clone()) {
                        if let Some(builder) = log_builder {
                            builder.push(swarm_replay::Event::AgentObservation {
                                agent_id: agent_id.clone(),
                                zone_id: zone_id.clone(),
                                tick: current_tick,
                            });
                            builder.push(swarm_replay::Event::TaskCompleted {
                                task_id: task.id.clone(),
                                agent_id: agent_id.clone(),
                                tick: current_tick,
                            });
                        }
                        node.coordinator.registry.complete_assigned_task(&task.id);
                        if let Some(zone) =
                            wildfire_state.zones.iter().find(|zone| zone.id == zone_id)
                        {
                            if zone.priority >= 5 {
                                metrics.high_priority_zones_mapped += 1;
                                if !first_high_risk_already_mapped
                                    && metrics.time_to_map_first_high_risk.is_none()
                                {
                                    metrics.time_to_map_first_high_risk = Some(current_tick);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    update_dynamic_threat(
        nodes,
        wildfire_state,
        wind,
        current_tick,
        log_builder,
        &mut metrics,
    );
    metrics.avg_threat_level = average_threat_level(wildfire_state);
    metrics
}

fn update_dynamic_threat(
    nodes: &mut [(AgentNode<InMemAgentTransport>, AgentId)],
    wildfire_state: &mut WildfireState,
    wind: Option<(f64, f64, f64)>,
    current_tick: u64,
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
    metrics: &mut WildfireTickMetrics,
) {
    if !wildfire_state.enable_dynamic_threat
        || current_tick == 0
        || !current_tick.is_multiple_of(wildfire_state.update_interval_ticks)
    {
        return;
    }

    let zone_count = wildfire_state.zones.len();
    let mut threat_changes: Vec<f64> = vec![0.0; zone_count];
    for (i, zone) in wildfire_state.zones.iter().enumerate() {
        let base_increase = 0.1;
        let mut increase = base_increase;
        if let Some((wx, wy, _)) = wind {
            let wind_magnitude = (wx * wx + wy * wy).sqrt();
            if wind_magnitude > 0.0 && zone.threat_level > 0.5 {
                increase += wind_magnitude * 0.05;
            }
        }
        threat_changes[i] = increase;
    }

    if wildfire_state.enable_spatial_spread {
        for i in 0..zone_count {
            if wildfire_state.zones[i].threat_level > 0.8 {
                let neighbors = match i {
                    0 => vec![1],
                    n if n == zone_count - 1 => vec![n - 1],
                    _ => vec![i - 1, i + 1],
                };
                for &neighbor in &neighbors {
                    if neighbor < zone_count {
                        threat_changes[neighbor] += 0.05;
                    }
                }
            }
        }
    }

    for (i, zone) in wildfire_state.zones.iter_mut().enumerate() {
        let old_threat = zone.threat_level;
        zone.threat_level = (zone.threat_level + threat_changes[i]).min(1.0);
        if zone.threat_level - old_threat > 0.2 {
            zone.priority = (zone.priority + 2).min(10);
        } else {
            zone.priority = (zone.priority + 1).min(10);
        }
        if let Some(builder) = log_builder {
            builder.push(swarm_replay::Event::HazardMapUpdated {
                zone_id: zone.id.clone(),
                new_threat_level: zone.threat_level,
                new_priority: zone.priority,
                tick: current_tick,
            });
        }
    }

    for (node, _) in nodes {
        for task in node.coordinator.registry.tasks_mut() {
            if task.kind != Some(TaskKind::MappingZone) {
                continue;
            }
            if let Some(zone) = wildfire_state
                .zones
                .iter()
                .find(|zone| zone.id == task.id.to_string())
            {
                let old_priority = task.priority;
                task.priority = zone.priority;
                if old_priority != task.priority {
                    metrics.priority_updates += 1;
                    if let Some(builder) = log_builder {
                        builder.push(swarm_replay::Event::TaskPriorityUpdated {
                            task_id: task.id.clone(),
                            old_priority,
                            new_priority: task.priority,
                            tick: current_tick,
                        });
                    }
                }
            }
        }
    }
}

fn average_threat_level(wildfire_state: &WildfireState) -> f64 {
    if wildfire_state.zones.is_empty() {
        0.0
    } else {
        wildfire_state
            .zones
            .iter()
            .map(|zone| zone.threat_level)
            .sum::<f64>()
            / wildfire_state.zones.len() as f64
    }
}
