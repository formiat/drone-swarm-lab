use super::{ScenarioSuite, ScenarioSuiteEntry};

/// Serialize a single entry to pretty-printed JSON.
pub fn export_entry(entry: &ScenarioSuiteEntry) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(entry)
}

/// Serialize a full suite to pretty-printed JSON.
pub fn export_suite(suite: &ScenarioSuite) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(suite)
}
