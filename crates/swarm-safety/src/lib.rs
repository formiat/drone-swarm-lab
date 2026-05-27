use serde::{Deserialize, Serialize};
use swarm_types::{Agent, AgentId, Pose, Task};

/// Axis-aligned bounding box for geofence or no-fly zone.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Aabb {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

impl Aabb {
    /// Check if a point is inside the AABB (inclusive bounds).
    pub fn contains(&self, pose: &Pose) -> bool {
        pose.x >= self.min_x && pose.x <= self.max_x && pose.y >= self.min_y && pose.y <= self.max_y
    }
}

/// Constraint: agent must stay inside this area.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Geofence {
    pub bounds: Aabb,
}

/// Prohibited area. Agents must not enter.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NoFlyZone {
    pub bounds: Aabb,
    /// First tick at which this zone is active (inclusive). `None` = always active from the start.
    #[serde(default)]
    pub active_from_tick: Option<u64>,
    /// Last tick at which this zone is active (inclusive). `None` = never expires.
    #[serde(default)]
    pub active_until_tick: Option<u64>,
}

impl NoFlyZone {
    /// Returns whether this zone is active at the given simulation tick.
    pub fn is_active_at(&self, tick: u64) -> bool {
        let after_start = self.active_from_tick.is_none_or(|t| tick >= t);
        let before_end = self.active_until_tick.is_none_or(|t| tick <= t);
        after_start && before_end
    }
}

/// Minimum distance between any two agents.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SeparationConstraint {
    pub min_distance_m: f64,
}

/// Complete safety configuration for a scenario.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SafetyConfig {
    #[serde(default)]
    pub geofence: Option<Geofence>,
    #[serde(default)]
    pub no_fly_zones: Vec<NoFlyZone>,
    #[serde(default)]
    pub separation: Option<SeparationConstraint>,
}

/// Type of safety violation detected.
#[derive(Clone, Debug, PartialEq)]
pub enum ViolationType {
    GeofenceExited,
    NoFlyZoneEntered,
    SeparationBreached { other_agent_id: AgentId },
}

/// A single safety violation by an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct SafetyViolation {
    pub agent_id: AgentId,
    pub violation_type: ViolationType,
}

/// Check an agent against all safety constraints at a specific simulation tick.
///
/// No-fly zones with `active_from_tick` / `active_until_tick` are only enforced when active.
pub fn check_agent_at_tick(
    config: &SafetyConfig,
    agent: &Agent,
    others: &[Agent],
    current_tick: u64,
) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();

    // Geofence: must be inside
    if let Some(ref geofence) = config.geofence {
        if !geofence.bounds.contains(&agent.pose) {
            violations.push(SafetyViolation {
                agent_id: agent.id.clone(),
                violation_type: ViolationType::GeofenceExited,
            });
        }
    }

    // No-fly zones: must NOT be inside when zone is active
    for nofly in &config.no_fly_zones {
        if nofly.is_active_at(current_tick) && nofly.bounds.contains(&agent.pose) {
            violations.push(SafetyViolation {
                agent_id: agent.id.clone(),
                violation_type: ViolationType::NoFlyZoneEntered,
            });
        }
    }

    // Separation: minimum distance to other agents
    if let Some(ref sep) = config.separation {
        for other in others {
            if other.id == agent.id {
                continue;
            }
            let dx = agent.pose.x - other.pose.x;
            let dy = agent.pose.y - other.pose.y;
            let dist_sq = dx * dx + dy * dy;
            let min_dist = sep.min_distance_m;
            if dist_sq < min_dist * min_dist {
                violations.push(SafetyViolation {
                    agent_id: agent.id.clone(),
                    violation_type: ViolationType::SeparationBreached {
                        other_agent_id: other.id.clone(),
                    },
                });
            }
        }
    }

    violations
}

/// Check an agent against all safety constraints (always-active zones).
///
/// This is a backward-compatible wrapper that treats all no-fly zones as permanently active.
/// Prefer [`check_agent_at_tick`] when the current tick is available.
pub fn check_agent(config: &SafetyConfig, agent: &Agent, others: &[Agent]) -> Vec<SafetyViolation> {
    let mut violations = Vec::new();

    // Geofence: must be inside
    if let Some(ref geofence) = config.geofence {
        if !geofence.bounds.contains(&agent.pose) {
            violations.push(SafetyViolation {
                agent_id: agent.id.clone(),
                violation_type: ViolationType::GeofenceExited,
            });
        }
    }

    // No-fly zones: must NOT be inside (treat all as permanently active)
    for nofly in &config.no_fly_zones {
        if nofly.bounds.contains(&agent.pose) {
            violations.push(SafetyViolation {
                agent_id: agent.id.clone(),
                violation_type: ViolationType::NoFlyZoneEntered,
            });
        }
    }

    // Separation: minimum distance to other agents
    if let Some(ref sep) = config.separation {
        for other in others {
            if other.id == agent.id {
                continue;
            }
            let dx = agent.pose.x - other.pose.x;
            let dy = agent.pose.y - other.pose.y;
            let dist_sq = dx * dx + dy * dy;
            let min_dist = sep.min_distance_m;
            if dist_sq < min_dist * min_dist {
                violations.push(SafetyViolation {
                    agent_id: agent.id.clone(),
                    violation_type: ViolationType::SeparationBreached {
                        other_agent_id: other.id.clone(),
                    },
                });
            }
        }
    }

    violations
}

/// Check whether a task's pose is reachable for an agent under safety config.
/// A task is unreachable if its pose lies inside an active no-fly zone.
pub fn is_task_reachable(config: &SafetyConfig, _agent: &Agent, task: &Task) -> bool {
    let task_pose = match task.pose {
        Some(p) => p,
        None => return true, // No pose = no spatial constraint
    };

    for nofly in &config.no_fly_zones {
        // Treat all zones as permanently active (backward-compatible behaviour)
        if nofly.bounds.contains(&task_pose) {
            return false;
        }
    }

    true
}

/// Filter tasks that are safe for the given agent to approach.
pub fn filter_safe_tasks<'a>(
    config: &SafetyConfig,
    agent: &Agent,
    tasks: &'a [Task],
) -> Vec<&'a Task> {
    tasks
        .iter()
        .filter(|task| is_task_reachable(config, agent, task))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::{Capability, Health, Role, TaskId};

    fn make_agent(id: &str, x: f64, y: f64) -> Agent {
        Agent {
            id: AgentId::from(id.to_owned()),
            role: Role::Scout,
            health: Health::Alive,
            pose: Pose {
                x,
                y,
                ..Default::default()
            },
            capabilities: vec![Capability::from("basic".to_owned())],
            current_task: None,
            battery: 100.0,
            comms_range: f64::INFINITY,
            generation: 1,
            speed: 0.0,
            max_range: 0.0,
            battery_drain_rate: 0.0,
            battery_model: None,
        }
    }

    fn make_task(id: &str, x: f64, y: f64) -> Task {
        Task {
            id: TaskId::from(id.to_owned()),
            status: swarm_types::TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: Some(Pose {
                x,
                y,
                ..Default::default()
            }),
            grid_cell: None,
            edge_id: None,
            kind: None,
        }
    }

    #[test]
    fn check_agent_no_violations_outside_nofly() {
        let config = SafetyConfig {
            geofence: None,
            no_fly_zones: vec![NoFlyZone {
                bounds: Aabb {
                    min_x: 10.0,
                    max_x: 20.0,
                    min_y: 10.0,
                    max_y: 20.0,
                },
                active_from_tick: None,
                active_until_tick: None,
            }],
            separation: None,
        };
        let agent = make_agent("a0", 0.0, 0.0);
        let violations = check_agent(&config, &agent, &[]);
        assert!(violations.is_empty());
    }

    #[test]
    fn check_agent_nofly_violation() {
        let config = SafetyConfig {
            geofence: None,
            no_fly_zones: vec![NoFlyZone {
                bounds: Aabb {
                    min_x: 0.0,
                    max_x: 10.0,
                    min_y: 0.0,
                    max_y: 10.0,
                },
                active_from_tick: None,
                active_until_tick: None,
            }],
            separation: None,
        };
        let agent = make_agent("a0", 5.0, 5.0);
        let violations = check_agent(&config, &agent, &[]);
        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations[0].violation_type,
            ViolationType::NoFlyZoneEntered
        );
    }

    #[test]
    fn check_agent_geofence_exited() {
        let config = SafetyConfig {
            geofence: Some(Geofence {
                bounds: Aabb {
                    min_x: 0.0,
                    max_x: 100.0,
                    min_y: 0.0,
                    max_y: 100.0,
                },
            }),
            no_fly_zones: vec![],
            separation: None,
        };
        let agent = make_agent("a0", 150.0, 50.0);
        let violations = check_agent(&config, &agent, &[]);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].violation_type, ViolationType::GeofenceExited);
    }

    #[test]
    fn check_agent_geofence_inside_no_violation() {
        let config = SafetyConfig {
            geofence: Some(Geofence {
                bounds: Aabb {
                    min_x: 0.0,
                    max_x: 100.0,
                    min_y: 0.0,
                    max_y: 100.0,
                },
            }),
            no_fly_zones: vec![],
            separation: None,
        };
        let agent = make_agent("a0", 50.0, 50.0);
        let violations = check_agent(&config, &agent, &[]);
        assert!(violations.is_empty());
    }

    #[test]
    fn check_agent_separation_breached() {
        let config = SafetyConfig {
            geofence: None,
            no_fly_zones: vec![],
            separation: Some(SeparationConstraint {
                min_distance_m: 5.0,
            }),
        };
        let agent = make_agent("a0", 0.0, 0.0);
        let other = make_agent("a1", 3.0, 0.0); // distance = 3 < 5
        let violations = check_agent(&config, &agent, &[other]);
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            violations[0].violation_type,
            ViolationType::SeparationBreached { .. }
        ));
    }

    #[test]
    fn check_agent_separation_ok() {
        let config = SafetyConfig {
            geofence: None,
            no_fly_zones: vec![],
            separation: Some(SeparationConstraint {
                min_distance_m: 5.0,
            }),
        };
        let agent = make_agent("a0", 0.0, 0.0);
        let other = make_agent("a1", 10.0, 0.0); // distance = 10 >= 5
        let violations = check_agent(&config, &agent, &[other]);
        assert!(violations.is_empty());
    }

    #[test]
    fn is_task_reachable_nofly_blocked() {
        let config = SafetyConfig {
            geofence: None,
            no_fly_zones: vec![NoFlyZone {
                bounds: Aabb {
                    min_x: 0.0,
                    max_x: 10.0,
                    min_y: 0.0,
                    max_y: 10.0,
                },
                active_from_tick: None,
                active_until_tick: None,
            }],
            separation: None,
        };
        let agent = make_agent("a0", 0.0, 0.0);
        let task = make_task("t0", 5.0, 5.0);
        assert!(!is_task_reachable(&config, &agent, &task));
    }

    #[test]
    fn is_task_reachable_safe_task() {
        let config = SafetyConfig {
            geofence: None,
            no_fly_zones: vec![NoFlyZone {
                bounds: Aabb {
                    min_x: 0.0,
                    max_x: 10.0,
                    min_y: 0.0,
                    max_y: 10.0,
                },
                active_from_tick: None,
                active_until_tick: None,
            }],
            separation: None,
        };
        let agent = make_agent("a0", 0.0, 0.0);
        let task = make_task("t0", 20.0, 20.0);
        assert!(is_task_reachable(&config, &agent, &task));
    }

    #[test]
    fn filter_safe_tasks_excludes_nofly() {
        let config = SafetyConfig {
            geofence: None,
            no_fly_zones: vec![NoFlyZone {
                bounds: Aabb {
                    min_x: 0.0,
                    max_x: 10.0,
                    min_y: 0.0,
                    max_y: 10.0,
                },
                active_from_tick: None,
                active_until_tick: None,
            }],
            separation: None,
        };
        let agent = make_agent("a0", 0.0, 0.0);
        let safe_task = make_task("t0", 20.0, 20.0);
        let unsafe_task = make_task("t1", 5.0, 5.0);
        let tasks = [safe_task.clone(), unsafe_task.clone()];
        let filtered = filter_safe_tasks(&config, &agent, &tasks);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, safe_task.id);
    }

    #[test]
    fn filter_safe_tasks_preserves_safe() {
        let config = SafetyConfig::default();
        let agent = make_agent("a0", 0.0, 0.0);
        let tasks = vec![make_task("t0", 1.0, 1.0), make_task("t1", 2.0, 2.0)];
        let filtered = filter_safe_tasks(&config, &agent, &tasks);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn serde_roundtrip() {
        let config = SafetyConfig {
            geofence: Some(Geofence {
                bounds: Aabb {
                    min_x: 0.0,
                    max_x: 100.0,
                    min_y: 0.0,
                    max_y: 100.0,
                },
            }),
            no_fly_zones: vec![NoFlyZone {
                bounds: Aabb {
                    min_x: 40.0,
                    max_x: 60.0,
                    min_y: 40.0,
                    max_y: 60.0,
                },
                active_from_tick: None,
                active_until_tick: None,
            }],
            separation: Some(SeparationConstraint {
                min_distance_m: 2.0,
            }),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SafetyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn nofly_zone_is_active_at_permanent() {
        let zone = NoFlyZone {
            bounds: Aabb {
                min_x: 0.0,
                max_x: 10.0,
                min_y: 0.0,
                max_y: 10.0,
            },
            active_from_tick: None,
            active_until_tick: None,
        };
        assert!(zone.is_active_at(0));
        assert!(zone.is_active_at(100));
        assert!(zone.is_active_at(u64::MAX));
    }

    #[test]
    fn nofly_zone_time_window_before_start() {
        let zone = NoFlyZone {
            bounds: Aabb {
                min_x: 0.0,
                max_x: 10.0,
                min_y: 0.0,
                max_y: 10.0,
            },
            active_from_tick: Some(10),
            active_until_tick: Some(20),
        };
        assert!(!zone.is_active_at(5));
        assert!(zone.is_active_at(10));
        assert!(zone.is_active_at(15));
        assert!(zone.is_active_at(20));
        assert!(!zone.is_active_at(25));
    }

    #[test]
    fn check_agent_at_tick_respects_time_window() {
        let config = SafetyConfig {
            geofence: None,
            no_fly_zones: vec![NoFlyZone {
                bounds: Aabb {
                    min_x: 0.0,
                    max_x: 10.0,
                    min_y: 0.0,
                    max_y: 10.0,
                },
                active_from_tick: Some(10),
                active_until_tick: Some(20),
            }],
            separation: None,
        };
        let agent = make_agent("a0", 5.0, 5.0); // inside zone

        // Before zone is active: no violation
        let v = check_agent_at_tick(&config, &agent, &[], 5);
        assert!(v.is_empty(), "expected no violation at tick 5");

        // During zone: violation
        let v = check_agent_at_tick(&config, &agent, &[], 15);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].violation_type, ViolationType::NoFlyZoneEntered);

        // After zone expires: no violation
        let v = check_agent_at_tick(&config, &agent, &[], 25);
        assert!(v.is_empty(), "expected no violation at tick 25");
    }

    #[test]
    fn nofly_zone_serde_optional_tick_fields() {
        // Old JSON without tick fields should deserialize with None
        let json = r#"{"bounds":{"min_x":0.0,"max_x":10.0,"min_y":0.0,"max_y":10.0}}"#;
        let zone: NoFlyZone = serde_json::from_str(json).unwrap();
        assert!(zone.active_from_tick.is_none());
        assert!(zone.active_until_tick.is_none());
    }
}
