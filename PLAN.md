# PLAN: Milestone 3 — Pluggable Transport + Multiprocess (v0.3)

## Context

Milestone 1 (v0.1) реализовал детерминированную in-process симуляцию: heartbeat, membership,
failure detection, task reallocation.

Milestone 2 (v0.2) добавил динамические задачи, capability matching, auction allocator, pluggable
`Allocator` trait.

Milestone 3 (v0.3) выводит runtime из "игрушечного одного процесса": вводит общий `AgentNode<T>`
runtime-контур, pluggable Transport с двумя реализациями (in-memory и UDP), запускает агентов как
отдельные OS-процессы, добавляет сериализацию, process crash test, basic observability через
tracing.

**Критерий готовности:**
1. Один и тот же runtime (`AgentNode<T: Transport>`) работает in-process через
   `AgentNode<InMemAgentTransport>`.
2. Тот же runtime работает как N OS-процессов через `AgentNode<UdpTransport>`.
3. `kill -9` одного процесса → остальные обнаруживают отказ → перераспределяют задачи.

**Источники контекста:** `DRONE_A.1.md` (roadmap v0.3), `DRONE_B.1.md` (архитектура).
INVESTIGATION.md в workspace отсутствует.

---

## "Один и тот же runtime" — доказательство критерия

Критерий "same runtime" выполняется через `AgentNode<T: Transport>` — общий runtime-контур,
вводимый в этом milestone:

- **Core runtime**: `Coordinator` + `Allocator` (реализованы в v0.1–v0.2). Код не меняется.
- **Runtime driver**: `AgentNode<T>` (новый в v0.3) — тонкий контур с методом `tick()`,
  который вызывает Coordinator и Allocator одинаково в обоих режимах.
- **Transport слой**: `InMemNetwork` (уже есть) и `UdpTransport` (новый) — оба реализуют
  `Transport` trait; `AgentNode` не знает, какой именно.

```
In-process mode:                      Multi-process mode:
AgentNode<InMemAgentTransport>        AgentNode<UdpTransport>
      |                                   |
      tick()                              tick()
      |                                   |
  [same code]                         [same code]
  Coordinator + Allocator             Coordinator + Allocator
```

`ScenarioRunner` рефакторится: теперь создаёт N объектов `AgentNode<InMemAgentTransport>`
(лёгкий wrapper над `InMemNetwork`) и гоняет их в одном потоке. `agent_process` создаёт один
`AgentNode<UdpTransport>`. Оба вызывают одинаковый `tick()` метод.

### Multiprocess message protocol (replicated-state approach)

В v0.3 применяется **deterministic replicated-state** подход:

- **Протокол**: только heartbeat-сообщения (`RawMessage { from: own_id, to: peer_id, payload:
  b"hb" }`), broadcast всем пирам каждый тик. Получатель идентифицирует отправителя через
  `msg.from` — не через payload.
- **Алгоритм**: каждый агент-процесс запускает идентичный `Coordinator` с одинаковым начальным
  состоянием (одинаковый список агентов + задач). При получении тех же heartbeats все агенты
  приходят к тому же выводу о failures и allocation-решениях.
- **Сходимость**: гарантируется детерминизмом `GreedyAllocator`/`AuctionAllocator` на одинаковых
  входных данных. Расхождение возможно при разном порядке heartbeat-сообщений (network reordering)
  и метрически отслеживается как `conflicting_assignments`.
- **Тестирование конвергенции**: `multiprocess_scenario` после kill-9 + wait читает JSON-файлы
  метрик всех выживших агентов и сравнивает `global_assignment_map` (полная карта `TaskId →
  AgentId`) — у всех выживших она должна совпадать. Дополнительно проверяется: задачи agent-0
  переназначены другим агентам, нет задач без владельца.

Полный state-sync (distribution of allocation decisions) — тема v0.4.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-comms/src/transport.rs` | Добавить serde к `RawMessage` |
| `crates/swarm-comms/src/network.rs` | Добавить `InMemAgentTransport` wrapper (реализует Transport для per-agent use) |
| `crates/swarm-comms/src/udp.rs` | Новый файл — `UdpTransport` + `UdpTransportError` |
| `crates/swarm-comms/src/lib.rs` | Экспортировать UDP модуль |
| `crates/swarm-comms/Cargo.toml` | Добавить `serde_json`, `tracing` |
| `crates/swarm-runtime/src/node.rs` | Новый файл — `AgentNode<T>` + `NodeTickOutput` |
| `crates/swarm-runtime/src/lib.rs` | Экспортировать `AgentNode`, `NodeTickOutput` |
| `crates/swarm-runtime/src/coordinator.rs` | Tracing spans |
| `crates/swarm-runtime/src/membership.rs` | Tracing spans |
| `crates/swarm-runtime/src/failure.rs` | Tracing spans |
| `crates/swarm-runtime/src/task_registry.rs` | Tracing spans |
| `crates/swarm-runtime/Cargo.toml` | Добавить `tracing`, `swarm-alloc` (для AgentNode) |
| `crates/swarm-alloc/Cargo.toml` | Добавить `tracing` |
| `crates/swarm-alloc/src/...` | Tracing spans в `allocate()` |
| `crates/swarm-sim/src/runner.rs` | Рефактор: использовать `AgentNode` внутри |
| `crates/swarm-examples/src/bin/agent_process.rs` | Новый файл — single-agent OS-process |
| `crates/swarm-examples/src/bin/multiprocess_scenario.rs` | Новый файл — launcher + crash test |
| `crates/swarm-examples/Cargo.toml` | Добавить зависимости для новых бинарей |
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

`serde_json`, `serde` уже есть в workspace.

---

### Шаг 2 — Добавить serde к `RawMessage`; добавить `InMemAgentTransport`

Файл: `crates/swarm-comms/src/transport.rs`

Добавить `#[derive(Serialize, Deserialize)]` к `RawMessage`:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawMessage { ... }
```

Файл: `crates/swarm-comms/src/network.rs`

Добавить `InMemAgentTransport` — per-agent Transport wrapper над shared `InMemNetwork`.
`InMemNetwork` сам по себе НЕ изменяется (не добавляем `own_id` поле).
`ScenarioRunner` создаёт один `InMemNetwork` на симуляцию и N `InMemAgentTransport` — по одному на агента:

```rust
pub struct InMemAgentTransport {
    bus: Rc<RefCell<InMemNetwork>>,
    own_id: AgentId,
}

impl Transport for InMemAgentTransport {
    type Error = Infallible;
    fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error> {
        self.bus.borrow_mut().send(msg)
    }
    fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error> {
        Ok(self.bus.borrow_mut().drain_ready(&self.own_id).into_iter().next())
    }
}
```

Добавить в `crates/swarm-comms/Cargo.toml`:
```toml
serde      = { workspace = true }
serde_json = { workspace = true }
tracing    = { workspace = true }
```

---

### Шаг 3 — Реализовать `UdpTransport`

Новый файл: `crates/swarm-comms/src/udp.rs`

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

`UdpTransport::new(bind_addr: SocketAddr, peers: HashMap<AgentId, SocketAddr>) -> Result<Self, UdpTransportError>`:
- `UdpSocket::bind(bind_addr)`
- `socket.set_nonblocking(true)`
- `recv_buf`: 65535 байт

`impl Transport for UdpTransport`:
- `send()`: `serde_json::to_vec(&msg)` → `socket.send_to(bytes, addr)`
- `poll()`: `socket.recv_from(&mut recv_buf)`:
  - `Ok((n, _))` → `from_slice(&recv_buf[..n])` → `Ok(Some(msg))`
  - `WouldBlock` → `Ok(None)`
  - Другие IO ошибки → `Err(UdpTransportError::Io(e))`

Обновить `crates/swarm-comms/src/lib.rs`:
```rust
pub mod udp;
pub use udp::{UdpTransport, UdpTransportError};
```

---

### Шаг 4 — Определить `AgentNode<T: Transport>` — общий runtime-контур

Новый файл: `crates/swarm-runtime/src/node.rs`

Это **единственный** runtime-контур, используемый и in-process, и в multi-process режиме.

```rust
use swarm_alloc::Allocator;
use swarm_comms::Transport;
use swarm_types::{AgentId, Task, TaskId};
use crate::{Coordinator, TaskRegistry};

pub struct AgentNode<T> {
    pub coordinator: Coordinator,
    transport: T,
    own_id: AgentId,
    /// ordered list of peers to send heartbeats to
    peer_ids: Vec<AgentId>,
}

pub struct NodeTickOutput {
    pub newly_failed: Vec<AgentId>,
    pub released_tasks: Vec<TaskId>,
    pub expired_task_ids: Vec<TaskId>,
    pub conflicting_assignments: u64,
}

impl<T: Transport> AgentNode<T> {
    pub fn new(own_id: AgentId, peer_ids: Vec<AgentId>,
               coordinator: Coordinator, transport: T) -> Self

    /// One coordination tick: broadcast heartbeat → receive msgs → coordinator → allocate.
    pub fn tick<A: Allocator>(
        &mut self,
        current_tick: u64,
        allocator: &A,
        injected: Vec<Task>,
    ) -> Result<NodeTickOutput, T::Error>
}
```

`tick()` внутри:
1. Отправить heartbeat всем `peer_ids`: `transport.send(RawMessage { from: own_id.clone(), to: peer_id, payload: b"hb".to_vec() })`
2. **Self-heartbeat**: `let mut heartbeat_senders = vec![own_id.clone()]` — собственный агент
   всегда считается живым на текущем тике. Это гарантирует, что локальный `Coordinator` никогда
   не пометит собственный `own_id` dead из-за отсутствия heartbeat.
3. Дренировать входящие: `while let Some(msg) = transport.poll()? { heartbeat_senders.push(msg.from) }`
   Отправитель определяется через `msg.from` (typed поле), payload игнорируется как content.
4. `coordinator.process_tick(heartbeat_senders, current_tick, injected)`
4. Если есть released/unassigned tasks → allocate, считать конфликты
5. Вернуть `NodeTickOutput`

Обновить `crates/swarm-runtime/src/lib.rs`:
```rust
pub mod node;
pub use node::{AgentNode, NodeTickOutput};
```

Добавить в `crates/swarm-runtime/Cargo.toml`:
```toml
tracing    = { workspace = true }
swarm-alloc = { workspace = true }
swarm-comms = { workspace = true }
```

---

### Шаг 5 — Добавить tracing в `swarm-runtime` и `swarm-alloc`

Файл: `crates/swarm-runtime/src/membership.rs`:
```rust
tracing::debug!(agent_id = %id, "heartbeat recorded");
tracing::warn!(agent_id = %id, "agent marked dead");
```

Файл: `crates/swarm-runtime/src/failure.rs`:
```rust
tracing::warn!(agent_id = %id, timeout_ticks, "failure detected");
```

Файл: `crates/swarm-runtime/src/task_registry.rs`:
```rust
tracing::info!(task_id = %id, agent_id = %owner, "task released after agent failure");
tracing::info!(task_id = %id, "task expired");
```

Файл: `crates/swarm-runtime/src/coordinator.rs`:
```rust
tracing::debug!(tick = current_tick, "coordinator tick");
```

Добавить `tracing = { workspace = true }` в `crates/swarm-alloc/Cargo.toml`.

В методе `allocate()` обеих реализаций:
```rust
tracing::debug!(task_id = %task_id, agent_id = %agent_id, "task allocated");
```

---

### Шаг 6 — Рефакторить `ScenarioRunner` для использования `AgentNode`

Файл: `crates/swarm-sim/src/runner.rs`

`ScenarioRunner::run_with()` рефакторится: вместо ручного вызова coordinator + network создаёт
N объектов `AgentNode<InMemAgentTransport>` (по одному на агента, на базе единой shared шины).
На каждом тике вызывает `node.tick()` для всех агентов последовательно.

`InMemAgentTransport` определён в Шаге 2 (`swarm-comms/src/network.rs`). Использует
`Rc<RefCell<InMemNetwork>>` — выбрано потому что симуляция однопоточная, `Arc<Mutex<>>` не
нужен и добавляет ненужные издержки.

Ключевой инвариант: `ScenarioRunner` использует `AgentNode::tick()`, не дублирует логику.
Все существующие тесты runner должны проходить после рефакторинга.

---

### Шаг 7 — Реализовать `agent_process` binary

Новый файл: `crates/swarm-examples/src/bin/agent_process.rs`

Принимает JSON-конфиг через `--config <path>`:
```json
{
  "agent_id": "agent-0",
  "bind_addr": "127.0.0.1:10150",
  "peers": { "agent-1": "127.0.0.1:10151", "agent-2": "127.0.0.1:10152" },
  "agents": [...],
  "tasks": [...],
  "timeout_ticks": 5,
  "tick_ms": 100,
  "max_ticks": 200,
  "metrics_path": "/tmp/swarm-v03/agent-0.json"
}
```

Конкретные порты (10150, 10151…) — это значения, выделенные `allocate_udp_ports()` в launcher-е
и записанные в config-файл каждого агента перед его запуском. `bind_addr` всегда содержит
конкретный порт — не 0.

Использует `AgentNode<UdpTransport>`:
```rust
let transport = UdpTransport::new(bind_addr, peers)?;
let coordinator = Coordinator::new(agents, tasks, timeout_ticks);
let mut node = AgentNode::new(agent_id.clone(), peer_ids, coordinator, transport);

let allocator = GreedyAllocator; // or AuctionAllocator
for tick in 0..max_ticks {
    let output = node.tick(tick, &allocator, vec![])?;
    // log output via tracing
    // periodically write metrics to metrics_path
    thread::sleep(Duration::from_millis(tick_ms));
}
```

Инициализирует `tracing-subscriber` с `RUST_LOG` filter.

Формат файла метрик (JSON, пишется каждые 10 тиков и при exit):
```json
{
  "agent_id": "agent-1",
  "total_ticks": 70,
  "detected_failures": ["agent-0"],
  "local_task_ids": ["task-2"],
  "global_assignment_map": {
    "task-0": "agent-1",
    "task-1": "agent-2",
    "task-2": "agent-1",
    "task-3": "agent-3"
  },
  "reallocation_count": 3
}
```

`local_task_ids` — задачи, назначенные именно этому агенту (per-agent subset).
`global_assignment_map` — полная карта `TaskId → AgentId` по всей системе
(вычисляется агентом из состояния `TaskRegistry`). Используется для проверки конвергенции.

---

### Шаг 8 — Реализовать `multiprocess_scenario` binary (launcher + crash test)

Новый файл: `crates/swarm-examples/src/bin/multiprocess_scenario.rs`

**Динамическое выделение UDP-портов** — чтобы избежать flaky failures на занятых портах.
Порты резервируются через UDP-сокеты (не TCP — TCP и UDP независимы):
```rust
fn allocate_udp_ports(n: usize) -> Vec<u16> {
    // Bind UDP sockets to port 0, OS assigns free ports.
    // Drop sockets immediately; child processes bind to these specific ports.
    // The race window is very small on loopback in a controlled test environment.
    (0..n)
        .map(|_| {
            let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
            sock.local_addr().unwrap().port()
        })
        .collect()
}

let ports = allocate_udp_ports(N);
```

**Сценарий:**
1. Выделить N (=5) свободных loopback-портов динамически
2. Записать config JSON для каждого агента в `/tmp/swarm-v03/config-N.json`
3. Запустить 5 дочерних `agent_process` процессов через `std::process::Command`
4. `thread::sleep(Duration::from_secs(2))` — стабилизация
5. `child[0].kill()` — SIGKILL на agent-0
6. `thread::sleep(Duration::from_secs(3))` — ждём failure detection (5 тиков × 100мс = 500мс реально; 3с — надёжный буфер)
7. Остановить оставшиеся процессы: `child.kill()` + `child.wait()`
8. Читать JSON-метрики из `/tmp/swarm-v03/agent-N.json`
9. Проверить инварианты:
   - **Все** выжившие агенты зафиксировали `"agent-0"` в `detected_failures` (соответствует
     критерию "остальные обнаруживают отказ")
   - `global_assignment_map` у всех выживших агентов совпадает (конвергенция replicated-state)
   - В `global_assignment_map` ни одна задача не принадлежит `"agent-0"` (reassigned)
   - В `global_assignment_map` нет задач без владельца (все задачи assigned)
10. Вывести отчёт; exit code `1` при нарушении

---

### Шаг 9 — Обновить `crates/swarm-examples/Cargo.toml`

Добавить:
```toml
swarm-comms        = { workspace = true }
swarm-runtime      = { workspace = true }
swarm-alloc        = { workspace = true }
tracing            = { workspace = true }
tracing-subscriber = { workspace = true }
serde              = { workspace = true }
serde_json         = { workspace = true }
```

Добавить записи `[[bin]]` для `agent_process` и `multiprocess_scenario`.

---

### Шаг 10 — Обновить `README.md`

- Добавить запись **Milestone 3** в раздел `## Current Status`
- Обновить таблицу crates: `swarm-comms` — добавить UDP transport; `swarm-runtime` — добавить `AgentNode`
- Добавить описание новых бинарей в `## Run Examples`:
  - `agent_process` — single-agent process, используется launcher-ом
  - `multiprocess_scenario` — запуск 5 агентов через UDP loopback, crash test
- Документировать `RUST_LOG` для observability

---

## Verification Commands

После реализации выполнить в указанном порядке:

```bash
# 1. Форматирование
cargo fmt --all

# 2. Lint (должен пройти без warnings)
cargo clippy --all-targets -- -D warnings

# 3. Полная сборка
cargo build --workspace

# 4. Тесты (регрессия + новые unit-тесты)
cargo test --workspace

# 5. Acceptance check: multiprocess crash scenario (exit code 0 = success)
cargo run -p swarm-examples --bin multiprocess_scenario

# 6. Белый дым: существующие сценарии без регрессий
cargo run -p swarm-examples --bin coverage_with_failure
cargo run -p swarm-examples --bin dynamic_auction
```

---

## Testing Strategy

### Категория 1 — Без рефакторинга (реализовать вместе с основными изменениями)

**`swarm-comms` — `UdpTransport` unit-тесты** (`crates/swarm-comms/src/udp.rs`):

- `udp_send_recv_loopback` — два `UdpTransport` на динамических портах loopback; один шлёт,
  другой получает — проверить payload
- `udp_unknown_peer_returns_error` — `send()` к неизвестному `AgentId` возвращает
  `UdpTransportError::UnknownPeer`
- `udp_poll_empty_returns_none` — `poll()` на пустом non-blocking сокете возвращает `Ok(None)`
- `udp_multiple_messages_received` — 3 сообщения подряд, цикл `poll()` возвращает каждое

**`swarm-comms` — serde `RawMessage`**:

- `raw_message_serde_roundtrip` — `serde_json::to_vec` → `from_slice` сохраняет все поля

**`swarm-comms` — `InMemNetwork::for_agent` + `poll()`**:

- `inmem_agent_poll_receives_own_messages` — `for_agent("a1")`, `send(to: "a1")`, `poll()` → `Some(msg)`
- `inmem_agent_poll_ignores_other_agent_messages` — `for_agent("a1")`, `send(to: "a2")`, `poll()` → `None`

**`swarm-runtime` — `AgentNode` unit-тесты** (`crates/swarm-runtime/src/node.rs`):

- `node_tick_sends_heartbeats_to_peers` — после одного `tick()` пиры получают heartbeat-сообщения
- `node_tick_self_heartbeat_keeps_own_agent_alive` — запустить `AgentNode` на `timeout_ticks + 2`
  тиков без каких-либо входящих сообщений (транспорт не шлёт ничего); проверить, что
  `own_id` ни разу не появляется в `NodeTickOutput::newly_failed`. Тест гарантирует корректность
  self-heartbeat логики.
- `node_tick_detects_failure` — агент не отправляет heartbeats N тиков → `newly_failed` содержит его id
- `node_tick_reallocates_after_failure` — задача упавшего агента перераспределяется через allocator
- `node_tick_same_output_inmem_vs_stub_transport` — идентичные inputs → идентичный `NodeTickOutput`
  (доказывает "same runtime" для обоих транспортов)

**Регрессионные тесты:**

- Все существующие тесты в `swarm-comms`, `swarm-runtime`, `swarm-alloc`, `swarm-sim` должны
  пройти после рефакторинга `ScenarioRunner` → `cargo test --workspace`

### Категория 2 — Лёгкий рефакторинг (потребуют небольшой инфраструктуры)

**Интеграционный тест `agent_process` запуска** (тест в `swarm-examples` или integration test crate):

- Запускает 2 `agent_process` на динамических портах через `std::process::Command`
- Ждёт 500мс, проверяет `child.try_wait()` == `None` (процессы живы)
- Посылает SIGKILL/SIGTERM, ждёт exit
- Читает metrics JSON: `total_ticks > 0`, `detected_failures` пуст (никто не падал)

Требует: маленький хелпер `free_port()` и сборка `agent_process` binary доступна в `target/`.

**Process crash test как `#[test]`** (с `#[ignore]`):

- Запускает `multiprocess_scenario` через `Command::new(binary).status()`
- Проверяет exit code == 0
- Пометить `#[ignore]` (занимает ~10с), запускать явно через `cargo test -- --ignored`

### Категория 3 — Тяжёлый рефакторинг (не планируется для v0.3)

- Property-based тест для `UdpTransport` с искусственной потерей пакетов — требует mock UDP proxy.
- Тест сходимости distributed coordinator при разном порядке доставки heartbeats — требует
  управляемой UDP-очереди. Актуально для v0.4 (gossip).
- Full multi-process property-based тест (N × M сценариев) — heavy harness с изоляцией портов.

### Покрытие gap

- **Gap**: tracing-вывод содержит ожидаемые события — нет tracing subscriber в тестах;
  приемлемый gap для v0.3 (логика покрыта, формат логов — нет).
- **Gap**: wallclock recovery time < N секунд не проверяется автоматически; тест из категории 2
  с `#[ignore]` является best-effort проверкой.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| Рефакторинг `ScenarioRunner` на `AgentNode` | Регрессия в существующих runner-тестах; изменение semantics tick loop | `cargo test -p swarm-sim` + `cargo test --workspace` |
| Добавление serde к `RawMessage` | Ошибки компиляции если `RawMessage` используется в местах без serde | `cargo check --workspace` |
| Добавление поля `own_id` в `InMemNetwork` | Изменение конструктора `InMemNetwork::new()` сломает существующие тесты | Обновить тесты в `swarm-comms`, `swarm-sim` |
| `InMemNetwork::poll()` теперь что-то возвращает | Код, который вызывал `poll()` и ожидал `Ok(None)`, может получить сообщение | `cargo test -p swarm-comms` |
| Добавление `tracing` в критические пути | Накладные расходы при отключённых подписчиках — минимальны (tracing использует no-op); | Запустить benchmark если нужно |
| UDP-порты могут конфликтовать между параллельными тестами | Flaky тесты | Использовать `free_loopback_port()` в каждом тесте, никогда не хардкодировать порты |
| `swarm-runtime` начинает зависеть от `swarm-alloc` (через AgentNode) | Потенциальный circular dependency если alloc зависит от runtime | Проверить: `swarm-alloc` зависит только от `swarm-types` — circular нет |
| `agent_process` пишет в `/tmp/swarm-v03/` | Директория может не существовать или быть нечитаемой | `std::fs::create_dir_all` + обработка ошибок |

---

## Risks and Tradeoffs

**1. AgentNode зависимости: swarm-runtime → swarm-alloc**

`AgentNode` использует `Allocator` trait из `swarm-alloc`. Это добавляет зависимость
`swarm-runtime → swarm-alloc`. Проверить что `swarm-alloc` не зависит от `swarm-runtime`
(на данный момент `swarm-alloc` → `swarm-types` only — circular нет).

**2. ScenarioRunner рефакторинг: `InMemAgentTransport` wrapper**

Выбран `Rc<RefCell<InMemNetwork>>` + `InMemAgentTransport` (определён в Шаге 2, используется
в Шаге 6). `Rc<RefCell<>>` достаточен: симуляция однопоточная, `Arc<Mutex<>>` избыточен.
Критерий: все существующие тесты ScenarioRunner проходят без изменения публичного API.

**6. Self-heartbeat корректность**

Каждый агент добавляет `own_id` в `heartbeat_senders` до `process_tick()`. Если этот шаг
пропустить, `FailureDetector` пометит живой агент dead после `timeout_ticks`. Риск
верифицируется тестом `node_tick_self_heartbeat_keeps_own_agent_alive` (категория 1).

**3. Replicated-state vs true distributed consensus**

V0.3 использует replicated-state подход. При network reordering агенты могут временно
расходиться. `conflicting_assignments` в метриках является единственным observable сигналом
о расхождениях. Для production-grade consistency нужен Raft/Paxos-подобный консенсус (v0.5+).

**4. serde_json для UDP framing**

JSON выбран за читаемость при отладке. На loopback с 5 агентами overhead незначителен.
При переходе к реальной сети — сменить на bincode без изменения Transport trait.

**5. Sync UDP transport**

`UdpTransport` синхронный (non-blocking). Не требует tokio для v0.3. Трейдоф: v0.4 потребует
async для gossip. Митигация: Transport trait остаётся стабильным, async impl добавляется позже.

---

## Open Questions

1. **SIGTERM handler в `agent_process` для записи финальных метрик?**
   Использовать `ctrlc` crate или просто писать метрики каждые N тиков. Второй вариант проще
   и достаточен для v0.3 (метрики пишутся периодически, не только при выходе).

2. **Добавить `tracing` в `swarm-sim` и `swarm-scenarios`?**
   Не критично для v0.3; трейсинг в runtime и alloc уже даёт нужную видимость.
