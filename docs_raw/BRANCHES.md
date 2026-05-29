# DRONE_B.15.branches — Ветки развития

Дата фиксации: 2026-05-28

## Структура документа

**Ветки 1–8** — равноправные стратегические направления. Каждое конкурирует за
фокус проекта. Выбор между ними зависит от цели: алгоритмическая глубина,
новые миссии, исследовательский артефакт, реализм, визуализация, реальные дроны,
API/платформа, или принципиально новая механика задач.

## Зависимости между ветками

```
Ветка 1 (Algorithm Depth)
 └─→ Ветка 3 — Research Benchmark     (зависит от Ветки 1)

Ветка 2 — Disaster Mapping v2         (самостоятельна)

Ветка 4 — Realism v2                  (независима)

Ветка 5 — Replay / Visualization      (после стабилизации replay schemas)

Ветка 6 — Real SITL / PX4             (самостоятельна)

Ветка 7 — Platform / API              (после стабилизации semantics)

Ветка 8 — New Mission                 (независима от веток 1–7)
```

---

## Ветка 1 — Algorithm Depth

**Суть:** новые алгоритмические возможности, которых сейчас нет. Не исправление
ошибок — развитие системы.

### Что сделать

**1. Dynamic reallocation при отказе агента.**

Сейчас при отказе агента его незавершённые задачи теряются. Нужно:
- Вернуть задачи fallen агента в пул без полного рестарта CBBA консенсуса.
- Перераспределить только освободившиеся задачи.
- Метрики: `reassignment_count`, `avg_reallocation_ticks`.
- Тест: детерминированный сценарий с одним failing агентом → задачи переданы
  оставшимся агентам.

**2. Communication-aware allocation.**

Сейчас scoring не учитывает стоимость сообщений. Нужно:
- Ввести `message_budget` как soft penalty или hard ограничение.
- При высоком packet loss — стратегии, порождающие много сообщений (CBBA),
  должны учитывать это в scoring.
- Benchmark comparison: message count vs success rate tradeoff по стратегиям.

**3. Mission-specific planner modes.**

Сейчас один planner для всех миссий. Нужно:
- Inspection linear → 2-opt (минимизация route length).
- SAR → greedy по uncertainty score (максимизация информации).
- Wildfire → priority-weighted nearest neighbour (высокоприоритетные зоны
  посещаются раньше).
- Выбор через `MissionAdapter::preferred_planner` или аналог.

**4. Hierarchical coordination.**

Для больших роёв (8+ агентов) централизованный coordinator неэффективен. Нужно:
- Разбить агентов на группы (2–4 агента), каждая с локальным лидером.
- Глобальный coordinator балансирует нагрузку между группами.
- Ограниченный обмен сообщениями между группами.
- Benchmark: scalability curve — как растёт message count при росте числа агентов.

### Done criteria

- Dynamic reallocation покрыт детерминированным тестом.
- `reassignment_count` и `avg_reallocation_ticks` видны в benchmark output.
- Хотя бы один mission-specific planner mode измеримо улучшает route metrics.
- Communication-aware scoring сравнён с baseline по message count / success tradeoff.

### Тесты

#### Без рефакторинга

- Unit test: задачи fallen агента возвращены в пул.
- Integration test: задачи перераспределены после отказа агента.
- Unit test: mission-specific planner mode выбирается через adapter.
- Benchmark smoke: message count с и без communication-aware scoring.

#### Лёгкий рефакторинг

- Fake agent failure scenarios.
- Deterministic communication profile fixtures.
- Route quality assertion helpers.

#### Тяжёлый рефакторинг

- Property tests: CBBA convergence under sustained packet loss.
- Scalability benchmark: message count vs agent count curve.
- Hierarchical coordination integration tests.

---

## Ветка 2 — Disaster Mapping v2

**Статус:** самостоятельная ветка.

**Суть:** довести wildfire до первоклассной миссии и закрыть вопрос о flood.

### Текущее состояние

Wildfire прототип есть: `WildfireProfile`, три сценарных файла в `scenarios/`,
`TaskKind::MappingZone`, `WildfireState`, hazard zones, dynamic threat update,
replay events. Но:

- priority updates не влияют на allocation — это event/field update, не реальный
  dynamic mission loop;
- success semantics неопределены (`medium-dynamic` даёт mismatch);
- wildfire metrics не экспортируются полноценно в JSON/CSV/table;
- нет документации DSL для wildfire;
- название "wildfire / flood mapping" обещает flood, которого нет.

### Что сделать

**1. Success semantics:**

- Выбрать и зафиксировать критерий успеха для `small-static` и `medium-dynamic`.
- Покрыть тестом: `success` и `completion` согласованы для каждого сценария.

**2. Dynamic mission loop:**

- Priority updates реально влияют на allocation: задачи с высоким приоритетом
  получают более высокий score в текущем тике.
- Dynamic task injection при изменении threat level.
- Optional: zone expansion over time.

**3. Metrics export:**

Добавить в JSON/CSV/table:
- `hazard_zones_mapped`;
- `high_priority_zones_mapped`;
- `priority_updates_count`;
- `time_to_first_critical_zone`;
- `final_avg_threat_level`.

**4. DSL docs:**

Задокументировать wildfire scenario fields:
- `hazard_zones`, `threat_level`, `priority`, `update_interval_ticks`,
  mapping completion semantics.

**5. Flood scope decision:**

*Вариант A — rename/docs cleanup:*
- Убрать "flood" из документации и CLI help там, где нет реализации.
- Wildfire остаётся единственной disaster mapping миссией.
- Flood явно помечается как future work.

*Вариант B — minimal flood variant:*
- Отдельная модель: flooded zones, water spread, critical zones, rescue-priority tasks.
- Scenario files: `flood.small-static.json`, `flood.medium-dynamic.json`.
- Metrics: `flooded_zones_mapped`, `critical_zones_mapped`, `time_to_first_critical`,
  `final_risk_level`.
- Интеграция в adapter/runner/replay/reporting.
- Regression smoke как experimental.

*Рекомендация:* Вариант A, если только disaster mapping не выбирается как основное
направление.

### Done criteria

- `small-static` и `medium-dynamic` имеют задокументированные success rules.
- Priority updates реально влияют на assignment.
- Metrics видны в JSON/CSV/table.
- Scenario files проходят catalog tests.
- Flood scope явно закрыт: либо реализация, либо out-of-scope в docs.

### Тесты

#### Без рефакторинга

- Wildfire scenario load test.
- `success` / `completion` consistency test для обоих сценариев.
- Metrics export test: wildfire rows содержат hazard fields.
- Replay event roundtrip для hazard zone updates.

#### Лёгкий рефакторинг

- Hazard map fixture builders.
- Helper для parsing wildfire rows из benchmark output.
- Threshold fixtures для wildfire regression.

#### Тяжёлый рефакторинг

- Property tests для dynamic hazard updates.
- Multi-seed wildfire benchmark comparison.
- Disaster mapping abstraction если flood выбирается.

---

## Ветка 3 — Research Benchmark

**Статус:** делать после Ветки 1.

**Суть:** превратить платформу в доказательный исследовательский артефакт.

### Почему не раньше

Ветка 1 меняет поведение алгоритмов — 1000-seed run до неё будет устаревшим артефактом.

### Что сделать

1. Full runs: 1000 seeds по каждой поддерживаемой mission-strategy паре.
2. Confidence intervals и variance для ключевых метрик.
3. Degradation curves: как метрики меняются при росте packet loss, числа агентов,
   размера сетки, уровня шума.
4. Strategy comparison report:
   - где CBBA выигрывает у greedy/auction;
   - где CBBA проигрывает;
   - где centralized лучше всех;
   - что даёт connectivity-aware стратегия.
5. Reproducible benchmark pack: manifest с git commit, seed range, конфигом,
   schema version.
6. `docs/BENCHMARK_RESULTS.md` с интерпретацией, а не только таблицами.
7. README summary table: текущие числа, а не список фич.

### Done criteria

- Есть воспроизводимый pack для каждой основной mission/strategy пары.
- Есть документ с интерпретацией и выводами по стратегиям.
- Benchmark воспроизводим из manifest.
- Regression thresholds основаны на реальных full-run числах.

### Тесты

#### Без рефакторинга

- Manifest содержит все обязательные поля (git commit, seed range, schema version).
- JSON/CSV/Markdown row counts согласованы.
- Regression thresholds не ниже p25 full-run распределения.

#### Лёгкий рефакторинг

- Helper для сравнения двух benchmark packs (delta report).
- Reusable assertions для manifest completeness.

#### Тяжёлый рефакторинг

- Статистический diff tooling.
- Historical baseline store с delta tracking.

---

## Ветка 4 — Realism v2

**Статус:** независима от остальных.

**Суть:** сделать realism profiles измеримым слоем, а не набором параметров.

### Текущее состояние

Foundation есть: `Pose.z`, battery model v2, altitude sensor penalty, wind drift,
pose noise, comms jitter, time-gated no-fly zones, `--realism` preset, сценарные
файлы для каждой миссии. Но нет сравнительного анализа, нет определения expected
effects, README Known Limitations противоречат статусу "Simulation Realism stable",
realism не интегрирован в regression.

### Что сделать

1. Определить expected effects для каждого профиля (light/medium/heavy):
   - какие метрики должны падать;
   - какие должны оставаться стабильными.
2. Сравнительный benchmark: ideal vs light vs medium vs heavy для каждой mission family.
3. Обновить docs: что моделируется, что нет, какие assumptions.
4. Исправить README Known Limitations.
5. Добавить realism metadata в manifest.
6. Stable realism smoke в regression; нестабильные — только experimental.

### Done criteria

- Expected realism effects задокументированы по профилям.
- Comparative benchmark воспроизводим из manifest.
- README не противоречит сам себе.
- Realism smoke в regression проходит стабильно.

### Тесты

#### Без рефакторинга

- Battery model v2 unit tests.
- Altitude sensor penalty boundary tests.
- Wind drift deterministic tests с фиксированным seed.
- No-fly time window тесты.

#### Лёгкий рефакторинг

- Ideal-vs-realism comparison helper.
- Deterministic fixture для realism profile selection.
- Manifest assertion helpers.

#### Тяжёлый рефакторинг

- Stochastic realism regression.
- Full comparative analysis old model vs realism-enabled.

---

## Ветка 5 — Replay / Visualization

**Статус:** делать после стабилизации replay schemas.

**Суть:** сделать поведение миссий видимым для анализа и демонстрации.

### Что сделать

**Шаг 1 — Replay summary для всех mission types:**

- Wildfire events: hazard zone updates, threat level changes.
- Realism events: battery drain, sensor misses, comms drops.
- SAR belief summary: entropy progression, detection ticks.
- Inspection graph summary: edge coverage progression.

**Шаг 2 — ASCII overlay:**

- `--tick N`, `--follow`.
- SAR: belief grid с posterior values.
- Inspection: edge coverage с visited/unvisited пометками.
- Wildfire: hazard grid с threat levels.
- Agents: позиции на сетке.

**Шаг 3 — Interactive UI (egui или Bevy):**

- Timeline с событиями, map/grid view, agent trajectories.
- BeliefMap, InspectionGraph, Wildfire hazard overlays.
- Strategy comparison viewer.
- UI не должен быть обязательным для headless benchmark path.

### Done criteria для Шага 1

- Replay CLI показывает wildfire и realism events.
- Replay summary для всех mission types без паники.
- Event log schema стабильна и задокументирована.

### Тесты

#### Без рефакторинга

- Replay summary tests для wildfire/realism events.
- ASCII snapshot tests для SAR/inspection grid.
- Replay JSON roundtrip для всех event types.

#### Лёгкий рефакторинг

- Reusable replay fixtures по mission type.
- Event log builders для mission-specific events.

#### Тяжёлый рефакторинг

- UI rendering tests.
- Interactive timeline tests.

---

## Ветка 6 — Real SITL / PX4

**Суть:** превратить feature-gated MAVLink scaffold в реальный end-to-end workflow.

### Текущее состояние

Mock SITL работает: `MockMavlinkTransport`, `sitl_agent --mock`,
`scenarios/sitl.waypoints.json`. Real `MavlinkTransport` feature-gated, но
`sitl_agent --connection` не создаёт реальный transport.

### Что сделать

**Этап 1 — Single-agent golden path:**

1. Подключить `MavlinkTransport` в `sitl_agent --connection`:
   - при feature `mavlink-transport` → реальный transport;
   - без feature → понятная ошибка, не silent fallback.
2. Mission upload в PX4: `MISSION_COUNT`, `MISSION_ITEM_INT`, ack handling.
3. Telemetry → `TaskStatus`: waypoint reached → complete, mission failed → failed.
4. arm/takeoff/execute/abort.
5. Safety validation перед upload: geofence, no-fly zones, separation.
6. Обновить `docs/SITL_SETUP.md`.

**Этап 2 — Multi-agent SITL:**

- Несколько агентов, координация через runtime.
- Failure handling: потеря агента → reallocation.

### Done criteria для Этапа 1

- Один агент проходит waypoints через PX4 SITL.
- `--connection` реально использует `MavlinkTransport`.
- Mock path остаётся полностью portable.
- Docs разделяют mock, SITL и real hardware.

### Тесты

#### Без рефакторинга

- Mock transport roundtrip tests.
- Waypoint conversion tests.
- CLI validation tests для `--connection` без feature.

#### Лёгкий рефакторинг

- Fake `MavlinkTransport` для unit tests.
- Typed error fixtures для MAVLink failures.
- SITL command dry-run mode.

#### Тяжёлый рефакторинг

- Real PX4 SITL integration tests.
- Multi-agent SITL tests.
- Hardware-in-the-loop tests.

---

## Ветка 7 — Platform / API

**Статус:** делать после стабилизации semantics (Ветка 0) и нескольких опытов
добавления новых миссий/стратегий.

**Суть:** снизить стоимость добавления новых миссий и стратегий.

### Риск

Преждевременная API stabilization фиксирует неправильные abstractions. Делать
после того, как `MissionAdapter` wiring устоялся и добавлена хотя бы одна миссия
сверх текущих.

### Что сделать

1. Stable crate boundaries:
   - публичные: `swarm-types`, `swarm-sim`, `swarm-scenarios`;
   - internal: `swarm-runtime`, `swarm-alloc`.
2. Documented extension points:
   - как добавить миссию: schema, adapter, builder, scenario files, metrics, replay events;
   - как добавить стратегию: allocator trait, registration, benchmark integration;
   - как добавить метрику: `RunMetrics`, `AggregateMetrics`, export schema.
3. Semver policy: major для breaking API changes, minor для новых миссий/стратегий.
4. Schema version policy для scenario files и replay log format.
5. Deprecation policy.
6. Machine-readable changelog начиная с текущей версии.

### Done criteria

- Документированный path для новой миссии без изменений ядра.
- Документированный path для новой стратегии.
- Stable report schema с version и policy.
- Integration test для extension path.

### Тесты

#### Без рефакторинга

- Schema roundtrip tests для scenario и replay files.
- Extension point smoke tests.

#### Лёгкий рефакторинг

- Example fixture для новой minimal mission.
- Shared scenario generator fixtures.

#### Тяжёлый рефакторинг

- External strategy harness.
- Schema compatibility tests across versions.
- Semver-oriented API checks.

---

## Ветка 8 — New Mission

**Статус:** независима от Веток 1–7.

**Суть:** добавить принципиально новый класс миссий с другой механикой координации —
не вариацию существующих "посети точки/ячейки/рёбра", а задачи с динамическими
целями или межзадачными зависимостями.

### Зачем нужна отдельная ветка

Существующие миссии (Coverage, SAR, Inspection, Wildfire) всё ещё об одном:
агент получает набор точек/зон и посещает их. Новые кандидаты проверят совершенно
другие стороны алгоритмов координации.

### Кандидаты

**1. Multi-target pursuit**

Суть: движущиеся цели, агенты перехватывают или сопровождают.

- Цели движутся по заданным траекториям или убегают по простой модели.
- Задача агента: догнать и войти в зону proximity (capture), или сопровождать
  (escort) в пределах некоторого радиуса.
- Задачи динамически появляются и исчезают (цель перехвачена → задача снимается).
- Требует: time-dependent tasks, reactive reallocation, predictive routing.
- Метрики: `capture_rate`, `time_to_intercept`, `targets_lost`, `total_pursuit_distance`.

Почему интересно для алгоритмов:
- CBBA с time-dependent scoring vs greedy;
- coordination под высокой динамикой пересмотра задач;
- ситуация когда количество задач и их приоритеты меняются каждый тик.

**2. Logistics / Delivery**

Суть: задачи с зависимостями и capacity constraints.

- Pickup и dropoff локации: нельзя доставить не забрав.
- Depot как база.
- Capacity: агент несёт ограниченный груз.
- Deadlines: некоторые задачи имеют временное окно.
- Требует: task dependency graph, capacity-aware allocation, deadline-aware scoring.
- Метрики: `delivery_rate`, `late_deliveries`, `total_route_cost`,
  `capacity_violations`, `unserved_deliveries`.

Почему интересно для алгоритмов:
- Впервые задачи имеют precedence constraints — allocator должен их учитывать;
- capacity-aware CBBA vs centralized solver;
- проверяет, что DSL и adapter layer обобщаются за пределы "набор точек".

### Что сделать (общее для обоих кандидатов)

1. Domain model: ключевые типы задач и состояний миссии.
2. DSL: поля в scenario JSON, параметры генерации.
3. `TaskKind` + `MissionAdapter`: completion, route cost, scoring.
4. Allocation compatibility: какие стратегии поддерживаются, какие — нет.
5. Replay events: mission-specific события.
6. Benchmark: small/medium scenarios, baseline по стратегиям.
7. Docs: semantics, limitations, supported strategy matrix.

### Порядок

Реализовывать один кандидат за раз. Рекомендация:

- Если цель — проверить алгоритмическую реактивность → **Multi-target pursuit**.
- Если цель — проверить расширяемость DSL и task dependencies → **Logistics / Delivery**.

### Done criteria

- Новая миссия описывается через DSL без изменений ядра.
- Есть минимум два сценария (small / medium).
- Benchmark запускается для stable стратегий.
- Replay содержит события, специфичные для новой миссии.
- Support matrix задокументирована.

### Тесты

#### Без рефакторинга

- DSL parse/validation tests для нового scenario type.
- Task generation tests.
- Completion semantics tests.
- Replay event serialization tests.
- Benchmark smoke test для small scenario.

#### Лёгкий рефакторинг

- Mission-specific fixture builders.
- Reusable mission benchmark fixtures.
- Assertion helpers для mission outcome.

#### Тяжёлый рефакторинг

- Property tests для dynamic task появления/исчезновения (pursuit).
- Property tests для precedence constraint satisfaction (logistics).
- Multi-seed mission stability tests.
- Comparative tests по всем стратегиям.
