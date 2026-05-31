pub mod benchmark;
pub mod clock;
pub mod dsl;
pub mod regression;
pub mod report_export;
pub mod runner;
pub mod scenario;
pub mod support_matrix;
pub mod urban;

pub use benchmark::{
    merged_benchmark_run_id, BenchmarkHarness, BenchmarkOptions, BenchmarkResult, ComparisonReport,
};
pub use clock::{Clock, Tick};
pub use dsl::{
    export_entry, export_suite, load_scenario_suite, validate_entry, validate_mission_specific,
    validate_scenario_suite, ScenarioSuite, ScenarioSuiteEntry, ValidationError,
};
pub use regression::{
    all_suites, default_suites, suites_by_group, Baseline, BaselineDelta, DeltaStatus,
    RegressionReport, RegressionRunner, RegressionSuite, SuiteGroup, SuiteMode, SuiteResult,
    Threshold, ThresholdChecker, ThresholdViolation,
};
pub use report_export::{
    compare_reports, export_csv, export_json, export_markdown, generate_focused_report,
    BenchmarkManifest,
};
pub use runner::{
    DynamicTaskEvent, FailureEvent, InspectionState, PartitionEvent, RunConfig, ScenarioRunner,
    UrbanState, WildfireState, WildfireZone,
};
pub use scenario::Scenario;
pub use support_matrix::{classify_support, SupportMatrixEntry, SupportReason, SupportStatus};
pub use urban::{expand_route_loop, judge_route, plan_route, pose_along_segment, UrbanRouteError};
