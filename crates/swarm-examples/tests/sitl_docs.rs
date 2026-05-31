const README: &str = include_str!("../../../README.md");
const SITL_SETUP: &str = include_str!("../../../docs/SITL_SETUP.md");
const HARDWARE_READINESS: &str = include_str!("../../../docs/HARDWARE_READINESS.md");
const EXTENSION_GUIDE: &str = include_str!("../../../docs/EXTENSION_GUIDE.md");
const REPLAY: &str = include_str!("../../../docs/REPLAY.md");
const STATUS: &str = include_str!("../../../docs/STATUS.md");

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
    ] {
        assert!(STATUS.contains(required), "Status doc missing {required}");
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
