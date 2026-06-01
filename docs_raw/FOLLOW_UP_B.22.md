# FOLLOW_UP_B.22 — Что взять из DRONE_A.21 и DRONE_B.21

Дата фиксации: 2026-05-31

Источники: `docs_raw/DRONE_A.21.md`, `docs_raw/DRONE_B.21.md`.

Документ фиксирует конкретные нереализованные items из этих планов, которые
имеет смысл взять в следующие milestones. Упорядочено по убыванию ROI.

Что не включено: speculative или требующее предварительного evidence (scaling
experiments, line-of-sight, multi-agent deconfliction policy, PX4 urban export).
Они остаются в `BRANCHES_B.22.md` как отдельные ветки.

---

## 1. SAR success threshold fix

**Источник:** B.21 M67, A.21 M69.

**Проблема:** `success ≈ 0` для всех стратегий в SAR benchmark, потому что
`all_targets_found()` требует 100% обнаружения при `detection_probability < 1.0`.
SAR строки в таблицах нечитаемы.

**Что сделать:**

1. Добавить `pod_success_threshold: f64` в `SarRunConfig` (default 1.0 для
   обратной совместимости; в benchmark profiles использовать 0.8 или 0.5).
2. Изменить success predicate в runner:
   вместо `gs.all_targets_found()` →
   `targets_found_rate >= pod_success_threshold`.
3. Один тест: при `pod_success_threshold=0.5` и ≥50% найденных целей
   `success=true`.
4. Doc comment в runner.rs: SAR success = probability-of-detection metric,
   не "все цели найдены".

**Затронутые файлы:** `crates/swarm-scenarios/src/sar_scenario.rs`,
`crates/swarm-sim/src/runner.rs`, `crates/swarm-examples/tests/support_matrix.rs`.

**Сложность:** низкая. Одно поле, одно условие, один тест.

---

## 2. Confidence intervals в benchmark

**Источник:** B.21 M67, A.21 M69.

**Проблема:** текущие экспорты (JSON/CSV/Markdown) содержат только mean.
Нет `stddev`, `stderr`, `min`, `max`. M69 1000-seed артефакт не дотягивает
до research-quality без интервалов.

**Что сделать:**

1. Добавить в `AggregateMetrics` поля `stddev_*` и `stderr_*` для key метрик
   (success_rate, avg_detection_ticks, avg_coverage_progress и др.):
   `stderr = stddev / sqrt(n)`.
2. Обновить JSON/CSV/Markdown экспорт.
3. Тест: `stderr` вычисляется корректно для trivial fixture (n=4, known values).

**Затронутые файлы:** `crates/swarm-metrics/src/metrics.rs`,
`crates/swarm-sim/src/report_export.rs`.

**Сложность:** низкая по коду, средняя по охвату экспортов.

---

## 3. Wildfire priority-triggered reallocation

**Источник:** B.21 M66, A.21 M68.

**Проблема:** `push_wildfire_priority_update` пишет событие в replay, но не
триггерит reallocation. Агент продолжает лететь к низкоприоритетной задаче
даже если другая зона стала критичной. Gap существует с ранних версий wildfire.

**Что сделать:**

1. При priority update для задачи с `new_priority >= threshold` (configurable,
   default 8) — поставить задачу в очередь force-reallocation в coordinator.
2. Coordinator при следующем тике может переназначить агента к более срочной
   задаче если он ещё не завершил текущую.
3. Добавить `wildfire_priority_reallocation_threshold: u8` в wildfire run config.
4. Тест: два агента, задача с `priority=2` и задача с `priority=9`. После
   priority update задачи с `priority=9` — она назначается ближайшему агенту.

**Затронутые файлы:** `crates/swarm-runtime/src/coordinator.rs`,
`crates/swarm-scenarios/src/wildfire.rs`, `crates/swarm-sim/src/runner.rs`.

**Сложность:** средняя.

---

## 4. Communication-aware allocation scoring

**Источник:** B.21 M66, A.21 M68.

**Проблема:** `comms_range` хранится в `AllocationAgent`, но greedy, auction,
CBBA и centralized игнорируют его. `ConnectivityAwareAllocator` учитывает
`comms_range` только для relay placement; scout-задачи уходят в greedy без
учёта дальности связи.

**Что сделать:**

1. Добавить `comms_penalty_weight: f64` в конфиг scoring (либо в `RunConfig`,
   либо как параметр allocator).
2. Penalty: `max(0, dist(agent, task) - comms_range) * comms_penalty_weight`
   вычитается из score.
3. Benchmark delta: сравнить success/task_conflicts с `weight=0` и `weight>0`
   на heavy-loss-* и partition-prone-* профилях coverage.
4. Unit тест: при `dist > comms_range` и `weight > 0` score агента снижается.

**Затронутые файлы:** `crates/swarm-alloc/src/allocator.rs`,
`crates/swarm-sim/src/runner.rs`.

**Сложность:** средняя.

---

## 5. Динамический автобус с `pose_at_tick`

**Источник:** B.21 M65.

**Проблема:** текущий `UrbanBus` статичен — единственная поза с
`active_from_tick`/`active_until_tick`. B.21 предлагал движущийся автобус
с route по road graph и линейной интерполяцией. Делает Urban Search реалистичнее
без физики.

**Что сделать:**

1. Добавить опциональный `route` в `UrbanBus`:

   ```rust
   pub struct UrbanBusRoute {
       pub stops: Vec<(UrbanNodeId, u64)>,  // (node, arrival_tick)
       pub speed_m_per_tick: f64,
   }
   ```

   Поле опциональное — статичный bus остаётся как `route: None`.

2. Добавить `UrbanBus::pose_at_tick(map: &UrbanMap, tick: u64) -> Option<Pose>`:
   линейная интерполяция между stop[i] и stop[i+1] по времени. Если `route`
   отсутствует — возвращает фиксированную `self.pose`.

3. В `detect_buses` использовать `pose_at_tick` вместо статичной `bus.pose`.

4. Добавить в DSL scenario JSON:

   ```json
   "buses": [{
     "id": "bus-0",
     "pose": { "x": 0.0, "y": 0.0, "z": 0.0 },
     "route": {
       "stops": [["n0", 0], ["n1", 20], ["n2", 40]],
       "speed_m_per_tick": 0.5
     }
   }]
   ```

5. Тесты:
   - `pose_at_tick` детерминирован: bus на пути n0→n1 в tick=10 находится
     на полпути.
   - Без `route`: поведение без изменений (обратная совместимость).
   - Urban Search: движущийся bus детектируется только когда агент и bus
     находятся в пределах detection_range одновременно.

**Затронутые файлы:** `crates/swarm-types/src/urban.rs`,
`crates/swarm-sim/src/urban.rs`, `crates/swarm-scenarios/src/urban.rs`.

**Сложность:** средняя.

---

## 6. Временные препятствия на road graph

**Источник:** A.21 M67.

**Проблема:** road graph статичен после загрузки сценария. Нет способа
тестировать replan/yield поведение без полноценной obstacle avoidance системы.
Временные blocked edges открывают этот класс тестов с минимальными изменениями.

**Что сделать:**

1. Добавить в `UrbanMap` опциональный список runtime obstacles:

   ```rust
   pub struct UrbanTemporaryObstacle {
       pub edge_id: UrbanEdgeId,
       pub appears_at_tick: u64,
       pub disappears_at_tick: Option<u64>,
   }
   ```

   В `RunConfig.urban_state` добавить `temporary_obstacles: Vec<UrbanTemporaryObstacle>`.

2. В runner per-tick: строить effective blocked edges = `edge.blocked ||
   temporary_obstacle.is_active(tick)`.

3. Replay event `UrbanEdgeBlocked { edge_id, tick }` и
   `UrbanEdgeUnblocked { edge_id, tick }`.

4. Тест: блокировка edge на тик 10, агент обходит её или останавливается.

**Затронутые файлы:** `crates/swarm-types/src/urban.rs`,
`crates/swarm-sim/src/runner.rs`, `crates/swarm-replay/src/event_log.rs`.

**Сложность:** средняя. Это prerequisite для replan/yield политик.

---

## 7. CBBA convergence diagnostics

**Источник:** B.21 M66, A.21 M68.

**Проблема:** 6 coverage профилей показывают `success=0.000, completion=1.000`
под CBBA. Причина неизвестна. Текущие `CbbaConverged`/`CbbaBundleUpdated` события
не содержат `conflicting_tasks`. Без этого поля replay не может объяснить провал.

**Что сделать:**

1. Расширить `CbbaBundleUpdated` или добавить отдельный event
   `CbbaConflictDetected { agent_id, conflicting_task_ids: Vec<TaskId>, tick }`,
   эмитируемый когда агент обнаруживает конфликт в консенсусе.

2. Прогнать `replay --timeline --category cbba` на heavy-loss профилях.

3. Проверить hypothesis: `gossip_interval_ticks` слишком велик при потере
   агентов — CBBA не успевает переконвергировать до `max_ticks`. Если
   подтверждается — добавить gossip burst при `AgentFailed` event в coordinator.

4. Если hypothesis не подтверждается — задокументировать в support matrix как
   inherent limitation с replay evidence.

**Затронутые файлы:** `crates/swarm-alloc/src/cbba.rs`,
`crates/swarm-replay/src/event_log.rs`, `crates/swarm-runtime/src/coordinator.rs`.

**Сложность:** средняя (преимущественно анализ + небольшой patch).

---

## 8. Local harness scripts для M58/M59

**Источник:** B.21 M63.

**Проблема:** воспроизвести M58/M59 PX4/SIH прогон сейчас требует ручных шагов:
запустить два PX4, ждать инициализации, запустить supervisor с нужными
параметрами, собрать артефакт. Скриптов нет.

**Что сделать:**

1. `scripts/run_m58_local.sh`:
   - запускает два PX4 SIH в фоне с known PIDs;
   - ждёт порты (nc/poll);
   - запускает `sitl_supervisor --connection --execute ...`;
   - убивает PX4 по завершению;
   - кладёт артефакт в `results/local_m58_YYYY-MM-DD/`.

2. `scripts/run_m59_local.sh`: аналогично, но убивает первый PX4 по timeout
   для симуляции потери агента.

3. README section: "Воспроизведение M58/M59 локально".

**Затронутые файлы:** `scripts/` (новая директория), `docs/SITL_SETUP.md`.

**Сложность:** низкая. Без изменений Rust кода.

---

## 9. Degradation suites

**Источник:** B.21 M67, A.21 M69.

**Проблема:** нет параметрических sweeps. Непонятно как метрики меняются при
росте packet_loss, числа агентов или плотности препятствий.

**Что сделать:**

1. Добавить infrastructure для параметрических профилей в `strategy_comparison`:
   `--sweep agents=2,4,8` или отдельные scenario files с параметризованными
   `RunConfig`.

2. Минимальный первый sweep: `success` vs `packet_loss_rate` (0.0/0.1/0.3/0.5)
   для coverage/ideal-* профилей. Уже покрыто существующими heavy-loss профилями
   частично, но без систематической sweep таблицы.

3. Second sweep: `success` vs `agents_count` (2/4/8) для coverage и wildfire.
   Требует новых scenario profiles с разным числом агентов.

4. Optional: `urban_violations` vs `obstacle_density` если временные препятствия
   (п.6) реализованы.

**Затронутые файлы:** `crates/swarm-examples/src/bin/strategy_comparison.rs`,
`crates/swarm-sim/src/benchmark.rs`, новые scenario profiles в `scenarios/`.

**Сложность:** средняя (infrastructure) + низкая (scenario files).

---

## Приоритет и порядок

| # | Item | Сложность | Зависит от | Блокирует |
|---|---|---|---|---|
| 1 | SAR success threshold | Низкая | — | Benchmark interpretation |
| 2 | Confidence intervals | Низкая | — | Research evidence |
| 3 | Wildfire priority realloc | Средняя | — | Algorithm benchmark delta |
| 4 | Comms-aware scoring | Средняя | — | Algorithm benchmark delta |
| 5 | Динамический bus | Средняя | — | Urban Search realism |
| 6 | Временные obstacles | Средняя | — | Replan/yield tests (Urban) |
| 7 | CBBA diagnostics | Средняя | — | CBBA support matrix clarity |
| 8 | Local harness scripts | Низкая | — | M58/M59 reproducibility |
| 9 | Degradation suites | Средняя | #1, #2 | Research benchmark |

Пункты 1–2 можно делать в любом порядке и параллельно — они не пересекаются
по файлам. Пункты 3 и 4 тоже независимы. Пункт 9 имеет смысл после 1–2.
Пункт 6 — prerequisite для replan/yield политик из `BRANCHES_B.22.md`.
