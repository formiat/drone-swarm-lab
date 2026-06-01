fn main() {
    if let Err(error) = swarm_examples::sitl_agent_runtime::run() {
        eprintln!("error: {error}");
        eprintln!(
            "usage: sitl_agent --mock|--dry-run|--connection <addr> --scenario <path> --agent-id <id> [--multi-agent-config <path>] [--safety-config <path>] [--allow-hardware-candidate] [--upload-only|--execute] [--no-arm] [--abort-after <seconds>] [--timeout <seconds>] [--telemetry-timeout <seconds>] [--no-progress-timeout <seconds>] [--run-report <path>] [--replay-log <path>]"
        );
        std::process::exit(1);
    }
}
