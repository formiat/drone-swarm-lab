#![allow(unused_imports)]
use std::collections::{HashMap, HashSet};
use std::path::Path;
#[cfg(any(feature = "mavlink-transport", test))]
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
#[cfg(feature = "mavlink-transport")]
use std::time::Instant;

use swarm_alloc::GreedyAllocator;
use swarm_comms::{MockMavlinkTransport, RawMessage, Waypoint};
use swarm_runtime::{AgentNode, Coordinator, NodeTickOutput, RuntimeMessage};
use swarm_types::{AgentId, TaskId, TaskStatus};

#[cfg(feature = "mavlink-transport")]
use crate::sitl_connection::{
    default_takeoff_altitude, task_ids_by_seq_from_items, waypoints_from_sitl_items,
};
use crate::sitl_connection::{SitlConnectionLifecycle, SitlSafetyGate};
use crate::sitl_multi_agent::{
    MultiAgentLifecycle, MultiAgentSitlManifest, MultiAgentSitlManifestAgent,
};
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_observability::{summarize_sitl_event_log, SitlEventLogSummary};
use crate::sitl_observability::{
    write_sitl_event_log, SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
};
use crate::sitl_plan::{
    classify_connection_string, first_sitl_entry, SitlConnectionClass, SitlError, SitlWaypointItem,
};
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_report::{write_sitl_multi_agent_run_report, SitlMultiAgentAgentReport};
use crate::sitl_report::{SitlMultiAgentReallocationReport, SitlMultiAgentRunReport};

mod config;
pub use config::*;
use config::*;

#[cfg(feature = "mavlink-transport")]
mod live;
#[cfg(feature = "mavlink-transport")]
pub use live::*;
#[cfg(feature = "mavlink-transport")]
use live::*;

mod mock;
pub use mock::*;
use mock::*;

mod ports;
pub use ports::*;
use ports::*;

mod supervisor_flows;
pub use supervisor_flows::*;
use supervisor_flows::*;

mod reallocation;
use reallocation::*;

mod events;
use events::*;

mod artifacts;
use artifacts::*;

mod validation_and_reports;
use validation_and_reports::*;

#[cfg(test)]
mod tests_cases;
#[cfg(test)]
mod tests_support;
