# DRONE_B.21 — Линейный план M63–M68

Дата: 2026-05-31

## Контекст

M57–M62 закрыты. У проекта есть simulation core, mission DSL, allocation слой,
regression/benchmark, replay, local PX4/SIH single-agent и multi-agent evidence,
controlled failure/reallocation artifact, extension guide.

Следующий шаг — добавить mission-level navigation с реалистичной картой и
mock perception, не дублируя PX4 flight control.

## Архитектурная граница

| Слой | Где живёт |
|---|---|
| Stabilization, attitude/rate control, motor physics | PX4 / autopilot |
| Следование waypoint-миссии | PX4 / autopilot |
| Настоящий lidar, SLAM, CV, object detection | внешний perception stack |
| Карта: road graph, allowed/forbidden zones | **этот проект** |
| Mission-level route planning | **этот проект** |
| Mission-level decision logic | **этот проект** |
| Geometric simulation judge | **этот проект** |
| Mock perception events | **этот проект** |
| Multi-agent coordination, task allocation, reallocation | **этот проект** |

Проект не пишет свой PX4. Проект пишет mission-level карту, route planning,
mock perception, judge и decision logic.

## Scope boundary

### Входит

- road graph map DSL в сценариях;
- AABB building obstacles;
- Dijkstra/A* patrol route planning;
- deterministic geometric judge;
- mock perception (BusDetector);
- communication-aware allocation scoring;
- mission-specific planners (wildfire priority, SAR belief entropy);
- 1000-seed benchmark с confidence intervals;
- replay timeline output;
- мелкие технические долги M59.

### Не входит

- polygon geometry и lidar raycast до стабильного Urban U1;
- настоящий CV / object detection;
- physics engine или замена PX4 flight control;
- hardware / HIL;
- distributed onboard autonomy;
- 2D/3D visual viewer;
- hierarchical coordination (8+ agents) — нет benchmark evidence что нужно;
- published crates / semver-stable public API.

## Линейный план

```
M63  Evidence / Cleanup
  -> M64  Urban Patrol v0
    -> M65  Urban Search v1
      -> M66  Algorithm Depth
        -> M67  Benchmark Refresh
          -> M68  Next Branch Decision
```

M64 и M66 можно начинать параллельно если есть ресурс — они независимы по коду.

---

## M63 — Evidence / Cleanup

### Цель

Закрыть технические долги перед крупным новым milestone. Короткий pass: docs,
tests и artifacts должны честно описывать то, что реально сделано.

### Что сделать

1. **Replay seq fix (M59 technical debt)**

   Баг: completion events для recovered tasks пишутся с seq из оригинального
   manifest, а не из replacement mission. `record_live_agent_run` берёт seq через
   `manifest_waypoint_for_task_id`, который ищет в исходном manifest, поэтому
   для wp-0 возвращает seq=0 (agent-0's original), хотя в replacement mission
   у agent-1 wp-0 имеет seq=2.

   Fix: добавить `completed_task_items: Vec<(u16, String)>` в `LiveAgentRun`
   (пары seq, task_id из активной mission). Заполнять из `SitlTaskProgress`
   в `Px4AgentController` и явно в `FakeLiveAgentController`. Использовать в
   `record_live_agent_run` напрямую, не ища seq в manifest.

   Затронутые файлы: `crates/swarm-examples/src/sitl_supervisor.rs`.

2. **Wildfire success semantics тест**

   Добавить тест, явно фиксирующий: `success == (mapped_ratio >= threshold)`.
   Inline fixture для small-static с контролируемым числом зон. При 100%
   task completion → `mapped_ratio` достигает threshold → success = true.
   Если threshold = 0.8 слишком строгий для medium-dynamic при ограниченных
   тиках — задокументировать это в runner.rs и `docs/BENCHMARK_RESULTS.md`.

3. **Flood scope закрытие**

   Убрать flood из пользовательских обещаний:
   - README Quick Start: «wildfire / flood mapping» → «wildfire mapping»;
   - doc comments `/// Wildfire / Flood Mapping` → `/// Wildfire Mapping`;
   - `docs/STATUS.md`: flood = future work, не partial.

4. **Benchmark artifact sync**

   Проверить, что `results/all_500_jobs14_m62_release/manifest.json` ссылается
   на коммит, который совпадает с текущим HEAD, или явно пометить его как
   historical baseline в `docs/BENCHMARK_RESULTS.md`.

5. **Replay timeline output**

   `replay --timeline`: хронологический список событий с agent_id префиксом
   и `elapsed_ms`. Не визуализация — инструмент отладки.

   ```
   00:00.001 [supervisor] run_started agents=2
   00:04.231 [agent-0]   disconnected
   00:04.232 [supervisor] agent_lost agent=agent-0
   00:04.234 [agent-1]   survivor_mission_update_started
   ```

   `replay --agent <id>`: только события одного агента.

   Затронутые файлы: `crates/swarm-examples/src/bin/replay.rs`.

6. **Local harness script**

   `scripts/run_m58_local.sh` — запускает два PX4 SIH, ждёт порты, запускает
   supervisor, останавливает PX4, кладёт артефакт в `results/local_YYYY-MM-DD/`.
   Аналогично `scripts/run_m59_local.sh` с kill первого PX4 по timeout.
   Не CI. Только локально и явно.

### Не делать

- Не добавлять minimal flood implementation — только cleanup wording.
- Не делать большой replay migration — только новые флаги.

### Done criteria

- replay seq в completion events совпадает с seq replacement mission;
- wildfire success rule покрыт тестом;
- README/docs не упоминают flood как реализованную feature;
- benchmark artifact явно помечен как current или historical;
- `replay --timeline` работает на M58/M59 артефактах.

### Тесты

#### Без рефакторинга

- `fake_live_supervisor_replacement_*`: asserting seq в completion events
  совпадает с seq replacement mission, не с оригинальным manifest.
- Wildfire small-static: `success == true` при expected completion.
- `replay --timeline` golden output на small inline fixture.
- `replay --agent <id>` фильтрует правильно.

#### Лёгкий рефакторинг

- Helper `assert_completion_seq_matches_mission(log, mission_items)`.
- Wildfire fixture builder с контролируемым числом зон и threshold.

#### Тяжёлый рефакторинг

- Benchmark artifact validator: проверяет, что `manifest.json` ссылается
  на коммит без machine-specific пути.

---

## M64 — Urban Patrol v0

### Цель

Один дрон объезжает квартал без нарушений карты. Первая mission-level navigation
с road graph, geometric judge и replay. Без лидара, без автобусов, без
произвольных полигонов.

### Что сделать

1. **Urban Map DSL**

   Новые типы в `crates/swarm-sim/src/urban.rs` или
   `crates/swarm-types/src/urban.rs`:

   ```rust
   pub struct RoadNode { pub id: NodeId, pub pose: Pose }
   pub struct RoadEdge { pub from: NodeId, pub to: NodeId,
                         pub width_m: f64, pub blocked: bool }
   pub struct Building  { pub id: String, pub min: Pose, pub max: Pose }
   pub struct UrbanMap  { pub nodes: Vec<RoadNode>,
                          pub edges: Vec<RoadEdge>,
                          pub buildings: Vec<Building>,
                          pub altitude_m: f64 }
   ```

   Scenario DSL extension — inline в scenario JSON:

   ```json
   "urban_map": {
     "altitude_m": 30.0,
     "nodes": [{ "id": "n0", "pose": { "x": 0.0, "y": 0.0, "z": 0.0 } }],
     "edges": [{ "from": "n0", "to": "n1", "width_m": 8.0, "blocked": false }],
     "buildings": [{ "id": "b0",
                     "min": { "x": 10.0, "y": 10.0, "z": 0.0 },
                     "max": { "x": 40.0, "y": 40.0, "z": 50.0 } }]
   }
   ```

   Сценарии добавить в `scenarios/urban.patrol.json`.

2. **Route planner**

   Dijkstra или A* по road graph. Детерминированный tie-breaking по node id.
   Возвращает `Vec<NodeId>` — ordered patrol loop.

   ```rust
   pub fn plan_patrol_route(
       map: &UrbanMap,
       start: NodeId,
       patrol_nodes: &[NodeId],
   ) -> Result<Vec<NodeId>, UrbanPlanError>;
   ```

   Маршрут планируется один раз до старта миссии. Без local replanning на U1.

3. **Geometric judge**

   Независимый deterministic judge — вызывается runner-ом, не частью миссии:

   ```rust
   pub struct UrbanJudge<'a> { pub map: &'a UrbanMap, pub min_separation_m: f64 }

   impl UrbanJudge<'_> {
       pub fn segment_hits_building(&self, from: Pose, to: Pose) -> Option<&Building>;
       pub fn point_outside_corridors(&self, pose: Pose) -> bool;
       pub fn separation_violated(&self, a: Pose, b: Pose) -> bool;
   }
   ```

   AABB vs segment intersection: стандартная 2D геометрия, без внешних
   зависимостей. Собственная реализация в том же файле.

4. **Urban Patrol mission**

   `TaskKind::Waypoint` переиспользовать если семантика совпадает. Добавить
   `TaskKind::UrbanWaypoint` только если нужна специфичная completion logic.

   `UrbanPatrolAdapter`: `is_completed` = агент достиг node в пределах
   `arrival_radius_m`. `score` = расстояние по route до следующей ноды.

5. **Replay events**

   ```rust
   UrbanRoutePlanned    { agent_id, node_ids: Vec<NodeId>, tick }
   UrbanSegmentEntered  { agent_id, from: NodeId, to: NodeId, tick }
   UrbanViolation       { agent_id, kind: ViolationKind, tick }
   // kind: BuildingCollision | CorridorExit | SeparationBreach
   UrbanPatrolCompleted { agent_id, segments_completed: usize, tick }
   ```

6. **Метрики**

   ```rust
   pub struct UrbanPatrolMetrics {
       pub patrol_completion_rate: f64,
       pub urban_violations: u64,
       pub building_collisions: u64,
       pub corridor_exits: u64,
       pub route_length_m: f64,
       pub route_efficiency: f64,   // optimal_length / actual_length
       pub time_to_complete_loop: Option<u64>,
   }
   ```

7. **Regression smoke**

   Маленький квадратный квартал 4×4 ноды, 1 агент, greedy. Должен завершаться
   без violation. В `cargo test` без subprocess.

8. **Support matrix entry**

   Зафиксировать какие стратегии stable / experimental / unsupported для
   `urban/patrol` профиля.

9. **PX4 export path (если практично)**

   Waypoints вдоль patrol route → `SitlWaypointItem`. Экспортировать в
   `sitl.urban-patrol.config.json`. Прогнать через `sitl_supervisor --dry-run`
   как portable smoke.

### Не делать

- Никакого лидара.
- Никаких автобусов.
- Никакого arbitrary polygon geometry — только AABB для зданий.
- Никакого PX4 requirement для unit тестов.
- Никакой визуализации.
- Никакого local replanning — маршрут фиксируется до старта.

### Done criteria

- Urban patrol scenario загружается через DSL.
- Dijkstra строит loop route для простого квадратного блока детерминированно.
- Judge сообщает building collision при маршруте через здание.
- Judge не сообщает violation при valid route.
- Replay содержит `UrbanRoutePlanned` и `UrbanPatrolCompleted`.
- Метрики экспортируются в JSON/CSV.
- Regression smoke проходит.
- `cargo test -p swarm-examples` green.

### Тесты

#### Без рефакторинга

- Road graph parse/validation из inline fixture.
- Dijkstra возвращает deterministic loop route на simple square block.
- Urban patrol завершается без violation на valid map.
- Judge: building collision при маршруте через здание.
- Judge: corridor exit при выходе за road edges.
- Replay roundtrip для urban events.
- `UrbanPlanError` при disconnected graph.

#### Лёгкий рефакторинг

- Route planning helper shared между runner и тестами.
- Urban map builder для square-block fixture.
- Metrics assertion helper для patrol completion и violations.
- Scenario builder для urban patrol с контролируемым числом нод.

#### Тяжёлый рефакторинг

- Property tests: random road graph → Dijkstra находит путь или возвращает
  `NoPath`.
- Property tests: любой valid route → judge не сообщает violation.
- PX4/SIH dry-run smoke для urban waypoints.

---

## M65 — Urban Search v1

### Цель

Дрон патрулирует до обнаружения автобуса. Первый mock perception event как
first-class concept в simulation layer.

### Что сделать

1. **Bus entity**

   ```rust
   pub struct BusEntity {
       pub id: BusId,
       pub route: Vec<(NodeId, u64)>,   // (node, arrival_tick)
       pub speed_m_per_tick: f64,
       pub appears_at_tick: u64,
       pub disappears_at_tick: Option<u64>,
   }

   impl BusEntity {
       pub fn pose_at_tick(&self, map: &UrbanMap, tick: u64) -> Option<Pose>;
   }
   ```

   Движение: линейная интерполяция по route. Детерминированное по tick.
   Никакой физики — только `pose_at_tick`.

   Добавить в scenario DSL:

   ```json
   "buses": [
     { "id": "bus-0",
       "route": [["n0", 0], ["n1", 20], ["n2", 40]],
       "speed_m_per_tick": 0.5,
       "appears_at_tick": 0 }
   ]
   ```

2. **BusDetector trait**

   ```rust
   pub trait BusDetector {
       fn detect(&self, agent_pose: Pose, bus_pose: Pose,
                 rng: &mut impl Rng) -> DetectionResult;
   }

   pub struct DetectionResult { pub detected: bool, pub false_positive: bool }

   pub struct MockBusDetector {
       pub range_m: f64,
       pub detection_probability: f64,
       pub false_positive_rate: f64,
   }
   ```

   Line-of-sight (проверка building AABB) — опционально, добавить в M67 если
   нужно.

3. **Mission policy**

   Patrol loop → проверить BusDetector на каждом тике → при `detected=true`
   завершить как `BusFound` → при `false_positive=true` продолжить патруль.
   Completion: `BusFound` | `PatrolTimeout` | `Violation`.

4. **Replay events**

   ```rust
   BusObserved              { agent_id, bus_id, agent_pose, bus_pose, tick }
   BusDetectionFalsePositive { agent_id, bus_id, tick }
   UrbanSearchCompleted     { agent_id, outcome: SearchOutcome, tick }
   // outcome: BusFound | Timeout | Violation
   ```

5. **Метрики**

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

6. **Regression smoke**

   Детерминированный: bus появляется на известном тике и известной позиции,
   `detection_probability=1.0`, `false_positive_rate=0.0`. Агент должен
   обнаружить автобус за предсказуемое число тиков.

7. **Support matrix entry**

   Зафиксировать какие стратегии stable для `urban/search` профиля.

### Не делать

- Никакого настоящего CV.
- Никакого image simulation.
- Никакой физической модели автобуса за пределами `pose_at_tick`.
- Никакого multi-agent bus search — сначала стабильный U1/U2 для одного.

### Done criteria

- Bus entity перемещается детерминированно по route.
- `MockBusDetector` возвращает `detected=true` в пределах range при
  `detection_probability=1.0`.
- Mission завершается как `BusFound` при детерминированном сценарии.
- Replay содержит `BusObserved` и `UrbanSearchCompleted`.
- False positive не завершает миссию.
- Regression smoke проходит.

### Тесты

#### Без рефакторинга

- `BusEntity::pose_at_tick` детерминирован.
- `MockBusDetector`: `detected=true` в пределах range.
- `MockBusDetector`: `detected=false` за пределами range.
- Mission `BusFound` при детерминированном bus на маршруте.
- Mission `Timeout` при отсутствующем bus.
- False positive не завершает поиск.
- Replay roundtrip для bus events.

#### Лёгкий рефакторинг

- Bus detection fixture с детерминированным seed.
- Urban search scenario builder с контролируемым bus schedule.
- Outcome assertion helper.

#### Тяжёлый рефакторинг

- Property tests: detection при `probability=1.0` всегда случается в range.
- Multi-seed stability: detection rate при `probability=0.7` сходится к 0.7.
- Line-of-sight building occlusion (если добавляем).

---

## M66 — Algorithm Depth

### Цель

Улучшить алгоритмы координации так, чтобы стратегии давали измеримо разные
результаты. Основная мотивация: сейчас в большинстве профилей greedy не
хуже auction и connectivity-aware. Нужна либо дифференциация стратегий,
либо честная документация что greedy достаточен.

### Что сделать

1. **Communication-aware allocation scoring**

   Текущий gap: `comms_range` существует на `AllocationAgent`, но greedy,
   auction, CBBA и centralized игнорируют его. `ConnectivityAwareAllocator`
   учитывает `comms_range` только для relay placement.

   Добавить `comms_penalty_weight: f64` в scoring (либо в конфиг allocator-а,
   либо в `MissionAdapter::score`). Если расстояние `agent → task > comms_range`,
   score снижается пропорционально: `penalty = comms_penalty_weight * max(0, dist - comms_range)`.

   Benchmark delta: сравнить с `comms_penalty_weight=0` и `>0` на профилях
   heavy-loss-* и partition-prone-* по метрикам `agent_availability`,
   `task_conflicts`, `success`.

   Затронутые файлы: `crates/swarm-alloc/src/allocator.rs`,
   `crates/swarm-types/src/adapter.rs`.

2. **Wildfire priority-triggered reallocation**

   Текущий gap: `push_wildfire_priority_update` пишет событие, но не триггерит
   reallocation. Агент летит к низкоприоритетной задаче, даже если другая зона
   стала критичной.

   При priority update для задачи с `new_priority >= 8` (критический порог)
   — поставить задачу в очередь force-reallocation. Coordinator при следующем
   тике может переназначить агента к более срочной задаче.

   Добавить тест: два агента, одна задача с `priority=2`, одна с `priority=9`.
   После priority update задача `priority=9` должна быть назначена ближайшему
   свободному агенту в том же тике.

   Затронутые файлы: `crates/swarm-runtime/src/coordinator.rs`,
   `crates/swarm-scenarios/src/wildfire.rs`.

3. **SAR belief-entropy driven ordering**

   Текущий gap: `sar_task_priority` статична — вычисляется при генерации
   сценария и не обновляется при изменении belief.

   При каждом scan event обновлять posterior belief для посещённой клетки.
   Пересчитывать priority незавершённых задач по убыванию remaining entropy.
   Добавить `dynamic_belief_updates: bool` в `SarRunConfig` — включать только
   явно, чтобы не ломать determinism текущих тестов.

   Затронутые файлы: `crates/swarm-scenarios/src/sar_scenario.rs`,
   `crates/swarm-runtime/src/runner.rs`.

4. **CBBA convergence диагностика**

   Текущий gap: 6 coverage профилей имеют `success=0.000, completion=1.000`
   под CBBA. Причина неизвестна — нужна диагностика.

   Добавить replay event `CbbaConvergenceEvent { agent_id, iteration,
   bundle_size, conflicting_tasks, tick }`. Проанализировать heavy-loss профили
   через `replay --timeline`. Hypothesis: `gossip_interval_ticks` слишком велик
   при потере агентов.

   Если hypothesis подтверждается — добавить gossip burst при agent loss event
   в `Coordinator`. Если нет — задокументировать как known limitation.

   Затронутые файлы: `crates/swarm-alloc/src/cbba.rs`,
   `crates/swarm-runtime/src/coordinator.rs`.

### Не делать

- Не делать hierarchical coordination (8+ агентов) — нет benchmark evidence
  что нужно.
- Не делать mission-specific planners для Urban Navigation — они входят в
  M64/M65.
- Не стабилизировать публичный API.

### Done criteria

- Unit test: comms_penalty_weight снижает score для агента за пределами
  `comms_range`.
- Wildfire: задача с `priority=9` вытесняет агента от задачи с `priority=2`
  после priority update.
- SAR: `dynamic_belief_updates=true` меняет порядок назначений (покрыто тестом).
- CBBA: replay содержит `CbbaConvergenceEvent` для heavy-loss профилей, причина
  `success=0.000` задокументирована.
- `cargo test --workspace` green.

### Тесты

#### Без рефакторинга

- Unit test: `comms_penalty_weight > 0` → score снижается при `dist > comms_range`.
- Unit test: wildfire `priority >= 8` → force-reallocation flag выставлен.
- SAR belief update: posterior отличается от prior после scan event.
- Support matrix tests для unsupported пар (существующие, должны остаться green).

#### Лёгкий рефакторинг

- Scoring comparison helper с детерминированными inputs.
- Wildfire fixture с контролируемыми priority updates и агентами.
- SAR belief fixture с явным prior/posterior.
- CBBA convergence replay fixture.

#### Тяжёлый рефакторинг

- Multi-seed benchmark delta: comms scoring с/без penalty.
- SAR dynamic priority benchmark delta.
- Property tests: CBBA bundle consistency под arbitrary message loss.

---

## M67 — Benchmark Refresh

### Цель

Обновить simulation benchmark claims после Urban Navigation и Algorithm Depth.
Закрыть открытые интерпретационные вопросы из M62 BENCHMARK_RESULTS.md.

### Что сделать

1. **Закрыть интерпретационные вопросы из M62**

   SAR success ≈ 0: добавить `pod_success_threshold: f64` в `SarRunConfig`.
   `success = (pod >= pod_success_threshold)` вместо `all_targets_found()`.
   Документально: SAR success = probability-of-detection metric, не "все цели
   найдены".

   Wildfire success = 0.247: добавить profile-specific threshold или вынести
   `zones_mapped_rate` как отдельную метрику рядом с `success`. Задокументировать
   в BENCHMARK_RESULTS.md.

   CBBA coverage 6 failed профилей: после M66 CBBA диагностики — либо fix,
   либо явная документация в support matrix.

2. **1000-seed release run**

   Только для supported mission-strategy пар. Unsupported остаются явно
   unsupported.

   ```bash
   cargo build --release -p swarm-examples --bin strategy_comparison
   target/release/strategy_comparison \
     --seeds 1000 --mission all --jobs 14 \
     --output-dir results/all_1000_jobs14_m67_release
   ```

3. **Confidence intervals**

   Добавить `mean ± stderr` для всех major метрик в JSON/CSV/Markdown export.
   `stderr = stddev / sqrt(n)`.

4. **Degradation curves**

   Добавить scenario suites для sweep:
   - `success` vs `agents_count` (2, 4, 8) для coverage и wildfire;
   - `success` vs `packet_loss_rate` (0.0, 0.1, 0.3, 0.5) для heavy-loss profiles;
   - `urban_violations` vs `obstacle_density` для Urban Patrol.

5. **Обновить документацию**

   `docs/BENCHMARK_RESULTS.md`, README status table, `docs/STATUS.md`.
   Явно отделить: simulation benchmark, SITL evidence, unsupported claims.

### Не делать

- Не включать unsupported пары как success claims.
- Не делать paper-level statistical analysis (p-value, effect size) если это
  не явная цель.
- Не делать 1000-seed до закрытия интерпретационных вопросов.

### Done criteria

- SAR success semantics задокументированы и покрыты тестом.
- Wildfire success/zones_mapped_rate разделены в метриках.
- 1000-seed artifact в `results/all_1000_jobs14_m67_release/`.
- Confidence intervals присутствуют в экспортах.
- Хотя бы один degradation sweep существует как artifact.
- BENCHMARK_RESULTS.md обновлён и не ссылается на M62 как на current если
  в simulation коде были изменения.

### Тесты

#### Без рефакторинга

- Существующие benchmark export tests.
- Manifest/report identity tests.
- Regression runner default suite.
- SAR success с `pod_success_threshold`: detects `success=true` при
  `pod >= threshold`.

#### Лёгкий рефакторинг

- Confidence interval helper тест.
- Benchmark pack validation helper.
- Degradation suite runner helper.

#### Тяжёлый рефакторинг

- Statistical delta report validation.
- Multi-pack comparison tooling.
- Long-run reproducibility harness.

---

## M68 — Next Branch Decision

### Цель

После M63–M67 осознанно выбрать следующее стратегическое направление.

На этом этапе у проекта будет:

- simulation foundation + regression gate;
- single-agent и multi-agent PX4/SIH execute evidence;
- controlled live failure/reallocation evidence;
- mission-level urban navigation (patrol + search);
- дифференцированные алгоритмы с измеримыми преимуществами;
- 1000-seed benchmark с confidence intervals;
- replay timeline для анализа runs.

### Возможные направления

**Option A — Urban Avoidance / Multi-Agent**

- Временные препятствия на road graph.
- Mock lidar-like range detector (geometry query, не raycast).
- Replan policy: stop / replan / yield.
- Два дрона на одном map, separation enforcement.
- Метрики: `avoided_collisions`, `replan_count`, `near_miss_count`.

Выбирать если: хочется добавить reactive decision-making к уже работающему
Urban workflow.

**Option B — New Mission (Logistics / Delivery)**

- `TaskKind::Pickup` / `TaskKind::Dropoff` с `requires_pickup`.
- Cargo capacity limits.
- Precedence validation в allocators.
- Проверяет extension path на mission с task dependencies.

Выбирать если: цель — стресс-тест DSL и allocators на stateful task dependencies.

**Option C — Algorithm Depth продолжение**

- Hierarchical coordination если benchmark покажет что нужно.
- More robust CBBA если диагностика M66 вскроет fixable issue.
- Communication-aware allocation с более богатой penalty model.

Выбирать если: M67 benchmark покажет что конкретные стратегии значимо хуже
в определённых условиях и есть гипотеза как исправить.

**Option D — Research Benchmark / Publication**

- 1000-seed уже есть после M67.
- Degradation curves, strategy comparison report.
- Publication-level claims если A/B/C уже сделаны.

Выбирать как последний шаг перед внешней публикацией.

---

## Сводная таблица

| Milestone | Результат | Зависит от | Параллельно |
|---|---|---|---|
| **M63** | Evidence cleanup, replay timeline, local harness | M62 | — |
| **M64** | Urban Patrol v0: road graph, judge, patrol mission | M63 | M66 |
| **M65** | Urban Search v1: bus entity, mock detector | M64 | — |
| **M66** | Algorithm Depth: comms scoring, wildfire/SAR planners, CBBA diagnostics | M63 | M64 |
| **M67** | Benchmark Refresh: 1000-seed, confidence intervals, degradation curves | M65, M66 | — |
| **M68** | Next Branch Decision | M67 | — |
