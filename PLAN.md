# PLAN.md — Milestone 2: v0.2, Dynamic Tasks + Auction

## Context

Swarm Coordination Runtime. Milestone 1 завершён (`72ba39a`): 29 тестов, 1000 сценариев,
100% success rate. Реализованы: InMemNetwork, heartbeat, membership, failure detection,
TaskRegistry, GreedyAllocator, ScenarioRunner, metrics.

Milestone 2 добавляет реалистичную задачу распределения:

- задачи появляются во время миссии (динамическая инжекция);
- задачи истекают (expiration);
- agent capability matching как жёсткое ограничение;
- auction-based allocation с cost function (расстояние / battery / role / capability);
- сравнение greedy vs auction;
- инварианты против duplicate ownership;
- обновление README.

По roadmap из DRONE_A.1.md: v0.2 = "dynamic tasks, simple auction, priorities, task expiration,
agent capability matching". DRONE_B.1.md подтверждает auction (CBBA-like) как Фазу 2 coordination.

## Investigation context

`INVESTIGATION.md` не обнаружен.

**Выводы из DRONE_A.1.md / DRONE_B.1.md, влияющие на дизайн:**

- Дрон — ресурс с capabilities, battery, pose, role. Cost function должна учитывать все четыре.
- CBBA/auction работают при неполной связности. В симуляции с InMemNetwork auction
  реализуется централизованно (ScenarioRunner как арбитр), но интерфейс оставляется
  pluggable для будущего децентрализованного CBBA.
- "Жёсткое правило: каждая фаза заканчивается чем-то запускаемым." — нужен
  бинарник с наблюдаемыми метриками.
- DRONE_A.1.md: `duplicate task ownership count` и `conflicting assignments` — явные метрики.
- Battery модель пока статическая (нет кинематики); drain появится в v0.3+.

**Текущее состояние кода:**

| Файл | Что меняется в Milestone 2 |
|------|---------------------------|
| `swarm-types/src/task.rs` | Добавить 4 новых поля в `Task` |
| `swarm-types/src/agent.rs` | Добавить `battery: f64` в `Agent` |
| `swarm-alloc/src/allocator.rs` | Перепроектировать `Allocator` trait, добавить `AuctionAllocator` |
| `swarm-runtime/src/coordinator.rs` | `process_tick` + dynamic injection + expiry |
| `swarm-runtime/src/task_registry.rs` | Поддержка expiration |
| `swarm-runtime/src/membership.rs` | `AgentEntry` + `battery` + `pose` |
| `swarm-sim/src/runner.rs` | Обобщить по `Allocator`, dynamic tasks, expiry |
| `swarm-metrics/src/metrics.rs` | Добавить 2 поля в `RunMetrics` и `AggregateMetrics` |
| `swarm-scenarios/src/coverage.rs` | Обновить под новый `Task`/`Agent` |
| `swarm-scenarios/src/` | Новый модуль `auction.rs` |
| `swarm-examples/src/bin/` | Новый бинарник `dynamic_auction.rs` |
| `README.md` | Обновить текущий статус |

## Affected components

| Крейт | Тип изменения |
|-------|--------------|
| `swarm-types` | Breaking: новые поля в `Task` и `Agent` |
| `swarm-alloc` | Breaking: новая сигнатура `Allocator`, новый `AuctionAllocator` |
| `swarm-runtime` | Breaking: `process_tick` signature, `CoordinatorOutput`, `AgentEntry` |
| `swarm-metrics` | Breaking: новые поля `RunMetrics` / `AggregateMetrics` |
| `swarm-sim` | Breaking: generic `ScenarioRunner`, новые поля `RunConfig` |
| `swarm-scenarios` | Обновление + новый модуль |
| `swarm-examples` | Новый бинарник |
| `README.md` | Обновление |

## Implementation steps

---

### Шаг 1 — swarm-types: расширить `Task` и `Agent`

**`crates/swarm-types/src/task.rs`**

Добавить в `Task`:

```rust
pub struct Task {
    pub id: TaskId,
    pub status: TaskStatus,
    pub assigned_to: Option<AgentId>,
    pub priority: u8,
    // New in Milestone 2:
    /// Hard constraint: agent must hold all listed capabilities to be assigned this task.
    pub required_capabilities: Vec<Capability>,
    /// Soft constraint: agent matching this role gets a cost bonus.
    pub preferred_role: Option<Role>,
    /// Task expires (is removed) when the simulation tick reaches this value.
    pub expires_at: Option<u64>,
    /// Geographic position of the task used in the distance cost function.
    pub pose: Option<Pose>,
}
```

Существующие места создания `Task` (в тестах и сценариях) должны получить
`required_capabilities: vec![]`, `preferred_role: None`, `expires_at: None`, `pose: None`.

**`crates/swarm-types/src/agent.rs`**

Добавить в `Agent`:

```rust
pub struct Agent {
    pub id: AgentId,
    pub role: Role,
    pub health: Health,
    pub pose: Pose,
    pub capabilities: Vec<Capability>,
    pub current_task: Option<TaskId>,
    // New in Milestone 2:
    /// Remaining battery level (0.0..=100.0). Static in Milestone 2; drain modelled in v0.3+.
    pub battery: f64,
}
```

Все существующие места создания `Agent` получают `battery: 100.0`.

---

### Шаг 2 — swarm-alloc: новый `Allocator` trait + `AuctionAllocator`

**`crates/swarm-alloc/src/allocator.rs`** — полная замена.

```rust
use swarm_types::{AgentId, Capability, Pose, Role, Task, TaskId};

/// Enriched task context passed to allocators.
pub struct AllocationTask<'a> {
    pub task: &'a Task,
}

/// Enriched agent context passed to allocators.
pub struct AllocationAgent<'a> {
    pub id: &'a AgentId,
    pub pose: Pose,
    pub battery: f64,
    pub capabilities: &'a [Capability],
    pub role: Role,
}
```

```rust
/// value: `(task_id, agent_id)` — allocation decisions
pub trait Allocator {
    fn allocate(
        &self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent<'_>],
    ) -> Vec<(TaskId, AgentId)>;
}
```

**`GreedyAllocator`** — обновить под новый trait:
- Capability matching как жёсткий фильтр (агент должен иметь все required capabilities).
- Остальная логика (round-robin by priority) без изменений.

```rust
impl Allocator for GreedyAllocator {
    fn allocate(&self, tasks: &[AllocationTask<'_>], agents: &[AllocationAgent<'_>]) -> Vec<(TaskId, AgentId)>;
}
```

**`AuctionAllocator`** — новый тип:

```rust
pub struct AuctionAllocator {
    pub weight_distance: f64,   // default 1.0
    pub weight_battery: f64,    // default 0.5
    pub weight_role: f64,       // default 0.3; bonus when role matches preferred_role
}

impl Default for AuctionAllocator { ... }
```

Cost function для пары (task, agent):
```
capability_gate: если agent не имеет хотя бы одной required_capabilities → cost = f64::INFINITY
distance_cost = weight_distance * euclidean(agent.pose, task.pose.unwrap_or(Pose{x:0.0,y:0.0}))
battery_cost  = weight_battery * (1.0 - agent.battery / 100.0)
role_bonus    = if task.preferred_role == Some(agent.role) { -weight_role } else { 0.0 }
total = distance_cost + battery_cost + role_bonus
```

Алгоритм `AuctionAllocator::allocate`:
1. Отсортировать задачи по убыванию `priority`.
2. Для каждой задачи — найти агента с минимальным `cost`. Если все `cost == INFINITY` → пропустить.
3. Назначить задачу агенту с минимальным cost (агент может получить несколько задач).
4. Вернуть `Vec<(TaskId, AgentId)>`.

---

### Шаг 3 — swarm-runtime: динамическая инжекция + expiry

**`crates/swarm-runtime/src/membership.rs`** — добавить в `AgentEntry`:

```rust
pub struct AgentEntry {
    pub role: Role,
    pub health: Health,
    pub capabilities: Vec<Capability>,
    pub last_heartbeat_tick: u64,
    // New in Milestone 2:
    pub battery: f64,
    pub pose: Pose,
}
```

`MembershipView::new(agents: Vec<Agent>)` инициализирует `battery` и `pose` из `Agent`.

**`crates/swarm-runtime/src/task_registry.rs`** — добавить:

```rust
impl TaskRegistry {
    /// Remove tasks whose expires_at <= current_tick. Returns expired TaskIds.
    ///
    /// Expiration rule (Milestone 2): only Unassigned and Assigned tasks expire.
    /// InProgress tasks are never expired — the agent is actively working on them.
    pub fn expire_tasks(&mut self, current_tick: u64) -> Vec<TaskId>;
}
```

Логика: итерировать по `tasks`, если `task.expires_at.is_some_and(|t| t <= current_tick)`
**И** `task.status != TaskStatus::InProgress` — удалить из registry, вернуть id.
InProgress задачи с истёкшим `expires_at` пропускаются (жёсткий дедлайн появится в v0.3+).

**`crates/swarm-runtime/src/coordinator.rs`** — изменить `CoordinatorOutput` и `process_tick`:

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoordinatorOutput {
    pub newly_failed: Vec<AgentId>,
    pub released_tasks: Vec<TaskId>,
    pub expired_task_ids: Vec<TaskId>,   // New
}

impl Coordinator {
    pub fn process_tick(
        &mut self,
        heartbeat_senders: Vec<AgentId>,
        current_tick: u64,
        injected_tasks: Vec<Task>,        // New: tasks dynamically added this tick
    ) -> CoordinatorOutput;

    /// Add a task dynamically at runtime.
    pub fn inject_task(&mut self, task: Task);
}
```

Логика `process_tick`:
1. Инжектировать `injected_tasks` в `registry`.
2. Обработать heartbeats, детектировать failures, release tasks (как раньше).
3. `let expired = self.registry.expire_tasks(current_tick)` — вернуть в output.

---

### Шаг 4 — swarm-sim: generic `ScenarioRunner` + dynamic tasks

**`crates/swarm-sim/src/runner.rs`** — добавить `DynamicTaskEvent` и обобщить runner:

```rust
#[derive(Clone, Debug)]
pub struct DynamicTaskEvent {
    pub at_tick: u64,
    pub task: Task,
}

pub struct RunConfig {
    pub max_ticks: u64,
    pub timeout_ticks: u64,
    pub max_unassigned_ticks: u64,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub failures: Vec<FailureEvent>,
    // New in Milestone 2:
    pub dynamic_tasks: Vec<DynamicTaskEvent>,
}
```

`ScenarioRunner::run_with<A: Allocator>`:

```rust
impl ScenarioRunner {
    pub fn run(scenario: &Scenario, config: RunConfig) -> RunMetrics {
        Self::run_with(scenario, config, GreedyAllocator)
    }

    pub fn run_with<A: Allocator>(
        scenario: &Scenario,
        config: RunConfig,
        allocator: A,
    ) -> RunMetrics;
}
```

Изменения в цикле тиков:
- До `coordinator.process_tick()`: собрать `injected_tasks` (все `DynamicTaskEvent` с `at_tick == current_tick`).
- Передать в `coordinator.process_tick(heartbeat_senders, current_tick, injected_tasks)`.
- `output.expired_task_ids` → инкремент `tasks_expired` в метриках.
- В `allocate_unassigned<A: Allocator>()` передавать `AllocationAgent` с pose/battery/capabilities из membership view.
- `allocate_unassigned` возвращает `u64` (число конфликтов этого тика); runner суммирует в `conflicting_assignments`.

Вспомогательная функция с явной обработкой конфликтов:

```rust
/// Returns the number of conflicting assignment decisions in this allocation round.
fn allocate_unassigned<A: Allocator>(
    coordinator: &mut Coordinator,
    allocator: &A,
) -> u64 {
    let tasks: Vec<_> = coordinator.registry.unassigned().into_iter().cloned().collect();
    let allocation_tasks: Vec<_> = tasks.iter().map(|task| AllocationTask { task }).collect();

    // Build agent context from membership view (owned copies to avoid lifetime issues).
    let agents: Vec<AllocationAgent> = coordinator
        .membership
        .alive_agents()
        .map(|(id, entry)| AllocationAgent {
            id: id.clone(),
            pose: entry.pose,
            battery: entry.battery,
            capabilities: entry.capabilities.clone(),
            role: entry.role.clone(),
        })
        .collect();
    let agent_refs: Vec<_> = agents.iter().collect();

    let decisions = allocator.allocate(&allocation_tasks, &agent_refs);

    // Deduplication pass: allocator must not produce two decisions for the same task.
    let mut seen = HashSet::new();
    let mut conflicts: u64 = 0;
    for (task_id, agent_id) in decisions {
        if !seen.insert(task_id.clone()) {
            // Duplicate task_id from allocator output — first decision wins.
            conflicts += 1;
            continue;
        }
        if coordinator.registry.assign(&task_id, agent_id).is_err() {
            // Task became non-assignable between unassigned() and assign() calls.
            conflicts += 1;
        }
    }
    conflicts
}
```

(Точные lifetime-аннотации определяются при реализации; выше — схема логики.)

---

### Шаг 4а — Ownership conflict handling

**Что такое conflict в этой модели:**

Ownership conflict = два assignment-решения для одной `TaskId` в одном раунде аукциона/аллокатора.

**Где обнаруживается:** в `allocate_unassigned` (см. Шаг 4), до применения к registry.

**Правило разрешения (winner selection):** первое вхождение `TaskId` в выводе аллокатора
выигрывает; все последующие отклоняются и считаются конфликтами. Порядок вывода аллокатора
детерминирован (отсортирован по убыванию priority → id), что делает winner selection
воспроизводимым при одинаковом seed.

**Дополнительный guard:** `TaskRegistry::assign()` возвращает `Err(InvalidTransition)`
если задача не находится в состоянии Unassigned. Этот error тоже считается конфликтом
и инкрементирует счётчик.

**Инвариант duplicate ownership:** `TaskRegistry` физически хранит `assigned_to: Option<AgentId>` —
одно значение на задачу. Второй владелец структурно невозможен. Тем не менее инвариант
тестируется явно (см. тест 2.6) для проверки, что runner не обходит registry напрямую.

**Что попадает в metrics и output:**
- `RunMetrics::conflicting_assignments: u64` — суммарное количество отклонённых дублей за всё время прогона.
- `CoordinatorOutput` конфликты не отражает: ownership conflict — это ошибка аллокатора/раунда,
  а не событие уровня coordinator. Runner владеет счётчиком и накапливает его независимо.

**`CoordinatorOutput`** остаётся без нового поля для conflict count: coordinator не участвует в
аллокации (это задача runner + allocator). Разделение ответственности сохраняется.

---

### Шаг 5 — swarm-metrics: расширить `RunMetrics`

**`crates/swarm-metrics/src/metrics.rs`**

```rust
pub struct RunMetrics {
    pub seed: u64,
    pub total_ticks: u64,
    pub messages_attempted: u64,
    pub messages_dropped: u64,
    pub detection_time_ticks: Option<u64>,
    pub reallocation_time_ticks: Option<u64>,
    pub max_task_unassigned_ticks: u64,
    pub all_tasks_assigned: bool,
    pub success: bool,
    // New in Milestone 2:
    pub tasks_injected: u64,
    pub tasks_expired: u64,
    /// Total conflicting assignment decisions rejected across all ticks (from DRONE_A.1.md metric).
    pub conflicting_assignments: u64,
}
```

Обновить тест-хелпер `run(...)` в `metrics.rs`, добавив
`tasks_injected: 0, tasks_expired: 0, conflicting_assignments: 0`.

`AggregateMetrics` — добавить:

```rust
pub avg_tasks_injected: f64,
pub avg_tasks_expired: f64,
pub avg_conflicting_assignments: f64,
```

---

### Шаг 6 — swarm-scenarios: DynamicAuction сценарий

**`crates/swarm-scenarios/src/coverage.rs`** — обновить `build_coverage_scenario`:
- `Task` → добавить `required_capabilities: vec![]`, `preferred_role: None`, `expires_at: None`, `pose: None`.
- `Agent` → добавить `battery: 100.0`.
- `RunConfig` → добавить `dynamic_tasks: vec![]`.

**`crates/swarm-scenarios/src/auction.rs`** — новый файл:

```rust
pub struct DynamicAuctionConfig {
    pub seed: u64,
    pub agent_count: usize,              // 5..=20
    pub initial_task_count: usize,
    pub dynamic_task_count: usize,       // tasks injected during mission
    pub dynamic_task_start_tick: u64,    // first injection tick
    pub dynamic_task_interval_ticks: u64, // ticks between injections
    pub task_expiry_ticks: u64,          // ticks until task expires from injection
    pub failure_tick: u64,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub timeout_ticks: u64,
    pub max_unassigned_ticks: u64,
    pub max_ticks: u64,
}

/// value: `(scenario, run_config)`
pub fn build_dynamic_auction_scenario(config: &DynamicAuctionConfig) -> (Scenario, RunConfig);
```

Логика построения:
- Агенты: разные роли (Scout, Mapper, Inspector по очереди), разные capabilities ("optical", "thermal", "lidar"), `battery: 100.0`, poses разбросаны по (0..50, 0..50) детерминированно через seed.
- Начальные задачи: `required_capabilities` из набора ("optical", "thermal", "lidar") циклически, `pose` распределены по зоне, `priority` разнообразные (1..=5), без expiration.
- Динамические задачи (в `RunConfig::dynamic_tasks`): инжектируются с шагом `dynamic_task_interval_ticks`, начиная с `dynamic_task_start_tick`. `expires_at = Some(injection_tick + task_expiry_ticks)`. Capabilities и poses генерируются детерминированно из seed.
- Один `FailureEvent` на `agent-0` в `failure_tick`.

**`crates/swarm-scenarios/src/lib.rs`** — добавить:

```rust
pub mod auction;
pub use auction::{build_dynamic_auction_scenario, DynamicAuctionConfig};
```

---

### Шаг 7 — swarm-examples: dynamic_auction binary

**`crates/swarm-examples/Cargo.toml`** — добавить `[[bin]]`:

```toml
[[bin]]
name = "dynamic_auction"
path = "src/bin/dynamic_auction.rs"
```

**`crates/swarm-examples/src/bin/dynamic_auction.rs`**:

Логика:
1. Для двух стратегий: `GreedyAllocator` и `AuctionAllocator::default()`.
2. Для каждой — 1000 прогонов (seed 0..999) с `DynamicAuctionConfig`:
   ```
   agent_count: 10, initial_task_count: 8, dynamic_task_count: 10,
   dynamic_task_start_tick: 5, dynamic_task_interval_ticks: 3,
   task_expiry_ticks: 15, failure_tick: 5, packet_loss_rate: 0.1,
   latency_ticks: 1, timeout_ticks: 3, max_unassigned_ticks: 8, max_ticks: 200
   ```
3. `AggregateMetrics::from_runs` для каждой стратегии.
4. Вывести сравнение:
   ```
   === greedy ===
   <metrics>
   === auction ===
   <metrics>
   ```
5. Если любая стратегия имеет `success_rate < 0.95` → exit code 1.

---

### Шаг 8 — swarm-sim: добавить `swarm-alloc` как зависимость

**`crates/swarm-sim/Cargo.toml`** — добавить:

```toml
swarm-alloc = { workspace = true }
```

(Если ещё не добавлено. ScenarioRunner уже импортирует `GreedyAllocator` — проверить.)

---

### Шаг 9 — README update

**`README.md`** — обновить секцию "Current Status":

- Milestone 1: ✓ (пометить как завершённый).
- Milestone 2: реализован (auction, dynamic tasks, expiry, capability matching, сравнение стратегий).
- Добавить секцию "Run `dynamic_auction`" с описанием вывода.

---

### Шаг 10 — Верификация и коммит

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p swarm-examples --bin empty_scenario
cargo run -p swarm-examples --bin coverage_with_failure
cargo run -p swarm-examples --bin dynamic_auction
git add Cargo.toml Cargo.lock crates/ README.md
git commit -m "feat: Milestone 2 — dynamic tasks, capability matching, auction allocator"
```

## Testing strategy

### Категория 1 — без рефакторинга (реализуются вместе с кодом)

**swarm-types:**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| 1.1 | `task_required_capabilities_serde` | swarm-types | `required_capabilities` сериализуется в JSON |
| 1.2 | `task_expires_at_serde` | swarm-types | `expires_at: Some(42)` сериализуется |
| 1.3 | `agent_battery_default_100` | swarm-types | Агент с battery=100.0 создаётся корректно |

**swarm-alloc:**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| 1.4 | `greedy_capability_gate_passes` | swarm-alloc | Задача требует "thermal", агент имеет "thermal" → назначена |
| 1.5 | `greedy_capability_gate_blocks` | swarm-alloc | Задача требует "thermal", ни у кого нет → пустой Vec |
| 1.6 | `greedy_with_rich_context_same_behavior` | swarm-alloc | Без required_capabilities поведение round-robin не изменилось |
| 1.7 | `auction_selects_closest_agent` | swarm-alloc | 2 агента, задача у позиции (10,0): агент на (9,0) побеждает |
| 1.8 | `auction_selects_capable_agent` | swarm-alloc | 2 агента, только один с "thermal" → capable побеждает несмотря на расстояние |
| 1.9 | `auction_skips_all_incapable` | swarm-alloc | Нет capable агента → пустой Vec |
| 1.10 | `auction_role_bonus_applied` | swarm-alloc | Агент с matching role имеет меньший cost |
| 1.11 | `auction_low_battery_penalized` | swarm-alloc | Агент с battery=10 проигрывает агенту с battery=100 при равных позициях |
| 1.12 | `no_duplicate_task_ownership` | swarm-alloc | AuctionAllocator никогда не назначает одну задачу двум агентам |

**swarm-runtime:**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| 1.13 | `task_registry_expire_at_tick` | swarm-runtime | `expire_tasks(5)` удаляет задачу с `expires_at=Some(5)` |
| 1.14 | `task_registry_expire_keeps_not_due` | swarm-runtime | Задача с `expires_at=Some(10)` остаётся при tick=5 |
| 1.15 | `task_registry_expire_assigned_task` | swarm-runtime | Истёкшая assigned задача тоже удаляется |
| 1.15b | `task_registry_expire_skips_in_progress` | swarm-runtime | Задача в InProgress с истёкшим `expires_at` НЕ удаляется |
| 1.16 | `coordinator_inject_task` | swarm-runtime | `inject_task()` → задача появляется в `unassigned()` |
| 1.17 | `coordinator_process_tick_injects` | swarm-runtime | `process_tick(_, tick, injected)` → tasks в registry |
| 1.18 | `coordinator_output_has_expired_ids` | swarm-runtime | `output.expired_task_ids` содержит истёкшие id |
| 1.19 | `membership_entry_has_battery_and_pose` | swarm-runtime | AgentEntry инициализируется из Agent.battery и Agent.pose |

**swarm-metrics:**

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| 1.20 | `aggregate_avg_tasks_injected` | swarm-metrics | Среднее tasks_injected вычисляется верно |
| 1.21 | `aggregate_avg_tasks_expired` | swarm-metrics | Среднее tasks_expired вычисляется верно |
| 1.22 | `task_registry_second_assign_returns_err` | swarm-runtime | Задача в Assigned/InProgress → повторный `assign()` возвращает `Err` |
| 1.23 | `allocate_unassigned_counts_duplicate_allocator_output` | swarm-sim | Аллокатор возвращает один `TaskId` дважды → `conflicts == 1`, задача назначена ровно одному агенту |

### Категория 2 — лёгкий рефакторинг

| # | Тест | Крейт | Описание |
|---|------|-------|----------|
| 2.1 | `runner_dynamic_task_appears_and_gets_assigned` | swarm-sim | Задача из DynamicTaskEvent назначается в нужный тик |
| 2.2 | `runner_expired_task_counted_in_metrics` | swarm-sim | tasks_expired в RunMetrics > 0 при наличии expiry |
| 2.3 | `runner_greedy_deterministic_with_capabilities` | swarm-sim | Два запуска — идентичные RunMetrics (с capabilities) |
| 2.4 | `runner_auction_deterministic` | swarm-sim | AuctionAllocator: два запуска с одним seed — идентичны |
| 2.5 | `runner_capability_gate_task_stays_unassigned` | swarm-sim | Задача с required capability, которой ни у кого нет → не назначена |
| 2.6 | `runner_no_duplicate_ownership_invariant` | swarm-sim | На каждом тике ни одна задача не имеет двух владельцев |
| 2.7 | `runner_conflict_counter_in_metrics` | swarm-sim | При stub-аллокаторе, возвращающем дубль, `RunMetrics::conflicting_assignments > 0` |

### Категория 3 — тяжёлый рефакторинг (будущие milestone)

| # | Тест | Описание |
|---|------|----------|
| 3.1 | Property-based тесты AuctionAllocator | `proptest`: при любом наборе агентов/задач — нет дублей, cost монотонен |
| 3.2 | 1000-seed stress для DynamicAuction | Вынесен в binary; слишком медленный для `cargo test` |
| 3.3 | Decentralized auction (CBBA) | Требует peer-to-peer messaging; появится в v0.4+ |
| 3.4 | Battery drain simulation | Требует кинематической модели; v0.3+ |

### Gap-анализ

- **Battery drain**: в Milestone 2 battery статична — стоимость battery не меняется в процессе миссии. Gap покрывается в Milestone 3 с кинематической моделью.
- **Decentralized conflict resolution**: auction централизован в ScenarioRunner. При decentralized агентах (v0.3 UDP) нужен CBBA или gossip-auction.
- **Сравнение greedy vs auction**: покрывается smoke-тестом бинарника, не unit-тестом (тест слишком медленный).

## Risks and tradeoffs

### Что могло сломаться

| Риск | Вероятность | Как проверить |
|------|-------------|---------------|
| Breaking changes в `Task` / `Agent` struct literals | Высокая | `cargo check --workspace` сразу покажет |
| `Allocator` trait смена сигнатуры ломает `allocate_unassigned` в runner | Высокая | `cargo build -p swarm-sim` |
| Lifetime-конфликт в `AllocationAgent<'a>` при сборке агентов из MembershipView | Средняя | Если возникает — использовать owned copies вместо ссылок |
| `expire_tasks` удаляет InProgress задачу, теряя прогресс | **Снято** | Правило зафиксировано: InProgress задачи не истекают (тест 1.15b) |
| `CoordinatorOutput` `expired_task_ids` не обновляет `max_task_unassigned_ticks` корректно | Средняя | Тест 1.15 и 2.2 покроют |
| Conflict counter неверно накапливается (двойной счёт или пропуск) | Средняя | Тест 1.23 и 2.7 покроют; stub-аллокатор даёт детерминированный входной сигнал |
| `dynamic_auction` binary: success_rate < 0.95 из-за expiry при плотных capabilities | Низкая | Параметры `max_ticks=200`, `max_unassigned_ticks=8` дают достаточно времени |

### Tradeoffs

- **`AllocationTask`/`AllocationAgent` как owned structs, не `&Task`/`&Agent`** — позволяет caller контролировать lifetime без борьбы с borrow checker. Минус: копирование, что на масштабе ~20 агентов несущественно.
- **Expiry удаляет задачи из registry** — альтернатива: `TaskStatus::Expired`. Удаление чище: registry не разрастается; минус: потеря истории. Для replay-модели будущих milestone придётся сохранять expired events в event log.
- **Auction централизован в ScenarioRunner** — соответствует текущей tick-based архитектуре. При переходе на multi-process (v0.3) auction нужно будет переделать в gossip-protocol.
- **`AuctionAllocator` может назначить несколько задач одному агенту** — соответствует поведению GreedyAllocator и текущей модели TaskRegistry (нет ограничения 1 task per agent). Если нужно ограничение — отдельная опция `max_tasks_per_agent`.
- **Battery статична** — осознанное упрощение. Динамический battery drain требует кинематики (v0.3+).

## Open questions

1. **InProgress задачи при expiration**: ~~открытый вопрос~~ **Решено:** InProgress задачи не истекают — агент активно работает над ними. Жёсткий дедлайн (`hard_deadline: bool`) появится в Milestone 3. Правило закреплено в `expire_tasks` и покрыто тестом 1.15b.
2. **`max_tasks_per_agent` для аукциона**: нужно ли ограничение "не более 1 новой задачи за раунд аукциона"? Влияет на fairness. Оставить как опциональный параметр `AuctionAllocator`.
3. **Сравнение greedy vs auction по метрикам**: какой порог успеха? Пока оба должны иметь `success_rate >= 0.95`. Если auction стабильно лучше greedy — зафиксировать в тесте или документировать в README.
4. **`Pose` для agents в сценариях**: в Milestone 1 все агенты на `(0.0, 0.0)`. Для auction distance cost нужны разные позиции. Разброс детерминирован через seed — каким паттерном (grid, random, ring)?
5. **Expiry в success criteria**: задача, истёкшая до назначения — это failure прогона или норма? Рекомендация: success = all non-expired tasks assigned OR completed.
