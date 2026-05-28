# M41 Semantics Audit & Algorithmic Gap Triage

Аудит выполнен как bounded implementation pass без long-run benchmark прогонов. Базовый
контекст перед реализацией: `01004f3` (`plan: include connectivity-aware in M41 audit`).

## Scope

M41 проверяет, что mission/task semantics не теряются между DSL, scenario builders,
allocator/planner paths, runner state aggregation and metrics. Цель этого шага - не
переписать алгоритмы, а отделить high-confidence wiring bugs от algorithm mismatch,
unsupported combinations и cases that need more data.

Не входило в scope:

- full 1000-seed validation;
- broad benchmark threshold tuning;
- переписывание CBBA/centralized/connectivity-aware algorithms;
- новый public product API beyond a small descriptive support-matrix model.

## Semantics paths

- `crates/swarm-types/src/adapter.rs`
  - `SarAdapter` uses `Task.grid_cell` and `RunState.scanned_cells`.
  - `InspectionAdapter` uses `Task.edge_id` and `RunState.covered_edges`.
  - `WildfireAdapter` uses task id as mapped zone id and `RunState.mapped_zones`.
  - `CoverageAdapter`, `RelayAdapter` and `WaypointAdapter` use exact `RunState.completed_tasks`.
- `crates/swarm-sim/src/runner.rs`
  - `build_run_state` converts runtime SAR/inspection/wildfire/task status state into `RunState`.
  - `adapter_driven_complete` checks all known task kinds through `AdapterRegistry`.
  - `compute_mission_success` still uses mission-specific state branches for SAR/inspection/wildfire; `adapter_complete` is a loop-exit guard and a future consistency signal, not the final success source for these branches.
- `crates/swarm-sim/src/dsl.rs`
  - Required semantic fields are validated per task kind.
  - M41 adds mission/task-kind mismatch validation for SAR, inspection, wildfire, SITL, coverage and emergency-mesh.
- `crates/swarm-alloc/src/allocator.rs`
  - Greedy/Auction keep default allocation but provide adapter-aware paths through `allocate_with_adapter` / `allocate_with_registry`.
- `crates/swarm-alloc/src/centralized.rs`
  - Centralized remains a benchmark/oracle baseline with static pre-planned assignments.
  - SAR + centralized is kept unsupported with `static_pre_plan`.
- `crates/swarm-alloc/src/connectivity_aware.rs`
  - Connectivity-aware is a separate relay-placement path, not a generic auction alias.
  - It splits relay/scout tasks, uses `task.pose` for relay placement, falls back to base auction when pose is absent and evaluates reachability with `simulate_reachability_with_agent_at_pose`.
- `crates/swarm-alloc/src/route_planner.rs`
  - `BatteryAwarePlanner` must test feasibility on the current ordered subset after each tail drop.

## Gap Classes

| Gap class | Evidence path | Classification | Confidence | Recommended action |
| --- | --- | --- | --- | --- |
| SAR + CBBA fails stable support | `crates/swarm-examples/tests/support_matrix.rs` and `classify_support` | algorithm mismatch / unsupported combination | high | Keep unsupported reason `delayed_reconvergence`; do not present as stable until distributed reconvergence semantics are improved. |
| SAR + centralized fails stable support | `support_matrix_sar_centralized_is_unsupported` | algorithm mismatch / unsupported combination | high | Keep unsupported reason `static_pre_plan`; centralized is static baseline, not SAR scan-feedback planner. |
| Connectivity-aware relay path was implicit | `crates/swarm-alloc/src/connectivity_aware.rs` tests | accepted experimental limitation | high | Track as experimental for emergency-mesh/relay until relay-placement invariants are promoted to regression gates. |
| Mission/task-kind mismatch accepted by DSL | `crates/swarm-sim/src/dsl.rs` tests | implementation bug | high | M41 fixes validator for explicit task kinds. |
| Battery-aware route subset weak assertion | `crates/swarm-alloc/src/route_planner.rs` tests | implementation/test gap | high | M41 strengthens ordered-subset and first-task-infeasible tests. |
| Wildfire medium-dynamic sensitivity | existing support-matrix test and regression comments | scenario/profile constrained | medium | Keep experimental; M42 can decide whether it becomes default gate or tracking-only suite. |
| CBBA mission-agnostic scoring | code audit of `cbba.rs` vs adapter-aware allocators | algorithm mismatch / accepted limitation | medium | Do not rewrite in M41; add root-cause note for algorithm backlog. |

## Support Matrix

M41 adds `crates/swarm-sim/src/support_matrix.rs` as a small descriptive model:

- `SupportStatus`: `Supported`, `Experimental`, `Unsupported`, `KnownBug`, `NotEvaluated`.
- `SupportReason`: stable baseline, delayed reconvergence, static pre-plan, dynamic threat drift, relay-placement experimental, profile constrained, missing evidence.

Current explicit classifications:

- SAR + greedy + ideal: supported baseline.
- SAR + CBBA: unsupported, `DelayedReconvergence`.
- SAR + centralized: unsupported, `StaticPrePlan`.
- inspection + greedy + linear: supported baseline.
- inspection + greedy + perimeter: experimental/profile constrained.
- wildfire + greedy + small-static: supported baseline.
- wildfire + greedy + medium-dynamic: experimental/dynamic threat drift.
- emergency-mesh + connectivity-aware: experimental/relay placement semantics.

The model is descriptive and intentionally not wired into regression gating yet. M42 should decide which classifications become default gates.

## High-Confidence Fixes

- Added explicit DSL mission/task-kind mismatch validation.
- Added adapter negative-path tests for exact SAR cell, inspection edge, wildfire zone and completed-task identity.
- Added runner tests for `build_run_state` and `adapter_driven_complete`.
- Added precise `BatteryAwarePlanner` ordered-subset tests.
- Added `ConnectivityAwareAllocator` tests for relay/scout split, pose-based reachability choice, no-pose fallback and no-relay behavior.
- Added centralized role-filter test.
- Added support-matrix model and tests for SAR unsupported combinations and connectivity-aware emergency-mesh classification.

## Regression Candidates

Good candidates for M42 default gates:

- adapter exact-semantics tests in `swarm-types`;
- DSL mission/task-kind mismatch tests in `swarm-sim`;
- route planner ordered-subset tests in `swarm-alloc`;
- support-matrix classification tests for known unsupported combinations.

Good candidates for M42 experimental/tracking suites:

- emergency-mesh + connectivity-aware end-to-end runs;
- wildfire medium-dynamic completion consistency;
- CBBA distributed behavior under packet loss and partitions;
- centralized route quality metrics where route planner is configured.

Excluded from default gates for now:

- 1000-seed benchmark packs;
- broad algorithm comparisons across every mission/profile/strategy;
- any scenario requiring remote simulator/SITL infrastructure.

## Follow-ups

- Decide whether `compute_mission_success` should consume `adapter_complete` directly for any mission family or keep mission-state branches as canonical success sources.
- Decide whether `connectivity-aware` should remain experimental or become stable for a narrow emergency-mesh profile.
- Add reproduction notes for suspicious metric mismatches once M42 defines regression suite boundaries.
- Add deeper CBBA vs adapter-aware scoring triage as a separate algorithm milestone.
