use crate::event_log::{Event, EventLog};
use swarm_types::{AgentId, Pose, TaskId};

/// Minimal replay state that reconstructs the system from an event log.
///
/// The replay engine does not re-run the simulation; it reconstructs
/// the final state by applying events in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplayState {
    pub failed_agents: Vec<(AgentId, u64)>,
    pub assigned_tasks: Vec<(TaskId, AgentId, u64)>,
    pub messages_sent: u64,
    pub messages_dropped: u64,
    pub partition_events: u64,
    pub final_poses: Vec<(AgentId, swarm_types::Pose)>,
}

/// Summary of a replay log with mission-specific counts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplaySummary {
    pub total_ticks: u64,
    pub assignments: usize,
    pub completions: usize,
    pub conflicts: usize,
    pub failures: usize,
    pub safety_violations: usize,
    pub sar_scans: usize,
    pub sar_detections: usize,
    pub edges_visited: usize,
    pub cbba_convergence_ticks: Vec<u64>,
    pub messages_sent: u64,
    pub messages_dropped: u64,
}

/// Snapshot of the system at a specific tick.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReplaySnapshot {
    pub tick: u64,
    pub agent_poses: Vec<(AgentId, Pose)>,
    pub assigned_tasks: Vec<(TaskId, AgentId)>,
    pub active_agents: Vec<AgentId>,
    pub failed_agents: Vec<AgentId>,
}

/// Replay an event log and produce the final reconstructed state.
pub fn replay(log: &EventLog) -> ReplayState {
    let mut state = ReplayState::default();

    for event in &log.events {
        match event {
            Event::AgentFailed { agent_id, tick } => {
                state.failed_agents.push((agent_id.clone(), *tick));
            }
            Event::TaskAssigned {
                task_id,
                agent_id,
                tick,
            } => {
                state
                    .assigned_tasks
                    .push((task_id.clone(), agent_id.clone(), *tick));
            }
            Event::MessageSent { .. } => {
                state.messages_sent += 1;
            }
            Event::MessageDropped { .. } => {
                state.messages_dropped += 1;
            }
            Event::PartitionAdded { .. } | Event::PartitionRemoved { .. } => {
                state.partition_events += 1;
            }
            Event::PoseUpdated { agent_id, pose, .. } => {
                // Overwrite previous pose for this agent
                if let Some(entry) = state.final_poses.iter_mut().find(|(id, _)| id == agent_id) {
                    entry.1 = *pose;
                } else {
                    state.final_poses.push((agent_id.clone(), *pose));
                }
            }
            Event::TickStart { .. }
            | Event::TaskStarted { .. }
            | Event::TaskCompleted { .. }
            | Event::TaskExpired { .. }
            | Event::SarScan { .. }
            | Event::SarDetection { .. }
            | Event::EdgeVisited { .. }
            | Event::SafetyViolation { .. }
            | Event::CbbaConverged { .. }
            | Event::CbbaBundleUpdated { .. }
            | Event::AgentObservation { .. }
            | Event::HazardMapUpdated { .. }
            | Event::TaskPriorityUpdated { .. } => {}
        }
    }

    state
}

/// Summarize an event log into key metrics.
pub fn summarize(log: &EventLog) -> ReplaySummary {
    let mut summary = ReplaySummary::default();

    for event in &log.events {
        match event {
            Event::TickStart { tick } => {
                summary.total_ticks = summary.total_ticks.max(*tick);
            }
            Event::TaskAssigned { .. } => {
                summary.assignments += 1;
            }
            Event::TaskCompleted { .. } => {
                summary.completions += 1;
            }
            Event::AgentFailed { .. } => {
                summary.failures += 1;
            }
            Event::MessageSent { .. } => {
                summary.messages_sent += 1;
            }
            Event::MessageDropped { .. } => {
                summary.messages_dropped += 1;
            }
            Event::SafetyViolation { .. } => {
                summary.safety_violations += 1;
            }
            Event::SarScan { .. } => {
                summary.sar_scans += 1;
            }
            Event::SarDetection { .. } => {
                summary.sar_detections += 1;
            }
            Event::EdgeVisited { .. } => {
                summary.edges_visited += 1;
            }
            Event::CbbaConverged { tick } => {
                summary.cbba_convergence_ticks.push(*tick);
            }
            _ => {}
        }
    }

    summary
}

/// Build a snapshot of the system state at the given tick.
pub fn snapshot_at_tick(log: &EventLog, target_tick: u64) -> ReplaySnapshot {
    let mut snapshot = ReplaySnapshot {
        tick: target_tick,
        ..Default::default()
    };

    for event in &log.events {
        match event {
            Event::TickStart { tick } => {
                if *tick > target_tick {
                    break;
                }
            }
            Event::AgentFailed { agent_id, tick } => {
                if *tick > target_tick {
                    continue;
                }
                if !snapshot.failed_agents.contains(agent_id) {
                    snapshot.failed_agents.push(agent_id.clone());
                }
            }
            Event::TaskAssigned {
                task_id,
                agent_id,
                tick,
            } => {
                if *tick > target_tick {
                    continue;
                }
                // Overwrite any previous assignment for this task
                if let Some(entry) = snapshot
                    .assigned_tasks
                    .iter_mut()
                    .find(|(tid, _)| tid == task_id)
                {
                    entry.1 = agent_id.clone();
                } else {
                    snapshot
                        .assigned_tasks
                        .push((task_id.clone(), agent_id.clone()));
                }
            }
            Event::PoseUpdated {
                agent_id,
                pose,
                tick,
                ..
            } => {
                if *tick > target_tick {
                    continue;
                }
                if let Some(entry) = snapshot
                    .agent_poses
                    .iter_mut()
                    .find(|(id, _)| id == agent_id)
                {
                    entry.1 = *pose;
                } else {
                    snapshot.agent_poses.push((agent_id.clone(), *pose));
                }
            }
            _ => {}
        }
    }

    // active agents = all agents in poses that are not failed
    for (agent_id, _) in &snapshot.agent_poses {
        if !snapshot.failed_agents.contains(agent_id) && !snapshot.active_agents.contains(agent_id)
        {
            snapshot.active_agents.push(agent_id.clone());
        }
    }

    snapshot
}

/// Render an ASCII grid snapshot.
///
/// Agents are rendered as 'A', failed agents as 'X', empty cells as '.'.
/// When multiple agents occupy the same cell, the count is shown.
pub fn render_ascii_grid(
    snapshot: &ReplaySnapshot,
    grid_bounds: (f64, f64, f64, f64),
    grid_size: usize,
) -> String {
    let (min_x, max_x, min_y, max_y) = grid_bounds;
    let cell_w = (max_x - min_x) / grid_size as f64;
    let cell_h = (max_y - min_y) / grid_size as f64;

    // Count agents per cell
    let mut grid: Vec<Vec<u32>> = vec![vec![0; grid_size]; grid_size];
    let mut failed_grid: Vec<Vec<u32>> = vec![vec![0; grid_size]; grid_size];

    for (agent_id, pose) in &snapshot.agent_poses {
        let gx = ((pose.x - min_x) / cell_w).clamp(0.0, grid_size as f64 - 1.0) as usize;
        let gy = ((pose.y - min_y) / cell_h).clamp(0.0, grid_size as f64 - 1.0) as usize;
        if snapshot.failed_agents.contains(agent_id) {
            failed_grid[gy][gx] += 1;
        } else {
            grid[gy][gx] += 1;
        }
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "Tick {}  ({}x{} grid)",
        snapshot.tick, grid_size, grid_size
    ));
    lines.push("-".repeat(grid_size));

    for gy in (0..grid_size).rev() {
        let mut row = String::new();
        for gx in 0..grid_size {
            let active = grid[gy][gx];
            let failed = failed_grid[gy][gx];
            let ch = if active > 0 && failed > 0 {
                '*'
            } else if active > 1 {
                char::from_digit(active.min(9), 10).unwrap_or('A')
            } else if active == 1 {
                'A'
            } else if failed > 1 {
                char::from_digit(failed.min(9), 10).unwrap_or('X')
            } else if failed == 1 {
                'X'
            } else {
                '.'
            };
            row.push(ch);
        }
        lines.push(row);
    }

    lines.push("-".repeat(grid_size));
    lines.push("Legend: A=active agent  X=failed agent  *=mixed  .=empty".to_owned());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_log::EventLogBuilder;
    use swarm_types::{AgentId, Pose, TaskId};

    #[test]
    fn replay_reconstructs_state() {
        let mut builder = EventLogBuilder::new("test", 0, "scenario");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::MessageSent {
            from: AgentId::from("a0".to_owned()),
            to: AgentId::from("a1".to_owned()),
            tick: 1,
            payload_len: 10,
        });
        builder.push(Event::TaskAssigned {
            task_id: TaskId::from("t0".to_owned()),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 2,
        });
        builder.push(Event::AgentFailed {
            agent_id: AgentId::from("a1".to_owned()),
            tick: 3,
        });

        let log = builder.build();
        let state = replay(&log);

        assert_eq!(state.messages_sent, 1);
        assert_eq!(state.assigned_tasks.len(), 1);
        assert_eq!(state.failed_agents.len(), 1);
    }

    #[test]
    fn summarize_counts_events_correctly() {
        let mut builder = EventLogBuilder::new("test", 0, "s");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::TickStart { tick: 1 });
        builder.push(Event::TaskAssigned {
            task_id: TaskId::from("t0".to_owned()),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 0,
        });
        builder.push(Event::TaskCompleted {
            task_id: TaskId::from("t0".to_owned()),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 1,
        });
        builder.push(Event::AgentFailed {
            agent_id: AgentId::from("a1".to_owned()),
            tick: 1,
        });
        builder.push(Event::SarScan {
            agent_id: AgentId::from("a0".to_owned()),
            cell: (0, 0),
            tick: 0,
            detected: false,
        });
        builder.push(Event::SarDetection {
            agent_id: AgentId::from("a0".to_owned()),
            target_pose: Pose { x: 0.0, y: 0.0 , ..Default::default()},
            tick: 1,
        });
        builder.push(Event::EdgeVisited {
            edge_id: "e0".to_owned(),
            agent_id: AgentId::from("a0".to_owned()),
            tick: 1,
        });
        builder.push(Event::SafetyViolation {
            agent_id: AgentId::from("a0".to_owned()),
            violation_type: crate::event_log::ViolationType::NoFly,
            tick: 1,
        });
        builder.push(Event::CbbaConverged { tick: 1 });
        builder.push(Event::MessageSent {
            from: AgentId::from("a0".to_owned()),
            to: AgentId::from("a1".to_owned()),
            tick: 0,
            payload_len: 10,
        });
        builder.push(Event::MessageDropped {
            from: AgentId::from("a0".to_owned()),
            to: AgentId::from("a1".to_owned()),
            tick: 0,
            reason: crate::event_log::DropReason::PacketLoss,
        });

        let log = builder.build();
        let s = summarize(&log);
        assert_eq!(s.total_ticks, 1);
        assert_eq!(s.assignments, 1);
        assert_eq!(s.completions, 1);
        assert_eq!(s.failures, 1);
        assert_eq!(s.sar_scans, 1);
        assert_eq!(s.sar_detections, 1);
        assert_eq!(s.edges_visited, 1);
        assert_eq!(s.safety_violations, 1);
        assert_eq!(s.cbba_convergence_ticks, vec![1]);
        assert_eq!(s.messages_sent, 1);
        assert_eq!(s.messages_dropped, 1);
    }

    #[test]
    fn snapshot_at_tick_reconstructs_poses() {
        let mut builder = EventLogBuilder::new("test", 0, "s");
        builder.push(Event::TickStart { tick: 0 });
        builder.push(Event::PoseUpdated {
            agent_id: AgentId::from("a0".to_owned()),
            pose: Pose { x: 1.0, y: 2.0 , ..Default::default()},
            tick: 0,
        });
        builder.push(Event::TickStart { tick: 1 });
        builder.push(Event::PoseUpdated {
            agent_id: AgentId::from("a0".to_owned()),
            pose: Pose { x: 3.0, y: 4.0 , ..Default::default()},
            tick: 1,
        });

        let log = builder.build();
        let snap0 = snapshot_at_tick(&log, 0);
        assert_eq!(snap0.agent_poses.len(), 1);
        assert_eq!(snap0.agent_poses[0].1, Pose { x: 1.0, y: 2.0 , ..Default::default()});

        let snap1 = snapshot_at_tick(&log, 1);
        assert_eq!(snap1.agent_poses[0].1, Pose { x: 3.0, y: 4.0 , ..Default::default()});
    }

    #[test]
    fn render_ascii_grid_basic() {
        let snapshot = ReplaySnapshot {
            tick: 0,
            agent_poses: vec![(AgentId::from("a0".to_owned()), Pose { x: 5.0, y: 5.0 , ..Default::default()})],
            assigned_tasks: vec![],
            active_agents: vec![AgentId::from("a0".to_owned())],
            failed_agents: vec![],
        };
        let grid = render_ascii_grid(&snapshot, (0.0, 10.0, 0.0, 10.0), 5);
        assert!(grid.contains('A'));
        assert!(grid.contains('.'));
    }
}
