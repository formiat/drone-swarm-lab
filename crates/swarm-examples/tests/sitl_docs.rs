const README: &str = include_str!("../../../README.md");
const SITL_SETUP: &str = include_str!("../../../docs/SITL_SETUP.md");
const HARDWARE_READINESS: &str = include_str!("../../../docs/HARDWARE_READINESS.md");
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
        "reallocation_completed",
        "Multi-Agent SITL Foundation",
        "scenarios/sitl.px4-golden.json",
        "scenarios/sitl.multi-agent.json",
        "scenarios/sitl.multi-agent.config.json",
        "sitl_supervisor",
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
        "Multi-Agent SITL Foundation",
        "scenarios/sitl.px4-golden.json",
        "scenarios/sitl.multi-agent.json",
        "scenarios/sitl.multi-agent.config.json",
        "sitl_supervisor",
        "--multi-agent-config",
        "duplicate ownership",
        "portable_sitl_regression_smoke",
        "sitl_observability",
        "sitl_docs",
        "external PX4",
        "Hardware Readiness Boundary",
        "docs/HARDWARE_READINESS.md",
        "--allow-hardware-candidate",
    ] {
        assert!(README.contains(required), "README missing {required}");
    }

    for required in [
        "Hardware Readiness Boundary",
        "operator checklist",
        "Physical kill switch",
        "Manual pilot override",
        "low-risk",
        "not flight certification",
        "--allow-hardware-candidate",
        "hardware_candidate",
        "scenarios/sitl.px4-golden.json",
        "scenarios/sitl.multi-agent.json",
        "scenarios/sitl.multi-agent.config.json",
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
        "reallocation_completed",
        "live multi-agent PX4 supervisor path does not yet inject failures",
        "scenarios/sitl.px4-golden.json",
    ] {
        assert!(REPLAY.contains(required), "Replay doc missing {required}");
    }

    for required in [
        "M48 Single-Agent PX4 SITL Golden Path",
        "Complete for local PX4 SIH",
        "scenarios/sitl.px4-golden.json",
        "results/m48_px4_sitl_2026-05-30",
        "M52 Multi-Agent SITL Foundation",
        "scenarios/sitl.multi-agent.json",
        "scenarios/sitl.multi-agent.config.json",
        "Prior default regression flake was not reproduced",
    ] {
        assert!(STATUS.contains(required), "Status doc missing {required}");
    }
}
