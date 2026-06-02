use serde::{Deserialize, Serialize};

/// Benchmark run manifest for reproducibility.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkManifest {
    pub timestamp: String,
    pub git_commit: String,
    pub command_line: String,
    pub suite_name: String,
    pub schema_version: String,
    pub seed_range_start: u64,
    pub seed_range_end: u64,
    pub strategy_names: Vec<String>,
    pub profile_names: Vec<String>,
    pub metric_schema_version: String,
    // v0.31 Realism metadata
    #[serde(default)]
    pub realism_profile: Option<String>,
    #[serde(default)]
    pub wind_enabled: bool,
    #[serde(default)]
    pub pose_noise_m: f64,
    #[serde(default)]
    pub comms_jitter_ticks: u64,
    // v0.37 Battery model metadata
    #[serde(default)]
    pub battery_model: Option<swarm_types::BatteryModel>,
    /// Number of rayon worker threads used; `None` means all available CPUs.
    #[serde(default)]
    pub jobs: Option<usize>,
    /// Cargo build profile when known (`debug` or `release`).
    #[serde(default)]
    pub build_profile: Option<String>,
}

impl BenchmarkManifest {
    pub fn new(
        suite_name: impl Into<String>,
        seed_range_start: u64,
        seed_range_end: u64,
        strategy_names: Vec<String>,
        profile_names: Vec<String>,
    ) -> Self {
        let git_commit = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_else(|| "unknown".to_owned())
            .trim()
            .to_owned();

        let command_line = std::env::args().collect::<Vec<_>>().join(" ");

        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            git_commit,
            command_line,
            suite_name: suite_name.into(),
            schema_version: "0.1".to_owned(),
            seed_range_start,
            seed_range_end,
            strategy_names,
            profile_names,
            metric_schema_version: "0.1".to_owned(),
            realism_profile: None,
            wind_enabled: false,
            pose_noise_m: 0.0,
            comms_jitter_ticks: 0,
            battery_model: None,
            jobs: None,
            build_profile: Some(
                if cfg!(debug_assertions) {
                    "debug"
                } else {
                    "release"
                }
                .to_owned(),
            ),
        }
    }
}
