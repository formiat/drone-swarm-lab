use swarm_metrics::AggregateMetrics;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct RowIdentity {
    pub(super) mission: String,
    pub(super) scenario: String,
    pub(super) strategy: String,
    pub(super) profile: String,
}

pub(super) fn row_identity(
    strategy_name: &str,
    profile_name: &str,
    metrics: &AggregateMetrics,
) -> RowIdentity {
    RowIdentity {
        mission: metrics.mission.clone(),
        scenario: metrics.scenario.clone(),
        strategy: strategy_name.to_owned(),
        profile: profile_name.to_owned(),
    }
}
