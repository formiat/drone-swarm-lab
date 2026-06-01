#[path = "sitl_observability_parts/events_and_io.rs"]
mod events_and_io;
pub use events_and_io::*;

#[cfg(test)]
#[path = "sitl_observability_parts/read_and_tests.rs"]
mod read_and_tests;
#[cfg(not(test))]
#[path = "sitl_observability_parts/read_and_tests.rs"]
mod read_and_tests;
pub use read_and_tests::*;
