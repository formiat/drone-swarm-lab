const README: &str = include_str!("../../../README.md");
const SITL_SETUP: &str = include_str!("../../../docs/SITL_SETUP.md");
const HARDWARE_READINESS: &str = include_str!("../../../docs/HARDWARE_READINESS.md");
const EXTENSION_GUIDE: &str = include_str!("../../../docs/EXTENSION_GUIDE.md");
const REPLAY: &str = include_str!("../../../docs/REPLAY.md");
const SCENARIO_DSL: &str = include_str!("../../../docs/SCENARIO_DSL.md");
const STATUS: &str = include_str!("../../../docs/STATUS.md");
const BENCHMARK_RESULTS: &str = include_str!("../../../docs/BENCHMARK_RESULTS.md");
const PREFLIGHT_SAFETY: &str = include_str!("../../../docs/PREFLIGHT_SAFETY.md");
const ARTIFACT_VALIDATION: &str = include_str!("../../../docs/ARTIFACT_VALIDATION.md");
const M62_RESULT_README: &str =
    include_str!("../../../results/all_500_jobs14_m62_release/README.md");

#[test]
fn sitl_docs_explain_portable_and_manual_boundaries() {
    for required in [
        "--dry-run",
        "--mock",
        "--connection",
        "mavlink-transport",
        "PX4 SITL",
    ] {
        assert!(README.contains(required), "README missing {required}");
        assert!(
            SITL_SETUP.contains(required),
            "SITL setup doc missing {required}"
        );
    }

    for required in [
        "CI / Manual Boundary",
        "Real Hardware Warning",
        "Troubleshooting",
        "portable_sitl_regression_smoke",
        "task_reassigned",
        "survivor_mission_update_started",
        "survivor_mission_update_completed",
        "reallocation_completed",
        "Multi-Agent SITL Foundation",
        "Supervisor Controller Boundary",
        "MockAgentController",
        "SupervisorMetrics",
        "scenarios/sitl.px4-golden.json",
        "scenarios/sitl.multi-agent.json",
        "scenarios/sitl.multi-agent.config.json",
        "scenarios/sitl.multi-agent.execute.config.json",
        "sitl_supervisor",
        "--connection --execute",
        "--reupload-on-failure",
        "--output-dir",
        "--run-id",
        "--force",
        "M60 PX4/SIH Supervisor Hardening",
        "Stable `sitl_supervisor` exit codes",
        "Port conflicts",
        "Wrong system id",
        "not hardware-ready",
        "--multi-agent-config",
        "Duplicate ownership",
        "no external PX4",
        "Connection Classes",
        "--allow-hardware-candidate",
        "docs/HARDWARE_READINESS.md",
        "docs/PREFLIGHT_SAFETY.md",
        "safety_validation_report.v1.json",
    ] {
        assert!(
            SITL_SETUP.contains(required),
            "SITL setup doc missing {required}"
        );
    }

    for required in [
        "Portable SITL Checks",
        "Dynamic Reallocation Checks",
        "survivor_mission_update_started",
        "survivor_mission_update_completed",
        "Multi-Agent SITL Foundation",
        "Supervisor Controller Boundary",
        "AgentController",
        "SupervisorMetrics",
        "scenarios/sitl.px4-golden.json",
        "scenarios/sitl.multi-agent.json",
        "scenarios/sitl.multi-agent.config.json",
        "scenarios/sitl.multi-agent.execute.config.json",
        "sitl_supervisor",
        "--connection --execute",
        "--reupload-on-failure",
        "--output-dir",
        "--run-id",
        "--force",
        "PX4/SIH Supervisor Hardening",
        "stable exit codes",
        "task_ownership",
        "events_summary",
        "final_status",
        "limitations",
        "--multi-agent-config",
        "duplicate ownership",
        "portable_sitl_regression_smoke",
        "sitl_observability",
        "sitl_docs",
        "external PX4",
        "Hardware Readiness Boundary",
        "docs/HARDWARE_READINESS.md",
        "--allow-hardware-candidate",
        "docs/EXTENSION_GUIDE.md",
        "M70 Urban Route Export + Geo Origin",
        "--dry-run-artifact",
        "sitl_dry_run_artifact.v1",
        "no hardware readiness is claimed",
    ] {
        assert!(README.contains(required), "README missing {required}");
    }

    for required in [
        "Hardware Readiness Boundary",
        "Supervisor Controller Boundary",
        "operator checklist",
        "Physical kill switch",
        "Manual pilot override",
        "low-risk",
        "not flight certification",
        "--allow-hardware-candidate",
        "PX4/SIH supervisor hardening",
        "not hardware-ready",
        "hardware_candidate",
        "scenarios/sitl.px4-golden.json",
        "scenarios/sitl.multi-agent.json",
        "scenarios/sitl.multi-agent.config.json",
        "scenarios/sitl.multi-agent.execute.config.json",
        "Urban route export dry-run",
        "sitl_dry_run_artifact.v1",
        "Preflight safety contract",
        "SafetyValidationReport",
    ] {
        assert!(
            HARDWARE_READINESS.contains(required),
            "Hardware readiness doc missing {required}"
        );
    }

    for required in [
        "agent_lost",
        "task_released",
        "task_reassigned",
        "survivor_mission_update_started",
        "survivor_mission_update_completed",
        "reallocation_completed",
        "live multi-agent PX4 supervisor path",
        "multi_agent_run_started",
        "multi_agent_agent_started",
        "multi_agent_agent_finished",
        "multi_agent_mission_item_sent",
        "multi_agent_task_completed",
        "agent_id",
        "multi_agent_run_finished",
        "--reupload-on-failure",
        "mission_replacement",
        "replay-summary.txt",
        "events_summary",
        "task_ownership",
        "MockAgentController",
        "SupervisorMetrics",
        "scenarios/sitl.px4-golden.json",
        "docs/EXTENSION_GUIDE.md",
        "sitl_dry_run_artifact.v1",
    ] {
        assert!(REPLAY.contains(required), "Replay doc missing {required}");
    }

    for required in [
        "M48 Single-Agent PX4 SITL Golden Path",
        "Complete for local PX4 SIH",
        "scenarios/sitl.px4-golden.json",
        "results/m48_px4_sitl_2026-05-30",
        "M52 Multi-Agent SITL Foundation",
        "M57 Supervisor Controller Boundary",
        "M58 Live Multi-Agent PX4/SIH Execute Orchestration",
        "M59 Live PX4/SIH Failure & Reallocation",
        "M60 PX4/SIH Supervisor Hardening",
        "--output-dir",
        "--run-id",
        "stable exit codes",
        "scenarios/sitl.multi-agent.json",
        "scenarios/sitl.multi-agent.config.json",
        "scenarios/sitl.multi-agent.execute.config.json",
        "Default regression determinism sweep passed after fixes",
        "results/m56_regression_determinism_2026-05-30",
        "M61 Platform / API Stabilization",
        "docs/EXTENSION_GUIDE.md",
        "semver-stable public API",
        "M70 Urban Route Export + Geo Origin",
        "sitl_dry_run_artifact.v1",
    ] {
        assert!(STATUS.contains(required), "Status doc missing {required}");
    }

    for required in [
        "Scenario Geo Origin",
        "scenario.geo_origin",
        "Urban Route Export dry-run path",
        "point_index_on_segment",
    ] {
        assert!(
            SCENARIO_DSL.contains(required),
            "Scenario DSL doc missing {required}"
        );
    }

    for required in [
        "Urban Route Export Dry-Run",
        "--dry-run-artifact",
        "sitl_dry_run_artifact.v1",
        "geo_origin",
        "not a hardware run",
    ] {
        assert!(
            SITL_SETUP.contains(required),
            "SITL setup doc missing {required}"
        );
    }

    for required in ["route export adapters", "geo_origin"] {
        assert!(
            EXTENSION_GUIDE.contains(required),
            "Extension guide missing {required}"
        );
    }

    for required in ["M70 Urban Route Export", "not a benchmark refresh"] {
        assert!(
            BENCHMARK_RESULTS.contains(required),
            "Benchmark results doc missing {required}"
        );
    }
}

#[test]
fn m71_docs_describe_preflight_safety_contract() {
    for required in [
        "M71 Preflight Safety And Invariant Contract",
        "SafetyValidationReport",
        "safety_validation_report.v1.json",
        "stable exit codes use 2/3/4/5",
        "not certified flight safety",
    ] {
        assert!(STATUS.contains(required), "STATUS missing {required}");
    }

    for required in [
        "Preflight Rules",
        "geofence.waypoint_outside",
        "nofly.waypoint_inside",
        "altitude.above_max",
        "ownership.duplicate_task_id",
        "urban.blocked_edge",
        "semantics.unsupported_strategy_pair",
        "Exit Code Convention",
        "Not Certified Flight Safety",
        "What Is Not Checked",
    ] {
        assert!(
            PREFLIGHT_SAFETY.contains(required),
            "PREFLIGHT_SAFETY missing {required}"
        );
    }

    for required in [
        "Preflight Safety",
        "max_altitude_m",
        "max_route_length_m",
        "geofence.waypoint_outside",
    ] {
        assert!(
            SCENARIO_DSL.contains(required),
            "SCENARIO_DSL missing {required}"
        );
    }
}

#[test]
fn m72_docs_describe_artifact_validator_and_manual_harness() {
    for required in [
        "M72 Artifact Validator + SITL Harness",
        "artifact_validator",
        "artifact.final_status_mismatch",
        "artifact.replacement_seq_mismatch",
        "scenario.snapshot.json",
        "config.snapshot.json",
        "command.txt",
        "manual-only PX4/SIH harness",
        "not automated PX4 CI or hardware readiness",
    ] {
        assert!(STATUS.contains(required), "STATUS missing {required}");
    }

    for required in [
        "Artifact Validation",
        "artifact_validation_report.v1",
        "artifact.safety_report_missing",
        "artifact.limitations_missing",
        "scripts/run_m58_local.sh",
        "scripts/run_m59_local.sh",
        "DRY_RUN=1",
        "Manual Boundary",
    ] {
        assert!(
            ARTIFACT_VALIDATION.contains(required),
            "ARTIFACT_VALIDATION missing {required}"
        );
    }

    for required in [
        "artifact_validator",
        "docs/ARTIFACT_VALIDATION.md",
        "scripts/run_m58_local.sh",
        "scripts/run_m59_local.sh",
        "scenario.snapshot.json",
    ] {
        assert!(README.contains(required), "README missing {required}");
        assert!(
            SITL_SETUP.contains(required),
            "SITL setup doc missing {required}"
        );
    }

    for required in [
        "Artifact validation",
        "docs/ARTIFACT_VALIDATION.md",
        "not automated PX4 CI",
    ] {
        assert!(
            HARDWARE_READINESS.contains(required),
            "Hardware readiness doc missing {required}"
        );
    }

    for required in [
        "artifact_validator",
        "multi_agent_task_completed",
        "multi_agent_mission_item_sent",
        "docs/ARTIFACT_VALIDATION.md",
    ] {
        assert!(REPLAY.contains(required), "Replay doc missing {required}");
    }

    assert!(
        PREFLIGHT_SAFETY.contains("artifact.safety_report_missing"),
        "Preflight safety doc missing M72 validator rule"
    );
    for required in ["benchmark-pack validation", "future"] {
        assert!(
            BENCHMARK_RESULTS.contains(required),
            "Benchmark results doc missing {required}"
        );
    }
}

#[test]
fn m63_status_honesty_docs_mark_historical_benchmark_and_flood_scope() {
    let benchmark_commit = "81260ca7afa114a5d9add7b832f6c5d7875b88cd";

    for (name, doc) in [
        ("README", README),
        ("STATUS", STATUS),
        ("BENCHMARK_RESULTS", BENCHMARK_RESULTS),
        ("M62_RESULT_README", M62_RESULT_README),
    ] {
        assert!(
            doc.contains(benchmark_commit),
            "{name} missing benchmark commit {benchmark_commit}"
        );
    }

    for required in [
        "historical validation",
        "evidence for that commit",
        "current-HEAD evidence",
        "unless a future",
        "benchmark refresh",
        "regenerates it",
    ] {
        assert!(
            M62_RESULT_README.contains(required),
            "M62 result README missing {required}"
        );
    }

    for required in [
        "flood remains future work",
        "not implemented as a separate mission",
        "Wildfire Mapping",
    ] {
        assert!(README.contains(required), "README missing {required}");
    }
    assert!(
        !README.contains("Wildfire / Flood"),
        "README should not contain active Wildfire / Flood wording"
    );

    for required in [
        "M63 Evidence Cleanup / Status Honesty",
        "future work; not implemented as a separate mission",
        "mapped_zone_count / total_zone_count >= wildfire_success_threshold",
        "Completion = 1.000",
    ] {
        assert!(STATUS.contains(required), "STATUS missing {required}");
    }

    for required in [
        "Historical M62 Run",
        "historical evidence for the commit above",
        "mapped_zone_count / total_zone_count >= wildfire_success_threshold",
        "It is not equivalent to mission success",
    ] {
        assert!(
            BENCHMARK_RESULTS.contains(required),
            "benchmark results doc missing {required}"
        );
    }
}

#[test]
fn extension_guide_documents_platform_extension_boundaries() {
    for required in [
        "TaskKind",
        "MissionAdapter",
        "StrategyRegistry",
        "RunMetrics",
        "AggregateMetrics",
        "schema_version",
        "sitl_event_log.v1",
        "sitl_run_report.v1",
        "sitl_multi_agent_run_report.v1",
        "multi_sitl.v1",
        "not semver-stable",
        "stable-ish extension points",
        "Add A Mission",
        "Add A Strategy",
        "Add A Metric",
        "Schema Version Policy",
        "Minimal Test-Only Extension Path",
    ] {
        assert!(
            EXTENSION_GUIDE.contains(required),
            "Extension guide missing {required}"
        );
    }

    for required in ["docs/EXTENSION_GUIDE.md", "not semver-stable"] {
        assert!(README.contains(required), "README missing {required}");
    }

    for required in ["docs/EXTENSION_GUIDE.md", "schema policy"] {
        assert!(REPLAY.contains(required), "Replay doc missing {required}");
    }

    assert!(
        SITL_SETUP.contains("docs/EXTENSION_GUIDE.md"),
        "SITL setup doc missing extension guide link"
    );
}

#[test]
fn m64_docs_describe_urban_foundation_boundaries() {
    for required in [
        "Urban Patrol",
        "M65",
        "UrbanMap",
        "scenarios/urban.patrol.json",
        "scenarios/urban.search.json",
        "scenarios/urban.multi-agent.json",
        "start_node",
        "0.01m",
        "AABB static obstacle judge",
        "mocked bus detector",
        "Urban Replay / Analysis",
        "route-trace",
        "judge-report",
        "--timeline",
        "lidar",
        "dynamic obstacles",
        "multi-agent deconfliction",
        "PX4/SITL export",
    ] {
        assert!(README.contains(required), "README missing {required}");
    }

    for required in [
        "M65 Urban Patrol v0",
        "M66 Urban Search v1",
        "M67 Urban Replay / Analysis",
        "simulation-only",
        "ordered road-graph patrol",
        "mocked bus detector",
        "zero Urban judge violations",
        "diagnostic tooling",
    ] {
        assert!(STATUS.contains(required), "STATUS missing {required}");
    }

    for required in [
        "urban-patrol",
        "run_config.urban_state",
        "Dijkstra",
        "AABB-only static obstacles",
        "TaskKind::Waypoint",
        "M65",
        "M66",
        "M67",
        "urban_search_state",
        "Urban Multi-Agent Analysis Fixture",
        "scenarios/urban.multi-agent.json",
        "detector.seed",
        "Completion means",
        "start_node",
        "0.01m",
    ] {
        assert!(
            SCENARIO_DSL.contains(required),
            "Scenario DSL doc missing {required}"
        );
    }

    for required in [
        "Urban Mission Path",
        "crates/swarm-types/src/urban.rs",
        "crates/swarm-sim/src/urban.rs",
        "urban_route_planned",
        "urban_patrol_completed",
        "bus_detected",
        "UrbanSearchCompleted",
        "urban_min_agent_separation_m",
        "urban_analysis/manifest.json",
        "--category urban",
        "arbitrary polygon dependencies",
    ] {
        assert!(
            EXTENSION_GUIDE.contains(required),
            "Extension guide missing {required}"
        );
    }

    for required in [
        "UrbanRoutePlanned",
        "UrbanSegmentEntered",
        "UrbanSegmentCompleted",
        "UrbanViolation",
        "obstacle_id",
        "UrbanPatrolCompleted",
        "BusObserved",
        "BusDetected",
        "BusFalsePositive",
        "UrbanSearchCompleted",
        "--timeline",
        "--category urban",
        "urban_analysis/manifest.json",
    ] {
        assert!(REPLAY.contains(required), "Replay doc missing {required}");
    }

    for forbidden in [
        "bus detector implemented",
        "lidar implemented",
        "real perception implemented",
    ] {
        assert!(
            !README.contains(forbidden),
            "README contains stale claim {forbidden}"
        );
        assert!(
            !STATUS.contains(forbidden),
            "STATUS contains stale claim {forbidden}"
        );
    }
}
