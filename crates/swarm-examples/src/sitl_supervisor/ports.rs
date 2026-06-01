use super::{
    AgentProgress, AgentStep, CompletedWaypoint, LiveAgentRun, MissionReplacementPlan,
    MultiAgentLifecycle, SitlError, SitlWaypointItem,
};

pub trait MissionClient {
    fn agent_id(&self) -> &str;
    fn mission_waypoints(&self) -> &[SitlWaypointItem];
    fn replace_mission(&mut self, plan: &MissionReplacementPlan) -> Result<(), SitlError>;
}

pub trait TelemetrySource {
    fn start(&mut self) -> Result<(), SitlError>;
    fn poll(&mut self) -> Result<Option<LiveAgentRun>, SitlError>;
    fn completed_task_count(&self) -> usize;
    fn completed_waypoints(&self) -> Vec<CompletedWaypoint>;
    fn completed_task_ids(&self) -> Vec<String>;
}

pub trait EventSink {
    fn record_agent_step(&mut self, _step: &AgentStep) {}
    fn record_agent_progress(&mut self, _progress: &AgentProgress) {}
}

pub trait SafetyGate {
    fn validate_agent_task_subset(
        &self,
        agent_id: &str,
        task_ids: &[String],
    ) -> Result<(), SitlError>;
}

pub trait LiveAgentController {
    fn agent_id(&self) -> &str;
    fn start_delay_ms(&self) -> u64;
    fn mission_waypoints(&self) -> &[SitlWaypointItem];
    fn replace_mission(&mut self, plan: &MissionReplacementPlan) -> Result<(), SitlError>;
    fn run(&mut self) -> Result<LiveAgentRun, SitlError>;
    fn start(&mut self) -> Result<(), SitlError> {
        Ok(())
    }
    fn poll(&mut self) -> Result<Option<LiveAgentRun>, SitlError> {
        Ok(Some(self.run()?))
    }
    fn completed_task_count(&self) -> usize {
        0
    }
    fn completed_waypoints(&self) -> Vec<CompletedWaypoint> {
        Vec::new()
    }
    fn completed_task_ids(&self) -> Vec<String> {
        Vec::new()
    }
}

pub trait AgentController {
    fn agent_id(&self) -> &str;
    fn lifecycle(&self) -> MultiAgentLifecycle;
    fn upload(&mut self, waypoints: &[SitlWaypointItem]) -> Result<AgentStep, SitlError>;
    fn start(&mut self) -> Result<AgentStep, SitlError>;
    fn poll(&mut self, tick: u64) -> Result<AgentProgress, SitlError>;
    fn abort(&mut self, reason: &str) -> Result<AgentStep, SitlError>;
}
