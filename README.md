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

**Milestone 3** — complete. Pluggable transport, multiprocess runtime:

- `AgentNode<T: Transport>` — unified runtime contour usable in both in-process and multi-process modes.
- Pluggable `Transport` trait with two implementations: `InMemAgentTransport` (shared bus for simulation) and `UdpTransport` (UDP loopback for OS-level multiprocess).
- `ScenarioRunner` refactored to use `AgentNode<InMemAgentTransport>` — same runtime, same allocator, same coordinator.
- Multiprocess scenario: 5 OS-processes communicating via UDP loopback; `kill -9` one agent, rest detect failure and reallocate tasks.
- Basic observability via `tracing` spans in `swarm-runtime` and `swarm-alloc`. Configure with `RUST_LOG`.

**Milestone 4** — complete. Partial connectivity, gossip-based convergence:

- `RuntimeMessage` typed protocol: heartbeat (with sender_tick and generation) + gossip/anti-entropy (full task→agent + agent→generation maps).
- Network partitions via `InMemNetwork::add_partition`/`remove_partition` — configurable agent-pair blocks.
- Stale heartbeat protection: `generation` (epoch) per agent; `record_heartbeat` ignores lower generation and old sender tick.
- Gossip/anti-entropy sync: agents periodically exchange assignment maps + generation maps; deterministic merge via `(generation, AgentId)` total order guarantees convergence after partition heals.
- Duplicate/delayed/reordered message handling: heartbeat is idempotent, gossip is commutative (applies in any order).
- New metrics: `partition_events`, `partitions_active`, `stale_messages_discarded`, `convergence_ticks`, `max_view_divergence`.
- Partition scenario: 6 agents, tick 10 partition into two groups, tick 30 heal, gossip converges after heal.

**Milestone 5** — complete. Emergency Mesh Network:

- `comms_range` on `Agent`: range-based connectivity using distance and BFS reachability.
- `GroundNode` type: passive mesh participants that do not receive tasks.
- `required_role` on `Task`: hard constraint for role-specific tasks (e.g., relay tasks).
- `ConnectivityModel` in `swarm-comms`: manual BFS/DFS on adjacency list for mesh reachability, hop count, and network availability fraction.
- `ConnectivityAwareAllocator` in `swarm-alloc`: optimizes relay placement by simulating each candidate's effect on network availability.
- `Allocator` trait extended with `allocate_with_connectivity` (default impl delegates to `allocate` for backward compatibility).
- Pose update in `ScenarioRunner`: agents move to their assigned task's pose, changing the connectivity graph dynamically.
- Network availability metrics: `network_availability`, `avg_hop_count`, `disconnected_agents_max`, `relay_reallocation_ticks`.
- Emergency Mesh scenario: base station, scouts, relays, ground nodes; relay failure and reallocation; 1000 seeds with availability threshold >= 0.8.

## Workspace Layout

| Crate | Purpose |
| --- | --- |
| `swarm-types` | Shared IDs, agent/task/message types, pose and velocity. |
| `swarm-comms` | Transport trait, in-memory network, UDP transport. |
| `swarm-runtime` | Membership, failure detection, task registry, coordinator, `AgentNode`. |
| `swarm-alloc` | Greedy and auction allocation strategies. |
| `swarm-sim` | Deterministic clock, scenario model, generic scenario runner. |
| `swarm-scenarios` | Scenario builders: Coverage With Failure, Dynamic Auction, and Emergency Mesh. |
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

Run the Milestone 3 multiprocess scenario (5 agents over UDP loopback, crash test):

```bash
cargo run -p swarm-examples --bin multiprocess_scenario
```

Run the Milestone 4 partition + convergence scenario (6 agents, in-process):

```bash
cargo run -p swarm-examples --bin partition_scenario
```

Run the Milestone 5 Emergency Mesh scenario (1000 seeds, connectivity-aware allocation, relay reallocation):

```bash
cargo run -p swarm-examples --bin emergency_mesh_scenario
```

Enable tracing for observability:

```bash
RUST_LOG=info cargo run -p swarm-examples --bin multiprocess_scenario
RUST_LOG=debug cargo run -p swarm-examples --bin agent_process -- --config /tmp/swarm-v03/config-0.json
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

`multiprocess_scenario` launches 5 OS-level `agent_process` instances on dynamic loopback UDP ports, kills `agent-0` after 2s, waits 3s for failure detection, then reads per-agent JSON metrics from `/tmp/swarm-v03/`. Verifies:
- All survivors detected `agent-0` as failed.
- `global_assignment_map` is identical across all survivors (convergence).
- No task remains assigned to `agent-0`.
- All 8 tasks are assigned.

Prints `PASS` on success, exits code `0`; reports violations with `FAIL` and exits code `1`.

`partition_scenario` runs a deterministic in-process partition scenario: 6 agents, 8 tasks, full connectivity until tick 10, then partition (agent-0,1,2 isolated from agent-3,4,5), heal at tick 30, gossip interval every 3 ticks. Verifies:
- Partition was active (`partitions_active: true`).
- Views diverge during partition (`max_view_divergence > 0`).
- Maps converge after heal (`convergence_ticks` is set).
- All tasks assigned (`success: true`).

Prints metrics and exits code `0` on success; panics on invariant violation.

`emergency_mesh_scenario` runs 1000 seeds with 4 scouts, 2 relays, 2 ground nodes, and a base station in a 20×0 area with `comms_range = 15.0`. One relay fails at tick 15. The `ConnectivityAwareAllocator` assigns relay tasks to optimize mesh reachability. After reallocation, the new relay agent moves to the relay task pose, restoring connectivity. Verifies:
- `network_availability >= 0.8` across all seeds.
- `relay_reallocation_ticks` is set (relay tasks reassigned after failure).
- All scout tasks assigned to capable agents.

Prints aggregate metrics and exits code `0` on success, `1` on invariant violation.

**Milestone 6** — complete. Strategy Comparison Platform:

- `Strategy` trait that wraps any `Allocator` and provides metadata (name, description).
- `StrategyRegistry` that holds all registered strategies for benchmark harnesses.
- `CentralizedPlanner` — oracle baseline with full global knowledge, greedy bipartite matching.
- New metrics: `coverage_progress`, `bytes_sent`, `stale_state_age_ticks`, `battery_margin_min`, `battery_margin_avg`.
- `NetworkProfile` and `FailureProfile` with `StandardProfiles` (Ideal, LightLoss, MediumLoss, HeavyLoss, HighLatency, PartitionProne × NoFailures, SingleFailure, MultipleFailures, CascadeFailure).
- `BenchmarkHarness` that runs strategies across seeds and profiles, producing `ComparisonReport` with markdown-compatible table output.
- `strategy_comparison` binary that runs all strategies against a reduced profile matrix and verifies invariants (e.g., centralized >= greedy on ideal network).

Run the Milestone 6 strategy comparison (quick benchmark, 10 seeds × 4 profiles × 3 strategies):

```bash
cargo run -p swarm-examples --bin strategy_comparison
```

Sample output:

```
| Стратегия | Профиль | Успех | Обнаружение | Перераспределение | Покрытие | Сообщения | Доступность |
|-----------|---------|-------|-------------|-------------------|----------|-----------|-------------|
| greedy    | ideal-no-failures | 1.000 |       0.000 |             0.000 |    1.000 |    90.000 |       1.000 |
| auction   | ideal-no-failures | 1.000 |       0.000 |             0.000 |    1.000 |    90.000 |       1.000 |
| connectivity-aware | ideal-no-failures | 1.000 |       0.000 |             0.000 |    1.000 |    90.000 |       1.000 |
```
