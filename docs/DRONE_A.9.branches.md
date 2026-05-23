# DRONE_A.9 — Текущий статус и развилка направлений после DRONE_B.8

Дата фиксации: 2026-05-23

## Короткий вывод

Ситуация уже сдвинулась дальше, чем в `DRONE_A.7.md` / `DRONE_B.7.md`.

Тогда общий roadmap был:

```text
M11 hardening -> Mission DSL -> выбор одной из 3 веток
```

Сейчас по локальному коду и `git log` видно, что общий ствол уже сделан, а `DRONE_B.8.md` фактически частично или почти полностью реализован как гибридный roadmap.

Реализованы:

- `112c0d9` — M11 hardening;
- `5573e5d` — Mission DSL v0.12;
- `82a8f59` — Safety Layer M13;
- `e74e456` / `19adb1a` — SAR v2 / Uncertainty Map M14;
- `bace304` / `91a18b1` / `0e66f22` — CBBA Robustness M15;
- `7b5efec` и связанные коммиты — Infrastructure Inspection M16;
- `bc698ca` — SITL / MAVLink M17 scaffold.

`cargo test --workspace` проходит: 241 тест плюс doc-tests.

Итоговый статус:

> проект уже не просто демка, а широкая исследовательская платформа с прототипами всех крупных веток; но это ещё не production system и не готовая real-drone интеграция.

## Что готово

### M11 Hardening

Готово:

- `mission` / `scenario` заполняются в JSON/CSV export;
- `benchmark_run_id` стал mission-aware;
- `seed_range_start` / `seed_range_end` попадают в export;
- добавлены property-based tests для distributed CBBA;
- README обновлён под hardened benchmark output.

Практический статус:

> M11 можно считать закрытым как feature + hardening milestone.

### Mission DSL v0.12

Готово:

- `ScenarioSuite`;
- `ScenarioSuiteEntry`;
- `load_scenario_suite`;
- `export_suite`;
- `--scenario-suite <path>` в `strategy_comparison`;
- JSON scenarios в `scenarios/`;
- unit/integration tests загрузки coverage, emergency-mesh и SAR suite-файлов.

Что это даёт:

- сценарии стали воспроизводимыми артефактами;
- benchmark cases можно добавлять без перекомпиляции;
- появляется основа для scenario catalog и regression suite.

Практический статус:

> DSL уже рабочий, но ему ещё нужен validation layer и тест "все `scenarios/*.json` загружаются".

### Safety Layer M13

Готово:

- новый crate `swarm-safety`;
- `Geofence`;
- `NoFlyZone`;
- `SeparationConstraint`;
- `SafetyConfig`;
- `check_agent`;
- `is_task_reachable`;
- `filter_safe_tasks`;
- `safety_config` в `RunConfig`;
- `safety_violations` в metrics/export;
- `scenarios/coverage.safety.json`.

Практический статус:

> safety layer работает как simulation/runtime constraint layer, но ещё не является real-world safety system.

### SAR v2 / Uncertainty Map M14

Готово:

- `BeliefMap`;
- `BeliefCell`;
- Bayes update;
- entropy;
- `highest_uncertainty_cells`;
- `SensorModel` v2 с `detection_probability` и `false_positive_rate`;
- false positives;
- confirmation scans;
- SAR v2 metrics в `RunMetrics`, `AggregateMetrics`, JSON/CSV/Markdown export;
- `scenarios/sar.uncertain.json`;
- `scenarios/sar.noisy.json`.

Проверено:

- `strategy_comparison --scenario-suite scenarios/sar.uncertain.json --json ...` запускается.

Практический статус:

> SAR v2 стал содержательной исследовательской миссией, но ещё нужен большой benchmark-анализ, чтобы превратить это в publishable result.

### CBBA Robustness M15

Готово:

- `order_bundle_tsp`;
- `avg_bundle_travel_distance`;
- retransmission config:
  - `retransmit_max_attempts`;
  - `retransmit_backoff_ticks`;
  - `retransmit_threshold_packet_loss`;
- convergence percentiles:
  - `convergence_ticks_p50`;
  - `convergence_ticks_p95`;
  - `convergence_ticks_max`;
- `crates/swarm-sim/tests/proptest_cbba.rs`;
- `scenarios/cbba_stress.json`.

Практический статус:

> CBBA уже можно исследовать как algorithmic benchmark, но ещё нужен полный 1000-seed анализ и аккуратная таблица выводов.

### Infrastructure Inspection M16

Готово:

- `InspectionEdge`;
- `InspectionGraph`;
- `linear_route`;
- `grid_perimeter`;
- `random_graph`;
- `InspectionConfig`;
- `InspectionProfile`;
- `build_inspection_scenario`;
- edge tasks через `Task.edge_id`;
- metrics:
  - `edge_coverage_rate`;
  - `missed_edges`;
  - `revisit_count`;
  - `route_efficiency`;
- JSON scenario files:
  - `scenarios/inspection.linear.json`;
  - `scenarios/inspection.perimeter.json`;
  - `scenarios/inspection.random.json`.

Практический статус:

> кодовая часть inspection mission есть, но DSL-сценарии inspection сейчас требуют фикса.

Конкретная проблема:

- `scenarios/inspection.linear.json` и часть inspection JSON содержат `Infinity`;
- `serde_json` не принимает `Infinity`;
- `strategy_comparison --scenario-suite scenarios/inspection.linear.json` падает с `expected value at line 26 column 26`.

Это не ломает unit tests, но ломает пользовательский DSL entrypoint для inspection.

### SITL / MAVLink M17

Готово:

- `crates/swarm-comms/src/mavlink.rs`;
- `MockMavlinkTransport`;
- optional feature `mavlink-transport`;
- `MavlinkTransport`;
- `task_to_waypoint`;
- `task_to_mavlink_waypoint`;
- `mavlink_status_to_task_status`;
- binary `sitl_agent`;
- `docs/SITL_SETUP.md`;
- `cargo test --features mavlink-transport -p swarm-comms` проходит.

Практический статус:

> это пока scaffold / mock-capable SITL layer, а не полноценная PX4 integration.

Конкретные ограничения:

- `sitl_agent` принимает `--connection`, но фактически всегда использует `MockMavlinkTransport`;
- real `MavlinkTransport` не подключён в `sitl_agent`;
- `sitl_agent --mock --scenario scenarios/coverage.ideal.json` отправляет 0 waypoint-ов, потому что в этом suite задачи без `pose`;
- `sitl_agent --mock --scenario scenarios/sar.ideal.json` тоже отправляет 0 waypoint-ов;
- `sitl_agent --mock --scenario scenarios/inspection.linear.json` падает из-за `Infinity` в JSON;
- реальный PX4 loop пока не проверен.

## Что ещё не готово

Главные gaps:

1. **Inspection DSL scenarios невалидны для `serde_json`.**
   Нужно убрать `Infinity` или заменить его на finite max range.

2. **Нет общего теста, что все `scenarios/*.json` загружаются.**
   Сейчас отдельные DSL-тесты покрывают coverage/emergency-mesh/SAR, но не весь каталог.

3. **SITL runner пока mock-only по факту.**
   `--connection` парсится, но не используется для реального transport path.

4. **Нет валидного SITL demo scenario с pose/waypoints.**
   Coverage/SAR DSL-файлы сейчас не дают waypoint-ов для `sitl_agent`, inspection мог бы дать, но JSON не грузится.

5. **Нет полного publishable benchmark анализа.**
   Есть механизм, метрики и сценарии, но нужны 1000-seed runs и интерпретация.

6. **Visualization / Replay UI не реализована.**
   Replay infrastructure есть, UI нет.

## Есть ли сейчас линейное направление

Да, но только короткое.

Сейчас нужен не выбор одной большой ветки, а обязательный стабилизационный слой:

> M17 hardening / platform consolidation.

Он нужен перед любым дальнейшим стратегическим направлением, потому что сейчас реализовано много вертикалей, но часть пользовательских entrypoint-ов ещё шероховатая.

## Ближайший обязательный слой — M17 hardening / platform consolidation

Что сделать:

1. Починить inspection JSON:
   - заменить `Infinity` на конечное значение;
   - проверить `inspection.linear`, `inspection.perimeter`, `inspection.random`.

2. Добавить тест на весь scenario catalog:
   - пройти по `scenarios/*.json`;
   - каждый файл должен грузиться через `load_scenario_suite`;
   - для каждого entry прогнать хотя бы smoke-run или validation.

3. Довести `sitl_agent`:
   - `--mock` использует `MockMavlinkTransport`;
   - `--connection` реально создаёт `MavlinkTransport` при feature `mavlink-transport`;
   - без feature выдаёт понятную ошибку;
   - добавить тесты на CLI parsing и waypoint extraction.

4. Добавить валидный SITL scenario:
   - небольшой JSON suite с 2-3 pose-задачами;
   - `sitl_agent --mock --scenario scenarios/sitl.waypoints.json` должен отправлять waypoints.

5. Добавить CLI smoke tests:
   - `strategy_comparison --scenario-suite scenarios/coverage.safety.json`;
   - `strategy_comparison --scenario-suite scenarios/sar.uncertain.json`;
   - `strategy_comparison --scenario-suite scenarios/cbba_stress.json`;
   - `strategy_comparison --scenario-suite scenarios/inspection.linear.json`;
   - `sitl_agent --mock --scenario scenarios/sitl.waypoints.json`.

6. Обновить README:
   - чётко отделить "mock SITL works" от "real PX4 requires setup";
   - явно указать, что real PX4 path experimental.

Этот слой не меняет стратегию проекта. Он делает текущую широкую реализацию пригодной для дальнейшего движения.

## Дальнейшая развилка

После M17 hardening снова остаются три основных направления.

### 1. Research / Publishable Benchmark

Цель:

> получить сильный исследовательский результат, а не просто набор фич.

Что делать:

- 1000-seed runs по SAR v2, CBBA stress, Infrastructure Inspection;
- таблицы p50/p95 convergence;
- PoD / entropy analysis для SAR v2;
- edge coverage / missed edges / route efficiency для Infrastructure Inspection;
- communication cost между стратегиями;
- сравнение greedy / auction / connectivity-aware / centralized / CBBA;
- reproducible benchmark artifacts;
- методологический отчёт с выводами.

Что даст:

- publishable result;
- понятные trade-offs алгоритмов;
- доказательство, где CBBA полезен, а где нет;
- внятную ценность SAR v2 и inspection missions;
- возможность использовать проект как research benchmark suite.

Где пригодится:

- swarm robotics research;
- distributed systems research;
- multi-agent task allocation;
- портфолио / technical report;
- подготовка academic paper.

Риски:

- нужны долгие прогоны;
- нужно аккуратно интерпретировать результаты;
- без хорошей статистики можно получить красивые, но методологически слабые таблицы.

### 2. Platform / Productization

Цель:

> сделать проект удобным инструментом, а не только кодовой базой.

Что делать:

- versioned DSL schema;
- validation errors вместо panic;
- structured scenario validation;
- stable report schema;
- scenario catalog;
- replay/visualization UI;
- tutorials;
- docs для добавления новых миссий;
- smoke/full benchmark profiles;
- reproducible output directories.

Что даст:

- внешний пользователь сможет запускать и модифицировать сценарии;
- проще показывать проект;
- проще отлаживать benchmark outcomes;
- легче поддерживать regression suite;
- ниже стоимость добавления новых миссий.

Где пригодится:

- demos;
- teaching;
- internal research tooling;
- scenario authoring;
- CI/regression testing;
- командная разработка.

Риски:

- можно потратить много времени на UX/tooling без нового research result;
- важно не уйти в полировку раньше, чем есть финальные метрики и use cases.

### 3. Real-World / SITL Bridge

Цель:

> двигаться от симуляции к настоящему robotics stack.

Что делать:

- подключить настоящий `MavlinkTransport` в `sitl_agent`;
- реализовать mission item upload в PX4 SITL;
- status feedback -> `TaskStatus`;
- single-agent SITL;
- затем multi-agent SITL;
- усилить Safety Layer до operational constraints;
- добавить manual/optional integration tests для PX4 окружения.

Что даст:

- мост к real-world robotics;
- проверку transport abstraction;
- демонстрацию поверх PX4 SITL;
- основу для hardware-in-the-loop.

Где пригодится:

- PX4 SITL;
- robotics demos;
- hardware-in-the-loop;
- будущие реальные дроны;
- middleware experiments for coordinated fleets.

Риски:

- самый дорогой и хрупкий путь;
- требует внешнего окружения;
- симуляционные гарантии не переносятся автоматически на реальные дроны;
- без жёсткого safety layer это нельзя трактовать как готовность к реальному полёту.

## Рекомендация

Ближайший шаг:

> M17 hardening / platform consolidation.

После него основной рекомендуемый фокус:

> Research / Publishable Benchmark.

Почему:

- большая часть исследовательской инфраструктуры уже реализована;
- SAR v2, CBBA robustness и Inspection уже есть в коде;
- осталось превратить набор возможностей в доказательный результат;
- это даст максимальную отдачу от уже сделанной работы.

Real-World / SITL Bridge уже начат, но сейчас это самый дорогой и рискованный путь. Его лучше продолжать после consolidation и после того, как будет понятно, какие сценарии и safety constraints действительно важны.

Platform / Productization полезна, но её лучше делать дозированно: сначала validation и стабильные schema/report, затем visualization и UX.

Итоговый маршрут:

```text
M17 hardening / platform consolidation
-> Research / Publishable Benchmark
-> Platform polish where it supports benchmark/replay
-> Real-World / SITL Bridge only after safety and validation are stronger
```
