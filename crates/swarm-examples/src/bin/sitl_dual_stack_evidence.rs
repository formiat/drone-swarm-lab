use std::path::PathBuf;
use std::process::ExitCode;

use swarm_examples::sitl_dual_stack_evidence::write_dual_stack_evidence_pack;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("sitl_dual_stack_evidence: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let cli = CliArgs::parse()?;
    let pack =
        write_dual_stack_evidence_pack(&cli.scenario, &cli.agent_id, &cli.output_dir, cli.force)
            .map_err(|error| error.to_string())?;
    println!(
        "dual-stack evidence written: {} profiles={} command_ir_hash={}",
        cli.output_dir.display(),
        pack.profiles.len(),
        pack.command_ir_hash
    );
    Ok(())
}

#[derive(Debug)]
struct CliArgs {
    scenario: PathBuf,
    agent_id: String,
    output_dir: PathBuf,
    force: bool,
}

impl CliArgs {
    fn parse() -> Result<Self, String> {
        let args: Vec<String> = std::env::args().collect();
        let mut scenario = None;
        let mut agent_id = None;
        let mut output_dir = None;
        let mut force = false;
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--scenario" => {
                    i += 1;
                    scenario = Some(PathBuf::from(
                        args.get(i)
                            .ok_or_else(|| "missing value for --scenario".to_owned())?,
                    ));
                }
                "--agent-id" => {
                    i += 1;
                    agent_id = Some(
                        args.get(i)
                            .ok_or_else(|| "missing value for --agent-id".to_owned())?
                            .to_owned(),
                    );
                }
                "--output-dir" => {
                    i += 1;
                    output_dir =
                        Some(PathBuf::from(args.get(i).ok_or_else(|| {
                            "missing value for --output-dir".to_owned()
                        })?));
                }
                "--force" => force = true,
                "--help" | "-h" => return Err(usage()),
                other => return Err(format!("unknown argument '{other}'\n{}", usage())),
            }
            i += 1;
        }
        Ok(Self {
            scenario: scenario
                .ok_or_else(|| format!("missing required --scenario <path>\n{}", usage()))?,
            agent_id: agent_id
                .ok_or_else(|| format!("missing required --agent-id <id>\n{}", usage()))?,
            output_dir: output_dir
                .ok_or_else(|| format!("missing required --output-dir <path>\n{}", usage()))?,
            force,
        })
    }
}

fn usage() -> String {
    "usage: sitl_dual_stack_evidence --scenario <path> --agent-id <id> --output-dir <path> [--force]"
        .to_owned()
}
