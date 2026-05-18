# PLAN: Phase 1 — True Distributed CBBA

## Context

Milestone 10 реализовал `CbbaAllocator` как Allocator facade: bundle building + consensus logic в одном вызове `allocate()`. Но CBBA не использует message exchange — `apply_remote_bids()` не вызывается, `RuntimeMessage::Cbba` существует но не используется. Это делает CBBA неотличимым от centralised planner с другой scoring function.

**Phase 1** закрывает этот gap: CBBA становится по-настоящему распределённым алгоритмом, где агенты обмениваются winning bids через transport/network.

**Источники контекста:** `docs/DRONE_A.5.md`, `docs/DRONE_B.5.md`. INVESTIGATION.md отсутствует.

**Текущее состояние (v0.10):**
- `CbbaAllocator` с `build_bundles()`, `apply_remote_bids()`, `check_convergence()`, `marginal_score()`
- `RuntimeMessage::Cbba` определён в message.rs, dispatch — no-op в node.rs
- CBBA работает через `allocate()` → все agents разделяют один allocator (shared state)
- `apply_remote_bids()` — dead code, нигде не вызывается
- Gossip канал (v0.4) обменивается assignment maps через `send_gossip()`/`apply_gossip_buffer()`
- `Allocator` trait не различает distributed vs local стратегии

**Критерий готовности:**
1. Каждый агент независимо строит bundle (Phase 1) и обменивается winning bids через `RuntimeMessage::Cbba` сообщения (Phase 2).
2. `apply_remote_bids()` вызывается с реальными remote bids, полученными из transport.
3. CBBA convergence: после N раундов обмена пакетами, bundles всех агентов консистентны.
4. `Allocator::is_distributed()` различает distributed (CBBA) от local стратегий для runner logic.
5. Метрики `cbba_rounds_to_convergence`, `cbba_messages` отражают реальные значения.
6. Тест `cbba_fails_without_message_delivery` — CBBA не находит цели при packet_loss=1.0.
7. Proptest `cbba_no_panic_under_random_conditions` — CBBA не паникует при случайных agents/tasks/packet loss/partitions.
8. Все существующие тесты проходят.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.5.md` и `docs/DRONE_B.5.md`:
- DRONE_A.5.md требует: (a) добавить `is_distributed() -> bool` в `Allocator` trait, (b) real message-driven CBBA consensus, (c) proptest.
- Оба документа: CBBA должен отличаться от greedy/auction не только scoring function, но и распределённым consensus loop.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-alloc/src/allocator.rs` | Добавить `is_distributed() -> bool` в `Allocator` trait; Greedy/Auction/CentralizedPlanner → false, CbbaAllocator → true |
| `crates/swarm-alloc/src/cbba.rs` | Per-agent CBBA state; `winning_bids_to_remote()` helper; убрать сброс converged при empty tasks |
| `crates/swarm-runtime/src/node.rs` | `pub cbba: Option<CbbaAllocator>`; `send_cbba_bids()`; расширить CBBA dispatch в process_inbox |
| `crates/swarm-sim/src/runner.rs` | `RunConfig.enable_cbba: bool`; runner-level registry sync после convergence; метрики из AgentNodes |
| `crates/swarm-runtime/src/message.rs` | Без изменений (`Cbba` variant уже существует) |
| `crates/swarm-metrics/src/metrics.rs` | Без изменений (поля уже есть) |
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | Без изменений (CBBA уже в реестре) |
| `README.md` | Обновить: CBBA как distributed consensus |
| `crates/swarm-sim/tests/proptest_cbba.rs` | **NEW** — proptest для CBBA |

---

## Implementation Steps

### Шаг 1 — `is_distributed()` в Allocator trait

Файл: `crates/swarm-alloc/src/allocator.rs`

```rust
pub trait Allocator {
    fn allocate(&mut self, ...) -> Vec<(TaskId, AgentId)>;
    fn allocate_with_connectivity(&mut self, ...) -> Vec<(TaskId, AgentId)> { ... }
    fn allocation_metrics(&self) -> (u64, bool, u64) { (0, false, 0) }

    /// Whether this allocator uses distributed message exchange.
    fn is_distributed(&self) -> bool { false }
}
```

`impl Allocator for CbbaAllocator` → `fn is_distributed(&self) -> bool { true }`.

**Тесты (категория 1):**
- `cbba_is_distributed` — CbbaAllocator.is_distributed() == true
- `greedy_is_not_distributed` — GreedyAllocator.is_distributed() == false

### Шаг 2 — `enable_cbba` в RunConfig

Файл: `crates/swarm-sim/src/runner.rs`

```rust
pub struct RunConfig {
    // ... existing fields ...
    pub enable_cbba: bool,  // NEW, default false
}
```

Все конструкторы `RunConfig` добавить `enable_cbba: false`.

### Шаг 3 — Per-agent CBBA state

Файл: `crates/swarm-runtime/src/node.rs`

```rust
pub struct AgentNode<T> {
    // ... existing fields ...
    pub cbba: Option<CbbaAllocator>,
}
```

Создание: `AgentNode::new()` — `cbba: if config.enable_cbba { Some(CbbaAllocator::default()) } else { None }`.

### Шаг 4 — `send_cbba_bids()`

Файл: `crates/swarm-runtime/src/node.rs`

```rust
impl<T: Transport> AgentNode<T> {
    pub fn send_cbba_bids(&mut self) -> Result<(), T::Error> {
        let Some(ref cbba) = self.cbba else { return Ok(()); };
        let payload = RuntimeMessage::cbba(
            cbba.current_round,
            cbba.winning_bids_to_hashmap(),
            cbba.bundles.get(&self.own_id).cloned().unwrap_or_default(),
        );
        let msg = RawMessage { from: self.own_id.clone(), to: AgentId::from("".to_owned()), payload };
        for peer_id in &self.peer_ids {
            let mut m = msg.clone();
            m.to = peer_id.clone();
            self.transport.send(m)?;
        }
        Ok(())
    }
}
```

Вызов: в `maybe_send_gossip()` после отправки gossip, если `self.cbba.is_some()` → `self.send_cbba_bids()?`.

### Шаг 5 — CBBA dispatch + registry sync

Файл: `crates/swarm-runtime/src/node.rs`

В `process_inbox_and_allocate()` изменить dispatch CBBA ветки:

```rust
Some(RuntimeMessage::Cbba { round: _, winning_bids, sender_bundle: _ }) => {
    if let Some(ref mut cbba) = self.cbba {
        cbba.apply_remote_bids(&[(msg.from.clone(), winning_bids.clone())]);
        // Registry sync: if remote bid claims a task for a remote agent,
        // register the assignment locally so all_tasks_assigned works.
        for (task_id, bid) in winning_bids {
            if self.coordinator.registry.tasks().any(|t| &t.id == &task_id) {
                let _ = self.coordinator.registry.assign(&task_id, bid.agent_id);
            }
        }
    }
}
```

После dispatch вставить CBBA Phase 1:

```rust
// CBBA Phase 1: Bundle building (local to this agent)
if let Some(ref mut cbba) = self.cbba {
    let alive_agents = build_allocation_agents(&self.coordinator.membership);
    let all_tasks = build_allocation_tasks(&self.coordinator.registry);
    cbba.build_bundles(&alive_agents, &all_tasks);
    // Register local assignments
    for (task_id, agent_id) in cbba.current_assignments() {
        if agent_id == self.own_id {
            let _ = self.coordinator.registry.assign(&task_id, agent_id);
        }
    }
}
```

### Шаг 6 — CBBA metric aggregation

Файл: `crates/swarm-sim/src/runner.rs`

После симуляции агрегировать метрики из всех AgentNodes:

```rust
let cbba_nodes: Vec<_> = nodes.iter().filter(|(n, _)| n.cbba.is_some()).collect();
let cbba_rounds_to_convergence = cbba_nodes.iter()
    .filter_map(|(n, _)| n.cbba.as_ref().map(|c| c.current_round as u64))
    .max().unwrap_or(0);
let cbba_converged = cbba_nodes.iter()
    .all(|(n, _)| n.cbba.as_ref().is_some_and(|c| c.converged));
let cbba_messages = cbba_nodes.iter()
    .filter_map(|(n, _)| n.cbba.as_ref().map(|c| c.messages_exchanged))
    .sum();
```

Заменить хардкоженные `cbba_rounds_to_convergence: 0, cbba_converged: false, cbba_messages: 0` на эти вычисления.

### Шаг 7 — Интеграционные тесты + proptest

Файл: `crates/swarm-alloc/src/cbba.rs` — тесты:

- `cbba_converges_via_message_exchange` — 2 агента, direct exchange, convergence
- `cbba_fails_without_message_delivery` — packet_loss=1.0 → CBBA не конвергирует

Файл: `crates/swarm-sim/tests/proptest_cbba.rs` (новый):

```rust
proptest! {
    #[test]
    fn cbba_no_panic_under_random_conditions(
        agents in agent_strategy(2..=8),
        tasks in task_strategy(1..=10),
        packet_loss in 0.0f64..0.5,
    ) {
        let config = RunConfig {
            enable_cbba: true,
            packet_loss_rate: packet_loss,
            ..RunConfig::default()
        };
        ScenarioRunner::run(&scenario, config);
    }
}
```

### Шаг 8 — Обновить README

- CBBA описан как distributed consensus через message exchange
- Документировать `enable_cbba: true` в `RunConfig`

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run -p swarm-examples --bin strategy_comparison
```

---

## Testing Strategy

### Категория 1 — unit тесты

- `cbba_is_distributed` / `greedy_is_not_distributed` — `is_distributed()` на trait
- `cbba_converges_via_message_exchange` — 2 агента, real message exchange, convergence
- `cbba_fails_without_message_delivery` — packet_loss=1.0, CBBA не конвергирует
- `cbba_handles_partition` — partition → convergence after heal
- `cbba_registry_sync` — remote bid регистрируется в локальном registry

### Категория 2 — proptest

- `cbba_no_panic_under_random_conditions` — случайные agents/tasks/packet_loss → no panic

### Категория 3 — тяжёлый (не для Phase 1)

- CBBA на SAR + EmergencyMesh с полным message exchange (через `strategy_comparison`)

### Покрытие gap

- **Gap**: full network topology CBBA (mesh with routing). Текущая модель: point-to-point broadcast. Приемлемо для v0.10.

---

## Risks and Tradeoffs

**1. Per-agent CBBA state vs shared state**

Переход к per-agent state даёт истинную distributed архитектуру. Memory overhead: ~1KB per agent × 5 agents = 5KB — незначительно.

**2. Registry sync через `assign()`**

При получении remote bid, локальный registry получает `assign(task_id, remote_agent_id)`. Это может создать дубликаты (если gossip тоже синхронизирует assignments). `registry.assign()` возвращает Err на дубликат — безопасно.

**3. `is_distributed()` runner check**

Runner должен проверять `is_distributed()` перед запуском CBBA-specific логики. Если CBBA используется но `is_distributed()` не реализован — runner должен пропустить CBBA фазу (graceful degradation).

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| `enable_cbba` не добавлен в RunConfig конструкторы | Компиляция падает во всех сценариях | `cargo check --workspace` |
| `is_distributed()` не реализован в StrategyWrapper | strategy_comparison не использует CBBA distributed logic | `cbba_is_distributed` тест |
| Registry sync создаёт дубликаты | `assign()` возвращает Err → counted as конфликт | `cbba_registry_sync` тест |
| Per-agent convergence расходится | Один агент converged, другой нет | `cbba_handles_partition` тест |

---

## Open Questions

1. **CBBA rounds per tick?** — 1 round = 1 tick. Phase 1 (bundle) + Phase 2 (send bids) в одном тике.
2. **Когда CBBA запускается?** — На каждом тике, если `enable_cbba = true` и `is_distributed()`.
3. **Shared Allocator facade для бенчмарков?** — Оставить `allocate()` facade для `strategy_comparison`, но основной runtime использует per-agent state.
