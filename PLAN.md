# Plan: Milestone 6 — Strategy Comparison Platform

## Context

After Milestone 5 (Emergency Mesh), the workspace has:

- `GreedyAllocator` — round-robin over capable agents.
- `AuctionAllocator` — cost-minimization over distance, battery, and role preference.
- `ConnectivityAwareAllocator` — auction + network-availability optimization for relay placement.
- 4 scenario types: `Coverage`, `DynamicAuction`, `Partition`, `EmergencyMesh`.
- Per-run metrics (`RunMetrics`) and aggregation (`AggregateMetrics`).
- Deterministic in-process simulation via `ScenarioRunner::run_with(scenario, config, allocator)`.

Milestone 6 turns the project into a research platform by making strategy comparison a first-class feature. Instead of ad-hoc binaries that compare 2 strategies on 1 scenario, the platform will run 1000 scenarios × multiple strategies × multiple network/failure profiles and produce structured comparison reports.

The long-term goal (per DRONE_A.1.md / DRONE_B.1.md) is to compare:

1. Centralized planner (optimal baseline with full global knowledge).
2. Greedy decentralized (existing).
3. Auction-based (existing).
4. Relay-aware / connectivity-aware (existing).
5. CBBA (Consensus-Based Bundle Algorithm) — later milestone.

## Investigation Context

DRONE_A.1.md and DRONE_B.1.md establish that the project should become a research platform comparing coordination strategies across reference missions. The current codebase already has the primitives (allocators, scenarios, metrics, deterministic runner). Milestone 6 builds the *comparison harness* and *additional metrics* needed for rigorous evaluation.

## Affected Components

| Crate | Changes |
|-------|---------|
| `swarm-alloc` | `Strategy` trait/wrapper; `CentralizedPlanner`; `StrategyRegistry`. |
| `swarm-metrics` | New fields: `coverage_progress`, `bytes_sent`, `stale_state_age`, `battery_margin_min`, `battery_margin_avg`. |
| `swarm-sim` | `BenchmarkHarness`, `NetworkProfile`, `FailureProfile`, comparison report generation. |
| `swarm-scenarios` | Reusable profile builders: `StandardProfiles` (low/medium/high packet loss, partition rates). |
| `swarm-examples` | `strategy_comparison.rs` — the main benchmark binary. |
| `README.md` | Milestone 6 section with usage and sample report output. |

## Implementation Steps

### Step 1: Strategy abstraction (`swarm-alloc`)

**Files:**
- `crates/swarm-alloc/src/strategy.rs` (new)
- `crates/swarm-alloc/src/lib.rs`

Create a `Strategy` trait that wraps any `Allocator` and provides metadata:

```rust
pub trait Strategy: Allocator {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
}
```

Implement `Strategy` for:
- `GreedyAllocator`
- `AuctionAllocator`
- `ConnectivityAwareAllocator`

Add a `StrategyRegistry` that holds a `Vec<Box<dyn Strategy>>` and can iterate over registered strategies.

### Step 2: Centralized planner baseline (`swarm-alloc`)

**Files:**
- `crates/swarm-alloc/src/centralized.rs` (new)
- `crates/swarm-alloc/src/lib.rs`

Implement `CentralizedPlanner` — an allocator that has *oracle* access to the full scenario state (all agent poses, all tasks, no communication constraints). It solves a bipartite matching problem using the Hungarian algorithm (or greedy if `petgraph` is unavailable) to minimize total cost (distance + battery penalty + role preference).

This is an **upper bound baseline**: no decentralized strategy should beat it in ideal conditions. It answers the question "how much do we lose by being decentralized?"

The `CentralizedPlanner` implements `Allocator` but requires a `Scenario` reference at construction time. For the benchmark harness, it will be constructed per-run with the current scenario.

### Step 3: New metrics (`swarm-metrics`)

**Files:**
- `crates/swarm-metrics/src/metrics.rs`

Extend `RunMetrics` and `AggregateMetrics` with:

```rust
// Coverage: fraction of area covered by assigned agents (0.0..=1.0)
pub coverage_progress: f64,
// Bytes sent: total payload bytes across all messages
pub bytes_sent: u64,
// Stale state age: max difference between local tick and last seen remote tick
pub stale_state_age_ticks: u64,
// Battery margins (if battery model enabled)
pub battery_margin_min: f64,
pub battery_margin_avg: f64,
```

Update `AggregateMetrics::from_runs` to compute averages for the new fields.

Update `Display for AggregateMetrics` to include new fields.

### Step 4: Network and failure profiles (`swarm-scenarios`)

**Files:**
- `crates/swarm-scenarios/src/profiles.rs` (new)
- `crates/swarm-scenarios/src/lib.rs`

Define reusable `NetworkProfile` and `FailureProfile` structs:

```rust
pub struct NetworkProfile {
    pub name: &'static str,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub latency_per_hop: u64,
}

pub struct FailureProfile {
    pub name: &'static str,
    pub failure_count: usize,
    pub failure_tick_range: (u64, u64),
}
```

Provide a `StandardProfiles` module with pre-defined profiles:

- Networks: `Ideal`, `LightLoss`, `MediumLoss`, `HeavyLoss`, `HighLatency`, `PartitionProne`
- Failures: `NoFailures`, `SingleFailure`, `MultipleFailures`, `CascadeFailure`

### Step 5: Benchmark harness (`swarm-sim`)

**Files:**
- `crates/swarm-sim/src/benchmark.rs` (new)
- `crates/swarm-sim/src/lib.rs`

Create `BenchmarkHarness` that runs:

```
for each seed in 0..1000:
  for each strategy in strategies:
    for each network_profile in network_profiles:
      for each failure_profile in failure_profiles:
        build scenario with seed + profile params
        run ScenarioRunner::run_with(...)
        collect RunMetrics
```

The harness produces a `ComparisonReport`:

```rust
pub struct ComparisonReport {
    pub strategy_names: Vec<String>,
    pub profile_names: Vec<String>,
    pub results: HashMap<(String, String), AggregateMetrics>,
}
```

Implement `Display for ComparisonReport` that prints a markdown-compatible table:

```
| Strategy | Profile | Success | Detection | Realloc | Coverage | Messages | Availability |
|----------|---------|---------|-----------|---------|----------|----------|--------------|
```

### Step 6: Strategy comparison binary (`swarm-examples`)

**Files:**
- `crates/swarm-examples/src/bin/strategy_comparison.rs` (new)

The main benchmark binary:

1. Register all strategies: `Greedy`, `Auction`, `ConnectivityAware`, `Centralized`.
2. Register all profiles from `StandardProfiles`.
3. Run the benchmark harness.
4. Print the comparison report.
5. Assert invariants (e.g., `Centralized` success rate >= `Greedy` success rate under ideal network).
6. Exit code `0` on success, `1` on invariant violation.

### Step 7: Update existing scenarios for new metrics

**Files:**
- `crates/swarm-sim/src/runner.rs`

Update `ScenarioRunner::run_with` to compute and populate:
- `coverage_progress` — geometric coverage of assigned task poses vs total area.
- `bytes_sent` — sum of `msg.payload.len()` across all messages.
- `stale_state_age_ticks` — max `current_tick - entry.last_heartbeat_tick` across all alive agents.
- `battery_margin_min` / `battery_margin_avg` — min/avg battery of alive agents at end of run.

### Step 8: README update

**Files:**
- `README.md`

Add Milestone 6 section describing:
- The strategy comparison platform.
- How to run `strategy_comparison`.
- Sample report output.
- Interpretation of metrics (what success rate, coverage, and availability mean).

## Testing Strategy

### Category 1: Pure unit tests (no refactoring needed)

- `strategy.rs`: `StrategyRegistry` add/iteration.
- `centralized.rs`: `CentralizedPlanner` assigns all tasks when enough agents exist; returns empty when no agents.
- `profiles.rs`: `StandardProfiles` contains expected profile names and parameter ranges.
- `benchmark.rs`: `ComparisonReport` aggregation and display formatting.

### Category 2: Light integration tests (in-process simulation)

- `strategy_comparison.rs` binary test: run a small benchmark (10 seeds × 2 strategies × 2 profiles) and verify report structure and invariant assertions.
- Verify that `CentralizedPlanner` outperforms or matches `GreedyAllocator` on ideal network (deterministic, no packet loss, no partitions).
- Verify that `ConnectivityAwareAllocator` achieves higher `network_availability` than `AuctionAllocator` on `PartitionProne` profile in Emergency Mesh scenario.

### Category 3: Heavy end-to-end tests (full benchmark)

- Run `strategy_comparison` with 1000 seeds, all 4 strategies, all standard profiles.
- Verify total runtime is reasonable (< 5 minutes on current hardware).
- Verify report contains all expected rows and no NaN values.
- Manual review of report for anomalies (e.g., negative success rate, impossible coverage > 1.0).

## Risks and Tradeoffs

| Risk | Impact | Mitigation |
|------|--------|------------|
| CentralizedPlanner requires scenario-level knowledge, breaking the `Allocator` abstraction. | Medium | Implement as a special-case `Allocator` that pre-computes optimal assignments from scenario data at construction time. Document that it is a benchmark-only strategy. |
| Coverage progress metric is expensive to compute geometrically. | Low | Use a coarse grid approximation (e.g., 20×20 grid) or task-based proxy (fraction of tasks with assigned agents within sensor range). |
| Benchmark runtime grows as O(seeds × strategies × networks × failures). | Medium | Default to a reduced matrix (e.g., 100 seeds) in CI/tests; full 1000 seeds only in release/benchmark mode. |
| Adding new metrics breaks backward compatibility of saved `RunMetrics` JSON. | Low | Add `#[serde(default)]` on new fields. Existing replay data without new fields deserializes safely. |
| CBBA deferred to later milestone creates gap in strategy comparison. | Low | Document CBBA as "planned for v0.7". The comparison framework supports adding new strategies without code changes. |

## Open Questions

1. **Coverage metric precision**: Should coverage be task-based (assigned agents near task poses) or area-based (grid discretization)? Task-based is cheaper but less meaningful for Search and Rescue scenarios.
2. **Battery model depth**: v0.3+ has static battery. Should Milestone 6 add a simple drain model (`battery -= distance * energy_per_meter`), or keep battery static and use `battery_margin` as a proxy for assignment quality?
3. **CBBA scope**: Should CBBA be a quick win in Milestone 6 (simplified bundle-building without full consensus), or deferred to a dedicated milestone with proper message protocol extensions?
4. **Report format**: Markdown table is human-readable but hard to parse programmatically. Should we also emit JSON/CSV for downstream analysis (Python/Polars)?
5. **Statistical significance**: The current aggregate metrics use simple means. Should we add confidence intervals or use Mann-Whitney U-test for strategy comparison? This could be deferred to a later research-phase milestone.

## Что могло сломаться

- **Behavior**: Existing scenario binaries (`coverage_with_failure`, `dynamic_auction`, etc.) should not change behavior. The new metrics fields have serde defaults, so existing code paths are unaffected.
- **API/Contracts**: `Allocator` trait is unchanged. `Strategy` is a new super-trait. Existing allocators continue to work.
- **Performance**: `ScenarioRunner` gains additional metric computation per tick. Coverage grid calculation may add ~5-10% overhead. Mitigation: only compute coverage when `coverage_progress` field is used (feature flag or config flag).
- **Determinism**: New metrics (bytes_sent, stale_state_age) must be computed deterministically from the same RNG seed and tick sequence. No new randomness should be introduced.
- **Integration**: `CentralizedPlanner` is benchmark-only and should not be used in multi-process or real-flight scenarios. Document this clearly.

## Verification Commands (for implement phase)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p swarm-examples --bin strategy_comparison
```
