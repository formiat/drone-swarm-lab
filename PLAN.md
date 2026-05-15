# PLAN.md — Milestone 0: Каркас

## Context

Строим Swarm Coordination Runtime — слой distributed coordination для автономных групп дронов,
проверяемый через Mission Digital Twin / Scenario Test Harness.

Главный продукт: runtime координации (membership, failure detection, task reallocation).
Стенд: воспроизводимый headless сценарный runner с управляемой сетью, инъекцией отказов и метриками.

Milestone 0 закладывает технический фундамент без бизнес-логики:
Cargo workspace, базовые типы, абстракции Transport и Clock, минимальный Scenario,
один запускаемый example.

## Investigation context

`INVESTIGATION.md` не обнаружен. Контекст получен из `DRONE_A.1.md` и `DRONE_B.1.md`.

Ключевые выводы, влияющие на дизайн:

- Дрон для runtime — агент с ID, ролью, health, capabilities, задачей, inbox/outbox.
  Неважно, реальный агент или симулируемый — runtime работает через абстракции.
- Transport, Clock, StateProvider — точки замены реального поведения симулированным.
- На Milestone 0 "умной" логики нет: ни membership, ни allocation, ни comms-degradation.
  Только фундамент, который станет основой для Milestone 1.
- Runtime не должен знать ничего о физике, автопилоте или PID.

## Affected components

Все компоненты создаются с нуля (репозиторий пуст).

| Крейт | Статус | Содержимое в Milestone 0 |
|---|---|---|
| `crates/swarm-types` | создаётся | AgentId, TaskId, MessageId, Pose, Velocity, Health, Role, Capability, Agent, Task, TaskStatus |
| `crates/swarm-comms` | создаётся | Transport trait, RawMessage |
| `crates/swarm-sim` | создаётся | Clock, Tick, Scenario |
| `crates/swarm-examples` | создаётся | bin `empty_scenario` — запуск пустого сценария |
| `crates/swarm-runtime` | stub | пустой крейт, `lib.rs` с `// TODO` |
| `crates/swarm-alloc` | stub | пустой крейт |
| `crates/swarm-metrics` | stub | пустой крейт |
| `crates/swarm-replay` | stub | пустой крейт |
| `crates/swarm-scenarios` | stub | пустой крейт |

## Implementation steps

### Шаг 1 — Workspace `Cargo.toml`

Файл: `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "crates/swarm-types",
    "crates/swarm-comms",
    "crates/swarm-sim",
    "crates/swarm-runtime",
    "crates/swarm-alloc",
    "crates/swarm-metrics",
    "crates/swarm-replay",
    "crates/swarm-scenarios",
    "crates/swarm-examples",
]

[workspace.dependencies]
swarm-types   = { path = "crates/swarm-types" }
swarm-comms   = { path = "crates/swarm-comms" }
swarm-sim     = { path = "crates/swarm-sim" }
derive_more   = { version = "1", features = ["as_ref", "as_mut", "deref", "deref_mut", "from", "into", "display"] }
serde         = { version = "1", features = ["derive"] }
thiserror     = "2"
```

### Шаг 2 — `crates/swarm-types`

**`crates/swarm-types/Cargo.toml`**

```toml
[package]
name    = "swarm-types"
version = "0.1.0"
edition = "2021"

[dependencies]
derive_more = { workspace = true }
serde       = { workspace = true }
```

**`crates/swarm-types/src/lib.rs`**

```rust
pub mod agent;
pub mod message;
pub mod pose;
pub mod task;

pub use agent::{Agent, AgentId, Capability, Health, Role};
pub use message::{Message, MessageId};
pub use pose::{Pose, Velocity};
pub use task::{Task, TaskId, TaskStatus};
```

**`crates/swarm-types/src/agent.rs`**

- `AgentId(String)` — newtype, приватное поле, derive AsMut/AsRef/Deref/DerefMut/From/Into/Display/Clone/Debug/PartialEq/Eq/Hash/Serialize/Deserialize
- `Health` — enum: `Alive`, `Degraded`, `Dead`; derive Clone/Debug/PartialEq/Eq/Serialize/Deserialize; serde(rename_all = "snake_case")
- `Role` — enum: `Scout`, `Relay`, `Mapper`, `Inspector`, `Carrier`; те же derive; serde snake_case
- `Capability(String)` — newtype с теми же правилами что AgentId
- `Agent` — struct: `id: AgentId`, `role: Role`, `health: Health`, `pose: Pose`, `capabilities: Vec<Capability>`, `current_task: Option<TaskId>`

**`crates/swarm-types/src/task.rs`**

- `TaskId(String)` — newtype, те же derive что AgentId
- `TaskStatus` — enum: `Unassigned`, `Assigned`, `InProgress`, `Completed`, `Failed`; serde snake_case
- `Task` — struct: `id: TaskId`, `status: TaskStatus`, `assigned_to: Option<AgentId>`, `priority: u8`

**`crates/swarm-types/src/message.rs`**

- `MessageId(String)` — newtype
- `Message<P>` — struct: `id: MessageId`, `from: AgentId`, `to: AgentId`, `payload: P`

**`crates/swarm-types/src/pose.rs`**

- `Pose` — struct: `x: f64`, `y: f64`; derive Clone/Copy/Debug/PartialEq/Serialize/Deserialize
- `Velocity` — struct: `vx: f64`, `vy: f64`; те же derive

### Шаг 3 — `crates/swarm-comms`

**`crates/swarm-comms/Cargo.toml`**

```toml
[package]
name    = "swarm-comms"
version = "0.1.0"
edition = "2021"

[dependencies]
swarm-types = { workspace = true }
thiserror   = { workspace = true }
```

**`crates/swarm-comms/src/lib.rs`**

```rust
pub mod transport;
pub use transport::{RawMessage, Transport};
```

**`crates/swarm-comms/src/transport.rs`**

- `RawMessage` — struct: `from: AgentId`, `to: AgentId`, `payload: Vec<u8>`; derive Clone/Debug
- `Transport` — trait:
  ```rust
  pub trait Transport {
      type Error: std::error::Error + Send + Sync + 'static;
      fn send(&mut self, msg: RawMessage) -> Result<(), Self::Error>;
      fn poll(&mut self) -> Result<Option<RawMessage>, Self::Error>;
  }
  ```

### Шаг 4 — `crates/swarm-sim`

**`crates/swarm-sim/Cargo.toml`**

```toml
[package]
name    = "swarm-sim"
version = "0.1.0"
edition = "2021"

[dependencies]
swarm-types = { workspace = true }
serde       = { workspace = true }
```

**`crates/swarm-sim/src/lib.rs`**

```rust
pub mod clock;
pub mod scenario;

pub use clock::{Clock, Tick};
pub use scenario::Scenario;
```

**`crates/swarm-sim/src/clock.rs`**

- `Tick(u64)` — newtype, приватное поле, derive AsMut/AsRef/Deref/DerefMut/From/Into/Clone/Copy/Debug/PartialEq/Eq/PartialOrd/Ord
- `Clock` — struct: `current: Tick`, `tick_duration_ms: u64`
  - `Clock::new(tick_duration_ms: u64) -> Self`
  - `fn now(&self) -> Tick`
  - `fn advance(&mut self)` — инкремент на 1 тик
  - `fn elapsed_ms(&self) -> u64` — `current.0 * tick_duration_ms`

**`crates/swarm-sim/src/scenario.rs`**

- `Scenario` — struct: `name: String`, `seed: u64`, `agents: Vec<Agent>`, `tasks: Vec<Task>`; derive Clone/Debug/Serialize/Deserialize
- `Scenario::empty(name: impl Into<String>, seed: u64) -> Self`

### Шаг 5 — Stub-крейты

Для каждого: `swarm-runtime`, `swarm-alloc`, `swarm-metrics`, `swarm-replay`, `swarm-scenarios`:

```
crates/<name>/
  Cargo.toml   (package name, version 0.1.0, edition 2021, нет зависимостей пока)
  src/lib.rs   (один комментарий // TODO: implement in future milestones)
```

### Шаг 6 — `crates/swarm-examples`

**`crates/swarm-examples/Cargo.toml`**

```toml
[package]
name    = "swarm-examples"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "empty_scenario"
path = "src/bin/empty_scenario.rs"

[dependencies]
swarm-types = { workspace = true }
swarm-sim   = { workspace = true }
```

**`crates/swarm-examples/src/bin/empty_scenario.rs`**

Логика:
1. Создать `Scenario::empty("empty", 42)`.
2. Создать `Clock::new(100)` (100 ms/tick).
3. Пройти 10 тиков в цикле, вызывая `clock.advance()`.
4. Вывести: `"Scenario '{}' finished: {} ticks ({} ms elapsed)"`.
5. Завершить с кодом 0.

### Шаг 7 — `cargo fmt --all` + clippy + test + smoke-test + commit

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p swarm-examples --bin empty_scenario
git add Cargo.toml Cargo.lock crates/
git commit -m "feat: Milestone 0 — workspace scaffold and foundational types"
```

## Testing strategy

### Категория 1 — без рефакторинга (реализуются вместе с кодом)

Unit-тесты: `crates/swarm-types/src/agent.rs`, `task.rs`, `crates/swarm-sim/src/clock.rs` — модули `#[cfg(test)]`.
Smoke-тест: `cargo run -p swarm-examples --bin empty_scenario` — выполняется в шаге 7.

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| 1.1 | `agent_id_newtype_roundtrip` | swarm-types | `AgentId::from("abc")` → `Deref` возвращает `"abc"` |
| 1.2 | `task_id_newtype_roundtrip` | swarm-types | Аналогично для `TaskId` |
| 1.3 | `message_id_newtype_roundtrip` | swarm-types | Аналогично для `MessageId` |
| 1.4 | `health_serde_snake_case` | swarm-types | `Health::Alive` сериализуется в `"alive"` |
| 1.5 | `role_serde_snake_case` | swarm-types | `Role::Scout` → `"scout"` |
| 1.6 | `task_status_serde_snake_case` | swarm-types | `TaskStatus::Unassigned` → `"unassigned"` |
| 1.7 | `clock_starts_at_zero` | swarm-sim | `Clock::new(100).now()` → `Tick(0)` |
| 1.8 | `clock_advance_increments` | swarm-sim | После `advance()` → `Tick(1)` |
| 1.9 | `clock_elapsed_ms` | swarm-sim | 3 тика × 100 ms = 300 ms |
| 1.10 | `scenario_empty_has_no_agents` | swarm-sim | `Scenario::empty(…).agents.is_empty()` |
| 1.11 | `scenario_empty_has_no_tasks` | swarm-sim | `Scenario::empty(…).tasks.is_empty()` |
| 1.12 | smoke-тест `empty_scenario` | swarm-examples | `cargo run -p swarm-examples --bin empty_scenario` завершается с кодом 0 и печатает итог |

### Категория 2 — лёгкий рефакторинг (после Milestone 0)

| # | Тест | Описание |
|---|------|----------|
| 2.1 | `transport_mock_send_recv` | Мок-реализация Transport в swarm-comms: отправленное сообщение можно получить через `poll()` |
| 2.2 | `agent_construction_valid` | Полная конструкция `Agent` с capabilities; проверить `current_task = None` |
| 2.3 | `capability_newtype_deref` | `Capability::from("thermal")` → deref возвращает `"thermal"` |

### Категория 3 — тяжёлый рефакторинг (будущие milestones)

| # | Тест | Описание |
|---|------|----------|
| 3.1 | Полноценный event-driven ScenarioRunner | Требует event loop, очереди событий, инъекции отказов; появится в Milestone 1 |
| 3.2 | Property-based тесты Clock | Требует `proptest`; добавить в Milestone 1 |
| 3.3 | Transport: in-memory реализация с потерями пакетов | Основная тестовая среда Milestone 1; здесь не место |

## Risks and tradeoffs

### Что могло сломаться

Milestone 0 — это первый коммит существенного кода. Регрессий в существующем коде нет.
Риски носят архитектурный характер:

| Риск | Вероятность | Митигация |
|------|-------------|-----------|
| API `swarm-types` потребует изменений в Milestone 1 | Средняя | Типы продуманы с запасом; breaking change — норма до v0.1.0 |
| `derive_more` v1 vs v2 — несовместимость feature flags | Низкая | Зафиксировать версию в workspace; проверить фичи при добавлении |
| `Tick(u64)` окажется неудобным для временны́х расчётов | Низкая | Можно добавить методы или перейти на `Duration` в Milestone 1 |
| Stub-крейты без `lib.rs` вызовут ошибку workspace | Высокая | Обязательно создать `src/lib.rs` во всех stub-крейтах |

### Tradeoffs

- **`Tick(u64)` вместо `DateTime<Utc>`** — в симуляции нет реального времени, tick-based clock проще для deterministic replay. `DateTime` добавим в модели событий/метрик позже.
- **Нет event-driven `ScenarioRunner` на Milestone 0** — `empty_scenario` вручную тикает Clock в цикле; этого достаточно для smoke-теста. Полноценный runner с event loop и инъекцией отказов появится в Milestone 1.
- **Все stub-крейты в workspace сразу** — позволяет зафиксировать структуру до начала реализации, избегает добавления крейтов по одному с перестройкой зависимостей.

## Open questions

1. **Формат `Capability`**: оставить `Capability(String)` или сделать enum с фиксированными вариантами (`Thermal`, `Optical`, `Relay`, …)? Enum проще матчить; newtype гибче. Решить в Milestone 1 когда появится allocation.
2. **`Message<P>` generics vs typed enum**: использовать generic payload или enum `MessagePayload`? Generic чище для Transport; enum проще для сериализации. Решить в Milestone 1 при реализации heartbeat.
3. **Нужен ли `Makefile` / `justfile`?**: удобен для `make clippy`, `make test`. Добавить вместе с Milestone 0 или позже?
