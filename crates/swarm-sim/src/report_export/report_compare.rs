use super::compare::compare_aggregate_metrics;
use super::identity::{row_identity, RowIdentity};

/// Compare two [`crate::ComparisonReport`]s for metric equality, ignoring timestamps,
/// run ids, and iteration-order differences in strategy/profile names.
///
/// Returns `Ok(())` when the reports agree on all checked metrics, or `Err(msgs)` with a
/// list of human-readable mismatch descriptions. Because both inputs are expected to use the
/// same seeds in sorted order, metric values must be bit-identical — no tolerance is applied.
pub fn compare_reports(
    a: &crate::ComparisonReport,
    b: &crate::ComparisonReport,
) -> Result<(), Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    validate_report_identity("first", a, &mut errors);
    validate_report_identity("second", b, &mut errors);

    compare_string_sets(
        "mission_names",
        &a.mission_names,
        &b.mission_names,
        &mut errors,
    );
    compare_string_sets(
        "scenario_names",
        &a.scenario_names,
        &b.scenario_names,
        &mut errors,
    );
    compare_string_sets(
        "strategy_names",
        &a.strategy_names,
        &b.strategy_names,
        &mut errors,
    );
    compare_string_sets(
        "profile_names",
        &a.profile_names,
        &b.profile_names,
        &mut errors,
    );

    if a.seed_range_start != b.seed_range_start {
        errors.push(format!(
            "seed_range_start differs: {} vs {}",
            a.seed_range_start, b.seed_range_start
        ));
    }
    if a.seed_range_end != b.seed_range_end {
        errors.push(format!(
            "seed_range_end differs: {} vs {}",
            a.seed_range_end, b.seed_range_end
        ));
    }
    if a.total_runs_per_cell != b.total_runs_per_cell {
        errors.push(format!(
            "total_runs_per_cell differs: {} vs {}",
            a.total_runs_per_cell, b.total_runs_per_cell
        ));
    }

    let a_identities = sorted_report_identities(a);
    let b_identities = sorted_report_identities(b);
    if a_identities != b_identities {
        errors.push(format!(
            "row identities differ: {:?} vs {:?}",
            a_identities, b_identities
        ));
    }

    if a.results.len() != b.results.len() {
        errors.push(format!(
            "row count differs: {} vs {}",
            a.results.len(),
            b.results.len()
        ));
    }

    // Per-row metric equality.
    for key in a.results.keys() {
        match (a.results.get(key), b.results.get(key)) {
            (Some(ma), Some(mb)) => {
                compare_aggregate_metrics(key, ma, mb, &mut errors);
            }
            (Some(_), None) => {
                errors.push(format!(
                    "key {key:?} present in first report but not in second"
                ));
            }
            _ => {}
        }
    }
    for key in b.results.keys() {
        if !a.results.contains_key(key) {
            errors.push(format!(
                "key {key:?} present in second report but not in first"
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn sorted_report_identities(report: &crate::ComparisonReport) -> Vec<RowIdentity> {
    let mut identities = Vec::new();
    for strategy_name in &report.strategy_names {
        for profile_name in &report.profile_names {
            let key = (strategy_name.clone(), profile_name.clone());
            if let Some(metrics) = report.results.get(&key) {
                identities.push(row_identity(strategy_name, profile_name, metrics));
            }
        }
    }
    identities.sort();
    identities
}

fn compare_string_sets(label: &str, a: &[String], b: &[String], errors: &mut Vec<String>) {
    let mut a_sorted = a.to_vec();
    let mut b_sorted = b.to_vec();
    a_sorted.sort();
    b_sorted.sort();
    if a_sorted != b_sorted {
        errors.push(format!("{label} differ: {a_sorted:?} vs {b_sorted:?}"));
    }
}

fn validate_report_identity(
    label: &str,
    report: &crate::ComparisonReport,
    errors: &mut Vec<String>,
) {
    validate_name_list(label, "strategy_names", &report.strategy_names, errors);
    validate_name_list(label, "profile_names", &report.profile_names, errors);

    let mut visible_identities = std::collections::BTreeSet::new();
    for identity in sorted_report_identities(report) {
        if identity.mission.is_empty() {
            errors.push(format!("{label}: row {identity:?} has empty mission"));
        }
        if identity.scenario.is_empty() {
            errors.push(format!("{label}: row {identity:?} has empty scenario"));
        }
        if identity.strategy.is_empty() {
            errors.push(format!("{label}: row {identity:?} has empty strategy"));
        }
        if identity.profile.is_empty() {
            errors.push(format!("{label}: row {identity:?} has empty profile"));
        }
        if !visible_identities.insert(identity.clone()) {
            errors.push(format!("{label}: duplicate row identity {identity:?}"));
        }
    }

    for key in report.results.keys() {
        if !report.strategy_names.contains(&key.0) {
            errors.push(format!(
                "{label}: results key {key:?} uses a strategy absent from strategy_names"
            ));
        }
        if !report.profile_names.contains(&key.1) {
            errors.push(format!(
                "{label}: results key {key:?} uses a profile absent from profile_names"
            ));
        }
    }
}

fn validate_name_list(
    report_label: &str,
    field_label: &str,
    values: &[String],
    errors: &mut Vec<String>,
) {
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        if value.is_empty() {
            errors.push(format!(
                "{report_label}: {field_label} contains an empty name"
            ));
        }
        if !seen.insert(value) {
            errors.push(format!(
                "{report_label}: {field_label} contains duplicate name {value:?}"
            ));
        }
    }
}

pub(super) fn compare_metric_field<T: PartialEq + std::fmt::Debug>(
    errors: &mut Vec<String>,
    key: &(String, String),
    field: &str,
    a: &T,
    b: &T,
) {
    if a != b {
        errors.push(format!("key {key:?}: {field} {a:?} vs {b:?}"));
    }
}
