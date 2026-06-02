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

mod errors;
pub use errors::*;

mod types;
use types::*;
pub use types::*;

#[cfg(feature = "mavlink-transport")]
mod observer;
#[cfg(feature = "mavlink-transport")]
pub use observer::*;
#[cfg(feature = "mavlink-transport")]
use observer::*;

mod transport;
pub use transport::*;
use transport::*;

#[cfg(feature = "mavlink-transport")]
mod mission_upload;
#[cfg(feature = "mavlink-transport")]
use mission_upload::*;

#[cfg(feature = "mavlink-transport")]
mod commands;
#[cfg(feature = "mavlink-transport")]
pub use commands::*;
#[cfg(feature = "mavlink-transport")]
use commands::*;

#[cfg(feature = "mavlink-transport")]
mod mission_items;
#[cfg(feature = "mavlink-transport")]
pub use mission_items::*;
#[cfg(feature = "mavlink-transport")]
use mission_items::*;

#[cfg(feature = "mavlink-transport")]
mod telemetry;
#[cfg(feature = "mavlink-transport")]
pub use telemetry::*;
#[cfg(feature = "mavlink-transport")]
use telemetry::*;

#[cfg(feature = "mavlink-transport")]
mod lifecycle;
#[cfg(feature = "mavlink-transport")]
use lifecycle::*;

#[cfg(test)]
mod tests_core;
#[cfg(all(test, feature = "mavlink-transport"))]
mod tests_mission_upload;
