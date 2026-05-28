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

### SAR + CBBA fails stable support

- Reproducible command or fixture: `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300s /home/formi/.local/bin/runlim cargo test -p swarm-examples --test support_matrix support_matrix_sar_cbba_is_unsupported`
- Expected behavior: unsupported SAR strategy combinations should be classified explicitly and should not be reported as stable support.
- Actual behavior: SAR + CBBA is classified as `Unsupported` with reason `DelayedReconvergence`; the executable support-matrix test expects `unsupported_reason = "delayed_reconvergence"`.
- Likely cause: CBBA is a distributed bundle algorithm whose current convergence/replanning semantics do not match SAR scan-feedback timing well enough to be a stable supported combination.
- Confidence: high.
- Classification: algorithm mismatch / unsupported strategy-mission combination / weak distributed behavior.
- Recommended action: keep SAR + CBBA outside stable support; use M42 to decide whether distributed reconvergence work becomes a separate algorithm milestone.

### SAR + centralized fails stable support

- Reproducible command or fixture: `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300s /home/formi/.local/bin/runlim cargo test -p swarm-examples --test support_matrix support_matrix_sar_centralized_is_unsupported`
- Expected behavior: centralized strategy should not be presented as stable for SAR if it cannot react to scan feedback and target discovery semantics.
- Actual behavior: SAR + centralized is classified as `Unsupported` with reason `StaticPrePlan`; the executable support-matrix test expects `unsupported_reason = "static_pre_plan"`.
- Likely cause: `CentralizedPlanner` is a static oracle baseline that precomputes assignments from initial scenario data, while SAR progress depends on runtime scan results and target discovery.
- Confidence: high.
- Classification: algorithm mismatch / unsupported strategy-mission combination.
- Recommended action: keep SAR + centralized unsupported unless a future centralized feedback-aware SAR planner is introduced.

### Connectivity-aware relay path was implicit

- Reproducible command or fixture: `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300s /home/formi/.local/bin/runlim cargo test -p swarm-alloc -- connectivity_aware`
- Expected behavior: emergency-mesh relay placement should be audited as a separate semantic strategy path, including relay/scout split, `task.pose`, no-pose fallback and reachability simulation.
- Actual behavior: M41 confirms the path is separate from generic `auction` behavior and adds tests for relay assignment, pose-based reachability choice, no-pose fallback and no-relay edge cases.
- Likely cause: `ConnectivityAwareAllocator::allocate_with_connectivity` implements mission-specific relay-placement semantics, but previous documentation/support-matrix scope treated it too implicitly.
- Confidence: high.
- Classification: accepted experimental limitation / support-matrix coverage gap.
- Recommended action: keep `connectivity-aware` experimental for emergency-mesh/relay until M42 promotes relay-placement invariants into default regression gates.

### Mission/task-kind mismatch accepted by DSL

- Reproducible command or fixture: `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300s /home/formi/.local/bin/runlim cargo test -p swarm-sim -- validate_`
- Expected behavior: explicit task kinds should match mission semantics: SAR should use SAR task kinds, inspection should use inspection edges, wildfire should use mapping zones, SITL should use waypoints, and emergency mesh should allow coverage + relay placement.
- Actual behavior: before M41, validator checked required fields per task kind but did not reject mission/task-kind mismatches. M41 adds explicit mismatch validation and tests.
- Likely cause: validation grew from field requirements first and did not yet encode mission-family compatibility.
- Confidence: high.
- Classification: implementation bug, fixed in M41.
- Recommended action: keep the validator strict for explicit task kinds; preserve emergency-mesh mixed-kind allowance.

### Battery-aware route subset weak assertion

- Reproducible command or fixture: `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300s /home/formi/.local/bin/runlim cargo test -p swarm-alloc -- route_planner`
- Expected behavior: `BatteryAwarePlanner` should drop only the tail tasks needed to make the current ordered subset feasible, and should return empty only when even the first task is infeasible.
- Actual behavior: route feasibility code already checks the ordered subset, but the existing test had a weak assertion that did not prove this property. M41 replaces it with precise ordered-subset and first-task-infeasible tests.
- Likely cause: previous test described the intended fix but did not encode an input where the subset behavior was observable.
- Confidence: high.
- Classification: implementation/test gap; no runtime logic change required in this pass.
- Recommended action: keep the strengthened tests as default gates; treat future route feasibility regressions as high-confidence implementation bugs.

### Wildfire medium-dynamic sensitivity

- Reproducible command or fixture: `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300s /home/formi/.local/bin/runlim cargo test -p swarm-examples --test support_matrix support_matrix_wildfire_medium_dynamic_completion_consistency`
- Expected behavior: wildfire medium-dynamic should remain runnable without `unsupported_reason`, but should not automatically become a default stable success gate until dynamic threat semantics are better bounded.
- Actual behavior: the support-matrix test asserts no `unsupported_reason`; success can still depend on mapped ratio, priority updates and max unassigned ticks.
- Likely cause: dynamic threat updates and profile constraints make completion sensitive to scenario parameters rather than a single obvious wiring bug.
- Confidence: medium.
- Classification: scenario/profile constrained / dynamic scenario weakness.
- Recommended action: keep as experimental tracking; M42 should decide whether to gate on completion, mapped ratio, or a narrower deterministic fixture.

### CBBA mission-agnostic scoring

- Reproducible command or fixture: code audit of `crates/swarm-alloc/src/cbba.rs` compared with adapter-aware `GreedyAllocator`/`AuctionAllocator` paths in `crates/swarm-alloc/src/allocator.rs`.
- Expected behavior: stable mission-aware allocation should receive enough task/mission context to score SAR, inspection, mapping, waypoint and relay tasks consistently with adapters.
- Actual behavior: current CBBA scoring is mostly distance/battery/bundle oriented and does not use the adapter registry in the same way as adapter-aware greedy/auction paths.
- Likely cause: CBBA implementation predates the current adapter-registry semantics and optimizes distributed bundle behavior first.
- Confidence: medium.
- Classification: algorithm mismatch / accepted limitation.
- Recommended action: do not rewrite CBBA in M41; create an algorithm backlog item if SAR/inspection/wildfire CBBA support becomes a chosen direction.

### Suspicious metric mismatch

- Reproducible command or fixture: bounded M41 code audit of `crates/swarm-sim/src/runner.rs`, `crates/swarm-metrics/src/metrics.rs`, `crates/swarm-sim/src/report_export.rs` and targeted tests listed above; no long benchmark replay was run by scope.
- Expected behavior: if a concrete metric mismatch is found, it should have a small reproduction path, expected value, actual value and high-confidence fix or follow-up.
- Actual behavior: no concrete suspicious metric mismatch was reproduced during this bounded M41 pass. The visible metric-related finding is semantic: `compute_mission_success` receives `adapter_complete` but uses mission-specific state branches for SAR/inspection/wildfire; this is documented as an intentional/audit-open design point rather than a proven metric bug.
- Likely cause: previous M40 work already hardened report identity/parity, while this M41 pass focused on semantics wiring and did not run broad benchmark comparisons.
- Confidence: medium.
- Classification: needs more data / no fix added in M41.
- Recommended action: M42 should add a small metric reproduction only if a concrete mismatch appears in regression candidates; do not add placeholder tests without a reproducible mismatch.

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
