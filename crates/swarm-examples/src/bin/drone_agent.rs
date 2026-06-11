//! Standalone drone agent process (M92).
//!
//! Usage: `drone_agent --config <path> [--dry-run]`

use std::process::ExitCode;

use swarm_examples::drone_agent_runtime::{run_agent, DroneAgentConfig};

fn main() -> ExitCode {
    match run_cli() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run_cli() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let mut config_path = None;
    let mut dry_run = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = Some(
                    args.get(i)
                        .ok_or_else(|| "missing value for --config".to_owned())?
                        .clone(),
                );
            }
            "--dry-run" => dry_run = true,
            "--help" | "-h" => return Err(usage()),
            other => return Err(format!("unknown argument '{other}'\n{}", usage())),
        }
        i += 1;
    }

    let config_path = config_path.ok_or_else(|| format!("missing --config\n{}", usage()))?;
    let text = std::fs::read_to_string(&config_path)
        .map_err(|error| format!("failed to read config {config_path}: {error}"))?;
    let config: DroneAgentConfig = serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse config {config_path}: {error}"))?;
    let report = run_agent(config, dry_run).map_err(|error| error.to_string())?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}

fn usage() -> String {
    "Usage: drone_agent --config <path> [--dry-run]".to_owned()
}
