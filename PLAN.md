# PLAN.md — три задачи: derive_more v2, Milestone 1, README

## Context

Swarm Coordination Runtime — слой distributed coordination для автономных групп дронов.
Milestone 0 завершён: workspace scaffold, базовые типы, Transport trait, детерминированные Clock/Scenario, smoke-пример.

Этот план охватывает три независимые задачи:

1. **Задача A** — миграция `derive_more` с `"0.99"` на `"2"`.
2. **Задача B** — README.md с описанием текущей стадии и инструкциями по запуску.
3. **Задача C** — Milestone 1: v0.1 Coverage With Failure — первый настоящий рабочий сценарий.

Порядок реализации: A → B → C.

## Investigation context

`INVESTIGATION.md` не обнаружен.

Ключевые факты из кодовой базы (Milestone 0):

- `derive_more = "0.99.20"` в `Cargo.lock`. AsMut используется в 5 местах: `AgentId`, `Capability` (agent.rs), `TaskId` (task.rs), `MessageId` (message.rs), `Tick` (clock.rs). Нигде не вызывается в прикладном коде — только дерайв.
- Для перехода на derive_more v2 **любой ценой** `AsMut` не сохраняем: в текущем коде он не используется, а mutable-доступ к внутренним строкам ID-newtype'ов не является обязательным контрактом Milestone 0/1.
- CLAUDE.md уже обновлён: правило newtype требует `AsRef, Deref, DerefMut, From, Into` (без AsMut).
- Stub-крейты `swarm-runtime`, `swarm-alloc`, `swarm-metrics`, `swarm-scenarios` — пустые (`// TODO`).
- `swarm-comms` содержит только `Transport` trait и `RawMessage`.
- `swarm-sim` содержит `Clock`, `Tick`, `Scenario::empty`.

## Affected components

### Задача A — derive_more v2

| Файл | Изменение |
|------|-----------|
| `Cargo.toml` | `derive_more = "0.99"` → `derive_more = { version = "2", features = ["as_ref", "deref", "deref_mut", "display", "from", "into"] }` |
| `crates/swarm-types/src/agent.rs` | Убрать `AsMut` из imports и derive (AgentId, Capability) |
| `crates/swarm-types/src/task.rs` | Убрать `AsMut` из imports и derive (TaskId) |
| `crates/swarm-types/src/message.rs` | Убрать `AsMut` из imports и derive (MessageId) |
| `crates/swarm-sim/src/clock.rs` | Убрать `AsMut` из imports и derive (Tick) |

### Задача B — README

| Файл | Изменение |
|------|-----------|
| `README.md` | Новый файл |

### Задача C — Milestone 1

| Крейт | Статус | Изменение |
|-------|--------|-----------|
| `Cargo.toml` | меняется | добавить `rand`, workspace paths для runtime/alloc/metrics/scenarios |
| `crates/swarm-comms` | меняется | добавить `network.rs` с `InMemNetwork` |
| `crates/swarm-runtime` | с нуля | `heartbeat`, `membership`, `failure`, `task_registry`, `coordinator`, `error` |
| `crates/swarm-alloc` | с нуля | `GreedyAllocator` |
| `crates/swarm-metrics` | с нуля | `RunMetrics`, `AggregateMetrics` |
| `crates/swarm-sim` | расширяется | добавить `runner.rs` с `ScenarioRunner` |
| `crates/swarm-scenarios` | с нуля | `CoverageWithFailure` scenario builder |
| `crates/swarm-examples` | расширяется | новый бинарник `coverage_with_failure` |

## Implementation steps

---

### Шаг A1 — Migrate derive_more → v2

**`Cargo.toml`** — заменить строку:

```toml
derive_more = { version = "2", features = ["as_ref", "deref", "deref_mut", "display", "from", "into"] }
```

**`crates/swarm-types/src/agent.rs`** — в imports и derive AgentId и Capability:

- Убрать `AsMut,` из `use derive_more::{...}`.
- Убрать `AsMut,` из `#[derive(...)]` у AgentId.
- Убрать `AsMut,` из `#[derive(...)]` у Capability.

**`crates/swarm-types/src/task.rs`** — аналогично для TaskId.

**`crates/swarm-types/src/message.rs`** — аналогично для MessageId.

**`crates/swarm-sim/src/clock.rs`** — аналогично для Tick.

Проверка: `cargo build --workspace` — должен собраться чисто. `cargo test --workspace` — 11 существующих тестов должны пройти.

---

### Шаг B1 — README.md

Файл: `README.md` (корень репозитория).

Содержание:
- Название проекта и одно-абзацное описание (Swarm Coordination Runtime).
- Текущий статус: Milestone 0 завершён, что реализовано.
- Структура крейтов: краткая таблица с назначением каждого.
- Секция **Build**: `cargo build`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`.
- Секция **Run examples**: `cargo run -p swarm-examples --bin empty_scenario`.
- Секция **Observe output**: объяснить, что выводит `empty_scenario` и что это означает.

---

### Шаг C1 — Workspace: добавить зависимости

**`Cargo.toml`** — в `[workspace.dependencies]` добавить:

```toml
swarm-runtime   = { path = "crates/swarm-runtime" }
swarm-alloc     = { path = "crates/swarm-alloc" }
swarm-metrics   = { path = "crates/swarm-metrics" }
swarm-scenarios = { path = "crates/swarm-scenarios" }
rand            = { version = "0.8", features = ["small_rng"] }
```

---

### Шаг C2 — swarm-comms: InMemNetwork

**`crates/swarm-comms/Cargo.toml`** — добавить:

```toml
rand = { workspace = true }
```

**`crates/swarm-comms/src/network.rs`** — новый файл.

Структуры:

```rust
pub struct NetworkConfig {
    pub packet_loss_rate: f64,  // 0.0 = нет потерь, 1.0 = 100% потерь
    pub latency_ticks: u64,     // константная задержка в тиках
    pub seed: u64,
}

/// key: `recipient AgentId`
pub struct InMemNetwork {
    /// value: `(delivery_tick, message)`
    in_flight: HashMap<AgentId, VecDeque<(u64, RawMessage)>>,
    config: NetworkConfig,
    rng: SmallRng,              // rand::rngs::SmallRng
    current_tick: u64,
}
```

API:

```rust
impl InMemNetwork {
    pub fn new(config: NetworkConfig) -> Self;
    /// Advance internal tick; messages with delivery_tick <= new tick become deliverable.
    pub fn advance_tick(&mut self);
    /// Return all deliverable messages for `recipient`.
    pub fn drain_ready(&mut self, recipient: &AgentId) -> Vec<RawMessage>;
    /// Total messages ever sent (including dropped), for metrics.
    pub fn messages_attempted(&self) -> u64;
    /// Total messages dropped due to packet loss.
    pub fn messages_dropped(&self) -> u64;
}
```

`InMemNetwork` также реализует `Transport`. В `send()`: бросить кубик (Bernoulli с `packet_loss_rate`), при потере — просто increment dropped counter и вернуть `Ok(())`. Иначе: добавить в `in_flight[msg.to]` с `delivery_tick = current_tick + latency_ticks`. В `poll()`: вернуть `None` (не используется в симуляции; ScenarioRunner использует `drain_ready`).

**`crates/swarm-comms/src/lib.rs`** — добавить:

```rust
pub mod network;
pub use network::{InMemNetwork, NetworkConfig};
```

---

### Шаг C3 — swarm-runtime

**`crates/swarm-runtime/Cargo.toml`**:

```toml
[package]
name    = "swarm-runtime"
version = "0.1.0"
edition = "2021"

[dependencies]
swarm-types = { workspace = true }
thiserror   = { workspace = true }
```

**Модульная структура:**

```
crates/swarm-runtime/src/
  error.rs         ← RuntimeError
  membership.rs    ← MembershipView, AgentEntry
  failure.rs       ← FailureDetector
  task_registry.rs ← TaskRegistry
  coordinator.rs   ← Coordinator, CoordinatorOutput
  lib.rs
```

**`error.rs`**:

```rust
#[derive(thiserror::Error, Debug)]
pub enum RuntimeError {
    #[error("task not found: {0:?}")]
    TaskNotFound(TaskId),
    #[error("invalid state transition from {from:?} to {to:?}")]
    InvalidTransition { from: TaskStatus, to: TaskStatus },
}
```

**`membership.rs`**:

```rust
pub struct AgentEntry {
    pub role: Role,
    pub health: Health,
    pub capabilities: Vec<Capability>,
    pub last_heartbeat_tick: u64,
}

/// key: `AgentId`
pub struct MembershipView {
    agents: HashMap<AgentId, AgentEntry>,
}

impl MembershipView {
    pub fn new(agents: Vec<Agent>) -> Self;
    pub fn record_heartbeat(&mut self, agent_id: &AgentId, tick: u64);
    pub fn mark_dead(&mut self, agent_id: &AgentId);
    pub fn alive_agents(&self) -> impl Iterator<Item = (&AgentId, &AgentEntry)>;
    pub fn get(&self, agent_id: &AgentId) -> Option<&AgentEntry>;
    pub fn is_alive(&self, agent_id: &AgentId) -> bool;
}
```

**`failure.rs`**:

```rust
pub struct FailureDetector {
    pub timeout_ticks: u64,
}

impl FailureDetector {
    pub fn new(timeout_ticks: u64) -> Self;
    /// Returns IDs of agents whose last heartbeat is older than timeout.
    pub fn detect(&self, view: &MembershipView, current_tick: u64) -> Vec<AgentId>;
}
```

**`task_registry.rs`**:

```rust
/// key: `TaskId`
pub struct TaskRegistry {
    tasks: HashMap<TaskId, Task>,
}

impl TaskRegistry {
    pub fn new(tasks: Vec<Task>) -> Self;
    pub fn assign(&mut self, task_id: &TaskId, agent_id: AgentId) -> Result<(), RuntimeError>;
    pub fn start(&mut self, task_id: &TaskId) -> Result<(), RuntimeError>;
    pub fn complete(&mut self, task_id: &TaskId) -> Result<(), RuntimeError>;
    pub fn fail_task(&mut self, task_id: &TaskId) -> Result<(), RuntimeError>;
    /// Release all tasks owned by agent; return released TaskIds.
    pub fn release_agent_tasks(&mut self, agent_id: &AgentId) -> Vec<TaskId>;
    pub fn unassigned(&self) -> Vec<&Task>;
    /// True when every task has an owner or is completed.
    pub fn all_assigned_or_completed(&self) -> bool;
}
```

Допустимые переходы TaskStatus:
- `Unassigned` → `Assigned` (assign)
- `Assigned` → `InProgress` (start)
- `InProgress` → `Completed` (complete)
- `InProgress` | `Assigned` → `Failed` (fail)
- `Assigned` | `InProgress` → `Unassigned` (release при смерти владельца)

**`coordinator.rs`**:

```rust
pub struct CoordinatorOutput {
    pub newly_failed: Vec<AgentId>,
    pub released_tasks: Vec<TaskId>,
}

pub struct Coordinator {
    pub membership: MembershipView,
    pub detector: FailureDetector,
    pub registry: TaskRegistry,
}

impl Coordinator {
    pub fn new(agents: Vec<Agent>, tasks: Vec<Task>, timeout_ticks: u64) -> Self;
    /// Process one tick: record heartbeats, detect failures, release tasks.
    /// `heartbeat_senders`: agents from whom heartbeats were received this tick.
    pub fn process_tick(
        &mut self,
        heartbeat_senders: Vec<AgentId>,
        current_tick: u64,
    ) -> CoordinatorOutput;
}
```

**`lib.rs`**:

```rust
pub mod coordinator;
pub mod error;
pub mod failure;
pub mod membership;
pub mod task_registry;

pub use coordinator::{Coordinator, CoordinatorOutput};
pub use error::RuntimeError;
pub use failure::FailureDetector;
pub use membership::{AgentEntry, MembershipView};
pub use task_registry::TaskRegistry;
```

---

### Шаг C4 — swarm-alloc: GreedyAllocator

**`crates/swarm-alloc/Cargo.toml`**:

```toml
[package]
name    = "swarm-alloc"
version = "0.1.0"
edition = "2021"

[dependencies]
swarm-types = { workspace = true }
```

**`crates/swarm-alloc/src/lib.rs`**:

```rust
pub mod allocator;
pub use allocator::{Allocator, GreedyAllocator};
```

**`crates/swarm-alloc/src/allocator.rs`**:

```rust
/// Allocates unassigned tasks to available alive agents.
///
/// value: `(task_id, agent_id)` — assignment decisions
pub trait Allocator {
    fn allocate(&self, tasks: &[&Task], agents: &[&AgentId]) -> Vec<(TaskId, AgentId)>;
}

/// Assigns each task to the next available agent in round-robin order.
pub struct GreedyAllocator;

impl Allocator for GreedyAllocator {
    fn allocate(&self, tasks: &[&Task], agents: &[&AgentId]) -> Vec<(TaskId, AgentId)>;
}
```

Логика `GreedyAllocator::allocate`: для каждого task (по убыванию priority), взять следующего агента по очереди. Если агентов нет — вернуть пустой Vec.

---

### Шаг C5 — swarm-metrics

**`crates/swarm-metrics/Cargo.toml`**:

```toml
[package]
name    = "swarm-metrics"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
```

**`crates/swarm-metrics/src/lib.rs`**:

```rust
pub mod metrics;
pub use metrics::{AggregateMetrics, RunMetrics};
```

**`crates/swarm-metrics/src/metrics.rs`**:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetrics {
    pub seed: u64,
    pub total_ticks: u64,
    pub messages_attempted: u64,
    pub messages_dropped: u64,
    /// Ticks from agent death to first detection by FailureDetector. None if no failure.
    pub detection_time_ticks: Option<u64>,
    /// Ticks from detection to all released tasks re-assigned. None if no failure.
    pub reallocation_time_ticks: Option<u64>,
    /// Maximum observed duration for any task staying unassigned.
    pub max_task_unassigned_ticks: u64,
    /// True when every task is assigned or completed at the end of the run.
    pub all_tasks_assigned: bool,
    /// v0.1 success criterion: all tasks assigned and no task exceeded max_unassigned_ticks.
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateMetrics {
    pub total_runs: u64,
    /// Fraction of runs where success == true.
    pub success_rate: f64,
    pub avg_detection_ticks: f64,
    pub avg_reallocation_ticks: f64,
    pub avg_messages_attempted: f64,
    pub avg_messages_dropped: f64,
}

impl AggregateMetrics {
    pub fn from_runs(runs: &[RunMetrics]) -> Self;
}

impl std::fmt::Display for AggregateMetrics { ... }
```

---

### Шаг C6 — swarm-sim: ScenarioRunner

**`crates/swarm-sim/Cargo.toml`** — добавить в dependencies:

```toml
swarm-runtime  = { workspace = true }
swarm-alloc    = { workspace = true }
swarm-comms    = { workspace = true }
swarm-metrics  = { workspace = true }
```

**`crates/swarm-sim/src/runner.rs`** — новый файл.

```rust
pub struct FailureEvent {
    pub agent_id: AgentId,
    /// Tick at which the agent stops sending heartbeats (simulates crash).
    pub at_tick: u64,
}

pub struct RunConfig {
    /// Maximum number of ticks before the run is forcefully stopped.
    pub max_ticks: u64,
    /// Ticks without heartbeat before FailureDetector marks agent Dead.
    pub timeout_ticks: u64,
    /// Maximum allowed duration for any task to remain Unassigned.
    pub max_unassigned_ticks: u64,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub failures: Vec<FailureEvent>,
}
```

Логика `ScenarioRunner::run(scenario: &Scenario, config: RunConfig) -> RunMetrics`:

1. Создать `InMemNetwork::new(NetworkConfig { packet_loss_rate, latency_ticks, seed: scenario.seed })`.
2. Создать `Coordinator::new(agents.clone(), tasks.clone(), timeout_ticks)`.
3. Создать `GreedyAllocator`.
4. Создать `Clock::new(1)`.
5. До цикла выполнить initial allocation pass:
   - взять `coordinator.registry.unassigned()`;
   - взять живых агентов из membership;
   - назначить все возможные задачи через `GreedyAllocator`;
   - это фиксирует стартовый invariant: при наличии живых агентов coverage tasks не остаются unassigned без причины.
6. Вести `HashMap<TaskId, u64>` с количеством тиков, проведённых задачей в `Unassigned`.
7. Вести `HashSet<AgentId>` `crashed_agents` как ground-truth состояние симулятора:
   - это не runtime membership view;
   - crash означает только "агент перестал отправлять heartbeat";
   - `MembershipView` остаётся `Alive`, пока `FailureDetector` сам не обнаружит timeout.
8. Цикл по тикам (`0..max_ticks`):
   a. `clock.advance()`.
   b. Для каждого `FailureEvent` с `at_tick == current_tick`: добавить агента в `crashed_agents`.
   c. Собрать агентов, которые считаются alive в membership view и **не входят** в `crashed_agents`: только они посылают heartbeat через `network.send(RawMessage { from: agent_id, to: coordinator_id, payload: agent_id_bytes })`.
   d. `network.advance_tick()`.
   e. Получить heartbeat-сообщения из сети: `network.drain_ready(&coordinator_id)`. Извлечь `AgentId` из payload каждого сообщения.
   f. `coordinator.process_tick(heartbeat_senders, current_tick)`:
      - обновляет heartbeat ticks для полученных heartbeats;
      - вызывает `FailureDetector`;
      - только для `newly_failed` делает `membership.mark_dead()`;
      - только после этого вызывает `registry.release_agent_tasks()`.
   g. Обновить unassigned-duration counters для всех задач в состоянии `Unassigned`; сохранить максимум в `RunMetrics.max_task_unassigned_ticks`.
   h. Если `output.released_tasks` непустые или в registry есть unassigned tasks:
      - Записать `detection_tick` (если `output.newly_failed` непустой и detection ещё не записан);
      - Вызвать `allocator.allocate(unassigned_tasks, alive_agents)`.
      - Применить назначения через `coordinator.registry.assign(...)`.
      - Если все released_tasks назначены — записать `reallocation_tick`.
   i. Если после failure/reallocation все задачи assigned/completed и `max_task_unassigned_ticks <= config.max_unassigned_ticks`: можно завершить прогон досрочно.
9. Вернуть `RunMetrics`:
   - `all_tasks_assigned = coordinator.registry.all_assigned_or_completed()`;
   - `success = all_tasks_assigned && max_task_unassigned_ticks <= config.max_unassigned_ticks`.

`coordinator_id` = `AgentId::from("coordinator")` — специальный ID, не является реальным агентом.

**`crates/swarm-sim/src/lib.rs`** — добавить:

```rust
pub mod runner;
pub use runner::{FailureEvent, RunConfig, ScenarioRunner};
```

---

### Шаг C7 — swarm-scenarios: CoverageWithFailure

**`crates/swarm-scenarios/Cargo.toml`**:

```toml
[package]
name    = "swarm-scenarios"
version = "0.1.0"
edition = "2021"

[dependencies]
swarm-types = { workspace = true }
swarm-sim   = { workspace = true }
rand        = { workspace = true }
```

**`crates/swarm-scenarios/src/lib.rs`**:

```rust
pub mod coverage;
pub use coverage::{build_coverage_scenario, CoverageConfig};
```

**`crates/swarm-scenarios/src/coverage.rs`**:

```rust
pub struct CoverageConfig {
    pub seed: u64,
    pub agent_count: usize,    // 5..=20
    pub task_count: usize,     // >= agent_count
    pub failure_tick: u64,     // tick at which one agent dies
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub timeout_ticks: u64,
    /// Maximum allowed duration for any task to remain Unassigned.
    pub max_unassigned_ticks: u64,
    pub max_ticks: u64,
}

/// Build a Scenario and RunConfig for the CoverageWithFailure scenario.
///
/// value: `(scenario, run_config)`
pub fn build_coverage_scenario(config: &CoverageConfig) -> (Scenario, RunConfig);
```

Логика `build_coverage_scenario`:
- Создать агентов с `Role::Scout`, `Health::Alive`, `Pose { x: 0.0, y: 0.0 }`, `capabilities: []`, `current_task: None`.
- AgentId: `"agent-{i}"` для i в `0..agent_count`.
- Задачи: `Task { id: "task-{i}", status: Unassigned, assigned_to: None, priority: 1 }`.
- Изначально распределить задачи между агентами (assign по одной на агента, остальные остаются Unassigned).
- FailureEvent: первый агент (`"agent-0"`) умирает в `failure_tick`.
- `RunConfig.max_unassigned_ticks = CoverageConfig.max_unassigned_ticks`.
- `RunConfig.packet_loss_rate`, `latency_ticks`, `timeout_ticks`, `max_ticks` и `failures` заполняются из `CoverageConfig`.

---

### Шаг C8 — swarm-examples: coverage_with_failure binary

**`crates/swarm-examples/Cargo.toml`** — добавить:

```toml
swarm-scenarios = { workspace = true }
swarm-metrics   = { workspace = true }

[[bin]]
name = "coverage_with_failure"
path = "src/bin/coverage_with_failure.rs"
```

**`crates/swarm-examples/src/bin/coverage_with_failure.rs`**:

Логика:
1. Создать 1000 конфигураций с `seed` от 0 до 999.
2. Для каждого seed:
   - `CoverageConfig { seed, agent_count: 10, task_count: 15, failure_tick: 5, packet_loss_rate: 0.1, latency_ticks: 1, timeout_ticks: 3, max_unassigned_ticks: 5, max_ticks: 200 }`.
   - `let (scenario, run_config) = build_coverage_scenario(&config)`.
   - `ScenarioRunner::run(&scenario, run_config)`.
3. `AggregateMetrics::from_runs(&runs)`.
4. Вывести агрегированные метрики через `println!("{metrics}")`.
5. Если `success_rate < 0.99` — выйти с кодом 1, иначе с кодом 0.

---

### Шаг C9 — Финальная верификация и коммит

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p swarm-examples --bin empty_scenario
cargo run -p swarm-examples --bin coverage_with_failure
git add Cargo.toml Cargo.lock crates/ README.md
git commit -m "feat: migrate derive_more to v2, add README, Milestone 1 Coverage With Failure"
```

## Testing strategy

### Категория 1 — без рефакторинга (реализуются вместе с кодом)

**Задача A: derive_more v2**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| A.1 | `derive_more_v2_newtypes_compile` | swarm-types | Существующие тесты `agent_id_newtype_roundtrip`, `task_id_newtype_roundtrip`, `message_id_newtype_roundtrip` проходят после миграции — неявная проверка компиляции |
| A.2 | `tick_newtype_still_works` | swarm-sim | Существующие тесты clock проходят — Tick без AsMut компилируется |

**Задача C: InMemNetwork**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| C.1 | `inmem_send_recv_no_loss` | swarm-comms | `send()`, `advance_tick()`, `drain_ready()` → сообщение доставлено |
| C.2 | `inmem_packet_loss_100pct` | swarm-comms | `packet_loss_rate=1.0` → ни одно сообщение не доставлено |
| C.3 | `inmem_latency_delays_delivery` | swarm-comms | `latency_ticks=2` → до `advance_tick()` × 2 сообщение не доставляется |
| C.4 | `inmem_deterministic_seed` | swarm-comms | Два InMemNetwork с одним seed и `loss_rate=0.5` дают одинаковый результат |
| C.5 | `inmem_message_counters` | swarm-comms | `messages_attempted()` и `messages_dropped()` корректны |

**Задача C: MembershipView**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| C.6 | `membership_record_heartbeat` | swarm-runtime | `record_heartbeat()` → `last_heartbeat_tick` обновляется |
| C.7 | `membership_mark_dead` | swarm-runtime | `mark_dead()` → `is_alive()` возвращает false |
| C.8 | `membership_alive_iter_excludes_dead` | swarm-runtime | `alive_agents()` не возвращает мёртвых |

**Задача C: FailureDetector**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| C.9 | `detector_no_timeout_with_recent_hb` | swarm-runtime | Агент с heartbeat на tick 5, проверка на tick 7 при timeout=3 → не обнаружен |
| C.10 | `detector_timeout_after_missed_hbs` | swarm-runtime | Агент с heartbeat на tick 0, проверка на tick 4 при timeout=3 → обнаружен |

**Задача C: TaskRegistry**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| C.11 | `registry_assign_unassigned` | swarm-runtime | `Unassigned` → `Assigned` успешно |
| C.12 | `registry_assign_already_assigned_fails` | swarm-runtime | Повторный assign на занятую задачу → `Err(InvalidTransition)` |
| C.13 | `registry_start_assigned_task` | swarm-runtime | `Assigned` → `InProgress` через `start()` |
| C.14 | `registry_release_agent_tasks` | swarm-runtime | Задачи агента → `Unassigned` после `release_agent_tasks()` |
| C.15 | `registry_all_assigned_or_completed` | swarm-runtime | true только когда нет задач `Unassigned` без владельца |

**Задача C: GreedyAllocator**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| C.16 | `greedy_assigns_to_alive_agents` | swarm-alloc | 3 задачи, 3 агента → все назначены |
| C.17 | `greedy_no_agents_returns_empty` | swarm-alloc | Нет агентов → пустой Vec |
| C.18 | `greedy_more_tasks_than_agents` | swarm-alloc | 5 задач, 2 агента → 2 назначения (round-robin) |

**Задача C: MetricsCollector**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| C.19 | `aggregate_success_rate` | swarm-metrics | 8 из 10 runs `success=true` → `success_rate=0.8` |
| C.20 | `aggregate_avg_detection` | swarm-metrics | Среднее `detection_time_ticks` вычисляется верно |

### Категория 2 — лёгкий рефакторинг (после реализации C)

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| C.21 | `runner_timeout_semantics_before_after_detection` | swarm-sim | После crash агент перестаёт слать heartbeat; до timeout он не `newly_failed`, после timeout появляется в `newly_failed`, задачи release → reallocate |
| C.22 | `runner_failure_triggers_reallocation` | swarm-sim | End-to-end: агент умирает, задачи перераспределяются за ≤ `timeout_ticks + latency_ticks + 1` тиков после последнего heartbeat |
| C.23 | `runner_deterministic_same_seed` | swarm-sim | Два запуска с одним seed → идентичные RunMetrics |
| C.24 | `runner_no_failure_assigns_all_tasks` | swarm-sim | Без failure_events все задачи назначаются живым агентам |

### Категория 3 — тяжёлый рефакторинг (будущие milestone)

| # | Тест | Описание |
|---|------|----------|
| C.25 | `coverage_1000_seeds_stress` | Тест из 1000 seed с assertion на success_rate >= 0.99; требует ~секунды |
| C.26 | Property-based тесты TaskRegistry state machine | Нужен `proptest`; добавить в Milestone 2 |
| C.27 | Multiple simultaneous failures | Несколько агентов умирают одновременно; требует расширения `FailureEvent` |

### Gap-анализ

- **Нет теста на отказ в записи метрик при dropped heartbeats**: сложно проверить детерминированно без контроля RNG seed в тесте — частично покрывается C.4 и C.21.
- **Нет E2E теста 1000 сценариев в unit-тестах**: такой тест слишком долгий для `cargo test`; вместо него — smoke через `coverage_with_failure` бинарник.

## Risks and tradeoffs

### Что могло сломаться

| Риск | Вероятность | Митигация |
|------|-------------|-----------|
| derive_more v2 feature-флаги не включают нужный derive | Низкая | План минимизирует derive-набор, удаляет `AsMut`; тест A.1/A.2 и `cargo build` поймают несовместимость |
| `rand = { version = "0.8", features = ["small_rng"] }` несовместим с другими зависимостями | Низкая | 0.8 — стабильная ветка; `cargo build` сразу покажет; feature `small_rng` нужна для `rand::rngs::SmallRng` |
| Circular dependency: swarm-sim → swarm-runtime → swarm-alloc | Нет | Граф ацикличен: types ← comms ← runtime ← alloc ← metrics ← sim ← scenarios ← examples |
| При 100% packet loss failure detection может запаздывать или не дать успешный run | Высокая | `max_ticks` ограничивает прогон; `success=false` и `success_rate` в метриках покажут проблему |
| `coordinator_id = AgentId("coordinator")` конфликтует с реальным агентом | Низкая | В `build_coverage_scenario` агентов называть `"agent-{i}"` — нет пересечения |

### Tradeoffs

- **ScenarioRunner в swarm-sim (не в swarm-scenarios)** — runner — общая инфраструктура; конкретные сценарии строятся поверх него в swarm-scenarios. Это позволяет добавлять новые сценарии без изменения runner.
- **GreedyAllocator без учёта Capability** — на Milestone 1 задачи не имеют capability-требований; добавим в Milestone 2 вместе с capability-aware allocation.
- **InMemNetwork.poll() возвращает None** — ScenarioRunner управляет тиками явно и использует `drain_ready()`. `Transport::poll()` оставляется для будущих реальных транспортов.
- **`success_rate >= 0.99` как критерий** — при packet_loss=0.1 и timeout=3 тика теоретически возможны редкие сценарии с неудачей; threshold 0.99 даёт допуск на 1% таких случаев.

## Open questions

1. **Где хранить `InMemNetwork.current_tick`?** В самой сети или передавать извне? Plan предлагает хранить внутри с `advance_tick()` для инкапсуляции.
2. **`GreedyAllocator` с round-robin или sorted-by-priority?** План предлагает сортировку по убыванию priority; если вес задач одинаков — порядок назначения любой.
3. **`display` feature derive_more**: нужен только для `AgentId`, `TaskId` и др. newtype в `agent.rs`, `task.rs`, `message.rs`. `Tick` его не имеет — нет `Display` derive у Tick, что нормально.
4. **Метрики: `Display` для `AggregateMetrics`** — вручную через `impl fmt::Display` (не через derive_more), чтобы контролировать формат вывода.
5. **README на каком языке?** Если проект планируется публичным — английский README; если internal — русский. Рекомендую: English README с кратким блоком на русском для команды.
