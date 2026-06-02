use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use rand::SeedableRng;
use swarm_alloc::Allocator;
use swarm_comms::{ConnectivityModel, ConnectivitySnapshot, InMemNetwork, Transport};
use swarm_metrics::RunMetrics;
use swarm_runtime::{AgentNode, Coordinator, GridState, NodeTickOutput};
use swarm_types::{
    AdapterRegistry, Agent, AgentId, Health, RunState, Task, TaskId, UrbanBusId, UrbanMap,
    UrbanPlannedRoute, UrbanRouteSegment, UrbanViolation,
};

use crate::{Clock, Scenario};

mod types;
use types::*;
pub use types::*;

mod internal;
mod scenario_runner_internal;
mod scenario_runner_public;
mod urban_events;
mod urban_helpers;
mod urban_metrics;
mod urban_patrol;
mod urban_search;
use urban_events::*;
use urban_helpers::*;
use urban_metrics::*;

#[cfg(test)]
mod tests;
