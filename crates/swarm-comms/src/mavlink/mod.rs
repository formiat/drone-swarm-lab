#[cfg(all(feature = "mavlink-transport", test))]
use std::time::Duration;

#[cfg(test)]
use crate::{RawMessage, Transport};

#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;

#[cfg(feature = "mavlink-transport")]
type CommonHeader = mavlink::MavHeader;
#[cfg(feature = "mavlink-transport")]
type CommonMessage = common::MavMessage;

mod errors;
pub use errors::*;

mod types;
pub use types::*;

#[cfg(feature = "mavlink-transport")]
mod observer;
#[cfg(feature = "mavlink-transport")]
pub use observer::*;

mod transport;
pub use transport::*;

#[cfg(feature = "mavlink-transport")]
mod mission_upload;
#[cfg(all(feature = "mavlink-transport", test))]
use mission_upload::*;

#[cfg(feature = "mavlink-transport")]
mod commands;
#[cfg(feature = "mavlink-transport")]
pub use commands::*;

#[cfg(feature = "mavlink-transport")]
mod mission_items;
#[cfg(feature = "mavlink-transport")]
pub use mission_items::*;

#[cfg(feature = "mavlink-transport")]
mod telemetry;
#[cfg(feature = "mavlink-transport")]
pub use telemetry::*;

#[cfg(feature = "mavlink-transport")]
mod lifecycle;
#[cfg(all(feature = "mavlink-transport", test))]
use lifecycle::*;

#[cfg(test)]
mod tests_core;
#[cfg(all(test, feature = "mavlink-transport"))]
mod tests_mission_upload;
