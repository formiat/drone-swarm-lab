#![allow(unused_imports)]
#[cfg(feature = "mavlink-transport")]
use std::borrow::Cow;
use std::collections::VecDeque;
#[cfg(feature = "mavlink-transport")]
use std::io::ErrorKind;
#[cfg(feature = "mavlink-transport")]
use std::time::{Duration, Instant};

use swarm_types::TaskStatus;

use crate::{RawMessage, Transport};

#[cfg(feature = "mavlink-transport")]
use mavlink::dialects::common;

#[cfg(feature = "mavlink-transport")]
type CommonHeader = mavlink::MavHeader;
#[cfg(feature = "mavlink-transport")]
type CommonMessage = common::MavMessage;

mod types_and_transport;
use types_and_transport::*;
pub use types_and_transport::*;

#[cfg(feature = "mavlink-transport")]
mod mission_execution;
#[cfg(feature = "mavlink-transport")]
pub use mission_execution::mavlink_message_to_telemetry_event;
#[cfg(feature = "mavlink-transport")]
use mission_execution::*;

#[cfg(feature = "mavlink-transport")]
mod commands_and_conversion;
#[cfg(feature = "mavlink-transport")]
pub use commands_and_conversion::*;
#[cfg(feature = "mavlink-transport")]
use commands_and_conversion::*;

#[cfg(test)]
mod tests_core;
#[cfg(all(test, feature = "mavlink-transport"))]
mod tests_mission_upload;
