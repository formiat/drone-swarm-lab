# Текущий статус после DRONE_A.3 / DRONE_B.3

Дата фиксации: 2026-05-17

## Короткий вывод

`docs/DRONE_A.3.md` и `docs/DRONE_B.3.md` уже устарели относительно текущего `HEAD`.

Они планировали Milestone 7-10:

- experiment infrastructure;
- replay/export/proptest;
- kinematics/battery;
- SAR;
- CBBA.

В текущем коде эти этапы уже в основном реализованы.

Текущий проект — это уже не просто runnable prototype. Это набор рабочих компонентов для mission-level coordination runtime и headless benchmark harness.

Следующая задача — собрать эти компоненты в цельную исследовательскую платформу.

## Что готово

Фактически закрыты Milestone 1-10.

### Milestone 1-6

Готово:

- runtime foundation;
- heartbeat / membership;
- failure detection;
- task ownership;
- reallocation;
- deterministic in-process simulation;
- UDP multiprocess;
- packet loss / latency / partitions;
- gossip convergence;
- Emergency Mesh;
- strategy comparison.

Стратегии:

- `centralized`;
- `greedy`;
- `auction`;
- `connectivity-aware`;
- `cbba`.

### Milestone 7 — Experiment Infrastructure

Готово:

- `swarm-replay` больше не placeholder;
- `EventLog`;
- replay reconstruction;
- JSON serialization;
- `ComparisonReport` с `benchmark_run_id`;
- JSON/CSV export;
- CLI-флаги для `strategy_comparison`:
  - `--json`;
  - `--csv`;
  - `--replay-log`;
  - `--run-id-prefix`;
- property-based tests через `proptest`.

### Milestone 8 — Kinematic + Battery

Готово:

- `Agent::speed`;
- `Agent::max_range`;
- `Agent::battery_drain_rate`;
- движение к task pose;
- battery drain;
- exhausted agents исключаются из allocation;
- movement влияет на connectivity.

### Milestone 9 — SAR v1

Готово:

- `SearchGrid`;
- hidden targets;
- `SensorModel`;
- scout / thermal / relay roles;
- `GridState`;
- SAR metrics:
  - `time_to_find`;
  - `coverage_over_time`;
  - `probability_of_detection`;
  - `targets_found`;
  - `scan_count`;
- runnable `sar_scenario`.

### Milestone 10 — CBBA

Готово:

- `CbbaAllocator`;
- bundle building;
- winning bids;
- consensus через remote bids;
- CBBA metrics:
  - `cbba_rounds_to_convergence`;
  - `cbba_converged`;
  - `cbba_messages`;
- `StrategyRegistry` включает 5 стратегий.

## Что ещё не готово

Остались более продуктовые и исследовательские слои:

- Mission DSL YAML/RON/JSON как полноценный формат сценариев.
- Wildfire Mapping.
- Infrastructure Inspection.
- Более полноценная uncertainty map, не только cell states и probability of detection.
- Более строгий SAR benchmark matrix.
- SAR пока отдельный scenario, а не полноценная часть `strategy_comparison`.
- Сравнение CBBA именно на SAR + EmergencyMesh как основном publishable benchmark.
- Safety layer:
  - geofence;
  - separation;
  - collision avoidance.
- Более зрелый replay: сейчас replay reconstructs state from events, но это ещё не полноценный time-travel/debugger.
- Нормальная CLI / experiment runner архитектура: `strategy_comparison` всё ещё в основном coverage-oriented.
- PX4 / MAVLink / zenoh / visualization — за горизонтом.

## Ключевая развилка

Раньше вопрос был:

> делать replay/proptest/SAR/CBBA или нет?

Сейчас это уже сделано.

Теперь главный вопрос:

> как превратить набор реализованных компонентов в цельную исследовательскую платформу?

Сейчас код — это сильный набор модулей и runnable examples.

Но публикуемый результат требует единого experiment runner, где можно честно сравнить стратегии на нескольких reference missions.

## Итоговое направление

Следующий этап:

> Milestone 11 — Unified Experiment Runner + Mission Benchmark Matrix.

Цель:

> собрать уже реализованное в единую систему экспериментов.

## Milestone 11 — Unified Experiment Runner + Mission Benchmark Matrix

Что сделать:

- Ввести общий `MissionBenchmark` / `ScenarioSuite` интерфейс.
- Подключить reference missions:
  - Coverage;
  - EmergencyMesh;
  - SAR;
  - позже Wildfire;
  - позже Infrastructure Inspection.
- Расширить `strategy_comparison`, чтобы он гонял не только Coverage:
  - `--mission coverage`;
  - `--mission emergency-mesh`;
  - `--mission sar`;
  - `--mission all`.
- Единый output:
  - JSON;
  - CSV;
  - replay logs;
  - stable `run_id`;
  - scenario / profile / strategy / seed в каждой строке.
- Сравнение всех 5 стратегий:
  - `centralized`;
  - `greedy`;
  - `auction`;
  - `connectivity-aware`;
  - `cbba`.
- Добавить SAR profiles:
  - target_count;
  - scout/thermal/relay mix;
  - packet loss;
  - battery constraints;
  - grid size.
- Добавить invariants:
  - no duplicate ownership;
  - bounded success rate;
  - no NaN metrics;
  - replay can reconstruct final assignment state;
  - CBBA convergence metric is populated where CBBA is used.

## Почему именно это

Milestone 7-10 дали компоненты.

Следующий уровень — не новый компонент, а связка компонентов в reproducible benchmark platform.

Это превращает проект из набора сильных milestones в цельную исследовательскую платформу.

## Что делать после Milestone 11

### Milestone 12 — Mission DSL

Ввести декларативное описание сценариев после того, как станет понятно, какие поля реально нужны для Coverage / EmergencyMesh / SAR benchmark matrix.

### Milestone 13 — Uncertainty Map / Sensor Model v2

Углубить SAR:

- повторные scans;
- confidence map;
- false positives;
- target belief.

### Milestone 14 — Infrastructure Inspection или Wildfire

Лучше начать с Infrastructure Inspection, потому что она хорошо проверяет:

- kinematics;
- battery;
- route coverage;
- missed segments;
- повторяемость маршрутов.

### Milestone 15 — Safety Layer

Добавить:

- geofence;
- no-fly cells;
- separation;
- collision avoidance-lite.

## Финальная рекомендация

Следующий правильный шаг — не писать ещё один алгоритм.

Нужно собрать текущие алгоритмы, replay/export/proptest и SAR/EmergencyMesh в единый benchmark runner.

Итоговая формула следующего этапа:

> Unified Experiment Runner + Mission Benchmark Matrix.

Это самый прямой путь от текущего набора компонентов к серьёзной исследовательской платформе.
