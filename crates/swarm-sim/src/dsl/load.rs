use super::ScenarioSuite;

/// Load a `ScenarioSuite` from a JSON file.
pub fn load_scenario_suite(path: &str) -> Result<ScenarioSuite, Box<dyn std::error::Error>> {
    let json = std::fs::read_to_string(path)?;
    let suite: ScenarioSuite = serde_json::from_str(&json)?;
    Ok(suite)
}
