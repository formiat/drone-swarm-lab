#[path = "node_parts/runtime.rs"]
mod runtime;
pub use runtime::*;

#[cfg(test)]
#[path = "node_parts/tests.rs"]
mod tests;
