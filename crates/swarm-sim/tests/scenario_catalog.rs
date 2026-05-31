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
    }
}
