# DRONE_B.21 — Итоговый набор векторов развития после M57–M62

Дата: 2026-05-31

Этот документ — синтез трёх независимых вариантов (A.20, B.20, C.20). Он не
выбирает одну обязательную последовательность, но фиксирует приоритеты, детали
реализации, done criteria и тестовую стратегию для каждого вектора.

---

## Архитектурный принцип

Перед выбором вектора важно зафиксировать, что проект делает, а что нет.

| Слой | Где живёт |
|---|---|
| Stabilization, attitude/rate control, motor physics | PX4 / autopilot |
| Следование waypoint-миссии | PX4 / autopilot |
| Настоящий lidar, SLAM, CV, object detection | внешний perception stack |
| Карта: allowed/forbidden zones, road graph | **этот проект** |
| Mission-level route planning | **этот проект** |
| Mission-level decision logic | **этот проект** |
| Geometric simulation judge | **этот проект** |
| Multi-agent coordination, task allocation, reallocation | **этот проект** |
| Mock perception events | **этот проект** |

Короткая формула:

> Не пишем свой PX4. Пишем mission-level карту, route planning, mock
> perception, judge и decision logic.

Это позволяет добавлять реализм на уровне миссии, не дублируя PX4 low-level
control и не претендуя на настоящий sensor stack.

---

## Текущая стартовая точка

**Сильные стороны после M57–M62:**

- mission DSL: `TaskKind`, `MissionAdapter`, `AdapterRegistry`, `RunState`;
- allocation слой: greedy, auction, CBBA, centralized, connectivity-aware;
- simulation runner, metrics/report export, regression/benchmark infrastructure;
- replay/event log schema;
- local PX4/SIH single-agent и multi-agent evidence;
- controlled live failure/reallocation artifact;
- 500-seed release benchmark baseline;
- extension guide (`docs/EXTENSION_GUIDE.md`).

**Открытые долги и ограничения:**

- нет полноценной карты как набора полигонов / коридоров / road graph;
- нет route planner через ограниченное пространство;
- нет continuous collision judge;
- нет mock perception interface как первоклассного концепта;
- flood до сих пор не реализован, но wording в части docs/README исторически
  смешивает wildfire/flood;
- wildfire success semantics задокументированы словами, но не зафиксированы тестом;
- replay completion events для M59 recovered tasks пишут seq из оригинального
  manifest, а не из replacement mission (technical debt);
- benchmark актуален для текущего HEAD, но не включает новых миссий и не имеет
  confidence intervals.

---

## Vector 0 — Evidence / Cleanup

### Суть

Короткий стабилизирующий pass перед крупным новым milestone. Цель — привести
docs, tests и artifacts в состояние, когда они честно описывают то, что
реально сделано, и не тащат ложные claims в следующий milestone.

### Что сделать

**0.1 — Replay seq fix (M59 technical debt)**

Баг: completion events для recovered tasks пишутся с seq из оригинального
manifest, а не из replacement mission. Например, wp-0 был seq=0 у agent-0,
после replacement у agent-1 он стал seq=2, но completion event пишет seq=0.

Причина: `record_live_agent_run` берёт seq через `manifest_waypoint_for_task_id`,
который ищет в исходном manifest. `SitlTaskProgress` знает правильный seq из
телеметрии, но эта информация теряется.

Минимальный fix: добавить `completed_task_items: Vec<(u16, String)>` в
`LiveAgentRun` — пары (seq, task_id) из активной mission controller-а.
`FakeLiveAgentController` и `Px4AgentController` заполняют их из реальной
mission sequence. `record_live_agent_run` использует их напрямую, не ища seq
в manifest.

Затронутые файлы:
- `crates/swarm-examples/src/sitl_supervisor.rs` — `LiveAgentRun`, оба
  fake-контроллера, `record_live_agent_run`;
- тесты: `fake_live_supervisor_reallocates_lost_before_start_to_pending_survivor`
  и `fake_live_supervisor_replacement_appends_recovered_tasks_in_manifest_order`
  должны проверить, что seq в completion events совпадает с seq из
  replacement mission.

**0.2 — Wildfire success semantics тест**

Добавить unit/integration тест, который явно фиксирует:
`success == (mapped_ratio >= threshold) && all_expected_failures_detected`.

Конкретно: небольшой inline fixture для small-static профиля с контролируемым
числом зон. При 100% task completion → `mapped_ratio` должен достигать
threshold → success = true. Если threshold = 0.8 слишком строгий для
medium-dynamic при ограниченных тиках — задокументировать это явно в
runner.rs и в `docs/BENCHMARK_RESULTS.md`.

**0.3 — Flood scope закрытие**

Убрать flood из пользовательских обещаний:
- README Quick Start шаг 6: переименовать «wildfire / flood mapping» в
  «wildfire mapping»;
- doc comments вида `/// Wildfire / Flood Mapping` → `/// Wildfire Mapping`;
- `docs/STATUS.md`: зафиксировать flood as future work, не partial.

Делать только вариант A (cleanup). Минимальная flood реализация — только если
Disaster Mapping снова станет основным направлением.

**0.4 — Benchmark artifact sync**

Проверить, что `results/all_500_jobs14_m62_release/manifest.json` ссылается на
коммит, который совпадает с текущим HEAD (или явно задокументировать его как
historical baseline). После любого simulation-affecting изменения в коде — либо
пометить artifact как historical, либо переприщитать.

### Done criteria

- replay seq совпадает в completion events и mission item sent events для
  recovered tasks;
- wildfire success rule покрыт тестом;
- README/docs не упоминают flood как реализованную feature;
- benchmark artifact не создаёт ложного впечатления актуальности если код
  изменился.

### Тесты

#### Без рефакторинга

- `fake_live_supervisor_replacement_*` тесты расширяются: asserting seq in
  completion events matches replacement mission seq.
- Wildfire small-static fixture: `success == true` при expected completion.
- Replay summary roundtrip для M59 events (уже есть, проверяет counts).

#### Лёгкий рефакторинг

- Helper `assert_completion_seq_matches_mission(log, manifest)`.
- Wildfire fixture builder с контролируемым числом зон и threshold.

#### Тяжёлый рефакторинг

- Benchmark artifact validator: проверяет, что `manifest.json` соответствует
  текущему git HEAD без machine-specific пути.

---

## Vector 1 — Urban Navigation / Mission Realism

### Суть

Добавить simulation mission family, приближённую к реальным задачам, но без
настоящей физики, CV, Gazebo или hardware dependence.

Этапы: U1 (patrol), U2 (bus search), U3 (avoidance/multi-agent) — строго
последовательно. Не начинать U2 до стабильного U1. Не начинать U3 до
стабильного U2.

### U1 — Urban Patrol v0

**Цель:** один дрон объезжает квартал без нарушений карты. Без лидара, без
автобусов, без полигонов на первом этапе.

#### Urban Map DSL

Начать с road graph, не с arbitrary polygons. Это сдерживает complexity.

```rust
/// Intersection or route waypoint on the road network.
pub struct RoadNode {
    pub id: NodeId,
    pub pose: Pose,
}

/// Traversable road segment between two nodes.
pub struct RoadEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub width_m: f64,
    pub blocked: bool,
}

/// Static obstacle occupying a rectangular footprint.
pub struct Building {
    pub id: String,
    pub min: Pose,
    pub max: Pose,
}

pub struct UrbanMap {
    pub nodes: Vec<RoadNode>,
    pub edges: Vec<RoadEdge>,
    pub buildings: Vec<Building>,
    pub altitude_m: f64,
}
```

Scenario DSL extension (inline в scenario JSON):

```json
"urban_map": {
  "altitude_m": 30.0,
  "nodes": [
    { "id": "n0", "pose": { "x": 0.0, "y": 0.0, "z": 0.0 } },
    ...
  ],
  "edges": [
    { "from": "n0", "to": "n1", "width_m": 8.0, "blocked": false },
    ...
  ],
  "buildings": [
    { "id": "b0", "min": { "x": 10.0, "y": 10.0, "z": 0.0 },
                  "max": { "x": 40.0, "y": 40.0, "z": 50.0 } }
  ]
}
```

#### Route Planner

Dijkstra или A* по road graph. Детерминированный tie-breaking (по id или
расстоянию). Возвращает `Vec<NodeId>` — ordered patrol loop.

```rust
pub fn plan_patrol_route(
    map: &UrbanMap,
    start: NodeId,
    patrol_nodes: &[NodeId],
) -> Result<Vec<NodeId>, UrbanPlanError>;
```

На первом этапе: без local obstacle avoidance. Маршрут планируется один раз
перед стартом миссии.

#### Geometric Judge

Независимый deterministic judge — не часть mission, вызывается runner-ом в
конце каждого тика или при завершении миссии.

```rust
pub struct UrbanJudge<'a> {
    pub map: &'a UrbanMap,
    pub min_separation_m: f64,
}

impl UrbanJudge<'_> {
    /// Check if a movement segment crosses a building AABB.
    pub fn segment_hits_building(&self, from: Pose, to: Pose) -> Option<&Building>;

    /// Check if a point is outside all road edge corridors.
    pub fn point_outside_corridors(&self, pose: Pose) -> bool;

    /// Check if two agent poses are too close.
    pub fn separation_violated(&self, a: Pose, b: Pose) -> bool;
}
```

AABB vs segment intersection: стандартная 2D геометрия, без зависимостей.
Добавить в `crates/swarm-sim/src/urban.rs` или отдельный `swarm-urban` crate
только если scope вырастет.

#### Replay Events

```rust
UrbanRoutePlanned { agent_id, node_ids: Vec<NodeId>, tick }
UrbanSegmentEntered { agent_id, from: NodeId, to: NodeId, tick }
UrbanViolation { agent_id, kind: ViolationKind, tick }
  // kind: BuildingCollision | CorridorExit | SeparationBreach
UrbanPatrolCompleted { agent_id, segments_completed, tick }
```

#### Метрики

```rust
pub struct UrbanPatrolMetrics {
    pub patrol_completion_rate: f64,   // segments_completed / segments_total
    pub urban_violations: u64,
    pub building_collisions: u64,
    pub corridor_exits: u64,
    pub separation_breaches: u64,
    pub route_length_m: f64,
    pub route_efficiency: f64,         // optimal_length / actual_length
    pub replans: u64,
    pub time_to_complete_loop: Option<u64>,  // ticks
}
```

#### TaskKind

Переиспользовать `TaskKind::Waypoint` если семантика совпадает. Добавить
`TaskKind::UrbanWaypoint` только если нужна специфичная completion logic
(например, "посетить node и вернуться на следующий за N тиков").

#### Не делать в U1

- Никакого лидара.
- Никаких автобусов.
- Никакого arbitrary polygon geometry (только AABB для зданий).
- Никакого PX4 requirement.
- Никакой визуализации.
- Никакого local replanning.

### U2 — Urban Search v1

**Цель:** дрон патрулирует до обнаружения автобуса.

#### Bus Entity

```rust
pub struct BusEntity {
    pub id: BusId,
    pub route: Vec<(NodeId, u64)>,  // (node, arrival_tick)
    pub speed_m_per_tick: f64,
    pub appears_at_tick: u64,
    pub disappears_at_tick: Option<u64>,
}
```

Движение: линейная интерполяция по route. Детерминированное по seed и тику.
Не нужна физика — только `pose_at_tick(tick) -> Pose`.

#### BusDetector trait

```rust
pub trait BusDetector {
    fn detect(
        &self,
        agent_pose: Pose,
        bus_pose: Pose,
        rng: &mut impl Rng,
    ) -> DetectionResult;
}

pub struct DetectionResult {
    pub detected: bool,
    pub false_positive: bool,
}
```

Mock реализация:

```rust
pub struct MockBusDetector {
    pub range_m: f64,
    pub detection_probability: f64,
    pub false_positive_rate: f64,
}
```

Line-of-sight (проверка building AABB) — опционально, добавить в U3 если нужно.

#### Mission Policy

Patrol loop → проверить BusDetector на каждом тике → при `detected=true`
завершить миссию как `BusFound`. При `false_positive=true` продолжить патруль.
Completion: `BusFound` или `PatrolTimeout`.

#### Replay Events

```rust
BusObserved { agent_id, bus_id, agent_pose, bus_pose, tick }
BusDetectionFalsePositive { agent_id, bus_id, tick }
UrbanSearchCompleted { agent_id, outcome: SearchOutcome, tick }
  // outcome: BusFound | Timeout | Violation
```

#### Метрики

```rust
pub struct UrbanSearchMetrics {
    pub bus_detection_rate: f64,
    pub time_to_detect_bus: Option<u64>,
    pub false_positive_count: u64,
    pub distance_before_detection_m: f64,
    pub search_success_without_violation: bool,
    pub patrol_loops_completed: u64,
}
```

#### Не делать в U2

- Никакого настоящего CV.
- Никакого image simulation.
- Никакой физической модели автобуса за пределами детерминированного
  движения по graph.
- Никакого multi-agent bus search без стабильного U1.

### U3 — Dynamic Avoidance v2

**Цель:** временные препятствия на маршруте, replan policy, multi-agent
separation.

Только после стабильного U1 и U2. Конкретный scope:

1. Временная блокировка edge в road graph (`blocked=true` с tick range).
2. Lidar-like range detector как mocked geometry query (не raycast):
   `obstacles_within_range(pose, range_m, map) -> Vec<ObstacleId>`.
3. Replan policy при blocked edge:
   - `Stop`: ждать N тиков;
   - `Replan`: Dijkstra минуя blocked edge;
   - `Yield`: уступить другому агенту.
4. Multi-agent scenario: два дрона на одном map, separation enforcement.

Метрики: `avoided_collisions`, `unresolved_blockages`, `wait_ticks`,
`replan_count`, `near_miss_count`.

**Не делать в U3:**

- Никакой аэродинамической модели.
- Никакого real lidar/raycast.
- Никакой гарантии hardware obstacle avoidance.
- Polygon geometry — только если road graph AABB оказался недостаточным.

### PX4 export path

После U1: waypoints вдоль patrol route можно экспортировать в формат
`sitl_supervisor` — те же `SitlWaypointItem`. Это позволит прогнать Urban
Patrol через реальный PX4/SIH без изменений в transport layer.
Делать только после стабильного U1 simulation path.

### Тесты

#### Без рефакторинга

- Road graph parse/validation из inline fixture.
- Dijkstra/A* возвращает deterministic loop route для простого квадратного
  блока.
- Urban patrol завершается без violation на valid map.
- Judge сообщает building collision при маршруте через здание.
- Judge сообщает corridor exit при выходе за пределы road corridor.
- Replay roundtrip для urban events.
- Bus entity `pose_at_tick` детерминирован по seed.
- MockBusDetector возвращает `detected=true` при агенте в пределах range.

#### Лёгкий рефакторинг

- Route planning helper shared between runner и тестами.
- Urban map builder для простых square-block fixtures.
- Metrics assertion helper для patrol completion и violations.
- Bus detection fixture с детерминированным seed.
- Scenario builder для urban patrol с контролируемым числом нод.

#### Тяжёлый рефакторинг

- Property tests: random road graph → Dijkstra находит путь или возвращает
  `NoPath`.
- Property tests: любой valid route → judge не сообщает violation.
- Multi-agent separation property tests.
- Dynamic edge blocking с replan: property test что replan не использует
  blocked edge.
- PX4/SIH export validation для urban waypoints.

---

## Vector 2 — Algorithm Depth

### Суть

Улучшить алгоритмы координации так, чтобы разные стратегии давали измеримо
разные результаты. Делать после Urban U1 — тогда алгоритмы тестируются на
более реалистичной среде.

### A1 — Communication-aware allocation scoring

**Текущий gap:** `comms_range` существует на `AllocationAgent`, но не
используется ни в одном allocator кроме `ConnectivityAwareAllocator`, и то
только для relay placement. Greedy, auction, CBBA, centralized игнорируют
`comms_range` при scoring.

**Что сделать:**

1. Добавить `comms_penalty_weight: f64` в конфигурацию allocator-а или в
   scoring helper. Если расстояние `agent → task > comms_range`, score
   снижается на `comms_penalty_weight * overshoot_distance`.

2. Benchmark delta: сравнить coverage/SAR/wildfire с `comms_penalty_weight=0`
   (текущее) и `comms_penalty_weight > 0` по метрикам `agent_availability`,
   `task_conflicts`, `success` на профилях heavy-loss-* и partition-prone-*.

3. Ожидаемый результат: в partition profiles connectivity-aware начинает
   явно выигрывать у greedy по availability, а при `comms_penalty_weight > 0`
   greedy тоже начинает избегать out-of-range assignments.

**Затронутые файлы:** `swarm-alloc/src/allocator.rs` (scoring),
`swarm-types/src/adapter.rs` (возможно score signature),
`swarm-sim/src/runner.rs` (параметры конфига).

### A2 — Mission-specific planners

**Текущий gap:** scoring отличается только коэффициентами. Динамические
события (wildfire priority updates) пишутся в replay, но re-allocation
при изменении приоритета не происходит.

**SAR — belief-entropy driven ordering:**

`sar_task_priority` сейчас статична (вычисляется при генерации сценария).
Dynamic priority update как в wildfire не реализован. Нужно:

1. При каждом scan event обновлять posterior belief для посещённых клеток.
2. Пересчитывать priority незавершённых задач по убыванию remaining entropy.
3. Это создаёт pressure для auction/CBBA переназначить задачи при изменении
   информационного ландшафта.

**Wildfire — priority-triggered reallocation:**

Сейчас `push_wildfire_priority_update` пишет событие, но не триггерит
reallocation. Нужно: при priority update для задачи с `new_priority >= 8`
(критический) — поставить задачу в очередь force-reallocation. Агент,
который летит к низкоприоритетной задаче, должен получить шанс быть
перенаправлен.

**Затронутые файлы:** `swarm-runtime/src/runner.rs` (dynamic reallocation
trigger), `swarm-scenarios/src/sar_scenario.rs` (dynamic belief update),
`swarm-scenarios/src/wildfire.rs` (force-reallocation hook).

### A3 — CBBA convergence диагностика

**Текущий gap:** 6 coverage профилей имеют `success=0.000, completion=1.000`
под CBBA. SAR + CBBA = `unsupported (delayed_reconvergence)`. Emergency mesh
+ CBBA = `conflicts=4.2` vs centralized `conflicts=0.0`.

**Что исследовать:**

1. Coverage heavy-loss/high-latency: hypothesis — при потере агентов CBBA не
   переконвергировал по `gossip_interval_ticks`. Проверить: уменьшить
   `gossip_interval_ticks` в failure профилях → меняется ли success.

2. SAR: при release задачи в runtime — отправлять gossip burst немедленно, не
   ждать следующего gossip интервала. Это может снизить delayed_reconvergence.

3. Добавить replay диагностику: `CbbaConvergenceEvent { agent_id, iteration,
   bundle_size, conflicting_tasks, tick }`. Это поможет понять, сколько итераций
   нужно для переконвергенции после failure.

**Затронутые файлы:** `swarm-alloc/src/cbba.rs`,
`swarm-runtime/src/coordinator.rs` (gossip burst при failure),
`swarm-examples/src/sitl_observability.rs` (новый event type).

### Тесты

#### Без рефакторинга

- Unit test: comms_penalty_weight > 0 снижает score для agent за пределами
  comms_range.
- Unit test: wildfire priority update с new_priority >= 8 триггерит
  force-reallocation флаг.
- Support matrix tests для всех unsupported пар (уже есть, проверить
  что остаются green).
- CBBA gossip burst тест: после failure event gossip отправляется в текущем
  тике.

#### Лёгкий рефакторинг

- Scoring comparison helper с детерминированными inputs.
- SAR belief-entropy update fixture.
- Wildfire scenario builder с контролируемыми priority updates.
- CBBA replay diagnostic fixture.

#### Тяжёлый рефакторинг

- Property tests для CBBA convergence под arbitrary message loss.
- Multi-seed comparison benchmark с/без comms_penalty_weight.
- SAR dynamic priority benchmark delta.

---

## Vector 3 — PX4 / SITL Hardening

### Суть

Поддерживающий трек. Делать точечно по мере необходимости, не как основной
вектор. M58/M59/M60 уже закрыли главный gap.

### D1 — Broader failure matrix

Текущий M59 артефакт покрывает один failure mode: kill PX4 process →
`disconnected`. Следующие paths:

- **Fail before upload:** PX4 недоступен при старте (port conflict, wrong
  system_id). Supervisor должен пометить агента failed до upload и
  реаллоцировать.
- **Fail after upload, before start:** `upload_and_execute_mission` успешно,
  но arm/takeoff rejected (низкий заряд батареи в SIH config, prearm checks).
- **Partial completion:** агент завершил N из M задач, потом `disconnected`.
  Survivor получает только оставшиеся M-N.
- **Survivor fails after replacement:** что делает supervisor если survivor
  тоже упал после получения replacement mission.

Для каждого path: fake-controller test + если возможно реальный SIH артефакт.

### D2 — Local harness script

```bash
# scripts/run_m58_local.sh
# Запускает два PX4 SIH инстанса, ждёт порты, запускает supervisor,
# при завершении останавливает PX4, кладёт артефакт в results/local_YYYY-MM-DD/
```

Не CI. Не запускать автоматически. Только локально и явно.

Аналогично `scripts/run_m59_local.sh` с kill первого PX4 по timeout.

### D3 — Replay timeline output

`replay --timeline` — хронологический список событий с префиксом agent_id.
Не визуализация. Инструмент отладки для разработчика.

```
00:00.001 [supervisor] run_started agents=2
00:00.012 [agent-0]   mission_item_sent seq=0 task=wp-0
00:00.018 [agent-1]   mission_item_sent seq=0 task=wp-2
...
00:04.231 [agent-0]   disconnected status=disconnected
00:04.232 [supervisor] agent_lost agent=agent-0
00:04.233 [supervisor] task_released task=wp-0 from=agent-0
00:04.234 [agent-1]   survivor_mission_update_started policy=mission_replacement
```

`replay --agent <id>` — только события одного агента.

**Затронутые файлы:** `crates/swarm-examples/src/bin/replay.rs`, новые
флаги, форматирование событий по `elapsed_ms`.

### Тесты

#### Без рефакторинга

- Fake live controller tests для каждого failure timing (before upload,
  after upload, partial completion).
- Report schema test для partial success.
- Event log summary test для repeated reallocation.
- Replay timeline output test: golden string на small fixture.

#### Лёгкий рефакторинг

- Shared fake live-controller scenario builder.
- Timeout classification helper с детерминированными inputs.

#### Тяжёлый рефакторинг

- Ignored/manual PX4 SIH integration tests для каждого failure path.
- Local PX4 launch harness с log capture.

---

## Vector 4 — Benchmark / Research

### Суть

Не делать следующим крупным этапом. Делать после Urban U1/U2 и/или Algorithm
Depth — иначе benchmark измеряет только текущие абстрактные сценарии.

### Что сделать (после Urban + Algorithm)

1. Закрыть интерпретационные вопросы из `docs/BENCHMARK_RESULTS.md`:
   - SAR success ≈ 0: relaxed success criterion `pod_success_threshold`
     вместо `all_targets_found()`. Задокументировать явно.
   - Wildfire success = 0.247: profile-specific threshold или отдельная
     метрика `zones_mapped_rate`.
   - CBBA coverage 6 failed profiles: replay диагностика (A3) + targeted fix.

2. 1000-seed release run для supported mission-strategy пар.

3. Confidence intervals: `mean ± stderr` для всех major метрик.

4. Degradation curves:
   - `success` vs `agents_count` (2–8) для coverage и wildfire;
   - `success` vs `packet_loss_rate` (0–0.5) для heavy-loss profiles;
   - `success` vs `map_size` для coverage;
   - `urban_violations` vs `obstacle_density` для Urban Patrol (после U1).

5. Strategy comparison report: для каждой пары (mission, profile) — winner
   strategy по каждой метрике с обоснованием.

6. Обновить `docs/BENCHMARK_RESULTS.md`, README, `docs/STATUS.md`.

### Не делать

- Не использовать benchmark как substitute для PX4/SIH evidence.
- Не включать unsupported pairs как success claims.
- Не делать 1000-seed до закрытия интерпретационных вопросов.

---

## Vector 5 — Replay / Analysis

### Суть

Поддерживающий трек. Развивать рядом с Urban Navigation — каждая новая mission
feature должна иметь replay representation.

### Что добавить

1. Urban replay events (описаны в U1/U2).
2. Route trace: `per-agent-pose-by-tick` в компактном формате для replay log.
3. Timeline output (`replay --timeline`, описан в D3).
4. CSV export: per-tick agent positions, per-event category counts.
5. Compatibility: при добавлении новых event types — backward-compatible
   deserialization (unknown events игнорируются, не ломают парсинг).

### Не делать

- Никакого 2D/3D визуального viewer пока нет Urban U1.
- Replay schema migration — только при breaking изменении формата.

---

## Decision Matrix

| Вектор | Пользовательская ценность | Исследовательская ценность | Риск scope creep | Когда |
|---|:---:|:---:|:---:|---|
| 0 Evidence/Cleanup | Средняя | Высокая | Низкий | Сейчас (коротко) |
| 1 Urban U1 Patrol | Высокая | Высокая | Средний | Следующий major |
| 1 Urban U2 Search | Высокая | Высокая | Средний | После U1 |
| 1 Urban U3 Avoidance | Средняя | Средняя | Высокий | После U2 |
| 2 Comms-aware (A1) | Средняя | Высокая | Низкий | После Urban U1 |
| 2 Mission planners (A2) | Средняя | Высокая | Средний | После Urban U1 |
| 2 CBBA convergence (A3) | Низкая | Высокая | Средний | После Urban U1 |
| 3 Failure matrix (D1) | Средняя | Средняя | Низкий | Поддерживающий |
| 3 Local harness (D2) | Средняя | Низкая | Низкий | Поддерживающий |
| 3 Replay timeline (D3) | Средняя | Средняя | Низкий | Поддерживающий |
| 4 Benchmark 1000-seed | Средняя | Высокая | Средний | После Urban + Algo |
| 5 Replay / Analysis | Средняя | Средняя | Низкий | Поддерживающий |

---

## Предлагаемый milestone split

| Milestone | Содержание | Зависит от |
|---|---|---|
| **M63** | Vector 0 (cleanup) + Urban U1 (patrol, road graph, judge) | M62 |
| **M64** | Urban U2 (bus search, mock detector) | M63 |
| **M65** | Algorithm Depth A1+A2 (comms scoring, mission planners) | M63 |
| **M66** | Benchmark refresh (1000-seed, confidence intervals, degradation curves) | M64, M65 |
| **M67** | Urban U3 (avoidance, multi-agent) или новая миссия — решение после M64 | M64 |

M63 и M65 можно начинать параллельно если есть ресурс — Urban U1 и Algorithm
Depth независимы по коду.

---

## Рекомендуемая последовательность

```text
Vector 0 (Evidence/Cleanup)
  -> Urban U1 (patrol, road graph, AABB buildings, judge, replay, metrics)
    -> Urban U2 (bus entity, mock detector, search-until-detected)
      -> Algorithm Depth A1+A2 (comms scoring, mission-specific planners)
        -> Benchmark Refresh (1000-seed, confidence intervals)
          -> Urban U3 или следующая миссия — решение по обстоятельствам
```

Поддерживающие треки (добавлять по мере необходимости):

```text
SITL Hardening D1 (failure matrix) — рядом с Urban U1/U2
Replay/Analysis D3 (timeline) — рядом с Urban U1
Local harness D2 — после Vector 0
```

---

## Что явно не входит в этот план

- Hardware / HIL — не планируется. Граница задокументирована в
  `docs/HARDWARE_READINESS.md`.
- Full polygon geometry и lidar raycast — не в первых итерациях. Начинать
  с road graph и AABB.
- Собственный physics engine или замена PX4 flight control.
- Настоящий object detection / CV.
- Distributed onboard autonomy.
- 2D/3D visual viewer — полезен, но не является инженерной приоритетной ценностью
  на текущем этапе.
- Hierarchical coordination (8+ agents) — слишком рано без benchmark evidence
  что flat coordination не справляется.
- Platform/API packaging (published crates, semver-stable API) — M61 extension
  guide достаточен для in-repository work. Вернуться после одной реальной новой
  миссии через extension path.
