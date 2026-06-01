#![allow(unused_imports)]
use super::*;

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
