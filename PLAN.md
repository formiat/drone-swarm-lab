# PLAN — Stage 5: SITL / MAVLink (M17)

## Контекст

Этот план описывает реализацию **Milestone 17 (M17)** — подключение одного агента к PX4 SITL через MAVLink. Цель: валидировать, что координационные алгоритмы, разработанные в симуляторе, работают поверх реального autopilot стека.

**Текущее состояние кодовой базы:**
- M1–M16 реализованы. Workspace из 10 crate'ов (добавлен `swarm-safety`).
- `Transport` trait стабилен, есть две реализации: `InMemAgentTransport` и `UdpTransport`.
- `AgentNode<T: Transport>` — унифицированный runtime контур.
- Mission DSL v0.12: `ScenarioSuite`, `load_scenario_suite`, `--scenario-suite` флаг.
- Safety Layer (M13) завершён: `Geofence`, `NoFlyZone`, `SeparationConstraint`, `SafetyAllocator`.
- Infrastructure Inspection (M16) завершён: `InspectionEdge`, `InspectionGraph`, `InspectionState`, метрики покрытия рёбер.

**Orchestrator docs:** `docs/DRONE_B.8.md` содержит детализированные требования Stage 5.

## Investigation context

Файл `INVESTIGATION.md` отсутствует. Анализ кодовой базы проведён путём чтения ключевых модулей:

- `crates/swarm-comms/src/transport.rs` — `Transport` trait: `send` + `poll`, `RawMessage`.
- `crates/swarm-comms/src/udp.rs` — `UdpTransport`: reference реализация для `MavlinkTransport`.
- `crates/swarm-runtime/src/node.rs` — `AgentNode<T: Transport>`: generic над транспортом.
- `Cargo.toml` — workspace dependencies, resolver = "2".
- `crates/swarm-comms/Cargo.toml` — минимальные зависимости (swarm-types, rand, serde).
- `crates/swarm-examples/Cargo.toml` — структура binary-целей.
- `crates/swarm-sim/src/scenario.rs` — `Scenario` со списками `agents`, `tasks`.
- `crates/swarm-types/src/task.rs` — `Task` с `pose`, `required_role`, `edge_id`.

## Зависимость от `mavlink` crate

Библиотека `rust-mavlink` (crate `mavlink`) — единственный зрелый MAVLink-парсер на Rust.
**Важно:** на момент планирования `mavlink` не добавлен в зависимости. Потребуется:

```toml
mavlink = "0.12"
```

Версия может быть уточнена при реализации. Крейт предоставляет:
- `mavlink::connect(protocol)` — соединение по UDP/TCP/Serial.
- `mavlink::MavConnection::recv()` — приём сообщений.
- `mavlink::MavConnection::send(msg)` — отправка сообщений.
- Готовые типы MAVLink сообщений (HEARTBEAT, COMMAND_INT, MISSION_ITEM и т.д.).

## Affected components

| Компонент | Файлы | Изменения |
|-----------|-------|-----------|
| `swarm-comms` (новый модуль) | `src/mavlink.rs`, `Cargo.toml` | `MavlinkTransport`, `MockMavlinkTransport`, `MavlinkError` |
| `swarm-comms` | `src/lib.rs` | Экспорт нового модуля |
| `swarm-mavlink` (новый крейт) или `swarm-comms` | `Cargo.toml`, `src/lib.rs` | Решение: разместить в `swarm-comms` для единообразия |
| `swarm-types` или `swarm-comms` | `src/lib.rs` | `task_to_mavlink_waypoint`, `mavlink_status_to_task_status` |
| `swarm-examples` | `src/bin/sitl_agent.rs`, `Cargo.toml` | Новый binary `sitl_agent` |
| Workspace root | `Cargo.toml` | Добавить `mavlink` в workspace dependencies |
| Docs | `docs/SITL_SETUP.md` | Документация по установке и запуску PX4 SITL |
| Root | `README.md` | Добавить M17 раздел |

## Implementation steps

### 1. Подготовка зависимостей

**Файлы:** `Cargo.toml` (workspace root), `crates/swarm-comms/Cargo.toml`.

1.1. Добавить `mavlink = "0.12"` в `[workspace.dependencies]` в корневом `Cargo.toml`.

1.2. Добавить `mavlink = { workspace = true }` в зависимости `swarm-comms/Cargo.toml`.

### 2. `MavlinkTransport` и `MockMavlinkTransport`

**Файлы:** `crates/swarm-comms/src/mavlink.rs` (новый), `crates/swarm-comms/src/lib.rs`.

2.1. Создать `mavlink/mod.rs` (или плоский `mavlink.rs`):

```rust
use std::collections::VecDeque;
use swarm_types::AgentId;
use crate::{RawMessage, Transport};

#[derive(Debug, thiserror::Error)]
pub enum MavlinkError {
    #[error("mavlink connection error: {0}")]
    Connection(String),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub struct MavlinkTransport {
    conn: Box<dyn mavlink::MavConnection>,
    agent_id: AgentId,
    recv_buf: VecDeque<RawMessage>,
}

impl MavlinkTransport {
    pub fn new(connection_string: &str, agent_id: AgentId) -> Result<Self, MavlinkError> {
        let conn = mavlink::connect(connection_string)
            .map_err(|e| MavlinkError::Connection(e.to_string()))?;
        Ok(Self {
            conn,
            agent_id,
            recv_buf: VecDeque::new(),
        })
    }
}

impl Transport for MavlinkTransport {
    type Error = MavlinkError;

    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        // Сериализовать RawMessage в MAVLink-совместимый формат
        // и отправить через MavConnection
        todo!()
    }

    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
        // Прочитать MAVLink сообщения из conn, разобрать в RawMessage
        todo!()
    }
}
```

2.2. `MockMavlinkTransport` — фиктивная реализация для тестов без PX4:

```rust
pub struct MockMavlinkTransport {
    sent: Vec<RawMessage>,
    inbox: VecDeque<RawMessage>,
}

impl MockMavlinkTransport {
    pub fn new() -> Self { ... }
    pub fn sent_messages(&self) -> &[RawMessage] { ... }
    pub fn push_incoming(&mut self, msg: RawMessage) { ... }
}

impl Transport for MockMavlinkTransport {
    type Error = MavlinkError;
    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        self.sent.push(msg);
        Ok(())
    }
    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
        Ok(self.inbox.pop_front())
    }
}
```

2.3. Экспортировать из `swarm-comms/src/lib.rs`:
```rust
pub mod mavlink;
pub use mavlink::{MavlinkError, MavlinkTransport, MockMavlinkTransport};
```

### 3. Mapping: задача → MAVLink waypoint

**Файлы:** `crates/swarm-comms/src/mavlink.rs` (продолжение).

3.1. `task_to_mavlink_waypoint(task: &Task) -> mavlink::MavMessage`:
- Извлечь `pose` из задачи.
- Если `pose` отсутствует — вернуть ошибку или пропустить.
- Создать `MAV_CMD_NAV_WAYPOINT` с координатами `(pose.x, pose.y, 0)`.
- Установить `current=1` для первой точки, `auto_continue=1`.

3.2. `mavlink_status_to_task_status(msg: &mavlink::MavMessage) -> Option<TaskStatus>`:
- При `MISSION_ACK` / `MISSION_CURRENT` с флагом завершения → `Completed`.
- При `HEARTBEAT` с режимом полёта → `InProgress`.
- При ошибке → `Failed`.
- Если статус не определён → `None`.

### 4. Single-agent SITL runner

**Файлы:** `crates/swarm-examples/src/bin/sitl_agent.rs` (новый), `crates/swarm-examples/Cargo.toml`.

4.1. Новый binary `sitl_agent` с CLI-аргументами:
```bash
cargo run --bin sitl_agent -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/coverage.ideal.json \
  --agent-id agent-0
```

4.2. Логика работы:
- Загрузить `Scenario` через `load_scenario_suite` или прямо из JSON.
- Создать `MavlinkTransport` с указанным connection string и agent_id.
- Извлечь задачи агента из `scenario.tasks` (фильтр по `assigned_to == agent_id`).
- Для каждой задачи с `pose`:
  - Вызвать `task_to_mavlink_waypoint`.
  - Отправить waypoint через `transport.send(...)`.
- В цикле `poll` читать статус через `mavlink_status_to_task_status`.
- По достижении `Completed` для всех задач — завершить.

4.3. Режим `--mock` (без PX4):
```bash
cargo run --bin sitl_agent -- \
  --mock \
  --scenario scenarios/coverage.ideal.json \
  --agent-id agent-0
```
В этом режиме используется `MockMavlinkTransport`; агент логирует отправленные waypoints и завершается.

### 5. Multi-agent SITL (design только)

**Файлы:** `PLAN.md` (данный документ), `docs/MULTI_AGENT_SITL.md`.

5.1. После успешного single-agent:
- Каждый агент = отдельный процесс с `MavlinkTransport` (разный UDP порт).
- Coordinator работает через `InMemAgentTransport` (in-process) — но в multi-agent сценарии агенты независимы.
- **Архитектура:** каждый экземпляр `sitl_agent` запускает `AgentNode<MavlinkTransport>`, где `MavlinkTransport` отвечает за связь с SITL, а координация между агентами — через in-memory или UDP transport (`AgentNode` поддерживает только один `Transport`).

**Проблема:** `AgentNode` имеет один `Transport` для координации. Для multi-agent SITL потребуется разделение:
- Transport A: координация между агентами (in-memory или UDP).
- Transport B: связь с PX4 SITL (MAVLink).

**Предлагаемое решение:** `AgentNode` остаётся с одним транспортом. MAVLink-взаимодействие выносится в отдельный слой поверх `AgentNode`. При движении агента (pose update) — `AgentNode` через callback/observer отправляет waypoint в SITL. Детали — в отдельном документе после single-agent.

### 6. Документация SITL setup

**Файлы:** `docs/SITL_SETUP.md`.

6.1. Инструкция по установке:
- Установка PX4:
  ```bash
  git clone https://github.com/PX4/PX4-Autopilot.git
  cd PX4-Autopilot
  make px4_sitl gazebo
  ```
- Установка Gazebo (если не установлен).

6.2. Команды для запуска SITL:
```bash
# Терминал 1: PX4 SITL + Gazebo
cd PX4-Autopilot
make px4_sitl gazebo_plane

# Терминал 2: MAVSDK или MAVProxy для мониторинга
mavproxy.py --master udp:127.0.0.1:14550

# Терминал 3: sitl_agent
cargo run --bin sitl_agent -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/coverage.ideal.json \
  --agent-id agent-0
```

6.3. Ожидаемый вывод:
- `sitl_agent` логирует отправленные waypoints.
- В MAVProxy видно полученные MAV_CMD.
- PX4 выполняет полёт по waypoints (в симуляции Gazebo).

### 7. Актуализация README

**Файл:** `README.md`.

7.1. Добавить раздел **Milestone 17 — SITL / MAVLink** после M16:
- Описание: одновагентный SITL-раннер, `MavlinkTransport`, mapping задач в MAVLink waypoints.
- Команды запуска:
  ```bash
  cargo run --bin sitl_agent -- --mock --scenario scenarios/coverage.ideal.json --agent-id agent-0
  cargo run --bin sitl_agent -- --connection udp:127.0.0.1:14550 --scenario scenarios/coverage.ideal.json --agent-id agent-0
  ```
- Ссылка на `docs/SITL_SETUP.md` для полной инструкции.

7.2. Обновить список workspace crate'ов — добавить упоминание MAVLink в описание `swarm-comms`.

## Testing strategy

| Категория | Что тестируем | Где | Как запускать |
|-----------|---------------|-----|---------------|
| 1 (unit, no refactor) | `MockMavlinkTransport::send` + `poll` roundtrip | `swarm-comms` unit tests | `cargo test -p swarm-comms mock_mavlink` |
| 1 (unit, no refactor) | `task_to_mavlink_waypoint` — корректный `MAV_CMD_NAV_WAYPOINT` | `swarm-comms` unit tests | `cargo test -p swarm-comms task_to_waypoint` |
| 1 (unit, no refactor) | `task_to_mavlink_waypoint` — задача без pose возвращает None/error | `swarm-comms` unit tests | `cargo test -p swarm-comms task_no_pose` |
| 1 (unit, no refactor) | `mavlink_status_to_task_status` — маппинг `MISSION_ACK` → `Completed`, `HEARTBEAT` → `InProgress` | `swarm-comms` unit tests | `cargo test -p swarm-comms status_mapping` |
| 1 (unit, no refactor) | `sitl_agent` в `--mock` режиме — отправляет waypoint для каждой задачи с pose | `swarm-examples` integration | `cargo test --bin sitl_agent mock_run` (или отдельный интеграционный тест) |
| 2 (integration, light) | `MavlinkTransport::new` с некорректным connection string возвращает ошибку | `swarm-comms` unit test | `cargo test -p swarm-comms mavlink_connect_error` |
| 3 (manual, PX4 SITL) | Single-agent SITL по coverage.ideal.json — PX4 выполняет полёт по waypoints | manual | По инструкции `docs/SITL_SETUP.md` |
| 3 (manual, PX4 SITL) | Multi-agent SITL (N агентов) — каждый летит по своим waypoints | manual | После завершения single-agent |

**Gaps (явно зафиксированные):**
- Multi-agent SITL не автоматизируется в этом milestone — требуется архитектурное решение для двух транспортов на один `AgentNode`.
- PX4 SITL integration тест — только manual (требует PX4 + Gazebo окружения, ~10 ГБ зависимостей).
- `MavlinkTransport` с реальным PX4 SITL требует UDP loopback — в CI без сетевых интерфейсов может не работать. Mock-режим должен быть основным для CI.

## Risks and tradeoffs

### Зависимость от `mavlink` crate (версия 0.12)
Крейт `rust-mavlink` версии 0.12 — наиболее стабильная версия на момент планирования. Если API изменился в более новых версиях, потребуется адаптация. **Решение:** зафиксировать `mavlink = "0.12"` в workspace dependencies. При реализации проверить актуальную версию на crates.io.

### Размещение MAVLink кода: новый крейт vs модуль в swarm-comms
- **Новый крейт `swarm-mavlink`:** чище с точки зрения изоляции зависимостей (не тянет `mavlink` в `swarm-comms` для всех потребителей). Минус: новый крейт в workspace, ещё один `Cargo.toml`.
- **Модуль в `swarm-comms`:** проще, не требует нового крейта, `mavlink` dependency добавляется только в `swarm-comms`. Минус: `swarm-comms` тянет MAVLink зависимости даже для проектов, не использующих SITL.
- **Решение:** разместить в `swarm-comms/src/mavlink.rs` для единообразия. Если в будущем появятся другие MAVLink-специфичные компоненты (mission plan upload, telemetry), можно выделить в отдельный крейт.

### `Transport` trait и MAVLink
`Transport` trait спроектирован для координационных сообщений между агентами (`RawMessage`). MAVLink — протокол связи с автопилотом, а не между агентами. `MavlinkTransport` будет обёрткой: MAVLink сообщения → `RawMessage` (сериализация) и обратно. Это работает, но добавляет overhead сериализации MAVLink → JSON → RawMessage.

### Multi-agent SITL — архитектурный вопрос
`AgentNode<T: Transport>` имеет один generic параметр транспорта. Для multi-agent SITL агенту нужно два транспорта: один для координации с другими агентами (in-memory/UDP), второй для связи с PX4 (MAVLink). Это требует либо:
- Двух `AgentNode` на один агент (сложно).
- Callback/observer слоя для MAVLink.
- Отдельного MAVLink bridge процесса.

Решение откладывается до multi-agent SITL. В single-agent задачи решаются через один `MavlinkTransport`.

### Совместимость MAVLink сообщений
`MAV_CMD_NAV_WAYPOINT` ожидает координаты в широте/долготе (WGS84). В нашей симуляции координаты — декартовы (x, y). Для SITL потребуется преобразование координат: `(x, y) → (lat, lon)` через локальную UTM проекцию. **Варианты:**
1. Использовать фиктивные GPS координаты (PX4 SITL в `--home`).
2. Добавить преобразование через `proj` crate.
3. Для first pass — использовать симуляцию в локальных координатах.

Начальный подход: задавать home координаты PX4 SITL через `PX4_HOME_LAT`/`PX4_HOME_LON` и смещать относительно них (`x*1e-5` градусов на метр).

## Что могло сломаться

| Риск | Компонент | Проверка |
|------|-----------|----------|
| **`mavlink` crate несовместимость** — версия 0.12 может иметь отличия в API | `swarm-comms` | При реализации: `cargo check -p swarm-comms`. Если не компилируется — обновить версию или адаптировать API |
| **`MavlinkTransport::send` блокируется** — MAVLink UDP send может блокироваться при недоступном адресате | `swarm-comms` | Использовать non-blocking socket; `MockMavlinkTransport` в unit-тестах |
| **Преобразование координат (декартовы → GPS)** — неправильное масштабирование приведёт к улёту агента в симуляции | `swarm-examples` | В `--mock` режиме проверять, что waypoint координаты корректно сериализуются; при SITL тесте — визуально в Gazebo |
| **Сериализация RawMessage через serde_json** — MAVLink сообщения уже бинарные, двойная сериализация добавляет overhead и может сломать бинарные протоколы | `swarm-comms` | Unit-тест: отправить `RawMessage` через `MockMavlinkTransport`, проверить что `sent_messages()` содержит то же сообщение |
| **AgentNode с MavlinkTransport** — `tick()` оживает `Transport::poll()` и `Transport::send()` для координации; MAVLink не предназначен для координации агентов | `swarm-runtime` | Single-agent SITL использует `MavlinkTransport` только для связи с PX4. Координации между агентами в single-agent нет. Для multi-agent — отдельный design |
| **Регрессия существующих transport'ов** — изменения в `swarm-comms` модулях не должны сломать `InMemAgentTransport` и `UdpTransport` | `swarm-comms` | `cargo test -p swarm-comms` — все существующие тесты должны проходить |

## Open questions

1. **Координаты:** как преобразовывать декартовы (x, y) из Scenario в GPS (lat, lon) для PX4 SITL? Варианты: (a) UTM через `proj`, (b) фиксированный `home` + offset в градусах. Для первого прототипа — (b).

2. **Синхронизация времени:** SITL работает в реальном времени, наш симулятор — в тактовом. Как синхронизировать? Первый подход: один MAVLink waypoint = одна задача; агент ждёт `MissionCurrent` или `MissionAck` перед следующей.

3. **Обработка ошибок MAVLink:** что делать при потере соединения? `MavlinkTransport` должен возвращать ошибку; `sitl_agent` — логировать и retry или завершаться. Первый подход: завершаться с кодом ошибки.

4. **Multi-agent SITL:** как быть с двумя транспортами? Возможные решения: (a) `AgentNode<CoordTransport, MavlinkTransport>`, (b) MAVLink observer на `AgentNode`, (c) отдельный процесс-мост. Решение откладывается до multi-agent фазы.

5. **Выбор крейта `mavlink`:** проверить на crates.io актуальную стабильную версию на момент реализации. Если `mavlink 0.12` не поддерживает UDP client mode — использовать `mavlink-core` или собрать вручную.
