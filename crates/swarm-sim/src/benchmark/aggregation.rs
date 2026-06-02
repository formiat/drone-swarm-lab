use super::ComparisonReport;

pub(super) fn generate_benchmark_run_id(
    start_seed: u64,
    end_seed: u64,
    scenario_name: &str,
    prefix: Option<&str>,
) -> String {
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H%M%SZ");
    let seed_count = end_seed - start_seed;
    let mode = if seed_count <= 1 {
        "smoke"
    } else if seed_count <= 10 {
        "quick"
    } else if seed_count >= 1000 {
        "full"
    } else {
        "custom"
    };
    if let Some(p) = prefix {
        format!(
            "{}_{}_{}_{}_{}",
            p,
            timestamp,
            scenario_name,
            end_seed - start_seed,
            mode
        )
    } else {
        format!(
            "{}_{}_{}_{}",
            timestamp,
            scenario_name,
            end_seed - start_seed,
            mode
        )
    }
}

/// Generate a merged benchmark run id for `--mission all` mode.
/// Preserves prefix and timestamp from the first report, replaces mission with "all".
/// For a single report, returns the original id unchanged.
pub fn merged_benchmark_run_id(reports: &[ComparisonReport]) -> String {
    if reports.len() == 1 {
        return reports[0].benchmark_run_id.clone();
    }
    let first_id = &reports[0].benchmark_run_id;
    let parts: Vec<&str> = first_id.split('_').collect();

    // Detect prefix by checking if first part looks like a timestamp (contains 'T')
    let (prefix, timestamp) = if parts.len() >= 5 && !parts[0].contains('T') {
        // Has prefix: prefix_timestamp_mission_count_mode
        (Some(parts[0]), parts[1])
    } else if parts.len() >= 4 && parts[0].contains('T') {
        // No prefix: timestamp_mission_count_mode
        (None, parts[0])
    } else {
        // Unrecognized format: fallback to appending _all
        return format!("{}_all", first_id);
    };

    let seed_count = reports[0].total_runs_per_cell;
    let mode = if seed_count <= 1 {
        "smoke"
    } else if seed_count <= 10 {
        "quick"
    } else if seed_count >= 1000 {
        "full"
    } else {
        "custom"
    };

    if let Some(p) = prefix {
        format!("{}_{}_all_{}_{}", p, timestamp, seed_count, mode)
    } else {
        format!("{}_all_{}_{}", timestamp, seed_count, mode)
    }
}
