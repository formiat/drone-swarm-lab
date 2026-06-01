// Split into include files to keep Rust source files below the repository line limit.
include!("sitl_supervisor_parts/config_and_controllers.rs");
include!("sitl_supervisor_parts/supervisor_flows.rs");
include!("sitl_supervisor_parts/reallocation.rs");
include!("sitl_supervisor_parts/validation_and_reports.rs");

#[cfg(test)]
mod tests {
    include!("sitl_supervisor_parts/tests_support.rs");
    include!("sitl_supervisor_parts/tests_cases.rs");
}
