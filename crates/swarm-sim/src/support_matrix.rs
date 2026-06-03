/// Current support status for a mission/profile/strategy combination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportStatus {
    Supported,
    SupportedWithCaveats,
    Experimental,
    Unsupported,
    KnownBug,
    NotEvaluated,
}

impl SupportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::SupportedWithCaveats => "supported_with_caveats",
            Self::Experimental => "experimental",
            Self::Unsupported => "unsupported",
            Self::KnownBug => "known_bug",
            Self::NotEvaluated => "not_evaluated",
        }
    }
}

/// Machine-readable reason for the current support classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportReason {
    StableBaseline,
    OracleBaselineCaveat,
    DelayedReconvergence,
    StaticPrePlan,
    DynamicThreatDrift,
    RelayPlacementExperimental,
    ProfileConstrained,
    CorridorPlannerExperimental,
    AlgorithmDifferentiationTargeted,
    CbbaConflictDiagnostic,
    MissingEvidence,
}

impl SupportReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StableBaseline => "stable_baseline",
            Self::OracleBaselineCaveat => "oracle_baseline_caveat",
            Self::DelayedReconvergence => "delayed_reconvergence",
            Self::StaticPrePlan => "static_pre_plan",
            Self::DynamicThreatDrift => "dynamic_threat_drift",
            Self::RelayPlacementExperimental => "relay_placement_experimental",
            Self::ProfileConstrained => "profile_constrained",
            Self::CorridorPlannerExperimental => "corridor_planner_experimental",
            Self::AlgorithmDifferentiationTargeted => "algorithm_differentiation_targeted",
            Self::CbbaConflictDiagnostic => "cbba_conflict_diagnostic",
            Self::MissingEvidence => "missing_evidence",
        }
    }
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
    let mission_prefix = format!("{mission}/");
    let profile = profile.strip_prefix(&mission_prefix).unwrap_or(profile);
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
            SupportStatus::SupportedWithCaveats,
            SupportReason::DynamicThreatDrift,
        ),
        ("emergency-mesh", _, "centralized") => (
            SupportStatus::SupportedWithCaveats,
            SupportReason::OracleBaselineCaveat,
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
        (
            "coverage",
            "m77-comms-heavy-loss" | "m77-comms-partition-prone",
            "greedy" | "auction" | "connectivity-aware",
        ) => (
            SupportStatus::Experimental,
            SupportReason::AlgorithmDifferentiationTargeted,
        ),
        ("coverage", "m77-cbba-heavy-loss", "cbba") => (
            SupportStatus::KnownBug,
            SupportReason::CbbaConflictDiagnostic,
        ),
        ("sar", "m77-dynamic-belief", "greedy" | "auction" | "connectivity-aware") => (
            SupportStatus::Experimental,
            SupportReason::AlgorithmDifferentiationTargeted,
        ),
        ("wildfire", "m77-priority-realloc", "greedy" | "auction" | "connectivity-aware") => (
            SupportStatus::Experimental,
            SupportReason::AlgorithmDifferentiationTargeted,
        ),
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
    fn emergency_mesh_centralized_is_supported_with_oracle_caveat() {
        let entry = classify_support("emergency-mesh", "ideal", "centralized");
        assert_eq!(entry.status, SupportStatus::SupportedWithCaveats);
        assert_eq!(entry.reason, SupportReason::OracleBaselineCaveat);
        assert_eq!(entry.status.as_str(), "supported_with_caveats");
        assert_eq!(entry.reason.as_str(), "oracle_baseline_caveat");
    }

    #[test]
    fn mission_scoped_profiles_are_normalized_before_classification() {
        let entry = classify_support("sar", "sar/ideal", "cbba");
        assert_eq!(entry.profile, "ideal");
        assert_eq!(entry.status, SupportStatus::Unsupported);
        assert_eq!(entry.reason, SupportReason::DelayedReconvergence);
    }

    #[test]
    fn urban_corridor_aware_is_experimental_until_benchmark_refresh() {
        let entry = classify_support("urban-patrol", "corridor-delta-corridor-aware", "greedy");
        assert_eq!(entry.status, SupportStatus::Experimental);
        assert_eq!(entry.reason, SupportReason::CorridorPlannerExperimental);
    }

    #[test]
    fn m77_targeted_profiles_are_experimental() {
        let entry = classify_support("coverage", "m77-comms-heavy-loss", "auction");
        assert_eq!(entry.status, SupportStatus::Experimental);
        assert_eq!(
            entry.reason,
            SupportReason::AlgorithmDifferentiationTargeted
        );
    }

    #[test]
    fn m77_cbba_heavy_loss_is_diagnostic_known_bug() {
        let entry = classify_support("coverage", "m77-cbba-heavy-loss", "cbba");
        assert_eq!(entry.status, SupportStatus::KnownBug);
        assert_eq!(entry.reason, SupportReason::CbbaConflictDiagnostic);
    }
}
