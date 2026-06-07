//! Standalone drone agent process (M92).
//!
//! Usage: `drone_agent --config <path> [--dry-run]`
//!
//! In `--dry-run` mode the agent runs through `max_ticks` ticks, sending
//! heartbeats over the configured transport, but never issues commands to the
//! flight controller.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use swarm_comms::{
    DroneLinkConfig, InternetLikeMock, NullDroneLink, RawMessage, SerialDroneLink, Transport,
    UdpDroneLink,
};
use swarm_types::AgentId;

// ─── Config types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct GcsLostPolicy {
    kind: String,
    #[serde(default)]
    after_ticks: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AutonomyConfig {
    #[serde(default)]
    gcs_lost_policy: Option<GcsLostPolicy>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DroneAgentConfig {
    agent_id: AgentId,
    #[serde(default)]
    drone_link: DroneLinkConfig,
    #[serde(default)]
    autonomy: AutonomyConfig,
    #[serde(default = "default_max_ticks")]
    max_ticks: u64,
}

fn default_max_ticks() -> u64 {
    200
}

// ─── Run report ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct RunReport {
    agent_id: AgentId,
    drone_link_kind: String,
    ticks_run: u64,
    dry_run: bool,
    elapsed_ms: u128,
}

// ─── Agent loop ───────────────────────────────────────────────────────────────

fn drone_link_kind(config: &DroneLinkConfig) -> &'static str {
    match config {
        DroneLinkConfig::Simulated => "simulated",
        DroneLinkConfig::Udp { .. } => "udp",
        DroneLinkConfig::InternetLikeMock { .. } => "internet_like_mock",
        DroneLinkConfig::Serial { .. } => "serial",
    }
}

fn run_loop<T: Transport>(mut transport: T, agent_id: &AgentId, max_ticks: u64, dry_run: bool)
where
    T::Error: std::fmt::Debug,
{
    let heartbeat_payload =
        format!(r#"{{"kind":"heartbeat","tick":0,"agent":"{agent_id}"}}"#).into_bytes();

    for tick in 0..max_ticks {
        // Poll incoming messages
        match transport.poll() {
            Ok(Some(_msg)) => {
                // In a real agent: dispatch to protocol handler
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("[{agent_id}] tick={tick} poll error: {e:?}");
            }
        }

        if !dry_run {
            // Real mode: would issue FC commands here
        }

        // Send heartbeat back to own agent (self-addressed for dry-run) or
        // rely on the transport to route it; the NullDroneLink will discard it.
        let hb = RawMessage {
            from: agent_id.clone(),
            to: agent_id.clone(),
            payload: heartbeat_payload.clone(),
        };
        if let Err(e) = transport.send(hb) {
            eprintln!("[{agent_id}] tick={tick} send error: {e:?}");
        }
    }
}

/// Core agent entry-point, callable from tests.
fn run_agent(config: DroneAgentConfig, dry_run: bool) -> Result<RunReport, String> {
    let kind = drone_link_kind(&config.drone_link).to_owned();
    let start = Instant::now();

    match config.drone_link.clone() {
        DroneLinkConfig::Simulated => {
            // In a standalone process, "simulated" has no shared bus — use null.
            let transport = NullDroneLink {
                own_id: config.agent_id.clone(),
            };
            run_loop(transport, &config.agent_id, config.max_ticks, dry_run);
        }

        DroneLinkConfig::Udp { bind_addr, peers } => {
            let bind: SocketAddr = bind_addr
                .parse()
                .map_err(|e| format!("invalid bind_addr: {e}"))?;
            let peer_map: HashMap<AgentId, SocketAddr> = peers
                .into_iter()
                .map(|(id, addr)| {
                    addr.parse::<SocketAddr>()
                        .map(|sa| (id, sa))
                        .map_err(|e| format!("invalid peer addr: {e}"))
                })
                .collect::<Result<_, _>>()?;
            let transport = UdpDroneLink::bind(config.agent_id.clone(), bind, peer_map)
                .map_err(|e| format!("UDP bind error: {e}"))?;
            run_loop(transport, &config.agent_id, config.max_ticks, dry_run);
        }

        DroneLinkConfig::InternetLikeMock { profile, seed } => {
            use swarm_comms::InternetLikeMockProfile;
            let mut transport = match profile {
                InternetLikeMockProfile::Lte => InternetLikeMock::with_lte_profile(seed),
                InternetLikeMockProfile::Satcom => InternetLikeMock::with_satcom_profile(seed),
            };
            for tick in 0..config.max_ticks {
                transport.advance_tick();
                match transport.poll() {
                    Ok(Some(_)) => {}
                    Ok(None) => {}
                    Err(e) => match e {},
                }
                let hb = RawMessage {
                    from: config.agent_id.clone(),
                    to: config.agent_id.clone(),
                    payload: format!("hb-{tick}").into_bytes(),
                };
                let _ = transport.send(hb);
            }
        }

        DroneLinkConfig::Serial { .. } => {
            let transport = SerialDroneLink {
                own_id: config.agent_id.clone(),
            };
            // Serial is a placeholder; ignore NotImplemented errors
            run_loop(transport, &config.agent_id, config.max_ticks, dry_run);
        }
    }

    Ok(RunReport {
        agent_id: config.agent_id,
        drone_link_kind: kind,
        ticks_run: config.max_ticks,
        dry_run,
        elapsed_ms: start.elapsed().as_millis(),
    })
}

// ─── CLI ──────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut config_path: Option<String> = None;
    let mut dry_run = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = Some(args[i].clone());
            }
            "--dry-run" => {
                dry_run = true;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let path = match config_path {
        Some(p) => p,
        None => {
            eprintln!("usage: drone_agent --config <path> [--dry-run]");
            std::process::exit(1);
        }
    };

    let json = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read config {path}: {e}");
            std::process::exit(1);
        }
    };

    let config: DroneAgentConfig = match serde_json::from_str(&json) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("invalid config: {e}");
            std::process::exit(1);
        }
    };

    match run_agent(config, dry_run) {
        Ok(report) => {
            let json = serde_json::to_string_pretty(&report).unwrap_or_default();
            println!("{json}");
        }
        Err(e) => {
            eprintln!("agent error: {e}");
            std::process::exit(1);
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_config(max_ticks: u64, drone_link: DroneLinkConfig) -> DroneAgentConfig {
        DroneAgentConfig {
            agent_id: AgentId::from("agent-test".to_owned()),
            drone_link,
            autonomy: AutonomyConfig::default(),
            max_ticks,
        }
    }

    #[test]
    fn drone_agent_dry_run_exits_zero() {
        // Verify that the agent loop completes without error using the null transport.
        let config = minimal_config(5, DroneLinkConfig::Simulated);
        let report = run_agent(config, true).expect("run_agent should succeed");
        assert_eq!(report.ticks_run, 5);
        assert!(report.dry_run);
        assert_eq!(report.drone_link_kind, "simulated");
    }

    #[test]
    fn drone_agent_dry_run_with_internet_like_mock() {
        let config = minimal_config(
            10,
            DroneLinkConfig::InternetLikeMock {
                profile: swarm_comms::InternetLikeMockProfile::Lte,
                seed: 42,
            },
        );
        run_agent(config, true).expect("should complete without error");
    }
}
