#[cfg(test)]
use crate::sitl_multi_agent::MultiAgentSitlManifest;
use crate::sitl_multi_agent::{MultiAgentLifecycle, MultiAgentSitlManifestAgent};
use crate::sitl_plan::{SitlError, SitlWaypointItem};

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
#[cfg(test)]
use reallocation::*;

mod events;

mod artifacts;

mod validation_and_reports;
#[cfg(test)]
use validation_and_reports::*;

#[cfg(test)]
use crate::sitl_plan::first_sitl_entry;
#[cfg(test)]
use crate::sitl_report::SitlMultiAgentRunReport;

#[cfg(test)]
mod tests_cases;
#[cfg(test)]
mod tests_support;
