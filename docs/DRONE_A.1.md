# Итоговый план: Swarm Coordination Runtime + Mission Digital Twin

Дата фиксации: 2026-05-15

## Главный вывод

Лучшее направление - не маленькая учебная песочница и не абстрактный "симулятор роя".

Итоговый проект:

> Swarm Coordination Runtime для автономных drone/fleet/swarm-систем.

Вокруг него нужен:

> Mission Digital Twin / Scenario Test Harness как испытательный стенд.

То есть:

- **runtime** - основной продукт;
- **digital twin** - среда разработки, проверки, сравнения стратегий и воспроизведения сценариев.

Итоговая формула:

```text
Mission Scenarios / Digital Twin
        ↓
Swarm Coordination Runtime
        ↓
Pluggable Transport
        ↓
Simulated agents / UDP processes / PX4 SITL later
```

## Что берем из DRONE_A.md

`DRONE_A.md` полезен как широкая карта домена:

- применения дронов и роев;
- архитектурные оси;
- mission/fleet/world/comms/safety model;
- reference missions;
- метрики;
- уровни симуляции;
- понимание, что без физических дронов можно серьезно работать на уровне mission autonomy, task allocation, communications и failure handling.

Слабость `DRONE_A.md`: слишком широкий объект разработки. "Mission-level digital twin" может расползтись в огромную платформу без четкого ядра.

## Что берем из DRONE_B.md

`DRONE_B.md` сильнее как проектная фокусировка:

- конкретный продукт: **Swarm Coordination Runtime**;
- четкая граница слоя: между mission/application и transport/autopilot/simulator;
- правильная аналогия с distributed systems;
- сильная пирамида тестирования;
- runtime можно разрабатывать без дронов;
- есть инкрементальный roadmap.

Слабость `DRONE_B.md`: runtime без mission/digital-twin harness рискует стать распределенной библиотекой в вакууме. Ему нужны reference scenarios, метрики и воспроизводимая среда проверки.

## Итоговая позиция

Не digital twin как самоцель.

Не runtime без сценариев.

А связка:

> Coordination Runtime as the product, Mission Digital Twin as the proving ground.

По-русски:

> Runtime координации как ядро продукта, цифровой двойник миссий как стенд для разработки и валидации.

## Что такое Swarm Coordination Runtime

Это библиотека/демон/процесс, который запускается на каждом агенте - физическом, симулируемом или тестовом - и реализует coordination layer.

Он находится между mission/application layer и транспортом/autopilot/simulator.

```text
┌─────────────────────────────────────────────────────┐
│              Mission / Application                  │
├─────────────────────────────────────────────────────┤
│         Swarm Coordination Runtime  ← ПРОЕКТ        │
│  • membership + heartbeat                           │
│  • failure detection                                │
│  • distributed task allocation                      │
│  • reallocation after failure                       │
│  • shared state exchange                            │
│  • consensus / agreement where needed               │
│  • partial connectivity handling                    │
│  • degraded-mode behavior                           │
├─────────────────────────────────────────────────────┤
│    Transport: in-memory / UDP / zenoh / MAVLink      │
├─────────────────────────────────────────────────────┤
│       Simulated agent / PX4 / ArduPilot / robot      │
└─────────────────────────────────────────────────────┘
```

Runtime не должен управлять моторами и не должен быть автопилотом.

Его задача:

- знать, кто жив;
- знать, какие задачи существуют;
- знать, кто за что отвечает;
- обнаруживать отказ или потерю связи;
- перераспределять задачи;
- обмениваться минимальным состоянием;
- работать при packet loss, latency, partitions и stale data;
- предоставлять mission layer понятный API.

## Почему это серьезный проект

Coordination runtime - это задача distributed systems в drone/swarm-домене.

"Дрон" для runtime - это агент с:

- ID;
- состоянием;
- capabilities;
- позицией;
- задачами;
- inbox/outbox;
- health;
- ограничениями.

Неважно, откуда берется позиция:

- из простой кинематики;
- из in-process simulation;
- из UDP-процесса;
- из Gazebo;
- из PX4 SITL;
- из реального MAVLink.

Разработка протоколов, отказоустойчивости, membership, task allocation и reallocation не требует физических дронов.

Аналогия:

> Raft, Paxos, TCP, etcd и CockroachDB не разрабатывают сначала на "реальном кластере в поле". Протоколы тестируют на тысячах симулированных сценариев с потерями пакетов, задержками и отказами.

Для swarm coordination подход такой же.

## Чего проект не делает на старте

Не писать:

- свой автопилот;
- PID/stabilization/control layer;
- физику моторов;
- photorealistic simulator;
- замену Gazebo;
- полную Mission Planner/QGroundControl альтернативу;
- MARL как первый шаг;
- Byzantine fault tolerance в ранних версиях.

Важно: Byzantine fault tolerance можно оставить как дальний исследовательский горизонт, но не включать в v0.x. Сначала нужно покрыть более практичные failure modes:

- crash failures;
- omission failures;
- message loss;
- latency;
- partitions;
- stale state;
- slow node;
- inconsistent local views.

## Mission Digital Twin / Scenario Test Harness

Digital twin в этом проекте - не отдельная огромная платформа, а испытательный стенд для runtime.

Он нужен для:

- описания сценариев;
- моделирования мира;
- моделирования флота;
- моделирования связи;
- инъекции отказов;
- воспроизводимых прогонов;
- property-based тестов;
- сбора метрик;
- replay;
- сравнения стратегий.

Минимальная модель сценария:

```text
Scenario
  World
  Fleet
  Tasks
  Comms
  Events
  Failures
  Metrics
```

Digital twin не обязан идеально симулировать аэродинамику. На раннем этапе ему достаточно mission/kinematic/comms/sensor-level моделей.

## Reference missions

Runtime должен проверяться не на абстрактных агентах в пустоте, а на нескольких эталонных миссиях.

### 1. Coverage with Failure

Минимальный первый сценарий.

Условия:

- есть область покрытия;
- есть 5-20 агентов;
- есть набор coverage tasks;
- один агент падает;
- runtime должен обнаружить отказ;
- задачи упавшего агента должны быть перераспределены.

Что проверять:

- время обнаружения отказа;
- время перераспределения;
- все ли задачи назначены;
- сколько сообщений ушло;
- как влияет packet loss;
- как влияет задержка.

### 2. Emergency Mesh Network

Сценарий для связи и relay-логики.

Условия:

- зона катастрофы;
- базовая станция видит не всех;
- есть ground nodes;
- есть scout-агенты и relay-агенты;
- сеть может разделяться;
- packet loss и bandwidth ограничены.

Что проверять:

- network availability;
- relay placement;
- сколько времени сеть связна;
- что происходит при потере relay;
- как быстро runtime перестраивает роли.

### 3. Search and Rescue

Сценарий для task allocation и uncertainty.

Условия:

- большая область;
- неизвестные цели;
- агенты с разными сенсорами;
- ограниченная связь;
- батарея ограничена;
- цель может быть обнаружена с false positive/false negative.

Что проверять:

- probability of detection;
- coverage over time;
- кому назначать проверку цели;
- когда возвращать агента;
- как работает reallocation.

### 4. Infrastructure Inspection

Сценарий для repeatable mission execution.

Условия:

- линия/граф объектов: ЛЭП, трубопровод, солнечная ферма;
- агенты с разной скоростью, сенсорами и батареей;
- важны покрытие и повторяемость.

Что проверять:

- task assignment;
- route coverage;
- battery margin;
- missed segments;
- reinspection tasks.

## Пирамида тестирования

### Уровень 1: Pure Unit Tests

Тестируется алгоритм без физики, сети и процессов.

Пример:

```rust
#[test]
fn reallocates_tasks_after_agent_death() {
    let mut swarm = TestSwarm::new(5);
    swarm.assign_tasks(vec![Task::cover("zone_a"), Task::cover("zone_b")]);
    swarm.kill_agent(AgentId(2));
    swarm.run_rounds(10);
    assert!(swarm.all_tasks_assigned());
}
```

Цель:

- проверить membership logic;
- проверить task state transitions;
- проверить reallocation;
- проверить invariants.

### Уровень 2: In-Process Async Simulation

N агентов как N async tasks в одном процессе.

Между ними - управляемая сеть:

```rust
struct SimNetwork {
    packet_loss: f32,
    latency_ms: Range<u64>,
    bandwidth_bps: u64,
    partitions: Vec<(AgentId, AgentId)>,
    jitter_ms: Range<u64>,
}
```

Это основная среда тестирования.

Что проверять:

- сходимость при потерях пакетов;
- поведение при partitions;
- stale state;
- slow agents;
- retransmission / gossip / heartbeat intervals;
- property-based сценарии через `proptest`.

### Уровень 3: Multi-Process Simulation

Каждый агент - отдельный OS-процесс.

Transport:

- UDP loopback;
- localhost ports;
- позже zenoh.

Physics:

- fake physics;
- простая кинематика;
- `position += velocity * dt`.

Цель:

- проверить real network stack;
- проверить serialization;
- проверить process crash;
- проверить recovery;
- проверить observability.

`kill -9` процесса - полезный тест failure detection.

### Уровень 4: PX4 / ArduPilot SITL

Поздний интеграционный уровень.

Цель:

- проверить совместимость команд с автопилотом;
- проверить MAVLink adapter;
- проверить, что runtime может работать с реальным vehicle backend.

Использовать редко, как integration test, не как основной цикл разработки.

## Предлагаемая структура репозитория

```text
swarm-lab/
  crates/
    swarm-types/
      # AgentId, TaskId, Pose, Velocity, Health, Capability, Role, Message

    swarm-runtime/
      # membership, heartbeat, failure detection, task ownership state machine

    swarm-alloc/
      # greedy, auction, CBBA later

    swarm-comms/
      # Transport trait, in-memory transport, UDP transport, simulated network

    swarm-sim/
      # scenario runner, simulation clock, event injection, fake kinematics

    swarm-metrics/
      # invariants, counters, reports, traces, CSV/JSON export

    swarm-replay/
      # event log, deterministic replay

    swarm-scenarios/
      # YAML/RON/JSON scenario definitions and loaders

    swarm-examples/
      # coverage_failure, emergency_mesh, sar, inspection
```

## Core abstractions

Минимальные сущности:

```text
AgentId
TaskId
MessageId
Epoch / Term / Round

Pose
Velocity
Battery
Health
Capability
Role

Task
TaskClaim
TaskAssignment
TaskStatus

Heartbeat
MembershipView
FailureDetector

Transport
NetworkModel
Inbox / Outbox

Scenario
Event
Failure
Metric
TraceEvent
```

Критичный принцип:

> runtime не должен знать, реальный агент перед ним или симулированный.

Он должен работать через абстракции:

```text
Transport
Clock
StateProvider
TaskProvider
CommandSink
```

## v0.1: первый серьезный milestone

Название:

> Coordination Runtime for resilient task reallocation under agent failure.

Состав:

- 5-20 агентов;
- heartbeat;
- membership;
- crash failure detection;
- task ownership;
- task reallocation after failure;
- in-memory simulated network;
- packet loss;
- latency;
- deterministic seed;
- property-based tests;
- один reference scenario: coverage with failure;
- метрики и отчет.

Успешный результат:

```text
Запускается 1000 сценариев с разными seed.
При crash failure одного агента задачи перераспределяются.
Инвариант: если жив хотя бы один capable agent, задача не остается unassigned дольше N rounds.
Измеряются detection time, reallocation time, message count, success rate.
```

## v0.2

Фокус:

- динамические задачи;
- простая auction-based allocation;
- priorities;
- task expiration;
- agent capability matching.

Результат:

- новые задачи появляются во время миссии;
- агенты конкурируют за задачи;
- задачи назначаются с учетом capabilities и расстояния/стоимости.

## v0.3

Фокус:

- pluggable transport;
- UDP transport;
- multi-process agents;
- process crash tests;
- basic observability.

Результат:

- один и тот же runtime работает in-process и как N процессов;
- можно убить процесс агента;
- остальные обнаруживают отказ и перераспределяют задачи.

## v0.4

Фокус:

- partial connectivity;
- network partitions;
- gossip;
- stale state handling;
- simple mesh behavior.

Результат:

- агенты имеют разные local views;
- runtime не разваливается при partition;
- после восстановления связи система сходится к согласованному task ownership.

## v0.5

Фокус:

- reference mission: emergency mesh;
- relay roles;
- connectivity-aware task allocation;
- metrics for network availability.

Результат:

- runtime может назначать relay-задачи;
- потеря relay приводит к reallocation;
- измеряется network availability over time.

## Что отложить за v0.x

- Byzantine fault tolerance;
- MARL;
- photorealistic simulation;
- full ROS 2 integration;
- advanced PX4/Gazebo workflows;
- real hardware;
- сложный SLAM/VIO;
- security/crypto beyond simple authentication;
- combat/offensive scenarios.

## Метрики

Минимальный набор:

- membership convergence time;
- failure detection time;
- task reallocation time;
- task unassigned duration;
- message count;
- bytes sent;
- success rate;
- duplicate task ownership count;
- conflicting assignments;
- network partition duration;
- stale state age;
- coverage progress;
- battery margin, если включена модель энергии.

## Инварианты

Примеры:

- task не должен быть одновременно owned двумя агентами, если система находится в stable connected state;
- task не должен оставаться unassigned дольше N rounds, если есть capable agent и сеть связна;
- dead agent не должен оставаться active member дольше failure timeout + grace period;
- после восстановления partition система должна сходиться;
- runtime не должен паниковать при duplicate, delayed или reordered messages;
- stale heartbeat не должен оживлять давно умершего агента без новой epoch/term логики.

## Технологический выбор

Базовый стек:

```text
Rust
tokio
serde
thiserror / anyhow
tracing
proptest
criterion
petgraph
nalgebra или glam
ron / yaml / json для сценариев
```

Визуализация:

- не обязательна в v0.1;
- можно добавить позже через egui или Bevy;
- не должна определять архитектуру runtime.

Интеграции позже:

- zenoh;
- MAVLink;
- PX4 SITL;
- ArduPilot SITL;
- ROS 2 bridge, если появится реальная необходимость.

## Где проходит граница проекта

Проект отвечает за:

- coordination;
- membership;
- task ownership;
- failure detection;
- reallocation;
- partial connectivity;
- testing harness;
- metrics;
- replay.

Проект не отвечает за:

- stabilization;
- motor control;
- low-level trajectory tracking;
- реальную аэродинамику;
- производство железа;
- full GCS;
- photorealistic simulation.

## Критерий "это не песочница"

Проект становится серьезным, когда появляется следующее:

```text
1000+ deterministic scenario runs
fault injection
property-based tests
measured invariants
multiple strategies
clear runtime API
pluggable transport
multi-process execution
failure/recovery behavior
documented reference missions
metrics report
```

Если это есть, даже без физических дронов проект имеет инженерную ценность.

## Итоговая рекомендация

Взять `DRONE_B.md` как ядро направления:

> Swarm Coordination Runtime.

Взять `DRONE_A.md` как карту домена и набор сценариев:

> Mission Digital Twin / Scenario Harness.

Финальный фокус:

> Distributed coordination runtime for autonomous drone fleet/swarm missions, tested through reproducible mission-level simulation with unreliable communications and failures.

По-русски:

> Распределенный runtime координации для автономных миссий групп дронов, проверяемый через воспроизводимую симуляцию миссий с ненадежной связью и отказами.
