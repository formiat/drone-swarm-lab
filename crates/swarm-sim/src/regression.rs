#[path = "regression_parts/types_and_runner.rs"]
mod types_and_runner;
#[cfg(test)]
use types_and_runner::extract_metric;
pub use types_and_runner::*;

#[path = "regression_parts/suites_and_tests.rs"]
mod suites_and_tests;
pub use suites_and_tests::*;
