use std::collections::HashMap;
use std::net::SocketAddr;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use swarm_alloc::GreedyAllocator;
use swarm_comms::UdpTransport;
use swarm_runtime::{AgentNode, Coordinator};
use swarm_types::{Agent, AgentId, Task, TaskId};

#[derive(Debug, Deserialize)]
struct ProcessConfig {
    agent_id: AgentId,
    bind_addr: String,
    peers: HashMap<AgentId, String>,
    agents: Vec<Agent>,
    tasks: Vec<Task>,
    timeout_ticks: u64,
    tick_ms: u64,
    max_ticks: u64,
    metrics_path: String,
}

#[derive(Debug, Serialize)]
struct AgentMetrics {
    agent_id: AgentId,
    total_ticks: u64,
    detected_failures: Vec<AgentId>,
    local_task_ids: Vec<TaskId>,
    global_assignment_map: HashMap<TaskId, AgentId>,
    reallocation_count: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let config_path = std::env::args()
        .nth(2)
        .or_else(|| std::env::args().nth(1).filter(|a| a != "--config"))
        .ok_or("usage: agent_process --config <path>")?;

    let config_json = std::fs::read_to_string(&config_path)?;
    let config: ProcessConfig = serde_json::from_str(&config_json)?;

    let bind_addr: SocketAddr = config.bind_addr.parse()?;
    let peers: HashMap<AgentId, SocketAddr> = config
        .peers
        .iter()
        .map(|(id, addr)| {
            let sa: SocketAddr = addr.parse().map_err(|e| format!("invalid addr: {e}"))?;
            Ok((id.clone(), sa))
        })
        .collect::<Result<_, String>>()?;

    let peer_ids: Vec<AgentId> = peers.keys().cloned().collect();

    let transport = UdpTransport::new(bind_addr, peers)?;
    let coordinator = Coordinator::new(
        config.agents.clone(),
        config.tasks.clone(),
        config.timeout_ticks,
    );
    let mut node = AgentNode::new(config.agent_id.clone(), peer_ids, coordinator, transport);

    let mut allocator = GreedyAllocator;
    let mut detected_failures: Vec<AgentId> = Vec::new();
    let mut reallocation_count: u64 = 0;

    // Ensure metrics directory exists
    if let Some(parent) = std::path::Path::new(&config.metrics_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    for tick in 0..config.max_ticks {
        let output = match node.tick(tick, &mut allocator, vec![]) {
            Ok(out) => out,
            Err(e) => {
                tracing::error!(error = %e, "tick failed");
                break;
            }
        };

        for failed in &output.newly_failed {
            if !detected_failures.contains(failed) {
                detected_failures.push(failed.clone());
            }
        }
        reallocation_count += output.released_tasks.len() as u64;

        tracing::info!(
            tick,
            agent = %config.agent_id,
            newly_failed = ?output.newly_failed,
            "tick processed"
        );

        // Write metrics every 10 ticks
        if tick % 10 == 0 && tick > 0 {
            write_metrics(&config, &node, tick, &detected_failures, reallocation_count);
        }

        thread::sleep(Duration::from_millis(config.tick_ms));
    }

    // Final metrics on exit
    write_metrics(
        &config,
        &node,
        config.max_ticks,
        &detected_failures,
        reallocation_count,
    );

    tracing::info!(agent = %config.agent_id, "agent process finished");
    Ok(())
}

fn write_metrics(
    config: &ProcessConfig,
    node: &AgentNode<UdpTransport>,
    total_ticks: u64,
    detected_failures: &[AgentId],
    reallocation_count: u64,
) {
    let local_task_ids: Vec<TaskId> = node
        .coordinator
        .registry
        .tasks()
        .filter(|t| t.assigned_to.as_ref() == Some(&config.agent_id))
        .map(|t| t.id.clone())
        .collect();

    let global_assignment_map: HashMap<TaskId, AgentId> = node
        .coordinator
        .registry
        .tasks()
        .filter_map(|t| {
            t.assigned_to
                .clone()
                .map(|agent_id| (t.id.clone(), agent_id))
        })
        .collect();

    let metrics = AgentMetrics {
        agent_id: config.agent_id.clone(),
        total_ticks,
        detected_failures: detected_failures.to_vec(),
        local_task_ids,
        global_assignment_map,
        reallocation_count,
    };

    let json = serde_json::to_string_pretty(&metrics).unwrap_or_else(|e| {
        tracing::error!(error = %e, "failed to serialize metrics");
        "{}".to_string()
    });

    if let Err(e) = std::fs::write(&config.metrics_path, json) {
        tracing::error!(path = %config.metrics_path, error = %e, "failed to write metrics");
    }
}
