# FOLLOW_UP_C.22 - что полезно взять из DRONE_A.21 и DRONE_B.21

Дата фиксации: 2026-05-31

Основа: `docs_raw/DRONE_A.21.md`, `docs_raw/DRONE_B.21.md`, текущий локальный
код, README/docs, committed artifacts после M69 и актуальная карта веток
`docs_raw/BRANCHES_C.22.md`.

Этот документ не является новым линейным планом. Это список полезных идей из
A.21/B.21, которые не полностью вошли в текущий C.21-ствол или были сознательно
сужены. Их стоит сохранить как backlog для следующих веток.

---

## Короткий вывод

Базовый Urban/M69 ствол закрыт в pragmatic scope:

- Evidence cleanup/status honesty;
- Urban road graph foundation;
- Urban Patrol v0;
- Urban Search v1 with static mocked bus target;
- Urban replay/analysis diagnostics;
- narrow `corridor-aware` route-risk delta;
- current 1000-seed built-in benchmark artifact.

Из A.21/B.21 полезно дальше взять не новый полный roadmap, а пять крупных
блоков:

1. Urban route export to SITL/PX4.
2. Urban v2: blocked edges, replan/wait/yield, multi-agent deconfliction.
3. Algorithm Depth: comms scoring, wildfire priority reallocation, SAR entropy,
   CBBA diagnostics.
4. Research Benchmark statistical layer: confidence intervals, degradation
   curves, Urban benchmark decision.
5. PX4/SITL local harness and artifact validation.

Дополнительно можно сохранить dynamic bus route, Platform/API hardening,
Logistics/Delivery and Multi-target Pursuit as later branches.

---

## Уже закрыто или поглощено текущим стволом

Не нужно планировать заново:

- M63 evidence cleanup;
- flood wording cleanup and future-work boundary;
- wildfire success/completion semantics;
- M58/M59 replay artifact sanity;
- M59 replacement completion seq sanity;
- replay timeline basics: `--timeline`, `--agent`, `--category urban`;
- Urban map model: road graph, AABB static obstacles, route loop;
- deterministic Urban route planning;
- Urban static route judge;
- Urban Patrol v0;
- Urban Search v1 with static bus target and mocked detector;
- Urban route trace and judge-report artifacts;
- two-agent Urban diagnostic fixture;
- route-risk metric and corridor-aware planner delta;
- M69 built-in benchmark pack.

These items can still be improved, but they are no longer open "missing
milestones" from A.21/B.21.

---

## Follow-up 1 - Urban route export to SITL/PX4

**Source:** A.21 M70, B.21 M64 optional PX4 export path, C.21 M70.

**What remains:**

- Convert an Urban planned route into ordered SITL/PX4 waypoint items.
- Reuse existing safety validation.
- Add a dry-run/export fixture or command path.
- Optionally capture local PX4/SIH upload or execute evidence.
- Document scope as local PX4/SIH only, not hardware readiness and not real
  obstacle avoidance.

**Why useful:**

This is the cleanest bridge between the new Urban simulation work and the
existing PX4/SIH supervisor workflow. It makes Urban routes executable as
waypoint missions without changing the project's architectural boundary.

**Minimum useful scope:**

1. Unit test: Urban route segments -> deterministic waypoint list.
2. Dry-run artifact or fixture for exported Urban route.
3. Safety pre-upload validation reused from existing SITL path.
4. Optional manual local PX4/SIH artifact if environment is available.

**Branch fit:** primary branch `Urban route export to SITL/PX4`; support from
`PX4/SITL Hardening`.

---

## Follow-up 2 - Urban v2: blocked edges, replan/wait/yield, deconfliction

**Source:** A.21 M67, B.21 M68 Option A.

**What remains:**

- Temporary blocked edges/nodes with appearance and disappearance ticks.
- Mock obstacle detector as deterministic graph/range query.
- Policy layer:
  - wait;
  - replan around blocked edge;
  - yield to another agent;
  - abort/reassign if blocked too long.
- Multi-agent route ownership.
- Duplicate segment ownership prevention/detection.
- Active separation/deconfliction policy, not only diagnostic measurement.
- Metrics:
  - `replan_count`;
  - `replan_success_rate`;
  - `wait_time_ticks`;
  - `near_miss_count`;
  - `avoided_collision_count`;
  - `duplicate_ownership_count`.

**Current state:**

The current code has route/judge/replay diagnostics, static blocked-edge route
validation, a two-agent analysis fixture and separation/conflict metrics. It
does not implement active avoidance, deconfliction or replan policy.

**Why useful:**

This turns Urban from "planned route following" into mission-level decision
logic. It is the most direct continuation of the user's original "облети
квартал / не столкнись / реагируй на obstacle" idea while staying above PX4.

**Minimum useful scope:**

1. One deterministic blocked edge appears before the drone reaches it.
2. Policy chooses wait or replan.
3. Replay records the decision.
4. Metrics distinguish planned route, blocked route and recovered route.

**Branch fit:** primary branch `Urban avoidance / multi-agent deconfliction`.

---

## Follow-up 3 - Dynamic bus route for Urban Search

**Source:** B.21 M65.

**What remains:**

- Bus route over the Urban road graph.
- `pose_at_tick` based on route schedule.
- Appearance/disappearance over time.
- Detector samples current bus pose, not only static pose.
- Replay/report makes observed bus pose inspectable.

**Current state:**

Urban Search v1 has a static bus `pose` plus optional active tick range and a
deterministic mocked detector. This is enough for v1, but not the full B.21
dynamic-bus shape.

**Why useful:**

It is a small, natural extension of "облетай квартал пока не встретишь автобус"
and a gentle bridge toward later dynamic target missions.

**Minimum useful scope:**

1. Route-scheduled bus with linear interpolation between graph nodes.
2. Detector uses bus pose for current tick.
3. Deterministic scenario where detection tick is predictable.
4. Replay includes observed moving bus pose.

**Branch fit:** Urban v2, or later `Multi-target Pursuit` preparation.

---

## Follow-up 4 - Communication-aware allocation scoring

**Source:** A.21 M68, B.21 M66.

**What remains:**

- Make `comms_range` affect scoring for relevant allocators.
- Add `comms_penalty_weight` or a message-budget equivalent.
- Preserve old behavior with zero/default penalty.
- Compare behavior under packet-loss and partition-prone profiles.

**Current state:**

`ConnectivityAwareAllocator` exists and uses connectivity concepts for relay
placement. Broad task scoring in greedy/auction/CBBA/centralized still does not
make communication range a first-class cost.

**Why useful:**

It is the clearest Algorithm Depth item from B.21: easy to explain, easy to
test, and likely to make strategies differ under degraded communication.

**Minimum useful scope:**

1. Unit-level scoring test for in-range vs out-of-range assignment.
2. Configurable penalty weight.
3. Small comparative run showing success/messages/availability tradeoff.

**Branch fit:** primary branch `Algorithm Depth`.

---

## Follow-up 5 - Wildfire priority-triggered reallocation

**Source:** A.21 M68, B.21 M66, earlier Disaster Mapping v2 notes.

**What remains:**

- Dynamic wildfire priority update should trigger reallocation when a zone
  becomes critical.
- High-priority task should be able to preempt lower-value work under an
  explicit policy.
- Replay/report should distinguish priority-driven reallocation from ordinary
  assignment.

**Current state:**

Wildfire priority fields exist and priority affects scoring when allocation
occurs. But a priority update itself does not force already assigned agents to
reconsider their assignments.

**Why useful:**

This is a compact mission-specific algorithm improvement. It strengthens
Disaster Mapping without starting a whole minimal flood mission.

**Minimum useful scope:**

1. Controlled fixture: low-priority task assigned, later critical zone appears.
2. Priority update emits a reallocation trigger.
3. Assignment changes deterministically.
4. Replay/report explains the trigger.

**Branch fit:** `Algorithm Depth`, optionally `Disaster Mapping / Wildfire v2`.

---

## Follow-up 6 - SAR belief/entropy ordering and success interpretation

**Source:** B.21 M66/M67.

**What remains:**

- Dynamic belief updates after scan events.
- Task ordering based on remaining uncertainty/entropy.
- Optional `dynamic_belief_updates`-style flag to preserve old deterministic
  behavior.
- Possible `pod_success_threshold` or explicit SAR probability-of-detection
  success semantics.

**Current state:**

SAR still uses strict `all_targets_found()` style success semantics in the
runner. M69 documents weak SAR success, but no relaxed threshold or dynamic
belief ordering is implemented.

**Why useful:**

SAR benchmark rows are hard to interpret. Entropy-aware planning and clearer PoD
success semantics would make SAR a better research case.

**Minimum useful scope:**

1. Small belief-grid fixture.
2. Scan event changes posterior.
3. Ordering changes only with dynamic mode enabled.
4. Docs explain SAR success vs probability-of-detection.

**Branch fit:** `Algorithm Depth` and `Research Benchmark / Publication`.

---

## Follow-up 7 - CBBA convergence diagnostics

**Source:** A.21 M68, B.21 M66.

**What remains:**

- Diagnostic event/counter for CBBA convergence state.
- Replay-driven explanation for heavy-loss/high-latency weak rows.
- Experiment with gossip interval or failure-triggered gossip burst only if
  evidence supports it.
- Support matrix distinction:
  - unsupported by design;
  - parameter issue;
  - fixable bug/regression.

**Current state:**

The support matrix marks some CBBA/SAR combinations as unsupported or weak, but
there is no detailed convergence timeline explaining exactly why.

**Why useful:**

It turns "CBBA failed here" into a useful technical claim. This is important
before publication-like benchmark interpretation.

**Minimum useful scope:**

1. Add a small diagnostic event or counter in controlled mode.
2. Run one heavy-loss profile with replay enabled.
3. Document whether the failure is reconvergence, stale ownership or success
   predicate mismatch.

**Branch fit:** `Algorithm Depth`, `Replay / Analysis`, `Research Benchmark`.

---

## Follow-up 8 - Benchmark statistical layer and degradation curves

**Source:** A.21 M69, B.21 M67.

**What remains:**

- `mean +/- stderr` or confidence intervals for key metrics.
- Degradation curves:
  - packet loss;
  - latency;
  - agent count;
  - map/task size;
  - Urban obstacle density;
  - bus detection probability.
- Benchmark pack validator.
- Urban inclusion in benchmark entrypoint, or explicit separate Urban benchmark
  decision.
- Stronger interpretation of SAR/wildfire/emergency-mesh/CBBA weak rows.

**Current state:**

M69 provides a 1000-seed release artifact for built-in `--mission all`, but
Urban is still separate M68 evidence and statistical summaries/degradation
curves are not implemented.

**Why useful:**

This is the difference between "large validation run exists" and
"publication-like research evidence".

**Minimum useful scope:**

1. Confidence/stderr helper over existing per-run aggregates.
2. Report/export test for new statistical fields.
3. One small degradation sweep before another long run.
4. Decide whether Urban joins `--mission all` or remains separate.

**Branch fit:** `Research Benchmark / Publication Evidence`.

---

## Follow-up 9 - PX4/SITL local harness scripts and artifact validator

**Source:** B.21 M63, A.21 M70.

**What remains:**

- `scripts/run_m58_local.sh`.
- `scripts/run_m59_local.sh`.
- Optional Urban export run harness later.
- Artifact validator for:
  - manifest;
  - run report;
  - event log;
  - replay summary;
  - output-dir/run-id consistency.

**Current state:**

M58/M59 artifacts exist, and `sitl_supervisor` has output directory discipline.
There is no committed local script harness and no standalone artifact validator.

**Why useful:**

It improves reproducibility of local PX4/SIH evidence without making PX4 part
of default CI.

**Minimum useful scope:**

1. Manual-only script skeleton with explicit assumptions.
2. Start/wait/run/cleanup behavior.
3. Output directory discipline.
4. Small validator over committed lightweight fixture.

**Branch fit:** `PX4/SITL Hardening`; support for `Urban route export to
SITL/PX4`.

---

## Follow-up 10 - Platform/API boundary hardening

**Source:** A.21 M70, B.21 M68, earlier extension-guide plans.

**What remains:**

- External-style mission example.
- Scenario/replay/report schema compatibility tests.
- Crate boundary review.
- Harder extension checklist backed by a real mission.
- No public semver promise unless explicitly chosen.

**Current state:**

`docs/EXTENSION_GUIDE.md` exists and Urban gives a real mission family, but the
project still does not promise external semver-stable APIs.

**Why useful:**

After Urban, extension work can be validated against a real mission rather than
only test-only fixtures.

**Minimum useful scope:**

1. External-style example using existing Urban or a tiny example mission.
2. Schema compatibility smoke tests.
3. Explicit public/internal boundary doc.

**Branch fit:** `Platform / API Packaging`.

---

## Follow-up 11 - Logistics / Delivery

**Source:** A.21 M70, B.21 M68 Option B, older New Mission branch notes.

**What remains:**

- `TaskKind::Pickup` / `TaskKind::Dropoff`.
- `requires_pickup` / precedence validation.
- Agent cargo capacity.
- Optional deadlines/time windows.
- Metrics:
  - `delivery_rate`;
  - `late_deliveries`;
  - `capacity_violations`;
  - `precedence_violations`;
  - `unserved_deliveries`.

**Why useful:**

This is the best candidate if the next goal is stateful task dependencies and
DSL/allocator generality.

**Minimum useful scope:**

One pickup/dropoff pair, one capacity limit, deterministic completion semantics,
portable regression smoke and support matrix entry.

**Branch fit:** `New Mission: Logistics / Delivery`.

---

## Follow-up 12 - Multi-target Pursuit

**Source:** A.21 M70, B.21 older New Mission branch notes.

**What remains:**

- Moving target trajectories.
- Intercept/escort mode.
- Capture radius.
- Dynamic target appearance/disappearance.
- Metrics:
  - `capture_rate`;
  - `time_to_intercept`;
  - `targets_lost`;
  - `pursuit_distance`.

**Why useful:**

This stresses reactive task allocation and time-dependent scoring more than
static waypoint/zone missions.

**Minimum useful scope:**

One deterministic target trajectory, one capture radius, one small scenario,
replay events from the start.

**Branch fit:** `New Mission: Multi-target Pursuit`.

---

## What not to take now

These A.21/B.21 ideas should stay deferred unless explicitly chosen:

- full polygon/navmesh engine before graph-based Urban v2 proves need;
- real lidar/raycast/SLAM/CV;
- hardware/HIL;
- production safety or certified collision avoidance;
- hierarchical coordination before 8/16-agent scaling evidence;
- public semver-stable API before extension boundary hardening;
- 1000-seed reruns before interpretation/statistical layer or new behavior
  justifies the runtime.

---

## Practical priority

If choosing by value-to-effort and current project direction:

1. Urban route export to SITL/PX4.
2. PX4/SITL local harness scripts and artifact validator.
3. Urban v2 blocked-edge wait/replan policy.
4. Communication-aware allocation scoring.
5. Benchmark statistical layer and Urban benchmark decision.
6. CBBA convergence diagnostics.
7. Wildfire priority-triggered reallocation.
8. Dynamic bus route.
9. SAR belief/entropy ordering and success interpretation.
10. Platform/API boundary hardening.
11. Logistics / Delivery.
12. Multi-target Pursuit.

This priority is not a commitment. It is a pragmatic extraction of useful
unimplemented work from A.21/B.21 after the current C.21/M69 state.
