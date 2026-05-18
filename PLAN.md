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
- `ScenarioRunner` использует two-phase tick: send heartbeats → process inbox

**Критерий готовности:**
1. Каждый агент независимо строит bundle (Phase 1) и обменивается winning bids через `RuntimeMessage::Cbba` сообщения (Phase 2).
2. `apply_remote_bids()` вызывается с реальными remote bids, полученными из transport.
3. CBBA convergence: после N раундов обмена пакетами, bundles всех агентов консистентны.
4. Метрики `cbba_rounds_to_convergence`, `cbba_messages` отражают реальные значения.
5. Тест `cbba_fails_without_message_delivery` — CBBA не находит цели, если сообщения не доставляются (доказывает, что message exchange работает).
6. Proptest `cbba_no_panic_under_random_conditions` — CBBA не паникует при случайных agents/tasks/packet loss/partitions.
7. Все существующие тесты проходят.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.5.md` и `docs/DRONE_B.5.md`:
- Оба документа требуют закрыть CBBA-gap: CBBA должен отличаться от greedy/auction не только scoring function, но и распределённым consensus loop.
- "Если сразу строить publishable benchmark, то CBBA-строка в таблице будет методологически сомнительной."

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-runtime/src/node.rs` | Добавить `send_cbba_bids()` — broadcast winning_bids; расширить dispatch для приёма CBBA сообщений |
| `crates/swarm-alloc/src/cbba.rs` | `CbbaAllocator` остаётся как shared state; метод `build_bundles_for_agent()` для per-agent Phase 1 |
| `crates/swarm-sim/src/runner.rs` | CBBA orchestration: Phase 1 (per-agent bundle building) → Phase 2 (exchange bids → apply_remote_bids) |
| `crates/swarm-runtime/src/message.rs` | Без изменений (`Cbba` variant уже существует) |
| `crates/swarm-metrics/src/metrics.rs` | Без изменений (поля уже есть) |
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | Без изменений (CBBA уже в реестре) |
| `README.md` | Обновить: CBBA как distributed consensus |
| `crates/swarm-sim/tests/proptest_cbba.rs` | **NEW** — proptest для CBBA |

---

## Implementation Steps

### Шаг 1 — Per-agent CBBA state

Перенести CBBA state из `ScenarioRunner` в `AgentNode`. Каждый агент имеет свой экземпляр `CbbaAllocator`:

```rust
pub struct AgentNode<T> {
    // ... existing fields ...
    pub cbba: Option<CbbaAllocator>,
}
```

При создании `AgentNode`: если `config.enable_cbba` — создать `CbbaAllocator::default()`.

**Почему не shared state:** shared state (один CbbaAllocator на все агенты) эквивалентен centralised planner. True CBBA требует per-agent state — каждый агент строит свой bundle независимо, и consensus достигается через message exchange.

### Шаг 2 — `send_cbba_bids()` + `collect_cbba_messages()`

Файл: `crates/swarm-runtime/src/node.rs`

Добавить методы:
```rust
impl<T: Transport> AgentNode<T> {
    /// Broadcast current winning bids to all peers.
    pub fn send_cbba_bids(&mut self) -> Result<(), T::Error> {
        if let Some(ref cbba) = self.cbba {
            let payload = RuntimeMessage::cbba(
                cbba.current_round,
                cbba.winning_bids_to_remote(),
                cbba.bundles.get(&self.own_id).cloned().unwrap_or_default(),
            );
            let msg = RawMessage {
                from: self.own_id.clone(),
                to: AgentId::from("placeholder".to_owned()),
                payload,
            };
            for peer_id in &self.peer_ids {
                let mut m = msg.clone();
                m.to = peer_id.clone();
                self.transport.send(m)?;
            }
        }
        Ok(())
    }
}
```

В `process_inbox_and_allocate()`: изменить dispatch `Cbba` ветки:
```rust
Some(RuntimeMessage::Cbba { round, winning_bids, sender_bundle }) => {
    if let Some(ref mut cbba) = self.cbba {
        cbba.collect_remote_bid(msg.from, winning_bids);
    }
}
```

### Шаг 3 — CBBA round в tick loop

Файл: `crates/swarm-runtime/src/node.rs`

В `process_inbox_and_allocate()` после `allocate_unassigned()`:

```rust
// CBBA Phase 1: Bundle building (local)
if let Some(ref mut cbba) = self.cbba {
    let atasks = build_allocation_tasks(&self.coordinator.registry);
    let aagents = build_allocation_agents(&self.coordinator.membership);
    cbba.build_bundles(&aagents, &atasks);
    
    // Apply CBBA decisions to registry
    for (task_id, agent_id) in cbba.current_assignments() {
        if agent_id == self.own_id {
            // Local decision — register
            let _ = self.coordinator.registry.assign(&task_id, agent_id);
        }
    }
}
```

В `maybe_send_gossip()` (после отправки gossip):

```rust
// CBBA Phase 2: Exchange winning bids via transport
if self.cbba.is_some() {
    self.send_cbba_bids()?;
}
```

### Шаг 4 — Convergence detection per-agent

Файл: `crates/swarm-alloc/src/cbba.rs`

Каждый агент независимо проверяет convergence:
```rust
pub fn check_convergence(&mut self) -> bool {
    // Convergence: winning_bids stable 2 rounds AND all bundles stable
    if self.prev_winning_bids == self.winning_bids && !self.winning_bids.is_empty() {
        self.converged = true;
        return true;
    }
    self.prev_winning_bids = self.winning_bids.clone();
    false
}
```

`collect_remote_bid()`: при получении remote bid обновляет локальные `winning_bids` и сбрасывает `converged = false` если remote bid отличается.

### Шаг 5 — CBBA metric aggregation

Файл: `crates/swarm-sim/src/runner.rs`

После симуляции агрегировать метрики из всех AgentNodes:
- `cbba_rounds_to_convergence` = max rounds across agents
- `cbba_converged` = all agents converged
- `cbba_messages` = total messages exchanged across agents

Добавить helper `pub fn cbba_metrics(&self) -> (u64, bool, u64)` в AgentNode.

### Шаг 6 — Интеграционный тест message delivery

Файл: `crates/swarm-alloc/src/cbba.rs`

Тест `cbba_converges_via_message_exchange`:
- 2 агента, 2 задачи, network с packet_loss=0
- Agent-0 строит bundle, отправляет CBBA сообщение
- Agent-1 получает, строит свой bundle, отправляет ответ
- После N раундов bundles консистентны

### Шаг 7 — Proptest для CBBA

Файл: `crates/swarm-sim/tests/proptest_cbba.rs` (новый)

```rust
proptest! {
    #[test]
    fn cbba_no_panic_under_random_conditions(
        agents in agent_strategy(2..=8),
        tasks in task_strategy(1..=10),
        packet_loss in 0.0f64..0.5,
        partitions in partition_strategy(),
    ) {
        let config = RunConfig {
            packet_loss_rate: packet_loss,
            partition_events: partitions,
            enable_cbba: true,
            ..RunConfig::default_for_cbba()
        };
        let scenario = Scenario { agents, tasks, ... };
        let result = ScenarioRunner::run(&scenario, config);
        // No panic — invariant check
    }
}
```

**Тесты (категория 1):**
- `cbba_converges_via_message_exchange` — 2 агента, bid exchange, convergence
- `cbba_fails_without_message_delivery` — packet_loss=1.0 → CBBA не сходится
- `cbba_handles_partition` — partition во время rounds → convergence after heal
- `cbba_message_serde_roundtrip` — обновить существующий тест
- `cbba_no_panic_under_random_conditions` — proptest (категория 2/3)

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

- `cbba_converges_via_message_exchange` — message-driven convergence
- `cbba_fails_without_message_delivery` — без доставки сообщений CBBA не сходится
- `cbba_handles_partition` — partition → convergence after heal
- `cbba_per_agent_state_independent` — два агента с разными bundles → сходятся через exchange

### Категория 2 — proptest

- `cbba_no_panic_under_random_conditions` — случайные agents/tasks/packet_loss/partitions → no panic

### Категория 3 — тяжёлый (не для Phase 1)

- CBBA на SAR + EmergencyMesh с message exchange (через `strategy_comparison`)

---

## Risks and Tradeoffs

**1. Per-agent CBBA state vs shared state**

Переход от shared state к per-agent state увеличивает memory (N экземпляров CbbaAllocator), но даёт истинную distributed архитектуру. Shared state facade остаётся для бенчмарков через `allocate()`.

**2. CBBA messages vs gossip messages**

CBBA winning_bids идут через тот же transport что heartbeats и gossip. При большом числе агентов может создавать contention. Для v0.10 с ~5 агентами — приемлемо.

**3. Convergence detection per-agent**

Каждый агент независимо проверяет convergence. Это более реалистично чем centralised convergence check. Но метрика `cbba_converged` требует all-agents agreement.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| `CbbaAllocator` в каждом AgentNode | Удвоение памяти, но ~5 агентов — незначительно | `cargo test --workspace` |
| CBBA сообщения через gossip канал | Контенция с heartbeats/gossip | benchmark timing |
| `send_cbba_bids()` каждый CBBA-тик | N² messages per round | CBBA messages метрика |
| Per-agent convergence может расходиться | Один агент считает CBBA converged, другой нет | Тест `cbba_handles_partition` |

---

## Open Questions

1. **CBBA rounds per tick?** — 1 round = 1 tick. Phase 1 + Phase 2 в одном тике.
2. **Когда CBBA запускается?** — На каждом тике, если `enable_cbba = true`.
3. **Shared CbbaAllocator wrapper для бенчмарков?** — Оставить `allocate()` facade для `strategy_comparison`, но основной runtime использует per-agent state.
