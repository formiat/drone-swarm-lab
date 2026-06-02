#[cfg(all(test, feature = "mavlink-transport"))]
use std::time::Duration;

#[cfg(all(test, feature = "mavlink-transport"))]
use crate::sitl_observability::SitlEventLogMode;
#[cfg(all(test, feature = "mavlink-transport"))]
use crate::sitl_plan::SitlPlan;
#[cfg(test)]
use crate::sitl_plan::{validate_connection_string, SitlError};
#[cfg(all(test, feature = "mavlink-transport"))]
use crate::sitl_report::{SitlRunFinalStatus, SitlRunReport};
#[cfg(all(test, feature = "mavlink-transport"))]
use swarm_comms::Waypoint;

mod cli_and_mock;
pub use cli_and_mock::run;
#[cfg(all(test, feature = "mavlink-transport"))]
use cli_and_mock::*;

mod connection_and_reports;
#[cfg(all(test, feature = "mavlink-transport"))]
use connection_and_reports::*;

#[cfg(feature = "mavlink-transport")]
mod telemetry;
#[cfg(all(test, feature = "mavlink-transport"))]
use telemetry::*;

#[cfg(test)]
mod tests;
