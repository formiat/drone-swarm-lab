use std::path::PathBuf;
use std::process::ExitCode;

use swarm_examples::artifact_validator::{
    validate_artifact_pack, ArtifactPackPaths, ArtifactValidationMode, ArtifactValidationOptions,
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("artifact_validator: {message}");
            ExitCode::from(3)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let cli = CliArgs::parse()?;
    let paths = ArtifactPackPaths::from_output_dir(&cli.output_dir);
    let report = validate_artifact_pack(
        &paths,
        ArtifactValidationOptions {
            mode: cli.mode,
            allow_historical: cli.allow_historical,
            strict: cli.strict,
        },
    );

    if cli.json {
        let json = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("report serialization failed: {error}"))?;
        println!("{json}");
    } else if report.passed {
        println!(
            "artifact validation passed: {}",
            report.output_dir.display()
        );
    } else {
        eprintln!(
            "artifact validation failed: {}",
            report.output_dir.display()
        );
        for violation in &report.violations {
            let path = violation
                .path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".to_owned());
            eprintln!(
                "- {} {:?} {}: {}",
                violation.rule_id, violation.severity, path, violation.reason
            );
        }
    }

    if report.passed {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(2))
    }
}

#[derive(Debug)]
struct CliArgs {
    output_dir: PathBuf,
    mode: ArtifactValidationMode,
    allow_historical: bool,
    strict: bool,
    json: bool,
}

impl CliArgs {
    fn parse() -> Result<Self, String> {
        let args: Vec<String> = std::env::args().collect();
        let mut output_dir = None;
        let mut mode = ArtifactValidationMode::SupervisorRun;
        let mut allow_historical = false;
        let mut strict = false;
        let mut json = false;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--output-dir" => {
                    i += 1;
                    output_dir =
                        Some(PathBuf::from(args.get(i).ok_or_else(|| {
                            "missing value for --output-dir".to_owned()
                        })?));
                }
                "--mode" => {
                    i += 1;
                    mode = parse_mode(
                        args.get(i)
                            .ok_or_else(|| "missing value for --mode".to_owned())?,
                    )?;
                }
                "--allow-historical" => allow_historical = true,
                "--strict" => strict = true,
                "--json" => json = true,
                "--help" | "-h" => return Err(usage()),
                other => return Err(format!("unknown argument '{other}'\n{}", usage())),
            }
            i += 1;
        }

        let output_dir = output_dir
            .ok_or_else(|| format!("missing required argument --output-dir <path>\n{}", usage()))?;
        Ok(Self {
            output_dir,
            mode,
            allow_historical,
            strict,
            json,
        })
    }
}

fn parse_mode(value: &str) -> Result<ArtifactValidationMode, String> {
    match value {
        "supervisor-run" => Ok(ArtifactValidationMode::SupervisorRun),
        "dry-run" => Ok(ArtifactValidationMode::DryRun),
        "dual-stack-evidence" => Ok(ArtifactValidationMode::DualStackEvidence),
        "dual-stack-execution" => Ok(ArtifactValidationMode::DualStackExecution),
        "historical" => Ok(ArtifactValidationMode::Historical),
        "benchmark-pack" => Ok(ArtifactValidationMode::BenchmarkPack),
        "urban-operational" => Ok(ArtifactValidationMode::UrbanOperational),
        _ => Err(format!(
            "unsupported --mode '{value}' (expected supervisor-run, dry-run, dual-stack-evidence, dual-stack-execution, historical, benchmark-pack, or urban-operational)"
        )),
    }
}

fn usage() -> String {
    "usage: artifact_validator --output-dir <path> [--mode supervisor-run|dry-run|dual-stack-evidence|dual-stack-execution|historical|benchmark-pack|urban-operational] [--allow-historical] [--strict] [--json]".to_owned()
}
