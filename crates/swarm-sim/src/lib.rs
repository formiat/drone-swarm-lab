pub mod benchmark;
pub mod clock;
pub mod dsl;
pub mod regression;
pub mod report_export;
pub mod runner;
pub mod scenario;
pub mod support_matrix;
pub mod urban;
pub mod urban_analysis;

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
pub use urban::{
    detect_buses, expand_route_loop, judge_route, plan_route, pose_along_segment,
    UrbanBusObservation, UrbanDetectionOutcome, UrbanRouteError,
};
pub use urban_analysis::{
    build_urban_judge_report, build_urban_route_trace, count_urban_events,
    measure_urban_separation, write_urban_judge_report_csv, write_urban_judge_report_json,
    write_urban_route_trace_csv, write_urban_route_trace_json, UrbanAgentRouteTrace,
    UrbanEventCounts, UrbanJudgeReport, UrbanJudgeViolationRecord, UrbanPoseTracePoint,
    UrbanRouteConflict, UrbanRouteTrace, UrbanSegmentStatus, UrbanSeparationSummary,
    UrbanTraceSegment,
};
