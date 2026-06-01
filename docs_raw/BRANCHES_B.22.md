# BRANCHES_B.22 — Актуальные ветки развития

Дата фиксации: 2026-05-31

Основа: анализ `docs_raw/BRANCHES.md`, `DRONE_A/B/C.16–21.md` и текущего кода
по состоянию HEAD `a839db5`.

Документ фиксирует все актуальные ветки — нереализованные или реализованные
частично. Полностью закрытые ветки (Ветка 6 Real SITL / PX4, M63–M69 Urban) не
включены.

---

## Зависимости между ветками

```
Ветка 1 (Algorithm Depth)
  └─→ Ветка 3 — Research Benchmark     (зависит: новые алгоритмы должны быть до полного run)

Ветка 2 — Disaster Mapping v2          (самостоятельна)

Ветка 3 — Research Benchmark           (после закрытия gap'ов Веток 1 и 2)

Ветка 4 — Realism v2                   (независима)

Ветка 5 — Replay / Visualization       (независима, Шаг 1 недорог)

Ветка 7 — Platform / API               (после стабилизации semantics)

Ветка 8 — New Mission                  (независима, B3 Perimeter Patrol — самый низкий порог)

Urban Replanning                        (после стабильного Urban patrol)
Urban Multi-Agent Deconfliction         (после Urban Replanning или параллельно)
SAR Success Threshold Fix               (самостоятельна, короткая)

M70 SITL Decision                       (decision milestone, после M69)
```

---

## Ветка 1 — Algorithm Depth

**Статус:** не сделано (M68 добавил corridor-aware planner — одна узкая опция).

**Суть:** измеримые улучшения существующих алгоритмов координации.

### A1 — Communication-aware allocation

**Текущее состояние:** `comms_range` хранится в `AllocationAgent`, но ни один
allocator (greedy, auction, CBBA, centralized) не использует его в scoring.
`ConnectivityAwareAllocator` оптимизирует размещение relay-агентов, но scout-задачи
уходят в greedy без учёта дальности связи.

**Что сделать:**

1. Добавить `comms_penalty_weight: f64` в scoring-функции allocators.
2. Если расстояние агент–задача превышает `comms_range` — снижать score
   пропорционально.
3. Benchmark delta: coverage/SAR/wildfire с/без penalty в heavy-loss и
   partition-prone профилях.

**Ожидаемый эффект:** явная разница между greedy и connectivity-aware в профилях
с ограниченной дальностью связи.

**Сложность:** средняя.

### A2 — Mission-specific planners

**Текущее состояние:** все миссии используют одинаковый scoring pattern. 2-opt
route planning существует в `swarm-alloc/src/route_planner.rs` и используется
только в `CentralizedPlanner`.

**Конкретные gap'ы:**

- **SAR:** задачи упорядочиваются по расстоянию, а не по убыванию belief entropy.
  Dynamic priority update при изменении belief — аналог `WildfireState` — отсутствует.
- **Wildfire:** priority updates пишутся в replay и обновляют поле `priority`,
  но re-allocation не происходит. Агент продолжает выполнять старое назначение
  даже если другая зона стала критичнее. Нужен механизм priority-triggered
  reallocation.
- **Inspection:** 2-opt для `CentralizedPlanner` уже работает, но greedy и
  auction не используют route optimization. Разница в `total_route_cost` должна
  быть измеримой.

**Сложность:** средняя–высокая.

### A3 — CBBA convergence analysis

**Текущее состояние:**

- Coverage CBBA: 6 профилей с `success=0.000`, `completion=1.000` — задачи
  выполняются, но success predicate не срабатывает.
- Emergency mesh CBBA: `success=0.427` vs centralized `0.828`.
- SAR CBBA: явно unsupported (`delayed_reconvergence`).

**Что исследовать:**

1. Coverage heavy-loss/high-latency: почему CBBA не переконвергирует при потере
   агентов? Hypothesis: `gossip_interval_ticks` слишком большой для быстрой
   реконвергенции после failure. Проверяется через replay анализ и изменение
   параметра.
2. SAR CBBA: более частый gossip при agent loss event как possible fix.
3. Emergency mesh конфликты: централизованный имеет inherent преимущество
   (глобальный view). Задокументировать явно в support matrix.

**Сложность:** средняя (преимущественно анализ и параметрический эксперимент).

### A4 — Hierarchical coordination (8+ агентов)

**Текущее состояние:** все тесты и benchmark профили ограничены 2–5 агентами.
Поведение при 8+ агентах не проверялось.

**Что сделать:** добавить сценарии с 8 и 16 агентами для coverage и wildfire,
измерить scaling по `time_to_completion`, `agent_availability`, `task_conflicts`.
Если scaling плохой — ввести hierarchical grouping: кластеры из 3–4 агентов с
локальным coordinator.

**Сложность:** высокая. Нет benchmark evidence о необходимости — делать только
после scaling measurement.

### Done criteria

- Хотя бы одна из A1/A2/A3 даёт измеримое улучшение в benchmark.
- `comms_penalty_weight` сравнён с baseline по message count / success tradeoff.
- Wildfire priority-triggered reallocation протестирован детерминированно.
- CBBA convergence gap задокументирован или пофикшен с replay evidence.

---

## Ветка 2 — Disaster Mapping v2

**Статус:** частично. Flood = future work (M63). Wildfire priority→allocation
частично работает, но priority-triggered reallocation не реализован.

**Суть:** довести wildfire до first-class миссии; flood — только если disaster
mapping становится основным направлением.

**Wildfire priority-triggered reallocation (не сделано):**

Priority поле обновляется и влияет на score в `WildfireAdapter`, но агент не
перераспределяется при изменении приоритета. Нужен механизм: при изменении
`task.priority` выше threshold — trigger reallocation для этой задачи.

**Flood (явно future work):**

Minimal flood mission делать только если disaster mapping снова становится
основным направлением. До тех пор — cleanup wording.

### Done criteria

- Priority-triggered reallocation покрыт детерминированным тестом для wildfire.
- Flood явно помечен как out-of-scope в user-facing docs.

---

## Ветка 3 — Research Benchmark (полная версия)

**Статус:** частично. M69 сделал 1000-seed run. До полного Research Benchmark
не хватает:

- confidence intervals (`mean ± stderr`) — нет;
- degradation curves (`success` vs `packet_loss_rate`, vs `agents_count`,
  vs `grid_size`) — нет;
- strategy comparison report — для каждой (mission, profile) пары winner strategy
  с объяснением — нет;
- Urban в `--mission all` — Urban профили до сих пор не в стандартном benchmark
  entrypoint;
- интерпретация открытых вопросов: SAR success, wildfire success, CBBA gaps.

**Суть:** превратить платформу в доказательный исследовательский артефакт.

**Что сделать:**

1. Закрыть интерпретационные вопросы из M69 перед новым большим run.
2. Добавить Urban профили в `--mission all` (или отдельный `--mission urban`).
3. Full release run: 1000 seeds, release build, confidence intervals.
4. Degradation suites: packet loss, latency, agents count, urban obstacle density,
   bus detection probability.
5. Strategy comparison report: где greedy достаточен, где CBBA/centralized
   выигрывает, где CBBA unsupported.
6. Обновить `docs/BENCHMARK_RESULTS.md` с интерпретацией, а не только таблицами.

**Зависит от:** Ветка 1 (A1/A2/A3) должна быть закрыта до финального run,
иначе артефакт устареет сразу.

### Done criteria

- Benchmark artifact identity explicit (git commit, seed range, build profile).
- Urban included или явно excluded с обоснованием.
- Docs различают current, historical и unsupported evidence.
- Tables include interpretation, not only numbers.

---

## Ветка 4 — Realism v2

**Статус:** не сделано как измеримый слой. Foundation есть: battery model v2,
wind drift, pose noise, comms jitter, realism profiles, time-gated no-fly zones.

**Суть:** сделать realism profiles измеримым слоем с задокументированными
expected effects, а не набором параметров.

**Что сделать:**

1. Определить expected effects для каждого профиля (light/medium/heavy):
   - какие метрики должны падать и насколько;
   - какие должны оставаться стабильными.
2. Comparative benchmark: ideal vs light vs medium vs heavy для каждой mission
   family.
3. Stable realism smoke в regression; нестабильные — только experimental.
4. Обновить docs: что моделируется, что нет, какие assumptions.

### Done criteria

- Expected effects задокументированы по профилям.
- Comparative benchmark воспроизводим из manifest.
- Realism smoke в regression проходит стабильно.

---

## Ветка 5 — Replay / Visualization

**Статус:** частично. Базовый replay есть (`--summary`, `--timeline`, `--agent`,
`--category urban`). Шаг 1 актуален и недорог.

**Суть:** сделать поведение миссий видимым для анализа и отладки.

### Шаг 1 — Расширение replay summary (актуален, низкая сложность)

Сейчас replay summary не показывает mission-specific события для wildfire и SAR.

Что добавить:

- Wildfire events в summary: hazard zone updates, threat level changes,
  high-priority zones mapped.
- SAR belief summary: entropy progression по тикам, detection events.
- Inspection graph summary: edge coverage progression.

### Шаг 2 — ASCII overlay для mission grids

- SAR: belief grid с posterior values по клеткам.
- Wildfire: hazard grid с threat levels.
- Inspection: edge coverage с visited/unvisited пометками.
- Agents: позиции на сетке (уже есть для агентов в целом).

### Шаг 3 — Interactive UI (egui или Bevy)

Явно отложен. Не является приоритетом на текущей стадии. Не должен быть
обязательным для headless benchmark path.

### Done criteria для Шага 1

- Replay summary для wildfire/SAR не пустой и не паникует.
- Event log schema стабильна и задокументирована.

---

## Ветка 7 — Platform / API

**Статус:** частично. `docs/EXTENSION_GUIDE.md` (M61) задокументировал
in-repository extension path. Публичный API не стабилизирован.

**Суть:** снизить стоимость добавления новых миссий и стратегий внешними
пользователями.

**Риск:** преждевременная API stabilization фиксирует неправильные abstractions.
Делать после того, как extension path проверен на реальных миссиях.

**Что не сделано:**

- Semver policy — нет обещаний о стабильности публичного API.
- Machine-readable changelog — нет.
- Schema compatibility tests across versions — нет.
- External crate publication — нет.

**Рекомендация:** актуально только если нужен external reuse. В рамках
in-repository work — достаточно текущего EXTENSION_GUIDE.md.

### Done criteria

- Документированный path для новой миссии без изменений ядра (M61 это закрыл).
- Stable report schema с version и policy.
- Semver policy задокументирована если публикация crates планируется.

---

## Ветка 8 — New Mission

**Статус:** Urban Navigation реализована (M64–M68). Остались три кандидата.

**Суть:** добавить принципиально новый класс миссий с другой механикой
координации.

### B1 — Multi-target Pursuit (не сделано)

Движущиеся цели по траекториям. Задачи динамически появляются и исчезают.
Completion: войти в proximity_radius от цели.

Domain model:

```rust
TaskKind::Pursuit { target_id: TargetId, mode: PursuitMode, proximity_radius: f64 }
enum PursuitMode { Intercept, Escort }
struct PursuitTarget { id: TargetId, trajectory: Vec<(Pose, u64)>, speed: f64 }
```

Метрики: `capture_rate`, `time_to_intercept`, `targets_lost`,
`total_pursuit_distance`, `interception_efficiency`.

Проверяет: алгоритмическую реактивность при динамическом появлении/исчезновении
задач. Stress-test для auction и CBBA.

**Сложность:** высокая. Нужен `active_targets` в RunState, tick-driven обновление
позиций целей.

### B2 — Logistics / Delivery (не сделано)

Pickup → Dropoff с precedence constraints. Агент с cargo capacity. Опционально:
time window (дедлайн доставки).

Domain model:

```rust
TaskKind::Pickup  { item_id: ItemId, location: Pose }
TaskKind::Dropoff { item_id: ItemId, location: Pose, requires_pickup: TaskId }
```

Метрики: `delivery_rate`, `late_deliveries`, `capacity_violations`,
`total_route_cost`, `unserved_deliveries`, `precedence_violations`.

Проверяет: первый mission type с task dependencies. Allocator не должен назначать
Dropoff без предшествующего Pickup.

**Сложность:** высокая. Dependency tracking в `swarm-runtime/src/registry.rs`,
validation в allocators.

### B3 — Perimeter Patrol (не сделано)

Полигон задаёт периметр объекта. Миссия: обойти периметр, посетив waypoints
вдоль него. Опционально: `forbidden_zones` — зоны, которые нельзя пересекать.

Уникальное свойство: единственный кандидат, напрямую совместимый с SITL pipeline
без изменений transport layer — waypoints по периметру уходят прямо в
`sitl_supervisor`.

**Сложность:** низкая–средняя. Новый scenario builder + DSL extension + новый
`MissionAdapter`. Нет breaking changes в `swarm-runtime`.

**Рекомендация:** B3 как первый шаг — минимальная сложность, максимальная связь
с реальными задачами и с существующим SITL pipeline.

### Done criteria

- Новая миссия описывается через DSL без изменений ядра.
- Есть минимум два сценария (small / medium).
- Benchmark запускается для stable стратегий.
- Support matrix задокументирована.

---

## Urban Replanning

**Статус:** метрика `urban_replan_count` заведена, всегда равна нулю. Механизма
нет.

**Суть:** сейчас агент планирует маршрут один раз в начале и следует ему до конца.
Если сегмент стал недоступен в процессе выполнения (runtime blocked edge, новое
препятствие), миссия не адаптируется.

**Что нужно:**

1. Trigger: runtime detection blocked segment или obstacle pop-up во время движения.
2. Policy: replan с текущей позиции по оставшимся сегментам.
3. Event: `UrbanReplanned { agent_id, tick, reason, new_segment_count }`.
4. Метрика: `urban_replan_count > 0` покрыт детерминированным тестом.

**Зависит от:** стабильного Urban patrol (M65 ✅).

**Сложность:** средняя.

### Done criteria

- `urban_replan_count > 0` в хотя бы одном тесте.
- Replay содержит `UrbanReplanned` event.
- Docs объясняют trigger и policy.

---

## Urban Multi-Agent Deconfliction

**Статус:** M67 измеряет separation конфликты (`urban_separation_violation_count`,
`route_conflict_count`), но ничего с ними не делает. Числа диагностические, без
следствий.

**Суть:** coordination policy для разрешения route конфликтов между агентами в
Urban сценариях.

**Возможные policy:**

- Priority-based yielding: агент с меньшим приоритетом ждёт.
- Time-slotted edge access: только один агент на сегменте в тике.
- Alternate route selection: реплан при обнаружении конфликта.

**Что нужно:**

1. Выбрать одну policy для v1.
2. Добавить conflict detection в runner tick loop.
3. Добавить events: `UrbanRouteConflictDetected`, `UrbanYielded`.
4. Покрыть двух-агентным детерминированным тестом.

**Зависит от:** Urban Replanning (для policy на основе реплана) или независима
(для yielding/time-slotted).

**Сложность:** средняя–высокая.

### Done criteria

- Двух-агентный тест: конфликт обнаружен и разрешён по выбранной policy.
- `urban_separation_violation_count` снижается при включённой policy.
- Replay содержит deconfliction events.

---

## SAR Success Threshold Fix

**Статус:** не сделано. SAR success ≈ 0 во всех стратегиях в текущем benchmark,
потому что `all_targets_found()` требует 100% обнаружения при
`detection_probability < 1.0`. Это делает SAR строки нечитаемыми.

**Суть:** короткий самостоятельный fix, не зависящий от Research Benchmark.

**Что сделать:**

1. Добавить `pod_success_threshold: f64` в `SarRunConfig` (default 0.8 или 1.0
   для обратной совместимости).
2. Изменить success predicate с `all_targets_found()` на
   `targets_found_rate >= threshold`.
3. Покрыть тестом: при `pod_success_threshold=0.5` и 60% найденных целей
   `success=true`.
4. Задокументировать SAR success как probability-of-detection based metric.

**Сложность:** низкая. Одно новое поле в config, изменение в runner, один тест,
doc comment.

### Done criteria

- SAR benchmark строки показывают non-zero success при reasonable threshold.
- Тест фиксирует semantics: threshold применяется корректно.
- Docs объясняют разницу между `all_targets_found` и `pod_success_threshold`.

---

## M70 — SITL Export And Platform Boundary Decision

**Статус:** не начат. Decision milestone после M69.

**Суть:** принять следующий осознанный выбор на основе M63–M69 evidence.

### Option A — Urban route export to SITL/PX4

Конвертировать Urban маршрут в waypoint mission, валидировать через safety gate,
прогнать через local PX4/SIH.

Scope: route-to-waypoint conversion, safety gate validation, PX4/SIH upload,
artifact с run_id.

Non-goals: hardware, Gazebo, real obstacle avoidance.

### Option B — SITL hardening

Local integration harness (reproducible script для M58/M59 прогонов), artifact
validator, broader failure modes, replay timeline для SITL events.

Scope: scripts/run_m58_local.sh, per-failure-mode artifacts, replay seq fix
(completion events для recovered tasks пишутся с seq из оригинального manifest,
а не из replacement mission).

### Option C — Platform/API packaging

External-style mission example, schema compatibility tests, crate boundary review.

Non-goals: public semver promise без explicit choice.

### Done criteria

- Конкретный следующий roadmap выбран на основе M63–M69 evidence.
- Если SITL — artifact reproducibility улучшена.
- Если platform — extension path проверен на реальной внешней миссии.
- Если algorithm depth — benchmark gaps обоснованы.

---

## Сводная таблица

| Ветка / направление | Статус | Сложность | Зависит от |
|---|---|---|---|
| 1A1 Comms-aware scoring | Не сделано | Средняя | — |
| 1A2 Mission-specific planners | Не сделано | Средняя–высокая | — |
| 1A3 CBBA convergence | Не сделано | Средняя | — |
| 1A4 Hierarchical 8+ агентов | Не сделано | Высокая | Scaling evidence |
| 2 Wildfire priority reallocation | Частично | Средняя | — |
| 2 Flood | Future work | — | Disaster mapping как основное направление |
| 3 Research Benchmark (полный) | Частично | Средняя (код) | Ветка 1, Urban в all |
| 4 Realism v2 | Не сделано | Средняя | — |
| 5 Replay Шаг 1 (mission events) | Не сделано | Низкая | — |
| 5 Replay Шаг 2 (ASCII overlays) | Не сделано | Средняя | — |
| 5 Replay Шаг 3 (Interactive UI) | Отложено | Высокая | Шаги 1–2 |
| 7 Platform / API | Частично | Высокая | Стабильные semantics |
| 8B3 Perimeter Patrol | Не сделано | Низкая–средняя | — |
| 8B1 Multi-target Pursuit | Не сделано | Высокая | — |
| 8B2 Logistics / Delivery | Не сделано | Высокая | — |
| Urban Replanning | Не сделано | Средняя | Urban patrol ✅ |
| Urban Multi-Agent Deconfliction | Не сделано | Средняя–высокая | Urban Replanning или —  |
| SAR Success Threshold Fix | Не сделано | Низкая | — |
| M70 SITL Decision | Не начато | — | M69 ✅ |
