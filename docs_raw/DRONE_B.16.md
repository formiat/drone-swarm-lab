# DRONE_B.16 — План развития: фокус на Ветку 6 (Real SITL / PX4)

Дата фиксации: 2026-05-28

**Горизонт:** M43–M51. Последний завершённый milestone — M42.

---

## Текущее состояние SITL (честный аудит)

**Что работает:**
- `sitl_agent --mock` — полный mock path, отправляет waypoints через `MockMavlinkTransport`
- `MavlinkTransport::new()` — подключается к MAVLink endpoint
- `task_to_mavlink_waypoint()` — корректно формирует `MISSION_ITEM_INT`
- `mavlink_status_to_task_status()` — парсит `MISSION_ACK` / `HEARTBEAT`

**Что сломано / недоделано:**
- `MavlinkTransport::send()` отправляет `RAW_RPM` (заглушка) вместо настоящих waypoint-сообщений
- `MavlinkTransport::poll()` оборачивает MAVLink-пакеты в debug-строку — непригодно для парсинга
- `sitl_agent --connection` не реализует протокол загрузки миссии (MISSION_COUNT → handshake → MISSION_ITEM_INT)
- Нет arm/takeoff/execute последовательности
- Нет telemetry loop — агент завершается сразу после «отправки», не дожидаясь подтверждения

---

## M43 — MAVLink Mission Upload Protocol

**Суть:** реализовать правильный handshake загрузки миссии.

### Что делать

1. В `MavlinkTransport` добавить `upload_mission(waypoints: &[Waypoint])`:
   - Отправить `MISSION_COUNT`
   - Ждать `MISSION_REQUEST_INT` для каждого waypoint
   - Отправить `MISSION_ITEM_INT` по запросу
   - Ждать `MISSION_ACK` → success / fail
   - Timeout: если ответа нет N итераций → `MavlinkError`

2. Исправить `MavlinkTransport::send()` — убрать заглушку RAW_RPM. Метод `send()` для raw-сообщений не нужен для SITL; вся логика upload — через `upload_mission`.

3. В `sitl_agent --connection` заменить ручную отправку на `transport.upload_mission(waypoints)`.

### Done criteria

- `sitl_agent --connection udp:127.0.0.1:14550 --scenario ... --agent-id agent-0` загружает миссию в PX4 SITL и получает `MISSION_ACK: accepted`
- Mock path без изменений

### Тесты

#### Без рефакторинга

- `FakePx4` (тестовый тип в tests) имитирует handshake: принимает MISSION_COUNT, отвечает MISSION_REQUEST_INT, принимает MISSION_ITEM_INT, отвечает MISSION_ACK. Проверяем: upload_mission отправляет правильное число пакетов.
- Unit test: upload_mission при MISSION_ACK(rejected) возвращает ошибку.
- Unit test: upload_mission при timeout возвращает ошибку.

#### Лёгкий рефакторинг

- `FakePx4` как отдельный test helper в `swarm-comms/src/` под `#[cfg(test)]`.

#### Тяжёлый рефакторинг

- Integration test с реальным PX4 SITL (только ручной запуск, feature-gated).

---

## M44 — Flight Sequence (arm / takeoff / execute / abort)

**Суть:** реализовать полную последовательность управления полётом.

### Что делать

В `MavlinkTransport` добавить:
- `arm(target_system, target_component)` → `COMMAND_LONG(MAV_CMD_COMPONENT_ARM_DISARM, param1=1.0)`
- `takeoff(altitude_m)` → `COMMAND_LONG(MAV_CMD_NAV_TAKEOFF)`
- `set_auto_mode()` → `COMMAND_LONG(MAV_CMD_DO_SET_MODE, AUTO)`
- `abort()` → `MAV_CMD_NAV_RETURN_TO_LAUNCH` или `MISSION_CLEAR_ALL`
- `wait_command_ack(command, timeout)` → ждёт `COMMAND_ACK` с нужным command ID

В `sitl_agent --connection` реализовать полный цикл:
```
upload_mission → arm → takeoff → set_auto_mode → poll telemetry → abort if error
```

### Done criteria

- Один агент выполняет waypoints через PX4 SITL от начала до конца
- При ошибке на любом шаге — `abort()` + понятное сообщение

### Тесты

#### Без рефакторинга

- Unit test: `arm()` отправляет COMMAND_LONG с правильным command ID и param1=1.0.
- Unit test: `takeoff(50.0)` отправляет COMMAND_LONG с правильным altitude.
- Unit test: `abort()` отправляет MAV_CMD_NAV_RETURN_TO_LAUNCH.
- CLI test: `--connection` без feature — чёткая ошибка с инструкцией по сборке.

#### Лёгкий рефакторинг

- `FakePx4` из M43 расширяется: отвечает COMMAND_ACK на arm/takeoff/mode.

---

## M45 — Telemetry Loop & TaskStatus Mapping

**Суть:** мониторинг выполнения миссии по телеметрии.

### Что делать

В `MavlinkTransport` добавить `poll_telemetry() -> Option<TelemetryEvent>`:

```rust
pub enum TelemetryEvent {
    WaypointReached { seq: u16 },
    MissionComplete,
    Heartbeat { state: FlightState },
    Disconnected,
}
```

В `sitl_agent` — telemetry loop:
- `MISSION_CURRENT` → текущий waypoint index
- `MISSION_ITEM_REACHED` → waypoint completed → `TaskStatus::Completed`
- `HEARTBEAT` → обновление состояния (Armed, InAir, Landed)
- Если HEARTBEAT не приходит N тиков → `abort()` + выход

Добавить `TaskStatus::Failed` mapping для `MISSION_ACK(rejected)`.

### Done criteria

- `sitl_agent` не завершается сразу, а ждёт `MISSION_ITEM_REACHED` для каждого waypoint
- При успешном завершении всех waypoints → exit 0
- При disconnect → abort + exit 1

### Тесты

#### Без рефакторинга

- Unit test: `FakePx4` эмитирует MISSION_ITEM_REACHED → `poll_telemetry` возвращает `WaypointReached`.
- Unit test: отсутствие HEARTBEAT → `TelemetryEvent::Disconnected`.
- Unit test: последовательность MISSION_ITEM_REACHED для всех waypoints → `MissionComplete`.

---

## M46 — Pre-upload Safety Validation

**Суть:** валидация миссии до отправки в PX4.

### Что делать

В `sitl_agent` перед `upload_mission` добавить валидацию через `swarm-safety`:
- Geofence: все waypoints в пределах geofence
- No-fly zones: ни один waypoint не попадает в no-fly zone
- Separation: минимальное расстояние между последовательными waypoints ≥ threshold
- При нарушении → понятная ошибка с номером waypoint и типом нарушения, `abort()` не нужен (ещё не взлетели)

Добавить в `SITL_SETUP.md` секцию о safety constraints в scenario JSON.

### Done criteria

- Сценарий с waypoint в no-fly zone — отклоняется с сообщением до arm
- Сценарий за пределами geofence — отклоняется с сообщением
- Корректный сценарий проходит валидацию

### Тесты

#### Без рефакторинга

- Unit test: waypoint в no-fly zone → validation error с типом `NoFlyZoneViolation`.
- Unit test: waypoint за geofence → `GeofenceViolation`.
- Unit test: нормальный сценарий → validation pass.

---

## M47 — SITL Docs & Mock Regression Smoke

**Суть:** задокументировать golden path, добавить portable regression suite.

### Что делать

Обновить `docs/SITL_SETUP.md`, разделив на три чётких секции:
1. **Mock mode** (без внешних зависимостей) — команда, ожидаемый вывод
2. **PX4 SITL** (с ArduPilot или PX4-Autopilot) — установка, команда запуска, connection string
3. **Real hardware** — предупреждение об experimental статусе, отличия от SITL

Добавить regression suite для SITL mock:

```rust
RegressionSuite {
    name: "sitl_mock_waypoint_smoke",
    group: SuiteGroup::Smoke,
    // Uses MockMavlinkTransport — always portable
    ...
}
```

### Done criteria

- `docs/SITL_SETUP.md` не противоречит сам себе
- Mock regression smoke проходит в `cargo test`
- Разделены mock / SITL / hardware пути

---

## M48 — Dynamic Reallocation (из Ветки 1)

**Суть:** обязательная предпосылка для multi-agent SITL — реальные дроны падают.

### Что делать

В `swarm-runtime` реализовать `reallocate_failed_agent(agent_id)`:
- Возвращает задачи упавшего агента в пул `Unassigned` без полного сброса CBBA
- Перераспределяет только освободившиеся задачи (не всю миссию)
- Метрики в `RunMetrics`: `reassignment_count: u64`, `avg_reallocation_ticks: f64`

В `ScenarioRunner` вызывать `reallocate_failed_agent` при обнаружении отказа вместо текущего flow.

### Done criteria

- Детерминированный тест: агент падает на тике 10, его задачи переданы выжившим агентам к тику 15
- `reassignment_count` > 0 в метриках при наличии failure event
- Существующие тесты на failure detection не сломаны

### Тесты

#### Без рефакторинга

- Integration test: 3 агента, 1 падает → его задачи переданы 2 оставшимся.
- Unit test: задачи упавшего агента имеют `assigned_to = None` после reallocation.
- Метрика `reassignment_count` корректна.

#### Лёгкий рефакторинг

- Helper для создания сценариев с предсказуемым failure.

#### Тяжёлый рефакторинг

- Property test: при любом количестве failures все задачи в конечном счёте получают агента.

---

## M49 — Multi-agent SITL

**Суть:** несколько дронов, координация через runtime.

### Что делать

В `sitl_agent`:
- Поддержка нескольких `--agent-id` или запуск нескольких процессов
- Каждый агент подключается к своему SITL instance (`udp:127.0.0.1:14550`, `udp:127.0.0.1:14560`, ...)
- Coordinator из `swarm-runtime` распределяет waypoints между агентами
- При падении агента → `reallocate_failed_agent` (M48) → перераспределение задач

Добавить сценарий `scenarios/sitl.multi-agent.json` для 2–3 дронов.

### Done criteria

- 2 агента выполняют разделённый набор waypoints через PX4 SITL
- При mock-падении одного → задачи переходят к другому
- Mock path multi-agent работает без PX4

### Тесты

#### Без рефакторинга

- Mock multi-agent smoke: 2 mock-агента, coordinator распределяет waypoints.
- Failure mock: 1 из 2 агентов «падает», задачи переходят к выжившему.

#### Тяжёлый рефакторинг

- Real multi-agent SITL integration test (ручной).

---

## M50 — Realism Calibration (из Ветки 4)

**Суть:** понять, насколько симуляция предсказывает SITL-поведение.

**Зачем сейчас:** без этого результаты симуляции не дают оценки реального поведения SITL.

### Что делать

Для waypoint-миссий задокументировать expected effects для каждого профиля:
- **light:** pose noise 0.2m, wind 0.05m/tick — ожидаемое отклонение от ideal ≤ 5%
- **medium:** pose noise 0.5m, wind 0.1m/tick — ≤ 15%
- **heavy:** pose noise 1.0m, wind 0.2m/tick — ≤ 30%

Добавить comparative benchmark для waypoint-типа миссии (используя coverage как proxy):
```
ideal vs light vs medium vs heavy → таблица в docs/
```

Добавить regression smoke: `sitl_realism_smoke` — coverage под medium realism, threshold `success_rate ≥ 0.70`.

### Done criteria

- Expected effects задокументированы
- Comparative benchmark воспроизводим
- Regression smoke для realism проходит стабильно

---

## M51 — SITL Replay (из Ветки 5)

**Суть:** post-hoc отладка реальных полётов.

### Что делать

Добавить SITL-специфичные события в `swarm-replay`:

```rust
Event::SitlArmed       { agent_id, tick }
Event::SitlTakeoff     { agent_id, altitude_m, tick }
Event::SitlWaypointReached { agent_id, seq, tick }
Event::SitlMissionComplete { agent_id, tick }
Event::SitlAborted     { agent_id, reason, tick }
```

`sitl_agent` логирует события через `EventLogBuilder` → JSON на диск.

Replay CLI: `cargo run --bin replay -- --log results/sitl_run.json --summary` показывает:
- Timeline событий по агентам
- Какие waypoints пройдены, какие нет
- Причина abort если была

ASCII overlay для waypoint-миссий: позиции агентов + путь waypoints на сетке.

### Done criteria

- `--replay-log results/sitl/` создаёт event log для SITL run
- `replay --summary` работает без паники для SITL событий
- Replay JSON проходит roundtrip тест

---

## Сводная таблица

| Milestone | Ветка | Результат | Зависит от |
|---|---|---|---|
| M43 | 6 | MAVLink mission upload handshake | — |
| M44 | 6 | arm/takeoff/execute/abort | M43 |
| M45 | 6 | Telemetry loop + TaskStatus mapping | M44 |
| M46 | 6 | Pre-upload safety validation | M43 |
| M47 | 6 | SITL docs + mock regression | M43–M46 |
| M48 | 1 | Dynamic reallocation on failure | — |
| M49 | 6 | Multi-agent SITL | M47 + M48 |
| M50 | 4 | Realism calibration для SITL-типа | M47 |
| M51 | 5 | SITL replay + debug tooling | M47 |

**Рекомендуемый порядок работы:**
M43 → M44 → M45 → M46 → M47 (Phase 1 complete) → M48 параллельно с M50/M51 → M49.
