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

/// Typed validation error for scenario suite entries.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}
