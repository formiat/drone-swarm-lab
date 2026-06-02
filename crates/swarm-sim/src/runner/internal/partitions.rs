use super::super::*;

pub(in crate::runner) struct PartitionTickOutcome {
    pub partition_events: u64,
    pub partitions_active: bool,
    pub heal_tick: Option<u64>,
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
    };

    for event in partition_events {
        if event.at_tick == current_tick {
            bus.borrow_mut()
                .add_partition(event.agents.0.clone(), event.agents.1.clone());
            outcome.partition_events += 1;
            outcome.partitions_active = true;
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
            for (node, _) in &mut *nodes {
                if let Some(ref mut cbba) = node.cbba {
                    cbba.converged = false;
                }
            }
        }
    }

    outcome
}
