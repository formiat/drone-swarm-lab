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
            if path.extension().is_some_and(|e| e == "json") {
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
}
