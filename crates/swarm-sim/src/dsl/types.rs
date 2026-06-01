use serde::{Deserialize, Serialize};

use crate::{RunConfig, Scenario};

fn default_schema_version() -> String {
    "0.1".to_owned()
}

/// A suite of scenarios with metadata for batch benchmarking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioSuite {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    pub name: String,
    pub description: String,
    pub scenarios: Vec<ScenarioSuiteEntry>,
}

/// A single entry in a scenario suite.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioSuiteEntry {
    pub mission: String,
    pub profile: String,
    pub scenario: Scenario,
    pub run_config: RunConfig,
}

/// Load a `ScenarioSuite` from a JSON file.
pub fn load_scenario_suite(path: &str) -> Result<ScenarioSuite, Box<dyn std::error::Error>> {
    let json = std::fs::read_to_string(path)?;
    let suite: ScenarioSuite = serde_json::from_str(&json)?;
    Ok(suite)
}

/// Serialize a single entry to pretty-printed JSON.
pub fn export_entry(entry: &ScenarioSuiteEntry) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(entry)
}

/// Serialize a full suite to pretty-printed JSON.
pub fn export_suite(suite: &ScenarioSuite) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(suite)
}

/// Typed validation error for scenario suite entries.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

/// Validate a full scenario suite.
pub fn validate_scenario_suite(suite: &ScenarioSuite) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if suite.name.trim().is_empty() {
        errors.push(ValidationError {
            field: "name".to_owned(),
            message: "Suite name must not be empty".to_owned(),
        });
    }

    if suite.scenarios.is_empty() {
        errors.push(ValidationError {
            field: "scenarios".to_owned(),
            message: "Scenario suite must contain at least one scenario".to_owned(),
        });
    }

    if suite.schema_version != "0.1" {
        errors.push(ValidationError {
            field: "schema_version".to_owned(),
            message: format!(
                "Unsupported schema version: {} (expected 0.1)",
                suite.schema_version
            ),
        });
    }

    for (i, entry) in suite.scenarios.iter().enumerate() {
        let mut entry_errors = validate_entry(entry);
        for e in &mut entry_errors {
            e.field = format!("scenarios[{}].{}", i, e.field);
        }
        errors.append(&mut entry_errors);
    }

    errors
}

/// Validate a single scenario suite entry.
pub fn validate_entry(entry: &ScenarioSuiteEntry) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if entry.mission.trim().is_empty() {
        errors.push(ValidationError {
            field: "mission".to_owned(),
            message: "Mission must not be empty".to_owned(),
        });
    }

    if entry.profile.trim().is_empty() {
        errors.push(ValidationError {
            field: "profile".to_owned(),
            message: "Profile must not be empty".to_owned(),
        });
    }

    if entry.scenario.name.trim().is_empty() {
        errors.push(ValidationError {
            field: "scenario.name".to_owned(),
            message: "Scenario name must not be empty".to_owned(),
        });
    }

    if entry.scenario.agents.is_empty() {
        errors.push(ValidationError {
            field: "scenario.agents".to_owned(),
            message: "Scenario must contain at least one agent".to_owned(),
        });
    }

    if entry.scenario.tasks.is_empty() {
        errors.push(ValidationError {
            field: "scenario.tasks".to_owned(),
            message: "Scenario must contain at least one task".to_owned(),
        });
    }

    if entry.run_config.max_ticks == 0 {
        errors.push(ValidationError {
            field: "run_config.max_ticks".to_owned(),
            message: "max_ticks must be greater than 0".to_owned(),
        });
    }

    // Mission-specific constraints
    errors.append(&mut validate_mission_specific(entry));

    errors
}

/// Validate mission-specific constraints.
pub fn validate_mission_specific(entry: &ScenarioSuiteEntry) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    match entry.mission.as_str() {
        "sar" => {
            if entry.run_config.grid_state.is_none() {
                errors.push(ValidationError {
                    field: "run_config.grid_state".to_owned(),
                    message: "SAR mission requires grid_state".to_owned(),
                });
            }
            let has_grid_cell = entry.scenario.tasks.iter().any(|t| t.grid_cell.is_some());
            if !has_grid_cell {
                errors.push(ValidationError {
                    field: "scenario.tasks".to_owned(),
                    message: "SAR mission requires at least one task with grid_cell".to_owned(),
                });
            }
        }
        "inspection" => {
            let has_edge = entry.scenario.tasks.iter().any(|t| t.edge_id.is_some());
            if !has_edge {
                errors.push(ValidationError {
                    field: "scenario.tasks".to_owned(),
                    message: "Inspection mission requires at least one task with edge_id"
                        .to_owned(),
                });
            }
            if !entry.run_config.enable_movement {
                errors.push(ValidationError {
                    field: "run_config.enable_movement".to_owned(),
                    message: "Inspection mission requires enable_movement = true".to_owned(),
                });
            }
        }
        "cbba-stress" => {
            if !entry.run_config.enable_cbba {
                errors.push(ValidationError {
                    field: "run_config.enable_cbba".to_owned(),
                    message: "CBBA-stress mission requires enable_cbba = true".to_owned(),
                });
            }
            if entry.run_config.gossip_interval_ticks > 5 {
                errors.push(ValidationError {
                    field: "run_config.gossip_interval_ticks".to_owned(),
                    message: "CBBA-stress mission requires gossip_interval_ticks <= 5".to_owned(),
                });
            }
        }
        "sitl" => {
            let has_pose = entry.scenario.tasks.iter().any(|t| t.pose.is_some());
            if !has_pose {
                errors.push(ValidationError {
                    field: "scenario.tasks".to_owned(),
                    message: "SITL mission requires at least one task with pose".to_owned(),
                });
            }
        }
        "urban-patrol" => match &entry.run_config.urban_state {
            Some(urban_state) => {
                if let Err(error) = crate::urban::UrbanPlannerMode::parse(&urban_state.planner) {
                    errors.push(ValidationError {
                        field: "run_config.urban_state.planner".to_owned(),
                        message: error.to_string(),
                    });
                }
                if !entry
                    .scenario
                    .agents
                    .iter()
                    .any(|agent| matches!(agent.health, swarm_types::Health::Alive))
                {
                    errors.push(ValidationError {
                        field: "scenario.agents".to_owned(),
                        message: "Urban Patrol requires at least one alive agent".to_owned(),
                    });
                }
                for error in urban_state.map.validate() {
                    errors.push(ValidationError {
                        field: format!("run_config.urban_state.map.{}", error.field),
                        message: error.message,
                    });
                }
                for error in urban_state.map.validate_route_loop(&urban_state.route_loop) {
                    errors.push(ValidationError {
                        field: format!("run_config.urban_state.{}", error.field),
                        message: error.message,
                    });
                }
                match crate::urban::expand_route_loop_with_planner_name(
                    &urban_state.map,
                    &urban_state.route_loop,
                    &urban_state.planner,
                ) {
                    Ok(route) => {
                        match crate::urban::route_start_node(
                            &urban_state.map,
                            &urban_state.route_loop,
                            &route,
                            urban_state.start_node.as_ref(),
                        ) {
                            Ok(start_node) => {
                                validate_urban_start_pose(entry, start_node.pose, &mut errors);
                            }
                            Err(error) => push_urban_state_error(&mut errors, error),
                        }
                        let violations = crate::urban::judge_route(&urban_state.map, &route);
                        for violation in violations {
                            errors.push(ValidationError {
                                field: "run_config.urban_state.route_loop".to_owned(),
                                message: format!("Urban route judge violation: {violation:?}"),
                            });
                        }
                    }
                    Err(error) => errors.push(ValidationError {
                        field: "run_config.urban_state.route_loop".to_owned(),
                        message: error.to_string(),
                    }),
                }
            }
            None => errors.push(ValidationError {
                field: "run_config.urban_state".to_owned(),
                message: "Urban patrol mission requires urban_state".to_owned(),
            }),
        },
        "urban-search" => {
            match &entry.run_config.urban_state {
                Some(urban_state) => {
                    if let Err(error) = crate::urban::UrbanPlannerMode::parse(&urban_state.planner)
                    {
                        errors.push(ValidationError {
                            field: "run_config.urban_state.planner".to_owned(),
                            message: error.to_string(),
                        });
                    }
                    if !entry
                        .scenario
                        .agents
                        .iter()
                        .any(|agent| matches!(agent.health, swarm_types::Health::Alive))
                    {
                        errors.push(ValidationError {
                            field: "scenario.agents".to_owned(),
                            message: "Urban Search requires at least one alive agent".to_owned(),
                        });
                    }
                    for error in urban_state.map.validate() {
                        errors.push(ValidationError {
                            field: format!("run_config.urban_state.map.{}", error.field),
                            message: error.message,
                        });
                    }
                    for error in urban_state.map.validate_route_loop(&urban_state.route_loop) {
                        errors.push(ValidationError {
                            field: format!("run_config.urban_state.{}", error.field),
                            message: error.message,
                        });
                    }
                    match crate::urban::expand_route_loop_with_planner_name(
                        &urban_state.map,
                        &urban_state.route_loop,
                        &urban_state.planner,
                    ) {
                        Ok(route) => {
                            match crate::urban::route_start_node(
                                &urban_state.map,
                                &urban_state.route_loop,
                                &route,
                                urban_state.start_node.as_ref(),
                            ) {
                                Ok(start_node) => {
                                    validate_urban_start_pose(entry, start_node.pose, &mut errors);
                                }
                                Err(error) => push_urban_state_error(&mut errors, error),
                            }
                            let violations = crate::urban::judge_route(&urban_state.map, &route);
                            for violation in violations {
                                errors.push(ValidationError {
                                    field: "run_config.urban_state.route_loop".to_owned(),
                                    message: format!("Urban route judge violation: {violation:?}"),
                                });
                            }
                        }
                        Err(error) => errors.push(ValidationError {
                            field: "run_config.urban_state.route_loop".to_owned(),
                            message: error.to_string(),
                        }),
                    }
                }
                None => errors.push(ValidationError {
                    field: "run_config.urban_state".to_owned(),
                    message: "Urban search mission requires urban_state".to_owned(),
                }),
            }
            match &entry.run_config.urban_search_state {
                Some(search_state) => {
                    for error in search_state.validate() {
                        errors.push(ValidationError {
                            field: format!("run_config.urban_search_state.{}", error.field),
                            message: error.message,
                        });
                    }
                }
                None => errors.push(ValidationError {
                    field: "run_config.urban_search_state".to_owned(),
                    message: "Urban search mission requires urban_search_state".to_owned(),
                }),
            }
        }
        _ => {}
    }

    // Safety scenarios require safety_config
    if entry.profile.contains("safety") && entry.run_config.safety_config.is_none() {
        errors.push(ValidationError {
            field: "run_config.safety_config".to_owned(),
            message: "Safety profile requires safety_config".to_owned(),
        });
    }

    // v0.33: validate task kind and required fields
    for (i, task) in entry.scenario.tasks.iter().enumerate() {
        if let Some(ref kind) = task.kind {
            if !mission_allows_task_kind(entry.mission.as_str(), kind) {
                errors.push(ValidationError {
                    field: format!("scenario.tasks[{i}].kind"),
                    message: format!(
                        "Mission '{}' does not support task kind {:?}",
                        entry.mission, kind
                    ),
                });
            }
            match kind {
                swarm_types::TaskKind::SarScan | swarm_types::TaskKind::SarConfirmationScan => {
                    if task.grid_cell.is_none() {
                        errors.push(ValidationError {
                            field: format!("scenario.tasks[{i}].grid_cell"),
                            message: "SAR task requires grid_cell".to_owned(),
                        });
                    }
                }
                swarm_types::TaskKind::InspectionEdge => {
                    if task.edge_id.is_none() {
                        errors.push(ValidationError {
                            field: format!("scenario.tasks[{i}].edge_id"),
                            message: "Inspection task requires edge_id".to_owned(),
                        });
                    }
                }
                swarm_types::TaskKind::CoverageCell
                | swarm_types::TaskKind::Waypoint
                | swarm_types::TaskKind::RelayPlacement
                | swarm_types::TaskKind::MappingZone => {
                    if task.pose.is_none() {
                        errors.push(ValidationError {
                            field: format!("scenario.tasks[{i}].pose"),
                            message: format!("{:?} task requires pose", kind),
                        });
                    }
                }
            }
        }
    }

    // v0.31: validate battery_model fields if present
    for (i, agent) in entry.scenario.agents.iter().enumerate() {
        if let Some(ref bm) = agent.battery_model {
            if bm.hover_drain_per_tick < 0.0 {
                errors.push(ValidationError {
                    field: format!("scenario.agents[{i}].battery_model.hover_drain_per_tick"),
                    message: "hover_drain_per_tick must be >= 0".to_owned(),
                });
            }
            if bm.climb_drain_per_meter < 0.0 {
                errors.push(ValidationError {
                    field: format!("scenario.agents[{i}].battery_model.climb_drain_per_meter"),
                    message: "climb_drain_per_meter must be >= 0".to_owned(),
                });
            }
            if bm.cruise_drain_per_meter < 0.0 {
                errors.push(ValidationError {
                    field: format!("scenario.agents[{i}].battery_model.cruise_drain_per_meter"),
                    message: "cruise_drain_per_meter must be >= 0".to_owned(),
                });
            }
            if !(0.0..=1.0).contains(&bm.reserve_fraction) {
                errors.push(ValidationError {
                    field: format!("scenario.agents[{i}].battery_model.reserve_fraction"),
                    message: "reserve_fraction must be in [0, 1]".to_owned(),
                });
            }
        }
    }

    // v0.31: validate sensor detection_range_m
    if let Some(ref gs) = entry.run_config.grid_state {
        if gs.sensor.detection_range_m < 0.0 {
            errors.push(ValidationError {
                field: "run_config.grid_state.sensor.detection_range_m".to_owned(),
                message: "detection_range_m must be >= 0".to_owned(),
            });
        }
    }

    // v0.31: validate no-fly zone time windows
    if let Some(ref safety) = entry.run_config.safety_config {
        for (i, nfz) in safety.no_fly_zones.iter().enumerate() {
            if let (Some(from), Some(until)) = (nfz.active_from_tick, nfz.active_until_tick) {
                if from > until {
                    errors.push(ValidationError {
                        field: format!("run_config.safety_config.no_fly_zones[{i}]"),
                        message: format!(
                            "active_from_tick ({from}) must be <= active_until_tick ({until})"
                        ),
                    });
                }
            }
        }
    }

    errors
}

fn push_urban_state_error(errors: &mut Vec<ValidationError>, error: crate::urban::UrbanRouteError) {
    match error {
        crate::urban::UrbanRouteError::InvalidInput { field, message } => {
            errors.push(ValidationError {
                field: format!("run_config.urban_state.{field}"),
                message,
            });
        }
        crate::urban::UrbanRouteError::NoRoute { .. } => {
            errors.push(ValidationError {
                field: "run_config.urban_state.route_loop".to_owned(),
                message: error.to_string(),
            });
        }
    }
}

fn validate_urban_start_pose(
    entry: &ScenarioSuiteEntry,
    start_pose: swarm_types::Pose,
    errors: &mut Vec<ValidationError>,
) {
    let Some((agent_index, agent)) = entry
        .scenario
        .agents
        .iter()
        .enumerate()
        .find(|(_, agent)| matches!(agent.health, swarm_types::Health::Alive))
    else {
        return;
    };
    let distance = agent.pose.distance_to(&start_pose);
    if distance > crate::urban::URBAN_START_POSE_TOLERANCE_M {
        errors.push(ValidationError {
            field: format!("scenario.agents[{agent_index}].pose"),
            message: format!(
                "Urban selected agent must start within {:.2}m of start_node pose; distance was {:.3}m",
                crate::urban::URBAN_START_POSE_TOLERANCE_M,
                distance
            ),
        });
    }
}

fn mission_allows_task_kind(mission: &str, kind: &swarm_types::TaskKind) -> bool {
    match mission {
        "sar" => matches!(
            kind,
            swarm_types::TaskKind::SarScan | swarm_types::TaskKind::SarConfirmationScan
        ),
        "inspection" => matches!(kind, swarm_types::TaskKind::InspectionEdge),
        "wildfire" => matches!(kind, swarm_types::TaskKind::MappingZone),
        "sitl" => matches!(kind, swarm_types::TaskKind::Waypoint),
        "urban-patrol" | "urban-search" => matches!(kind, swarm_types::TaskKind::Waypoint),
        "coverage" => matches!(kind, swarm_types::TaskKind::CoverageCell),
        "emergency-mesh" => matches!(
            kind,
            swarm_types::TaskKind::CoverageCell | swarm_types::TaskKind::RelayPlacement
        ),
        _ => true,
    }
}
