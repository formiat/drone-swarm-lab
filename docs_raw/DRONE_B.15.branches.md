# DRONE_B.15.branches — Ветки развития

Дата фиксации: 2026-05-28

## Короткий вывод

Проект имеет широкую, но неглубоко склеенную реализацию. Большинство подсистем
реализованы отдельно и слабо интегрированы между собой:

- `MissionAdapter` реализован для шести типов задач, но runner вызывает адаптеры
  ровно в одном месте — ни completion, ни scoring через adapter не проходят;
- `BatteryAwarePlanner` проверяет feasibility по исходному `tasks`, а не по
  усечённому маршруту — логический дефект;
- `ComparisonReport` при `--mission all` падает на `mission_names.first()` когда
  `metrics.mission` пуст — benchmark artifacts ненадёжны;
- `wildfire medium-dynamic` даёт `Completion=1.0` при `Success=0.0` — semantics
  не определены;
- SAR + CBBA/centralized классифицированы как unsupported без точного диагноза.

Все эти проблемы лежат в обязательном общем стволе (Ветка 1), который нужен
почти для любого дальнейшего направления. После него открываются равноправные
ветки, в том числе Ветка 2 — алгоритмическая глубина, которая отделена от Ветки 1
намеренно: Ветка 1 исправляет то, что есть, Ветка 2 строит то, чего ещё нет.

## Зависимости между ветками

```
Ветка 1 — Algorithm Wiring & Correctness  (обязательный ствол)
 ├─→ Ветка 2 — Algorithm Depth
 ├─→ Ветка 4 — Research Benchmark
 ├─→ Ветка 6 — Replay / Visualization
 └─→ Ветка 7 — Real SITL / PX4

Ветка 1
 └─→ Ветка 3 — Disaster Mapping v2        (можно и независимо, но чище после)

Ветка 2
 └─→ Ветка 4 — Research Benchmark         (зависит от обеих)

Ветка 5 — Realism v2                      (независима, осмысленна после Ветки 1)

Ветка 8 — Platform / API                  (после стабилизации semantics)
```

Ветка 1 — не опциональна. Без неё алгоритмические изменения оптимизируют
неправильную модель, research benchmark считает неверные метрики, SITL строится
на неверной семантике задач.

Ветка 2 отделена от Ветки 1 намеренно. Ветка 1 — это корректность: подключить
то, что есть, убрать дефекты, зафиксировать границы поддержки. Ветка 2 — это
развитие: новые алгоритмические возможности, которых сейчас нет.

---

## Ветка 1 — Algorithm Wiring & Correctness

**Статус:** обязательный общий ствол.

**Суть:** подключить то, что реализовано, к тому, что реально запускается.

### Проблемы, которые решает эта ветка

**1. MissionAdapter не используется в runner.**

`CoverageAdapter`, `SarAdapter`, `InspectionAdapter`, `WildfireAdapter`,
`RelayAdapter`, `WaypointAdapter` — все шесть адаптеров реализованы в
`swarm-types/src/adapter.rs`. Но в `runner.rs` adapter вызывается ровно в одном
месте. Completion conditions, scoring и route cost остаются в ad hoc runner blocks.
Архитектура M27 фактически мёртвый код.

**2. BatteryAwarePlanner — логический дефект.**

`BatteryAwarePlanner::order` вызывает `is_feasible` и затем в цикле отбрасывает
задачи с конца. Необходимо гарантировать, что feasibility проверяется на текущем
усечённом кандидате маршрута, а не на исходном полном списке задач. Если это не
так — исправить.

**3. SAR + CBBA/centralized — нет точного диагноза.**

Стратегии классифицированы как unsupported, но корень не зафиксирован в коде.
После wiring адаптеров диагностика станет конкретной: видно, где теряется
`grid_cell`, где completion не срабатывает, где scoring игнорирует тип задачи.

**4. Wildfire success/completion mismatch.**

`medium-dynamic` показывает `Completion=1.0` при `Success=0.0`. Причина —
не определены semantics: что означает успешность wildfire миссии.

### Что сделать

1. Провести `MissionAdapter::is_completed` через runner для всех mission types.
2. Подключить adapter `score` и `route_cost` для CBBA как минимум, для других
   стратегий там, где это имеет смысл.
3. Убрать или заменить ad hoc completion blocks в runner на adapter path.
4. Проверить `BatteryAwarePlanner::order`: гарантировать, что feasibility
   проверяется на текущем усечённом маршруте, добавить unit test.
5. После wiring: диагностировать SAR + CBBA — найти точку, где тип задачи
   теряется. Либо починить, либо зафиксировать точную причину с regression test.
6. Определить wildfire success semantics: выбрать один критерий, задокументировать,
   закрепить тестом.
7. Обновить support matrix: статусы с конкретными причинами, не просто флагами.

### Done criteria

- `MissionAdapter::is_completed` вызывается в runner для всех mission types.
- Для CBBA scoring и route cost идут через adapter.
- `BatteryAwarePlanner` имеет тест на корректность feasibility logic.
- SAR + CBBA либо работает объяснимо, либо имеет тест, фиксирующий точную причину.
- Wildfire `success` и `completion` согласованы и покрыты тестами.
- Support matrix обновлена с причинами.

### Тесты

#### Без рефакторинга

- Unit test: `BatteryAwarePlanner` — battery-constrained bundle усекается корректно.
- Integration test: wildfire small-static — `success` и `completion` согласованы.
- Integration test: SAR + CBBA — статус объяснимый, не немотивированный 0%.
- Unit test: adapter `is_completed` вызывается для SAR scan задачи в runner.
- Test: support matrix не называет stable то, что является experimental.

#### Лёгкий рефакторинг

- Shared builders для задач каждого `TaskKind`.
- In-memory `RunState` fixtures для adapter tests.
- Reusable `BatteryModel` fixtures с контролируемыми drain параметрами.

#### Тяжёлый рефакторинг

- Property tests: valid task kind → adapter `is_completed` не паникует.
- Full lifecycle tests: DSL → adapter → allocator → runner → metrics.

---

## Ветка 2 — Algorithm Depth

**Статус:** делать после Ветки 1.

**Суть:** новые алгоритмические возможности, которых сейчас нет, — не исправление
ошибок, а развитие системы.

### Отличие от Ветки 1

Ветка 1 — correctness: подключить адаптеры, починить planner, зафиксировать
диагнозы. Ветка 2 — depth: dynamic reallocation, communication-awareness,
mission-specific planners, hierarchical coordination. Это самостоятельное
исследовательское направление, которое конкурирует с Research Benchmark,
Disaster Mapping и SITL при выборе фокуса.

### Что сделать

**1. Dynamic reallocation при отказе агента.**

Сейчас при отказе агента bundle теряется. Нужно:
- Когда агент уходит в состояние failed, его незавершённые задачи
  возвращаются в пул.
- Перераспределение без полного рестарта CBBA консенсуса — только для
  освободившихся задач.
- Метрики: `reassignment_count`, `avg_reallocation_ticks`.
- Тест: детерминированный сценарий с одним failing агентом → задачи
  переданы другому агенту.

**2. Communication-aware allocation.**

Сейчас scoring не учитывает стоимость сообщений. Нужно:
- Ввести `message_budget` как ограничение или soft penalty.
- Стратегии, порождающие много сообщений (CBBA), должны это учитывать
  в scoring при высоком packet loss.
- Benchmark comparison: message count vs success rate tradeoff по стратегиям.

**3. Mission-specific planner modes.**

Сейчас один planner используется для всех миссий. Нужно:
- Inspection linear → 2-opt оптимален (минимизация route length).
- SAR → greedy по uncertainty score оптимален (максимизация информации).
- Wildfire → priority-weighted nearest neighbour (высокоприоритетные зоны
  посещаются раньше).
- Отдельный `PlannerMode` per mission kind, выбирается через adapter.

**4. Hierarchical coordination.**

Для больших роёв (8+ агентов) координация через единый coordinator неэффективна.
Нужно:
- Разбиение агентов на локальные группы (2-4 агента).
- Локальный лидер координирует группу.
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

- Unit test: failed agent tasks returned to pool.
- Integration test: tasks reassigned after agent failure.
- Unit test: mission-specific planner mode выбирается через adapter.
- Benchmark smoke: message count с и без communication-aware scoring.

#### Лёгкий рефакторинг

- Fake agent failure scenarios.
- Deterministic communication profile fixtures.
- Route quality assertion helpers.

#### Тяжёлый рефакторинга

- Property tests: CBBA convergence under sustained packet loss.
- Scalability benchmark: message count vs agent count curve.
- Hierarchical coordination integration tests.

---

## Ветка 3 — Disaster Mapping v2

**Статус:** самостоятельная ветка, лучше делать после Ветки 1.

**Суть:** довести wildfire до первоклассной миссии и закрыть вопрос о flood.

### Текущее состояние

Wildfire прототип есть: `WildfireProfile`, три сценарных файла в `scenarios/`,
`TaskKind::MappingZone`, `WildfireState`, hazard zones, dynamic threat update,
replay events. Но:

- priority updates не влияют на allocation — это event/field update, а не
  реальный dynamic mission loop;
- success semantics неопределены (`medium-dynamic` даёт mismatch);
- wildfire metrics не экспортируются полноценно в JSON/CSV/table;
- нет документации DSL для wildfire;
- название "wildfire / flood mapping" обещает flood, которого нет.

### Что сделать

**1. Определить success semantics:**

- Выбрать и зафиксировать критерий успеха для `small-static` и `medium-dynamic`.
- Покрыть тестом: для каждого сценария `success` и `completion` согласованы.

**2. Dynamic mission loop:**

- Priority updates должны реально влиять на allocation: задачи с высоким приоритетом
  получают более высокий score в текущем тике.
- Добавить dynamic task injection при изменении threat level.
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
- `hazard_zones`;
- `threat_level`;
- `priority`;
- `update_interval_ticks`;
- mapping completion semantics.

**5. Flood scope decision:**

Выбрать один из двух путей:

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

*Рекомендация:* сначала Вариант A. Вариант B — только если disaster mapping
выбирается как основное направление проекта.

**6. Regression:**

Добавить wildfire suites в default regression как experimental первым шагом,
потом promote в quick после стабилизации semantics.

### Done criteria

- `small-static` и `medium-dynamic` имеют понятные, задокументированные success rules.
- Priority updates реально влияют на assignment в текущем тике.
- Metrics видны в JSON/CSV/table.
- Scenario files проходят catalog tests.
- Flood scope явно закрыт: либо реализация, либо явный out-of-scope в docs.

### Тесты

#### Без рефакторинга

- Wildfire scenario load test.
- `success` / `completion` consistency test для `small-static` и `medium-dynamic`.
- Metrics export test: wildfire rows в JSON/CSV содержат hazard fields.
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

## Ветка 4 — Research Benchmark

**Статус:** делать после Веток 1 и 2.

**Суть:** превратить платформу в доказательный исследовательский артефакт.

### Почему не раньше

Benchmark сейчас содержит methodological bugs: SAR+CBBA без точного диагноза,
wildfire без semantics, report identity ненадёжен при `--mission all`. Публиковать
результаты до исправления — значит публиковать неверные таблицы.

Ветка 2 также важна перед публикацией: dynamic reallocation и mission-specific
planners меняют числа, поэтому 1000-seed run до них — устаревший артефакт.

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
   schema version — достаточно для воспроизведения без догадок.
6. Документ `docs/BENCHMARK_RESULTS.md` с интерпретацией, а не только таблицами.
7. README summary table: текущие числа, а не список фич.

### Done criteria

- Есть воспроизводимый pack для каждой основной mission/strategy пары.
- Есть документ с интерпретацией и выводами по стратегиям.
- Benchmark можно повторить из manifest без чтения кода.
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

## Ветка 5 — Realism v2

**Статус:** независима от остальных, но осмыслена после Ветки 1.

**Суть:** сделать realism profiles измеримым слоем, а не набором параметров.

### Текущее состояние

Foundation есть: `Pose.z`, battery model v2, altitude sensor penalty, wind drift,
pose noise, comms jitter, time-gated no-fly zones, `--realism` preset, сценарные
файлы (`coverage.realism.json`, `sar.realism.json`, `inspection.realism.json`,
`wildfire.realism.json`). Но:

- нет сравнительного анализа ideal vs realism;
- нет определения expected effects по профилям;
- README Known Limitations противоречат статусу "Simulation Realism stable";
- realism не интегрирован в regression.

### Что сделать

1. Определить expected effects для каждого профиля (light/medium/heavy):
   - какие метрики должны падать;
   - на сколько примерно;
   - какие метрики должны оставаться стабильными.
2. Сравнительный benchmark: ideal vs light vs medium vs heavy для каждой mission family.
3. Обновить docs:
   - что моделируется (battery v2, altitude sensor, wind drift, comms jitter);
   - что не моделируется (инерция, GPS noise, реальная аэродинамика);
   - какие assumptions.
4. Исправить README Known Limitations: убрать противоречие.
5. Добавить realism metadata в manifest: active profile, параметры.
6. Stable realism smoke в regression; нестабильные метрики — только experimental.

### Done criteria

- Expected realism effects задокументированы по профилям.
- Comparative benchmark воспроизводим из manifest.
- README не противоречит сам себе.
- Realism smoke suite в regression проходит стабильно.
- Неустойчивые realism checks помечены как experimental.

### Тесты

#### Без рефакторинга

- Battery model v2 unit tests с контролируемыми параметрами.
- Altitude sensor penalty boundary tests.
- Wind drift deterministic tests с фиксированным seed.
- No-fly time window активации/деактивации tests.

#### Лёгкий рефакторинг

- Ideal-vs-realism comparison helper.
- Deterministic fixture для realism profile selection.
- Manifest assertion helpers для realism metadata.

#### Тяжёлый рефакторинг

- Stochastic realism regression.
- Comparative analysis old model vs realism-enabled.

---

## Ветка 6 — Replay / Visualization

**Статус:** делать после стабилизации replay schemas.

**Суть:** сделать поведение миссий видимым для анализа и демонстрации.

### Что сделать

**Шаг 1 — Replay summary для всех mission types:**

Расширить replay CLI:
- Wildfire events: hazard zone updates, threat level changes.
- Realism events: battery drain, sensor misses, comms drops.
- SAR belief summary: entropy progression, detection ticks.
- Inspection graph summary: edge coverage progression.

**Шаг 2 — ASCII overlay:**

- `--tick N`: показать состояние на конкретный тик.
- `--follow`: live follow режим.
- SAR: belief grid с posterior values.
- Inspection: edge coverage с visited/unvisited пометками.
- Wildfire: hazard grid с threat levels.
- Agents: позиции на сетке.

**Шаг 3 — Interactive UI (egui или Bevy):**

- Timeline с событиями.
- Map/grid view с agent trajectories.
- BeliefMap overlay для SAR.
- InspectionGraph overlay.
- Wildfire hazard overlay.
- Strategy comparison viewer.

UI не должен быть обязательным для headless benchmark path.

### Done criteria для Шага 1

- Replay CLI показывает wildfire и realism events.
- Replay summary для всех mission types без паники.
- Event log schema стабильна и задокументирована.

### Done criteria для Шага 2

- ASCII overlay для SAR, inspection, wildfire.
- Headless benchmark path не зависит от overlay.

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
- Screenshot/pixel comparison.

---

## Ветка 7 — Real SITL / PX4

**Статус:** важная ветка к robotics workflow; делать после Ветки 1.

**Суть:** превратить feature-gated MAVLink scaffold в реальный end-to-end workflow.

### Текущее состояние

Mock SITL работает: `MockMavlinkTransport`, `sitl_agent --mock`,
`scenarios/sitl.waypoints.json` отправляет waypoints. Real `MavlinkTransport`
feature-gated, но в `sitl_agent` не используется: `--connection` не создаёт
реальный transport.

### Что сделать

**Этап 1 — Single-agent golden path:**

1. Подключить `MavlinkTransport` в `sitl_agent --connection`:
   - при feature `mavlink-transport` создаётся реальный transport;
   - без feature — понятная ошибка, не silent fallback в mock.
2. Mission upload в PX4:
   - `MISSION_COUNT`;
   - `MISSION_ITEM_INT`;
   - mission ack handling.
3. Telemetry → `TaskStatus`:
   - waypoint reached → task complete;
   - mission failed → task failed.
4. arm/takeoff/execute/abort для одного агента.
5. Safety validation перед upload: geofence, no-fly zones, separation.
6. Обновить `docs/SITL_SETUP.md`: mock mode, real PX4 mode, prerequisites,
   known limitations, troubleshooting.

**Этап 2 — Multi-agent SITL:**

- Несколько агентов одновременно.
- Координация через runtime.
- Failure handling: потеря одного агента → reallocation.

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
- Multi-agent SITL coordination tests.
- Hardware-in-the-loop tests.

---

## Ветка 8 — Platform / API

**Статус:** делать после стабилизации semantics (Ветка 1) и нескольких опытов
добавления новых миссий/стратегий.

**Суть:** снизить стоимость добавления новых миссий и стратегий.

### Риск

Преждевременная API stabilization фиксирует неправильные abstractions. Делать
после того, как `MissionAdapter` wiring устоялся и добавлена хотя бы одна миссия
сверх текущих.

### Что сделать

1. Stable crate boundaries:
   - какие crates публичные: `swarm-types`, `swarm-sim`, `swarm-scenarios`;
   - какие internal: `swarm-runtime`, `swarm-alloc`.
2. Documented extension points:
   - how to add a mission: schema, adapter, builder, scenario files, metrics, replay events;
   - how to add a strategy: allocator trait, registration, benchmark integration;
   - how to add a metric: `RunMetrics`, `AggregateMetrics`, export schema.
3. Semver policy: major для breaking API changes, minor для новых миссий/стратегий.
4. Schema version policy для scenario files и replay log format.
5. Deprecation policy: как убирать старые форматы без breaking changes.
6. Changelog: machine-readable changelog начиная с текущей версии.

### Done criteria

- Документированный path для новой миссии без изменений ядра.
- Документированный path для новой стратегии.
- Stable report schema с version и policy.
- Хотя бы один integration test, который проверяет extension path.

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
