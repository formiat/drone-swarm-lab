#[cfg(test)]
mod tests {
    #[test]
    fn all_scenario_files_load() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios");
        let entries = std::fs::read_dir(dir).expect("scenarios dir exists");
        let mut loaded = 0;
        let mut failed: Vec<String> = Vec::new();
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();
            if path.extension().is_some_and(|e| e == "json") && !file_name.ends_with(".config.json")
            {
                let path_str = path.to_str().unwrap();
                match swarm_sim::load_scenario_suite(path_str) {
                    Ok(_) => loaded += 1,
                    Err(e) => failed.push(format!("{}: {}", path.display(), e)),
                }
            }
        }
        assert!(loaded > 0, "no scenario files loaded");
        if !failed.is_empty() {
            panic!("failed to load: {}", failed.join("\n"));
        }
    }

    #[test]
    fn urban_patrol_scenario_loads_and_validates() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scenarios/urban.patrol.json"
        );
        let suite = swarm_sim::load_scenario_suite(path).expect("urban scenario loads");
        assert_eq!(suite.name, "Urban Patrol Small Block");
        assert_eq!(suite.scenarios.len(), 1);
        let entry = &suite.scenarios[0];
        assert_eq!(entry.mission, "urban-patrol");
        let errors = swarm_sim::validate_entry(entry);
        assert!(
            errors.is_empty(),
            "urban scenario must validate: {errors:?}"
        );
        let urban_state = entry
            .run_config
            .urban_state
            .as_ref()
            .expect("urban_state exists");
        let route =
            swarm_sim::expand_route_loop(&urban_state.map, &urban_state.route_loop).unwrap();
        assert_eq!(route.total_length_m, 80.0);
        assert!(swarm_sim::judge_route(&urban_state.map, &route).is_empty());

        let metrics = swarm_sim::ScenarioRunner::run(&entry.scenario, entry.run_config.clone());
        assert!(metrics.success);
        assert!(metrics.urban_patrol_completed);
        assert_eq!(metrics.urban_time_to_complete_loop, Some(40));
    }

    #[test]
    fn urban_search_scenario_loads_validates_and_detects_bus() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scenarios/urban.search.json"
        );
        let suite = swarm_sim::load_scenario_suite(path).expect("urban search scenario loads");
        assert_eq!(suite.name, "Urban Search Static Bus");
        assert_eq!(suite.scenarios.len(), 1);
        let entry = &suite.scenarios[0];
        assert_eq!(entry.mission, "urban-search");
        let errors = swarm_sim::validate_entry(entry);
        assert!(
            errors.is_empty(),
            "urban search scenario must validate: {errors:?}"
        );
        let urban_search_state = entry
            .run_config
            .urban_search_state
            .as_ref()
            .expect("urban_search_state exists");
        assert!(urban_search_state.validate().is_empty());

        let metrics = swarm_sim::ScenarioRunner::run(&entry.scenario, entry.run_config.clone());
        assert!(metrics.success);
        assert!(metrics.bus_detected);
        assert_eq!(metrics.time_to_detect_bus, Some(2));
        assert!(metrics.search_success_without_violation);
    }

    #[test]
    fn urban_multi_agent_scenario_loads_and_measures_route() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scenarios/urban.multi-agent.json"
        );
        let suite = swarm_sim::load_scenario_suite(path).expect("urban multi-agent scenario loads");
        assert_eq!(suite.name, "Urban Multi-Agent Small Block");
        let entry = &suite.scenarios[0];
        assert_eq!(entry.mission, "urban-patrol");
        assert_eq!(entry.profile, "multi-agent-small-block");
        assert_eq!(entry.scenario.agents.len(), 2);
        let errors = swarm_sim::validate_entry(entry);
        assert!(
            errors.is_empty(),
            "urban multi-agent scenario must validate: {errors:?}"
        );
        let urban_state = entry
            .run_config
            .urban_state
            .as_ref()
            .expect("urban_state exists");
        let route =
            swarm_sim::expand_route_loop(&urban_state.map, &urban_state.route_loop).unwrap();
        assert!(swarm_sim::judge_route(&urban_state.map, &route).is_empty());

        let (metrics, log) = swarm_sim::ScenarioRunner::run_with_log(
            &entry.scenario,
            entry.run_config.clone(),
            swarm_alloc::GreedyAllocator,
        );
        let log = log.expect("urban multi-agent run should produce replay log");
        let trace = swarm_sim::build_urban_route_trace(&log);
        assert_eq!(trace.agents.len(), 2);
        assert!(
            trace
                .agents
                .iter()
                .all(|agent| !agent.pose_trace.is_empty()),
            "each analysis agent should have pose trace data"
        );
        let separation = swarm_sim::measure_urban_separation(
            &trace,
            swarm_sim::URBAN_ANALYSIS_DEFAULT_SEPARATION_THRESHOLD_M,
        );
        assert_eq!(
            metrics.urban_min_agent_separation_m,
            separation.min_separation_m
        );
        assert_eq!(
            metrics.urban_separation_violation_count,
            separation.separation_violation_count
        );
        assert_eq!(
            metrics.urban_route_conflict_count,
            separation.route_conflict_count
        );
        assert!(
            separation
                .min_separation_m
                .is_some_and(|distance| distance > 0.0),
            "two-agent fixture should produce a meaningful separation measurement"
        );
        assert!(
            separation.route_conflict_count > 0,
            "two-agent fixture should produce route-conflict measurements"
        );
    }

    #[test]
    fn urban_corridor_delta_scenario_loads_and_improves_risk() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scenarios/urban.corridor-delta.json"
        );
        let suite = swarm_sim::load_scenario_suite(path).expect("urban corridor scenario loads");
        assert_eq!(suite.name, "Urban Corridor Planner Delta");
        assert_eq!(suite.scenarios.len(), 2);

        let baseline = suite
            .scenarios
            .iter()
            .find(|entry| entry.profile == "corridor-delta-dijkstra")
            .expect("dijkstra profile exists");
        let corridor = suite
            .scenarios
            .iter()
            .find(|entry| entry.profile == "corridor-delta-corridor-aware")
            .expect("corridor-aware profile exists");

        for entry in [baseline, corridor] {
            let errors = swarm_sim::validate_entry(entry);
            assert!(
                errors.is_empty(),
                "urban corridor scenario must validate: {errors:?}"
            );
        }

        let baseline_metrics =
            swarm_sim::ScenarioRunner::run(&baseline.scenario, baseline.run_config.clone());
        let corridor_metrics =
            swarm_sim::ScenarioRunner::run(&corridor.scenario, corridor.run_config.clone());

        assert!(baseline_metrics.success);
        assert!(corridor_metrics.success);
        assert_eq!(baseline_metrics.urban_violation_count, 0);
        assert_eq!(corridor_metrics.urban_violation_count, 0);
        assert!(corridor_metrics.urban_route_length_m > baseline_metrics.urban_route_length_m);
        assert!(
            corridor_metrics.urban_route_risk_score < baseline_metrics.urban_route_risk_score,
            "corridor-aware risk {} should be below dijkstra risk {}",
            corridor_metrics.urban_route_risk_score,
            baseline_metrics.urban_route_risk_score
        );
    }
}
