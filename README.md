# Swarm Coordination Runtime

Swarm Coordination Runtime is a Rust workspace for mission-level coordination of autonomous drone fleets. The current code focuses on deterministic simulation, task ownership, heartbeat-based membership, failure detection, and measurable recovery behavior rather than low-level flight control.

## Current Status

Milestone 0 is complete and Milestone 1 introduces the first runnable coordination scenario: `Coverage With Failure`.

The project currently includes:

- foundational swarm types (`AgentId`, `TaskId`, `MessageId`, `Agent`, `Task`, `Pose`, `Velocity`);
- a pluggable transport trait and in-memory simulated network;
- membership, timeout-based failure detection, and task ownership state;
- greedy task reallocation;
- deterministic scenario execution;
- metrics aggregation;
- runnable examples.

## Workspace Layout

| Crate | Purpose |
| --- | --- |
| `swarm-types` | Shared IDs, agent/task/message types, pose and velocity. |
| `swarm-comms` | Transport trait and in-memory network with latency and packet loss. |
| `swarm-runtime` | Membership, failure detection, task registry, coordinator. |
| `swarm-alloc` | Greedy allocation strategy for unassigned tasks. |
| `swarm-sim` | Deterministic clock, scenario model, scenario runner. |
| `swarm-scenarios` | Scenario builders such as Coverage With Failure. |
| `swarm-metrics` | Per-run and aggregate metrics. |
| `swarm-replay` | Placeholder for future replay support. |
| `swarm-examples` | Runnable binaries. |

## Build

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

## Run Examples

Run the empty Milestone 0 smoke example:

```bash
cargo run -p swarm-examples --bin empty_scenario
```

Run the Milestone 1 Coverage With Failure scenario:

```bash
cargo run -p swarm-examples --bin coverage_with_failure
```

## Observe Output

`empty_scenario` advances a deterministic clock for 10 ticks and prints elapsed simulated time.

`coverage_with_failure` runs 1000 deterministic seeds. Each run starts with 10 agents and 15 coverage tasks, crashes `agent-0`, detects the missed heartbeats through the failure detector, releases the failed agent's tasks, reallocates them, and reports aggregate metrics:

- `success_rate`;
- average failure detection ticks;
- average reallocation ticks;
- attempted messages;
- dropped messages.

A successful run exits with code `0`. If the aggregate success rate drops below `0.99`, the example exits with code `1`.
