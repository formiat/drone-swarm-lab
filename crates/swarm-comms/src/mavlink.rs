// Split into include files to keep Rust source files below the repository line limit.
include!("mavlink_parts/types_and_transport.rs");
include!("mavlink_parts/mission_execution.rs");
include!("mavlink_parts/commands_and_conversion.rs");
include!("mavlink_parts/tests_core.rs");
include!("mavlink_parts/tests_mission_upload.rs");
