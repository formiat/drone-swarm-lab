#[cfg(feature = "mavlink-transport")]
use std::collections::BTreeMap;
#[cfg(feature = "mavlink-transport")]
use std::path::Path;
#[cfg(feature = "mavlink-transport")]
use std::time::Duration;
#[cfg(feature = "mavlink-transport")]
use std::time::Instant;

#[cfg(feature = "mavlink-transport")]
use crate::sitl_observability::{SitlEventLogMode, SitlEventRecorder};
use crate::sitl_plan::{validate_connection_string, SitlError, SitlPlan};
#[cfg(feature = "mavlink-transport")]
use crate::sitl_report::{write_sitl_run_report, SitlRunFinalStatus, SitlRunMode, SitlRunReport};
#[cfg(feature = "mavlink-transport")]
use swarm_comms::Waypoint;

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
