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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregateMetrics {
    pub total_runs: u64,
    pub success_rate: f64,
    pub avg_detection_ticks: f64,
    pub avg_reallocation_ticks: f64,
    pub avg_messages_attempted: f64,
    pub avg_messages_dropped: f64,
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
            };
        }

        let total_runs = runs.len() as u64;
        let success_count = runs.iter().filter(|run| run.success).count() as f64;
        let total_messages_attempted: u64 = runs.iter().map(|run| run.messages_attempted).sum();
        let total_messages_dropped: u64 = runs.iter().map(|run| run.messages_dropped).sum();

        Self {
            total_runs,
            success_rate: success_count / runs.len() as f64,
            avg_detection_ticks: average_optional(runs.iter().map(|run| run.detection_time_ticks)),
            avg_reallocation_ticks: average_optional(
                runs.iter().map(|run| run.reallocation_time_ticks),
            ),
            avg_messages_attempted: total_messages_attempted as f64 / runs.len() as f64,
            avg_messages_dropped: total_messages_dropped as f64 / runs.len() as f64,
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
        write!(f, "avg_messages_dropped: {:.3}", self.avg_messages_dropped)
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
}
