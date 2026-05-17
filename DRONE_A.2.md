# Текущий статус проекта после DRONE_A.1 / DRONE_B.1

Дата фиксации: 2026-05-17

## Короткий вывод

Проект уже не пустая демка. Это рабочий mission-level coordination sandbox/runtime prototype примерно до v0.6: есть runtime, симулятор, отказы, ненадёжная сеть, UDP multiprocess, partition/gossip, emergency mesh и сравнение стратегий.

Но это ещё не готовый продукт и не полноценный digital twin из `DRONE_A.1.md` / `DRONE_B.1.md`. Текущий статус точнее описывать так:

> ядро coordination runtime + headless scenario harness, на котором уже можно строить исследовательскую платформу.

## Что готово

По плану `DRONE_A.1.md` / `DRONE_B.1.md` фактически закрыто:

- Workspace-архитектура: `swarm-types`, `swarm-runtime`, `swarm-comms`, `swarm-sim`, `swarm-alloc`, `swarm-scenarios`, `swarm-metrics`, `swarm-examples`.
- Runtime foundation: membership, heartbeat, failure detection, task registry, task ownership, reallocation.
- Deterministic scenario runner: seed-based in-process simulation.
- Comms model: packet loss, latency, hop latency, partitions, in-memory transport.
- UDP/multiprocess: есть `agent_process` и `multiprocess_scenario`.
- Task allocation: greedy, auction, connectivity-aware, centralized baseline.
- Dynamic tasks: injection, expiration, capability/role constraints.
- Partial connectivity: gossip/anti-entropy, stale heartbeat protection через generation, convergence after partition.
- Emergency Mesh: relay roles, ground nodes, comms range, network availability metrics.
- Strategy comparison platform: quick/full benchmark, profiles сети/отказов, markdown report, 4 стратегии.
- Metrics: success, detection, reallocation, messages, bytes, dropped, conflicts, stale state, coverage proxy, battery margin, network availability.

## Что не готово

Крупные части из `DRONE_A.1.md` / `DRONE_B.1.md` ещё отсутствуют:

- Mission DSL YAML/RON и загрузчики сценариев.
- Настоящие SAR / Wildfire / Infrastructure Inspection миссии.
- CBBA как consensus-based bundle algorithm. Сейчас есть auction, но не CBBA.
- Нормальная kinematic simulation: сейчас движение очень грубое, по сути pose update к assigned task.
- Battery drain / energy model.
- Sensor/world model: uncertainty, false positive/false negative, probability of detection.
- Replay: `swarm-replay` пока placeholder.
- CSV/JSON export для анализа; сейчас в основном Display/markdown.
- Property-based tests и benchmark suite через `proptest`/`criterion`.
- Safety layer: geofence, separation, collision avoidance.
- PX4/MAVLink/zenoh интеграции.
- Устойчивая публичная API-граница runtime как продукта.

## Куда двигаться дальше

Не стоит идти сразу в PX4/визуализацию. Следующий лучший шаг — сделать платформу исследовательски полезной, а не просто runnable.

### Milestone 7: Replay + structured reports

- Event log в `swarm-replay`.
- JSON/CSV export для `ComparisonReport`.
- Deterministic replay одного seed/profile/strategy.
- CLI: `strategy_comparison --json out.json --csv out.csv`.

Это даст базу для анализа, регрессий и будущих публикационных графиков.

### Milestone 8: Mission DSL

- YAML/RON scenario schema.
- Loader в `swarm-scenarios`.
- Сначала покрыть существующие Coverage/EmergencyMesh.
- Затем описывать миссии без правки Rust-кода.

### Milestone 9: настоящая SAR mission

- Area/grid.
- Hidden target.
- Scout/thermal/relay roles.
- Metrics: probability_of_detection, time_to_find, coverage over time, network availability.

Это приблизит проект к `DRONE_B.1.md`: настоящий benchmark, не песочница.

### Milestone 10: CBBA

- Отдельная стратегия в `swarm-alloc`.
- Message/round model.
- Сравнение с greedy/auction/centralized/relay-aware на SAR + EmergencyMesh.

## Рекомендация

Ближайший шаг:

> Milestone 7 — Replay + structured reports.

Это менее эффектно, чем SAR/CBBA, но сильно повышает качество фундамента: после него любые новые стратегии и миссии можно сравнивать, воспроизводить и анализировать нормально.
