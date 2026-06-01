# FOLLOW_UP_A.22 - useful follow-up items from DRONE_A.21 and DRONE_B.21

Дата фиксации: 2026-05-31

Основа: сравнение `docs_raw/DRONE_A.21.md`, `docs_raw/DRONE_B.21.md`,
текущего локального кода, README/docs и committed result artifacts после M69.

Этот документ не является новым линейным roadmap. Это список полезных
follow-up работ: остаточных, добивочных или углубляющих пунктов, которые не
надо считать провалом текущего ствола M63-M69, но стоит сохранить как хороший
backlog для следующих веток.

## Короткий статус

Базовый ствол M63-M69 закрыт в текущем pragmatic scope:

- M63 evidence cleanup/status honesty;
- M64 Urban Foundations;
- M65 Urban Patrol v0;
- M66 Urban Search v1;
- M67 Urban replay/analysis diagnostics;
- M68 narrow Urban corridor-aware planner delta;
- M69 1000-seed release benchmark for built-in `--mission all` suite.

Но A.21/B.21 содержали более широкие идеи. Часть из них была сознательно
сужена, отложена или превращена в future branches. Ниже перечислено, что из
этих остатков полезно взять дальше.

## Follow-up 1 - Urban route export to SITL/PX4

**Источник:** A.21 M70, B.21 M64/M68, later C.21 M70 framing.

**Что осталось:**

- Urban route-to-waypoint conversion.
- Dry-run export path: Urban planned route -> SITL waypoint mission.
- Optional local PX4/SIH upload/execute artifact for generated Urban waypoints.
- Explicit docs: local PX4/SIH evidence only, no hardware and no real obstacle
  avoidance claim.

**Почему полезно:**

Это самый прямой мост между уже реализованным Urban simulation layer и уже
реализованным PX4/SIH workflow. После этого можно честно говорить, что Urban
routes are exportable to a local SITL waypoint workflow.

**Куда встроить:**

- Branch: Urban route export to SITL/PX4.
- Supporting branch: PX4/SITL Hardening.

**Минимальный следующий scope:**

1. Unit test: route segments -> ordered waypoint items.
2. Dry-run command or config fixture for exported Urban route.
3. Safety validation reuse.
4. Optional manual PX4/SIH artifact if environment is available.

## Follow-up 2 - Urban v2: replan / wait / yield / deconfliction

**Источник:** A.21 M67, B.21 M68 Option A.

**Что осталось:**

- Runtime dynamic blocked-edge lifecycle.
- Mock obstacle detector as active policy trigger.
- Stop / wait / replan / yield policies.
- Multi-agent route ownership and route conflict resolution.
- Separation-aware route deconfliction.
- Metrics such as:
  - `replan_count`;
  - `replan_success_rate`;
  - `wait_time_ticks`;
  - `near_miss_count`;
  - `avoided_collision_count`;
  - `route_conflict_count`.

**Текущее состояние:**

Есть Urban road graph, static blocked-edge validation, judge/report artifacts,
two-agent diagnostic fixture and aggregate separation/conflict metrics. Но это
observability/analysis, not active avoidance or deconfliction behavior.

**Почему полезно:**

Это превращает Urban из route-following mission в mission-level decision logic:
дрон не просто идёт по заранее построенному маршруту, а реагирует на
изменившиеся условия в deterministic simulation.

**Куда встроить:**

- Branch: Urban v2 / reactive mission realism.
- Later benchmark branch for obstacle-density and multi-agent sweeps.

**Минимальный следующий scope:**

1. One deterministic blocked edge appears before agent reaches it.
2. Policy chooses wait or replan.
3. Replay records decision.
4. Metrics distinguish planned route, blocked route, and recovered route.

## Follow-up 3 - Algorithm Depth: communication-aware allocation

**Источник:** A.21 M68, B.21 M66.

**Что осталось:**

- `comms_range` should affect scoring for relevant allocators.
- `comms_penalty_weight` or `message_budget`.
- Controlled benchmark delta under heavy-loss and partition-prone profiles.

**Текущее состояние:**

`ConnectivityAwareAllocator` exists, but broad allocator scoring still does not
fully use communication constraints. M68 only added a narrow Urban
corridor-aware planner delta.

**Почему полезно:**

Это один из самых понятных способов сделать стратегии отличимыми: assignments
that leave reliable communication range should have measurable cost.

**Куда встроить:**

- Branch: Algorithm Depth.
- Later Research Benchmark / support matrix.

**Минимальный следующий scope:**

1. Unit-level scoring test for in-range vs out-of-range assignment.
2. Configurable penalty weight with zero preserving old behavior.
3. Small comparative run showing message/success/availability tradeoff.

## Follow-up 4 - Algorithm Depth: wildfire priority-triggered reallocation

**Источник:** A.21 M68, B.21 M66, older Disaster Mapping v2 notes.

**Что осталось:**

- Dynamic priority update should trigger reassignment when a zone becomes
  critical.
- High-priority wildfire tasks should preempt lower-value work when policy says
  so.
- Replay and metrics should show that reallocation was caused by threat/priority
  change.

**Текущее состояние:**

Wildfire priority fields and success semantics exist. Priority affects scoring
when allocation happens, but a priority update does not automatically force a
new allocation loop for already assigned agents.

**Почему полезно:**

This is a compact, mission-specific algorithm improvement with clear expected
behavior and deterministic tests. It also keeps Disaster Mapping useful without
starting full flood implementation.

**Куда встроить:**

- Branch: Algorithm Depth.
- Branch: Disaster Mapping / Wildfire v2.

**Минимальный следующий scope:**

1. Controlled scenario with low-priority assigned task and later critical zone.
2. Priority update emits reallocation trigger.
3. Agent assignment changes in deterministic test.
4. Replay/report distinguishes priority update from ordinary assignment.

## Follow-up 5 - Algorithm Depth: SAR belief/entropy ordering

**Источник:** B.21 M66.

**Что осталось:**

- Dynamic belief updates after scan events.
- Task ordering based on remaining uncertainty / entropy, not just static
  priority and distance.
- Optional config flag so existing deterministic behavior remains available.

**Почему полезно:**

SAR success has historically been hard to interpret. Belief-aware ordering would
make SAR more semantically meaningful and give algorithms better pressure than
static cell visitation.

**Куда встроить:**

- Branch: Algorithm Depth.
- Branch: Research Benchmark if SAR remains part of published claims.

**Минимальный следующий scope:**

1. Small belief-grid fixture.
2. Scan event changes posterior.
3. Ordering changes only when dynamic belief mode is enabled.
4. Docs explain SAR success vs probability-of-detection semantics.

## Follow-up 6 - Algorithm Depth: CBBA convergence diagnostics

**Источник:** A.21 M68, B.21 M66.

**Что осталось:**

- Replay events or diagnostic counters for CBBA convergence state.
- Clear explanation of coverage CBBA rows where completion is high but success
  is poor under heavy loss/high latency.
- Experiment with gossip interval or failure-triggered gossip burst if evidence
  supports it.

**Текущее состояние:**

Support matrix marks some CBBA/SAR gaps as unsupported/analysis-only, but there
is no deep diagnostic timeline that explains the convergence failure.

**Почему полезно:**

Without this, benchmark interpretation stays shallow: "CBBA failed" is less
useful than knowing whether it is an unsupported design boundary, a parameter
issue, or a fixable bug.

**Куда встроить:**

- Branch: Algorithm Depth.
- Branch: Replay / Analysis Tooling.
- Branch: Research Benchmark.

**Минимальный следующий scope:**

1. Add CBBA diagnostic event/counter in controlled mode.
2. Run a small heavy-loss profile with replay enabled.
3. Document whether the failure is reconvergence, stale ownership, or success
   predicate mismatch.

## Follow-up 7 - Research Benchmark statistical layer

**Источник:** A.21 M69, B.21 M67.

**Что осталось:**

- Confidence intervals / stderr.
- Degradation curves.
- Full support matrix integration.
- Current-vs-historical benchmark classifier.
- Urban inclusion in benchmark entrypoint, or explicit separate Urban suite
  decision.

**Текущее состояние:**

M69 produced a 1000-seed release artifact for built-in `--mission all`, but
`--mission all` currently covers coverage, emergency-mesh, SAR, inspection and
wildfire. Urban evidence remains separate in the M68 corridor-delta artifact.

**Почему полезно:**

The 1000-seed artifact is useful validation evidence, but publication-like
claims need interpretation and statistical summaries.

**Куда встроить:**

- Branch: Research Benchmark / Publication Evidence.

**Минимальный следующий scope:**

1. Add stderr/confidence helper over existing aggregate metrics.
2. Expand support matrix output in reports.
3. Add an explicit Urban benchmark mode or document why Urban stays separate.
4. Generate one small degradation sweep before doing any new long run.

## Follow-up 8 - PX4/SITL local harness scripts

**Источник:** B.21 M63.

**Что осталось:**

- `scripts/run_m58_local.sh`.
- `scripts/run_m59_local.sh`.
- Process cleanup and artifact collection around local PX4/SIH.

**Текущее состояние:**

M58/M59 artifacts exist and replay tooling exists, but there is no committed
`scripts/` harness directory for repeating the local PX4/SIH runs.

**Почему полезно:**

This improves reproducibility without making PX4 part of default CI.

**Куда встроить:**

- Branch: PX4/SITL Hardening.
- Branch: Urban route export to SITL/PX4 if Urban export is chosen.

**Минимальный следующий scope:**

1. Script skeleton with explicit environment assumptions.
2. Start/wait/run/cleanup behavior.
3. Output directory discipline.
4. Docs section explaining manual-only nature.

## Follow-up 9 - Urban Search dynamic bus route

**Источник:** B.21 M65.

**Что осталось:**

- Bus route over road graph.
- `pose_at_tick` from route schedule.
- Appearance/disappearance over time with deterministic movement.

**Текущее состояние:**

Urban Search v1 has a static mocked bus target with active tick range and
deterministic detector behavior. That is enough for v1, but not the full B.21
dynamic-bus shape.

**Почему полезно:**

This is the next natural step for "облетай квартал пока не встретишь автобус":
the target should move along the same road graph as the drone.

**Куда встроить:**

- Branch: Urban v2.
- Branch: New Mission / dynamic target mechanics if Pursuit is later chosen.

**Минимальный следующий scope:**

1. Route-scheduled bus with linear interpolation between nodes.
2. Detector samples bus pose at tick.
3. Replay shows bus movement or at least observed bus pose.
4. One deterministic scenario where detection tick is predictable.

## Follow-up 10 - Platform/API boundary from extension work

**Источник:** A.21 M70, B.21 M68, earlier extension-guide plans.

**Что осталось:**

- External-style mission example.
- Schema compatibility tests.
- Crate boundary review.
- Report/replay schema policy hardening.

**Почему полезно:**

After Urban, the extension path is no longer hypothetical. Platform/API work can
use Urban as a real example instead of test-only fixtures.

**Куда встроить:**

- Branch: Platform / API Packaging.

**Минимальный следующий scope:**

1. Documented extension checklist backed by a real mission.
2. Schema compatibility smoke tests for scenario/replay/report.
3. Explicit "no public semver promise yet" note unless publication is chosen.

## Practical priority

If choosing purely by value-to-effort, I would rank these follow-ups:

1. Urban route export to SITL/PX4.
2. PX4/SITL local harness scripts.
3. Urban v2 blocked-edge replan/wait policy.
4. Communication-aware allocation scoring.
5. Research Benchmark statistical layer.
6. CBBA convergence diagnostics.
7. Wildfire priority-triggered reallocation.
8. Dynamic bus route.
9. SAR belief/entropy ordering.
10. Platform/API boundary hardening.

This order is not mandatory. It is a pragmatic sequence if the goal is to keep
the project close to mission-level drone coordination while avoiding real
hardware and low-level flight-control scope.
