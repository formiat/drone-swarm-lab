# DRONE_B.17 — Итоговый план развития: фокус на Ветку 6 (Real SITL / PX4)

Дата фиксации: 2026-05-28

**Горизонт:** M43–M51. Последний завершённый milestone — M42.

Основа: структура `DRONE_A.16.md`, исправленная нумерация milestone, детали
реализации из `DRONE_B.16.md`.

---

## Контекст

Выбранный фокус: **Ветка 6 — Real SITL / PX4**.

Цель направления: сделать проект не только headless simulator / benchmark harness,
а системой, которая может взять DSL-сценарий с waypoint tasks, загрузить mission в
PX4 SITL, наблюдать прогресс и связать telemetry обратно с `TaskStatus`.

Важно: этот план не означает немедленный переход к реальным дронам. Правильная
траектория — сначала воспроизводимый single-agent PX4 SITL workflow, затем safety
and observability, затем multi-agent SITL, и только потом явная граница hardware
readiness.

---

## Текущий статус SITL

**Что работает:**
- `sitl_agent --mock` — полный mock path, отправляет waypoints через `MockMavlinkTransport`
- `MavlinkTransport::new()` — подключается к MAVLink endpoint
- `task_to_mavlink_waypoint()` — корректно формирует `MISSION_ITEM_INT`
- `mavlink_status_to_task_status()` — парсит `MISSION_ACK` / `HEARTBEAT`
- DSL validation для `sitl` tasks with `pose`
- `docs/SITL_SETUP.md` — базовая документация

**Что сломано / недоделано:**
- `MavlinkTransport::send()` отправляет `RAW_RPM` (заглушка) вместо настоящих сообщений
- `MavlinkTransport::poll()` оборачивает MAVLink-пакеты в debug-строку — непригодно для парсинга
- `sitl_agent --connection` не реализует PX4 mission upload protocol
- Нет arm/takeoff/start/abort lifecycle
- Нет telemetry loop и TaskStatus mapping
- Нет строгого preflight safety gate
- Нет SITL event log / replay summary
- Multi-agent SITL отсутствует

---

## Принцип плана

Акцент остаётся на Ветке 6. Подзадачи из других веток добавляются только там, где
они прямо помогают real SITL / PX4 workflow:

- из Ветки 1 — только failure handling and reallocation, только для multi-agent;
- из Ветки 4 — только safety/preflight constraints, без full realism calibration;
- из Ветки 5 — SITL observability and replay summary;
- из Ветки 7 — минимальный transport/agent extension contract;
- из Ветки 3 — не брать research benchmark;
- из Веток 2 и 8 — ничего.

---

## Линейный план

```
M43  SITL Contract & Dry-Run Foundation
  -> M44  MAVLink Mission Upload Protocol
  -> M45  PX4 Telemetry to TaskStatus
  -> M46  Single-Agent PX4 SITL Golden Path
  -> M47  Safety Preflight Gate
  -> M48  SITL Observability & Replay
  -> M49  Multi-Agent SITL Foundation
  -> M50  Failure Handling & Reallocation
  -> M51  Hardware Readiness Boundary
```

---

## M43 — SITL Contract & Dry-Run Foundation

**Цель:** зафиксировать portable SITL contract до подключения настоящего PX4 workflow.

**Суть:** разделить mock, dry-run и real connection modes так, чтобы разработка real
MAVLink path не ломала CI-friendly mock path. `--dry-run` должен стать способом
проверять весь scenario → waypoint/mission-plan pipeline без внешнего PX4.

### Что сделать

1. Разделить режимы `sitl_agent`:
   - `--mock`;
   - `--dry-run`;
   - `--connection <addr>`.
2. Добавить dry-run output:
   - agent id;
   - scenario path/name;
   - task ids;
   - waypoint sequence;
   - координаты;
   - frame/altitude interpretation.
3. Вынести waypoint extraction/conversion из CLI в тестируемый helper.
4. Зафиксировать coordinate-frame contract:
   - что означает `Pose { x, y, z }`;
   - как это преобразуется в local/global MAVLink coordinates;
   - какие ограничения пока существуют.
5. Добавить typed errors:
   - invalid scenario;
   - no pose tasks;
   - feature missing;
   - bad connection string;
   - unsupported coordinate frame.
6. Обновить `docs/SITL_SETUP.md`:
   - mock mode;
   - dry-run mode;
   - real PX4 mode;
   - known limitations.

### Done criteria

- `sitl_agent --dry-run --scenario scenarios/sitl.waypoints.json` показывает
  mission upload plan без подключения к PX4
- `--connection` без feature даёт стабильную понятную ошибку с инструкцией по сборке
- Mock path остаётся полностью portable

### Тесты

#### Без рефакторинга

- Waypoint extraction helper tests.
- Dry-run formatting tests.
- CLI validation test: missing mode → typed error.
- CLI validation test: `--connection` без `mavlink-transport` → typed error.
- Scenario с zero pose tasks возвращает typed error.

#### Лёгкий рефакторинг

- Shared SITL scenario fixture.
- Helper для проверки CLI output.
- Reusable typed error assertions.

#### Тяжёлый рефакторинг

- Нет для этого milestone.

---

## M44 — MAVLink Mission Upload Protocol

**Цель:** заменить текущий debug/raw-message real path на настоящий PX4 mission
upload protocol.

**Суть:** сейчас `MavlinkTransport::send()` отправляет `RAW_RPM` — заглушка.
Нужно реализовать state machine, которая разговаривает с PX4 через стандартный
mission upload flow.

### Что сделать

1. Реализовать mission upload state machine в `MavlinkTransport::upload_mission(waypoints)`:
   - wait heartbeat;
   - `MISSION_CLEAR_ALL`;
   - `MISSION_COUNT`;
   - обработка `MISSION_REQUEST_INT`;
   - fallback на `MISSION_REQUEST` если нужен;
   - отправка `MISSION_ITEM_INT`;
   - обработка `MISSION_ACK`.
2. Добавить timeout/retry policy:
   - retry count;
   - per-step timeout;
   - clear error on timeout;
   - abort on wrong sequence.
3. Добавить typed `MavlinkMissionError`:
   - connection failed;
   - heartbeat timeout;
   - mission request timeout;
   - unexpected request seq;
   - mission rejected;
   - unsupported coordinate frame.
4. Сделать fake MAVLink connection для unit tests без PX4.
5. Убрать заглушку `RAW_RPM` из `MavlinkTransport::send()`.
6. В `sitl_agent --connection` заменить ручную отправку на `transport.upload_mission(waypoints)`.
7. Сохранить mock path без внешних зависимостей.

### Done criteria

- `sitl_agent --connection` реально вызывает mission upload protocol
- Happy path upload покрыт unit tests через fake connection
- Failure paths покрыты typed errors

### Тесты

#### Без рефакторинга

- Mission upload happy path с fake connection.
- `MISSION_REQUEST_INT` seq order test.
- Wrong seq rejection test.
- Rejected `MISSION_ACK` test.
- Timeout test.
- `task_to_mavlink_waypoint` conversion test.

#### Лёгкий рефакторинг

- `MavlinkConnection` trait или аналогичный test seam.
- Fake connection script fixtures.
- Typed error fixture helpers.

#### Тяжёлый рефакторинг

- Real PX4 SITL integration test (ручной, feature-gated).

---

## M45 — PX4 Telemetry to TaskStatus

**Цель:** связать PX4 telemetry/progress с внутренним task lifecycle.

**Суть:** после mission upload нужно понимать, что происходит. Нужно получать
telemetry, понимать текущий waypoint/mission seq и обновлять `TaskStatus` для
исходных tasks.

### Что сделать

1. Добавить `poll_telemetry() -> Option<TelemetryEvent>` в `MavlinkTransport`:

```rust
pub enum TelemetryEvent {
    WaypointReached { seq: u16 },
    MissionComplete,
    Heartbeat { state: FlightState },
    Disconnected,
}
```

2. Обработать основные MAVLink messages:
   - `HEARTBEAT`;
   - `MISSION_CURRENT`;
   - `MISSION_ITEM_REACHED`;
   - `MISSION_ACK`;
   - disconnect/timeout.
3. Добавить mapping: mission item seq → task id → status.
4. Ввести progress loop:
   - current seq;
   - completed waypoint count;
   - last telemetry timestamp;
   - timeout on no progress.
5. Добавить task status transitions: `Unassigned` → `InProgress` → `Completed` / `Failed`.
6. Добавить human-readable progress output.

### Done criteria

- Fake telemetry seq 0/1/2 превращается в completed task statuses
- CLI показывает progress
- Mission failure превращается в failed status
- `sitl_agent` не завершается сразу, а ждёт `MISSION_ITEM_REACHED` для каждого waypoint

### Тесты

#### Без рефакторинга

- `MISSION_CURRENT` → current task test.
- Waypoint reached → completed task test.
- Mission rejected → failed task/run test.
- Disconnect timeout test.

#### Лёгкий рефакторинг

- Telemetry parser helper.
- Fake telemetry stream.
- Task-status assertion helpers.

#### Тяжёлый рефакторинг

- Real PX4 telemetry integration test (ручной).

---

## M46 — Single-Agent PX4 SITL Golden Path

**Цель:** получить первый настоящий end-to-end PX4 SITL workflow для одного агента.

**Суть:** это первый milestone, где проект перестаёт быть только headless simulation.
Scope намеренно узкий: один агент, waypoint scenario, PX4 SITL, upload and execute.

### Что сделать

1. Реализовать CLI lifecycle options:
   - `--upload-only`;
   - `--execute`;
   - `--no-arm`;
   - `--abort-after <seconds>`;
   - `--timeout <seconds>`.
2. Добавить полный flight sequence в `MavlinkTransport`:
   - `arm(target_system, target_component)` → `COMMAND_LONG(MAV_CMD_COMPONENT_ARM_DISARM)`;
   - `takeoff(altitude_m)` → `COMMAND_LONG(MAV_CMD_NAV_TAKEOFF)`;
   - `set_auto_mode()` → `COMMAND_LONG(MAV_CMD_DO_SET_MODE, AUTO)`;
   - `abort()` → `MAV_CMD_NAV_RETURN_TO_LAUNCH`;
   - `wait_command_ack(command, timeout)`.
3. Реализовать полный lifecycle в `sitl_agent`:
   ```
   connect → wait heartbeat → upload mission →
   arm → takeoff → set_auto_mode → poll telemetry → finish/abort
   ```
4. Добавить abort behavior:
   - user interrupt (Ctrl-C);
   - timeout;
   - failed ack;
   - telemetry loss.
5. Документировать tested PX4 setup в `docs/SITL_SETUP.md`:
   - PX4 version/command;
   - simulator backend;
   - connection string;
   - expected ports;
   - troubleshooting.

### Done criteria

- Один агент проходит `scenarios/sitl.waypoints.json` в PX4 SITL от начала до конца
- Mock/dry-run остаются portable
- Docs чётко разделяют mock, dry-run, PX4 SITL, real hardware

### Тесты

#### Без рефакторинга

- CLI option parsing tests.
- Lifecycle command construction tests.
- Abort condition tests с fake connection.
- Unit test: `arm()` отправляет COMMAND_LONG с правильным command ID.
- Unit test: `takeoff(50.0)` с правильным altitude.

#### Лёгкий рефакторинг

- Dry-run lifecycle plan fixture.
- Fake PX4 script для golden path.

#### Тяжёлый рефакторинг

- Real PX4 SITL integration test (ручной).

---

## M47 — Safety Preflight Gate

**Цель:** не отправлять потенциально опасную или некорректную mission в transport.

**Суть:** даже в SITL real connection path должен иметь explicit preflight validation.
Это особенно важно перед любыми будущими hardware experiments.

### Что сделать

1. Ввести `SitlSafetyConfig`:
   - geofence bounds;
   - min/max altitude;
   - max distance between waypoints;
   - max mission radius from home;
   - no-fly zones;
   - required home/base point.
2. Валидировать перед upload:
   - empty mission;
   - duplicate waypoint ids;
   - missing pose;
   - invalid altitude;
   - outside geofence;
   - inside no-fly zone;
   - unsafe waypoint jump.
3. Ошибки должны содержать:
   - rule id;
   - task id / waypoint seq;
   - actual value;
   - allowed value/range.
4. Добавить `--safety-config <path>`.
5. Реализовать safe defaults для SITL.
6. Нет silent override — если нужен override, он должен быть explicit, но лучше отложить.
7. Переиспользовать `swarm-safety` где возможно; расширить для SITL-специфичных правил.

### Done criteria

- Невалидная mission не уходит в MAVLink transport
- Ошибки actionable: rule id + waypoint seq + actual/allowed value
- Safety config portable and testable

### Тесты

#### Без рефакторинга

- Geofence rejection test.
- Altitude bounds test.
- No-fly zone test.
- Max waypoint jump test.
- Duplicate waypoint id test.
- Valid mission passes test.

#### Лёгкий рефакторинг

- Safety config fixture builder.
- Scenario mutation helpers.

#### Тяжёлый рефакторинг

- Нет для этого milestone.

---

## M48 — SITL Observability & Replay

**Цель:** сделать SITL behavior inspectable and reproducible after a run.

**Суть:** PX4 workflow без event log трудно отлаживать. Нужно писать компактный
SITL run log и уметь получать summary через replay tooling.

### Что сделать

1. Добавить SITL event types в `swarm-replay`:

```rust
Event::SitlConnectionOpened { agent_id, connection_string, tick }
Event::SitlHeartbeatReceived { agent_id, tick }
Event::SitlMissionClearSent  { agent_id, tick }
Event::SitlMissionCountSent  { agent_id, count, tick }
Event::SitlMissionItemSent   { agent_id, seq, tick }
Event::SitlMissionAckReceived { agent_id, accepted, tick }
Event::SitlArmed             { agent_id, tick }
Event::SitlTakeoff           { agent_id, altitude_m, tick }
Event::SitlWaypointReached   { agent_id, seq, tick }
Event::SitlMissionComplete   { agent_id, tick }
Event::SitlAborted           { agent_id, reason, tick }
```

2. `sitl_agent` логирует события через `EventLogBuilder` → JSON на диск (`--replay-log <dir>`).

3. Добавить machine-readable SITL run report:
   - scenario;
   - agent id;
   - connection mode;
   - mission item count;
   - completed item count;
   - final status;
   - duration;
   - error if any.

4. Расширить replay CLI:
   - `cargo run --bin replay -- --log <path> --summary` работает для SITL events без паники;
   - compact text summary: timeline событий, какие waypoints пройдены, причина abort.

5. Документировать log schema.

### Done criteria

- `--replay-log results/sitl/` создаёт event log для SITL run
- `replay --summary` работает для SITL событий
- Replay JSON проходит roundtrip тест

### Тесты

#### Без рефакторинга

- Event log serialization roundtrip.
- Summary counts mission upload events.
- Failure event summary test.
- Mock run writes expected events.

#### Лёгкий рефакторинг

- Event log builder fixture.
- Replay fixture by event type.

#### Тяжёлый рефакторинг

- Interactive visualization tests, если когда-нибудь появится UI.

---

## M49 — Multi-Agent SITL Foundation

**Цель:** перейти от single-agent SITL к нескольким агентам без усложнения алгоритмов.

**Суть:** сначала нужен foundation: mapping agents to connections, task subset split,
multi-agent dry-run и проверка отсутствия ownership conflicts. Не нужно сразу делать
сложную swarm coordination на PX4.

### Что сделать

1. Описать mapping:
   - `agent_id` → MAVLink system id;
   - `agent_id` → component id;
   - `agent_id` → connection string;
   - `agent_id` → assigned task subset.
2. Поддержать config (JSON):
   - per-agent connection string;
   - per-agent start delay;
   - per-agent upload-only/execute flags.
3. Добавить multi-agent dry-run:
   - какие tasks кому уходят;
   - какие connection strings используются;
   - ownership summary.
4. Поддержать два режима запуска:
   - несколько `sitl_agent` процессов;
   - один supervisor process.
5. Проверять no duplicate task ownership before upload.
6. Добавить сценарий `scenarios/sitl.multi-agent.json` для 2–3 дронов.

### Done criteria

- Два mock/SITL агента получают разные waypoint subsets
- Есть multi-agent dry-run manifest
- Duplicate ownership rejected before upload
- Mock multi-agent path работает без PX4

### Тесты

#### Без рефакторинга

- Agent connection config parse test.
- Task split test.
- Duplicate ownership rejection test.
- Multi-agent dry-run output test.
- Mock multi-agent smoke: 2 агента, coordinator распределяет waypoints.

#### Лёгкий рефакторинг

- Agent config fixture.
- Supervisor fake transport.

#### Тяжёлый рефакторинг

- Real multi-agent PX4 SITL integration test (ручной).

---

## M50 — Failure Handling & Reallocation

**Цель:** добавить минимальный failure/reallocation behavior, нужный для
multi-agent SITL.

**Суть:** точечное заимствование из Ветки 1. Не нужно брать весь Algorithm Depth.
Нужно только обработать потерю агента и вернуть его незавершённые tasks в pool.

### Что сделать

1. В `swarm-runtime` реализовать `reallocate_failed_agent(agent_id)`:
   - возвращает задачи упавшего агента в пул `Unassigned` без полного сброса CBBA;
   - перераспределяет только освободившиеся задачи.
2. Heartbeat timeout → agent lost → реализовать в SITL supervisor.
3. Добавить метрики в `RunMetrics`:
   - `reassignment_count: u64`;
   - `avg_reallocation_ticks: f64`.
4. Отразить reallocation в event log (событие `AgentReallocated`).

Связь с Веткой 1: берётся только dynamic reallocation при отказе агента. Не
включаем hierarchical coordination, communication-aware scoring и broad algorithm
work.

### Done criteria

- Детерминированный тест: агент теряется, его задачи получает другой агент
- Event log показывает failure and reallocation
- Task ownership remains unique
- Метрика `reassignment_count` > 0 при наличии failure event

### Тесты

#### Без рефакторинга

- Lost agent returns tasks to pool.
- Reallocation assigns tasks to surviving agent.
- Duplicate assignment invariant after reallocation.
- Event log contains reallocation event.

#### Лёгкий рефакторинг

- Fake heartbeat stream.
- Deterministic failure scenario fixture.

#### Тяжёлый рефакторинг

- Multi-agent SITL failure integration test (ручной).
- Property test: при любом количестве failures все задачи в конечном счёте получают агента.

---

## M51 — Hardware Readiness Boundary

**Цель:** явно отделить tested SITL workflow от real hardware claims.

**Суть:** даже после PX4 SITL нельзя утверждать, что проект готов к реальным
дронам. Нужно оформить границу готовности, assumptions и checklist.

### Что сделать

1. Добавить `docs/HARDWARE_READINESS.md`:
   - что проверено в mock;
   - что проверено в dry-run;
   - что проверено в PX4 SITL;
   - что не проверено на hardware;
   - safety assumptions;
   - operator checklist.
2. Добавить explicit CLI warning для hardware-looking connections.
3. Разделить connection classes в коде:
   - mock;
   - dry-run;
   - local PX4 SITL UDP;
   - remote/serial hardware candidate.
4. Не обещать production safety.
5. Зафиксировать требования перед любым hardware experiment:
   - physical kill switch;
   - geofence;
   - manual pilot override;
   - low-risk environment;
   - no autonomous flight outside controlled test.

### Done criteria

- Понятно, где заканчивается research/SITL workflow
- Real hardware path не выглядит случайно включаемым
- Есть operator checklist before hardware experiments

### Тесты

#### Без рефакторинга

- Connection classifier tests.
- Hardware warning output test.

#### Лёгкий рефакторинг

- CLI warning helper.

#### Тяжёлый рефакторинг

- Hardware-in-the-loop tests. Не делать в обычном CI.

---

## Что не делать сейчас

- Full Research Benchmark из Ветки 3.
- Interactive UI из Ветки 5.
- Full Realism Calibration из Ветки 4.
- Flood / New Mission из Веток 2/8.
- Broad Algorithm Depth из Ветки 1.
- Public API stabilization из Ветки 7.

Причина: всё это отвлекает от главной ценности выбранной Ветки 6 — получить
реальный, воспроизводимый PX4 SITL path.

---

## Сводная таблица

| Milestone | Ветка | Результат | Зависит от |
|---|---|---|---|
| M43 | 6 | Dry-run contract, typed errors, CLI режимы | — |
| M44 | 6 | MAVLink mission upload state machine | M43 |
| M45 | 6 | Telemetry loop + TaskStatus mapping | M44 |
| M46 | 6 | Single-agent PX4 SITL golden path | M45 |
| M47 | 4/6 | Safety preflight gate | M43 |
| M48 | 5/6 | SITL observability & replay | M46 |
| M49 | 6 | Multi-agent SITL foundation | M46 |
| M50 | 1/6 | Failure handling & reallocation | M49 |
| M51 | 6 | Hardware readiness boundary | M46 |

**Начинать с M43.** Это небольшой, безопасный этап, который быстро даёт пользу:
dry-run без PX4, понятный contract, testable helpers, стабильные ошибки, подготовка
к настоящему MAVLink upload в M44.
