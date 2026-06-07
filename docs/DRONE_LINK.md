# Drone Link Transport — M92

Location: `swarm-comms/src/drone_link.rs`  
Config type: `DroneLinkConfig` (in `swarm-comms`)

## Overview

M92 introduces four pluggable `Transport` implementations that let the same
agent code run in simulation, in multi-process integration tests, and on real
hardware by changing only the config — not the code.

```text
Before M92:  1 process = N agents sharing in-memory bus
After  M92:  1 process = 1 agent; N processes = N drones
```

The `drone_agent` binary (`swarm-examples`) is the companion-computer entry
point. A real drone runs the same binary as a CI integration test; only the
`drone_link` config differs.

---

## Transport implementations

### 1. `InMemNetwork` / `InMemAgentTransport` — simulated (default)

Used by `ScenarioRunner` for all existing simulation scenarios.  
Selected by `DroneLinkConfig::Simulated` (the default).

**When to use:** unit tests, scenario development, any case where all agents
run in the same process.

### 2. `UdpDroneLink` — real inter-process communication

```rust
UdpDroneLink::bind(own_id, bind_addr, peers)
```

- Non-blocking UDP unicast.
- Each agent binds one socket; peer addresses are registered at construction.
- `send()` returns `UnknownPeer` if the recipient is absent from the peer
  table, `PayloadTooLarge` if `msg.payload` exceeds 65 507 bytes.
- `poll()` returns `None` immediately when no datagram is ready.

**When to use:** multi-process integration tests on localhost; companion
computer ↔ GCS over LAN; any scenario where agents run as separate OS
processes.

### 3. `InternetLikeMock` — high-latency / lossy channel simulation

```rust
InternetLikeMock::with_lte_profile(seed)    // ~1-tick latency, 3 % loss
InternetLikeMock::with_satcom_profile(seed) // ~6-tick latency, 8 % loss
```

- All messages pass through an internal pending queue with configurable base
  latency, jitter, packet-loss rate, burst-drop probability, and reorder
  probability.
- **No actual network is used** — suitable for deterministic CI tests.
- Call `advance_tick()` once per simulation tick to move the clock forward.

**When to use:** testing M91 protocol behaviour under high-latency cellular or
satellite links without real hardware; fuzzing for race conditions.

### 4. `SerialDroneLink` — serial port placeholder

All operations return `SerialDroneLinkError::NotImplemented`.

**When to use:** compile-time verification that serial-backed agents can be
written without a serial dependency; target for a future `serialport`
implementation.

### 5. `NullDroneLink` — no-op transport for unit tests

Drops all outgoing messages; `poll()` always returns `None`.

**When to use:** unit-testing agent logic that does not depend on message
delivery; the `drone_agent --dry-run` path when `DroneLinkConfig::Simulated`
is specified for a standalone process.

---

## `DroneLinkConfig` — selecting the transport

```rust
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DroneLinkConfig {
    Simulated,                               // default
    Udp { bind_addr, peers },
    InternetLikeMock { profile, seed },
    Serial { path, baud },
}
```

Added to `RunConfig` with `#[serde(default)]` so existing scenario JSON files
that omit the field continue to load and default to `Simulated`.

### Example: multi-process UDP scenario

`agent-0.json`:
```json
{
  "agent_id": "agent-0",
  "drone_link": {
    "kind": "udp",
    "bind_addr": "127.0.0.1:7001",
    "peers": { "agent-1": "127.0.0.1:7002" }
  },
  "max_ticks": 200
}
```

`agent-1.json`:
```json
{
  "agent_id": "agent-1",
  "drone_link": {
    "kind": "udp",
    "bind_addr": "127.0.0.1:7002",
    "peers": { "agent-0": "127.0.0.1:7001" }
  },
  "max_ticks": 200
}
```

Run in two terminals:
```bash
drone_agent --config agent-0.json --dry-run
drone_agent --config agent-1.json --dry-run
```

### Example: LTE simulation

```json
{
  "agent_id": "agent-0",
  "drone_link": { "kind": "internet_like_mock", "profile": "lte", "seed": 42 },
  "max_ticks": 500
}
```

---

## Adding a new transport

1. Implement `Transport` for your struct in `swarm-comms/src/drone_link.rs`.
2. Define a typed error enum with `thiserror`.
3. Add a variant to `DroneLinkConfig`.
4. Handle the new variant in `drone_agent.rs` `run_agent()`.
5. Add a serde roundtrip test to `drone_link_config_serde_roundtrip_all_variants`.
6. Update this document.

---

## Lease model interaction

`UdpDroneLink` and `InternetLikeMock` carry `SwarmMessageEnvelope` payloads
(from M91) transparently. The `DuplicateSuppressor` in `swarm-comms` should
be instantiated per-agent to handle retransmissions that arrive over lossy
links.

---

## Correlation with the run report

`drone_agent` writes a JSON run report to stdout on exit. The
`drone_link_kind` field records which transport was active:

```json
{
  "agent_id": "agent-0",
  "drone_link_kind": "udp",
  "ticks_run": 200,
  "dry_run": true,
  "elapsed_ms": 12
}
```
