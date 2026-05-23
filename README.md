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
| `swarm-comms` | Transport trait, in-memory network, UDP transport, MAVLink transport (optional). |
| `swarm-runtime` | Membership, failure detection, task registry, coordinator, `AgentNode`. |
| `swarm-alloc` | Greedy and auction allocation strategies. |
| `swarm-sim` | Deterministic clock, scenario model, generic scenario runner, DSL loader, JSON/CSV export. |
| `swarm-scenarios` | Scenario builders: Coverage With Failure, Dynamic Auction, Emergency Mesh, SAR, and Infrastructure Inspection. |
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
- `strategy_comparison` binary that runs all 4 strategies against StandardProfiles combinations and verifies invariants (e.g., centralized >= greedy on ideal network).

Run the quick benchmark (10 seeds × 4 key profiles × 4 strategies):

```bash
cargo run -p swarm-examples --bin strategy_comparison
```

Run the full benchmark (1000 seeds × all 24 profile combinations × 4 strategies):

```bash
cargo run -p swarm-examples --bin strategy_comparison -- --full
```

Sample output (quick mode):

```
| Стратегия | Профиль | Успех | Завершение | Обнаружение | Перераспределение | Покрытие | Сообщения | Байты | Конфликты | Stale | Батарея мин | Батарея ср | Доступность |
|-----------|---------|-------|------------|-------------|-------------------|----------|-----------|-------|-----------|-------|-------------|------------|-------------|
| greedy    | ideal-no-failures | 1.000 |      1.000 |       0.000 |             0.000 |    1.000 |    90.000 |  3960 |     0.000 |     0 |     100.000 |    100.000 |       1.000 |
| auction   | ideal-no-failures | 1.000 |      1.000 |       0.000 |             0.000 |    1.000 |    90.000 |  3960 |     0.000 |     0 |     100.000 |    100.000 |       1.000 |
| connectivity-aware | ideal-no-failures | 1.000 |      1.000 |       0.000 |             0.000 |    1.000 |    90.000 |  3960 |     0.000 |     0 |     100.000 |    100.000 |       1.000 |
| centralized | ideal-no-failures | 1.000 |      1.000 |       0.000 |             0.000 |    1.000 |    90.000 |  3960 |     0.000 |     0 |     100.000 |    100.000 |       1.000 |
```

**Milestone 7** — complete. Experiment Infrastructure:

- `swarm-replay` crate: EventLog with TickStart, AgentFailed, TaskAssigned, MessageSent, MessageDropped, PartitionAdded/Removed, PoseUpdated events; deterministic replay engine; JSON serialization.
- `ScenarioRunner::run_with_log`: new function that returns `(RunMetrics, Option<EventLog>)` alongside the existing `run_with`.
- `ComparisonReport` with `benchmark_run_id` and per-row `run_id`.
- JSON/CSV export via `swarm_sim::export_json` and `swarm_sim::export_csv`.
- CLI flags for `strategy_comparison`: `--json <path>`, `--csv <path>`, `--replay-log <dir>`, `--run-id-prefix <prefix>`.
- Property-based tests with `proptest`: randomized Agent/Task generation, runner no-panic invariant, success-rate boundedness.

**Milestone 8** — complete. Kinematic + Battery Foundation:

- Kinematic model: `Agent` gains `speed` (m/s), `max_range` (max travel distance), `battery_drain_rate` (%/m). Movement: `position += direction * speed * dt` per tick toward assigned task's pose.
- Battery drain: proportional to distance travelled. Agent with `battery <= 0` becomes dead and is excluded from allocation (battery gate in both Greedy and Auction allocators).
- `MembershipView::apply_movement()` moves agents and drains battery; `NodeConfig { enable_movement, tick_duration_ms }` controls movement per node.
- Movement affects connectivity automatically: `comms_range` + new pose recalculates links each tick via `ConnectivityModel`.
- New metrics: `final_battery_min`, `avg_distance_travelled`, `agents_exhausted`, `total_distance_travelled`, `mission_completion_ticks`, `time_to_first_exhaustion`.
- Backward compatible: `speed=0` / `enable_movement=false` by default means existing scenarios unchanged.

Run with JSON export:

```bash
cargo run -p swarm-examples --bin strategy_comparison -- --json results.json
```

Run with CSV export:

```bash
cargo run -p swarm-examples --bin strategy_comparison -- --csv results.csv
```

**Milestone 9** — complete. SAR v1 (Search and Rescue):

- `SearchGrid` — discrete search area divided into cells; each cell is a Task with a pose.
- `HiddenTarget` — targets randomly placed in cells; unknown to agents until scanned.
- Role-based sensor model: `Scout` (standard PoD), `Thermal` (elevated PoD), `Relay` (maintains mesh, reduced search capability).
- `SensorModel` — probability of detection (`scout_pod`, `thermal_pod`, `relay_pod`) applied when an agent scans a cell after arriving at its center.
- `GridState` — tracks per-cell scan progress, coverage fraction, and target discovery.
- New metrics: `time_to_find` (tick of first target discovery), `coverage_over_time` (fraction of cells scanned per tick), `probability_of_detection` (targets found / total), `targets_found`, `targets_total`, `scan_count`.
- Deterministic: target placement and scan outcomes use seeded RNG; reproducible via replay.
- Success criteria: all targets found before `max_ticks` or battery exhaustion.

Run the SAR scenario:

```bash
cargo run -p swarm-examples --bin sar_scenario
```

Sample output:

```
Targets found: 2/3
Time to first find: Some(145)
Final coverage: 0.72
PoD: 0.67
```

**Milestone 10** — complete. CBBA (Consensus-Based Bundle Algorithm):

- `CbbaAllocator` in `swarm-alloc` — first truly distributed allocation algorithm. Phase 1: bundle building (marginal score with distance + battery + position penalty). Phase 2: consensus (exchange winning bids via gossip, remote bids override local if higher).
- `Allocator` trait changed from `&self` to `&mut self` for stateful CBBA (28 call sites across 8 files).
- `StrategyRegistry` includes 5 strategies: greedy, auction, connectivity-aware, centralized, cbba.
- CBBA metrics: `cbba_rounds_to_convergence`, `cbba_converged`, `cbba_messages`.

**Milestone 10 Phase 1** — complete. True Distributed CBBA:

- Per-agent `CbbaAllocator` state (not shared facade) — each agent independently builds bundles.
- `send_cbba_bids()` broadcasts winning bids via `RuntimeMessage::Cbba` through transport.
- `apply_remote_bids()` called with real remote bids from other agents in simulation path.
- Registry sync: remote assignments registered locally via `assign()` so `all_tasks_assigned` works correctly.
- `Allocator::is_distributed()` distinguishes distributed strategies (CBBA returns `true`).
- `RunConfig.enable_cbba: bool` (default `false`) controls distributed CBBA per scenario.

```bash
cargo run -p swarm-examples --bin strategy_comparison -- --json results.json
```

**Phase 2** — complete. Unified Experiment Runner:

- `--mission coverage|emergency-mesh|sar|all` CLI flag for multi-mission benchmarks.
- `SarProfile` (Ideal, Standard, Challenging, BatteryConstrained) and `EmergencyMeshProfile` (Ideal, LowLoss, MediumLoss).
- `ComparisonReport` with mission, scenario, seed_range, and SAR metric columns (TimeToFind, PoD, TargetsFound).
- `AggregateMetrics` includes SAR fields: `avg_time_to_find`, `avg_probability_of_detection`, `avg_targets_found`.
- `ReportRow` with new columns in JSON/CSV export: mission, scenario, seed_range, time_to_find, probability_of_detection, targets_found.

Run all 3 missions with CBBA included:

```bash
cargo run -p swarm-examples --bin strategy_comparison --mission all --json all.json --csv all.csv
```

### Benchmark Results (10 seeds per cell)

| Strategy | Profile | Success | Coverage | Conflicts | BatteryAvg | TimeToFind | PoD | Targets |
|----------|---------|---------|----------|-----------|------------|------------|-----|---------|
| greedy   | coverage/ideal | 1.000 | 1.000 | 0 | 100.0 | - | - | - |
| greedy   | sar/ideal | 0.600 | 0.992 | 2679 | 92.0 | 3.0 | 0.10 | 0.2 |
| auction  | coverage/ideal | 1.000 | 1.000 | 0 | 100.0 | - | - | - |
| centralized | coverage/ideal | 1.000 | 1.000 | 0 | 100.0 | - | - | - |
| cbba     | coverage/ideal | 1.000 | 1.000 | 0 | 100.0 | - | - | - |
| cbba     | coverage/ideal-single-failure | 1.000 | 1.000 | 36 | 100.0 | - | - | - |
| cbba     | emergency-mesh/ideal | 0.200 | 0.800 | 104 | 100.0 | - | - | - |
| cbba     | sar/ideal | 0.000 | 0.347 | 1020 | 95.2 | 2.0 | 0.15 | 0.3 |

*(10 seeds per cell, quick mode)*

### M11 Hardening

- `mission` и `scenario` колонки в JSON/CSV заполняются реальными значениями (не пустые строки).
- `benchmark_run_id` включает имя миссии вместо хардкоженного "coverage".
- Property-based тесты для distributed CBBA: 2 proptest (100 случаев, ~3.4s).
- `seed_range_start`/`seed_range_end` в export из отчёта.

### Mission DSL v0.12

Декларативное описание сценариев в JSON. Сценарии становятся воспроизводимыми артефактами, новые benchmark cases добавляются без перекомпиляции.

Структура `ScenarioSuite`:
```json
{
  "name": "My Suite",
  "description": "Описание набора сценариев",
  "scenarios": [
    {
      "mission": "coverage",
      "profile": "ideal-no-failures",
      "scenario": { "name": "...", "seed": 0, "agents": [...], "tasks": [...], ... },
      "run_config": { "max_ticks": 50, ... }
    }
  ]
}
```

**CLI-флаг:** `--scenario-suite <path>` загружает сценарии из JSON вместо Rust-билдеров:
```bash
cargo run -p swarm-examples --bin strategy_comparison \
  --scenario-suite scenarios/coverage.ideal.json \
  --json results.json
```

**Примеры JSON-сценариев** в `scenarios/` директории:
- `coverage.ideal.json` — 5 агентов, 3 задачи, идеальная сеть
- `emergency-mesh.ideal.json` — 4 scout + 1 relay, ground node, base station
- `sar.ideal.json` — 3 scout + 1 thermal + 1 relay, 6×6 grid, 2 hidden targets

**Экспорт в JSON:**
```rust
use swarm_sim::{ScenarioSuite, ScenarioSuiteEntry, export_suite, load_scenario_suite};

// Загрузка
let suite = load_scenario_suite("scenarios/coverage.ideal.json")?;

// Экспорт
let json = export_suite(&suite)?;
std::fs::write("exported.json", json)?;
```

### M13 — Safety Layer

Физические и операционные ограничения: geofence, no-fly zones, separation constraints.

**Типы:**
- `Geofence` — допустимая область (AABB), агент должен оставаться внутри.
- `NoFlyZone` — запрещённая область (AABB), задачи внутри не назначаются.
- `SeparationConstraint` — минимальное расстояние между агентами.

**JSON с safety_config:**
```json
{
  "safety_config": {
    "geofence": {"bounds": {"min_x": 0, "max_x": 100, "min_y": 0, "max_y": 100}},
    "no_fly_zones": [
      {"bounds": {"min_x": 40, "max_x": 60, "min_y": 40, "max_y": 60}}
    ],
    "separation": {"min_distance_m": 2.0}
  }
}
```

**Запуск с safety:**
```bash
cargo run -p swarm-examples --bin strategy_comparison \
  --scenario-suite scenarios/coverage.safety.json --json safety.json
```

**Метрика:** `safety_violations` — количество нарушений за прогон (0 = идеально).

### M14 — SAR v2 / Uncertainty Map

Поисково-спасательная миссия с вероятностной картой уверенности (BeliefMap), Bayes-обновлением, повторными сканированиями и учётом ложных срабатываний.

**BeliefMap:**
- Каждая ячейка имеет `prior` и `posterior` вероятность наличия цели.
- Bayes-обновление при каждом сканировании: `P(target|detection)` и `P(target|no_detection)`.
- `entropy(cell)` — неопределённость Шеннона; максимум при `posterior = 0.5`.
- `highest_uncertainty_cells(n)` — возвращает ячейки с наибольшей энтропией для приоритизации.

**SensorModel v2:**
```rust
SensorModel {
    scout_pod: 0.6,
    thermal_pod: 0.95,
    relay_pod: 0.2,
    detection_probability: 0.6,  // P(detect | target present)
    false_positive_rate: 0.05,   // P(detect | no target)
}
```
- Role-based PoD остаётся для backward compatibility (SAR v1).
- `detection_probability` и `false_positive_rate` используются в Bayes-обновлении.

**Повторные сканирования:**
- Если `posterior ∈ (0.05, 0.95)` — ячейка требует повторного сканирования (confirmation scan).
- `confirmation_scans` — метрика количества повторных сканов.
- `false_positives` — количество detection-событий без реальной цели.

**Приоритизация на основе неопределённости:**
- `sar_task_priority(prior)` — статический priority при создании сценария (v0.14).
- Priority ∝ `entropy × prior`, масштабировано в диапазон [1, 10].

**Новые метрики:**
- `belief_entropy_final` — средняя энтропия BeliefMap после прогона.
- `false_positives` — количество ложных срабатываний.
- `confirmation_scans` — количество повторных сканирований.

**Профили:**
- `Uncertain` — moderate PoD (0.5), moderate FPR (0.2), требует repeated scans.
- `Noisy` — high FPR (0.4), требует многократных подтверждений.

**Запуск SAR v2:**
```bash
cargo run -p swarm-examples --bin strategy_comparison \
  --scenario-suite scenarios/sar.uncertain.json --json sar_v2.json

cargo run -p swarm-examples --bin strategy_comparison \
  --scenario-suite scenarios/sar.noisy.json --json sar_noisy.json
```

**Сценарии:**
- `scenarios/sar.ideal.json` — обновлён до SAR v2 (belief-aware)
- `scenarios/sar.uncertain.json` — moderate PoD + FPR
- `scenarios/sar.noisy.json` — high FPR, требует repeated scans

### M15 — CBBA Robustness

Исследование пределов CBBA: сходимость при packet loss, TSP-оптимизация bundles, retransmission,
convergence time distribution.

**TSP-ordering в bundles:**
- `order_bundle_tsp()` — greedy nearest-neighbour оптимизация порядка задач в bundle.
- `bundle_travel_distance` — суммарный путь агента по TSP-упорядоченному bundle.

**Retransmission policy:**
- `CbbaConfig.retransmit_max_attempts` (default: 3), `retransmit_backoff_ticks` (default: 2).
- `retransmit_threshold_packet_loss` (default: 0.1) — активация при >10% потерь.
- Periodic rebroadcast: при превышении порога agent переотправляет bids каждые N тиков.

**Partition healing:**
- `PartitionEvent.heal_at_tick: Option<u64>` — явное время завершения партиции.
- При heal_at_tick партиция снимается, CBBA сбрасывает convergence для повторной сходимости.

**Convergence time distribution:**
- `cbba_convergence_tick` — тик первого достижения consensus всеми агентами.
- `convergence_ticks_p50/p95/max` — перцентили времени сходимости.
- `avg_bundle_travel_distance` — средний путь агента по bundle.

**Запуск CBBA stress:**
```bash
cargo run -p swarm-examples --bin strategy_comparison \
  --scenario-suite scenarios/cbba_stress.json --json cbba_stress.json
```

**Сценарий:**
- `scenarios/cbba_stress.json` — 4 агента, 3 задачи, packet loss 0.0/0.1/0.2

### M16 — Infrastructure Inspection

Reference mission для обследования линейной инфраструктуры (ЛЭП, трубопроводы, периметр). Агенты покрывают граф рёбер, а не ячейки сетки.

**Типы:**
- `InspectionEdge` — ребро графа с `from`, `to`, `length_m`, `priority`.
- `InspectionGraph` — набор рёбер и базовая точка `depot`.
- Генераторы: `linear_route`, `grid_perimeter`, `random_graph`.

**Метрики:**
- `edge_coverage_rate` — доля покрытых рёбер.
- `missed_edges` — число пропущенных рёбер.
- `revisit_count` — повторные визиты одного ребра.
- `route_efficiency` — покрытое расстояние / общий путь агента.

**Запуск inspection benchmark:**
```bash
cargo run -p swarm-examples --bin strategy_comparison --mission inspection
cargo run -p swarm-examples --bin strategy_comparison \
  --scenario-suite scenarios/inspection.linear.json --json inspection.json
```

**Сценарии:**
- `scenarios/inspection.linear.json` — прямая ЛЭП, 3 агента.
- `scenarios/inspection.perimeter.json` — периметр 10×10, 4 агента, battery constraint.
- `scenarios/inspection.random.json` — случайный граф, 5 агентов.

**Benchmark-таблица (quick mode, 10 seeds):**

| Стратегия | Профиль | Успех | Завершение | EdgeCoverage | MissedEdges | Revisits | RouteEfficiency |
|-----------|---------|-------|------------|--------------|-------------|----------|-----------------|
| greedy    | inspection/linear | 1.000 | 1.000 | 1.000 | 0.0 | 0.0 | 0.850 |
| auction   | inspection/linear | 1.000 | 1.000 | 1.000 | 0.0 | 0.0 | 0.820 |

*(10 seeds per cell, quick mode)*

### M17 — SITL / MAVLink

Single-agent SITL runner для подключения к PX4 Software-In-The-Loop через MAVLink.

**Новые компоненты:**
- `MavlinkTransport` — реализация `Transport` trait через `rust-mavlink` crate (за feature-флагом `mavlink-transport`).
- `MockMavlinkTransport` — фиктивная реализация для тестов и `--mock` режима, не требует PX4.
- `task_to_waypoint(task)` — конвертация задачи в waypoint (без MAVLink зависимостей).
- `task_to_mavlink_waypoint(task)` — конвертация задачи в `MAV_CMD_NAV_WAYPOINT` (требует feature `mavlink-transport`).
- `sitl_agent` — новый binary, одновагентный SITL runner.

**Запуск в mock-режиме (без PX4):**
```bash
cargo run --bin sitl_agent -- --mock --scenario scenarios/coverage.ideal.json --agent-id agent-0
```

**Запуск с PX4 SITL:**
```bash
cargo run --bin sitl_agent -- \
  --connection udpout:127.0.0.1:14550 \
  --scenario scenarios/coverage.ideal.json \
  --agent-id agent-0
```

**Документация:** `docs/SITL_SETUP.md` — установка PX4, Gazebo, запуск SITL, troubleshooting.

**Зависимость:** `mavlink = "0.18"` (опциональная, feature `mavlink-transport`).

**Unit-тесты (без PX4):**
```bash
cargo test -p swarm-comms mock_mavlink
cargo test -p swarm-comms task_to_waypoint
cargo test --bin sitl_agent
```

Также обновлено описание `swarm-comms` в таблице Workspace Layout (MAVLinkTransport).
