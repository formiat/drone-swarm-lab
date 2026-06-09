use super::super::*;

pub(in crate::runner) struct PartitionTickOutcome {
    pub partition_events: u64,
    pub partitions_active: bool,
    pub heal_tick: Option<u64>,
    pub added_pairs: Vec<(AgentId, AgentId)>,
    pub healed_pairs: Vec<(AgentId, AgentId)>,
}

pub(in crate::runner) fn apply_partition_events<T: Transport>(
    partition_events: &[PartitionEvent],
    current_tick: u64,
    bus: &Rc<RefCell<InMemNetwork>>,
    nodes: &mut [(AgentNode<T>, AgentId)],
    log_builder: &mut Option<swarm_replay::EventLogBuilder>,
) -> PartitionTickOutcome {
    let mut outcome = PartitionTickOutcome {
        partition_events: 0,
        partitions_active: false,
        heal_tick: None,
        added_pairs: Vec::new(),
        healed_pairs: Vec::new(),
    };

    for event in partition_events {
        let pair = if event.agents.0.as_ref() <= event.agents.1.as_ref() {
            (event.agents.0.clone(), event.agents.1.clone())
        } else {
            (event.agents.1.clone(), event.agents.0.clone())
        };
        if event.at_tick == current_tick {
            bus.borrow_mut()
                .add_partition(event.agents.0.clone(), event.agents.1.clone());
            outcome.partition_events += 1;
            outcome.partitions_active = true;
            outcome.added_pairs.push(pair.clone());
            if let Some(builder) = log_builder {
                builder.push(swarm_replay::Event::PartitionAdded {
                    agent_a: event.agents.0.clone(),
                    agent_b: event.agents.1.clone(),
                    tick: current_tick,
                });
            }
        }
        if event.until_tick == Some(current_tick) {
            bus.borrow_mut()
                .remove_partition(event.agents.0.clone(), event.agents.1.clone());
            outcome.heal_tick = Some(current_tick);
            outcome.healed_pairs.push(pair.clone());
            if let Some(builder) = log_builder {
                builder.push(swarm_replay::Event::PartitionRemoved {
                    agent_a: event.agents.0.clone(),
                    agent_b: event.agents.1.clone(),
                    tick: current_tick,
                });
            }
        }
        if event.heal_at_tick == Some(current_tick) {
            bus.borrow_mut()
                .remove_partition(event.agents.0.clone(), event.agents.1.clone());
            outcome.heal_tick = Some(current_tick);
            outcome.healed_pairs.push(pair);
            for (node, _) in &mut *nodes {
                if let Some(ref mut cbba) = node.cbba {
                    cbba.converged = false;
                }
            }
        }
    }

    outcome
}
