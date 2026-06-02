use swarm_replay::{Event, EventLog};

use super::{optional_id, UrbanJudgeReport, UrbanJudgeViolationRecord};

/// Build a structured Urban judge report from replay violation events.
pub fn build_urban_judge_report(log: &EventLog) -> UrbanJudgeReport {
    let mut violations = Vec::new();
    for event in &log.events {
        if let Event::UrbanViolation {
            agent_id,
            tick,
            segment_index,
            edge_id,
            obstacle_id,
            pose,
            reason,
        } = event
        {
            violations.push(UrbanJudgeViolationRecord {
                agent_id: agent_id.clone(),
                tick: *tick,
                violation_type: classify_violation(reason),
                segment_index: *segment_index,
                edge_id: edge_id.clone(),
                obstacle_id: obstacle_id.clone(),
                pose: *pose,
                reason: reason.clone(),
            });
        }
    }
    violations.sort_by(|a, b| {
        a.tick
            .cmp(&b.tick)
            .then_with(|| a.agent_id.as_ref().cmp(b.agent_id.as_ref()))
            .then_with(|| a.segment_index.cmp(&b.segment_index))
            .then_with(|| optional_id(a.edge_id.as_ref()).cmp(&optional_id(b.edge_id.as_ref())))
    });
    UrbanJudgeReport {
        run_id: log.run_id.clone(),
        scenario_name: log.scenario_name.clone(),
        violations,
    }
}

fn classify_violation(reason: &str) -> String {
    if reason.contains("ObstacleIntersection") {
        "obstacle_intersection".to_owned()
    } else if reason.contains("BlockedEdge") {
        "blocked_edge".to_owned()
    } else if reason.contains("MissingEdge") {
        "missing_edge".to_owned()
    } else {
        "unknown".to_owned()
    }
}
