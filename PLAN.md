# План M41: Semantics Audit & Algorithmic Gap Triage

## Context

Задача M41 из `docs_raw/DRONE_A.15.linear.md` — провести короткий, но строгий аудит семантического wiring перед следующей волной regression work. Цель не в том, чтобы переписать стратегии или закрыть все слабые комбинации, а в том, чтобы отделить:

- баги метрик;
- баги реализации;
- algorithm mismatch;
- scenario/profile too hard или ill-posed;
- accepted limitations;
- cases that need more data.

Текущая база уже содержит важные элементы M40/M35/M39:

- `crates/swarm-types/src/adapter.rs` и `crates/swarm-types/src/mission.rs` задают `MissionAdapter`, `RunState`, adapter registry и per-kind completion/scoring semantics.
- `crates/swarm-sim/src/runner.rs` строит `RunState`, вызывает adapter-driven completion, считает mission-specific success и пишет `unsupported_reason`.
- `crates/swarm-sim/src/dsl.rs` валидирует `ScenarioSuite`, mission-specific requirements и часть task-kind required fields.
- `crates/swarm-alloc/src/route_planner.rs` содержит `BatteryAwarePlanner` и уже имеет несколько feasibility tests.
- `crates/swarm-examples/tests/support_matrix.rs` частично документирует supported/unsupported combinations через тесты, но полноценной support matrix model пока нет.
- `crates/swarm-sim/src/regression.rs` содержит default suites and thresholds, где часть weak combinations уже помечена comments как experimental/physically constrained.

Важное наблюдение по текущему коду: `compute_mission_success` в `crates/swarm-sim/src/runner.rs` принимает `_adapter_complete`, но параметр сейчас не используется напрямую в mission-specific branches. Это не обязательно баг, но M41 должен явно проверить, где completion должен быть adapter-driven, где mission-state driven, а где это intentional fallback.

Длинные прогоны и большие validation artifacts не входят в этот milestone. Все проверки в реализации M41 должны быть короткими, локальными и покрываться unit/integration tests с hard timeout при запуске.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует.

Контекст, собранный перед планированием:

- `docs_raw/DRONE_A.15.linear.md`, секция M41.
- `crates/swarm-types/src/adapter.rs`, `mission.rs`, `task.rs`.
- `crates/swarm-sim/src/runner.rs`, особенно `compute_mission_success`, `build_run_state`, `adapter_driven_complete`, финальный metrics block.
- `crates/swarm-sim/src/dsl.rs`, особенно `validate_mission_specific`.
- `crates/swarm-alloc/src/route_planner.rs`, especially `BatteryAwarePlanner`.
- `crates/swarm-examples/tests/support_matrix.rs`.
- `crates/swarm-sim/src/regression.rs`, default suites comments/thresholds.

## Affected components

- `crates/swarm-types/src/adapter.rs`
  - adapter completion/scoring tests for SAR, inspection, wildfire/mapping, waypoint, coverage/relay.
- `crates/swarm-types/src/mission.rs`
  - `RunState` semantics and potential fixture helpers.
- `crates/swarm-types/src/task.rs`
  - task-kind semantic fields: `grid_cell`, `edge_id`, `pose`, `kind`.
- `crates/swarm-sim/src/runner.rs`
  - `build_run_state`, `adapter_driven_complete`, `compute_mission_success`, final `RunMetrics`.
- `crates/swarm-sim/src/dsl.rs`
  - DSL validation for mission/task-kind mismatches and required semantic fields.
- `crates/swarm-sim/src/regression.rs`
  - possible follow-up tagging of stable vs experimental suites, but avoid broad M42 work.
- `crates/swarm-alloc/src/allocator.rs`
  - adapter-aware allocation path for greedy/auction-style allocators.
- `crates/swarm-alloc/src/cbba.rs`
  - likely classification target: CBBA scoring is mostly distance/battery and may not be mission-adapter-aware.
- `crates/swarm-alloc/src/route_planner.rs`
  - battery-aware feasibility and route subset behavior.
- `crates/swarm-scenarios/src/sar_scenario.rs`
  - SAR task `grid_cell` and scenario state wiring.
- `crates/swarm-scenarios/src/inspection.rs`
  - inspection task `edge_id` and graph wiring.
- `crates/swarm-scenarios/src/wildfire.rs`
  - mapping zone task identity/threat/priority wiring.
- `crates/swarm-scenarios/src/coverage.rs`, `emergency_mesh.rs`
  - waypoint/coverage/relay pose and task-kind validation context.
- `crates/swarm-examples/tests/support_matrix.rs`
  - current executable support-boundary assertions.
- New or updated docs artifact, proposed path:
  - `docs_raw/M41_SEMANTICS_AUDIT.md`

## Implementation steps

1. Inventory current semantics paths.
   - Read and summarize `crates/swarm-types/src/adapter.rs`, `crates/swarm-sim/src/runner.rs`, `crates/swarm-sim/src/dsl.rs`, `crates/swarm-alloc/src/allocator.rs`, `crates/swarm-alloc/src/cbba.rs`, `crates/swarm-alloc/src/route_planner.rs`.
   - Create `docs_raw/M41_SEMANTICS_AUDIT.md` with sections: `Scope`, `Semantics paths`, `Gap classes`, `Support matrix`, `High-confidence fixes`, `Regression candidates`, `Follow-ups`.
   - Do not use long benchmark runs as evidence; use code audit and small deterministic fixtures.

2. Add a lightweight support matrix model or make the current support matrix explicit.
   - Preferred: add `crates/swarm-sim/src/support_matrix.rs` with small enums/structs such as `SupportStatus`, `SupportReason`, and a lookup function keyed by mission/profile/strategy.
   - Export it from `crates/swarm-sim/src/lib.rs` only if needed by tests or examples.
   - Mirror existing expectations from `crates/swarm-examples/tests/support_matrix.rs`: SAR + greedy supported, SAR + CBBA `delayed_reconvergence`, SAR + centralized `static_pre_plan`, wildfire/inspection stable/experimental boundaries.
   - Keep this model descriptive; do not wire it into regression gating yet unless implementation is trivial. Gating belongs mostly to M42.

3. Audit and test adapter completion semantics.
   - Extend tests in `crates/swarm-types/src/adapter.rs`.
   - Cover happy path and negative path:
     - SAR scan completes only when its exact `grid_cell` exists in `RunState.scanned_cells`.
     - SAR confirmation scan uses the SAR adapter and retains cell context.
     - Inspection edge completes only when exact `edge_id` is covered.
     - Wildfire/mapping zone completes only when the corresponding zone id is mapped.
     - Waypoint/coverage/relay completion uses `completed_tasks` and does not accidentally pass due to unrelated mission state.
   - Add edge cases for missing `grid_cell`, missing `edge_id`, missing pose where relevant.

4. Audit runner wiring from runtime state to `RunState`.
   - Add or extend unit tests near `crates/swarm-sim/src/runner.rs` for `build_run_state` and `adapter_driven_complete`.
   - If private visibility blocks direct tests, prefer small local test-only helpers or `pub(crate)` exposure over broad public API changes.
   - Verify:
     - SAR visited/target-found cells become `RunState.scanned_cells`.
     - Inspection covered edges become `RunState.covered_edges`.
     - Wildfire mapped zone ids become `RunState.mapped_zones`.
     - Assigned/completed coverage/waypoint/relay tasks become `RunState.completed_tasks`.
   - Explicitly decide whether `compute_mission_success` should use `adapter_complete` for each mission branch. If it should not, document why in `docs_raw/M41_SEMANTICS_AUDIT.md`.

5. Audit allocator/planner semantic context.
   - In `crates/swarm-alloc/src/allocator.rs`, confirm adapter-aware greedy/auction paths call `adapter.score` and `adapter.route_cost` where intended.
   - In `crates/swarm-alloc/src/cbba.rs`, classify current mission-agnostic scoring as either accepted limitation, experimental, or implementation gap. Avoid rewriting CBBA unless a tiny high-confidence bug is found.
   - In `crates/swarm-alloc/src/route_planner.rs`, strengthen `BatteryAwarePlanner` tests so they prove it drops only as many tasks as required for the current ordered subset. Existing tests show the area but one test has a weak assertion and should be made precise.

6. Strengthen DSL validation for task-kind mismatches.
   - Update `crates/swarm-sim/src/dsl.rs`.
   - Existing validation checks required fields for task kinds. Add tests and, if missing, logic for mismatches such as:
     - `mission = "sar"` with non-SAR task kind;
     - `mission = "inspection"` with non-inspection task kind;
     - `mission = "wildfire"` with non-`MappingZone` task kind;
     - `mission = "sitl"` with non-`Waypoint` task kind when a kind is present.
   - Preserve permissiveness for `coverage`, `emergency-mesh`, and mixed suites only where the current domain genuinely requires mixed task kinds, especially coverage + relay placement in emergency mesh.

7. Classify gap classes in `docs_raw/M41_SEMANTICS_AUDIT.md`.
   - For each gap class record:
     - reproducible unit/integration test or small fixture;
     - expected behavior;
     - actual behavior;
     - likely cause;
     - confidence: high/medium/low;
     - classification: metric bug, implementation bug, algorithm mismatch, scenario too hard, accepted limitation, needs more data;
     - recommended action.
   - Start with known classes from M41: suspicious metric mismatch, unsupported strategy/mission combination, weak distributed behavior, profile-specific failure, dynamic scenario weakness, route/battery feasibility mismatch.

8. Fix only high-confidence wiring bugs.
   - Allowed fixes:
     - obvious metric extraction bug;
     - obvious success predicate inconsistency;
     - obvious assignment/completion mismatch;
     - support matrix mistake;
     - small DSL validation mismatch;
     - small planner feasibility mismatch.
   - For algorithmic weakness or ambiguous semantics, add classification and follow-up instead of broad code changes.

9. Prepare M42 input.
   - In `docs_raw/M41_SEMANTICS_AUDIT.md`, add `Regression candidates`:
     - checks that should become default gates;
     - checks that should remain experimental;
     - checks that should be excluded from default gate;
     - known unsupported combinations that must not be presented as stable.
   - Do not implement full regression harness v3 in M41.

10. Keep verification bounded.
   - Run only targeted tests touched by M41, for example:
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300s /home/formi/.local/bin/runlim cargo test -p swarm-types -- adapter`
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300s /home/formi/.local/bin/runlim cargo test -p swarm-sim -- dsl runner`
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300s /home/formi/.local/bin/runlim cargo test -p swarm-alloc -- route_planner`
     - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300s /home/formi/.local/bin/runlim cargo test -p swarm-examples --test support_matrix`
   - If code changes are made, run `cargo fmt --all` and clippy over affected crates with a hard timeout.
   - Do not run full 1000-seed validation or broad benchmark packs.

## Testing strategy

### 1. Tests that need no refactoring

- `crates/swarm-types/src/adapter.rs`
  - Add adapter negative-path tests for missing or mismatched semantic context:
    - SAR task with `grid_cell = None` is not completed.
    - SAR task with different scanned cell is not completed.
    - inspection edge with different covered edge is not completed.
    - wildfire zone with unrelated mapped zone is not completed.
    - waypoint/coverage/relay require exact `completed_tasks` id.
- `crates/swarm-sim/src/dsl.rs`
  - Add validation tests for task-kind required field failures already supported by validator.
  - Add validation tests for mission/task-kind mismatches before implementing the validator change.
- `crates/swarm-alloc/src/route_planner.rs`
  - Replace weak feasibility assertion with a precise test where the full ordered route is infeasible but dropping exactly the tail task makes it feasible.
  - Add one negative case where even the first task is infeasible and output should be empty.
- `crates/swarm-examples/tests/support_matrix.rs`
  - Add assertions for combinations already known unsupported or experimental, without requiring long runs.
- `crates/swarm-metrics/src/metrics.rs`
  - Add targeted metric consistency tests if a high-confidence metric bug is found.

### 2. Tests that need light refactoring

- Add small task builders by `TaskKind`, preferably local to test modules first:
  - SAR scan fixture with `grid_cell`;
  - inspection edge fixture with `edge_id`;
  - wildfire mapping fixture with zone id/priority;
  - waypoint fixture with pose.
- Add in-memory `RunState` fixture helpers for adapter/runner tests.
- Add small scenario fixtures for runner tests that avoid shelling out to binaries.
- Add support matrix fixture builder if `crates/swarm-sim/src/support_matrix.rs` is introduced.
- Add helper to compare per-run metrics and aggregate metrics for suspicious metric mismatch tests, reusing M40 compare style where practical.

### 3. Tests that need heavy refactoring

- Full lifecycle tests: DSL -> adapter -> allocator -> runner -> metrics.
- Property tests for success/completion/coverage consistency across generated task kinds and mission states.
- Algorithm-comparison oracle tests for CBBA vs centralized/greedy.
- Scenario minimization tooling for failed simulation combinations.
- Mission-specific simulation invariants that run across many profiles/seeds.

These heavy tests should be listed as follow-ups unless a small subset can be isolated without expanding M41 beyond its scope.

## Risks and tradeoffs

Что могло сломаться при реализации M41:

- Behavior:
  - Stricter DSL validation can reject scenario suites that previously loaded despite ambiguous task kinds. Проверка: add backward-compatible tests for existing `scenarios/*.json` and document intentional rejections.
- API/contracts:
  - Introducing a support matrix API may create a public contract too early. Проверка: keep it `pub(crate)` or clearly minimal unless external crates need it.
- Metrics:
  - Changing success/completion semantics can move benchmark/regression numbers. Проверка: only change high-confidence bugs and add targeted before/after tests.
- Algorithm behavior:
  - Making allocators more adapter-aware can change task assignment order. Проверка: keep broad allocator rewrites out of M41; classify algorithm mismatch instead.
- Data/docs:
  - `docs_raw/M41_SEMANTICS_AUDIT.md` can become stale if it mixes observations and intended future work. Проверка: record exact commit hash and keep follow-ups separate from verified findings.
- Performance/resources:
  - New tests should stay small and in-memory. Проверка: run targeted tests only with `timeout 300s` and `/home/formi/.local/bin/runlim` for `cargo test`.

Tradeoff: a stricter support matrix and validator improve clarity but can expose existing weak combinations as unsupported/experimental. That is expected for M41, as long as unsupported combinations are not silently hidden as stable support.

## Open questions

- Should the support matrix be a code-level module in `swarm-sim`, a docs-only artifact, or both? Recommended: both, but keep the code API small and mostly internal.
- Should `compute_mission_success` use `adapter_complete` directly for SAR/inspection/wildfire, or are mission-specific state checks intentionally more precise? This must be decided per mission and documented.
- Are SAR + CBBA and SAR + centralized permanently unsupported, or experimental until planner/strategy semantics improve?
- For wildfire/flood mapping, is task id the canonical zone identity, or should task metadata carry an explicit zone id/threat id?
- Should mission/task-kind mismatch validation be strict for every mission, or should emergency mesh remain explicitly mixed-kind?
- Which M41 findings should become M42 default gates versus experimental tracking suites?
