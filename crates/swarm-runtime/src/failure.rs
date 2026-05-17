use swarm_types::AgentId;

use crate::MembershipView;

pub struct FailureDetector {
    pub timeout_ticks: u64,
}

impl FailureDetector {
    pub fn new(timeout_ticks: u64) -> Self {
        Self { timeout_ticks }
    }

    pub fn detect(&self, view: &MembershipView, current_tick: u64) -> Vec<AgentId> {
        view.alive_agents()
            .filter(|(agent_id, entry)| {
                let timed_out =
                    current_tick.saturating_sub(entry.last_heartbeat_tick) > self.timeout_ticks;
                if timed_out {
                    tracing::warn!(
                        agent_id = %agent_id,
                        timeout_ticks = self.timeout_ticks,
                        "failure detected"
                    );
                }
                timed_out
            })
            .map(|(agent_id, _)| agent_id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::{Agent, Health, Pose, Role};

    fn agent(id: &str) -> Agent {
        Agent {
            id: AgentId::from(id.to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose { x: 0.0, y: 0.0 },
            capabilities: Vec::new(),
            current_task: None,
            battery: 100.0,
            comms_range: f64::INFINITY,
            generation: 1,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
        }
    }

    #[test]
    fn detector_no_timeout_with_recent_hb() {
        let mut view = MembershipView::new(vec![agent("agent-0")]);
        let id = AgentId::from("agent-0".to_owned());
        view.record_heartbeat(&id, 5, 1);
        let detector = FailureDetector::new(3);

        assert!(detector.detect(&view, 7).is_empty());
    }

    #[test]
    fn detector_timeout_after_missed_hbs() {
        let view = MembershipView::new(vec![agent("agent-0")]);
        let detector = FailureDetector::new(3);

        assert_eq!(
            detector.detect(&view, 4),
            vec![AgentId::from("agent-0".to_owned())]
        );
    }
}
