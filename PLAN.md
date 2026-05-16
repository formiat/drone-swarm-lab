# PLAN: Milestone 4 — Partial Connectivity (v0.4)

## Context

Milestone 1 (v0.1) реализовал membership, failure detection, task ownership, детерминированную in-process симуляцию.

Milestone 2 (v0.2) добавил динамические задачи, capability matching, auction allocator, pluggable `Allocator` trait.

Milestone 3 (v0.3) ввёл `AgentNode<T: Transport>`, pluggable Transport (in-memory + UDP), multiprocess-запуск, сериализацию, tracing.

**Milestone 4 (v0.4)** превращает runtime в частично-распределённую систему: network partitions, divergent local views, stale state handling, gossip/anti-entropy sync, convergence после восстановления связи.

**Источники контекста:** `DRONE_A.1.md` (v0.4: partial connectivity, gossip, stale state, convergence), `DRONE_B.1.md` (Фаза 2: distributed task allocation, CBBA, comms model). INVESTIGATION.md отсутствует.

**Критерий готовности:**
1. При network partition система не падает (runtime не паникует, нет deadlock).
2. Разные агенты имеют разные `MembershipView` во время partition (разные множества alive/dead peers).
3. Runtime не паникует на duplicate / delayed / reordered messages.
4. Stale heartbeat не реактивирует давно умершего агента.
5. После восстановления связи `global_assignment_map` сходится у всех агентов в partition.
6. Сходимость проверяется автоматически (не ручной check).

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-comms/src/network.rs` | Поддержка partitions в `InMemNetwork`, аннотированные метрики |
| `crates/swarm-comms/src/transport.rs` | Добавить `message_seq: u64` в `RawMessage` для replay/staleness detection |
| `crates/swarm-types/src/agent.rs` | Добавить `generation: u64` (epoch) в `Agent` для stale heartbeat protection |
| `crates/swarm-runtime/src/membership.rs` | Stale heartbeat guard: не обновлять `last_heartbeat_tick` если generation старее текущего; метод `record_heartbeat` теперь принимает generation |
| `crates/swarm-runtime/src/failure.rs` | Без изменений в логике (уже работает поверх membership) |
| `crates/swarm-runtime/src/coordinator.rs` | Без изменений |
| `crates/swarm-runtime/src/node.rs` | Добавить gossip round: `exchange_gossip()` — обмен TaskId→AgentId картами + AgentId→generation картами; stale-ownership merge logic |
| `crates/swarm-runtime/src/task_registry.rs` | `merge_assignment(task_id, agent_id, remote_generation)` — принимает remote assignment только если generation не старее локального |
| `crates/swarm-runtime/src/lib.rs` | Экспортировать gossip/merge методы |
| `crates/swarm-sim/src/runner.rs` | Поддержка `PartitionEvent` в `RunConfig`, вызов gossip раундов в tick loop |
| `crates/swarm-metrics/src/metrics.rs` | Новые поля: `partition_events`, `stale_messages_discarded`, `convergence_ticks` |
| `crates/swarm-scenarios/src/partition.rs` | Новый файл: `PartitionScenario` builder |
| `crates/swarm-examples/src/bin/partition_scenario.rs` | Новый binary: partition → heal → convergence check |
| `crates/swarm-examples/Cargo.toml` | Добавить `[[bin]]` для `partition_scenario` |
| `Cargo.toml` (workspace) | Без изменений зависимостей (уже есть всё) |
| `README.md` | Обновить статус до Milestone 4, описать новый сценарий |

---

## Implementation Steps

### Шаг 1 — Добавить `message_seq` и `generation` для staleness protection

Файл: `crates/swarm-comms/src/transport.rs`

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawMessage {
    pub from: AgentId,
    pub to: AgentId,
    pub seq: u64,          // NEW: message sequence number (per-sender, monotonic)
    pub payload: Vec<u8>,
}
```

Файл: `crates/swarm-types/src/agent.rs`

```rust
/// Agent generation (epoch). Incremented each time the agent restarts or
/// is considered "stale". Heartbeats with old generation are discarded.
pub type Generation = u64;

pub struct Agent {
    // ... existing fields ...
    pub generation: Generation,   // NEW: 1 at creation
}
```

Добавить `Generation` в `AgentEntry` в `membership.rs`:
```rust
pub struct AgentEntry {
    // ... existing fields ...
    pub generation: Generation,
}
```

`MembershipView::record_heartbeat` теперь принимает `generation: Generation` и обновляет только если `generation >= entry.generation`.

**Тесты (категория 1):**
- `stale_heartbeat_with_lower_generation_is_ignored` — heartbeat с generation=1 не перезаписывает entry с generation=2
- `fresh_heartbeat_with_higher_generation_updates_entry` — heartbeat с generation=3 обновляет entry с generation=2

---

### Шаг 2 — Network partitions в `InMemNetwork`

Файл: `crates/swarm-comms/src/network.rs`

Расширить `NetworkConfig`:
```rust
pub struct NetworkConfig {
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub seed: u64,
    /// NEW: set of (src, dst) agent pairs that are partitioned.
    /// Messages from src to dst AND from dst to src are dropped 100%.
    pub partitions: HashSet<(AgentId, AgentId)>,
}
```

В `InMemNetwork::send()`: перед processing проверять `partitions`. Если `(msg.from, msg.to)` или `(msg.to, msg.from)` в partitions — увеличить `messages_dropped` и вернуть `Ok(())` (имитирует partition).

Методы для динамического управления partitions:
```rust
impl InMemNetwork {
    pub fn add_partition(&mut self, a: AgentId, b: AgentId);
    pub fn remove_partition(&mut self, a: AgentId, b: AgentId);
    pub fn partitioned_pairs(&self) -> &HashSet<(AgentId, AgentId)>;
}
```

**Тесты (категория 1):**
- `partition_blocks_bidirectional_traffic` — сообщения в обе стороны дропаются
- `partition_removal_restores_traffic` — после `remove_partition` сообщения снова доходят
- `non_partitioned_pairs_unaffected` — агенты не в partition общаются нормально

---

### Шаг 3 — Gossip / anti-entropy protocol in `AgentNode`

Файл: `crates/swarm-runtime/src/node.rs`

Новый метод:
```rust
impl<T: Transport> AgentNode<T> {
    /// Send gossip message to all peers: current task assignment map + agent generations.
    pub fn send_gossip(&mut self) -> Result<(), T::Error>;

    /// Receive gossip messages and merge remote state into local view.
    /// Returns count of merged assignments and stale messages discarded.
    pub fn receive_and_merge_gossip(&mut self) -> Result<(u64, u64), T::Error>;
}
```

Gossip payload (через `RawMessage.payload` как JSON):
```json
{
  "type": "gossip",
  "assignments": { "task-0": "agent-1", "task-1": "agent-2", ... },
  "generations": { "agent-0": 1, "agent-1": 1, "agent-2": 2, ... }
}
```

`receive_and_merge_gossip()` логика:
1. Poll transport для всех gossip-сообщений
2. Для каждого remote assignment `(task_id, agent_id)`:
   - Если локально `task_id` unassigned: принять assignment через `registry.assign()`
   - Если локально `task_id` assigned другому агенту:
     - Сравнить generations владельцев; больше generation — authoritative
     - При равенстве: больше AgentId лексикографически — authoritative (arbitrary tiebreaker)
     - Если remote authoritative: release локального owner, assign remote owner
3. Обновить `MembershipView` из `generations`: если remote generation > local, обновить (агент перезапускался)
4. Вернуть `(merged_count, discarded_stale_count)`

**Инвариант**: gossip merge не должен создавать дублирующий ownership (если локально task уже assigned кому-то с тем же generation — не трогаем).

**Тесты (категория 1):**
- `gossip_merge_unassigned_task_from_remote` — удалённый assignment для unassigned task принимается
- `gossip_merge_higher_generation_overrides_local` — remote generation 3 перезаписывает локального owner с generation 1
- `gossip_merge_equal_generation_keeps_local` — при равных generations локальное состояние сохраняется
- `gossip_merge_updates_membership_generations` — generations в MembershipView обновляются из gossip

---

### Шаг 4 — Интеграция gossip в tick loop (AgentNode и ScenarioRunner)

Файл: `crates/swarm-runtime/src/node.rs`

Добавить `gossip_interval_ticks` в `AgentNode`:
```rust
pub struct AgentNode<T> {
    pub coordinator: Coordinator,
    pub transport: T,
    pub own_id: AgentId,
    pub peer_ids: Vec<AgentId>,
    pub gossip_interval_ticks: u64,  // NEW
    ticks_since_last_gossip: u64,     // NEW
}
```

Модифицировать `tick()`: после `process_tick()` + allocate, если `ticks_since_last_gossip >= gossip_interval_ticks`:
1. `send_gossip()`
2. `receive_and_merge_gossip()`
3. Сбросить `ticks_since_last_gossip = 0`

Файл: `crates/swarm-sim/src/runner.rs`

Добавить в `RunConfig`:
```rust
pub struct RunConfig {
    // ... existing fields ...
    pub partition_events: Vec<PartitionEvent>,  // NEW
    pub gossip_interval_ticks: u64,              // NEW
}

pub struct PartitionEvent {
    pub at_tick: u64,           // start of partition
    pub until_tick: Option<u64>, // None = permanent, Some = heals at this tick
    pub agents: (AgentId, AgentId),
}
```

В tick loop:
- Применять `partition_events` (добавлять/убирать partitions из `InMemNetwork`)
- Gossip вызывается внутри `AgentNode::tick()` автоматически через `gossip_interval_ticks`

---

### Шаг 5 — Stale state handling (duplicate/delayed/reordered messages)

Текущий runtime уже не паникует на random messages (heartbeat просто записывает `last_heartbeat_tick`, неизвестный отправитель игнорируется `record_heartbeat`). Но нужно добавить:

**a) Duplicate heartbeat protection**: если `last_heartbeat_tick` уже >= tick в сообщении — игнорировать.

Файл: `crates/swarm-runtime/src/membership.rs`
```rust
pub fn record_heartbeat(&mut self, agent_id: &AgentId, tick: u64, generation: Generation) {
    if let Some(entry) = self.agents.get_mut(agent_id) {
        if generation < entry.generation {
            tracing::debug!(agent_id = %agent_id, generation, local_generation = entry.generation, "stale heartbeat ignored");
            return;
        }
        if generation > entry.generation {
            entry.generation = generation;
        }
        if tick > entry.last_heartbeat_tick {
            entry.last_heartbeat_tick = tick;
        }
        tracing::debug!(agent_id = %agent_id, "heartbeat recorded");
    }
}
```

**b) Duplicate task assignment**: `TaskRegistry::assign()` уже возвращает `Err` при повторном assign — это безопасно, но нужно логировать.

**c) Delayed/reordered messages**: Transport poll возвращает сообщения в порядке получения. Gossip merge обрабатывает их в любом порядке (сравнение generations). Heartbeat обрабатывается идемпотентно.

**Тесты (категория 1):**
- `stale_heartbeat_with_old_tick_not_applied` — heartbeat tick=5 при local tick=10 не меняет last_heartbeat_tick
- `duplicate_assignment_returns_err_not_panics` — повторный assign возвращает Err
- `reordered_gossip_messages_produce_same_result` — два gossip сообщения в разном порядке дают одинаковый финальный state

---

### Шаг 6 — Partition scenario builder

Новый файл: `crates/swarm-scenarios/src/partition.rs`

```rust
pub struct PartitionConfig {
    pub seed: u64,
    pub agents: Vec<Agent>,
    pub tasks: Vec<Task>,
    pub timeout_ticks: u64,
    pub max_ticks: u64,
    pub gossip_interval_ticks: u64,
    /// (at_tick, (group_a, group_b)) — agents are split into two groups
    pub partition_start_tick: u64,
    pub partition_heal_tick: u64,
    pub group_a: Vec<AgentId>,
    pub group_b: Vec<AgentId>,
}

pub struct PartitionScenarioOutput {
    pub scenario: Scenario,
    pub run_config: RunConfig,
}

pub fn build_partition_scenario(config: &PartitionConfig) -> PartitionScenarioOutput;
```

Логика: для каждой пары `(a in group_a, b in group_b)` создаётся `PartitionEvent { at_tick: partition_start, until_tick: Some(heal), agents: (a, b) }`.

---

### Шаг 7 — Partition scenario binary

Новый файл: `crates/swarm-examples/src/bin/partition_scenario.rs`

Сценарий:
1. 6 агентов (agent-0..agent-5) + 8 задач
2. Конфиг: timeout_ticks=5, gossip_interval_ticks=3
3. Ticks 0..9: все связаны (full mesh)
4. Tick 10: partition — agent-0,1,2 изолированы от agent-3,4,5
5. Tick 30: partition исцеляется (heal)
6. Tick 60: max_ticks
7. Проверить:
   - После tick 10: `MembershipView` agent-0 не содержит agent-3,4,5 (разные local views) → проверяется через `detected_failures`
   - После tick 30: gossip сходится — `global_assignment_map` идентичен у agent-0 и agent-3
   - Все 8 задач назначены

Binary запускает `ScenarioRunner::run_with(partition_scenario, config, GreedyAllocator)` и проверяет метрики.

**Тест (категория 1 в swarm-examples):**
- `in_process_partition_scenario_converges` — запускает `partition_scenario` in-process через `ScenarioRunner`, проверяет exit-подобные условия unit-тестом

---

### Шаг 8 — Метрики для Milestone 4

Файл: `crates/swarm-metrics/src/metrics.rs`

Добавить поля в `RunMetrics`:
```rust
pub struct RunMetrics {
    // ... existing fields ...
    pub partition_events: u64,           // NEW
    pub partitions_active: bool,         // NEW: true at least once
    pub stale_messages_discarded: u64,   // NEW
    pub convergence_ticks: Option<u64>,  // NEW: ticks after heal until maps converge
    pub max_view_divergence: u64,        // NEW: max number of agents with differing maps
}
```

Обновить `AggregateMetrics` и `Display`.

Метрики заполняются в `ScenarioRunner::run_with()`.

**Тест (категория 1):**
- `partition_metrics_present_after_partition_heal` — после partition+heal в метриках `partitions_active=true`, `partition_events > 0`

---

### Шаг 9 — Обновить `crates/swarm-examples/Cargo.toml`

Добавить:
```toml
[[bin]]
name = "partition_scenario"
path = "src/bin/partition_scenario.rs"
```

---

### Шаг 10 — Обновить `README.md`

- Добавить **Milestone 4** в раздел `## Current Status`
- Добавить `partition_scenario` в `## Run Examples`
- Документировать gossip interval, partition convergence

---

## Verification Commands

После реализации:
```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run -p swarm-examples --bin partition_scenario
cargo run -p swarm-examples --bin coverage_with_failure
cargo run -p swarm-examples --bin dynamic_auction
cargo run -p swarm-examples --bin multiprocess_scenario
```

---

## Testing Strategy

### Категория 1 — Без рефакторинга (реализовать вместе с основными изменениями)

**`swarm-comms` — network partitions:**
- `partition_blocks_bidirectional_traffic`
- `partition_removal_restores_traffic`
- `non_partitioned_pairs_unaffected`

**`swarm-comms` — message_seq:**
- `raw_message_with_seq_serde_roundtrip`

**`swarm-runtime` — stale heartbeat protection:**
- `stale_heartbeat_with_lower_generation_is_ignored`
- `stale_heartbeat_with_old_tick_not_applied`
- `fresh_heartbeat_with_higher_generation_updates_entry`

**`swarm-runtime` — gossip merge:**
- `gossip_merge_unassigned_task_from_remote`
- `gossip_merge_higher_generation_overrides_local`
- `gossip_merge_equal_generation_keeps_local`
- `gossip_merge_updates_membership_generations`

**`swarm-runtime` — duplicate/delayed/reordered (defensive):**
- `duplicate_assignment_returns_err_not_panics`
- `reordered_gossip_messages_produce_same_result`

**`swarm-sim` — partition runner:**
- `runner_partition_scenario_detects_partition_views`
- `runner_partition_scenario_converges_after_heal`

**`swarm-metrics` — новые поля:**
- `partition_metrics_present_after_partition_heal`

**`swarm-examples` — in-process partition test:**
- `in_process_partition_scenario_converges`

**Регрессионные тесты:**
- Все существующие тесты в `swarm-comms`, `swarm-runtime`, `swarm-alloc`, `swarm-sim`, `swarm-metrics`, `swarm-scenarios` должны пройти (нет изменений существующей логики)

### Категория 2 — Лёгкий рефакторинг

- **Интеграционный тест multiprocess + partition**: запустить 6 `agent_process` через UDP, инжектировать partition через отдельный канал (e.g. `iptables` дроп на loopback портах, или встроить partition-команды в gossip протокол). Проверить divergence во время partition и convergence после. Пометить `#[ignore]` (требует сетевых прав).

### Категория 3 — Тяжёлый рефакторинг (не планируется для v0.4)

- Property-based partition test с случайными топологиями и длительностями partition.
- Distributed CBBA (Consensus-Based Bundle Algorithm) — полноценный consensus поверх gossip. Отложен до v0.5.

### Покрытие gap

- **Gap**: wallclock convergence time проверяется только in-process. Мультипроцессное время зависит от tick_ms и gossip_interval. Приемлемо для v0.4.
- **Gap**: `iptables`-based partition injection для multiprocess требует sudo. Альтернатива: partition-команды в gossip протоколе (добавить `{"type": "partition_ctl", "action": "block", "peer": "agent-X"}`) — не планируется для v0.4, но может быть добавлено позже.
- **Gap**: formal proof of convergence (TLA+). Не в scope v0.4.

---

## Risks and Tradeoffs

**1. Stale heartbeat reactivation**

Без generation/epoch поле `last_heartbeat_tick` не отличает свежий heartbeat от задержанного. Риск: старый heartbeat от перезапущенного агента реактивирует его как alive, хотя он уже dead по таймауту и его задачи перераспределены. Митигация: `generation` field — меньше generation → игнорировать.

**2. Gossip overhead**

Каждый gossip round шлёт N×M сообщений (N агентов × M соседей). При gossip_interval_ticks=3 и 6 агентах: ~30 сообщений на тик. На loopback/UDP это незначительно. Для продакшена: delta-сжатие (только изменения с прошлого раунда) — тема v0.5.

**3. Fake partition vs process crash**

При partition агент не получает heartbeats и детектит пиров как dead. После heal пиры «оживают» (gossip приносит свежий generation). Отличие от настоящего crash: при partition агенты продолжают работать и после heal их generation тот же; при настоящем crash+restart generation инкрементируется. Митигация: generation остаётся прежним при partition (не инкрементируется), что позволяет отличить partition от crash в метриках.

**4. Convergence vs divergence**

Gossip с generation-based merge гарантирует convergence при условии что partition healed и gossip раунды проходят регулярно. Без gossip система НЕ сходится (replicated-state подход v0.3 расходился без gossip — исправлено в v0.4). Митигация: тесты проверяют convergence через фиксированное число тиков после heal.

**5. Breaking changes**

Добавление `seq` в `RawMessage` и `generation` в `Agent`/`AgentEntry` — breaking change для сериализации. Все существующие тесты, использующие создание `RawMessage` или `Agent`, нужно обновить. Митигация: grep всех мест создания и добавить `seq: 0` / `generation: 1`.

**6. `TaskRegistry::merge_assignment` корректность**

При merge remote assignment нужно проверять не только generation владельца, но и то, жив ли владелец локально. Если локально владелец dead, а remote говорит что он владеет задачей — игнорировать remote (stale data). Митигация: проверка `membership.is_alive()` перед merge.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| Добавление `seq` в `RawMessage` | Все места, создающие `RawMessage`, не компилируются | `cargo check --workspace` |
| Добавление `generation` в `Agent`/`AgentEntry` | Сериализация, тесты, конструкторы | `cargo test --workspace` |
| Изменение сигнатуры `record_heartbeat` | Все вызовы в coordinator, node, тестах | `cargo check --workspace` + `cargo test --workspace` |
| Partition в `InMemNetwork` | Существующие runner-тесты могут сломаться если partition случайно влияет | `cargo test -p swarm-sim` |
| Gossip merge логика | Некорректный merge может создать дублирующий ownership или потерять assignment | Тесты `gossip_merge_*` |
| `ScenarioRunner` integration с partition events | Регрессия в существующих runner-тестах | `cargo test --workspace` |
| `Cargo.lock` изменился | Должен быть включён в commit | `git diff --stat` перед commit |

---

## Open Questions

1. **Gossip interval default**: 3 тика. При timeout_ticks=5 это даёт 1 gossip round до detection + 2 до reallocation. Достаточно ли? Если нет — уменьшить до 1, но увеличится message overhead.

2. **Full state vs delta gossip**: в v0.4 шлём полную карту assignments. При 8 задачах overhead незначителен. Для v0.5 с 100+ задачами — нужен delta-протокол.

3. **Generation persistence**: generation хранится in-memory. При перезапуске процесса generation не восстанавливается (hardcoded 1). Нужна ли persistence? Для v0.4 — нет (процессы не перезапускаются, только crash). Для v0.5 с recovery — да.

4. **Multiprocess partition injection**: как инжектировать partition в UDP-процессы без `iptables`? Варианты: (a) встроить `pause_send`/`resume_send` команды в gossip протокол; (b) использовать `iptables` на loopback как integration test с `sudo`. Для v0.4 — достаточно in-process partition через `InMemNetwork`. Multiprocess partition отложен до v0.5.

5. **Should gossip use separate transport channel?** Текущий `Transport` trait — point-to-point. Gossip сообщения идут тем же каналом что и heartbeats. Это упрощает архитектуру, но может создавать contention. Приемлемо для v0.4 с ~6 агентами.
