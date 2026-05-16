# Swarm Coordination Runtime

Swarm Coordination Runtime is a Rust workspace for mission-level coordination of autonomous drone fleets. The current code focuses on deterministic simulation, task ownership, heartbeat-based membership, failure detection, and measurable recovery behaviour rather than low-level flight control.

## Current Status

**Milestone 1** — complete. Foundational coordination: heartbeat-based membership, timeout failure detection, task registry state machine, greedy reallocation, deterministic scenario runner, metrics.

**Milestone 2** — complete. Realistic task allocation:

- Dynamic task injection at configurable ticks during a mission.
- Task expiration: Unassigned and Assigned tasks are removed when their deadline passes. InProgress tasks are never expired.
- Agent capability matching as a hard constraint: an agent that lacks a required capability is excluded from allocation.
- Auction-based allocation (`AuctionAllocator`) with a cost function over Euclidean distance, battery level, and role preference.
- Pluggable `Allocator` trait: `GreedyAllocator` and `AuctionAllocator` are both usable as drop-in strategies.
- Ownership conflict detection: duplicate allocation decisions in one round are counted in metrics.
- Extended metrics: `tasks_injected`, `tasks_expired`, `conflicting_assignments`.
- Side-by-side comparison of Greedy vs Auction over 1 000 deterministic seeds.

## Workspace Layout

| Crate | Purpose |
| --- | --- |
| `swarm-types` | Shared IDs, agent/task/message types, pose and velocity. |
| `swarm-comms` | Transport trait and in-memory network with latency and packet loss. |
| `swarm-runtime` | Membership, failure detection, task registry, coordinator. |
| `swarm-alloc` | Greedy and auction allocation strategies. |
| `swarm-sim` | Deterministic clock, scenario model, generic scenario runner. |
| `swarm-scenarios` | Scenario builders: Coverage With Failure and Dynamic Auction. |
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

Run the baseline empty smoke example:

```bash
cargo run -p swarm-examples --bin empty_scenario
```

Run the Milestone 1 Coverage With Failure scenario (1 000 seeds):

```bash
cargo run -p swarm-examples --bin coverage_with_failure
```

Run the Milestone 2 Dynamic Auction comparison (1 000 seeds × 2 strategies):

```bash
cargo run -p swarm-examples --bin dynamic_auction
```

## Observe Output

`empty_scenario` advances a deterministic clock for 10 ticks and prints elapsed simulated time.

`coverage_with_failure` runs 1 000 deterministic seeds with 10 agents and 15 tasks, crashes `agent-0`, detects the failure, reallocates tasks, and reports aggregate metrics. Exits with code `1` if `success_rate < 0.99`.

`dynamic_auction` runs 1 000 seeds for both Greedy and Auction strategies using the Dynamic Auction scenario: 10 agents with heterogeneous capabilities and poses, 8 initial tasks with capability requirements, 10 tasks injected dynamically (each with a 15-tick expiry window), and 1 agent failure. Outputs side-by-side aggregate metrics:

```
=== greedy ===
runs: 1000
success_rate: 1.000
avg_detection_ticks: ...
avg_reallocation_ticks: ...
avg_messages_attempted: ...
avg_messages_dropped: ...
avg_tasks_injected: 10.000
avg_tasks_expired: ...
avg_conflicting_assignments: 0.000
=== auction ===
...
```

Exits with code `1` if either strategy has `success_rate < 0.95`.
