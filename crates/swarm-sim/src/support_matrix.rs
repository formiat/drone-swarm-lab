/// Current support status for a mission/profile/strategy combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportStatus {
    Supported,
    Experimental,
    Unsupported,
    KnownBug,
    NotEvaluated,
}

/// Machine-readable reason for the current support classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportReason {
    StableBaseline,
    DelayedReconvergence,
    StaticPrePlan,
    DynamicThreatDrift,
    RelayPlacementExperimental,
    ProfileConstrained,
    CorridorPlannerExperimental,
    MissingEvidence,
}

/// Descriptive support-matrix entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupportMatrixEntry {
    pub mission: String,
    pub profile: String,
    pub strategy: String,
    pub status: SupportStatus,
    pub reason: SupportReason,
}

/// Classify a known mission/profile/strategy combination without running a simulation.
pub fn classify_support(mission: &str, profile: &str, strategy: &str) -> SupportMatrixEntry {
    let (status, reason) = match (mission, profile, strategy) {
        ("sar", _, "cbba") => (
            SupportStatus::Unsupported,
            SupportReason::DelayedReconvergence,
        ),
        ("sar", _, "centralized") => (SupportStatus::Unsupported, SupportReason::StaticPrePlan),
        ("sar", "ideal", "greedy") => (SupportStatus::Supported, SupportReason::StableBaseline),
        ("inspection", "linear", "greedy") => {
            (SupportStatus::Supported, SupportReason::StableBaseline)
        }
        ("inspection", "perimeter", "greedy") => (
            SupportStatus::Experimental,
            SupportReason::ProfileConstrained,
        ),
        ("wildfire", "small-static", "greedy") => {
            (SupportStatus::Supported, SupportReason::StableBaseline)
        }
        ("wildfire", "medium-dynamic", "greedy") => (
            SupportStatus::Experimental,
            SupportReason::DynamicThreatDrift,
        ),
        ("urban-patrol", "patrol-small-block", "greedy") => {
            (SupportStatus::Supported, SupportReason::StableBaseline)
        }
        ("urban-patrol", "corridor-delta-dijkstra", "greedy") => {
            (SupportStatus::Supported, SupportReason::StableBaseline)
        }
        ("urban-patrol", "corridor-delta-corridor-aware", "greedy") => (
            SupportStatus::Experimental,
            SupportReason::CorridorPlannerExperimental,
        ),
        ("urban-search", "search-static-bus", "greedy") => {
            (SupportStatus::Supported, SupportReason::StableBaseline)
        }
        ("emergency-mesh", _, "connectivity-aware") => (
            SupportStatus::Experimental,
            SupportReason::RelayPlacementExperimental,
        ),
        _ => (SupportStatus::NotEvaluated, SupportReason::MissingEvidence),
    };

    SupportMatrixEntry {
        mission: mission.to_owned(),
        profile: profile.to_owned(),
        strategy: strategy.to_owned(),
        status,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sar_cbba_is_unsupported_delayed_reconvergence() {
        let entry = classify_support("sar", "ideal", "cbba");
        assert_eq!(entry.status, SupportStatus::Unsupported);
        assert_eq!(entry.reason, SupportReason::DelayedReconvergence);
    }

    #[test]
    fn sar_centralized_is_unsupported_static_pre_plan() {
        let entry = classify_support("sar", "ideal", "centralized");
        assert_eq!(entry.status, SupportStatus::Unsupported);
        assert_eq!(entry.reason, SupportReason::StaticPrePlan);
    }

    #[test]
    fn emergency_mesh_connectivity_aware_is_experimental() {
        let entry = classify_support("emergency-mesh", "ideal", "connectivity-aware");
        assert_eq!(entry.status, SupportStatus::Experimental);
        assert_eq!(entry.reason, SupportReason::RelayPlacementExperimental);
    }

    #[test]
    fn urban_corridor_aware_is_experimental_until_benchmark_refresh() {
        let entry = classify_support("urban-patrol", "corridor-delta-corridor-aware", "greedy");
        assert_eq!(entry.status, SupportStatus::Experimental);
        assert_eq!(entry.reason, SupportReason::CorridorPlannerExperimental);
    }
}
