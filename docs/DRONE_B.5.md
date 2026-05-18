# Итоговое направление после DRONE_A.4 / DRONE_B.4

Дата фиксации: 2026-05-18

## Короткий вывод

`DRONE_A.4.md` и `DRONE_B.4.md` предлагают близкую большую цель, но разные ближайшие приоритеты.

Оба документа сходятся в главном:

> проект должен стать цельной исследовательской benchmark-платформой вокруг Swarm Coordination Runtime.

Различие не в конечной цели, а в порядке:

- `DRONE_B.4.md` предлагает сразу собрать Unified Experiment Runner + Mission Benchmark Matrix.
- `DRONE_A.4.md` предлагает сначала закрыть технический долг в CBBA, потому что текущая CBBA-реализация ещё не является по-настоящему распределённой.

Итоговое направление:

> сначала True Distributed CBBA, затем Unified Experiment Runner + Mission Benchmark Matrix.

Так получится не просто красивая benchmark-таблица, а честный publishable result.

## Что общее в A.4 и B.4

Оба документа исходят из того, что Milestone 7-10 уже дали сильный набор компонентов:

- replay;
- JSON/CSV export;
- property-based tests;
- kinematics + battery;
- SAR v1;
- CBBA;
- 5 стратегий;
- runnable scenarios.

Оба ведут к одной цели:

> сделать из набора компонентов цельную исследовательскую benchmark-платформу.

## Главное отличие

### DRONE_B.4

`DRONE_B.4.md` говорит:

> следующий шаг — Unified Experiment Runner + Mission Benchmark Matrix.

То есть:

- собрать Coverage / EmergencyMesh / SAR в единый runner;
- сравнивать 5 стратегий;
- выводить JSON/CSV/replay;
- получить publishable benchmark.

Это правильное продуктовое направление.

### DRONE_A.4

`DRONE_A.4.md` говорит:

> перед этим надо исправить главный технический долг: CBBA сейчас не по-настоящему распределённый.

Это важное замечание.

По текущему коду видно:

- `RuntimeMessage::Cbba` есть;
- `CbbaAllocator::apply_remote_bids()` есть;
- но полноценной tick-loop orchestration через сеть, где агенты реально обмениваются bid-ами и вызывается consensus path, сейчас нет.

Значит текущий CBBA ближе к stateful local allocator, чем к настоящему distributed CBBA.

## Насколько направления отличаются

Направления отличаются не конечной целью, а dependency ordering:

- `B.4` — сначала собрать большую benchmark-платформу.
- `A.4` — сначала сделать CBBA честным, иначе benchmark с CBBA будет методологически слабым.

Это не конфликт.

Это зависимость:

> publishable benchmark должен сравнивать настоящую distributed CBBA, а не local approximation.

## Итоговый Milestone 11

Название:

> True Distributed CBBA + Unified Benchmark Runner.

Milestone 11 должен состоять из двух фаз.

## Phase 1 — True Distributed CBBA

Сначала закрыть CBBA-gap.

### Что реализовать

- `AgentNode::send_cbba_bids(...)`.
- `AgentNode::collect_cbba_messages(...)`.
- Реальную доставку `RuntimeMessage::Cbba` через transport/network.
- Вызов `CbbaAllocator::apply_remote_bids()` с реальными remote bids.
- Convergence на основе обмена bid-ами.
- Tests/proptest для CBBA under packet loss / topology / partitions.

### Почему это первым

Если сразу строить publishable benchmark, то CBBA-строка в таблице будет методологически сомнительной.

CBBA должен отличаться от greedy/auction не только scoring function, но и распределённым consensus loop.

### Критерии готовности Phase 1

- CBBA agents обмениваются bid-ами через `RuntimeMessage::Cbba`.
- `apply_remote_bids()` вызывается с непустыми remote bids в реальном simulation path.
- CBBA convergence metrics отражают реальные consensus rounds.
- Есть тест, который ломается, если CBBA-сообщения не доставляются.
- Есть proptest, где CBBA не паникует при случайных agents/tasks/packet loss/partitions.

## Phase 2 — Unified Experiment Runner

После этого взять направление `DRONE_B.4.md`.

### Что реализовать

- Общий `MissionBenchmark` / `ScenarioSuite`.
- Поддержка missions:
  - `coverage`;
  - `emergency-mesh`;
  - `sar`;
  - `all`.
- CLI:
  - `--mission coverage`;
  - `--mission emergency-mesh`;
  - `--mission sar`;
  - `--mission all`.
- Все 5 стратегий:
  - `centralized`;
  - `greedy`;
  - `auction`;
  - `connectivity-aware`;
  - `cbba`.
- Единый output:
  - JSON;
  - CSV;
  - replay logs;
  - stable `run_id`.
- В каждой строке отчёта:
  - mission;
  - scenario;
  - profile;
  - strategy;
  - seed range / total runs.
- SAR profiles:
  - target_count;
  - scout/thermal/relay mix;
  - packet loss;
  - battery constraints;
  - grid size.
- README с реальными числовыми таблицами.

### Критерии готовности Phase 2

- `strategy_comparison --mission all --json results.json --csv results.csv` запускается.
- В JSON/CSV есть строки по Coverage, EmergencyMesh и SAR.
- В отчёте есть все 5 стратегий.
- SAR metrics экспортируются вместе с общими метриками.
- Replay logs можно включить тем же runner’ом.
- README содержит таблицу с реальными числами из benchmark-прогона.

## Phase 3 — После Milestone 11

После Milestone 11 следующий порядок:

1. **Mission DSL.**
   Ввести YAML/RON/JSON-сценарии после того, как benchmark matrix покажет, какие поля реально нужны.

2. **Uncertainty Map / Sensor Model v2.**
   Углубить SAR: repeated scans, confidence map, false positives, target belief.

3. **Infrastructure Inspection.**
   Следующая reference mission, хорошо проверяющая kinematics, battery, route coverage и missed segments.

4. **Safety Layer.**
   Geofence, no-fly cells, separation, collision avoidance-lite.

## Финальная рекомендация

Не выбирать чисто `A.4` или чисто `B.4`.

Итоговый маршрут:

1. True Distributed CBBA.
2. Unified Experiment Runner + Mission Benchmark Matrix.
3. Mission DSL.
4. Uncertainty Map / Sensor Model v2.
5. Infrastructure Inspection.
6. Safety Layer.

Главная причина такого порядка:

> сначала нужно сделать алгоритм честным, потом строить вокруг него publishable benchmark.

Итоговая формула:

> True Distributed CBBA first; publishable multi-mission benchmark second.
