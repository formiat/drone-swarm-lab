use super::{ScenarioSuiteEntry, ValidationError};

pub(super) fn validate_urban_patrol_entry(
    entry: &ScenarioSuiteEntry,
    errors: &mut Vec<ValidationError>,
) {
    validate_urban_route_state(
        entry,
        "Urban Patrol requires at least one alive agent",
        "Urban patrol mission requires urban_state",
        errors,
    );
}

pub(super) fn validate_urban_search_entry(
    entry: &ScenarioSuiteEntry,
    errors: &mut Vec<ValidationError>,
) {
    validate_urban_route_state(
        entry,
        "Urban Search requires at least one alive agent",
        "Urban search mission requires urban_state",
        errors,
    );

    match &entry.run_config.urban_search_state {
        Some(search_state) => {
            let validation_errors = entry
                .run_config
                .urban_state
                .as_ref()
                .map(|urban_state| search_state.validate_with_map(&urban_state.map))
                .unwrap_or_else(|| search_state.validate());
            for error in validation_errors {
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

fn validate_urban_route_state(
    entry: &ScenarioSuiteEntry,
    no_alive_agent_message: &str,
    missing_state_message: &str,
    errors: &mut Vec<ValidationError>,
) {
    match &entry.run_config.urban_state {
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
                    message: no_alive_agent_message.to_owned(),
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
                            validate_urban_start_pose(entry, start_node.pose, errors);
                        }
                        Err(error) => push_urban_state_error(errors, error),
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
            message: missing_state_message.to_owned(),
        }),
    }
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
