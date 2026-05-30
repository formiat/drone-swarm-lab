use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use swarm_safety::Aabb;
use swarm_sim::ScenarioSuiteEntry;
use swarm_types::Pose;

use crate::sitl_plan::SitlError;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SitlNoFlyZone {
    pub id: String,
    pub bounds: Aabb,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SitlSafetyConfig {
    #[serde(default = "default_geofence")]
    pub geofence: Option<Aabb>,
    #[serde(default = "default_min_altitude_m")]
    pub min_altitude_m: f64,
    #[serde(default = "default_max_altitude_m")]
    pub max_altitude_m: f64,
    #[serde(default = "default_max_waypoint_jump_m")]
    pub max_waypoint_jump_m: f64,
    #[serde(default = "default_max_mission_radius_m")]
    pub max_mission_radius_m: f64,
    #[serde(default)]
    pub no_fly_zones: Vec<SitlNoFlyZone>,
    #[serde(default)]
    pub home: Option<Pose>,
    #[serde(default = "default_require_home")]
    pub require_home: bool,
}

impl Default for SitlSafetyConfig {
    fn default() -> Self {
        Self {
            geofence: default_geofence(),
            min_altitude_m: default_min_altitude_m(),
            max_altitude_m: default_max_altitude_m(),
            max_waypoint_jump_m: default_max_waypoint_jump_m(),
            max_mission_radius_m: default_max_mission_radius_m(),
            no_fly_zones: Vec::new(),
            home: None,
            require_home: default_require_home(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SitlSafetyRuleId {
    EmptyMission,
    DuplicateWaypointId,
    MissingPose,
    InvalidAltitude,
    OutsideGeofence,
    InsideNoFlyZone,
    UnsafeWaypointJump,
    MissionRadiusExceeded,
    MissingHome,
}

impl SitlSafetyRuleId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EmptyMission => "empty_mission",
            Self::DuplicateWaypointId => "duplicate_waypoint_id",
            Self::MissingPose => "missing_pose",
            Self::InvalidAltitude => "invalid_altitude",
            Self::OutsideGeofence => "outside_geofence",
            Self::InsideNoFlyZone => "inside_no_fly_zone",
            Self::UnsafeWaypointJump => "unsafe_waypoint_jump",
            Self::MissionRadiusExceeded => "mission_radius_exceeded",
            Self::MissingHome => "missing_home",
        }
    }
}

impl fmt::Display for SitlSafetyRuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SitlSafetyViolation {
    pub rule_id: SitlSafetyRuleId,
    pub task_id: Option<String>,
    pub seq: Option<u16>,
    pub actual: String,
    pub allowed: String,
}

impl SitlSafetyViolation {
    fn new(
        rule_id: SitlSafetyRuleId,
        task_id: Option<String>,
        seq: Option<u16>,
        actual: impl Into<String>,
        allowed: impl Into<String>,
    ) -> Self {
        Self {
            rule_id,
            task_id,
            seq,
            actual: actual.into(),
            allowed: allowed.into(),
        }
    }
}

impl fmt::Display for SitlSafetyViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rule_id={}", self.rule_id)?;
        if let Some(task_id) = &self.task_id {
            write!(f, " task_id={task_id}")?;
        }
        if let Some(seq) = self.seq {
            write!(f, " seq={seq}")?;
        }
        write!(f, " actual={} allowed={}", self.actual, self.allowed)
    }
}

pub fn load_sitl_safety_config(path: Option<&Path>) -> Result<SitlSafetyConfig, SitlError> {
    let config = match path {
        Some(path) => {
            let content =
                std::fs::read_to_string(path).map_err(|error| SitlError::SafetyConfigRead {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;
            serde_json::from_str(&content).map_err(|error| SitlError::SafetyConfigParse {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?
        }
        None => SitlSafetyConfig::default(),
    };
    validate_config(&config)?;
    Ok(config)
}

pub fn validate_pre_upload_safety(
    entry: &ScenarioSuiteEntry,
    agent_id: &str,
    config: &SitlSafetyConfig,
) -> Result<(), SitlError> {
    let violations = collect_pre_upload_safety_violations(entry, agent_id, config);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(SitlError::SafetyValidationFailed {
            message: format_violations(&violations),
        })
    }
}

pub fn validate_pre_upload_safety_for_task_ids(
    entry: &ScenarioSuiteEntry,
    agent_id: &str,
    config: &SitlSafetyConfig,
    task_ids: &[String],
) -> Result<(), SitlError> {
    let violations =
        collect_pre_upload_safety_violations_for_task_ids(entry, agent_id, config, task_ids);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(SitlError::SafetyValidationFailed {
            message: format_violations(&violations),
        })
    }
}

pub fn collect_pre_upload_safety_violations(
    entry: &ScenarioSuiteEntry,
    agent_id: &str,
    config: &SitlSafetyConfig,
) -> Vec<SitlSafetyViolation> {
    collect_pre_upload_safety_violations_impl(entry, agent_id, config, None)
}

pub fn collect_pre_upload_safety_violations_for_task_ids(
    entry: &ScenarioSuiteEntry,
    agent_id: &str,
    config: &SitlSafetyConfig,
    task_ids: &[String],
) -> Vec<SitlSafetyViolation> {
    let task_ids: HashSet<&str> = task_ids.iter().map(String::as_str).collect();
    collect_pre_upload_safety_violations_impl(entry, agent_id, config, Some(&task_ids))
}

fn collect_pre_upload_safety_violations_impl(
    entry: &ScenarioSuiteEntry,
    agent_id: &str,
    config: &SitlSafetyConfig,
    allowed_task_ids: Option<&HashSet<&str>>,
) -> Vec<SitlSafetyViolation> {
    let mut violations = Vec::new();
    let selected_tasks: Vec<_> = entry
        .scenario
        .tasks
        .iter()
        .filter(|task| {
            let task_id = task.id.to_string();
            allowed_task_ids.is_none_or(|ids| ids.contains(task_id.as_str()))
        })
        .collect();

    if selected_tasks.is_empty() {
        violations.push(SitlSafetyViolation::new(
            SitlSafetyRuleId::EmptyMission,
            None,
            None,
            "tasks=0",
            "at least one waypoint task",
        ));
        return violations;
    }

    collect_duplicate_ids(&selected_tasks, &mut violations);
    let home = resolve_home(entry, agent_id, config);
    if config.require_home && home.is_none() {
        violations.push(SitlSafetyViolation::new(
            SitlSafetyRuleId::MissingHome,
            None,
            None,
            "home=missing",
            "config.home, scenario.base_station, or selected agent pose",
        ));
    }

    let mut pose_waypoints = Vec::new();
    for task in selected_tasks {
        let task_id = task.id.to_string();
        let Some(pose) = task.pose else {
            violations.push(SitlSafetyViolation::new(
                SitlSafetyRuleId::MissingPose,
                Some(task_id),
                None,
                "pose=missing",
                "pose required for SITL waypoint upload",
            ));
            continue;
        };

        let seq = pose_waypoints.len() as u16;
        validate_pose_waypoint(&task_id, seq, pose, home, config, &mut violations);
        pose_waypoints.push((task_id, seq, pose));
    }

    if pose_waypoints.is_empty() {
        violations.push(SitlSafetyViolation::new(
            SitlSafetyRuleId::EmptyMission,
            None,
            None,
            "pose_waypoints=0",
            "at least one task with pose",
        ));
    }

    for pair in pose_waypoints.windows(2) {
        let (from_id, from_seq, from_pose) = &pair[0];
        let (to_id, to_seq, to_pose) = &pair[1];
        let distance = from_pose.distance_to(to_pose);
        if !distance.is_finite() || distance > config.max_waypoint_jump_m {
            violations.push(SitlSafetyViolation::new(
                SitlSafetyRuleId::UnsafeWaypointJump,
                Some(to_id.clone()),
                Some(*to_seq),
                format!("from={from_id} seq={from_seq} distance={distance:.3}m"),
                format!("<= {:.3}m", config.max_waypoint_jump_m),
            ));
        }
    }

    violations
}

pub fn format_violations(violations: &[SitlSafetyViolation]) -> String {
    violations
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

fn validate_config(config: &SitlSafetyConfig) -> Result<(), SitlError> {
    validate_optional_aabb("geofence", config.geofence.as_ref())?;
    for zone in &config.no_fly_zones {
        if zone.id.trim().is_empty() {
            return safety_config_invalid("no_fly_zones.id", "must not be empty");
        }
        validate_aabb(&format!("no_fly_zones[{}].bounds", zone.id), &zone.bounds)?;
    }
    validate_finite("min_altitude_m", config.min_altitude_m)?;
    validate_finite("max_altitude_m", config.max_altitude_m)?;
    if config.min_altitude_m > config.max_altitude_m {
        return safety_config_invalid("altitude", "min_altitude_m must be <= max_altitude_m");
    }
    validate_positive("max_waypoint_jump_m", config.max_waypoint_jump_m)?;
    validate_positive("max_mission_radius_m", config.max_mission_radius_m)?;
    if let Some(home) = config.home {
        validate_pose("home", home)?;
    }
    Ok(())
}

fn validate_pose_waypoint(
    task_id: &str,
    seq: u16,
    pose: Pose,
    home: Option<Pose>,
    config: &SitlSafetyConfig,
    violations: &mut Vec<SitlSafetyViolation>,
) {
    if !pose.z.is_finite() || pose.z < config.min_altitude_m || pose.z > config.max_altitude_m {
        violations.push(SitlSafetyViolation::new(
            SitlSafetyRuleId::InvalidAltitude,
            Some(task_id.to_owned()),
            Some(seq),
            format!("z={:.3}", pose.z),
            format!(
                "{:.3}..={:.3}",
                config.min_altitude_m, config.max_altitude_m
            ),
        ));
    }

    if let Some(geofence) = &config.geofence {
        if !geofence.contains(&pose) {
            violations.push(SitlSafetyViolation::new(
                SitlSafetyRuleId::OutsideGeofence,
                Some(task_id.to_owned()),
                Some(seq),
                format!("point=({:.3},{:.3})", pose.x, pose.y),
                format!("geofence={}", format_aabb(geofence)),
            ));
        }
    }

    for zone in &config.no_fly_zones {
        if zone.bounds.contains(&pose) {
            violations.push(SitlSafetyViolation::new(
                SitlSafetyRuleId::InsideNoFlyZone,
                Some(task_id.to_owned()),
                Some(seq),
                format!("point=({:.3},{:.3}) zone_id={}", pose.x, pose.y, zone.id),
                format!("outside {}", format_aabb(&zone.bounds)),
            ));
        }
    }

    if let Some(home) = home {
        let distance = home.distance_to(&pose);
        if !distance.is_finite() || distance > config.max_mission_radius_m {
            violations.push(SitlSafetyViolation::new(
                SitlSafetyRuleId::MissionRadiusExceeded,
                Some(task_id.to_owned()),
                Some(seq),
                format!("distance={distance:.3}m"),
                format!("<= {:.3}m from home", config.max_mission_radius_m),
            ));
        }
    }
}

fn collect_duplicate_ids(tasks: &[&swarm_types::Task], violations: &mut Vec<SitlSafetyViolation>) {
    let mut seen = HashSet::new();
    for task in tasks {
        let task_id = task.id.to_string();
        if !seen.insert(task_id.clone()) {
            violations.push(SitlSafetyViolation::new(
                SitlSafetyRuleId::DuplicateWaypointId,
                Some(task_id.clone()),
                None,
                format!("task_id={task_id}"),
                "unique task ids",
            ));
        }
    }
}

fn resolve_home(
    entry: &ScenarioSuiteEntry,
    agent_id: &str,
    config: &SitlSafetyConfig,
) -> Option<Pose> {
    config.home.or(entry.scenario.base_station).or_else(|| {
        entry
            .scenario
            .agents
            .iter()
            .find(|agent| agent.id.to_string() == agent_id)
            .map(|agent| agent.pose)
    })
}

fn validate_optional_aabb(field: &str, bounds: Option<&Aabb>) -> Result<(), SitlError> {
    if let Some(bounds) = bounds {
        validate_aabb(field, bounds)?;
    }
    Ok(())
}

fn validate_aabb(field: &str, bounds: &Aabb) -> Result<(), SitlError> {
    for (name, value) in [
        ("min_x", bounds.min_x),
        ("max_x", bounds.max_x),
        ("min_y", bounds.min_y),
        ("max_y", bounds.max_y),
    ] {
        validate_finite(&format!("{field}.{name}"), value)?;
    }
    if bounds.min_x > bounds.max_x || bounds.min_y > bounds.max_y {
        return safety_config_invalid(field, "min bounds must be <= max bounds");
    }
    Ok(())
}

fn validate_pose(field: &str, pose: Pose) -> Result<(), SitlError> {
    validate_finite(&format!("{field}.x"), pose.x)?;
    validate_finite(&format!("{field}.y"), pose.y)?;
    validate_finite(&format!("{field}.z"), pose.z)?;
    Ok(())
}

fn validate_finite(field: &str, value: f64) -> Result<(), SitlError> {
    if value.is_finite() {
        Ok(())
    } else {
        safety_config_invalid(field, "must be finite")
    }
}

fn validate_positive(field: &str, value: f64) -> Result<(), SitlError> {
    validate_finite(field, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        safety_config_invalid(field, "must be > 0")
    }
}

fn safety_config_invalid<T>(field: &str, message: &str) -> Result<T, SitlError> {
    Err(SitlError::SafetyConfigInvalid {
        field: field.to_owned(),
        message: message.to_owned(),
    })
}

fn format_aabb(bounds: &Aabb) -> String {
    format!(
        "[x:{:.3}..={:.3}, y:{:.3}..={:.3}]",
        bounds.min_x, bounds.max_x, bounds.min_y, bounds.max_y
    )
}

fn default_geofence() -> Option<Aabb> {
    Some(Aabb {
        min_x: -1000.0,
        max_x: 1000.0,
        min_y: -1000.0,
        max_y: 1000.0,
    })
}

fn default_min_altitude_m() -> f64 {
    0.0
}

fn default_max_altitude_m() -> f64 {
    120.0
}

fn default_max_waypoint_jump_m() -> f64 {
    500.0
}

fn default_max_mission_radius_m() -> f64 {
    1000.0
}

fn default_require_home() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_sim::{RunConfig, Scenario};
    use swarm_types::{Agent, AgentId, Health, Role, Task, TaskId, TaskStatus};

    fn agent(id: &str, x: f64, y: f64) -> Agent {
        Agent {
            id: AgentId::from(id.to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose { x, y, z: 0.0 },
            capabilities: vec![],
            current_task: None,
            battery: 100.0,
            comms_range: 1000.0,
            generation: 1,
            speed: 0.0,
            max_range: 1000.0,
            battery_drain_rate: 0.0,
            battery_model: None,
        }
    }

    fn task(id: &str, pose: Option<Pose>) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose,
            grid_cell: None,
            edge_id: None,
            kind: None,
        }
    }

    fn pose(x: f64, y: f64, z: f64) -> Pose {
        Pose { x, y, z }
    }

    fn entry(tasks: Vec<Task>) -> ScenarioSuiteEntry {
        ScenarioSuiteEntry {
            mission: "sitl".to_owned(),
            profile: "waypoints".to_owned(),
            scenario: Scenario {
                name: "sitl_waypoints".to_owned(),
                seed: 0,
                agents: vec![agent("agent-0", 0.0, 0.0)],
                tasks,
                ground_nodes: vec![],
                base_station: None,
            },
            run_config: RunConfig {
                max_ticks: 50,
                ..Default::default()
            },
        }
    }

    fn default_entry() -> ScenarioSuiteEntry {
        entry(vec![
            task("wp-0", Some(pose(10.0, 20.0, 10.0))),
            task("wp-1", Some(pose(30.0, 40.0, 20.0))),
        ])
    }

    fn assert_rule(violations: &[SitlSafetyViolation], rule_id: SitlSafetyRuleId) {
        assert!(
            violations
                .iter()
                .any(|violation| violation.rule_id == rule_id),
            "missing rule {rule_id}; violations={violations:?}"
        );
    }

    #[test]
    fn valid_mission_passes_with_safe_defaults() {
        let violations = collect_pre_upload_safety_violations(
            &default_entry(),
            "agent-0",
            &SitlSafetyConfig::default(),
        );

        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn geofence_rejection_test() {
        let config = SitlSafetyConfig {
            geofence: Some(Aabb {
                min_x: 0.0,
                max_x: 20.0,
                min_y: 0.0,
                max_y: 25.0,
            }),
            ..SitlSafetyConfig::default()
        };

        let violations = collect_pre_upload_safety_violations(&default_entry(), "agent-0", &config);

        assert_rule(&violations, SitlSafetyRuleId::OutsideGeofence);
        let message = format_violations(&violations);
        assert!(message.contains("rule_id=outside_geofence"));
        assert!(message.contains("task_id=wp-1"));
        assert!(message.contains("actual=point="));
        assert!(message.contains("allowed=geofence="));
    }

    #[test]
    fn subset_safety_ignores_unselected_unsafe_task() {
        let config = SitlSafetyConfig {
            geofence: Some(Aabb {
                min_x: 0.0,
                max_x: 20.0,
                min_y: 0.0,
                max_y: 25.0,
            }),
            ..SitlSafetyConfig::default()
        };
        let task_ids = vec!["wp-0".to_owned()];

        let violations = collect_pre_upload_safety_violations_for_task_ids(
            &default_entry(),
            "agent-0",
            &config,
            &task_ids,
        );

        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn subset_safety_rejects_selected_unsafe_task() {
        let config = SitlSafetyConfig {
            geofence: Some(Aabb {
                min_x: 0.0,
                max_x: 20.0,
                min_y: 0.0,
                max_y: 25.0,
            }),
            ..SitlSafetyConfig::default()
        };
        let task_ids = vec!["wp-1".to_owned()];

        let violations = collect_pre_upload_safety_violations_for_task_ids(
            &default_entry(),
            "agent-0",
            &config,
            &task_ids,
        );

        assert_rule(&violations, SitlSafetyRuleId::OutsideGeofence);
        assert!(format_violations(&violations).contains("task_id=wp-1"));
    }

    #[test]
    fn altitude_bounds_test() {
        let config = SitlSafetyConfig {
            min_altitude_m: 5.0,
            max_altitude_m: 15.0,
            ..SitlSafetyConfig::default()
        };
        let entry = entry(vec![
            task("too-low", Some(pose(0.0, 0.0, 1.0))),
            task("too-high", Some(pose(10.0, 0.0, 20.0))),
        ]);

        let violations = collect_pre_upload_safety_violations(&entry, "agent-0", &config);

        assert_eq!(
            violations
                .iter()
                .filter(|violation| violation.rule_id == SitlSafetyRuleId::InvalidAltitude)
                .count(),
            2
        );
    }

    #[test]
    fn no_fly_zone_test() {
        let config = SitlSafetyConfig {
            no_fly_zones: vec![SitlNoFlyZone {
                id: "nfz-0".to_owned(),
                bounds: Aabb {
                    min_x: 25.0,
                    max_x: 35.0,
                    min_y: 35.0,
                    max_y: 45.0,
                },
            }],
            ..SitlSafetyConfig::default()
        };

        let violations = collect_pre_upload_safety_violations(&default_entry(), "agent-0", &config);

        assert_rule(&violations, SitlSafetyRuleId::InsideNoFlyZone);
        assert!(format_violations(&violations).contains("zone_id=nfz-0"));
    }

    #[test]
    fn max_waypoint_jump_test() {
        let config = SitlSafetyConfig {
            max_waypoint_jump_m: 10.0,
            ..SitlSafetyConfig::default()
        };

        let violations = collect_pre_upload_safety_violations(&default_entry(), "agent-0", &config);

        assert_rule(&violations, SitlSafetyRuleId::UnsafeWaypointJump);
    }

    #[test]
    fn duplicate_waypoint_id_test() {
        let entry = entry(vec![
            task("wp-0", Some(pose(0.0, 0.0, 10.0))),
            task("wp-0", Some(pose(10.0, 0.0, 10.0))),
        ]);

        let violations =
            collect_pre_upload_safety_violations(&entry, "agent-0", &SitlSafetyConfig::default());

        assert_rule(&violations, SitlSafetyRuleId::DuplicateWaypointId);
    }

    #[test]
    fn missing_pose_test() {
        let entry = entry(vec![task("wp-0", None)]);

        let violations =
            collect_pre_upload_safety_violations(&entry, "agent-0", &SitlSafetyConfig::default());

        assert_rule(&violations, SitlSafetyRuleId::MissingPose);
    }

    #[test]
    fn mission_radius_test() {
        let config = SitlSafetyConfig {
            max_mission_radius_m: 15.0,
            ..SitlSafetyConfig::default()
        };

        let violations = collect_pre_upload_safety_violations(&default_entry(), "agent-0", &config);

        assert_rule(&violations, SitlSafetyRuleId::MissionRadiusExceeded);
    }

    #[test]
    fn missing_home_test() {
        let mut entry = default_entry();
        entry.scenario.agents.clear();

        let violations =
            collect_pre_upload_safety_violations(&entry, "agent-0", &SitlSafetyConfig::default());

        assert_rule(&violations, SitlSafetyRuleId::MissingHome);
    }

    #[test]
    fn config_rejects_invalid_ranges() {
        let config = SitlSafetyConfig {
            min_altitude_m: 10.0,
            max_altitude_m: 5.0,
            ..SitlSafetyConfig::default()
        };

        let error = validate_config(&config).unwrap_err();

        assert!(matches!(error, SitlError::SafetyConfigInvalid { .. }));
    }

    #[test]
    fn config_rejects_unknown_top_level_field() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), r#"{ "max_altitude": 50.0 }"#).unwrap();

        let error = load_sitl_safety_config(Some(file.path())).unwrap_err();

        assert!(matches!(error, SitlError::SafetyConfigParse { .. }));
    }

    #[test]
    fn config_rejects_unknown_no_fly_zone_field() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"{
  "no_fly_zones": [
    {
      "id": "nfz-0",
      "bounds": { "min_x": 0.0, "max_x": 1.0, "min_y": 0.0, "max_y": 1.0 },
      "unexpected": true
    }
  ]
}"#,
        )
        .unwrap();

        let error = load_sitl_safety_config(Some(file.path())).unwrap_err();

        assert!(matches!(error, SitlError::SafetyConfigParse { .. }));
    }
}
