use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunMetrics {
    pub seed: u64,
    pub total_ticks: u64,
    pub messages_attempted: u64,
    pub messages_dropped: u64,
    pub detection_time_ticks: Option<u64>,
    pub reallocation_time_ticks: Option<u64>,
    pub max_task_unassigned_ticks: u64,
    pub all_tasks_assigned: bool,
    pub success: bool,
    pub tasks_injected: u64,
    pub tasks_expired: u64,
    pub conflicting_assignments: u64,
    pub partition_events: u64,
    pub partitions_active: bool,
    pub stale_messages_discarded: u64,
    pub convergence_ticks: Option<u64>,
    pub max_view_divergence: u64,
    // v0.5 network availability metrics
    pub network_availability: f64,
    pub relay_reallocation_ticks: Option<u64>,
    pub avg_hop_count: f64,
    pub disconnected_agents_max: u64,
    // v0.6 strategy comparison metrics
    #[serde(default)]
    pub coverage_progress: f64,
    #[serde(default)]
    pub bytes_sent: u64,
    #[serde(default)]
    pub stale_state_age_ticks: u64,
    #[serde(default)]
    pub battery_margin_min: f64,
    #[serde(default)]
    pub battery_margin_avg: f64,
    // v0.8 kinematic metrics
    #[serde(default)]
    pub final_battery_min: f64,
    #[serde(default)]
    pub avg_distance_travelled: f64,
    #[serde(default)]
    pub agents_exhausted: u64,
    #[serde(default)]
    pub total_distance_travelled: f64,
    #[serde(default)]
    pub mission_completion_ticks: u64,
    #[serde(default)]
    pub time_to_first_exhaustion: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregateMetrics {
    pub total_runs: u64,
    pub success_rate: f64,
    pub avg_detection_ticks: f64,
    pub avg_reallocation_ticks: f64,
    pub avg_messages_attempted: f64,
    pub avg_messages_dropped: f64,
    pub avg_tasks_injected: f64,
    pub avg_tasks_expired: f64,
    pub avg_conflicting_assignments: f64,
    // v0.5
    pub avg_network_availability: f64,
    pub avg_relay_reallocation_ticks: f64,
    pub avg_avg_hop_count: f64,
    pub avg_disconnected_agents_max: f64,
    // v0.6
    pub avg_coverage_progress: f64,
    pub avg_bytes_sent: f64,
    pub avg_stale_state_age_ticks: f64,
    pub avg_battery_margin_min: f64,
    pub avg_battery_margin_avg: f64,
    pub avg_task_completion_rate: f64,
}

impl AggregateMetrics {
    pub fn from_runs(runs: &[RunMetrics]) -> Self {
        if runs.is_empty() {
            return Self {
                total_runs: 0,
                success_rate: 0.0,
                avg_detection_ticks: 0.0,
                avg_reallocation_ticks: 0.0,
                avg_messages_attempted: 0.0,
                avg_messages_dropped: 0.0,
                avg_tasks_injected: 0.0,
                avg_tasks_expired: 0.0,
                avg_conflicting_assignments: 0.0,
                avg_network_availability: 0.0,
                avg_relay_reallocation_ticks: 0.0,
                avg_avg_hop_count: 0.0,
                avg_disconnected_agents_max: 0.0,
                avg_coverage_progress: 0.0,
                avg_bytes_sent: 0.0,
                avg_stale_state_age_ticks: 0.0,
                avg_battery_margin_min: 0.0,
                avg_battery_margin_avg: 0.0,
                avg_task_completion_rate: 0.0,
            };
        }

        let total_runs = runs.len() as u64;
        let success_count = runs.iter().filter(|run| run.success).count() as f64;
        let total_messages_attempted: u64 = runs.iter().map(|run| run.messages_attempted).sum();
        let total_messages_dropped: u64 = runs.iter().map(|run| run.messages_dropped).sum();
        let total_tasks_injected: u64 = runs.iter().map(|run| run.tasks_injected).sum();
        let total_tasks_expired: u64 = runs.iter().map(|run| run.tasks_expired).sum();
        let total_conflicting: u64 = runs.iter().map(|run| run.conflicting_assignments).sum();
        let total_network_availability: f64 = runs.iter().map(|run| run.network_availability).sum();
        let total_avg_hop_count: f64 = runs.iter().map(|run| run.avg_hop_count).sum();
        let total_disconnected_max: u64 = runs.iter().map(|run| run.disconnected_agents_max).sum();
        let total_coverage_progress: f64 = runs.iter().map(|run| run.coverage_progress).sum();
        let total_bytes_sent: u64 = runs.iter().map(|run| run.bytes_sent).sum();
        let total_stale_state_age: u64 = runs.iter().map(|run| run.stale_state_age_ticks).sum();
        let total_battery_margin_min: f64 = runs.iter().map(|run| run.battery_margin_min).sum();
        let total_battery_margin_avg: f64 = runs.iter().map(|run| run.battery_margin_avg).sum();
        let task_completion_count = runs.iter().filter(|run| run.all_tasks_assigned).count() as f64;
        let n = runs.len() as f64;

        Self {
            total_runs,
            success_rate: success_count / n,
            avg_detection_ticks: average_optional(runs.iter().map(|run| run.detection_time_ticks)),
            avg_reallocation_ticks: average_optional(
                runs.iter().map(|run| run.reallocation_time_ticks),
            ),
            avg_messages_attempted: total_messages_attempted as f64 / n,
            avg_messages_dropped: total_messages_dropped as f64 / n,
            avg_tasks_injected: total_tasks_injected as f64 / n,
            avg_tasks_expired: total_tasks_expired as f64 / n,
            avg_conflicting_assignments: total_conflicting as f64 / n,
            avg_network_availability: total_network_availability / n,
            avg_relay_reallocation_ticks: average_optional(
                runs.iter().map(|run| run.relay_reallocation_ticks),
            ),
            avg_avg_hop_count: total_avg_hop_count / n,
            avg_disconnected_agents_max: total_disconnected_max as f64 / n,
            avg_coverage_progress: total_coverage_progress / n,
            avg_bytes_sent: total_bytes_sent as f64 / n,
            avg_stale_state_age_ticks: total_stale_state_age as f64 / n,
            avg_battery_margin_min: total_battery_margin_min / n,
            avg_battery_margin_avg: total_battery_margin_avg / n,
            avg_task_completion_rate: task_completion_count / n,
        }
    }
}

impl fmt::Display for AggregateMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "runs: {}", self.total_runs)?;
        writeln!(f, "success_rate: {:.3}", self.success_rate)?;
        writeln!(f, "avg_detection_ticks: {:.3}", self.avg_detection_ticks)?;
        writeln!(
            f,
            "avg_reallocation_ticks: {:.3}",
            self.avg_reallocation_ticks
        )?;
        writeln!(
            f,
            "avg_messages_attempted: {:.3}",
            self.avg_messages_attempted
        )?;
        writeln!(f, "avg_messages_dropped: {:.3}", self.avg_messages_dropped)?;
        writeln!(f, "avg_tasks_injected: {:.3}", self.avg_tasks_injected)?;
        writeln!(f, "avg_tasks_expired: {:.3}", self.avg_tasks_expired)?;
        writeln!(
            f,
            "avg_conflicting_assignments: {:.3}",
            self.avg_conflicting_assignments
        )?;
        writeln!(
            f,
            "avg_network_availability: {:.3}",
            self.avg_network_availability
        )?;
        writeln!(
            f,
            "avg_relay_reallocation_ticks: {:.3}",
            self.avg_relay_reallocation_ticks
        )?;
        writeln!(f, "avg_avg_hop_count: {:.3}", self.avg_avg_hop_count)?;
        writeln!(
            f,
            "avg_disconnected_agents_max: {:.3}",
            self.avg_disconnected_agents_max
        )?;
        writeln!(
            f,
            "avg_coverage_progress: {:.3}",
            self.avg_coverage_progress
        )?;
        writeln!(f, "avg_bytes_sent: {:.3}", self.avg_bytes_sent)?;
        writeln!(
            f,
            "avg_stale_state_age_ticks: {:.3}",
            self.avg_stale_state_age_ticks
        )?;
        writeln!(
            f,
            "avg_battery_margin_min: {:.3}",
            self.avg_battery_margin_min
        )?;
        writeln!(
            f,
            "avg_battery_margin_avg: {:.3}",
            self.avg_battery_margin_avg
        )?;
        write!(
            f,
            "avg_task_completion_rate: {:.3}",
            self.avg_task_completion_rate
        )
    }
}

fn average_optional(values: impl Iterator<Item = Option<u64>>) -> f64 {
    let mut count = 0_u64;
    let mut sum = 0_u64;

    for value in values.flatten() {
        count += 1;
        sum += value;
    }

    if count == 0 {
        0.0
    } else {
        sum as f64 / count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(success: bool, detection_time_ticks: Option<u64>) -> RunMetrics {
        RunMetrics {
            seed: 0,
            total_ticks: 10,
            messages_attempted: 10,
            messages_dropped: 2,
            detection_time_ticks,
            reallocation_time_ticks: Some(1),
            max_task_unassigned_ticks: 1,
            all_tasks_assigned: success,
            success,
            tasks_injected: 0,
            tasks_expired: 0,
            conflicting_assignments: 0,
            partition_events: 0,
            partitions_active: false,
            stale_messages_discarded: 0,
            convergence_ticks: None,
            max_view_divergence: 0,
            network_availability: 1.0,
            relay_reallocation_ticks: None,
            avg_hop_count: 0.0,
            disconnected_agents_max: 0,
            coverage_progress: 0.0,
            bytes_sent: 0,
            stale_state_age_ticks: 0,
            battery_margin_min: 0.0,
            battery_margin_avg: 0.0,
            final_battery_min: 0.0,
            avg_distance_travelled: 0.0,
            agents_exhausted: 0,
            total_distance_travelled: 0.0,
            mission_completion_ticks: 0,
            time_to_first_exhaustion: None,
        }
    }

    #[test]
    fn aggregate_success_rate() {
        let mut runs = Vec::new();
        for _ in 0..8 {
            runs.push(run(true, Some(2)));
        }
        for _ in 0..2 {
            runs.push(run(false, Some(4)));
        }

        let metrics = AggregateMetrics::from_runs(&runs);

        assert_eq!(metrics.success_rate, 0.8);
    }

    #[test]
    fn aggregate_avg_detection() {
        let runs = vec![run(true, Some(2)), run(true, Some(4)), run(true, None)];

        let metrics = AggregateMetrics::from_runs(&runs);

        assert_eq!(metrics.avg_detection_ticks, 3.0);
    }

    #[test]
    fn aggregate_avg_tasks_injected() {
        let mut runs = vec![run(true, None), run(true, None), run(true, None)];
        runs[0].tasks_injected = 3;
        runs[1].tasks_injected = 6;
        runs[2].tasks_injected = 0;

        let metrics = AggregateMetrics::from_runs(&runs);

        assert_eq!(metrics.avg_tasks_injected, 3.0);
    }

    #[test]
    fn aggregate_avg_tasks_expired() {
        let mut runs = vec![run(true, None), run(true, None)];
        runs[0].tasks_expired = 2;
        runs[1].tasks_expired = 4;

        let metrics = AggregateMetrics::from_runs(&runs);

        assert_eq!(metrics.avg_tasks_expired, 3.0);
    }

    #[test]
    fn aggregate_avg_task_completion_rate() {
        let runs = vec![run(true, None), run(true, None), run(false, None)];
        // 2 out of 3 runs have all_tasks_assigned=true (set by run(success, ...))
        // The third run has success=false, so all_tasks_assigned=false
        let metrics = AggregateMetrics::from_runs(&runs);

        assert!(
            (metrics.avg_task_completion_rate - 0.666_666_7).abs() < 1e-6,
            "Expected ~0.666667 for 2/3 completed runs, got {}",
            metrics.avg_task_completion_rate
        );
    }
}
