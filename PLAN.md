# PLAN.md — три задачи: derive_more v2 · Milestone 1 · README

## Context

Swarm Coordination Runtime — слой distributed coordination для автономных групп дронов,
проверяемый через Mission Digital Twin / Scenario Test Harness.

Milestone 0 завершён (`ac7179d`): workspace scaffold, базовые типы, Transport trait,
детерминированные Clock/Scenario, smoke-пример.

Этот план охватывает три задачи, реализованные в `72ba39a`:

1. **Задача A** — миграция `derive_more 0.99` → `2` (убрать `AsMut`, добавить feature-флаги).
2. **Задача B** — `README.md` с описанием текущей стадии и инструкциями по запуску.
3. **Задача C** — Milestone 1: первый рабочий сценарий Coverage With Failure.

## Investigation context

`INVESTIGATION.md` не обнаружен.

Ключевые факты:

- `derive_more "0.99"` используется в 5 местах: `AgentId`, `Capability`, `TaskId`, `MessageId`, `Tick`.
  `AsMut` нигде не вызывается в логике — только дерайв. В v2 `AsMut` отсутствует; остальные derive
  доступны через явные feature-флаги.
- Stub-крейты `swarm-runtime`, `swarm-alloc`, `swarm-metrics`, `swarm-scenarios` готовы к наполнению.
- `swarm-comms` содержит только `Transport` trait и `RawMessage`.
- `swarm-sim` содержит `Clock`, `Tick`, `Scenario::empty`.

## Affected components

### Задача A — derive_more v2

| Файл | Изменение |
|------|-----------|
| `Cargo.toml` | `derive_more = { version = "2", features = ["as_ref", "deref", "deref_mut", "display", "from", "into"] }` |
| `crates/swarm-types/src/agent.rs` | Убрать `AsMut` (AgentId, Capability) |
| `crates/swarm-types/src/task.rs` | Убрать `AsMut` (TaskId) |
| `crates/swarm-types/src/message.rs` | Убрать `AsMut` (MessageId) |
| `crates/swarm-sim/src/clock.rs` | Убрать `AsMut` (Tick) |

### Задача B — README

| Файл | Изменение |
|------|-----------|
| `README.md` | Новый файл |

### Задача C — Milestone 1

| Крейт | Изменение |
|-------|-----------|
| `Cargo.toml` | `rand = "0.8"`, workspace paths для runtime/alloc/metrics/scenarios |
| `crates/swarm-comms` | Новый `network.rs`: `InMemNetwork`, `NetworkConfig` |
| `crates/swarm-runtime` | С нуля: `coordinator`, `error`, `failure`, `membership`, `task_registry` |
| `crates/swarm-alloc` | С нуля: `allocator.rs`, `GreedyAllocator` |
| `crates/swarm-metrics` | С нуля: `metrics.rs`, `RunMetrics`, `AggregateMetrics` |
| `crates/swarm-sim` | Новый `runner.rs`: `ScenarioRunner`, `RunConfig`, `FailureEvent` |
| `crates/swarm-scenarios` | С нуля: `coverage.rs`, `CoverageConfig`, `build_coverage_scenario` |
| `crates/swarm-examples` | Новый бинарник `coverage_with_failure` |

## Implementation steps

### Шаг A1 — Migrate derive_more → v2

В `Cargo.toml` заменить:
```toml
derive_more = { version = "2", features = ["as_ref", "deref", "deref_mut", "display", "from", "into"] }
```

В `agent.rs`, `task.rs`, `message.rs`, `clock.rs` — убрать `AsMut` из `use` и `#[derive(...)]`.

### Шаг B1 — README.md

Содержание: описание проекта, текущий статус, таблица крейтов, секции Build / Run examples / Observe output.

### Шаг C1 — Workspace: добавить зависимости

В `[workspace.dependencies]`:
```toml
swarm-runtime   = { path = "crates/swarm-runtime" }
swarm-alloc     = { path = "crates/swarm-alloc" }
swarm-metrics   = { path = "crates/swarm-metrics" }
swarm-scenarios = { path = "crates/swarm-scenarios" }
rand            = { version = "0.8", features = ["small_rng"] }
```

### Шаг C2 — swarm-comms: InMemNetwork

Файл `crates/swarm-comms/src/network.rs`.

```rust
pub struct NetworkConfig {
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub seed: u64,
}

pub struct InMemNetwork {
    /// key: `recipient AgentId`; value: `(delivery_tick, message)`
    in_flight: HashMap<AgentId, VecDeque<(u64, RawMessage)>>,
    config: NetworkConfig,
    rng: SmallRng,
    current_tick: u64,
    messages_attempted: u64,
    messages_dropped: u64,
}
```

`InMemNetwork::send()`: Bernoulli-бросок по `packet_loss_rate`. При потере — инкремент dropped, `Ok(())`.
Иначе: добавить в `in_flight[to]` с `delivery_tick = current_tick + latency_ticks`.
`advance_tick()`: инкремент `current_tick`.
`drain_ready(recipient)`: вернуть все сообщения с `delivery_tick <= current_tick`.

### Шаг C3 — swarm-runtime

Модули: `error`, `membership`, `failure`, `task_registry`, `coordinator`.

**MembershipView**: хранит `HashMap<AgentId, AgentEntry>` (`role`, `health`, `last_heartbeat_tick`).
Методы: `record_heartbeat`, `mark_dead`, `alive_agents`, `is_alive`.

**FailureDetector**: `timeout_ticks: u64`.
`detect(view, current_tick) -> Vec<AgentId>` — агенты, чей `last_heartbeat_tick + timeout < current_tick`.

**TaskRegistry**: `HashMap<TaskId, Task>`.
`assign`, `start`, `complete`, `fail_task`, `release_agent_tasks` (возвращает `Vec<TaskId>`), `unassigned`, `all_assigned_or_completed`.
Допустимые переходы: `Unassigned→Assigned`, `Assigned→InProgress`, `InProgress→Completed/Failed`,
`Assigned/InProgress→Unassigned` (при release).

**Coordinator**: объединяет `MembershipView` + `FailureDetector` + `TaskRegistry`.
`process_tick(heartbeat_senders: Vec<AgentId>, tick: u64) -> CoordinatorOutput`.
`CoordinatorOutput { newly_failed: Vec<AgentId>, released_tasks: Vec<TaskId> }`.

### Шаг C4 — swarm-alloc: GreedyAllocator

Trait `Allocator::allocate(tasks: &[&Task], agents: &[&AgentId]) -> Vec<(TaskId, AgentId)>`.
`GreedyAllocator`: назначает задачи агентам round-robin по убыванию priority.

### Шаг C5 — swarm-metrics

`RunMetrics`: `seed`, `total_ticks`, `messages_attempted`, `messages_dropped`,
`detection_time_ticks: Option<u64>`, `reallocation_time_ticks: Option<u64>`, `all_tasks_completed`.

`AggregateMetrics::from_runs(runs: &[RunMetrics]) -> Self`:
`success_rate`, `avg_detection_ticks`, `avg_reallocation_ticks`, `avg_messages_attempted`, `avg_messages_dropped`.

### Шаг C6 — swarm-sim: ScenarioRunner

Файл `crates/swarm-sim/src/runner.rs`.

```rust
pub struct RunConfig {
    pub max_ticks: u64,
    pub timeout_ticks: u64,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub failures: Vec<FailureEvent>,
}
```

Цикл `ScenarioRunner::run(scenario, config) -> RunMetrics`:
1. Создать `InMemNetwork`, `Coordinator`, `GreedyAllocator`.
2. На каждом тике:
   a. Инжектировать failure (mark_dead в membership).
   b. Живые агенты посылают heartbeat через сеть (`coordinator` как получатель).
   c. `network.advance_tick()`.
   d. `network.drain_ready("coordinator")` → `coordinator.process_tick(senders, tick)`.
   e. При `released_tasks` → `allocator.allocate` → применить назначения.
   f. Записать detection/reallocation tick.
3. Выйти досрочно при `all_assigned_or_completed()`.

### Шаг C7 — swarm-scenarios: CoverageWithFailure

`build_coverage_scenario(config: &CoverageConfig) -> (Scenario, RunConfig)`.
Конфиг: `seed`, `agent_count` (5–20), `task_count`, `failure_tick`, `packet_loss_rate`, `latency_ticks`,
`timeout_ticks`, `max_ticks`.
Агенты: `"agent-{i}"`, Role::Scout, Health::Alive. FailureEvent: `agent-0` умирает в `failure_tick`.

### Шаг C8 — swarm-examples: coverage_with_failure

1000 прогонов с seed 0–999; `CoverageConfig { agent_count: 10, task_count: 15, failure_tick: 5,
packet_loss_rate: 0.1, latency_ticks: 1, timeout_ticks: 3, max_ticks: 200 }`.
Вывод `AggregateMetrics`. Выход с кодом 1 если `success_rate < 0.99`.

### Шаг C9 — Верификация и коммит

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

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| A.1 | `agent_id_newtype_roundtrip` (существующий) | swarm-types | Подтверждает компиляцию без AsMut |
| A.2 | `clock_starts_at_zero` (существующий) | swarm-sim | Tick без AsMut компилируется |
| C.1 | `inmem_send_recv_no_loss` | swarm-comms | send + drain_ready без задержки и потерь |
| C.2 | `inmem_packet_loss_100pct` | swarm-comms | 100% потерь → ничего не доставляется |
| C.3 | `inmem_latency_delays_delivery` | swarm-comms | latency=2 → до advance×2 сообщение не готово |
| C.4 | `inmem_deterministic_seed` | swarm-comms | Одинаковый seed → идентичные drop-решения |
| C.5 | `inmem_message_counters` | swarm-comms | `messages_attempted` и `messages_dropped` корректны |
| C.6 | `membership_record_heartbeat` | swarm-runtime | `last_heartbeat_tick` обновляется |
| C.7 | `membership_mark_dead` | swarm-runtime | `is_alive()` возвращает false после mark_dead |
| C.8 | `membership_alive_iter_excludes_dead` | swarm-runtime | `alive_agents()` не содержит мёртвых |
| C.9 | `detector_no_timeout_with_recent_hb` | swarm-runtime | Недавний heartbeat — не детектируется |
| C.10 | `detector_timeout_after_missed_hbs` | swarm-runtime | Устаревший heartbeat — детектируется |
| C.11 | `registry_assign_unassigned` | swarm-runtime | Unassigned→Assigned успешно |
| C.12 | `registry_assign_already_assigned_fails` | swarm-runtime | Повторный assign → Err(InvalidTransition) |
| C.13 | `registry_release_agent_tasks` | swarm-runtime | Задачи агента возвращаются в Unassigned |
| C.14 | `registry_all_assigned_or_completed` | swarm-runtime | true только когда все задачи выполнены/назначены |
| C.15 | `greedy_assigns_to_alive_agents` | swarm-alloc | 3 задачи, 3 агента → все назначены |
| C.16 | `greedy_no_agents_returns_empty` | swarm-alloc | Нет агентов → пустой Vec |
| C.17 | `greedy_more_tasks_than_agents` | swarm-alloc | 5 задач, 2 агента → 2 назначения |
| C.18 | `aggregate_success_rate` | swarm-metrics | 8/10 runs completed → rate=0.8 |
| C.19 | `aggregate_avg_detection` | swarm-metrics | Среднее detection_time вычисляется верно |

### Категория 2 — лёгкий рефакторинг

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| C.20 | `runner_failure_triggers_reallocation` | swarm-sim | Агент умирает → задачи переназначены |
| C.21 | `runner_deterministic_same_seed` | swarm-sim | Два запуска с одним seed → идентичные RunMetrics |
| C.22 | `runner_no_failure_assigns_all_tasks` | swarm-sim | Без failure все задачи завершаются |
| C.23 | `runner_timeout_semantics_before_after_detection` | swarm-sim | Проверка точного тика детектирования |

### Категория 3 — тяжёлый рефакторинг (будущие milestone)

| # | Тест | Описание |
|---|------|----------|
| C.24 | Property-based тесты TaskRegistry | Нужен `proptest`; добавить в Milestone 2 |
| C.25 | Одновременный отказ нескольких агентов | Расширение FailureEvent |
| C.26 | Capability-aware allocation | После добавления capability-требований к Task |

### Gap-анализ

- Нет теста на корректность поведения при `packet_loss=100%` в полном сценарии (detection не происходит
  никогда). Частично покрывается C.22 (без потерь) и наблюдением success_rate в 1000-run smoke-тесте.
- 1000-run stress-тест намеренно не включён в `cargo test` (слишком долгий); живёт в binary `coverage_with_failure`.

## Risks and tradeoffs

### Что могло сломаться

| Риск | Вероятность | Как проверить |
|------|-------------|---------------|
| derive_more v2 feature-флаги не включают нужный derive | Проверено | `cargo build` + 11 существующих тестов |
| Circular dependency swarm-sim → swarm-runtime → ... | Нет | Граф: types←comms←runtime, alloc, metrics←sim←scenarios←examples |
| `all_assigned_or_completed()` не достигается при высоком packet loss | Покрыт `max_ticks` | `success_rate` в aggregate metrics покажет проблему |
| `"coordinator"` AgentId конфликтует с реальным агентом | Низкая | Агенты называются `"agent-{i}"` |

### Tradeoffs

- **`AsMut` убран из newtypes** — не использовался в логике; v2 не предоставляет его derive.
  Стандартный `impl AsMut<String>` при необходимости добавляется вручную.
- **ScenarioRunner в swarm-sim, не в swarm-scenarios** — runner — общая инфраструктура;
  конкретные сценарии строятся поверх в swarm-scenarios.
- **GreedyAllocator без capability** — tasks не имеют capability-требований на Milestone 1;
  capability-aware allocation добавим в Milestone 2.
- **`Transport::poll()` возвращает None в InMemNetwork** — ScenarioRunner управляет тиками
  явно через `drain_ready()`. `poll()` оставлен для будущих реальных транспортов.

## Open questions

1. **Capability-aware allocation**: добавить `required_capabilities: Vec<Capability>` к `Task`
   и матчить с `agent.capabilities` в Milestone 2?
2. **Multiple simultaneous failures**: `FailureEvent` сейчас поддерживает несколько событий,
   но сценарий Coverage создаёт только один. Тест с двумя одновременными отказами?
3. **Метрики в реальном времени**: добавить `tracing`-интеграцию или оставить только
   post-run aggregate?
