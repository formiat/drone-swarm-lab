use std::path::Path;

use swarm_comms::{task_to_waypoint, MockMavlinkTransport};
use swarm_sim::load_scenario_suite;

struct CliArgs {
    mock: bool,
    connection: Option<String>,
    scenario: String,
    agent_id: String,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut cli = CliArgs {
        mock: false,
        connection: None,
        scenario: String::new(),
        agent_id: String::new(),
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--mock" => cli.mock = true,
            "--connection" => {
                i += 1;
                if i < args.len() {
                    cli.connection = Some(args[i].clone());
                }
            }
            "--scenario" => {
                i += 1;
                if i < args.len() {
                    cli.scenario = args[i].clone();
                }
            }
            "--agent-id" => {
                i += 1;
                if i < args.len() {
                    cli.agent_id = args[i].clone();
                }
            }
            _ => {}
        }
        i += 1;
    }

    cli
}

fn main() {
    let cli = parse_args();

    if cli.scenario.is_empty() {
        eprintln!("Usage: sitl_agent --mock|--connection <addr> --scenario <path> --agent-id <id>");
        std::process::exit(1);
    }

    let scenario_path = Path::new(&cli.scenario);
    if !scenario_path.exists() {
        eprintln!("Scenario file not found: {}", cli.scenario);
        std::process::exit(1);
    }

    let suite = load_scenario_suite(&cli.scenario)
        .unwrap_or_else(|e| panic!("Failed to load scenario suite: {e}"));

    if suite.scenarios.is_empty() {
        eprintln!("Scenario suite is empty");
        std::process::exit(1);
    }

    let entry = &suite.scenarios[0];
    let agent_tasks: Vec<_> = entry
        .scenario
        .tasks
        .iter()
        .filter(|t| t.pose.is_some())
        .collect();

    eprintln!(
        "SITL Agent: {} | {} tasks with pose | mock={}",
        cli.agent_id,
        agent_tasks.len(),
        cli.mock
    );

    let mut transport = MockMavlinkTransport::new();

    for (idx, task) in agent_tasks.iter().enumerate() {
        if let Some(wp) = task_to_waypoint(task) {
            let msg = format!(
                "WAYPOINT seq={} x={:.1} y={:.1} z={:.1}",
                idx, wp.x, wp.y, wp.z
            );
            eprintln!("{msg}");
            transport.send_waypoint(wp);
        }
    }

    eprintln!("All waypoints sent. Completed.");
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use swarm_types::{Pose, Task, TaskId, TaskStatus};

    #[test]
    fn sitl_agent_mock_sends_all_waypoints() {
        let tasks = vec![
            Task {
                id: TaskId::from("wp-0".to_owned()),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: 1,
                required_capabilities: vec![],
                required_role: None,
                preferred_role: None,
                expires_at: None,
                pose: Some(Pose { x: 10.0, y: 20.0 }),
                grid_cell: None,
                edge_id: None,
            },
            Task {
                id: TaskId::from("wp-1".to_owned()),
                status: TaskStatus::Unassigned,
                assigned_to: None,
                priority: 1,
                required_capabilities: vec![],
                required_role: None,
                preferred_role: None,
                expires_at: None,
                pose: Some(Pose { x: 30.0, y: 40.0 }),
                grid_cell: None,
                edge_id: None,
            },
        ];

        let mut transport = MockMavlinkTransport::new();
        for task in &tasks {
            if let Some(wp) = task_to_waypoint(task) {
                transport.send_waypoint(wp);
            }
        }

        assert_eq!(transport.waypoints().len(), 2);
        assert!((transport.waypoints()[0].x - 10.0).abs() < 1e-6);
        assert!((transport.waypoints()[1].y - 40.0).abs() < 1e-6);
    }
}
