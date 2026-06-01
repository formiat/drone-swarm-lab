# BRANCHES_A.22 - актуальные ветки развития после M69

Дата фиксации: 2026-05-31

Основа: `docs_raw/BRANCHES.md`, `DRONE_A/B/C.16-21.md`, текущий локальный
код и состояние после M69. Документ собирает именно ветки/ответвления развития,
а не линейный milestone-plan.

Главная позиция: базовый ствол M63-M69 уже выполнен как Urban + replay +
algorithm delta + 1000-seed evidence. Поэтому старые пункты вроде "basic Urban
Patrol", "basic Urban Search", "basic Real SITL/PX4" и "просто 1000-seed run"
не надо снова считать открытыми ветками. Их можно углублять, но не начинать с
нуля.

## Что уже не является самостоятельной открытой веткой

- Evidence Cleanup / Status Honesty в базовом виде.
- Real SITL / PX4 foundation: single-agent, multi-agent execute,
  controlled failure/reallocation.
- M59 replacement replay seq fix.
- Basic Urban Foundations, Urban Patrol v0, Urban Search v1.
- Basic Urban replay/timeline/analysis.
- Narrow Urban corridor-aware planner delta.
- Текущий 1000-seed M69 benchmark pack.

Эти направления остаются важными как база, но как стратегическая развилка уже
закрыты.

## Что пока явно не выбираем

- Real hardware / HIL.
- Production-grade safety или certified obstacle avoidance.
- Реальный lidar / SLAM / CV / sensor fusion.
- Distributed onboard autonomy на борту реальных дронов.
- Собственный low-level flight control вместо PX4.
- Visual UI как главный milestone.

Проект должен оставаться mission-level simulation, planning, coordination,
replay, metrics and local SITL evidence layer.

## Короткая карта актуальных веток

### Primary branches

1. Urban route export to SITL/PX4.
2. Urban v2: avoidance, replan, multi-agent deconfliction.
3. Algorithm Depth.
4. Research Benchmark / Publication Evidence.
5. Platform / API Packaging.
6. New Mission: Logistics / Delivery.
7. New Mission: Multi-target Pursuit.

### Supporting branches

8. PX4/SITL Hardening.
9. Replay / Analysis Tooling.
10. Scenario Generation / Synthetic Testbed.
11. Realism v2 / Simulation Fidelity.
12. Disaster Mapping / Wildfire v2 / optional Flood.

### Cross-cutting усилители

Эти темы не стоит оформлять как отдельные равноправные ветки, но их полезно
встраивать почти в любой выбранный путь:

- Scenario library and generators.
- Oracle / solver baselines.
- Invariant / safety-case checks at simulation level.

---

## Branch 1 - Urban Route Export to SITL/PX4

**Статус:** актуальная primary branch и естественный вариант M70.

**Суть:** взять уже существующие Urban routes и экспортировать их в waypoint
mission для local PX4/SIH. Это связывает Urban simulation layer с уже
реализованным PX4/SIH workflow.

**Зачем нужно:**

- Проверить, что mission-level route planning можно превратить в executable
  waypoint plan.
- Дать Urban ветке реальный local SITL artifact, а не только headless
  simulation metrics.
- Использовать уже сильную сторону проекта: safety gate, MAVLink mission upload,
  SITL event logs, replay, artifact discipline.

**Что сделать:**

1. Route-to-waypoint conversion для Urban planned route.
2. Dry-run export: route segments -> mission items -> validation report.
3. Reuse existing safety validation: geofence, no-fly zones, separation where
   applicable.
4. Optional local PX4/SIH upload-only или execute artifact.
5. Result directory with manifest, command, route summary, SITL log and replay
   summary.
6. Docs wording: local PX4/SIH evidence, not hardware readiness and not real
   obstacle avoidance.

**Где пригодится:**

- Demonstrating practical route execution path.
- Future local SITL regression/manual checks.
- Comparing pure simulation route semantics with PX4 waypoint execution.

**Риски:**

- Можно случайно начать обещать real obstacle avoidance. Нельзя: в этой ветке
  PX4 исполняет waypoints, а не доказывает безопасность городского полета.
- Local PX4 setup remains manual/host-specific, so default tests must stay
  mock/fake.

**Первый разумный milestone:** Urban route to waypoint dry-run + conversion unit
tests + one local PX4/SIH upload artifact if environment is available.

## Branch 2 - Urban v2: Avoidance, Replan, Multi-Agent Deconfliction

**Статус:** актуальная primary branch после Urban v0/v1.

**Суть:** развить Urban от "маршрут построен и пройден" к mission-level
decision logic: что делать, если route blocked, появился объект, два агента
конфликтуют на одном corridor, или judge предсказывает violation.

Это не certified collision avoidance. Это deterministic simulation and
decision layer above autopilot.

**Что сделать:**

1. Temporary blocked edges / road segments.
2. Mock obstacle detector как geometry query over known map, не настоящий lidar.
3. Stop / wait / replan / yield policies.
4. Multi-agent route conflict representation.
5. Separation enforcement at mission/judge level.
6. Replay events:
   - `UrbanObstacleDetected`;
   - `UrbanReplanStarted`;
   - `UrbanReplanCompleted`;
   - `UrbanYieldDecision`;
   - `UrbanNearMiss`;
   - `UrbanConflictResolved`.
7. Metrics:
   - `replan_count`;
   - `replan_success_rate`;
   - `wait_time`;
   - `route_conflict_count`;
   - `near_miss_count`;
   - `avoided_collision_count`;
   - `urban_violation_count`.

**Где пригодится:**

- Для задач типа "облетай квартал, пока не встретишь автобус".
- Для multi-agent urban scenarios.
- Для проверки алгоритмов на более реалистичных constraints.

**Риски:**

- Легко уйти в geometry engine. Начинать надо с road graph and blocked edges.
- Легко перепутать mock perception с реальным perception. Документация должна
  явно фиксировать границу.

**Первый разумный milestone:** deterministic blocked-edge replan on road graph
with replay and metrics.

## Branch 3 - Algorithm Depth

**Статус:** актуальная primary branch. M68 сделал только узкий corridor-aware
delta, но не закрыл широкую алгоритмическую ветку.

**Суть:** сделать стратегии реально различимыми и лучше объяснимыми: где greedy
достаточен, где нужен centralized, где CBBA ломается, где communication-aware
scoring даёт смысл.

**Основные workstreams:**

1. **Communication-aware allocation**
   - `comms_range` сейчас не является полноценным фактором scoring для большинства
     allocators.
   - Добавить `comms_penalty_weight` или `message_budget`.
   - Сравнить success / messages / availability under packet loss and partition
     profiles.

2. **Mission-specific planners**
   - SAR: information gain / belief entropy priority.
   - Wildfire: priority-triggered reallocation after threat updates.
   - Inspection: route optimization beyond centralized path.
   - Urban: replan-aware scoring and deconfliction costs.

3. **CBBA convergence and support matrix**
   - Разделить "unsupported by design" и "bug/regression".
   - Replay-driven diagnostics for delayed reconvergence.
   - Failure-triggered gossip burst or tuned gossip interval.
   - Re-benchmark CBBA after targeted changes.

4. **Scale beyond small swarms**
   - 8-agent and 16-agent profiles.
   - Message-count scaling curves.
   - Hierarchical coordination only if measurement proves need.

**Где пригодится:**

- Research Benchmark.
- Publication-quality claims.
- Выбор стратегии под конкретный mission type.

**Риски:**

- Можно добавить сложность без измеримого выигрыша. Поэтому каждое изменение
  должно иметь benchmark delta.
- Hierarchical coordination лучше не начинать без evidence, что 8+ agents
  реально требуют нового дизайна.

**Первый разумный milestone:** communication-aware scoring или wildfire
priority-triggered reallocation, потому что оба дают понятную гипотезу и
измеримый delta.

## Branch 4 - Research Benchmark / Publication Evidence

**Статус:** актуальная primary/supporting branch. M69 дал 1000-seed artifact, но
не сделал весь research layer.

**Суть:** превратить benchmark из "таблиц и артефактов" в доказательную базу с
интерпретацией, confidence intervals, degradation curves and support matrix.

**Что уже есть:**

- Release 1000-seed M69 artifact.
- JSON/CSV/Markdown exports.
- Manifest discipline.
- Baseline result docs.

**Чего не хватает:**

1. Confidence intervals / stderr.
2. Degradation curves:
   - packet loss;
   - latency;
   - agent count;
   - grid/map size;
   - urban obstacle density;
   - bus detection probability.
3. Strategy comparison report with interpretation.
4. Full support matrix by mission/strategy/profile.
5. Current-vs-historical benchmark classifier.
6. Urban inclusion in standard benchmark entrypoint, or explicit reason why
   Urban is separate.
7. Clear explanation of SAR success, wildfire success, CBBA gaps.

**Где пригодится:**

- Перед внешней публикацией.
- Для выбора следующей algorithm branch.
- Для честного README/status.

**Риски:**

- Если делать до Algorithm Depth, артефакт быстро устареет.
- 1000 seeds без интерпретации не добавляют столько ценности, сколько кажется.

**Первый разумный milestone:** confidence interval helper + support matrix
expansion + benchmark interpretation pass over existing M69 results.

## Branch 5 - Platform / API Packaging

**Статус:** актуальная primary branch, но не обязательно первая.

**Суть:** сделать проект удобнее для добавления миссий, стратегий, метрик и
reports без постоянного изменения ядра.

**Что сделать:**

1. Review crate boundaries:
   - what is public;
   - what remains internal;
   - what is test-only.
2. External-style mission example inside workspace.
3. Schema compatibility tests for scenario/replay/report.
4. Extension path validation:
   - new mission;
   - new strategy;
   - new metric;
   - new replay event.
5. Stable report schema with explicit version policy.
6. Deprecation/changelog discipline.

**Где пригодится:**

- Если проект должен стать reusable toolkit.
- Если новые миссии будут добавляться часто.
- Если нужен понятный entrypoint для внешнего пользователя.

**Риски:**

- Преждевременный semver может зацементировать неправильные abstractions.
- Лучше делать после ещё одной non-trivial mission или после Urban export.

**Первый разумный milestone:** external-style mission fixture + schema
compatibility tests, without public semver promise.

## Branch 6 - New Mission: Logistics / Delivery

**Статус:** актуальная primary branch, не реализована.

**Суть:** добавить миссию с precedence constraints and capacity: pickup/dropoff,
cargo capacity, deadlines/time windows later.

**Зачем нужно:**

- Проверяет, что система умеет не только "посетить точки", но и соблюдать
  зависимости между задачами.
- Стресс-тестит allocator: нельзя назначить dropoff без pickup.
- Хорошо проверяет DSL, validation, state tracking and metrics.

**Что сделать:**

1. `TaskKind::Pickup` / `TaskKind::Dropoff`.
2. Item/cargo domain model.
3. Agent cargo state and capacity.
4. Precedence validation in mission adapter / runtime.
5. Deadline/time window as optional later extension.
6. Metrics:
   - `delivery_rate`;
   - `late_deliveries`;
   - `capacity_violations`;
   - `precedence_violations`;
   - `unserved_deliveries`;
   - `total_route_cost`.
7. Replay events:
   - pickup completed;
   - dropoff completed;
   - capacity violation;
   - precedence violation.

**Где пригодится:**

- Проверка platform/API extension path.
- Исследование task dependencies.
- Benchmark для scheduling/VRP-like scenarios.

**Риски:**

- Может превратиться в отдельный vehicle routing project.
- Нужен careful scope: сначала small deterministic cases, потом deadlines.

**Первый разумный milestone:** small pickup/dropoff scenario without deadlines,
with capacity and precedence tests.

## Branch 7 - New Mission: Multi-Target Pursuit

**Статус:** актуальная primary branch, не реализована.

**Суть:** движущиеся цели, которые нужно перехватить или сопровождать. Это
первый mission type, где задачи меняют положение во времени.

**Зачем нужно:**

- Проверяет reactive reallocation.
- Делает CBBA/auction/greedy comparison гораздо интереснее.
- Стресс-тестит replay and benchmark under dynamic tasks.

**Что сделать:**

1. `TaskKind::Pursuit` with target id and mode.
2. Target state and trajectory model.
3. Intercept / escort completion predicate.
4. Dynamic target appearance/disappearance.
5. Predictive routing or simple leading heuristic.
6. Metrics:
   - `capture_rate`;
   - `time_to_intercept`;
   - `targets_lost`;
   - `total_pursuit_distance`;
   - `interception_efficiency`.
7. Replay events:
   - target observed;
   - target moved;
   - intercept started;
   - target captured/lost.

**Где пригодится:**

- Dynamic task allocation research.
- Stress-test for algorithm depth.
- Future mock perception work.

**Риски:**

- Сложнее Logistics, потому что меняется state every tick.
- Лучше делать после replay/diagnostics maturity.

**Первый разумный milestone:** deterministic moving target with one agent and
one target, then multi-agent allocation.

## Branch 8 - PX4/SITL Hardening

**Статус:** актуальная supporting branch. Базовая Real SITL/PX4 ветка закрыта,
но воспроизводимость и coverage можно улучшать.

**Суть:** сделать local PX4/SIH workflow менее ручным и более проверяемым.

**Что сделать:**

1. Local integration harness:
   - start/stop two PX4 SIH instances;
   - wait for endpoints;
   - run supervisor;
   - collect logs;
   - cleanup processes.
2. Artifact validator:
   - manifest fields;
   - event categories;
   - expected agent ids;
   - replacement/reallocation semantics;
   - replay parse.
3. Broader failure modes:
   - no-progress timeout;
   - mission rejection;
   - partial completion then failure;
   - repeated failures if useful.
4. SITL documentation:
   - exact commands;
   - known PX4 version;
   - troubleshooting.

**Где пригодится:**

- M70 Urban export.
- Regression-safe manual verification.
- Better confidence in local SITL artifacts.

**Риски:**

- CI-managed PX4 is heavy and should not be default.
- Host environment variability remains real.

**Первый разумный milestone:** artifact validator + local harness script.

## Branch 9 - Replay / Analysis Tooling

**Статус:** актуальная supporting branch.

**Суть:** улучшить способность объяснять runs без GUI-first подхода.

**Что сделать:**

1. Replay timeline filters:
   - by agent;
   - by event category;
   - by task id;
   - by tick/time range.
2. Route trace summaries for Urban and SITL.
3. Cross-run replay diff:
   - same seed before/after algorithm change;
   - event category deltas;
   - route length / violation changes.
4. Artifact inspection command:
   - manifest summary;
   - result summary;
   - replay parse status;
   - known limitations.
5. Optional ASCII overlays for grid/graph missions.

**Где пригодится:**

- Debugging CBBA convergence.
- Explaining Urban replan/deconfliction.
- Validating SITL artifacts.
- Supporting benchmark interpretation.

**Риски:**

- Interactive UI can distract from core research. Keep first step headless.

**Первый разумный milestone:** replay filters + artifact inspection summary.

## Branch 10 - Scenario Generation / Synthetic Testbed

**Статус:** предлагаемый cross-cutting branch/supporting branch.

**Суть:** перейти от небольшого набора hand-written scenarios к воспроизводимым
scenario families with controlled parameters.

**Что сделать:**

1. Scenario generator API with deterministic seed.
2. Generators for:
   - urban maps;
   - bus schedules;
   - blocked edges;
   - packet loss profiles;
   - failure profiles;
   - wildfire threat patterns;
   - logistics pickup/dropoff sets.
3. Scenario library:
   - tiny/small/medium;
   - stress;
   - regression-stable;
   - experimental.
4. Manifest records generator parameters.
5. Tests that generated scenarios are valid and deterministic.

**Где пригодится:**

- Benchmark / Research Evidence.
- Algorithm Depth.
- Urban v2.
- New Mission validation.

**Риски:**

- Random scenario generation без контроля создаст noisy benchmark. Нужны
  deterministic seeds and clearly named profiles.

**Первый разумный milestone:** deterministic Urban scenario generator for
blocked-edge and bus-search profiles.

## Branch 11 - Realism v2 / Simulation Fidelity

**Статус:** supporting branch. Foundation есть, измеримого слоя ещё нет.

**Суть:** сделать realism profiles не просто набором параметров, а проверяемым
слоем с expected effects.

**Что сделать:**

1. Define expected effects for light/medium/heavy:
   - success should drop or remain stable;
   - route cost should increase;
   - availability should degrade;
   - false positives/misses should behave within expected range.
2. Comparative benchmark:
   - ideal vs light;
   - ideal vs medium;
   - ideal vs heavy.
3. Stable realism smoke in regression.
4. Experimental stochastic realism outside default gate.
5. Manifest metadata for realism profile.
6. Docs: what is modeled and what is not.

**Где пригодится:**

- More honest benchmark claims.
- Urban v2 and mock perception.
- Publication evidence.

**Риски:**

- Stochastic tests can become flaky. Default gate must use deterministic seeds
  and conservative assertions.

**Первый разумный milestone:** expected-effects doc + deterministic realism
smoke for one mission family.

## Branch 12 - Disaster Mapping / Wildfire v2 / Optional Flood

**Статус:** supporting/specialized branch. Не главный путь сейчас, но актуальна
если проект снова выбирает disaster domain.

**Суть:** довести wildfire до более сильной миссии и решить flood только при
явном выборе disaster mapping как фокуса.

**Что сделать для Wildfire v2:**

1. Priority-triggered reallocation after threat updates.
2. Dynamic task injection or zone expansion if useful.
3. Better wildfire metrics:
   - `hazard_zones_mapped`;
   - `high_priority_zones_mapped`;
   - `priority_updates_count`;
   - `time_to_first_critical_zone`;
   - `final_avg_threat_level`.
4. Support matrix for strategies.
5. Success semantics tests across profiles.

**Optional Flood:**

Делать только если disaster mapping становится основным направлением.

Minimal scope:

- flooded zones;
- water spread or static flood map;
- critical zones;
- rescue/mapping priority;
- flood-specific metrics;
- replay events.

**Где пригодится:**

- Disaster-response research framing.
- Mission-specific planner work.
- Benchmark comparison for dynamic priorities.

**Риски:**

- Flood может расползтись в новую большую миссию.
- Если flood не выбран, лучше держать его как documented future work.

**Первый разумный milestone:** wildfire priority-triggered reallocation, not
minimal flood.

---

## Cross-Cutting Усилители

### Scenario Library / Generators

Не отдельная стратегия, а инфраструктура для почти всех веток. Особенно важна
для Urban v2, Algorithm Depth, Benchmark and New Mission.

Минимальный полезный результат: deterministic generator + manifest parameters
for one mission family.

### Oracle / Solver Baselines

Добавить near-optimal or exact-small-case baselines там, где это возможно:

- shortest path / A* for Urban;
- exhaustive small pickup/dropoff solver for Logistics;
- centralized pursuit baseline for tiny target sets;
- idealized route cost lower bound.

Польза: сравнивать стратегии не только друг с другом, но и с понятным upper/lower
bound.

### Invariant / Safety-Case Layer

Это simulation-level guarantees, not hardware safety:

- no duplicate ownership;
- no completion without predicate;
- no route through forbidden zones;
- no dropoff before pickup;
- replay sequence consistency;
- artifact manifest consistency;
- no unsupported strategy silently pretending to be supported.

Польза: повышает доверие к любому выбранному направлению.

## Рекомендуемая развилка на ближайший выбор

Если нужен самый прямой следующий шаг после M69:

1. **Urban route export to SITL/PX4** - если хочется связать Urban с уже
   доказанным local SITL workflow.
2. **Urban v2 / replan / deconfliction** - если хочется развивать
   mission-level decision logic.
3. **Algorithm Depth** - если хочется улучшать стратегии и объяснять benchmark.
4. **Platform/API Packaging** - если цель сделать проект reusable.
5. **Research Benchmark** - если цель подготовить publication-like evidence, но
   лучше после хотя бы одного algorithm/urban improvement.

Мой практический приоритет:

```text
Urban route export to SITL/PX4
  -> PX4/SITL artifact validator / harness
  -> Urban v2 blocked-edge replan
  -> Algorithm Depth benchmark delta
  -> Research Benchmark interpretation
```

Logistics and Pursuit remain strong next-mission candidates, but I would choose
one of them after the Urban/SITL boundary is clarified.
