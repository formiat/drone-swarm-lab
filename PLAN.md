# PLAN: Milestone 3 — Pluggable Transport + Multiprocess (v0.3)

## Context

Milestone 1 (v0.1) реализовал детерминированную in-process симуляцию: heartbeat, membership,
failure detection, task reallocation.

Milestone 2 (v0.2) добавил динамические задачи, capability matching, auction allocator, pluggable
`Allocator` trait.

Milestone 3 (v0.3) выводит runtime из "игрушечного одного процесса": вводит pluggable Transport
с двумя реализациями (in-memory уже есть, UDP — новая), запускает агентов как отдельные
OS-процессы, добавляет сериализацию, probing process crash, basic observability через tracing.

**Критерий готовности:**
1. Один и тот же runtime работает in-process (через `InMemNetwork`).
2. Тот же runtime работает как N OS-процессов через UDP loopback.
3. `kill -9` одного процесса → остальные обнаруживают отказ → перераспределяют задачи.

**Источники контекста:** `DRONE_A.1.md` (roadmap v0.3), `DRONE_B.1.md` (архитектура).
INVESTIGATION.md в workspace отсутствует.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-comms/src/transport.rs` | Добавить serde к `RawMessage` |
| `crates/swarm-comms/src/udp.rs` | Новый файл — `UdpTransport` |
| `crates/swarm-comms/src/lib.rs` | Экспортировать UDP модуль и ошибки |
| `crates/swarm-comms/Cargo.toml` | Добавить `serde_json`, `tracing` |
| `crates/swarm-runtime/src/coordinator.rs` | Добавить tracing spans |
| `crates/swarm-runtime/src/membership.rs` | Добавить tracing spans |
| `crates/swarm-runtime/src/failure.rs` | Добавить tracing spans |
| `crates/swarm-runtime/src/task_registry.rs` | Добавить tracing spans |
| `crates/swarm-runtime/Cargo.toml` | Добавить `tracing` |
| `crates/swarm-alloc/Cargo.toml` | Добавить `tracing` |
| `crates/swarm-alloc/src/...` | Добавить tracing spans в allocate() |
| `crates/swarm-examples/src/bin/agent_process.rs` | Новый файл — single-agent OS-process |
| `crates/swarm-examples/src/bin/multiprocess_scenario.rs` | Новый файл — launcher + crash test |
| `crates/swarm-examples/Cargo.toml` | Добавить `tracing`, `tracing-subscriber`, `serde_json`, `swarm-comms`, `swarm-runtime`, `swarm-alloc` |
| `Cargo.toml` (workspace) | Добавить `tracing`, `tracing-subscriber` |
| `README.md` | Обновить статус до Milestone 3, описать новые команды |

---

## Implementation Steps

### Шаг 1 — Добавить зависимости в workspace `Cargo.toml`

Файл: `Cargo.toml`

Добавить в `[workspace.dependencies]`:
```toml
tracing             = "0.1"
tracing-subscriber  = { version = "0.3", features = ["env-filter"] }
```

`serde_json` уже в workspace. `serde` уже в workspace.

---

### Шаг 2 — Добавить serde к `RawMessage`

Файл: `crates/swarm-comms/src/transport.rs`

`RawMessage` сейчас не имеет `Serialize`/`Deserialize`. Добавить:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawMessage { ... }
```

Это необходимо для UDP-фреймирования: весь `RawMessage` сериализуется в байты и отправляется
через `UdpSocket::send_to()`.

Добавить в `crates/swarm-comms/Cargo.toml`:
```toml
serde      = { workspace = true }
serde_json = { workspace = true }
tracing    = { workspace = true }
```

---

### Шаг 3 — Реализовать `UdpTransport`

Новый файл: `crates/swarm-comms/src/udp.rs`

**Структура:**
```rust
pub struct UdpTransport {
    socket: std::net::UdpSocket,
    /// key: `agent_id`
    peers: HashMap<AgentId, SocketAddr>,
    recv_buf: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum UdpTransportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("unknown peer: {0}")]
    UnknownPeer(AgentId),
}
```

`UdpTransport::new(bind_addr: SocketAddr, peers: HashMap<AgentId, SocketAddr>)`:
- Создаёт `UdpSocket`, bind на `bind_addr`
- Устанавливает `set_nonblocking(true)`
- Устанавливает `recv_buf` размером 65535 байт

`impl Transport for UdpTransport`:
- `send(&mut self, msg: RawMessage)` → `serde_json::to_vec(&msg)` → `socket.send_to(bytes, addr)`
- `poll(&mut self)` → `socket.recv_from(&mut recv_buf)`:
  - `Ok((n, _))` → `serde_json::from_slice(&recv_buf[..n])` → `Ok(Some(msg))`
  - `Err(e) if e.kind() == WouldBlock` → `Ok(None)` (нет сообщений)
  - Другие ошибки → `Err(UdpTransportError::Io(e))`

Обновить `crates/swarm-comms/src/lib.rs`:
```rust
pub mod udp;
pub use udp::{UdpTransport, UdpTransportError};
```

---

### Шаг 4 — Добавить tracing в `swarm-runtime`

Файл: `crates/swarm-runtime/Cargo.toml`

Добавить:
```toml
tracing = { workspace = true }
```

Файл: `crates/swarm-runtime/src/membership.rs`

В `record_heartbeat()` и `mark_dead()`:
```rust
tracing::debug!(agent_id = %id, "heartbeat recorded");
tracing::warn!(agent_id = %id, "agent marked dead");
```

Файл: `crates/swarm-runtime/src/failure.rs`

В `detect()`, когда агент превысил таймаут:
```rust
tracing::warn!(agent_id = %id, timeout_ticks, "failure detected");
```

Файл: `crates/swarm-runtime/src/task_registry.rs`

В `release_agent_tasks()` и `expire_tasks()`:
```rust
tracing::info!(task_id = %id, agent_id = %owner, "task released after agent failure");
tracing::info!(task_id = %id, "task expired");
```

Файл: `crates/swarm-runtime/src/coordinator.rs`

В `process_tick()`:
```rust
tracing::debug!(tick = current_tick, "coordinator tick");
```

---

### Шаг 5 — Добавить tracing в `swarm-alloc`

Файл: `crates/swarm-alloc/Cargo.toml`

Добавить:
```toml
tracing = { workspace = true }
```

В методе `allocate()` обеих реализаций (`GreedyAllocator`, `AuctionAllocator`):
```rust
tracing::debug!(task_id = %task_id, agent_id = %agent_id, "task allocated");
```

---

### Шаг 6 — Реализовать `agent_process` binary

Новый файл: `crates/swarm-examples/src/bin/agent_process.rs`

Бинарь запускается как отдельный OS-процесс. Принимает CLI аргументы:
- `--id <agent_id>` — идентификатор этого агента
- `--port <u16>` — UDP-порт для bind (127.0.0.1:<port>)
- `--peers <agent_id>:<port>[,...]` — список пиров в формате `agent-1:10001,agent-2:10002`
- `--tasks <json>` — JSON-массив задач (только для агента с ролью "coordinator-0")
- `--timeout-ticks <u64>` — тиков до failure detection (default: 5)
- `--tick-ms <u64>` — реальное время одного тика в мс (default: 100)
- `--max-ticks <u64>` — максимальное число тиков (default: 200)
- `--metrics-path <path>` — путь к файлу для записи итоговых метрик в JSON

**Архитектура:**

Каждый агент:
1. Парсит аргументы, создаёт `UdpTransport`
2. Создаёт `Coordinator` с полным начальным списком агентов и задач
3. Инициализирует `tracing-subscriber` с `RUST_LOG` filter, JSON/pretty форматтером
4. Запускает тик-цикл (wall-clock based через `std::thread::sleep`):
   - Отправляет heartbeat самому себе (как в sim-loop) + всем пирам
   - Drain-цикл `transport.poll()` до `None` — собирает входящие heartbeats
   - Вызывает `coordinator.process_tick(heartbeat_senders, tick, vec![])`
   - Если есть released/unassigned tasks — вызывает allocator
   - Логирует события через tracing
5. После `max_ticks` или SIGTERM — записывает метрики в `--metrics-path`

**Heartbeat сообщение:** `RawMessage { from: own_id, to: peer_id, payload: own_id.as_bytes() }`

**Дизайн-решение:** Каждый агент запускает идентичную копию `Coordinator` + `Allocator`.
Поскольку все агенты получают одинаковые heartbeats (детерминированный порядок не гарантирован,
но алгоритм сходится), они независимо приходят к схожим allocation-решениям. Конфликты
засчитываются в метрики. Это "replicated state machine" упрощённого вида — подходит для v0.3.

---

### Шаг 7 — Реализовать `multiprocess_scenario` binary (launcher + crash test)

Новый файл: `crates/swarm-examples/src/bin/multiprocess_scenario.rs`

**Сценарий:**
- 5 агентов (`agent-0`..`agent-4`), порты `10100`..`10104`
- 8 задач, распределённых при старте
- Через 2с после запуска — `kill -9` на `agent-0`
- Через 5с после kill — читаем метрики оставшихся агентов
- Проверяем: failure detection + reallocation произошли

**Реализация:**
1. Определяет конфигурацию агентов: 5 структур `{ id, port, tasks }`
2. Запускает 5 дочерних процессов через `std::process::Command`; каждому передаёт
   `--id`, `--port`, `--peers`, `--tasks`, `--metrics-path /tmp/swarm-v03/agent-N.json`
3. Создаёт директорию `/tmp/swarm-v03/` через `std::fs::create_dir_all`
4. `thread::sleep(Duration::from_secs(2))` — ждём стабилизации
5. Отправляет SIGKILL первому процессу через `child.kill()` (`Child::kill()` = SIGKILL на Unix)
6. `thread::sleep(Duration::from_secs(5))` — ждём failure detection (timeout=5 тиков × 100мс/тик = 500мс, но буфер большой)
7. Отправляет сигнал `terminate` оставшимся процессам через `child.kill()` + `child.wait()`
8. Читает JSON-файлы метрик из `/tmp/swarm-v03/agent-*.json`
9. Проверяет инварианты:
   - Минимум 1 агент зафиксировал отказ `agent-0` (поле `detected_failures`)
   - Задачи, ранее принадлежавшие `agent-0`, переназначены
10. Выводит сводный отчёт; завершается с кодом `1` при нарушении инвариантов

Файл метрик агента (формат):
```json
{
  "agent_id": "agent-1",
  "total_ticks": 70,
  "detected_failures": ["agent-0"],
  "tasks_assigned": ["task-2", "task-5"],
  "reallocation_count": 3
}
```

---

### Шаг 8 — Обновить `crates/swarm-examples/Cargo.toml`

Добавить:
```toml
swarm-comms     = { workspace = true }
swarm-runtime   = { workspace = true }
swarm-alloc     = { workspace = true }
tracing         = { workspace = true }
tracing-subscriber = { workspace = true }
serde_json      = { workspace = true }
serde           = { workspace = true }
```

Добавить записи `[[bin]]` для новых бинарей.

---

### Шаг 9 — Обновить `README.md`

- Добавить запись **Milestone 3** в раздел `## Current Status`
- Добавить описание новых бинарей:
  - `agent_process` — single agent process (используется launcher-ом)
  - `multiprocess_scenario` — запуск 5 агентов через UDP, crash test
- Документировать переменную `RUST_LOG` для наблюдаемости
- Обновить таблицу crates: `swarm-comms` — добавить UDP transport

---

## Testing Strategy

### Категория 1 — Без рефакторинга (реализовать вместе с основными изменениями)

**`swarm-comms` — UdpTransport unit-тесты** (`crates/swarm-comms/src/udp.rs`):

- `udp_send_recv_loopback` — два `UdpTransport`, один шлёт, другой получает на loopback
  (127.0.0.1:port_a → 127.0.0.1:port_b), проверить payload
- `udp_unknown_peer_returns_error` — `send()` к неизвестному AgentId возвращает
  `UdpTransportError::UnknownPeer`
- `udp_poll_empty_returns_none` — `poll()` на пустом сокете возвращает `Ok(None)`
- `udp_multiple_messages_in_order` — 3 сообщения подряд, `poll()` возвращает каждое

**`swarm-comms` — serde RawMessage**:

- `raw_message_serde_roundtrip` — `serde_json::to_vec` + `from_slice` roundtrip сохраняет
  все поля (from, to, payload)

**`swarm-runtime` — tracing проверка (без реального subscriber)**:

- Существующие тесты `coordinator_*`, `membership_*`, `failure_*`, `task_registry_*`
  должны продолжать проходить (tracing spans не ломают логику). Запустить и убедиться.

### Категория 2 — Лёгкий рефакторинг (потребуют небольшой инфраструктуры)

**Интеграционный тест `agent_process` startup**: 

Тест в `swarm-examples` (или отдельный integration test crate):
- Запускает `agent_process` с 2 агентами через `std::process::Command`
- Ждёт 500мс
- Проверяет, что оба процесса живы (`child.try_wait()` = None)
- Посылает SIGTERM, ждёт exit
- Читает metrics JSON, проверяет: `total_ticks > 0`, heartbeats обменялись

Требует: маленький хелпер для выбора свободных портов (`TcpListener::bind(0)` trick).

**Process crash test как `#[test]`**:

- Запускает `multiprocess_scenario` через `Command::new(...).status()`
- Проверяет exit code == 0
- Тест медленный (~10с), нужно пометить `#[ignore]` или вынести в separate test suite
- Требует: сборка `agent_process` и `multiprocess_scenario` доступна в `target/`

### Категория 3 — Тяжёлый рефакторинг (не планируется для v0.3)

- Property-based тест для `UdpTransport` с искусственной потерей пакетов — требует либо
  mock-слоя над `UdpSocket`, либо подмены транспорта на прокси-сокет. Не нужно для v0.3,
  in-memory уже покрывает это через `InMemNetwork` с `packet_loss_rate`.
- Тест сходимости distributed coordinator при разном порядке доставки heartbeats —
  требует управляемой UDP-очереди и детерминированного ввода. Актуально для v0.4 (gossip).
- Full multi-process property-based тест (N процессов × M сценариев отказов) — требует
  heavy test harness с изоляцией портов и параллельными запусками.

### Покрытие gap

- Не тестируется автоматически: сценарий kill -9 + wallclock recovery time < N секунд
  (тест с `#[ignore]` из категории 2 — best effort).
- Не тестируется: tracing-вывод содержит ожидаемые события (сложно без трейсинг-подписчика
  в тестах; приемлемый gap для v0.3).

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| Добавление serde к `RawMessage` | Бинарная несовместимость с существующим кодом, если где-то ожидается `RawMessage` без serde. Компиляция укажет. | `cargo check --workspace` |
| Изменение `swarm-comms/src/lib.rs` (добавление `pub mod udp`) | Конфликт имён, ошибки импортов в downstream | `cargo build --workspace` |
| Добавление `tracing` зависимостей | Версионный конфликт с уже имеющимися зависимостями | `cargo build --workspace` |
| `InMemNetwork` не изменяется: `ScenarioRunner` должен продолжить работать | Регрессия в существующих тестах | `cargo test --workspace` |
| Порты для UDP-тестов могут быть заняты | Flaky тесты на занятых портах | Использовать `TcpListener::bind("127.0.0.1:0")` для получения свободного порта |
| `agent_process` loop tick-период vs реальный UDP RTT | Если tick слишком быстрый (< 1мс), агенты могут не успеть обменяться | Минимальный `tick_ms = 10`; default 100 |
| Concurrent UDP poll от нескольких тестов | Flaky: если тесты запускаются параллельно и используют одинаковые порты | Каждый тест использует уникальные порты |

---

## Risks and Tradeoffs

**1. Sync vs async транспорт**

`UdpTransport` реализуется синхронно (non-blocking socket + `poll()`). Это проще и не
требует `tokio`. Трейдоф: v0.4 вероятно потребует async для gossip и partition handling.
Митигация: `Transport` trait останется стабильным — swap на async-реализацию не затронет
core runtime.

**2. Replicated state machine vs true distributed consensus**

Каждый агент-процесс запускает идентичный `Coordinator`. При неодинаковом порядке heartbeats
разные агенты могут сделать разные allocation-решения. Для v0.3 это приемлемо (конфликты
считаются в метрики). Настоящий consensus (Raft-подобный) — тема v0.5+.

**3. serde_json vs bincode для UDP**

JSON выбран за читаемость и простоту отладки. Минус: ~3-5× больше байт, чем bincode.
На loopback с 5 агентами это не имеет значения. Если в будущем планируется реальная сеть —
сменить сериализатор без изменения Transport trait.

**4. `/tmp/swarm-v03/` для метрик**

Простое решение для v0.3. Не очищается автоматически между запусками. Прием: добавить
`--metrics-path` в качестве CLI-аргумента, launcher передаёт уникальный путь на базе
timestamp.

**5. Tracing без structured spans trace_id**

В v0.3 tracing добавляется как basic spans без distributed trace propagation между
процессами. Для v0.3 этого достаточно. Distributed tracing (OpenTelemetry) — будущее.

---

## Open Questions

1. **Следует ли рефакторить `ScenarioRunner` для generic `T: Transport`?**
   В v0.3 `ScenarioRunner` остаётся привязанным к `InMemNetwork` (использует `drain_ready`,
   `advance_tick` которых нет в trait). Если рефакторить — нужно расширить trait или
   создать `SimTransport` wrapper trait.

2. **CLI формат для `--tasks` в `agent_process`?**
   Передача JSON-массива задач через CLI неудобна. Альтернатива: config JSON-файл с полной
   конфигурацией агента (`--config agent-0.json`). Решение принять перед имплементацией.

3. **Нужен ли отдельный `swarm-agent` crate или достаточно бинаря в `swarm-examples`?**
   Бинарь в `swarm-examples` проще для v0.3. `swarm-agent` crate понадобится когда появится
   смысл использовать agent logic как библиотеку (v0.4+).

4. **Как обрабатывать SIGTERM в `agent_process`?**
   Для записи метрик перед выходом нужен signal handler. Можно использовать `ctrlc` crate
   или `signal-hook`. В v0.3 достаточно записывать метрики периодически (каждые N тиков),
   а не только при exit.

5. **Добавить `tracing` в `swarm-sim` и `swarm-scenarios`?**
   Не критично для v0.3 — эти crates используются только in-process, tracing в runtime и
   alloc уже даёт нужную видимость. Можно добавить опционально.
