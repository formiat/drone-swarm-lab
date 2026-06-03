use serde::{Deserialize, Serialize};

use crate::{RunConfig, Scenario};

pub const SCENARIO_GENERATOR_MANIFEST_SCHEMA_VERSION: &str = "scenario_generator_manifest.v1";

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator_manifest: Option<ScenarioGeneratorManifest>,
    pub scenarios: Vec<ScenarioSuiteEntry>,
}

/// Reproducibility metadata for suites produced by a deterministic generator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioGeneratorManifest {
    pub schema_version: String,
    pub generator_name: String,
    pub generator_version: String,
    pub seed: u64,
    pub category: String,
    pub parameters: Vec<ScenarioGeneratorParameter>,
}

/// A stable key/value setting captured in a generator manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioGeneratorParameter {
    pub key: String,
    pub value: String,
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
