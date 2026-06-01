use super::{
    AgentController, AgentProgress, AgentStep, MultiAgentLifecycle, MultiAgentSitlManifestAgent,
    SitlError, SitlWaypointItem,
};
use swarm_comms::{MockMavlinkTransport, Waypoint};

pub struct MockAgentController {
    agent_id: String,
    lifecycle: MultiAgentLifecycle,
    fail_after_ticks: Option<u64>,
    transport: MockMavlinkTransport,
}

impl MockAgentController {
    pub fn new(agent: &MultiAgentSitlManifestAgent, fail_after_ticks: Option<u64>) -> Self {
        Self {
            agent_id: agent.agent_id.clone(),
            lifecycle: agent.lifecycle,
            fail_after_ticks,
            transport: MockMavlinkTransport::new(),
        }
    }

    pub fn waypoints_sent(&self) -> usize {
        self.transport.waypoints().len()
    }
}

impl AgentController for MockAgentController {
    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn lifecycle(&self) -> MultiAgentLifecycle {
        self.lifecycle
    }

    fn upload(&mut self, waypoints: &[SitlWaypointItem]) -> Result<AgentStep, SitlError> {
        for waypoint in waypoints {
            self.transport.send_waypoint(Waypoint {
                x: waypoint.x,
                y: waypoint.y,
                z: waypoint.z,
                seq: waypoint.seq,
            });
        }
        Ok(AgentStep {
            agent_id: self.agent_id.clone(),
            waypoint_count: self.waypoints_sent(),
        })
    }

    fn start(&mut self) -> Result<AgentStep, SitlError> {
        Ok(AgentStep {
            agent_id: self.agent_id.clone(),
            waypoint_count: self.waypoints_sent(),
        })
    }

    fn poll(&mut self, tick: u64) -> Result<AgentProgress, SitlError> {
        let heartbeat_seen = self
            .fail_after_ticks
            .is_none_or(|fail_after_ticks| tick < fail_after_ticks);
        Ok(AgentProgress {
            agent_id: self.agent_id.clone(),
            heartbeat_seen,
        })
    }

    fn abort(&mut self, _reason: &str) -> Result<AgentStep, SitlError> {
        Ok(AgentStep {
            agent_id: self.agent_id.clone(),
            waypoint_count: self.waypoints_sent(),
        })
    }
}
