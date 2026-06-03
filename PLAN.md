# M77 - Algorithm Differentiation

## Context

M77 продолжает линейный план из `docs_raw/BEFORE_HARDWARE_A.23.md`: до реального железа проекту нужно не просто иметь несколько стратегий, а уметь показывать, где и почему они ведут себя по-разному. Сейчас часть стратегий проходит через одинаковые или почти одинаковые decision paths, поэтому benchmark иногда измеряет общую инфраструктуру, а не различия алгоритмов.

Цель M77: сделать различия измеримыми и честно задокументированными, не переписывая архитектуру и не заявляя неподтвержденные преимущества. Все новые настройки должны быть выключены или нейтральны по умолчанию, чтобы старые сценарии, логи и результаты оставались воспроизводимыми.

Не делаем в M77:

- hierarchical coordination;
- полный rewrite allocator/runtime;
- большой publication run на 1000 seeds;
- новые hardware/PX4 claims;
- algorithm behavior changes без targeted evidence.

## Investigation context

`INVESTIGATION.md` отсутствует. Отдельного расследования перед M77 нет; план основан на текущем коде, `docs_raw/BEFORE_HARDWARE_A.23.md`, README/docs и локальной структуре crates.

Ключевые факты из discovery:

- `AllocationAgent` уже содержит `comms_range` в `crates/swarm-types/src/allocation.rs`; `GreedyAllocator` и `AuctionAllocator` в `crates/swarm-alloc/src/allocator.rs` сейчас фактически не используют connectivity при scoring.
- `RunConfig` находится в `crates/swarm-sim/src/runner/types.rs`; туда логично добавить флаги M77 с `serde(default)`.
- SAR belief map уже существует: `GridState::scan_cell` в `crates/swarm-runtime/src/grid_state.rs` обновляет `BeliefMap` через Bayes rule, а `BeliefMap::entropy` и `highest_uncertainty_cells` есть в `crates/swarm-types/src/grid.rs`. M77 должен переупорядочить unfinished SAR tasks по существующей энтропии, а не строить новый sensor stack.
- Wildfire dynamic threat уже обновляет `TaskPriorityUpdated` в `crates/swarm-sim/src/runner/internal/wildfire.rs`, но priority crossing не вызывает force reallocation.
- CBBA replay уже пишет `CbbaBundleUpdated` и `CbbaConverged`, но `CbbaBundleUpdated` не содержит conflict count. Runtime агрегирует `NodeTickOutput.conflicting_assignments` в `run_tick_loop`, поэтому диагностическое поле можно добавить без изменения core CBBA algorithm.
- Targeted benchmark plumbing уже есть вокруг `crates/swarm-examples/src/strategy_comparison_runtime/*`, но для M77 нужен узкий delta run, а не повтор M69/M62.

## Affected components

- `crates/swarm-alloc/src/allocator.rs`: comms-aware scoring for greedy/auction, default-neutral behavior, allocator tests.
- `crates/swarm-types/src/allocation.rs`: только если нужен helper around `AllocationAgent.comms_range`; основное поле уже есть.
- `crates/swarm-sim/src/runner/types.rs`: M77 config flags:
  - `comms_penalty_weight: f64`, default `0.0`;
  - `wildfire_priority_realloc_threshold: Option<u8>`, default `None`;
  - `dynamic_belief_updates: bool`, default `false`;
  - optional CBBA gossip burst knob only if diagnostic replay supports it.
- `crates/swarm-sim/src/runner/internal/tick_loop.rs`: pass M77 config into SAR/wildfire handling, record CBBA conflict diagnostics, wire priority-triggered reallocation.
- `crates/swarm-sim/src/runner/internal/wildfire.rs`: detect priority threshold crossings and emit explicit reallocation requests.
- `crates/swarm-sim/src/runner/internal/sar.rs`: update unfinished SAR task ordering when dynamic belief updates are enabled.
- `crates/swarm-replay/src/event_log.rs`: add backward-compatible replay fields/events.
- `crates/swarm-replay/src/replay/render.rs`, `summary.rs`, `state.rs`, tests: render/parse new diagnostics.
- `crates/swarm-sim/src/support_matrix.rs`: update support matrix based on actual M77 result, especially CBBA limitation or gossip-burst evidence.
- `crates/swarm-examples/src/strategy_comparison_runtime/cli.rs`, `missions.rs`, `run.rs` or adjacent runtime files: add targeted M77 delta run support.
- `README.md`, `docs/STATUS.md`, `docs/BENCHMARK_RESULTS.md`, `docs/SCENARIO_DSL.md`, `docs/REPLAY.md`, `docs/EXTENSION_GUIDE.md`: document settings, behavior, targeted evidence, limitations, and support matrix changes.
- `results/m77_algorithm_delta/` or more specific `results/m77_<profile>_<date>/`: targeted result artifacts.

## Implementation steps

1. Add neutral M77 config fields to `RunConfig`.

   File: `crates/swarm-sim/src/runner/types.rs`, near `pub struct RunConfig`.

   Add fields with serde defaults:

   ```rust
   /// Penalty weight for assigning tasks outside an agent's communication range.
   #[serde(default)]
   pub comms_penalty_weight: f64,
   /// Wildfire priority threshold that forces release/reallocation when crossed.
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub wildfire_priority_realloc_threshold: Option<u8>,
   /// Re-rank unfinished SAR tasks by posterior uncertainty after scan events.
   #[serde(default)]
   pub dynamic_belief_updates: bool,
   ```

   Keep defaults equivalent to current behavior: `0.0`, `None`, `false`.

   Update scenario DSL docs in `docs/SCENARIO_DSL.md` after implementation.

2. Make allocation scoring communication-aware, with old behavior preserved when `comms_penalty_weight == 0.0`.

   File: `crates/swarm-alloc/src/allocator.rs`.

   Convert `GreedyAllocator` from unit struct to a backward-compatible configured struct:

   ```rust
   #[derive(Clone, Debug)]
   pub struct GreedyAllocator {
       pub comms_penalty_weight: f64,
   }

   impl Default for GreedyAllocator {
       fn default() -> Self {
           Self { comms_penalty_weight: 0.0 }
       }
   }
   ```

   Update call sites that instantiate `GreedyAllocator` as a unit struct to `GreedyAllocator::default()` or `GreedyAllocator { comms_penalty_weight }`.

   Extend `AuctionAllocator`:

   ```rust
   pub struct AuctionAllocator {
       pub weight_distance: f64,
       pub weight_battery: f64,
       pub weight_role: f64,
       pub comms_penalty_weight: f64,
   }
   ```

   Add a shared helper:

   ```rust
   fn communication_penalty(weight: f64, task: &Task, agent: &AllocationAgent) -> f64 {
       if weight <= 0.0 {
           return 0.0;
       }
       let Some(task_pose) = task.pose else {
           return 0.0;
       };
       if !agent.comms_range.is_finite() || agent.comms_range <= 0.0 {
           return 0.0;
       }
       let over_range = agent.pose.distance_to(&task_pose) - agent.comms_range;
       weight * over_range.max(0.0)
   }
   ```

   Use this helper in `AuctionAllocator::cost`, `AuctionAllocator::allocate_with_adapter`, and greedy adapter-aware selection. For plain greedy, keep priority ordering and capability filtering, but choose the lowest communication penalty among capable agents when weight is non-zero; tie-break deterministically by original round-robin position and `agent.id`.

   Avoid using `ConnectivityContext` as a hard filter in M77; it can remain input evidence for later work. The M77 scope explicitly asks to use `AllocationAgent.comms_range`.

3. Thread `comms_penalty_weight` from simulation config into allocator construction.

   Inspect the allocator factory paths in `crates/swarm-sim/src/runner/*` and `crates/swarm-examples/src/strategy_comparison_runtime/*`. Wherever a run builds `GreedyAllocator` or `AuctionAllocator`, pass `RunConfig.comms_penalty_weight`.

   Required behavior:

   - Existing scenarios without the field produce identical allocation decisions.
   - M77 targeted profiles can set a non-zero weight.
   - CSV/JSON/report output records the effective value, either in scenario metadata or result manifest, so benchmark artifacts are interpretable.

4. Implement wildfire priority-triggered reallocation as an explicit event path.

   Files:

   - `crates/swarm-sim/src/runner/internal/wildfire.rs`
   - `crates/swarm-sim/src/runner/internal/tick_loop.rs`
   - `crates/swarm-replay/src/event_log.rs`

   Add an internal request type:

   ```rust
   pub(in crate::runner) struct PriorityReallocationRequest {
       pub task_id: TaskId,
       pub old_priority: u8,
       pub new_priority: u8,
       pub previous_agent_id: Option<AgentId>,
       pub tick: u64,
   }
   ```

   Extend `WildfireTickMetrics` with `priority_reallocation_requests: Vec<PriorityReallocationRequest>`.

   In `update_dynamic_threat`, after task priority is updated:

   ```rust
   if let Some(threshold) = wildfire_priority_realloc_threshold {
       if old_priority < threshold && task.priority >= threshold {
           metrics.priority_reallocation_requests.push(...);
           builder.push(Event::WildfirePriorityReallocationRequested { ... });
       }
   }
   ```

   The release/reassign rule should be deterministic:

   - threshold crossing releases the task from the current owner in every alive node registry;
   - release happens after `TaskPriorityUpdated` at tick `T`;
   - normal `process_alive_nodes` allocator path reassigns it at tick `T + 1`;
   - replay/report records `priority_update`, `priority_reallocation_requested`, `task_released`, and the later assignment separately.

   Do not mutate completed tasks. If a task is already completed or unmapped, emit no reallocation request.

5. Add SAR belief/entropy ordering behind `dynamic_belief_updates`.

   Files:

   - `crates/swarm-sim/src/runner/internal/sar.rs`
   - `crates/swarm-sim/src/runner/internal/tick_loop.rs`
   - `crates/swarm-runtime/src/grid_state.rs` only if a small helper is needed.

   Pass `config.dynamic_belief_updates` into `record_sar_scans`.

   After scan events update `GridState.belief_map`, re-rank unfinished SAR tasks when enabled:

   ```rust
   fn rerank_unfinished_sar_tasks_by_entropy<T: Transport>(
       nodes: &mut [(AgentNode<T>, AgentId)],
       grid_state: &GridState,
   ) {
       let Some(belief) = grid_state.belief_map.as_ref() else { return; };
       for (node, _) in nodes {
           for task in node.coordinator.registry.tasks_mut() {
               if !matches!(task.kind, Some(TaskKind::SarScan | TaskKind::SarConfirmationScan)) {
                   continue;
               }
               if task.completed {
                   continue;
               }
               let Some(cell) = task.grid_cell else { continue; };
               let entropy = belief.entropy(cell);
               task.priority = entropy_to_priority(entropy, task.id.as_ref());
           }
       }
   }
   ```

   Use stable deterministic mapping, for example entropy buckets `0..=10` with task-id tie-break handled by allocator ordering. Static behavior must remain unchanged when `dynamic_belief_updates == false`.

   Replay should continue to log `SarScan`/`SarDetection`; add a new event only if needed for analysis, e.g. `SarBeliefPriorityUpdated`, but keep it optional to avoid noisy logs. If a new event is added, document it in `docs/REPLAY.md`.

6. Add CBBA convergence diagnostics before changing CBBA behavior.

   Files:

   - `crates/swarm-replay/src/event_log.rs`
   - `crates/swarm-replay/src/replay/render.rs`
   - `crates/swarm-sim/src/runner/internal/tick_loop.rs`
   - `crates/swarm-runtime/src/node/runtime.rs` only if a per-node conflict count helper is cleaner.

   Extend `Event::CbbaBundleUpdated` with a backward-compatible field:

   ```rust
   CbbaBundleUpdated {
       agent_id: AgentId,
       bundle_size: usize,
       #[serde(default)]
       conflict_count: u64,
       tick: u64,
   }
   ```

   Populate it from the per-node `NodeTickOutput.conflicting_assignments` or a dedicated CBBA state helper if needed. Render it as `bundle_size=N conflict_count=M`.

   Run a focused heavy-loss/partition-prone diagnostic after implementation. Decision rule:

   - if replay shows CBBA convergence stalls correlate with conflicts persisting after failure recovery, add a bounded optional gossip burst setting, disabled by default;
   - if replay does not support that hypothesis, do not add the burst and instead document the limitation in `support_matrix`.

   If adding the burst, keep it explicit and bounded, for example `cbba_failure_gossip_burst_ticks: u64` default `0`, and trigger only after detected agent failure.

7. Add targeted M77 benchmark/delta mode.

   Files:

   - `crates/swarm-examples/src/strategy_comparison_runtime/cli.rs`
   - `crates/swarm-examples/src/strategy_comparison_runtime/missions.rs`
   - adjacent runtime/report code in `crates/swarm-examples/src/strategy_comparison_runtime/`
   - `docs/BENCHMARK_RESULTS.md`

   Prefer a reusable profile filter rather than a one-off hardcoded mode:

   ```text
   strategy_comparison --mission coverage --profiles heavy-loss,partition-prone --seeds 10 --jobs 4 ...
   strategy_comparison --mission wildfire --profiles m77-priority-realloc --seeds 10 --jobs 4 ...
   strategy_comparison --mission sar --profiles m77-dynamic-belief --seeds 10 --jobs 4 ...
   ```

   If the existing CLI shape makes `--profiles` too invasive, add `--m77-delta` as a small wrapper that expands to the same profile set.

   Targeted comparisons required for M77:

   - coverage heavy-loss/partition-prone with `comms_penalty_weight = 0.0` vs non-zero;
   - wildfire medium dynamic with threshold disabled vs enabled;
   - SAR small/medium with `dynamic_belief_updates = false` vs true;
   - CBBA heavy-loss diagnostic with replay enabled.

   Store artifacts under `results/m77_algorithm_delta/` with a README that says this is targeted evidence, not a full benchmark refresh.

8. Update replay compatibility and validation.

   Files:

   - `crates/swarm-replay/src/event_log.rs`
   - `crates/swarm-replay/src/replay/tests.rs`
   - `crates/swarm-replay/src/replay/summary.rs`
   - `docs/REPLAY.md`

   Add tests that old logs without `conflict_count` still deserialize with `0`. If adding `WildfirePriorityReallocationRequested` or `SarBeliefPriorityUpdated`, include parser/render/summary smoke tests.

9. Update docs and support matrix after code and targeted run.

   Files:

   - `README.md`: Strategy Support Matrix, milestone/status table, benchmark paragraph.
   - `docs/STATUS.md`: mark M77 status, limitations, targeted evidence path.
   - `docs/BENCHMARK_RESULTS.md`: add M77 targeted delta section with exact commands, seeds/jobs, commit hash, and interpretation.
   - `docs/SCENARIO_DSL.md`: document new `RunConfig` fields.
   - `docs/REPLAY.md`: document new replay field/event names.
   - `docs/EXTENSION_GUIDE.md`: note how mission authors can opt into M77 knobs.

   Wording rules:

   - no claim that M77 is publication-grade benchmark;
   - no claim that CBBA limitation is fixed unless the gossip-burst evidence and tests support it;
   - explain success metrics vs task completion if wildfire/SAR deltas change.

10. Commit only after code, tests, targeted run artifacts, and docs are aligned.

    Use a focused commit message, for example:

    ```text
    Implement M77 algorithm differentiation
    ```

## Testing strategy

Tests that need no refactoring and should be implemented with the main changes:

- `crates/swarm-alloc/src/allocator.rs`
  - `auction_comms_penalty_zero_preserves_assignment`
  - `auction_comms_penalty_prefers_in_range_agent`
  - `greedy_comms_penalty_zero_preserves_round_robin`
  - `greedy_comms_penalty_tie_break_is_deterministic`
- `crates/swarm-sim/src/runner/internal/wildfire.rs` or `crates/swarm-sim/src/runner/tests.rs`
  - `wildfire_priority_threshold_crossing_requests_reallocation`
  - `wildfire_priority_below_threshold_does_not_release_task`
  - `wildfire_priority_reallocation_ignores_completed_task`
- `crates/swarm-sim/src/runner/internal/sar.rs` or `crates/swarm-sim/src/runner/tests.rs`
  - `sar_dynamic_belief_updates_disabled_preserves_prior_order`
  - `sar_dynamic_belief_updates_rank_unfinished_tasks_by_entropy`
  - `sar_dynamic_belief_updates_skips_completed_tasks`
- `crates/swarm-replay/src/replay/tests.rs`
  - `cbba_bundle_updated_defaults_conflict_count_for_old_logs`
  - `cbba_bundle_updated_render_includes_conflict_count`
  - `wildfire_priority_reallocation_event_round_trips` if the new event is added.
- `crates/swarm-sim/src/support_matrix.rs`
  - test that M77-updated support classifications match documented CBBA outcome.

Tests that need light refactoring:

- Shared allocator fixture builders for agents/tasks with poses, battery, roles, and comms range.
- A small wildfire fixture helper that creates dynamic threat zones, assigned mapping tasks, and a threshold crossing at a deterministic tick.
- A SAR grid fixture helper with explicit posterior/entropy setup and deterministic scan RNG.
- A targeted benchmark-pack validation helper that checks result manifest identity, seed count, profile names, and replay presence.
- Shared docs/status assertion helper for required limitation phrases and artifact paths.

Tests that need heavy refactoring and should not block M77 unless they become necessary:

- Property tests over comms-aware allocation monotonicity across many topology shapes.
- Multi-seed statistical assertion suite for strategy deltas.
- Structured status manifest replacing duplicated Markdown claims.
- Full CBBA convergence model tests with partition/failure/gossip burst state machine.
- Large 1000-seed publication benchmark gate.

Required verification commands after implementation:

```bash
cargo fmt --all
timeout 300 cargo clippy --workspace --all-targets -- -D warnings
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-alloc comms_penalty
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim wildfire_priority
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim sar_dynamic_belief
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay cbba_bundle_updated
timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim support_matrix
```

Targeted run commands after tests pass:

```bash
cargo build --release
timeout 300 /home/formi/.local/bin/runlim cargo run --release -p swarm-examples --bin strategy_comparison -- --mission coverage --profiles heavy-loss,partition-prone --seeds 10 --jobs 4 --output-dir results/m77_algorithm_delta/coverage --run-id-prefix m77-coverage
timeout 300 /home/formi/.local/bin/runlim cargo run --release -p swarm-examples --bin strategy_comparison -- --mission wildfire --profiles m77-priority-realloc --seeds 10 --jobs 4 --output-dir results/m77_algorithm_delta/wildfire --run-id-prefix m77-wildfire
timeout 300 /home/formi/.local/bin/runlim cargo run --release -p swarm-examples --bin strategy_comparison -- --mission sar --profiles m77-dynamic-belief --seeds 10 --jobs 4 --output-dir results/m77_algorithm_delta/sar --run-id-prefix m77-sar
timeout 300 /home/formi/.local/bin/runlim cargo run --release -p swarm-examples --bin strategy_comparison -- --mission coverage --profiles m77-cbba-heavy-loss --seeds 10 --jobs 4 --output-dir results/m77_algorithm_delta/cbba --run-id-prefix m77-cbba
```

If a targeted run exceeds 5 minutes, stop it, keep the failed/partial run log only if useful, lower seed count for local smoke evidence, and document the skipped longer run explicitly in `docs/BENCHMARK_RESULTS.md` and the final report.

## Risks and tradeoffs

- Converting `GreedyAllocator` from unit struct to configured struct can break direct unit-struct construction. Mitigation: update all call sites in one pass and preserve `Default`.
- A non-zero comms penalty may look like a behavior regression in old comparisons. Mitigation: default remains `0.0`; docs and result manifests must record the effective value.
- Wildfire force reallocation can introduce assignment churn if threshold is too low. Mitigation: default `None`, deterministic threshold crossing semantics, and no release for completed tasks.
- SAR entropy ordering mutates task priorities and can change benchmark timing. Mitigation: default `false`, stable entropy buckets, task-id tie-breaks, and targeted tests.
- CBBA gossip burst can mask a limitation instead of explaining it. Mitigation: add burst only if replay diagnostics show persistent conflicts after failure; otherwise document the limitation.
- Replay schema changes can break old artifacts if fields are not backward-compatible. Mitigation: use `#[serde(default)]` for added fields and add old-log deserialization tests.
- Targeted M77 runs are useful engineering evidence but not a publication-grade benchmark. Docs must say that directly.

## Open questions

- Exact non-zero `comms_penalty_weight` for targeted M77 evidence: start with a value strong enough to change heavy-loss/partition-prone allocation, then record it in the result README.
- Wildfire threshold value for fixtures: use `Some(8)` unless existing medium-dynamic profiles have a better natural crossing point.
- Whether CBBA needs a bounded gossip burst or only a documented limitation. This must be decided from the new conflict-count replay diagnostic, not from intuition.
- Whether SAR entropy ordering should update `Task.priority` directly or use a separate ordering hook in allocation. Direct priority mutation is simpler and visible in reports; a separate hook is cleaner but larger.
