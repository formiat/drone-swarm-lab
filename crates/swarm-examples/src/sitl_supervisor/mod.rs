use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::thread;
use std::time::Duration;

use swarm_alloc::GreedyAllocator;
use swarm_comms::{MockMavlinkTransport, RawMessage};
use swarm_runtime::{AgentNode, Coordinator, NodeTickOutput, RuntimeMessage};
use swarm_types::{AgentId, TaskId, TaskStatus};

use crate::sitl_connection::SitlSafetyGate;
use crate::sitl_multi_agent::{
    MultiAgentLifecycle, MultiAgentSitlManifest, MultiAgentSitlManifestAgent,
};
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_observability::summarize_sitl_event_log;
use crate::sitl_observability::{
    write_sitl_event_log, SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
};
use crate::sitl_plan::{
    classify_connection_string, first_sitl_entry, SitlConnectionClass, SitlError, SitlWaypointItem,
};
#[cfg(any(feature = "mavlink-transport", test))]
use crate::sitl_report::write_sitl_multi_agent_run_report;
use crate::sitl_report::SitlMultiAgentRunReport;

mod config;
pub use config::*;

#[cfg(feature = "mavlink-transport")]
mod live;
#[cfg(feature = "mavlink-transport")]
pub use live::*;

mod mock;
pub use mock::*;

mod ports;
pub use ports::*;

mod supervisor_flows;
pub use supervisor_flows::*;

mod reallocation;
use reallocation::*;

mod events;

mod artifacts;

mod validation_and_reports;
use validation_and_reports::*;

#[cfg(test)]
mod tests_cases;
#[cfg(test)]
mod tests_support;
