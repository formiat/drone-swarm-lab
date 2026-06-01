#![allow(clippy::module_inception)]

#[path = "dsl_parts/dsl.rs"]
mod dsl;
pub use dsl::*;

#[cfg(test)]
#[path = "dsl_parts/tests.rs"]
mod tests;
