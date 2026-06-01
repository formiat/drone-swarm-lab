#![allow(unused_imports)]
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use swarm_alloc::{
    route_cost, Allocator, BatteryAwarePlanner, NearestNeighbourPlanner, RoutePlanner,
};
use swarm_comms::{
    ConnectivityModel, ConnectivitySnapshot, InMemAgentTransport, InMemNetwork, NetworkConfig,
    Transport,
};
use swarm_metrics::RunMetrics;
use swarm_runtime::{AgentNode, Coordinator, GridState, NodeTickOutput};
use swarm_safety::SafetyConfig;
use swarm_types::{
    AdapterRegistry, Agent, AgentId, EdgeId, Health, InspectionGraph, Role, RunState, Task, TaskId,
    UrbanBusId, UrbanMap, UrbanNodeId, UrbanPlannedRoute, UrbanRouteLoop, UrbanRouteSegment,
    UrbanSearchState, UrbanViolation,
};

use crate::{Clock, Scenario};

mod types;
use types::*;
pub use types::*;

mod internal;
mod scenario_runner_internal;
mod scenario_runner_public;
mod scenario_runner_urban;
use internal::*;
mod urban_helpers;
use urban_helpers::*;

#[cfg(test)]
mod tests;
