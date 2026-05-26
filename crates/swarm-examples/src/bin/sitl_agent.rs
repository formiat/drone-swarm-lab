use std::path::Path;

#[cfg(feature = "mavlink-transport")]
use swarm_comms::Transport;
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

    let suite = match load_scenario_suite(&cli.scenario) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error loading {}: {}", cli.scenario, e);
            std::process::exit(1);
        }
    };

    let errors = swarm_sim::validate_scenario_suite(&suite);
    if !errors.is_empty() {
        eprintln!("Validation failed for {}:", cli.scenario);
        for err in &errors {
            eprintln!("  [{}] {}", err.field, err.message);
        }
        std::process::exit(1);
    }

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

    if agent_tasks.is_empty() {
        eprintln!("Warning: no tasks with pose found in scenario. No waypoints to send.");
        std::process::exit(1);
    }

    eprintln!(
        "SITL Agent: {} | {} tasks with pose | mock={}",
        cli.agent_id,
        agent_tasks.len(),
        cli.mock
    );

    if cli.mock {
        // Mock path: always works, no external dependencies
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
        eprintln!("Mock mode: {} waypoints sent.", transport.waypoints().len());
    } else if let Some(connection_string) = cli.connection {
        // Real MAVLink path: only with feature "mavlink-transport"
        #[cfg(feature = "mavlink-transport")]
        {
            use swarm_comms::MavlinkTransport;
            let agent_id = swarm_types::AgentId::from(cli.agent_id.clone());
            let mut transport =
                MavlinkTransport::new(&connection_string, agent_id).unwrap_or_else(|e| {
                    eprintln!("Failed to connect to MAVLink: {}", e);
                    std::process::exit(1);
                });
            for (idx, task) in agent_tasks.iter().enumerate() {
                if let Some(wp) = task_to_waypoint(task) {
                    let msg = format!(
                        "WAYPOINT seq={} x={:.1} y={:.1} z={:.1}",
                        idx, wp.x, wp.y, wp.z
                    );
                    eprintln!("{msg}");
                    let raw = swarm_comms::RawMessage {
                        from: swarm_types::AgentId::from(cli.agent_id.clone()),
                        to: swarm_types::AgentId::from("px4".to_owned()),
                        payload: msg.into_bytes(),
                    };
                    if let Err(e) = transport.send(raw) {
                        eprintln!("Failed to send waypoint: {}", e);
                    }
                }
            }
            eprintln!("Real MAVLink mode: waypoints sent.");
        }
        #[cfg(not(feature = "mavlink-transport"))]
        {
            let _ = connection_string;
            eprintln!("Error: --connection requires feature 'mavlink-transport'.");
            eprintln!("  Build with: cargo build --bin sitl_agent --features mavlink-transport");
            std::process::exit(1);
        }
    } else {
        eprintln!("Error: specify --mock or --connection <addr>");
        std::process::exit(1);
    }

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
                kind: None,
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
                kind: None,
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

    #[test]
    fn sitl_agent_mock_warns_zero_pose_tasks() {
        let tasks = [Task {
            id: TaskId::from("t0".to_owned()),
            status: TaskStatus::Unassigned,
            assigned_to: None,
            priority: 1,
            required_capabilities: vec![],
            required_role: None,
            preferred_role: None,
            expires_at: None,
            pose: None,
            grid_cell: None,
            kind: None,
            edge_id: None,
        }];
        let pose_tasks: Vec<_> = tasks.iter().filter(|t| t.pose.is_some()).collect();
        assert!(pose_tasks.is_empty());
    }
}
