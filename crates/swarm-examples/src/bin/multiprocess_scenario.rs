use std::collections::HashMap;
use std::net::UdpSocket;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use swarm_types::{Agent, AgentId, Capability, Health, Pose, Role, Task, TaskId, TaskStatus};

#[derive(Debug, Deserialize)]
struct AgentMetrics {
    #[allow(dead_code)]
    agent_id: AgentId,
    #[allow(dead_code)]
    total_ticks: u64,
    detected_failures: Vec<AgentId>,
    #[allow(dead_code)]
    local_task_ids: Vec<TaskId>,
    global_assignment_map: HashMap<TaskId, AgentId>,
    #[allow(dead_code)]
    reallocation_count: u64,
}

const N: usize = 5;
const TICK_MS: u64 = 100;
const TIMEOUT_TICKS: u64 = 5;
const MAX_TICKS: u64 = 200;

fn allocate_udp_ports(n: usize) -> Vec<u16> {
    (0..n)
        .map(|_| {
            let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
            sock.local_addr().unwrap().port()
        })
        .collect()
}

fn make_agent(id: &str) -> Agent {
    Agent {
        id: AgentId::from(id.to_owned()),
        role: Role::Scout,
        health: Health::Alive,
        pose: Pose { x: 0.0, y: 0.0 },
        capabilities: vec![Capability::from("basic".to_owned())],
        current_task: None,
        battery: 100.0,
        generation: 1,
    }
}

fn make_task(id: &str) -> Task {
    Task {
        id: TaskId::from(id.to_owned()),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority: 1,
        required_capabilities: vec![Capability::from("basic".to_owned())],
        preferred_role: None,
        expires_at: None,
        pose: None,
    }
}

fn make_peer_map(
    agent_ids: &[AgentId],
    ports: &[u16],
    own_index: usize,
) -> HashMap<AgentId, String> {
    let mut peers = HashMap::new();
    for (i, id) in agent_ids.iter().enumerate() {
        if i != own_index {
            peers.insert(id.clone(), format!("127.0.0.1:{}", ports[i]));
        }
    }
    peers
}

fn main() {
    let tmp_dir = "/tmp/swarm-v03";
    std::fs::create_dir_all(tmp_dir).expect("create tmp dir");

    let ports = allocate_udp_ports(N);
    eprintln!("allocated ports: {ports:?}");

    let agent_ids: Vec<AgentId> = (0..N)
        .map(|i| AgentId::from(format!("agent-{i}")))
        .collect();

    let agents: Vec<Agent> = agent_ids.iter().map(|id| make_agent(id)).collect();
    let tasks: Vec<Task> = (0..8).map(|i| make_task(&format!("task-{i}"))).collect();

    // Write config files and launch children
    let mut children: Vec<(AgentId, Child)> = Vec::new();
    let binary = std::env::current_exe()
        .ok()
        .and_then(|p| {
            // cargo run sets current_exe to the target binary, but we need agent_process
            // Try to find it relative to current_exe
            let parent = p.parent()?;
            Some(parent.join("agent_process"))
        })
        .unwrap_or_else(|| {
            // Fallback: try target/debug/agent_process
            std::path::PathBuf::from("target/debug/agent_process")
        });

    for i in 0..N {
        let config = serde_json::json!({
            "agent_id": agent_ids[i].to_string(),
            "bind_addr": format!("127.0.0.1:{}", ports[i]),
            "peers": make_peer_map(&agent_ids, &ports, i),
            "agents": agents,
            "tasks": tasks,
            "timeout_ticks": TIMEOUT_TICKS,
            "tick_ms": TICK_MS,
            "max_ticks": MAX_TICKS,
            "metrics_path": format!("{}/agent-{}.json", tmp_dir, i),
        });

        let config_path = format!("{tmp_dir}/config-{i}.json");
        std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .expect("write config");

        let child = Command::new(&binary)
            .arg("--config")
            .arg(&config_path)
            .env("RUST_LOG", "info")
            .spawn()
            .expect("spawn agent_process");

        eprintln!("started agent-{i} (pid={})", child.id());
        children.push((agent_ids[i].clone(), child));
    }

    // Stabilization period
    eprintln!("waiting 2s for stabilization...");
    thread::sleep(Duration::from_secs(2));

    // Kill agent-0
    eprintln!("killing agent-0...");
    children[0].1.kill().expect("kill agent-0");
    children[0].1.wait().expect("wait agent-0");

    // Wait for failure detection
    eprintln!("waiting 3s for failure detection...");
    thread::sleep(Duration::from_secs(3));

    // Stop remaining agents — kill all at once so they run the same number of ticks
    for (id, child) in &mut children[1..] {
        eprintln!("stopping {id}...");
        let _ = child.kill();
    }
    // Wait for all survivors to write final metrics
    thread::sleep(Duration::from_millis(500));
    for (_id, child) in &mut children[1..] {
        let _ = child.wait();
    }

    // Read metrics and verify
    let mut all_survivors_metrics: Vec<AgentMetrics> = Vec::new();
    for i in 1..N {
        let path = format!("{tmp_dir}/agent-{i}.json");
        match std::fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str::<AgentMetrics>(&json) {
                Ok(metrics) => {
                    eprintln!("agent-{i} metrics: total_ticks={}", metrics.total_ticks);
                    all_survivors_metrics.push(metrics);
                }
                Err(e) => eprintln!("failed to parse agent-{i} metrics: {e}"),
            },
            Err(e) => eprintln!("failed to read agent-{i} metrics: {e}"),
        }
    }

    if all_survivors_metrics.len() < (N - 1) {
        eprintln!(
            "ERROR: expected {} survivors, got {}",
            N - 1,
            all_survivors_metrics.len()
        );
        std::process::exit(1);
    }

    // Verify invariants
    let mut errors = Vec::new();

    // 1. All survivors detected agent-0 as failed
    for metrics in &all_survivors_metrics {
        let agent_0 = AgentId::from("agent-0".to_owned());
        if !metrics.detected_failures.contains(&agent_0) {
            errors.push(format!(
                "agent {} did not detect agent-0 failure (detected: {:?})",
                metrics.agent_id, metrics.detected_failures
            ));
        }
    }

    // 2. Check convergence — hard failure, per PLAN.md and README.md invariant
    if all_survivors_metrics.len() >= 2 {
        let reference = &all_survivors_metrics[0].global_assignment_map;
        for metrics in &all_survivors_metrics[1..] {
            if &metrics.global_assignment_map != reference {
                errors.push(format!(
                    "agent {} assignment map differs from agent {}",
                    metrics.agent_id, all_survivors_metrics[0].agent_id
                ));
            }
        }
    }

    // 3. No task belongs to agent-0
    let agent_0 = AgentId::from("agent-0".to_owned());
    for metrics in &all_survivors_metrics {
        for (task_id, owner) in &metrics.global_assignment_map {
            if owner == &agent_0 {
                errors.push(format!(
                    "task {task_id} still assigned to agent-0 in agent {}'s view",
                    metrics.agent_id
                ));
            }
        }
    }

    // 4. All tasks are assigned
    let expected_tasks: Vec<TaskId> = (0..8).map(|i| TaskId::from(format!("task-{i}"))).collect();
    for metrics in &all_survivors_metrics {
        for task_id in &expected_tasks {
            if !metrics.global_assignment_map.contains_key(task_id) {
                errors.push(format!(
                    "task {task_id} is not assigned in agent {}'s view",
                    metrics.agent_id
                ));
            }
        }
    }

    if errors.is_empty() {
        println!("PASS: all invariants satisfied (including assignment map convergence)");
        std::process::exit(0);
    } else {
        for error in &errors {
            eprintln!("FAIL: {error}");
        }
        std::process::exit(1);
    }
}
