#![allow(unused_imports)]
#[cfg(feature = "mavlink-transport")]
use std::collections::BTreeMap;
use std::path::Path;
use std::thread;
use std::time::Duration;
#[cfg(feature = "mavlink-transport")]
use std::time::Instant;

use crate::sitl_multi_agent::{
    agent_config, build_multi_agent_manifest, load_multi_agent_config, MultiAgentLifecycle,
    MultiAgentSitlAgentConfig,
};
use crate::sitl_observability::{
    write_sitl_event_log, SitlEventLogMetadata, SitlEventLogMode, SitlEventRecorder,
};
use crate::sitl_plan::{
    build_sitl_plan_for_task_ids, classify_connection_string, first_sitl_entry,
    format_dry_run_plan, load_sitl_suite, validate_connection_string, SitlConnectionClass,
    SitlError, SitlMode, SitlPlan,
};
#[cfg(feature = "mavlink-transport")]
use crate::sitl_report::{write_sitl_run_report, SitlRunFinalStatus, SitlRunMode, SitlRunReport};
use crate::sitl_safety::{
    load_sitl_safety_config, validate_pre_upload_safety, validate_pre_upload_safety_for_task_ids,
};
use swarm_comms::{MockMavlinkTransport, Waypoint};

mod cli_and_mock;
pub use cli_and_mock::run;
use cli_and_mock::*;

mod connection_and_reports;
use connection_and_reports::*;

#[cfg(feature = "mavlink-transport")]
mod telemetry;
#[cfg(feature = "mavlink-transport")]
use telemetry::*;

#[cfg(test)]
mod tests;
