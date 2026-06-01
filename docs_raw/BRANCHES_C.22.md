# BRANCHES_C.22 - Актуальные ветки развития после M69

Дата фиксации: 2026-05-31

Основа: `docs_raw/BRANCHES.md`, `DRONE_A/B/C.16-21.md`, текущий локальный
код и обсуждение после аудита `DRONE_C.21.md`.

Этот документ фиксирует не линейный milestone-plan, а именно ветки развития:
крупные направления, между которыми можно выбирать после текущего Urban/M69
ствола. Уже закрытые работы не перечисляются как самостоятельные ветки.

Текущий ствол:

```text
M63 Evidence Cleanup
M64 Urban Foundations
M65 Urban Patrol v0
M66 Urban Search v1
M67 Urban Replay / Analysis + Multi-Agent Prep
M68 Algorithm Depth On Urban + Existing Missions
M69 Benchmark Refresh / Research Evidence
M70 SITL Export And Platform Boundary Decision
```

M70 должен выбрать следующий основной путь. Не все ветки ниже равноправны:
часть является primary candidate, часть лучше держать как supporting track.

---

## Что уже не является открытой веткой

Эти направления уже закрыты как базовый scope или включены в текущий ствол:

- basic Evidence / Cleanup;
- basic Real SITL / PX4 foundation;
- single-agent PX4/SIH execute;
- multi-agent PX4/SIH execute;
- controlled PX4/SIH failure and replacement evidence;
- M59 replacement replay seq fix;
- basic Urban road graph foundation;
- Urban Patrol v0;
- Urban Search v1;
- basic Urban replay/timeline/analysis;
- one narrow Urban corridor-aware planner delta;
- current built-in 1000-seed M69 benchmark pack.

Их можно углублять, но не стоит считать "начать с нуля" отдельной развилкой.

---

## Короткая карта выбора

### Primary branches

1. Urban route export to SITL/PX4.
2. Urban avoidance / multi-agent deconfliction / richer geometry.
3. Algorithm Depth.
4. Research Benchmark / Publication Evidence.
5. Platform / API Packaging.
6. New Mission: Logistics / Delivery.
7. New Mission: Multi-target Pursuit.

### Supporting branches

8. PX4/SITL Hardening.
9. Replay / Analysis.
10. Scenario Generation / Synthetic Testbed.
11. Realism v2 / Simulation Fidelity.
12. Disaster Mapping / Flood / Wildfire v2.

### Deferred / not now

- real hardware / HIL;
- production-grade safety;
- certified obstacle avoidance;
- real lidar / SLAM / CV;
- distributed onboard autonomy;
- visual UI as the main milestone.

---

## Branch 1 - Urban route export to SITL/PX4

**Status:** актуальная M70 primary branch.

**Core idea:** взять уже существующий Urban route и экспортировать его в waypoint
mission для local PX4/SIH. Это связывает новую Urban mission family с уже
созданным PX4/SIH workflow, не обещая hardware readiness.

**Scope:**

1. Convert Urban planned route segments to waypoint mission items.
2. Reuse existing pre-upload safety validation.
3. Run local PX4/SIH upload or execute if practical.
4. Capture artifacts with existing `--output-dir` / `--run-id` discipline.
5. Document that this is local PX4/SIH evidence, not hardware or real obstacle
   avoidance.

**Why it matters:**

- Gives Urban work a path into the SITL evidence layer.
- Tests whether mission-level route planning can become an executable waypoint
  plan.
- Uses existing project strengths instead of creating a new physics stack.

**Non-goals:**

- no hardware;
- no Gazebo gate by default;
- no real obstacle avoidance claim;
- no low-level flight control.

**Suggested first milestone:**

Urban route to waypoint conversion + dry-run validation + one upload-only local
PX4/SIH artifact if available.

---

## Branch 2 - Urban avoidance / multi-agent deconfliction / richer geometry

**Status:** актуальная primary branch, but should start narrowly.

**Core idea:** продолжить Urban после v0/v1: добавить mission-level decisions
around blocked routes, temporary obstacles and multiple agents sharing the same
map.

This is not certified collision avoidance. It is deterministic simulation and
decision logic above the autopilot layer.

**Possible work:**

1. Temporary obstacles on graph edges or road segments.
2. Mock obstacle detector as a geometry query, not real lidar.
3. Stop / wait / replan / yield policy.
4. Multi-agent route conflict representation.
5. Separation enforcement at mission/judge level.
6. Metrics:
   - `replan_count`;
   - `avoided_collision_count`;
   - `near_miss_count`;
   - `wait_time`;
   - `replan_success_rate`;
   - `route_conflict_count`.

**Later geometry work:**

- polygon boundaries;
- allowed/forbidden corridors;
- point-in-polygon and segment-vs-polygon tests;
- raycast/lidar-like simulation only after graph-based decisions are stable.

**Why it matters:**

- Adds reactive decision-making to Urban missions.
- Makes the "облети квартал" family closer to practical mission logic.
- Provides better pressure for algorithms and benchmarks.

**Risks:**

- can drift into geometry engine work;
- can be mistaken for physical safety;
- multi-agent deconfliction can expand quickly.

**Suggested first milestone:**

Dynamic blocked edge on road graph + deterministic replan/wait policy + replay
events and metrics.

---

## Branch 3 - Algorithm Depth

**Status:** актуальная primary branch. M68 only covered one narrow Urban planner
delta.

**Core idea:** improve strategies and planners so benchmark comparisons become
meaningfully different.

**Workstreams:**

### 3A - Communication-aware allocation

Current gap: `comms_range` exists, but most allocators do not use it in scoring.

Possible work:

1. Add communication penalty or `message_budget`.
2. Penalize assignments that move agents outside reliable communication range.
3. Compare greedy, auction, CBBA and connectivity-aware under loss/partition
   profiles.
4. Report success vs messages vs availability tradeoff.

### 3B - Mission-specific planners

Possible work:

- SAR: prioritize high-information cells and dynamic belief entropy.
- Wildfire: priority-triggered reallocation after threat updates.
- Inspection: route optimization for non-centralized strategies.
- Urban: replan-aware route scoring and deconfliction costs.

### 3C - CBBA convergence and support matrix

Possible work:

1. Separate unsupported-by-design from bug/regression.
2. Add replay-driven diagnostics for delayed reconvergence.
3. Experiment with gossip interval and failure-triggered gossip burst.
4. Re-benchmark CBBA after targeted changes.

### 3D - Scale beyond small swarms

Possible work:

- 8-agent and 16-agent scenario profiles;
- message-count scaling curves;
- hierarchical coordination only if benchmark shows need.

**Suggested first milestone:**

Communication-aware scoring or one mission-specific planner, with a small
before/after benchmark delta and support-matrix update.

---

## Branch 4 - Research Benchmark / Publication Evidence

**Status:** актуальная primary branch, but should follow stronger behavior.

M69 produced a current 1000-seed release benchmark for the built-in simulation
suite. That is valuable, but it is not the full research/publication branch.

**Remaining work:**

1. Confidence intervals for key metrics.
2. Degradation curves:
   - packet loss;
   - latency;
   - number of agents;
   - map size;
   - task density;
   - failure count;
   - Urban obstacle density;
   - bus detection probability.
3. Urban-inclusive benchmark entrypoint or an explicit Urban benchmark suite.
4. Strategy comparison report:
   - where greedy is enough;
   - where CBBA wins;
   - where centralized is an oracle;
   - where pairs are unsupported.
5. Interpretation of M69 weak rows:
   - SAR success;
   - wildfire success;
   - emergency-mesh;
   - CBBA under heavy-loss/high-latency.
6. Reproducible benchmark pack validation.

**Why it matters:**

- Turns simulation results into defensible evidence.
- Forces support matrix honesty.
- Makes algorithm changes measurable.

**Risks:**

- long runs can produce large tables without better understanding;
- benchmark before algorithm/mission improvements may become stale quickly.

**Suggested first milestone:**

Benchmark interpretation pass + confidence interval helper + Urban suite
decision, before another long run.

---

## Branch 5 - Platform / API Packaging

**Status:** актуальная primary branch after at least one real mission family.

**Core idea:** turn the stable-ish extension guide into a stricter project
boundary for external-style mission, strategy and metric work.

**Possible work:**

1. External-style mission example.
2. Crate boundary review.
3. Schema compatibility tests for scenario and replay formats.
4. Public/internal API distinction.
5. Semver/publishing checklist only if explicitly chosen.
6. Machine-readable changelog or schema manifest if publication is planned.

**Why it matters:**

- Makes the project easier to extend.
- Reduces coupling between crates.
- Validates that new missions can be added without broad core churn.

**Risks:**

- premature API stabilization can freeze poor abstractions;
- lower immediate research value than Urban/Algorithm branches.

**Suggested first milestone:**

External-style example using existing Urban or a small toy mission, plus schema
compatibility tests. No public semver promise yet.

---

## Branch 6 - New Mission: Logistics / Delivery

**Status:** актуальная primary branch, deferred until Urban decision unless
task dependencies become the priority.

**Core idea:** introduce pickup/dropoff and precedence constraints. This tests
stateful task dependencies, capacity and deadlines, which current missions do
not cover.

**Possible domain model:**

```rust
TaskKind::Pickup { item_id, location }
TaskKind::Dropoff { item_id, location, requires_pickup }
AgentState::cargo
RunState::delivered_items
```

**Possible metrics:**

- `delivery_rate`;
- `late_deliveries`;
- `capacity_violations`;
- `precedence_violations`;
- `unserved_deliveries`;
- `total_route_cost`.

**Why it matters:**

- Stress-tests DSL and allocator semantics.
- Adds task dependencies absent from coverage/SAR/inspection/wildfire/Urban.
- Useful for platform/API validation.

**Risks:**

- can become a VRP/scheduling project;
- less tied to physical movement than Urban.

**Suggested first milestone:**

Small pickup/dropoff scenario with one precedence rule, one capacity limit,
portable regression smoke and explicit support matrix.

---

## Branch 7 - New Mission: Multi-target Pursuit

**Status:** актуальная primary branch, deferred until replay/trace tooling is
strong enough for dynamic targets.

**Core idea:** moving targets, intercept/escort behavior and dynamic task
appearance. This stresses reactive allocation.

**Possible domain model:**

```rust
TaskKind::Pursuit { target_id, mode, proximity_radius }
PursuitTarget { id, trajectory, speed }
RunState::active_targets
RunState::captured_targets
```

**Possible metrics:**

- `capture_rate`;
- `time_to_intercept`;
- `targets_lost`;
- `total_pursuit_distance`;
- `interception_efficiency`.

**Why it matters:**

- Tests reactive planning and time-dependent scoring.
- Creates a strong stress case for auction/CBBA.
- Adds dynamics different from static point/zone/edge missions.

**Risks:**

- moving target semantics can become ambiguous;
- without good replay it is hard to debug;
- may become a toy chase model unless scoped carefully.

**Suggested first milestone:**

One deterministic target trajectory, one capture radius, one small scenario,
route trace and replay events from day one.

---

## Branch 8 - PX4/SITL Hardening

**Status:** supporting branch. Use when live workflow reliability is the main
problem, not as the default next feature branch.

**Core idea:** deepen the local PX4/SIH workflow after M58/M59/M60/M69.

**Possible work:**

1. Broader failure matrix:
   - fail before upload;
   - fail after upload before start;
   - fail during mission;
   - fail after completing one task;
   - survivor failure after replacement.
2. Repeated failure recovery.
3. Local launch harness for multiple PX4/SIH instances.
4. Artifact validator for run report, event log, manifest and replay summary.
5. Telemetry robustness:
   - no-progress timeout tuning;
   - heartbeat disconnect classification;
   - mission-current/reached event correlation.

**Why it matters:**

- Makes PX4/SIH evidence repeatable.
- Reduces manual artifact risk.
- Supports Urban route export if that branch is chosen.

**Risks:**

- slow and machine-dependent;
- can absorb time without improving mission intelligence.

**Suggested first milestone:**

Artifact validator + one additional fake-controller failure timing test. Local
PX4 harness can follow if M70 chooses SITL.

---

## Branch 9 - Replay / Analysis

**Status:** supporting branch. Basic timeline/Urban analysis exists; richer
analysis should be tied to a mission or algorithm change.

**Core idea:** make behavior inspectable without building a visual UI.

**Possible work:**

1. Route traces for more mission families.
2. Per-agent pose/task assignment timelines.
3. Textual map diagnostics:
   - route segments;
   - violation locations;
   - blocked edges;
   - detection events.
4. CSV export for per-tick analysis.
5. Cross-run diff tooling.
6. Mission-specific summaries:
   - wildfire hazard progression;
   - SAR belief entropy;
   - inspection edge coverage;
   - SITL mission replacement timeline.

**Why it matters:**

- Helps debug Urban, Algorithm Depth, SITL and Benchmark work.
- Avoids premature UI work.
- Improves reviewability of long artifacts.

**Risks:**

- tooling-only work can drift unless attached to real behavior.

**Suggested first milestone:**

Add the minimum replay/analysis output required by the chosen primary branch.

---

## Branch 10 - Scenario Generation / Synthetic Testbed

**Status:** new supporting branch added in C.22.

**Core idea:** build deterministic generators for scenario families, so future
algorithm and benchmark claims do not rely only on a few hand-written fixtures.

This is not a new mission. It is testbed infrastructure for Urban, Algorithm
Depth and Benchmark branches.

**Possible work:**

1. Seeded Urban road graph generator:
   - grid blocks;
   - corridor widths;
   - static obstacle density;
   - blocked edges;
   - bus placement.
2. Failure/fault scenario generator:
   - agent loss timing;
   - no-progress windows;
   - partial completion before failure.
3. Communication profile generator:
   - packet loss;
   - latency;
   - partitions;
   - agent count.
4. Benchmark manifest records generator seed and parameters.
5. Property/fuzz-style tests for route planning, judging and replay artifact
   consistency.

**Why it matters:**

- Makes benchmark/degradation curves easier to produce.
- Reduces overfitting to one synthetic scenario.
- Gives Algorithm Depth stronger evidence.

**Risks:**

- generators can become another project;
- random scenario families need strict reproducibility and small CI-safe tests.

**Suggested first milestone:**

Seeded Urban corridor/obstacle generator used only in tests and one small
benchmark smoke. Keep generated fixtures deterministic and portable.

---

## Branch 11 - Realism v2 / Simulation Fidelity

**Status:** supporting branch. Foundation exists, but measured realism effects
are incomplete.

**Core idea:** make realism profiles measurable and documented, rather than a
collection of knobs.

**Possible work:**

1. Define expected effects for light/medium/heavy profiles.
2. Comparative benchmark: ideal vs realism profiles.
3. Realism metadata in manifests.
4. Stable realism smoke in regression; unstable suites stay experimental.
5. Docs explaining what is modeled and what is not.

**Why it matters:**

- Helps interpret benchmark results.
- Makes simulation claims more precise.
- Can support publication branch.

**Risks:**

- realism without SITL/physical calibration can be misleading;
- can distract from mission-level behavior.

**Suggested first milestone:**

One comparative smoke for a stable mission and updated docs describing expected
metric changes.

---

## Branch 12 - Disaster Mapping / Flood / Wildfire v2

**Status:** low-priority supporting branch unless disaster mapping becomes the
main product/research direction.

M63 already moved flood to future work and clarified wildfire success semantics.
The remaining branch is optional.

**Possible work:**

1. Minimal flood mission:
   - `FloodConfig`;
   - flooded zones;
   - water spread;
   - critical zones;
   - flood replay events;
   - experimental smoke.
2. Wildfire priority-triggered reallocation:
   - reassign when dynamic threat changes priority enough;
   - measure time to map high-priority zones.
3. Stronger wildfire benchmark interpretation.

**Why it matters:**

- Keeps the old disaster mapping promise alive if needed.
- Adds another domain with dynamic priority.

**Risks:**

- flood can become a new mission family with low connection to the current
  Urban/PX4 direction;
- wildfire priority knobs need benchmark evidence to be useful.

**Suggested first milestone:**

Do not start with flood. If this branch is chosen, first implement
priority-triggered reallocation for existing wildfire.

---

## Recommended choice points

### If the next goal is physical-adjacent evidence

Choose:

```text
Branch 1 Urban route export to SITL/PX4
  + Branch 8 PX4/SITL Hardening as support
  + Branch 9 Replay / Analysis as support
```

### If the next goal is richer mission decision logic

Choose:

```text
Branch 2 Urban avoidance / deconfliction
  + Branch 10 Scenario Generation / Synthetic Testbed as support
  + Branch 9 Replay / Analysis as support
```

### If the next goal is stronger algorithms

Choose:

```text
Branch 3 Algorithm Depth
  + Branch 10 Scenario Generation / Synthetic Testbed as support
  + Branch 4 Research Benchmark after measurable deltas exist
```

### If the next goal is external reuse

Choose:

```text
Branch 5 Platform / API Packaging
  + Branch 6 Logistics / Delivery if task dependencies are desired
  + Branch 9 Replay / Analysis for extension diagnostics
```

### If the next goal is a publication-like artifact

Choose:

```text
Branch 4 Research Benchmark / Publication Evidence
  after at least one of Branch 2 or Branch 3 creates new behavior
```

---

## Current recommendation

Do not add more peer branches now. The set above is already broad enough.

Best next primary choices after M69:

1. **Urban route export to SITL/PX4** if M70 should connect Urban to the existing
   local PX4/SIH evidence layer.
2. **Urban avoidance / deconfliction** if the project should keep deepening
   mission-level realism in pure simulation.
3. **Algorithm Depth** if the priority is measurable strategy improvement.
4. **Platform/API Packaging** if the priority is external reuse.

The new C.22 addition is **Scenario Generation / Synthetic Testbed**, but it
should remain supporting infrastructure, not the main branch by itself.
