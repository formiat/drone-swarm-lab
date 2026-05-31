use std::collections::BTreeMap;
use std::time::Duration;

use swarm_comms::MavlinkTelemetryEvent;
use swarm_types::TaskStatus;

use crate::sitl_plan::SitlPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SitlMissionFinalStatus {
    Completed,
    Failed,
    Disconnected,
    Rejected,
    TimedOutNoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitlMissionProgressReport {
    pub final_status: SitlMissionFinalStatus,
    pub total_tasks: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub current_task_id: Option<String>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SitlProgressUpdate {
    Heartbeat,
    Current {
        seq: u16,
        task_id: String,
        completed_count: usize,
        total_count: usize,
    },
    Reached {
        seq: u16,
        task_id: String,
        completed_count: usize,
        total_count: usize,
    },
    Completed(SitlMissionProgressReport),
    Failed(SitlMissionProgressReport),
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SitlProgressError {
    #[error("SITL mission progress requires at least one waypoint task")]
    EmptyMission,
    #[error("duplicate mission seq in SITL plan: {seq}")]
    DuplicateMissionSeq { seq: u16 },
    #[error("unknown mission seq in telemetry: {seq}")]
    UnknownMissionSeq { seq: u16 },
}

/// Tracks single-agent SITL waypoint progress and maps mission seqs to task ids.
#[derive(Debug, Clone)]
pub struct SitlTaskProgress {
    /// key: `mission_item_seq`
    seq_to_task_id: BTreeMap<u16, String>,
    /// key: `task_id`
    task_status_by_id: BTreeMap<String, TaskStatus>,
    current_seq: Option<u16>,
    last_heartbeat: Option<Duration>,
    last_progress: Option<Duration>,
    final_status: Option<SitlMissionFinalStatus>,
    failure_reason: Option<String>,
}

impl SitlTaskProgress {
    pub fn from_plan(plan: &SitlPlan) -> Result<Self, SitlProgressError> {
        Self::from_waypoints(
            plan.waypoints
                .iter()
                .map(|waypoint| (waypoint.seq, waypoint.task_id.clone())),
        )
    }

    pub fn from_waypoints<I, S>(waypoints: I) -> Result<Self, SitlProgressError>
    where
        I: IntoIterator<Item = (u16, S)>,
        S: Into<String>,
    {
        let mut seq_to_task_id = BTreeMap::new();
        let mut task_status_by_id = BTreeMap::new();

        for (seq, task_id) in waypoints {
            let task_id = task_id.into();
            if seq_to_task_id.insert(seq, task_id.clone()).is_some() {
                return Err(SitlProgressError::DuplicateMissionSeq { seq });
            }
            task_status_by_id.insert(task_id, TaskStatus::Unassigned);
        }

        if seq_to_task_id.is_empty() {
            return Err(SitlProgressError::EmptyMission);
        }

        Ok(Self {
            seq_to_task_id,
            task_status_by_id,
            current_seq: None,
            last_heartbeat: None,
            last_progress: None,
            final_status: None,
            failure_reason: None,
        })
    }

    pub fn apply_event(
        &mut self,
        event: MavlinkTelemetryEvent,
        now: Duration,
    ) -> Result<SitlProgressUpdate, SitlProgressError> {
        match event {
            MavlinkTelemetryEvent::Heartbeat => {
                self.last_heartbeat = Some(now);
                Ok(SitlProgressUpdate::Heartbeat)
            }
            MavlinkTelemetryEvent::MissionCurrent { seq } => {
                let task_id = self.task_id_for_seq(seq)?.to_owned();
                self.current_seq = Some(seq);
                self.last_progress = Some(now);
                if let Some(status) = self.task_status_by_id.get_mut(&task_id) {
                    if !matches!(status, TaskStatus::Completed | TaskStatus::Failed) {
                        *status = TaskStatus::InProgress;
                    }
                }
                Ok(SitlProgressUpdate::Current {
                    seq,
                    task_id,
                    completed_count: self.completed_count(),
                    total_count: self.total_count(),
                })
            }
            MavlinkTelemetryEvent::WaypointReached { seq } => {
                let task_id = self.task_id_for_seq(seq)?.to_owned();
                self.current_seq = Some(seq);
                self.last_progress = Some(now);
                if let Some(status) = self.task_status_by_id.get_mut(&task_id) {
                    *status = TaskStatus::Completed;
                }
                if self.completed_count() == self.total_count() {
                    self.final_status = Some(SitlMissionFinalStatus::Completed);
                    return Ok(SitlProgressUpdate::Completed(self.final_report()));
                }
                Ok(SitlProgressUpdate::Reached {
                    seq,
                    task_id,
                    completed_count: self.completed_count(),
                    total_count: self.total_count(),
                })
            }
            MavlinkTelemetryEvent::MissionComplete => {
                self.last_progress = Some(now);
                if self.completed_count() == self.total_count() {
                    self.final_status = Some(SitlMissionFinalStatus::Completed);
                    Ok(SitlProgressUpdate::Completed(self.final_report()))
                } else {
                    let report = self.mark_failed(
                        SitlMissionFinalStatus::Failed,
                        "mission complete before all waypoint tasks were reached",
                    );
                    Ok(SitlProgressUpdate::Failed(report))
                }
            }
            MavlinkTelemetryEvent::MissionRejected { reason } => {
                let report = self.mark_failed(SitlMissionFinalStatus::Rejected, reason);
                Ok(SitlProgressUpdate::Failed(report))
            }
            MavlinkTelemetryEvent::Disconnected => {
                let report = self.mark_failed(
                    SitlMissionFinalStatus::Disconnected,
                    "telemetry disconnected",
                );
                Ok(SitlProgressUpdate::Failed(report))
            }
        }
    }

    pub fn mark_no_progress_timeout(
        &mut self,
        reason: impl Into<String>,
    ) -> SitlMissionProgressReport {
        self.mark_failed(SitlMissionFinalStatus::TimedOutNoProgress, reason)
    }

    pub fn task_status(&self, task_id: &str) -> Option<&TaskStatus> {
        self.task_status_by_id.get(task_id)
    }

    pub fn current_seq(&self) -> Option<u16> {
        self.current_seq
    }

    pub fn last_heartbeat(&self) -> Option<Duration> {
        self.last_heartbeat
    }

    pub fn last_progress(&self) -> Option<Duration> {
        self.last_progress
    }

    pub fn completed_count(&self) -> usize {
        self.task_status_by_id
            .values()
            .filter(|status| matches!(status, TaskStatus::Completed))
            .count()
    }

    pub fn completed_task_ids(&self) -> Vec<String> {
        self.completed_waypoints()
            .into_iter()
            .map(|(_, task_id)| task_id)
            .collect()
    }

    pub fn completed_waypoints(&self) -> Vec<(u16, String)> {
        self.seq_to_task_id
            .iter()
            .filter(|(_, task_id)| {
                self.task_status_by_id
                    .get(*task_id)
                    .is_some_and(|status| matches!(status, TaskStatus::Completed))
            })
            .map(|(seq, task_id)| (*seq, task_id.clone()))
            .collect()
    }

    pub fn failed_count(&self) -> usize {
        self.task_status_by_id
            .values()
            .filter(|status| matches!(status, TaskStatus::Failed))
            .count()
    }

    pub fn total_count(&self) -> usize {
        self.task_status_by_id.len()
    }

    fn task_id_for_seq(&self, seq: u16) -> Result<&str, SitlProgressError> {
        self.seq_to_task_id
            .get(&seq)
            .map(String::as_str)
            .ok_or(SitlProgressError::UnknownMissionSeq { seq })
    }

    fn mark_failed(
        &mut self,
        final_status: SitlMissionFinalStatus,
        reason: impl Into<String>,
    ) -> SitlMissionProgressReport {
        self.mark_incomplete_failed();
        self.final_status = Some(final_status);
        self.failure_reason = Some(reason.into());
        self.final_report()
    }

    fn mark_incomplete_failed(&mut self) {
        for status in self.task_status_by_id.values_mut() {
            if !matches!(status, TaskStatus::Completed) {
                *status = TaskStatus::Failed;
            }
        }
    }

    fn final_report(&self) -> SitlMissionProgressReport {
        SitlMissionProgressReport {
            final_status: self.final_status.unwrap_or(SitlMissionFinalStatus::Failed),
            total_tasks: self.total_count(),
            completed_count: self.completed_count(),
            failed_count: self.failed_count(),
            current_task_id: self
                .current_seq
                .and_then(|seq| self.seq_to_task_id.get(&seq).cloned()),
            failure_reason: self.failure_reason.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress() -> SitlTaskProgress {
        SitlTaskProgress::from_waypoints([(0, "wp-0"), (1, "wp-1"), (2, "wp-2")]).unwrap()
    }

    #[test]
    fn mission_current_maps_seq_to_task_id_and_in_progress() {
        let mut progress = progress();

        let update = progress
            .apply_event(
                MavlinkTelemetryEvent::MissionCurrent { seq: 1 },
                Duration::from_secs(1),
            )
            .unwrap();

        assert_eq!(
            update,
            SitlProgressUpdate::Current {
                seq: 1,
                task_id: "wp-1".to_owned(),
                completed_count: 0,
                total_count: 3,
            }
        );
        assert_eq!(progress.task_status("wp-1"), Some(&TaskStatus::InProgress));
        assert_eq!(progress.current_seq(), Some(1));
    }

    #[test]
    fn waypoint_reached_marks_task_completed() {
        let mut progress = progress();

        let update = progress
            .apply_event(
                MavlinkTelemetryEvent::WaypointReached { seq: 0 },
                Duration::from_secs(2),
            )
            .unwrap();

        assert_eq!(
            update,
            SitlProgressUpdate::Reached {
                seq: 0,
                task_id: "wp-0".to_owned(),
                completed_count: 1,
                total_count: 3,
            }
        );
        assert_eq!(progress.task_status("wp-0"), Some(&TaskStatus::Completed));
    }

    #[test]
    fn all_waypoints_reached_returns_completed_report() {
        let mut progress = progress();

        progress
            .apply_event(
                MavlinkTelemetryEvent::WaypointReached { seq: 0 },
                Duration::from_secs(1),
            )
            .unwrap();
        progress
            .apply_event(
                MavlinkTelemetryEvent::WaypointReached { seq: 1 },
                Duration::from_secs(2),
            )
            .unwrap();
        let update = progress
            .apply_event(
                MavlinkTelemetryEvent::WaypointReached { seq: 2 },
                Duration::from_secs(3),
            )
            .unwrap();

        assert_eq!(
            update,
            SitlProgressUpdate::Completed(SitlMissionProgressReport {
                final_status: SitlMissionFinalStatus::Completed,
                total_tasks: 3,
                completed_count: 3,
                failed_count: 0,
                current_task_id: Some("wp-2".to_owned()),
                failure_reason: None,
            })
        );
    }

    #[test]
    fn rejected_mission_marks_incomplete_tasks_failed() {
        let mut progress = progress();
        progress
            .apply_event(
                MavlinkTelemetryEvent::WaypointReached { seq: 0 },
                Duration::from_secs(1),
            )
            .unwrap();

        let update = progress
            .apply_event(
                MavlinkTelemetryEvent::MissionRejected {
                    reason: "MAV_MISSION_ERROR".to_owned(),
                },
                Duration::from_secs(2),
            )
            .unwrap();

        assert_eq!(progress.task_status("wp-0"), Some(&TaskStatus::Completed));
        assert_eq!(progress.task_status("wp-1"), Some(&TaskStatus::Failed));
        assert_eq!(progress.task_status("wp-2"), Some(&TaskStatus::Failed));
        assert!(matches!(
            update,
            SitlProgressUpdate::Failed(SitlMissionProgressReport {
                final_status: SitlMissionFinalStatus::Rejected,
                completed_count: 1,
                failed_count: 2,
                ..
            })
        ));
    }

    #[test]
    fn disconnected_marks_active_and_incomplete_tasks_failed() {
        let mut progress = progress();
        progress
            .apply_event(
                MavlinkTelemetryEvent::MissionCurrent { seq: 1 },
                Duration::from_secs(1),
            )
            .unwrap();

        let update = progress
            .apply_event(MavlinkTelemetryEvent::Disconnected, Duration::from_secs(2))
            .unwrap();

        assert_eq!(progress.task_status("wp-0"), Some(&TaskStatus::Failed));
        assert_eq!(progress.task_status("wp-1"), Some(&TaskStatus::Failed));
        assert_eq!(progress.task_status("wp-2"), Some(&TaskStatus::Failed));
        assert!(matches!(
            update,
            SitlProgressUpdate::Failed(SitlMissionProgressReport {
                final_status: SitlMissionFinalStatus::Disconnected,
                failed_count: 3,
                ..
            })
        ));
    }

    #[test]
    fn out_of_range_seq_returns_typed_error() {
        let mut progress = progress();

        let error = progress
            .apply_event(
                MavlinkTelemetryEvent::MissionCurrent { seq: 9 },
                Duration::from_secs(1),
            )
            .unwrap_err();

        assert_eq!(error, SitlProgressError::UnknownMissionSeq { seq: 9 });
    }

    #[test]
    fn duplicate_waypoint_reached_is_idempotent() {
        let mut progress = progress();

        progress
            .apply_event(
                MavlinkTelemetryEvent::WaypointReached { seq: 0 },
                Duration::from_secs(1),
            )
            .unwrap();
        progress
            .apply_event(
                MavlinkTelemetryEvent::WaypointReached { seq: 0 },
                Duration::from_secs(2),
            )
            .unwrap();

        assert_eq!(progress.completed_count(), 1);
        assert_eq!(progress.task_status("wp-0"), Some(&TaskStatus::Completed));
    }

    #[test]
    fn no_progress_timeout_report_counts_failed_tasks() {
        let mut progress = progress();
        progress
            .apply_event(
                MavlinkTelemetryEvent::WaypointReached { seq: 0 },
                Duration::from_secs(1),
            )
            .unwrap();

        let report = progress.mark_no_progress_timeout("no mission progress before timeout");

        assert_eq!(
            report.final_status,
            SitlMissionFinalStatus::TimedOutNoProgress
        );
        assert_eq!(report.completed_count, 1);
        assert_eq!(report.failed_count, 2);
        assert_eq!(
            report.failure_reason,
            Some("no mission progress before timeout".to_owned())
        );
    }

    #[test]
    fn completed_task_ids_follow_mission_order() {
        let mut progress = progress();
        progress
            .apply_event(
                MavlinkTelemetryEvent::WaypointReached { seq: 1 },
                Duration::from_secs(1),
            )
            .unwrap();
        progress
            .apply_event(
                MavlinkTelemetryEvent::WaypointReached { seq: 0 },
                Duration::from_secs(2),
            )
            .unwrap();

        assert_eq!(progress.completed_task_ids(), vec!["wp-0", "wp-1"]);
        assert_eq!(
            progress.completed_waypoints(),
            vec![(0, "wp-0".to_owned()), (1, "wp-1".to_owned())]
        );
    }
}
