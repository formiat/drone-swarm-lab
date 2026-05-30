const README: &str = include_str!("../../../README.md");
const SITL_SETUP: &str = include_str!("../../../docs/SITL_SETUP.md");

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
        "no external PX4",
    ] {
        assert!(
            SITL_SETUP.contains(required),
            "SITL setup doc missing {required}"
        );
    }

    for required in [
        "Portable SITL Checks",
        "Dynamic Reallocation Checks",
        "portable_sitl_regression_smoke",
        "sitl_observability",
        "sitl_docs",
        "external PX4",
    ] {
        assert!(README.contains(required), "README missing {required}");
    }
}
