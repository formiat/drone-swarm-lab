# PLAN: Milestone 4 — Partial Connectivity (v0.4)

## Context

Milestone 1 (v0.1) реализовал membership, failure detection, task ownership, детерминированную in-process симуляцию.

Milestone 2 (v0.2) добавил динамические задачи, capability matching, auction allocator, pluggable `Allocator` trait.

Milestone 3 (v0.3) ввёл `AgentNode<T: Transport>`, pluggable Transport (in-memory + UDP), multiprocess-запуск, сериализацию, tracing.

**Milestone 4 (v0.4)** превращает runtime в частично-распределённую систему: network partitions, divergent local views, stale state handling, gossip/anti-entropy sync, convergence после восстановления связи.

**Источники контекста:** `DRONE_A.1.md` (v0.4: partial connectivity, gossip, stale state, convergence), `DRONE_B.1.md` (Фаза 2: distributed task allocation, CBBA, comms model). INVESTIGATION.md отсутствует.

**Критерий готовности:**
1. При network partition система не падает (runtime не паникует, нет deadlock).
2. Разные агенты имеют разные множества живых агентов (`alive_agents()`) во время partition.
3. Runtime не паникует на duplicate / delayed / reordered messages.
4. Stale heartbeat не реактивирует давно умершего агента.
5. После восстановления связи `global_assignment_map` сходится у всех агентов в partition.
6. Сходимость проверяется автоматически (не ручной check).

---

## Message Protocol (единый envelope)

v0.4 вводит типизированный message protocol. `RawMessage.payload` содержит JSON с полем `"type"`, определяющим дальнейшую обработку:

```rust
// In swarm-runtime (новый модуль crate::message)
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum RuntimeMessage {
    /// Heartbeat: alive signal with sender tick and generation.
    #[serde(rename = "hb")]
    Heartbeat {
        sender_tick: u64,
        generation: u64,
    },
    /// Anti-entropy gossip: full task→agent map + agent→generation map.
    #[serde(rename = "gossip")]
    Gossip {
        assignments: HashMap<TaskId, AgentId>,
        generations: HashMap<AgentId, u64>,
    },
}

impl RuntimeMessage {
    fn from_payload(payload: &[u8]) -> Option<Self> {
        serde_json::from_slice(payload).ok()
    }
}
```

**Единый dispatch за тик**: `AgentNode` один раз за тик дренирует `transport.poll()` и для каждого `RawMessage` пытается десериализовать `RuntimeMessage`. По типу:
- `Heartbeat` → извлечь `(msg.from, sender_tick, generation)` → обновить membership через `record_heartbeat`
- `Gossip` → накопить в буфер gossip сообщений, после heartbeat-фазы выполнить merge
- `None` (неизвестный/битый payload) → увеличить `discarded_messages`, залогировать warning

Это гарантирует, что gossip-сообщения не будут ошибочно обработаны как heartbeats, и наоборот. Дополнительный `seq` в `RawMessage` НЕ добавляется — идентификация дубликатов/reordering происходит внутри payload через `sender_tick` (heartbeat) и через generation-based merge (gossip).

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-runtime/src/message.rs` | **Новый**: `RuntimeMessage` enum, `from_payload()` |
| `crates/swarm-comms/src/network.rs` | Поддержка partitions в `InMemNetwork` |
| `crates/swarm-types/src/agent.rs` | Добавить `generation: u64` в `Agent` |
| `crates/swarm-runtime/src/membership.rs` | Stale heartbeat guard: `record_heartbeat(id, sender_tick, generation)`, protection по generation и tick |
| `crates/swarm-runtime/src/failure.rs` | Без изменений |
| `crates/swarm-runtime/src/coordinator.rs` | Без изменений |
| `crates/swarm-runtime/src/node.rs` | Новый dispatch loop: drain inbox → dispatch RuntimeMessage; gossip send/merge методы; `gossip_interval_ticks` |
| `crates/swarm-runtime/src/task_registry.rs` | `merge_assignment(task_id, agent_id, remote_generation)` |
| `crates/swarm-runtime/src/lib.rs` | Экспортировать модуль message, gossip методы |
| `crates/swarm-sim/src/runner.rs` | `PartitionEvent` в `RunConfig`, вызов gossip через AgentNode |
| `crates/swarm-metrics/src/metrics.rs` | Новые поля: `partition_events`, `stale_messages_discarded`, `convergence_ticks` |
| `crates/swarm-scenarios/src/partition.rs` | **Новый**: `PartitionScenario` builder |
| `crates/swarm-examples/src/bin/partition_scenario.rs` | **Новый**: partition → heal → convergence check |
| `crates/swarm-examples/Cargo.toml` | `[[bin]]` для `partition_scenario` |
| `README.md` | Обновить статус до Milestone 4 |

---

## Implementation Steps

### Шаг 1 — RuntimeMessage enum + единый dispatch

Новый файл: `crates/swarm-runtime/src/message.rs`

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use swarm_types::{AgentId, TaskId};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuntimeMessage {
    #[serde(rename = "hb")]
    Heartbeat {
        sender_tick: u64,
        generation: u64,
    },
    #[serde(rename = "gossip")]
    Gossip {
        assignments: HashMap<TaskId, AgentId>,
        generations: HashMap<AgentId, u64>,
    },
}

impl RuntimeMessage {
    pub fn from_payload(payload: &[u8]) -> Option<Self> {
        serde_json::from_slice(payload).ok()
    }

    pub fn to_payload(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn heartbeat(sender_tick: u64, generation: u64) -> Vec<u8> {
        Self::Heartbeat { sender_tick, generation }.to_payload()
    }

    pub fn gossip(
        assignments: HashMap<TaskId, AgentId>,
        generations: HashMap<AgentId, u64>,
    ) -> Vec<u8> {
        Self::Gossip { assignments, generations }.to_payload()
    }
}
```

Обновить `crates/swarm-runtime/src/node.rs` — переписать `process_inbox_and_allocate()`:
- Вместо текущего цикла `while let Some(msg) = transport.poll()` который все `msg.from` кладёт в `heartbeat_senders`:
- Новый dispatch loop:
  ```
  1. Drain all messages from transport into Vec<RawMessage>
  2. Для каждого msg:
     - Пытаемся десериализовать RuntimeMessage::from_payload(&msg.payload)
     - Heartbeat { sender_tick, generation } → записать (msg.from, sender_tick, generation) в hb_list
     - Gossip { ... } → записать в gossip_buffer
     - None → discarded_messages += 1, log warning
  3. Применить hb_list: для каждого (from, sender_tick, gen) вызвать membership.record_heartbeat(&from, sender_tick, gen)
  4. Coordinator::process_tick с теми же heartbeat_senders (для обратной совместимости: sender_ids = все уникальные from из hb_list)
  5. Если в gossip_buffer есть сообщения → gossip_merge(gossip_buffer)
  6. Allocate unassigned как обычно
  ```

**Тесты (категория 1):**
- `runtime_message_hb_serde_roundtrip`
- `runtime_message_gossip_serde_roundtrip`
- `unknown_payload_returns_none_not_panics`
- `dispatch_heartbeat_updates_membership` — heartbeat правильно обновляет membership
- `dispatch_gossip_does_not_affect_heartbeat_senders` — gossip не попадает в heartbeat_senders
- `dispatch_unknown_payload_is_discarded` — неизвестный payload не вызывает panic

---

### Шаг 2 — generation в Agent/AgentEntry, record_heartbeat с protection

Файл: `crates/swarm-types/src/agent.rs`

```rust
pub struct Agent {
    // ... existing fields ...
    pub generation: u64,   // NEW: 1 at creation
}
```

Файл: `crates/swarm-runtime/src/membership.rs`

```rust
pub struct AgentEntry {
    // ... existing fields ...
    pub generation: u64,   // NEW
    pub last_heartbeat_tick: u64,  // existing
}
```

Новая сигнатура `record_heartbeat`:
```rust
pub fn record_heartbeat(&mut self, agent_id: &AgentId, sender_tick: u64, generation: u64) {
    let Some(entry) = self.agents.get_mut(agent_id) else { return; };

    // Stale generation: ignore
    if generation < entry.generation {
        tracing::debug!(agent_id = %agent_id, generation, local_gen = entry.generation, "stale heartbeat ignored (old generation)");
        return;
    }

    // Newer generation: update and accept tick unconditionally
    if generation > entry.generation {
        entry.generation = generation;
        entry.last_heartbeat_tick = sender_tick;
        tracing::debug!(agent_id = %agent_id, generation, "heartbeat recorded (new generation)");
        return;
    }

    // Same generation: accept only if sender_tick is fresher
    // This handles delayed/reordered heartbeats idempotently
    if sender_tick > entry.last_heartbeat_tick {
        entry.last_heartbeat_tick = sender_tick;
        tracing::debug!(agent_id = %agent_id, sender_tick, "heartbeat recorded");
    } else {
        tracing::debug!(agent_id = %agent_id, sender_tick, local_tick = entry.last_heartbeat_tick, "stale heartbeat ignored (old tick)");
    }
}
```

**Тесты (категория 1):**
- `stale_heartbeat_with_lower_generation_is_ignored` — gen=1 при local gen=2 → не меняет tick
- `stale_heartbeat_with_old_tick_ignored` — sender_tick=5 при local last_hb=10, same gen → не меняет tick
- `fresh_heartbeat_with_higher_generation_updates` — gen=3 при local gen=2 → обновляет gen и tick
- `heartbeat_idempotent_same_tick_same_gen` — повторный hb с теми же tick/gen → no-op (без panic)

---

### Шаг 3 — Network partitions в `InMemNetwork`

Файл: `crates/swarm-comms/src/network.rs`

Расширить `NetworkConfig`:
```rust
pub struct NetworkConfig {
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub seed: u64,
    pub partitions: HashSet<(AgentId, AgentId)>,  // NEW
}
```

В `InMemNetwork::send()`: перед обработкой — если `(msg.from, msg.to)` или `(msg.to, msg.from)` в partitions → увеличить `messages_dropped`, `Ok(())`.

Динамические методы:
```rust
impl InMemNetwork {
    pub fn add_partition(&mut self, a: AgentId, b: AgentId);
    pub fn remove_partition(&mut self, a: AgentId, b: AgentId);
}
```

**Тесты (категория 1):**
- `partition_blocks_bidirectional_traffic`
- `partition_removal_restores_traffic`
- `non_partitioned_pairs_unaffected`

---

### Шаг 4 — Gossip / anti-entropy методы в `AgentNode`

Файл: `crates/swarm-runtime/src/node.rs`

```rust
impl<T: Transport> AgentNode<T> {
    /// Send gossip to all peer_ids.
    pub fn send_gossip(&mut self) -> Result<(), T::Error>;

    /// Merge all buffered gossip messages into local state.
    pub fn apply_gossip_buffer(&mut self, buffer: &[RuntimeMessage]) -> (u64, u64);
}
```

`send_gossip()`: собирает текущий `global_assignment_map` из `TaskRegistry` + `generations` из `MembershipView`, сериализует через `RuntimeMessage::gossip(...)`, шлёт каждому peer.

`apply_gossip_buffer()` logic (для каждого Gossip сообщения):
1. Для каждой пары `(task_id, remote_agent_id)` из `assignments`:
   - Получить локального владельца через `registry` (если assigned)
   - Если локально `task_id` unassigned → принять remote assign
   - Если локально assigned тому же `remote_agent_id` → no-op (уже согласны)
   - Если локально assigned другому агенту:
     - Взять `local_gen = membership.generation(local_owner)`, `remote_gen` из `generations` карты (fallback: 1)
     - Детерминированный tiebreaker:
       - Больший `generation` → authoritative (свежее состояние)
       - Равный `generation` → больший `AgentId` (лексикографически) → authoritative
     - Если remote authoritative: release локального owner, assign `remote_agent_id`
2. Обновить `MembershipView`: если remote `generation` > локальный `generation` для агента — обновить (агент перезапускался, stale view исправлен)
3. Вернуть `(merged_count, discarded_stale_count)`

**Инвариант**: total order по `(generation, AgentId)` — детерминирован для всех агентов. Все агенты, применив один и тот же набор gossip сообщений в любом порядке, приходят к одному `global_assignment_map` (commutative merge).

**Тесты (категория 1):**
- `gossip_merge_unassigned_task_from_remote` — remote assign для локально unassigned принимается
- `gossip_merge_higher_generation_overrides_local` — remote gen=3 перезаписывает локального owner с gen=1
- `gossip_merge_equal_generation_max_agentid_wins` — при равных gen, больший AgentId authoritative (проверить что BOTH стороны приходят к одному owner после обмена)
- `gossip_merge_lower_generation_is_ignored` — remote gen=1 не перезаписывает локального owner с gen=3
- `gossip_merge_same_owner_no_op` — remote говорит то же что и локально → без изменений
- `gossip_merge_updates_membership_generations` — generations в MembershipView обновляются из gossip
- `gossip_merge_ignores_dead_remote_owner` — если remote владелец локально dead → игнорировать remote assign

---

### Шаг 5 — Интеграция gossip в tick loop

Файл: `crates/swarm-runtime/src/node.rs`

Добавить в `AgentNode`:
```rust
pub gossip_interval_ticks: u64,
ticks_since_last_gossip: u64,
```

В `tick()` (и `process_inbox_and_allocate()`): после dispatch + coordinator + allocate:
```rust
self.ticks_since_last_gossip += 1;
if self.ticks_since_last_gossip >= self.gossip_interval_ticks {
    self.send_gossip()?;
    // gossip messages will arrive in next tick's dispatch
    self.ticks_since_last_gossip = 0;
}
```

Файл: `crates/swarm-sim/src/runner.rs`

Добавить в `RunConfig`:
```rust
pub partition_events: Vec<PartitionEvent>,
pub gossip_interval_ticks: u64,
```

```rust
pub struct PartitionEvent {
    pub at_tick: u64,
    pub until_tick: Option<u64>,
    pub agents: (AgentId, AgentId),
}
```

В tick loop: в начале каждого тика применять `partition_events` к `InMemNetwork`.

---

### Шаг 6 — Duplicate/delayed/reordered message handling

Это не отдельный шаг, а сквозное свойство протокола, документированное здесь:

**Heartbeat**: идемпотентен через `record_heartbeat(id, sender_tick, generation)`:
- Duplicate: тот же `sender_tick` и `generation` → `sender_tick > last_heartbeat_tick` = false → no-op
- Delayed: старый `sender_tick` → `sender_tick <= last_heartbeat_tick` → no-op  
- Reordered: gossip обрабатывается в любом порядке (generation + AgentId total order)

**Gossip**: commutative merge — порядок применения gossip не влияет на финальный результат:
- Каждое решение базируется на `(generation, AgentId)` total order
- Этот порядок одинаков для всех агентов

**Task assignment**: `registry.assign()` уже возвращает `Err` на дубликат — безопасно.

**Тесты (категория 1):**
- `duplicate_assignment_returns_err_not_panics`
- `reordered_gossip_messages_produce_same_result` — два gossip в разном порядке → одинаковый финальный state

---

### Шаг 7 — Partition scenario builder

Новый файл: `crates/swarm-scenarios/src/partition.rs`

```rust
pub struct PartitionConfig {
    pub seed: u64,
    pub agents: Vec<Agent>,
    pub tasks: Vec<Task>,
    pub timeout_ticks: u64,
    pub max_ticks: u64,
    pub gossip_interval_ticks: u64,
    pub partition_start_tick: u64,
    pub partition_heal_tick: u64,
    pub group_a: Vec<AgentId>,
    pub group_b: Vec<AgentId>,
}

pub fn build_partition_scenario(config: &PartitionConfig) -> (Scenario, RunConfig);
```

Для каждой пары `(a in group_a, b in group_b)` создаётся `PartitionEvent`.

---

### Шаг 8 — Partition scenario binary

Новый файл: `crates/swarm-examples/src/bin/partition_scenario.rs`

Сценарий (in-process, детерминированный):
1. 6 агентов (agent-0..agent-5) + 8 задач, timeout_ticks=5, gossip_interval_ticks=3
2. Tick 0..9: full mesh (без partition)
3. Tick 10: partition — agent-0,1,2 изолированы от agent-3,4,5
4. Tick 30: partition heal
5. Tick 60: конец симуляции
6. Проверки (через `assert!` в binary):
   - В тиках 11-29: `agent-0.coordinator.membership.alive_agents()` НЕ совпадает с `agent-3.coordinator.membership.alive_agents()` (разные local views)
   - После tick 30: `global_assignment_map` у agent-0 и agent-3 совпадает (gossip convergence)
   - Все 8 задач назначены хотя бы одному живому агенту

**Тест (категория 1 в swarm-examples):**
- `in_process_partition_scenario_converges` — запускает partition сценарий через `ScenarioRunner` и проверяет все три инварианта unit-тестом

---

### Шаг 9 — Метрики для Milestone 4

Файл: `crates/swarm-metrics/src/metrics.rs`

Новые поля в `RunMetrics`:
```rust
pub partition_events: u64,           // количество partition event'ов за прогон
pub partitions_active: bool,         // true если хотя бы один partition был активен
pub stale_messages_discarded: u64,   // неизвестные/битые payloads
pub convergence_ticks: Option<u64>,  // тиков после heal до сходимости maps
pub max_view_divergence: u64,        // макс. число агентов с разными maps в одном тике
```

Обновить `AggregateMetrics` и `Display`.

**Тест (категория 1):**
- `partition_metrics_present_after_partition_heal`

---

### Шаг 10 — Обновить `crates/swarm-examples/Cargo.toml`

```toml
[[bin]]
name = "partition_scenario"
path = "src/bin/partition_scenario.rs"
```

---

### Шаг 11 — Обновить `README.md`

- Добавить **Milestone 4** в `## Current Status`
- Добавить `partition_scenario` в `## Run Examples`
- Документировать gossip interval, partition convergence, `RUST_LOG` для отладки

---

## Verification Commands

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

### Категория 1 — Без рефакторинга

**`swarm-runtime` — RuntimeMessage + dispatch (6 тестов):**
- `runtime_message_hb_serde_roundtrip`
- `runtime_message_gossip_serde_roundtrip`
- `unknown_payload_returns_none_not_panics`
- `dispatch_heartbeat_updates_membership`
- `dispatch_gossip_does_not_affect_heartbeat_senders`
- `dispatch_unknown_payload_is_discarded`

**`swarm-runtime` — stale heartbeat (4 теста):**
- `stale_heartbeat_with_lower_generation_is_ignored`
- `stale_heartbeat_with_old_tick_ignored`
- `fresh_heartbeat_with_higher_generation_updates`
- `heartbeat_idempotent_same_tick_same_gen`

**`swarm-comms` — partitions (3 теста):**
- `partition_blocks_bidirectional_traffic`
- `partition_removal_restores_traffic`
- `non_partitioned_pairs_unaffected`

**`swarm-runtime` — gossip merge (7 тестов):**
- `gossip_merge_unassigned_task_from_remote`
- `gossip_merge_higher_generation_overrides_local`
- `gossip_merge_equal_generation_max_agentid_wins`
- `gossip_merge_lower_generation_is_ignored`
- `gossip_merge_same_owner_no_op`
- `gossip_merge_updates_membership_generations`
- `gossip_merge_ignores_dead_remote_owner`

**`swarm-runtime` — defensive (2 теста):**
- `duplicate_assignment_returns_err_not_panics`
- `reordered_gossip_messages_produce_same_result`

**`swarm-sim` — partition runner (2 теста):**
- `runner_partition_scenario_detects_different_alive_views`
- `runner_partition_scenario_converges_after_heal`

**`swarm-metrics` (1 тест):**
- `partition_metrics_present_after_partition_heal`

**`swarm-examples` (1 тест):**
- `in_process_partition_scenario_converges`

**Регрессия**: все существующие 115+ тестов должны пройти.

### Категория 2 — Лёгкий рефакторинг

- **Multiprocess UDP partition test (`#[ignore]`)**: 6 `agent_process` через UDP loopback. Partition через `iptables` дроп правил на конкретные порты. Проверить divergence и convergence. Требует `sudo`/`CAP_NET_ADMIN`.

### Категория 3 — Тяжёлый (не для v0.4)

- Property-based partition test со случайными топологиями, длительностями, message reordering.
- Distributed CBBA — полноценный consensus поверх gossip (v0.5).
- Формальное доказательство convergence (TLA+).

### Покрытие gap

- **Gap**: wallclock convergence time проверяется только in-process. Multiprocess зависит от tick_ms + gossip_interval. Приемлемо для v0.4.
- **Gap**: multiprocess partition injection требует `iptables`. Альтернатива — partition-ctl команды в gossip протоколе — не в scope v0.4.
- **Gap**: TLA+ не в scope v0.4.

---

## Risks and Tradeoffs

**1. Stale heartbeat reactivation**

Без generation старый heartbeat мог бы реактивировать dead агента. Митигация: `generation` + `sender_tick` в heartbeat payload. `record_heartbeat` игнорирует меньший generation и старый tick.

**2. Gossip overhead**

При gossip_interval_ticks=3 и 6 агентах: 6×5 = 30 сообщений за round (~10/тик average). На loopback/UDP — незначительно. Для продакшена — delta-сжатие (v0.5).

**3. Partition vs process crash**

При partition агенты не получают heartbeats, детектят пиров как dead. После heal — gossip приносит свежий generation (тот же что был, т.к. агент не перезапускался). Отличие от crash: generation не инкрементирован.

**4. Convergence гарантия**

Gossip merge с детерминированным total order `(generation, AgentId)` гарантирует сходимость при восстановленной связности и регулярных gossip раундах. Без gossip система НЕ сходится (v0.3 replicated-state расходился).

**5. Breaking change: RawMessage payload теперь JSON-typed**

Все существующие тесты, создающие `RawMessage` с `payload: b"hb"`, нужно обновить на `RuntimeMessage::heartbeat(sender_tick, generation)`. Это затронет ~20 мест в тестах `swarm-runtime` и `swarm-sim`.

**6. `record_heartbeat` сигнатура изменилась**

Было: `fn record_heartbeat(&mut self, agent_id: &AgentId, tick: u64)`
Стало: `fn record_heartbeat(&mut self, agent_id: &AgentId, sender_tick: u64, generation: u64)`

Все вызовы через `process_tick` нужно обновить. `coordinator.process_tick` сохраняет текущую сигнатуру (принимает `current_tick`, передаёт в `record_heartbeat`).

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| Изменение формата `RawMessage.payload` (b"hb" → JSON) | Все тесты, создающие hb payload, не компилируются или падают | `cargo test --workspace` |
| Изменение сигнатуры `record_heartbeat` | coordinator.rs, node.rs, тесты | `cargo check --workspace` |
| Partition по умолчанию пуст (`HashSet::new()` в `NetworkConfig`) | Существующие runner-тесты используют `NetworkConfig` без `partitions` поля | `cargo test -p swarm-sim` |
| Gossip сообщения потребляют transport bandwidth | При низком gossip_interval может замедлить failure detection | Измерить в тестах: detection_time не должен расти |
| `HashMap` в `RuntimeMessage::Gossip` сериализуется недетерминированно | Разные агенты могут посылать разный JSON для одного и того же состояния | Сериализация HashMap через serde_json детерминирована (sorted keys в Rust ≥1.78) |
| `Cargo.lock` изменён | Должен быть включён в commit | `git diff --stat` |

---

## Open Questions

1. **Gossip interval default**: 3 тика. При timeout_ticks=5 это даёт ~1 gossip round до detection. Приемлемо? Если нет — уменьшить до 1.

2. **Full state vs delta gossip**: v0.4 шлёт полную карту. При 8 задачах overhead незначителен. v0.5 с 100+ задачами — delta.

3. **Generation persistence**: in-memory only. При process restart generation=1. Для v0.4 — достаточно (процессы не перезапускаются).

4. **Multiprocess partition injection**: как инжектировать partition в UDP без `iptables`? Для v0.4 — только in-process partition. Multiprocess отложен до v0.5.

5. **Should `current_tick` propagate through coordinator unchanged?** `coordinator.process_tick` принимает `current_tick` и передаёт его в `record_heartbeat`. Но теперь `record_heartbeat` использует `sender_tick` из heartbeat payload, а не `current_tick`. `current_tick` всё ещё нужен для `expire_tasks`. Нужно сохранить оба параметра или передавать `sender_tick` напрямую. Решение: `process_tick` передаёт `sender_tick` (из hb payload), а `current_tick` используется только для `expire_tasks` и `detect`. Для self-heartbeat — `sender_tick = current_tick`.
