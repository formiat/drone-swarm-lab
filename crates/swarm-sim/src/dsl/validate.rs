use std::collections::HashSet;

use super::urban_validate::{validate_urban_patrol_entry, validate_urban_search_entry};
use super::{
    ScenarioGeneratorManifest, ScenarioSuite, ScenarioSuiteEntry, ValidationError,
    SCENARIO_GENERATOR_MANIFEST_SCHEMA_VERSION,
};
use crate::PrimitiveMission;

fn is_primitive_mission_name(mission: &str) -> bool {
    matches!(
        mission,
        "hover" | "orbit" | "takeoff-land" | "takeoff-hold-land" | "waypoint-square"
    )
}

fn validate_primitive_mission(mission: &PrimitiveMission, errors: &mut Vec<ValidationError>) {
    fn positive_finite(field: &str, value: f64, errors: &mut Vec<ValidationError>) {
        if !value.is_finite() || value <= 0.0 {
            errors.push(ValidationError {
                field: field.to_owned(),
                message: format!("{field} must be finite and greater than 0"),
            });
        }
    }

    match mission {
        PrimitiveMission::Hover {
            altitude_m,
            hold_seconds,
        } => {
            positive_finite(
                "run_config.primitive_mission.altitude_m",
                *altitude_m,
                errors,
            );
            positive_finite(
                "run_config.primitive_mission.hold_seconds",
                f64::from(*hold_seconds),
                errors,
            );
        }
        PrimitiveMission::Orbit {
            altitude_m,
            turns,
            radius_m,
        } => {
            positive_finite(
                "run_config.primitive_mission.altitude_m",
                *altitude_m,
                errors,
            );
            positive_finite(
                "run_config.primitive_mission.turns",
                f64::from(*turns),
                errors,
            );
            positive_finite(
                "run_config.primitive_mission.radius_m",
                f64::from(*radius_m),
                errors,
            );
        }
        PrimitiveMission::TakeoffLand { altitude_m } => {
            positive_finite(
                "run_config.primitive_mission.altitude_m",
                *altitude_m,
                errors,
            );
        }
        PrimitiveMission::WaypointSquare { altitude_m, side_m } => {
            positive_finite(
                "run_config.primitive_mission.altitude_m",
                *altitude_m,
                errors,
            );
            positive_finite("run_config.primitive_mission.side_m", *side_m, errors);
        }
    }
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

    if let Some(manifest) = &suite.generator_manifest {
        validate_generator_manifest(manifest, &mut errors);
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

fn validate_generator_manifest(
    manifest: &ScenarioGeneratorManifest,
    errors: &mut Vec<ValidationError>,
) {
    if manifest.schema_version != SCENARIO_GENERATOR_MANIFEST_SCHEMA_VERSION {
        errors.push(ValidationError {
            field: "generator_manifest.schema_version".to_owned(),
            message: format!(
                "Unsupported generator manifest schema version: {} (expected {SCENARIO_GENERATOR_MANIFEST_SCHEMA_VERSION})",
                manifest.schema_version
            ),
        });
    }
    for (field, value) in [
        ("generator_name", manifest.generator_name.as_str()),
        ("generator_version", manifest.generator_version.as_str()),
        ("category", manifest.category.as_str()),
    ] {
        if value.trim().is_empty() {
            errors.push(ValidationError {
                field: format!("generator_manifest.{field}"),
                message: format!("{field} must not be empty"),
            });
        }
    }

    let mut keys = HashSet::new();
    for (index, parameter) in manifest.parameters.iter().enumerate() {
        if parameter.key.trim().is_empty() {
            errors.push(ValidationError {
                field: format!("generator_manifest.parameters[{index}].key"),
                message: "parameter key must not be empty".to_owned(),
            });
        }
        if !keys.insert(parameter.key.as_str()) {
            errors.push(ValidationError {
                field: format!("generator_manifest.parameters[{index}].key"),
                message: format!("duplicate generator parameter key '{}'", parameter.key),
            });
        }
    }
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

    if !is_primitive_mission_name(&entry.mission) && entry.scenario.tasks.is_empty() {
        errors.push(ValidationError {
            field: "scenario.tasks".to_owned(),
            message: "Scenario must contain at least one task".to_owned(),
        });
    }

    if let Some(origin) = entry.scenario.geo_origin {
        validate_geo_origin(origin, &mut errors);
    }

    if entry.run_config.max_ticks == 0 {
        errors.push(ValidationError {
            field: "run_config.max_ticks".to_owned(),
            message: "max_ticks must be greater than 0".to_owned(),
        });
    }

    // Mission-specific constraints
    errors.append(&mut validate_mission_specific(entry));
    errors.append(&mut validate_preflight_errors(entry));

    errors
}

pub fn run_preflight_report(
    entry: &ScenarioSuiteEntry,
) -> swarm_safety::preflight::SafetyValidationReport {
    crate::preflight::run_preflight(entry)
}

fn validate_preflight_errors(entry: &ScenarioSuiteEntry) -> Vec<ValidationError> {
    run_preflight_report(entry)
        .violations
        .into_iter()
        .filter(|violation| violation.severity == swarm_safety::preflight::ViolationSeverity::Error)
        .map(|violation| ValidationError {
            field: violation.rule_id,
            message: violation.reason,
        })
        .collect()
}

fn validate_geo_origin(origin: crate::scenario::GeoOrigin, errors: &mut Vec<ValidationError>) {
    if !origin.lat_deg.is_finite() || !(-90.0..=90.0).contains(&origin.lat_deg) {
        errors.push(ValidationError {
            field: "scenario.geo_origin.lat_deg".to_owned(),
            message: "lat_deg must be finite and within [-90, 90]".to_owned(),
        });
    }
    if !origin.lon_deg.is_finite() || !(-180.0..=180.0).contains(&origin.lon_deg) {
        errors.push(ValidationError {
            field: "scenario.geo_origin.lon_deg".to_owned(),
            message: "lon_deg must be finite and within [-180, 180]".to_owned(),
        });
    }
    if !origin.alt_m.is_finite() {
        errors.push(ValidationError {
            field: "scenario.geo_origin.alt_m".to_owned(),
            message: "alt_m must be finite".to_owned(),
        });
    }
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
        "urban-patrol" => validate_urban_patrol_entry(entry, &mut errors),
        "urban-search" => validate_urban_search_entry(entry, &mut errors),
        mission if is_primitive_mission_name(mission) => {
            if let Some(mission) = &entry.run_config.primitive_mission {
                validate_primitive_mission(mission, &mut errors);
            } else {
                errors.push(ValidationError {
                    field: "run_config.primitive_mission".to_owned(),
                    message: format!(
                        "{} mission requires run_config.primitive_mission",
                        entry.mission
                    ),
                });
            }
            if !entry.scenario.tasks.is_empty() {
                errors.push(ValidationError {
                    field: "scenario.tasks".to_owned(),
                    message: format!("{} mission must have an empty task list", entry.mission),
                });
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
