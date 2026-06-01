# BEFORE_HARDWARE_B.22 — Что делать без железа чтобы приблизиться к боевому уровню

Дата фиксации: 2026-05-31

## Контекст

Проект сейчас — исследовательский симулятор с хорошей инфраструктурой (TRL 3–4).
PX4/SIH workflow доказан для single-agent и multi-agent. Железа нет и в ближайшее
время не будет.

Цель этого плана: без железа поднять проект до уровня, на котором алгоритмы
**измеримо дифференцированы**, benchmark **читаем и credible**, а **путь к
железу** не потребует переписывания при его появлении.

```text
BH1  Benchmark Credibility
  -> BH2  Algorithm Differentiation
    -> BH3  Hardware Path Preparation   (параллельно с BH2, от него не зависит)
      -> BH4  Mission Realism
```

BH1 и BH3 независимы, можно параллельно. BH2 нужен до BH4 (нет смысла добавлять
dynamic bus пока алгоритмы не дифференцированы — нечего измерять).

---

## Текущие проблемы, которые блокируют "серьёзность"

1. SAR success ≈ 0 у всех стратегий → таблицы benchmark нечитаемы.
2. CBBA coverage 6 профилей: `success=0.000, completion=1.000` — необъяснённо.
3. Greedy ≈ auction ≈ connectivity-aware → непонятно зачем сложные алгоритмы.
4. `Pose.x/y` — локальные метры, но `MissionHomeOrigin` hardcode Цюрих → при
   смене полигона операции нужно лезть в код.
5. Нет confidence intervals → числа выглядят анекдотически.

---

## BH1 — Benchmark Credibility

### Цель

Сделать benchmark читаемым и credible без новых алгоритмов.

Текущее состояние: 1000-seed артефакт есть, но SAR строки показывают `success
≈ 0` у всех стратегий, CBBA gaps не объяснены, в экспортах нет статистики
дисперсии. Внешний человек, увидев таблицы, решит что проект не работает.

### B1.1 — SAR success threshold

**Проблема:** `sar_success` в runner.rs:277 использует `gs.all_targets_found()`,
что требует 100% обнаружения при `detection_probability < 1.0`. В текущих
профилях `success ≈ 0` у всех стратегий. Метрика `probability_of_detection`
уже есть в `RunMetrics` и заполняется корректно — нужно просто использовать
её в success predicate.

**Что сделать:**

1. Добавить `sar_success_threshold: f64` в `RunConfig` рядом с
   `wildfire_success_threshold` (default: 1.0 для обратной совместимости).
2. В `compute_mission_success` заменить `gs.all_targets_found()` на:
   ```rust
   let pod = gs.targets_found as f64 / gs.targets.len().max(1) as f64;
   let sar_success = pod >= sar_success_threshold
       && all_expected_failures_detected
       && max_task_unassigned_ticks <= max_unassigned_ticks_config;
   ```
3. В scenario profiles SAR (`ideal`, `standard`) установить
   `sar_success_threshold: 0.8`.
4. Doc comment в runner.rs: SAR success = probability-of-detection metric,
   не "все цели найдены".

**Затронутые файлы:**
- `crates/swarm-sim/src/runner.rs` — `compute_mission_success`, `RunConfig`
- `crates/swarm-scenarios/src/sar_scenario.rs` — добавить threshold в профили

**Тесты:**

Без рефакторинга:
- `sar_success_threshold_0_8_succeeds_with_partial_detection`: при 80%
  найденных целях и `threshold=0.8` → `success=true`.
- `sar_success_threshold_1_0_requires_all_found`: при `threshold=1.0` и 80%
  → `success=false` (обратная совместимость).
- `support_matrix_sar_greedy_with_threshold`: существующий тест переписать
  с явным threshold.

### B1.2 — Confidence intervals в экспортах

**Проблема:** `AggregateMetrics` содержит только mean. В JSON/CSV/Markdown нет
stddev, stderr, min, max. 1000-seed run без интервалов выглядит анекдотически.

**Что сделать:**

1. Добавить в `AggregateMetrics` поля `stddev_success_rate: f64` и
   `stderr_success_rate: f64`. Для v1 достаточно success_rate — самая
   важная метрика. Остальные при необходимости.
2. В `AggregateMetrics::from_runs` вычислять:
   ```rust
   let mean = sum / n;
   let variance = runs.iter().map(|r| (r.success as f64 - mean).powi(2)).sum::<f64>() / n;
   let stddev = variance.sqrt();
   let stderr = stddev / n.sqrt();
   ```
3. Добавить в Markdown export строку `±stderr` рядом с mean.
4. Добавить в JSON/CSV поля `stddev_success_rate`, `stderr_success_rate`.

**Затронутые файлы:**
- `crates/swarm-metrics/src/metrics.rs`
- `crates/swarm-sim/src/report_export.rs`

**Тесты:**

Без рефакторинга:
- `aggregate_stderr_is_zero_for_uniform_runs`: все runs с одним результатом
  → `stderr=0`.
- `aggregate_stderr_matches_formula`: 4 runs с known values → проверить
  `stderr = stddev / sqrt(4)` численно.
- `report_export_markdown_contains_stderr`: Markdown вывод содержит `±`.

### B1.3 — CBBA convergence диагностика

**Проблема:** 6 coverage профилей CBBA: `success=0.000, completion=1.000`.
Причина неизвестна. Текущие replay события (`CbbaConverged`,
`CbbaBundleUpdated`) не содержат `conflicting_tasks`. Без этого поля
невозможно понять через replay почему консенсус ломается.

**Что сделать:**

1. Добавить в `CbbaBundleUpdated` опциональное поле
   `conflicting_task_count: u64` — число задач, по которым в этом тике был
   конфликт с другим агентом. Вычисляется в cbba.rs при merge bids.
2. Добавить event `CbbaGossipBurst { agent_id, tick, reason: String }` —
   эмитируется если добавить gossip burst при agent failure (см. ниже).
3. Прогнать `replay --timeline --category generic` на одном heavy-loss
   профиле и выяснить: теряют ли агенты консенсус после failure event?
4. Проверить hypothesis: при `AgentFailed` event CBBA не успевает
   переконвергировать за `gossip_interval_ticks`. Fix: при обнаружении
   `AgentFailed` в coordinator — один внеплановый gossip round для CBBA.
5. Если hypothesis подтверждается — патч в coordinator. Если нет —
   задокументировать как inherent limitation в support matrix с replay
   evidence.

**Затронутые файлы:**
- `crates/swarm-replay/src/event_log.rs` — новое поле в `CbbaBundleUpdated`
- `crates/swarm-alloc/src/cbba.rs` — подсчёт conflicting_task_count
- `crates/swarm-runtime/src/coordinator.rs` — gossip burst at agent failure

**Тесты:**

Без рефакторинга:
- `cbba_bundle_updated_has_conflict_count`: при конфликте двух агентов за
  одну задачу → `conflicting_task_count >= 1`.
- Существующий `cbba_round_assignments_converge` остаётся green.

Лёгкий рефакторинг:
- CBBA fixture с явным agent failure mid-run: проверить, что gossip burst
  ускоряет реконвергенцию (если patch добавлен).

### Done criteria BH1

- SAR benchmark строки показывают non-zero success при `threshold=0.8`.
- JSON/CSV/Markdown экспорт содержит `stddev_success_rate`, `stderr_success_rate`.
- Причина `success=0` у CBBA heavy-loss профилей задокументирована или
  пофикшена с replay evidence.
- `cargo test --workspace` green.

---

## BH2 — Algorithm Differentiation

### Цель

Сделать так чтобы стратегии давали **измеримо разные** результаты в
подходящих условиях. Сейчас greedy ≈ всем остальным — непонятно зачем
использовать что-то сложнее.

### B2.1 — Communication-aware allocation scoring

**Проблема:** `AllocationAgent.comms_range` хранится и передаётся, но ни
один allocator (greedy, auction, CBBA, centralized) не использует его в
scoring. В тестах `comms_range = f64::INFINITY`. `AuctionAllocator.cost()`
учитывает только distance, battery, role.

**Что сделать:**

1. Добавить `comms_penalty_weight: f64` в `RunConfig` (default 0.0 — off).
2. В greedy и auction scoring добавить penalty:
   ```rust
   let dist = agent.pose.distance_to(&task_pose);
   let comms_penalty = if dist > agent.comms_range {
       comms_penalty_weight * (dist - agent.comms_range)
   } else {
       0.0
   };
   ```
3. Benchmark delta: прогнать coverage heavy-loss-* и partition-prone-* с
   `comms_penalty_weight=0` и `comms_penalty_weight=1.0`. Ожидаемый
   результат: connectivity-aware получает advantage в потере-связных профилях.
4. Добавить `avg_comms_penalty_weight` в manifest/report.

**Затронутые файлы:**
- `crates/swarm-alloc/src/allocator.rs` — `AuctionAllocator.cost()`,
  `GreedyAllocator.allocate()`
- `crates/swarm-sim/src/runner.rs` — `RunConfig`, передача в allocator

**Тесты:**

Без рефакторинга:
- `comms_penalty_reduces_score_beyond_range`: агент с `comms_range=10.0`
  и задача на расстоянии 20.0 → score ниже чем при `comms_range=∞`.
- `comms_penalty_zero_no_effect`: при `weight=0.0` поведение идентично
  текущему (regression guard).
- `comms_penalty_infinite_range_no_effect`: при `comms_range=∞` penalty=0
  независимо от расстояния.

### B2.2 — Wildfire priority-triggered reallocation

**Проблема:** `push_wildfire_priority_update` в runner.rs пишет событие
`TaskPriorityUpdated` в replay и инкрементирует `priority_updates`, но не
триггерит reallocation. Агент продолжает лететь к задаче с `priority=2`
даже если другая зона стала `priority=9`.

**Что сделать:**

1. Добавить `wildfire_priority_realloc_threshold: u8` в `RunConfig` (default 8).
2. В runner, при wildfire priority update: если `new_priority >= threshold` —
   добавить `task.id` в `force_realloc_queue: HashSet<TaskId>`.
3. В начале следующего тика coordinator проверяет `force_realloc_queue`:
   освобождает текущее назначение агента к этой задаче, возвращает задачу
   в `Unassigned`, очищает очередь.
4. Тест: два агента, задача A с `priority=2` (у агента-0), задача B с
   `priority=1`. После priority update задачи B до `priority=9` — агент-0
   должен быть переназначен на B в следующем тике.

**Затронутые файлы:**
- `crates/swarm-sim/src/runner.rs` — wildfire tick loop, `RunConfig`
- `crates/swarm-runtime/src/coordinator.rs` — force realloc механизм

**Тесты:**

Без рефакторинга:
- `wildfire_priority_trigger_reallocates_agent`: при `priority >= threshold`
  агент покидает текущую задачу и переходит к высокоприоритетной.
- `wildfire_priority_below_threshold_no_realloc`: при `priority < threshold`
  реаллокации нет.
- `wildfire_priority_threshold_configurable`: разные threshold дают разное
  поведение в детерминированном сценарии.

### B2.3 — SAR belief-entropy ordering

**Проблема:** `sar_task_priority` статичен — задаётся при генерации сценария
и не обновляется по мере сканирования. Агент не узнаёт что клетка уже
частично обследована и её приоритет изменился.

**Что сделать:**

1. Добавить `dynamic_belief_updates: bool` в `SarRunConfig` (default false —
   не ломает существующие тесты).
2. При `dynamic_belief_updates=true`: после каждого scan event пересчитать
   posterior belief для посещённой клетки. Незавершённые задачи ранжировать
   по убыванию remaining uncertainty: `priority ∝ prior * (1 - detection_prob)`.
3. Обновлять `task.priority` в `task_registry` — coordinator учтёт при
   следующем allocation round.
4. Тест: сценарий с двумя клетками, одна с high prior, одна с low prior.
   После сканирования high-prior клетки — агент переключается на low-prior
   (которая теперь имеет higher remaining uncertainty).

**Затронутые файлы:**
- `crates/swarm-scenarios/src/sar_scenario.rs` — `SarRunConfig`
- `crates/swarm-sim/src/runner.rs` — SAR scan tick loop

**Тесты:**

Без рефакторинга:
- `sar_dynamic_belief_updates_change_task_order`: при `dynamic=true` порядок
  назначений отличается от статичного.
- `sar_static_belief_unchanged_with_flag_false`: при `dynamic=false` поведение
  идентично текущему.

### B2.4 — После BH2: benchmark delta

После B2.1–B2.3 прогнать targeted benchmark:
- coverage heavy-loss с/без `comms_penalty_weight`.
- wildfire medium-dynamic с/без priority realloc.
- SAR с/без dynamic belief.

Зафиксировать delta в `docs/BENCHMARK_RESULTS.md` с интерпретацией: где
сложные алгоритмы выигрывают и почему.

### Done criteria BH2

- Unit тест: comms_penalty_weight снижает score при `dist > comms_range`.
- Wildfire: агент перераспределяется при priority ≥ threshold (детерминированный тест).
- SAR: `dynamic_belief_updates=true` меняет порядок назначений (тест).
- Benchmark delta committed с интерпретацией.
- `cargo test --workspace` green.

---

## BH3 — Hardware Path Preparation

### Цель

Убрать технический blocker для железа не трогая железо. Когда железо
появится — не должно быть "ой, нам нужно переписать coordinate frame".

### Текущее состояние

**Хорошая новость:** математика уже есть. В `swarm-comms/src/mavlink.rs`
существуют `local_to_lat_deg`, `local_to_lon_deg`, `MissionHomeOrigin` с
полями `lat_deg`, `lon_deg`. Конвертация работает для PX4/SIH с hardcode
Цюриха (`lat=47.397742, lon=8.545594`).

**Проблема:** `MissionHomeOrigin` не доступен из scenario DSL. В `RunConfig`,
`Scenario`, `sitl_supervisor` нет способа задать координаты реального полигона
операции. При смене локации нужно менять код.

### B3.1 — Configurable geo_origin в scenario DSL

**Что сделать:**

1. Добавить в `Scenario` опциональный `geo_origin`:
   ```rust
   pub struct GeoOrigin {
       pub lat_deg: f64,
       pub lon_deg: f64,
       pub alt_m: f64,
   }
   // в Scenario:
   pub geo_origin: Option<GeoOrigin>,
   ```
2. В `sitl_agent` и `sitl_supervisor`: при наличии `scenario.geo_origin`
   передавать его в `MissionUploadOptions.home_origin` вместо default.
3. Добавить в `scenarios/sitl.px4-golden.json` явный `geo_origin` с
   текущим PX4 SIH default (Цюрих) — поведение не меняется, но поле
   становится видимым.
4. Добавить в `scenarios/sitl.multi-agent.json` тот же origin.
5. Dry-run тест: сценарий с `geo_origin { lat=55.75, lon=37.62 }` (Москва)
   → dry-run вывод содержит waypoints в этих координатах, без PX4.

**Затронутые файлы:**
- `crates/swarm-types/src/lib.rs` или новый `geo.rs` — `GeoOrigin`
- `crates/swarm-sim/src/dsl.rs` — parse `geo_origin`
- `crates/swarm-examples/src/sitl_plan.rs` — передача в `MissionUploadOptions`
- `crates/swarm-examples/src/sitl_supervisor.rs` — аналогично
- `scenarios/sitl.px4-golden.json`, `scenarios/sitl.multi-agent.json`

**Тесты:**

Без рефакторинга:
- `geo_origin_overrides_default_in_dry_run`: waypoint lat/lon совпадают с
  origin + local offset, а не с hardcode Цюрихом.
- `geo_origin_absent_uses_sitl_default`: без поля — текущее поведение.
- `geo_origin_roundtrip_json`: сериализация/десериализация без потерь.

### B3.2 — Local harness scripts для M58/M59

**Проблема:** воспроизвести M58/M59 прогон требует ручных шагов. Нет
документированного порядка, нет script'ов. Для любого нового человека
(и для самого себя через месяц) это барьер.

**Что сделать:**

1. Создать `scripts/run_m58_local.sh`:
   ```bash
   #!/usr/bin/env bash
   # Запускает два PX4 SIH, supervisor, собирает артефакт.
   # Использование: ./scripts/run_m58_local.sh [output_dir]
   ```
   - Запускает PX4 SIH instance 1 (`--instance 0`) и instance 2 (`--instance 1`)
     в background с known PIDs.
   - Ждёт порты 14550 и 14560 (`nc -z localhost PORT`).
   - Запускает `sitl_supervisor --connection --execute ...` с `--output-dir`.
   - Убивает PX4 процессы по завершению (`trap cleanup EXIT`).
   - Кладёт артефакт в `results/local_m58_$(date +%Y-%m-%d)/`.

2. Создать `scripts/run_m59_local.sh` — аналогично, плюс:
   - Kill первого PX4 через N секунд после старта (`sleep N && kill $PX4_PID_1 &`).
   - Supervisor детектирует потерю агента и реаллоцирует.

3. Добавить секцию "Локальное воспроизведение M58/M59" в `docs/SITL_SETUP.md`.

**Затронутые файлы:**
- `scripts/run_m58_local.sh` (новый)
- `scripts/run_m59_local.sh` (новый)
- `docs/SITL_SETUP.md`

**Тесты:** нет автоматических (скрипты требуют PX4). Проверяется вручную.

### Done criteria BH3

- `scenarios/sitl.px4-golden.json` содержит явный `geo_origin`.
- Dry-run с `geo_origin { lat=55.75, lon=37.62 }` выводит корректные
  глобальные координаты без PX4.
- `scripts/run_m58_local.sh` и `run_m59_local.sh` существуют и документированы.
- Поведение M58/M59 с `geo_origin` из scenario идентично hardcode (regression).

---

## BH4 — Mission Realism

### Цель

Добавить mission types и механики, которые делают симуляцию ближе к реальным
задачам. BH4 разумно делать после BH2 — нет смысла добавлять движущийся bus
пока алгоритмы не дифференцированы и нечего на нём мерить.

### B4.1 — Динамический автобус с `pose_at_tick`

**Проблема:** `UrbanBus` статичен — одна поза с `active_from_tick/until_tick`.
Реальная цель для urban search движется по маршруту. Детектировать статичный
объект — задача проще чем детектировать движущийся.

**Что сделать:**

1. Добавить опциональный `route` в `UrbanBus`:
   ```rust
   pub struct UrbanBusStop {
       pub node_id: UrbanNodeId,
       pub arrival_tick: u64,
   }
   pub struct UrbanBusRoute {
       pub stops: Vec<UrbanBusStop>,
       pub speed_m_per_tick: f64,
   }
   // в UrbanBus:
   pub route: Option<UrbanBusRoute>,
   ```
   Поле опциональное — статичный bus остаётся как `route: None`.

2. Добавить `UrbanBus::pose_at_tick(map: &UrbanMap, tick: u64) -> Option<Pose>`:
   - Без route: возвращает `Some(self.pose)` если bus активен, иначе `None`.
   - С route: линейная интерполяция между `stops[i]` и `stops[i+1]` по tick.
     Если tick < stops[0].arrival_tick или > stops.last().arrival_tick — `None`.

3. В `detect_buses` заменить `bus.pose` на `bus.pose_at_tick(map, tick)`.

4. Добавить в scenario DSL:
   ```json
   "buses": [{
     "id": "bus-0",
     "pose": { "x": 0.0, "y": 0.0, "z": 0.0 },
     "route": {
       "stops": [
         { "node_id": "n0", "arrival_tick": 0 },
         { "node_id": "n1", "arrival_tick": 20 },
         { "node_id": "n2", "arrival_tick": 40 }
       ],
       "speed_m_per_tick": 0.5
     }
   }]
   ```

**Затронутые файлы:**
- `crates/swarm-types/src/urban.rs` — `UrbanBusRoute`, `UrbanBusStop`
- `crates/swarm-sim/src/urban.rs` — `pose_at_tick`, `detect_buses`
- `crates/swarm-scenarios/src/urban.rs` — search scenario с moving bus

**Тесты:**

Без рефакторинга:
- `bus_pose_at_tick_static_returns_fixed_pose`: без route — всегда одна поза.
- `bus_pose_at_tick_interpolates_between_stops`: tick=10 на пути n0(t=0)→n1(t=20)
  → поза на полпути между n0 и n1.
- `bus_pose_at_tick_returns_none_outside_window`: tick вне диапазона → `None`.
- `detect_buses_finds_moving_bus_when_in_range`: агент на пересечении маршрута
  bus в нужный tick → детектирует.
- `detect_buses_misses_moving_bus_out_of_range`: тот же агент, другой tick →
  bus в другом месте → не детектирует.
- Обратная совместимость: существующие `SearchStaticBus` тесты проходят без
  изменений.

### B4.2 — Временные препятствия на road graph

**Проблема:** road graph статичен после загрузки. Нет возможности тестировать
replan/yield поведение без полноценной avoidance системы. Временные blocked
edges — минимальное изменение, открывающее этот класс сценариев.

**Что сделать:**

1. Добавить в `UrbanState`:
   ```rust
   pub struct UrbanTemporaryObstacle {
       pub edge_id: UrbanEdgeId,
       pub appears_at_tick: u64,
       pub disappears_at_tick: Option<u64>,
   }
   // в RunConfig.urban_state:
   pub temporary_obstacles: Vec<UrbanTemporaryObstacle>,
   ```

2. В runner per-tick: вычислять effective blocked set:
   ```rust
   let blocked_edges: HashSet<UrbanEdgeId> = map.edges.iter()
       .filter(|e| e.blocked)
       .chain(urban_state.temporary_obstacles.iter()
           .filter(|o| o.is_active(tick))
           .filter_map(|o| map.edge(&o.edge_id)))
       .map(|e| e.id.clone())
       .collect();
   ```

3. Добавить replay events:
   ```rust
   UrbanEdgeBlocked   { edge_id: UrbanEdgeId, tick: u64 }
   UrbanEdgeUnblocked { edge_id: UrbanEdgeId, tick: u64 }
   ```

4. Тест: patrol route проходит через edge, которая блокируется на тик 5.
   До тика 5 — route valid. После тика 5 — judge должен зафиксировать
   если агент пытается пройти через неё (зависит от policy).

**Это prerequisite для replan/yield политик** из `BRANCHES_B.22.md`. Сами
политики — отдельная работа после этого milestone.

**Затронутые файлы:**
- `crates/swarm-types/src/urban.rs` — `UrbanTemporaryObstacle`
- `crates/swarm-sim/src/runner.rs` — per-tick effective blocked set
- `crates/swarm-replay/src/event_log.rs` — два новых события

**Тесты:**

Без рефакторинга:
- `temporary_obstacle_is_active_within_window`: `is_active(tick)` возвращает
  true между appears и disappears.
- `temporary_obstacle_no_disappears_stays_forever`: если `disappears_at_tick =
  None` — active до конца run.
- `runner_emits_edge_blocked_event`: при появлении obstacle в replay есть
  `UrbanEdgeBlocked`.
- `runner_emits_edge_unblocked_event`: при исчезновении — `UrbanEdgeUnblocked`.
- `judge_sees_blocked_edge_as_violation`: если агент на заблокированном
  сегменте — `urban_violation_count > 0`.

### B4.3 — Perimeter Patrol

**Проблема:** нет mission типа, который напрямую отображается на реальную задачу
охраны периметра. Это единственный из предложенных новых mission-кандидатов,
который без изменений совместим с SITL pipeline: waypoints вдоль периметра
уходят напрямую в `sitl_supervisor`.

**Суть:** полигон задаёт периметр объекта → builder генерирует waypoints вдоль
рёбер с заданным spacing → агент обходит их как обычный patrol.

**Что сделать:**

1. Добавить в scenario DSL:
   ```json
   "perimeter_patrol": {
     "polygon": [
       { "x": 0.0, "y": 0.0, "z": 0.0 },
       { "x": 100.0, "y": 0.0, "z": 0.0 },
       { "x": 100.0, "y": 100.0, "z": 0.0 },
       { "x": 0.0, "y": 100.0, "z": 0.0 }
     ],
     "waypoint_spacing_m": 20.0,
     "altitude_m": 30.0
   }
   ```

2. Builder `perimeter_waypoints(polygon: &[Pose], spacing: f64) -> Vec<Pose>`:
   - Обходит рёбра полигона.
   - Размещает waypoints с шагом `spacing` вдоль каждого ребра.
   - Последний waypoint = первый (замкнутый маршрут).
   - Детерминированный порядок.

3. `PerimeterPatrolAdapter`: задачи генерируются из waypoints. Completion:
   все waypoints посещены. Success: completion + все expected failures detected.

4. Метрики:
   - `perimeter_completion_rate: f64`
   - `perimeter_length_m: f64`
   - `time_to_complete_perimeter: Option<u64>`
   - `perimeter_violations: u64`

5. Regression smoke: квадрат 100×100 м, spacing 20 м → 20 waypoints,
   1 агент, greedy, должен завершить без violation.

6. PX4 dry-run smoke: `sitl_supervisor --dry-run` с perimeter scenario →
   список waypoints в глобальных координатах (если geo_origin из BH3 есть).

**Затронутые файлы:**
- `crates/swarm-scenarios/src/` — новый `perimeter.rs`
- `crates/swarm-sim/src/` — `PerimeterPatrolAdapter` или extension через `MissionAdapter`
- `crates/swarm-metrics/src/metrics.rs` — новые поля
- `scenarios/perimeter.patrol.json`

**Тесты:**

Без рефакторинга:
- `perimeter_waypoints_square_correct_count`: квадрат 100×100, spacing=20 →
  ровно 20 waypoints (4 стороны × 5 waypoints).
- `perimeter_waypoints_is_deterministic`: два вызова → одинаковый результат.
- `perimeter_waypoints_first_equals_last`: замкнутый маршрут.
- `perimeter_patrol_completes_on_square`: regression smoke.
- `perimeter_patrol_metrics_exported`: JSON содержит `perimeter_completion_rate`.

Лёгкий рефакторинг:
- Shared builder для произвольного полигона (convex и concave).

Тяжёлый рефакторинг:
- Property test: любой convex полигон → все waypoints на рёбрах, не внутри.
- Multi-agent: разделить периметр на disjoint сегменты между агентами.
- PX4 dry-run integration с geo_origin.

### Done criteria BH4

- `UrbanBus::pose_at_tick` возвращает интерполированную позу для moving bus.
- Urban Search с dynamic bus детектирует его только когда пересекаются позиции.
- `UrbanTemporaryObstacle` активируется/деактивируется по tick, replay содержит события.
- Perimeter patrol завершается детерминированно на simple square.
- Все новые тесты portable (нет machine-specific зависимостей).
- `cargo test --workspace` green.

---

## Итоговый порядок работы

```text
BH1  Benchmark Credibility           (2–3 дня, делать первым — immediate credibility)
  B1.1  SAR success threshold        (низкая сложность)
  B1.2  Confidence intervals         (низкая сложность)
  B1.3  CBBA diagnostics             (средняя — диагностика + возможный patch)

BH3  Hardware Path Preparation       (параллельно с BH1 или сразу после)
  B3.1  Configurable geo_origin      (низкая — математика уже есть)
  B3.2  Local harness scripts        (низкая — без Rust кода)

BH2  Algorithm Differentiation       (после BH1 — нужен readable benchmark для delta)
  B2.1  Comms-aware scoring          (средняя)
  B2.2  Wildfire priority realloc    (средняя)
  B2.3  SAR belief-entropy ordering  (средняя)
  B2.4  Benchmark delta              (прогон + документация)

BH4  Mission Realism                  (после BH2 — есть что мерить)
  B4.1  Динамический bus             (средняя)
  B4.2  Временные препятствия        (средняя — prerequisite для replan)
  B4.3  Perimeter Patrol             (средняя — самая "боевая" новая mission)
```

## Что не делать в этом плане

- **UI/visualizer** — не приближает к железу. Другая задача.
- **Hierarchical coordination (8+ агентов)** — нет benchmark evidence о необходимости.
- **Polygon geometry / lidar raycast** — риск превратиться в geometry engine.
- **Published API / semver** — пока нет внешних пользователей.
- **Logistics/Delivery mission** — интересна, но не "боевая" без working
  precedence constraints в allocator. После BH4.
- **1000-seed rerun сразу** — делать после BH2, когда алгоритмы дифференцированы.
