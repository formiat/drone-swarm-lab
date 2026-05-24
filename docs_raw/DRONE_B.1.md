# Mission-Level Digital Twin — итоговый план

> Синтез DRONE_A.md и DRONE_B.md. Сессия: май 2026.
> Контекст: Rust-разработчик, нет физических дронов, серьёзный долгосрочный проект.

---

## Формулировка проекта

**Mission-Level Digital Twin for Autonomous Drone Fleet/Swarm Systems**

Платформа для моделирования, сравнения и валидации стратегий управления автономными
миссиями групп дронов — с упором на distributed coordination, resilient communications
и failure handling.

Ключевая установка:

> Объект моделирования — не пропеллеры, не полёт как самоцель.
> Объект моделирования — миссия: как группа аппаратов решает задачу
> в условиях отказов, ограниченной связи и неполной информации.

---

## Что дрон в этой системе

Не физическое тело с моторами. Ресурс с возможностями:

```
vehicle:
  id, role
  position          # (x, y) или (lat, lon)
  battery           # текущий заряд, энергомодель
  speed_limit
  sensor_range
  comms_range
  payload
  health            # alive / degraded / dead
  current_task
```

Физика подключается позже как один из backend'ов — не в центре проекта.

---

## Reference missions (benchmarks)

Архитектура проектируется вокруг конкретных миссий, а не абстрактных агентов.
Каждая миссия — это benchmark: алгоритм либо решает задачу, либо нет.

### 1. Search and Rescue (SAR)

```
Область: 10×10 км
Флот: 8 дронов (2 thermal, 2 relay, 4 scout)
Связь: только в части области
Цель: неизвестна
Событие: один дрон теряется через 20 минут
```

Метрики: probability of detection, time to find, coverage%, network availability.

### 2. Wildfire Mapping

```
Фронт пожара движется.
Часть зоны опасна для входа.
Периодическая актуализация тепловой карты.
Связь деградирует.
```

Метрики: map freshness, coverage of active front, safe separation from hazard zone.

### 3. Infrastructure Inspection

```
Линия ЛЭП или трубопровод.
Полное покрытие обязательно.
Battery constraints.
Повторяемость маршрутов.
```

Метрики: coverage completeness, mission time, battery margin, missed segments.

### 4. Emergency Mesh Network

```
Зона катастрофы, наземная связь разрушена.
База + наземные точки которым нужна связь.
Часть дронов — relay.
Наземные узлы могут двигаться.
```

Метрики: network availability over time, connectivity %, relay repositioning latency.

---

## Слои системы

```
┌──────────────────────────────────────────────────────────┐
│  Mission Layer       цели, ограничения, приоритеты,      │
│                      Mission DSL (YAML/RON)              │
├──────────────────────────────────────────────────────────┤
│  Planning Layer      декомпозиция миссии, задачи,        │
│                      маршруты, расписание                │
├──────────────────────────────────────────────────────────┤
│  Fleet Layer         аппараты, роли, статусы, батареи,   │
│                      payload, health, lifecycle          │
├──────────────────────────────────────────────────────────┤
│  Coordination Layer  task allocation, consensus,         │  ← Фаза 2
│                      failure detection, reallocation     │
├──────────────────────────────────────────────────────────┤
│  Comms Layer         mesh, latency, packet loss,         │
│                      routing, bandwidth, partition       │
├──────────────────────────────────────────────────────────┤
│  World Model Layer   карта, зоны, цели, uncertainty,     │
│                      динамические события                │
├──────────────────────────────────────────────────────────┤
│  Safety Layer        geofence, separation, CA, failsafe  │
├──────────────────────────────────────────────────────────┤
│  Sensor Model        вероятностные модели сенсоров,      │
│                      false positive / false negative     │
├──────────────────────────────────────────────────────────┤
│  Vehicle Interface   MAVLink / PX4 SITL (поздний этап)   │
└──────────────────────────────────────────────────────────┘
```

---

## Уровни симуляции

### A — Mission Simulation (старт)

Дрон — ресурс. Физики нет. Моделируются задачи, роли, связь, отказы.
Это главный уровень на старте.

### B — Kinematic Simulation

```
position += velocity * dt
```

Маршруты, столкновения, separation, оценка времени и батареи.
Без моторов, PID и аэродинамики.

### C — Communication Simulation

```
link exists if distance < range
packet_loss = f(distance, obstacles, congestion)
latency = base + jitter + queue_delay
messages may be dropped / reordered / delayed
```

Проверка mesh, relay-дронов, consensus, distributed task allocation под нагрузкой.
Для роя связь важнее физики полёта.

### D — Sensor / World Model Simulation

```
thermal_sensor:
  range: 120m
  false_positive_rate: 0.02
  false_negative_rate: 0.15
  detection_probability: f(distance, weather, occlusion)
```

Моделирование обнаружения целей, uncertainty map, sensor fusion.

### E — Vehicle / Autopilot (поздний этап)

PX4 SITL, ArduPilot SITL, Gazebo. Подключается когда нужно
верифицировать совместимость с реальным autopilot.

---

## Фазы разработки

**Жёсткое правило: каждая фаза заканчивается чем-то запускаемым.**
Нет фазы проектирования без артефакта.

---

### Фаза 1 — Фундамент

**Результат**: запускаемый сценарий SAR на уровне A (mission simulation).

Что строим:

**Mission DSL** — формат описания сценариев:

```yaml
mission:
  type: search_and_rescue
  area: { width_km: 10, height_km: 10 }
  objectives:
    - maximize_probability_of_detection
    - maintain_comms_to_base
  constraints:
    max_mission_time_min: 45
    return_battery_percent: 20

fleet:
  - count: 4
    role: scout
    sensor: optical
  - count: 2
    role: scout
    sensor: thermal
  - count: 2
    role: relay
```

**Fleet model** — Vehicle struct, roles, battery model, health states.

**World model** — карта, зоны, цели (известные и скрытые), опасные зоны.

**Comms model** — link existence by range, configurable packet_loss и latency.

**Scenario runner** — детерминированный (seed-based), headless, воспроизводимый.

**Metrics engine** — coverage%, detection probability, mission time, network availability.

Стратегия управления на этой фазе: простейшая (greedy / random) — нужна только чтобы
было что измерять. Алгоритмы придут в фазе 2.

---

### Фаза 2 — Coordination Layer (первый серьёзный компонент)

**Результат**: coordination runtime, тестируемый независимо и встроенный в платформу.

Что строим:

**Membership + heartbeat** — кто жив, failure detection по таймауту.

**Distributed task allocation** — CBBA (Consensus-Based Bundle Algorithm):
- агенты торгуются за задачи через сообщения
- работает при неполной связности
- сходится без центрального координатора

**Failure recovery** — при смерти агента его задачи возвращаются в пул и
перераспределяются оставшимися.

**Pluggable transport** — протокол поверх абстрактного Message<From, To, Payload>,
не привязан к UDP / MAVLink / zenoh.

Тестирование — независимо от симулятора:

```rust
// Уровень 1: чистые unit-тесты алгоритма
#[test]
fn cbba_reallocates_on_agent_death() {
    let mut swarm = SwarmSim::new(5);
    swarm.assign_tasks(tasks);
    swarm.kill_agent(2);
    swarm.run_rounds(10);
    assert!(swarm.all_tasks_covered());
}

// Уровень 2: in-process с simulated network
let net = SimNetwork { packet_loss: 0.3, latency_ms: 50, partition: vec![] };
// → property-based тесты, 1000 случайных сценариев отказов

// Уровень 3: N OS-процессов через UDP loopback
// kill -9 на процесс = тест failure detection
```

На этой фазе coordination runtime одновременно:
1. Компонент внутри digital twin
2. Standalone библиотека — потенциально запускаемая на реальном железе

---

### Фаза 3 — Сравнение стратегий

**Результат**: исследовательская платформа, сравнивающая алгоритмы по метрикам.

Что строим:

Несколько стратегий управления за один интерфейс:

```rust
trait MissionStrategy {
    fn assign_tasks(&self, fleet: &Fleet, world: &World) -> TaskAssignment;
    fn replan(&self, event: &Event, state: &SimState) -> TaskAssignment;
}
```

Реализации:
1. Centralized planner (оптимально, нет отказоустойчивости)
2. Greedy decentralized (просто, быстро)
3. CBBA / auction-based (из фазы 2)
4. Relay-aware strategy (учитывает связность при назначении)

Сравнительный запуск:

```
1000 сценариев × 4 стратегии × reference missions SAR + Wildfire

Отчёт:
  success_rate
  mean_mission_time
  coverage_percent
  network_availability
  messages_per_minute
  battery_margin
  degradation under 10% / 20% / 30% agent loss
  degradation under 10% / 30% / 50% packet loss
```

Это уже публикуемый результат, не песочница.

---

### Фаза 4 — Расширение (по приоритетам)

В любом порядке, по интересу:

- **Safety layer**: geofence, minimum separation, collision avoidance (ORCA)
- **Sensor model**: probabilistic detection, false positives, uncertainty map
- **Kinematic simulation**: реальные маршруты, battery drain по расстоянию
- **PX4 SITL backend**: валидация на реальном flight stack
- **Visualization**: rerun.io или Bevy — карта, траектории, replay

---

## Архитектура симулятора

```
+----------------------+
| Scenario Definition  |
| YAML / RON           |
+----------+-----------+
           |
           v
+----------------------+
| Simulation Runtime   |  seed-based, event-driven, headless-first
| clock, events        |
+----------+-----------+
           |
     ┌─────┴──────┐
     v            v
+----------+  +----------+
| World    |  | Fleet    |
| Model    |  | Model    |
+----------+  +----------+
     |            |
     └─────┬──────┘
           v
+----------------------+
| Comms Model          |  packet loss, latency, partition
+----------+-----------+
           |
           v
+----------------------+
| Coordination Layer   |  ← Фаза 2: task allocation, consensus, failure
+----------+-----------+
           |
           v
+----------------------+
| Metrics / Replay     |  CSV / JSON, воспроизводимость, сравнение
+----------------------+
```

---

## Стек

```toml
# Rust core
serde = { features = ["derive"] }   # сценарии
serde_yaml / ron                    # Mission DSL
petgraph                            # граф связности, mesh
nalgebra / glam                     # математика
rand / rand_pcg                     # seed-based PRNG
tracing                             # логи
criterion                           # benchmark
proptest                            # property-based тесты

# Визуализация (опционально, поздно)
rerun                               # 3D replay и траектории
# или
bevy                                # интерактивная визуализация
egui                                # простой UI поверх

# Анализ результатов
polars / python notebooks           # обработка CSV с метриками
```

---

## Что НЕ строим

- Не симулируем моторы, пропеллеры, аэродинамику — это задача PX4/Gazebo
- Не делаем фотореалистичную визуализацию — не цель
- Не делаем ещё один Gazebo / ARGoS — они существуют
- Визуализация — не главный продукт, главный продукт — headless sim + метрики

---

## Что считать настоящим результатом

Не "в окне летают точки".

```
Запуск 1000 сценариев SAR с разными seed.

Сравнение 4 стратегий:
  centralized planner
  greedy decentralized
  CBBA auction-based
  relay-aware

Отчёт по каждой:
  success rate при 0% / 10% / 20% / 30% agent loss
  mean time to detection
  coverage at T=45min
  network availability
  messages per minute
  battery margin distribution
```

Это исследовательская платформа. Это публикуемый результат.
Это не зависит от наличия физических дронов.

---

## Связанные файлы

- `SWARM_2.md` — базовый конспект: алгоритмы, железо, симуляторы, рой vs флот
- `DRONE_A.md` — альтернативный план: Mission-Level Digital Twin, макро-архитектура
- `DRONE_B.md` — Swarm Coordination Runtime: distributed systems подход, пирамида тестирования
