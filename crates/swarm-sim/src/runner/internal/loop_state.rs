use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use swarm_alloc::Allocator;
use swarm_comms::{InMemAgentTransport, InMemNetwork, NetworkConfig};
use swarm_runtime::{AgentNode, Coordinator, GridState};
use swarm_types::{AdapterRegistry, AgentId, Pose, TaskId};

use super::super::{InspectionState, RunConfig, SafetyAllocator, WildfireState};
use crate::{Clock, Scenario};

/// Mutable state owned by the main scenario tick loop.
pub(in crate::runner) struct TickLoopState<A: Allocator> {
    pub nodes: Vec<(AgentNode<InMemAgentTransport>, AgentId)>,
    pub bus: Rc<RefCell<InMemNetwork>>,
    pub allocator: SafetyAllocator<A>,
    pub clock: Clock,
    pub log_builder: Option<swarm_replay::EventLogBuilder>,
    pub failure_ticks: HashMap<AgentId, u64>,
    pub crashed_agents: HashSet<AgentId>,
    pub detected_agents: HashSet<AgentId>,
    pub unassigned_durations: HashMap<TaskId, u64>,
    pub max_task_unassigned_ticks: u64,
    pub detection_time_ticks: Option<u64>,
    pub detection_tick: Option<u64>,
    pub reallocation_time_ticks: Option<u64>,
    pub total_ticks: u64,
    pub tasks_injected: u64,
    pub tasks_expired: u64,
    pub conflicting_assignments: u64,
    pub stale_messages_discarded: u64,
    pub partition_events: u64,
    pub partitions_active: bool,
    pub convergence_ticks: Option<u64>,
    pub heal_tick: Option<u64>,
    pub max_view_divergence: u64,
    pub revisit_count: u64,
    pub total_distance_travelled: f64,
    pub time_to_first_exhaustion: Option<u64>,
    pub safety_violations: u64,
    pub cbba_convergence_tick: Option<u64>,
    pub adapter_registry: AdapterRegistry,
    pub wildfire_state: Option<WildfireState>,
    pub priority_updates: u64,
    pub high_priority_zones_mapped: u64,
    pub time_to_map_first_high_risk: Option<u64>,
    pub threat_level_over_time: Vec<f64>,
    pub zone_observations: u64,
    pub coverage_over_time: Vec<f64>,
    pub grid_state: Option<GridState>,
    pub inspection_state: Option<InspectionState>,
    pub availability_per_tick: Vec<f64>,
    pub disconnected_agents_max: u64,
    pub relay_reallocation_ticks: Option<u64>,
    pub relay_detection_tick: Option<u64>,
    pub total_hop_count_sum: f64,
    pub total_hop_count_ticks: u64,
    pub base_id: AgentId,
    pub base_pose: Pose,
}

impl<A: Allocator> TickLoopState<A> {
    pub(in crate::runner) fn new(
        scenario: &Scenario,
        config: &RunConfig,
        allocator: A,
        log_builder: Option<swarm_replay::EventLogBuilder>,
    ) -> Self {
        let allocator = SafetyAllocator {
            inner: allocator,
            safety_config: config.safety_config.clone(),
        };
        let bus = Rc::new(RefCell::new(InMemNetwork::new(NetworkConfig {
            packet_loss_rate: config.packet_loss_rate,
            latency_ticks: config.latency_ticks,
            latency_per_hop: config.latency_per_hop,
            seed: scenario.seed,
            partitions: HashSet::new(),
            comms_jitter_ticks: config.comms_jitter_ticks,
        })));

        let agent_ids: Vec<AgentId> = scenario.agents.iter().map(|a| a.id.clone()).collect();
        let nodes: Vec<(AgentNode<InMemAgentTransport>, AgentId)> = scenario
            .agents
            .iter()
            .map(|agent| {
                let peer_ids: Vec<AgentId> = agent_ids
                    .iter()
                    .filter(|id| *id != &agent.id)
                    .cloned()
                    .collect();
                let transport = InMemAgentTransport::new(bus.clone(), agent.id.clone());
                let coordinator = Coordinator::new(
                    scenario.agents.clone(),
                    scenario.tasks.clone(),
                    config.timeout_ticks,
                );
                let mut node = AgentNode::new(agent.id.clone(), peer_ids, coordinator, transport);
                node.gossip_interval_ticks = config.gossip_interval_ticks;
                node.config.enable_movement = config.enable_movement;
                node.config.tick_duration_ms = config.tick_duration_ms;
                if config.enable_cbba {
                    #[allow(clippy::field_reassign_with_default)]
                    {
                        let mut cbba = swarm_alloc::CbbaAllocator::default();
                        cbba.packet_loss_rate = config.packet_loss_rate;
                        node.cbba = Some(cbba);
                    }
                }
                (node, agent.id.clone())
            })
            .collect();

        let failure_ticks = config
            .failures
            .iter()
            .map(|failure| (failure.agent_id.clone(), failure.at_tick))
            .collect();
        let base_id = config
            .base_id
            .clone()
            .unwrap_or_else(|| AgentId::from("base".to_owned()));
        let base_pose = scenario.base_station.unwrap_or(Pose {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        });

        Self {
            nodes,
            bus,
            allocator,
            clock: Clock::new(1),
            log_builder,
            failure_ticks,
            crashed_agents: HashSet::new(),
            detected_agents: HashSet::new(),
            unassigned_durations: HashMap::new(),
            max_task_unassigned_ticks: 0,
            detection_time_ticks: None,
            detection_tick: None,
            reallocation_time_ticks: None,
            total_ticks: 0,
            tasks_injected: 0,
            tasks_expired: 0,
            conflicting_assignments: 0,
            stale_messages_discarded: 0,
            partition_events: 0,
            partitions_active: false,
            convergence_ticks: None,
            heal_tick: None,
            max_view_divergence: 0,
            revisit_count: 0,
            total_distance_travelled: 0.0,
            time_to_first_exhaustion: None,
            safety_violations: 0,
            cbba_convergence_tick: None,
            adapter_registry: AdapterRegistry::new(),
            wildfire_state: config.wildfire_state.clone(),
            priority_updates: 0,
            high_priority_zones_mapped: 0,
            time_to_map_first_high_risk: None,
            threat_level_over_time: Vec::new(),
            zone_observations: 0,
            coverage_over_time: Vec::new(),
            grid_state: config.grid_state.clone(),
            inspection_state: config.inspection_state.clone(),
            availability_per_tick: Vec::new(),
            disconnected_agents_max: 0,
            relay_reallocation_ticks: None,
            relay_detection_tick: None,
            total_hop_count_sum: 0.0,
            total_hop_count_ticks: 0,
            base_id,
            base_pose,
        }
    }
}
