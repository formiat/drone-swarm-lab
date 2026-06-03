use swarm_alloc::{
    AllocationAgent, AllocationTask, AuctionAllocator, CbbaAllocator, CentralizedPlanner,
    ConnectivityAwareAllocator, GreedyAllocator,
};
use swarm_sim::{RunConfig, Scenario};

pub(super) type StrategyFactory =
    Box<dyn Fn(&Scenario, &RunConfig) -> Box<dyn swarm_alloc::Strategy> + Send + Sync>;

#[derive(Clone)]
pub(super) enum PlannerChoice {
    NearestNeighbour,
    TwoOpt,
    BatteryAware,
}

fn make_cbba_allocator(planner: &PlannerChoice) -> CbbaAllocator {
    use swarm_alloc::route_planner::{BatteryAwarePlanner, NearestNeighbourPlanner, TwoOptPlanner};
    let mut cbba = CbbaAllocator::default();
    cbba.route_planner = match planner {
        PlannerChoice::NearestNeighbour => Box::new(NearestNeighbourPlanner),
        PlannerChoice::TwoOpt => Box::new(TwoOptPlanner::default()),
        PlannerChoice::BatteryAware => Box::new(BatteryAwarePlanner::default()),
    };
    cbba
}

pub(super) fn make_factories(planner: &PlannerChoice) -> Vec<StrategyFactory> {
    vec![
        Box::new(|_scenario: &Scenario, run_config: &RunConfig| {
            Box::new(GreedyAllocator {
                comms_penalty_weight: run_config.comms_penalty_weight,
            })
        }),
        Box::new(|_scenario: &Scenario, run_config: &RunConfig| {
            Box::new(AuctionAllocator {
                comms_penalty_weight: run_config.comms_penalty_weight,
                ..AuctionAllocator::default()
            })
        }),
        Box::new(|_scenario: &Scenario, run_config: &RunConfig| {
            Box::new(ConnectivityAwareAllocator {
                base_allocator: AuctionAllocator {
                    comms_penalty_weight: run_config.comms_penalty_weight,
                    ..AuctionAllocator::default()
                },
            })
        }),
        Box::new(|scenario: &Scenario, _run_config: &RunConfig| {
            let allocation_tasks: Vec<AllocationTask<'_>> = scenario
                .tasks
                .iter()
                .map(|t| AllocationTask { task: t })
                .collect();
            let allocation_agents: Vec<AllocationAgent> = scenario
                .agents
                .iter()
                .map(|a| AllocationAgent {
                    id: a.id.clone(),
                    pose: a.pose,
                    battery: a.battery,
                    capabilities: a.capabilities.clone(),
                    role: a.role.clone(),
                    comms_range: a.comms_range,
                    speed: 0.0,
                    max_range: 0.0,
                    battery_drain_rate: 0.0,
                })
                .collect();
            Box::new(CentralizedPlanner::new(
                &allocation_tasks,
                &allocation_agents,
            ))
        }),
        Box::new({
            let planner = planner.clone();
            move |_scenario: &Scenario, _run_config: &RunConfig| {
                Box::new(make_cbba_allocator(&planner))
            }
        }),
    ]
}
