# PLAN: Milestone 10 — CBBA (Consensus-Based Bundle Algorithm)

## Context

Milestones 1-9 построили coordination runtime с физической моделью, connectivity-aware allocation, и reference миссией SAR. Четыре стратегии работают: Greedy, Auction, ConnectivityAware, CentralizedPlanner. Но все они принимают решение **локально** на одном агенте — это не распределённый консенсус.

**Milestone 10 (v0.10)** добавляет CBBA — первый по-настоящему распределённый алгоритм, где агенты обмениваются bids через сообщения и итеративно сходятся к консистентному task assignment без центрального координатора. После Milestone 10 проект имеет публикуемый результат: сравнение 5 стратегий на 2 reference missions.

**Источники контекста:** `docs/DRONE_A.3.md` (CBBA как алгоритмический шаг после SAR), `docs/DRONE_B.3.md` (CBBA + сравнение на SAR/EmergencyMesh — "публикуемый результат"). INVESTIGATION.md отсутствует.

**Текущее состояние (v0.9):**
- 4 стратегии: GreedyAllocator, AuctionAllocator, ConnectivityAwareAllocator, CentralizedPlanner
- `Allocator` trait: `allocate(tasks, agents) -> Vec<(TaskId, AgentId)>`, `allocate_with_connectivity`
- `Strategy` trait: `name()` + `description()` на базе `Allocator`
- Gossip/anti-entropy из v0.4 — агенты уже обмениваются assignment maps
- SAR (v0.9) и EmergencyMesh (v0.5) reference миссии
- `strategy_comparison` binary с JSON/CSV export

**Критерий готовности:**
1. `CbbaAllocator` работает в `ScenarioRunner` — каждый тик одна CBBA-итерация (Phase 1 + Phase 2), assignment decisions применяются.
2. CBBA сходится: после N раундов (тиков) все агенты имеют консистентные winning_bids и bundles.
3. CBBA работает на SAR и EmergencyMesh сценариях.
4. `strategy_comparison` расширен: 5 стратегий на 2 миссиях.
5. Метрики: `cbba_rounds_to_convergence`, `cbba_messages_per_round`, `cbba_converged`.
6. Все существующие тесты проходят.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.3.md` и `docs/DRONE_B.3.md`:
- DRONE_A.3: CBBA — отдельная стратегия в `swarm-alloc`, message/round model, сравнение с другими стратегиями, запуск на SAR + EmergencyMesh.
- DRONE_B.3: после Milestone 9 у проекта появляется публикуемый результат — сравнение 4+ стратегий на reference missions с 1000 seeds. CBBA закрывает gap "настоящий распределённый алгоритм".

---

## CBBA Architectural Integration

CBBA не вписывается в per-agent вызов `allocate()` — ему нужна shared state и round coordination между агентами. Решение: CBBA работает как **обёртка над tick loop**, управляемая `ScenarioRunner`.

### Shared state model

`CbbaAllocator` — единственный экземпляр на всю симуляцию, хранится в `ScenarioRunner::run_internal()`. Он содержит `bundles: HashMap<AgentId, Vec<TaskId>>` и `winning_bids: HashMap<TaskId, (AgentId, f64)>`. Состояние переживает между тиками.

### Tick loop flow (один тик = один CBBA раунд)

```
1. Heartbeat phase (send + poll) — как обычно
2. Gossip phase (send + receive) — как обычно
3. CBBA Phase 1 (Bundle Building) — локально для каждого агента:
   - Для каждого агента: вычислить marginal score для не-assigned задач
   - Если есть задача с положительным marginal score и bundle не полон → добавить
4. CBBA Phase 2 (Consensus) — через gossip:
   - Каждый агент отправляет свои winning_bids через RuntimeMessage::Cbba
   - При получении remote bids: обновить локальные winning_bids
   - Если remote bid выше локального на ту же задачу → удалить задачу из bundle
5. Allocation: после CBBA, применить решения: release tasks от проигравших агентов, assign tasks победителям
6. Movement + scan (как обычно)
```

`ScenarioRunner` отвечает за шаги 3-5. `AgentNode` не знает о CBBA — он продолжает работать с обычным allocation (шаг 5).

### Convergence detection

После каждого раунда: если `winning_bids` не изменились за последние 2 раунда → `converged = true`. При `converged == true` CBBA пропускает Phase 1/2 (bundles финальны). При `current_round >= max_rounds` → принудительная остановка с текущими assignments.

### Почему не через `Allocator::allocate()`?

Существующие allocators stateless — `allocate()` берёт tasks/agents и возвращает decisions. CBBA требует:
1. Shared state между агентами (winning_bids)
2. Round-based итерации (не single-shot decision)
3. Обмена сообщениями через gossip между агентами

Поэтому CBBA управляется `ScenarioRunner` напрямую, а не через `allocator.allocate()` в `AgentNode::allocate_unassigned()`. Однако для `strategy_comparison` CBBA реализует `Strategy` trait facade — `allocate()` вызывает `run_round()` как single-shot (для бенчмарка без изменения runner).

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-alloc/src/cbba.rs` | **NEW** — `CbbaAllocator`: bundles, winning_bids, score, run_round() |
| `crates/swarm-alloc/src/allocator.rs` | `Allocator::allocate` меняет сигнатуру `&self` → `&mut self` |
| `crates/swarm-alloc/src/connectivity_aware.rs` | Allocate вызов (1 site) → `&mut self` |
| `crates/swarm-alloc/src/centralized.rs` | Allocate внутри `new` (1 site) → `&mut self` |
| `crates/swarm-alloc/src/strategy.rs` | `Strategy` trait → `&mut self`; добавить `CbbaAllocator` в `StrategyRegistry` |
| `crates/swarm-alloc/src/lib.rs` | Export `CbbaAllocator` |
| `crates/swarm-runtime/src/message.rs` | Добавить `Cbba` variant в `RuntimeMessage` enum |
| `crates/swarm-runtime/src/node.rs` | `allocate_unassigned` call → `&mut self` (1 site) |
| `crates/swarm-sim/src/runner.rs` | CBBA round orchestration: Phase 1, Phase 2, convergence check, assignment apply |
| `crates/swarm-sim/src/benchmark.rs` | Allocate calls (2 sites) → `&mut self` |
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | Allocate calls (2 sites) → `&mut self` |
| `crates/swarm-metrics/src/metrics.rs` | Новые поля: `cbba_rounds_to_convergence`, `cbba_converged`, `cbba_messages` |
| `README.md` | Обновить статус до Milestone 10 |

Всего ~28 call sites в 7 файлах меняют `&self` на `&mut self` для `Allocator::allocate()`:
- `swarm-alloc/src/allocator.rs` — 4 impl sites (Greedy, Auction, trait default)
- `swarm-alloc/src/centralized.rs` — 1 impl site + 1 internal call
- `swarm-alloc/src/connectivity_aware.rs` — 1 impl site + internal calls
- `swarm-alloc/src/strategy.rs` — 3 impl sites (Greedy, Auction, ConnectivityAware)
- `swarm-runtime/src/node.rs` — 1 call site (allocate_unassigned)
- `swarm-sim/src/benchmark.rs` — 3 call sites
- `swarm-examples/src/bin/strategy_comparison.rs` — 3 call sites

---

## Implementation Steps

### Шаг 1 — CBBA типы, состояние, и score function

Файл: `crates/swarm-alloc/src/cbba.rs` (новый)

```rust
pub struct CbbaConfig {
    pub max_bundle_size: usize,    // max tasks per agent (default 5)
    pub max_rounds: u32,           // convergence rounds before forced stop (default 20)
    pub score_weight_distance: f64, // weight for distance in score (default 1.0)
    pub score_weight_battery: f64, // weight for battery in score (default 0.5)
}

pub struct CbbaAllocator {
    config: CbbaConfig,
    bundles: HashMap<AgentId, Vec<TaskId>>,
    winning_bids: HashMap<TaskId, (AgentId, f64)>,
    prev_winning_bids: HashMap<TaskId, (AgentId, f64)>,
    current_round: u32,
    converged: bool,
}
```

**Score function (marginal, с учётом позиции в bundle):**

```rust
fn marginal_score(&self, agent: &AllocationAgent, task: &Task, bundle: &[TaskId]) -> f64 {
    // Base score: distance + battery
    let task_pose = task.pose.unwrap_or(Pose { x: 0.0, y: 0.0 });
    let dist = agent.pose.distance_to(&task_pose);
    let base = -self.config.score_weight_distance * dist
               + self.config.score_weight_battery * agent.battery;

    // Capability gate
    if !has_all_capabilities(agent, &task.required_capabilities)
        || !has_required_role(agent, &task.required_role)
        || agent.battery <= 0.0
    {
        return f64::NEG_INFINITY;
    }

    // Marginal penalty: task further from the END of existing bundle costs more
    // (TSP-like incremental travel distance)
    if let Some(&last_task_id) = bundle.last() {
        // last_task pose determines start of next leg
        // simplified: apply distance penalty proportional to bundle position
        let position_penalty = bundle.len() as f64 * 0.1 * dist;
        return base - position_penalty;
    }
    base
}
```

Пояснение: когда агент уже имеет N задач в bundle, назначение (N+1)-й задачи должно учитывать дополнительное расстояние от последней задачи в bundle до новой. Упрощённо: penalty пропорционален позиции в bundle × distance. Полноценный TSP-ordering — v0.11.

**Тесты (категория 1):**
- `cbba_config_defaults`
- `cbba_score_distance` — дальняя задача → меньший score
- `cbba_bundle_position_penalty` — задача дальше от последней в bundle → меньший marginal score
- `cbba_bundle_capped_by_max_size`

---

### Шаг 2 — `Allocator::allocate(&self)` → `&mut self`

Файлы: все перечисленные в Affected Components таблице (28 call sites, 7 файлов)

```rust
pub trait Allocator {
    fn allocate(
        &mut self,           // WAS: &self
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)>;

    fn allocate_with_connectivity(
        &mut self,           // WAS: &self
        ...
    ) -> Vec<(TaskId, AgentId)> { ... }
}
```

Для существующих stateless allocators (Greedy, Auction, CentralizedPlanner, ConnectivityAware): просто добавить `mut` в сигнатуры, без изменения логики.

Для `Strategy` trait: также изменить `&self` → `&mut self` на `allocate`.

**Тесты:** все существующие ~160 тестов должны пройти (только сигнатуры меняются).

---

### Шаг 3 — `Cbba` variant в `RuntimeMessage`

Файл: `crates/swarm-runtime/src/message.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuntimeMessage {
    #[serde(rename = "hb")]
    Heartbeat { sender_tick: u64, generation: u64 },
    #[serde(rename = "gossip")]
    Gossip { assignments: HashMap<TaskId, AgentId>, generations: HashMap<AgentId, u64> },
    #[serde(rename = "cbba")]
    Cbba {
        round: u32,
        winning_bids: HashMap<TaskId, (AgentId, f64)>,
        sender_bundle: Vec<TaskId>,
    },
}
```

В `process_inbox_and_allocate()`: dispatch `Cbba` сообщений в буфер `cbba_buffer: Vec<RuntimeMessage>` (рядом с `gossip_buffer`).

**Тесты (категория 1):**
- `cbba_message_serde_roundtrip`
- `dispatch_cbba_does_not_affect_heartbeat_senders` — голосовое сообщение CBBA не считается heartbeat

---

### Шаг 4 — CBBA round orchestration в ScenarioRunner

Файл: `crates/swarm-sim/src/runner.rs`

В `run_internal()`:
1. Создать `let mut cbba = config.cbba_allocator;` (Option-обёртка — None если не CBBA)
2. В tick loop, после gossip dispatch (Phase 1 heartbeat + Phase 2 process), добавить CBBA block:

```rust
if let Some(ref mut cbba) = cbba {
    // Phase 1: Bundle Building (local to each agent)
    cbba.build_bundles(&agents, &tasks);

    // Phase 2: Consensus (exchange winning_bids via gossip)
    // CBBA messages are already dispatched and buffered in cbba_buffer
    cbba.apply_remote_bids(&cbba_buffer);

    // Apply CBBA assignment decisions
    let decisions = cbba.current_assignments();
    for (task_id, agent_id) in decisions {
        // Release previous owner if different
        // Assign task to winning agent
    }

    // Check convergence
    if cbba.check_convergence() {
        cbba.converged = true;
    }
    cbba.current_round += 1;
}
```

3. Метрики CBBA собираются из `cbba` в конце симуляции:
```rust
cbba_rounds_to_convergence: cbba.current_round as u64,
cbba_converged: cbba.converged,
cbba_messages: cbba.messages_exchanged,
```

**Тесты (категория 1):**
- `cbba_round_assignments_converge` — после N раундов winning_bids стабильны

---

### Шаг 5 — Метрики CBBA

Файл: `crates/swarm-metrics/src/metrics.rs`

```rust
#[serde(default)] pub cbba_rounds_to_convergence: u64,
#[serde(default)] pub cbba_converged: bool,
#[serde(default)] pub cbba_messages: u64,
```

---

### Шаг 6 — CBBA в strategy_comparison

Файл: `crates/swarm-alloc/src/strategy.rs`

```rust
impl Strategy for CbbaAllocator {
    fn name(&self) -> &'static str { "cbba" }
    fn description(&self) -> &'static str {
        "Consensus-Based Bundle Algorithm — distributed auction with bundle building"
    }
}
```

В `StrategyRegistry::default()`:
```rust
reg.register(Box::new(CbbaAllocator::default()));
```

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

CBBA автоматически включается через реестр (5 стратегий: greedy, auction, connectivity-aware, centralized, cbba).

**Тесты (категория 2):**
- `cbba_vs_greedy_on_small_scenario` — CBBA и Greedy дают разные assignments на фиксированном seed

---

### Шаг 7 — CBBA на SAR и EmergencyMesh

CBBA работает во всех сценариях без изменений (управляется `ScenarioRunner`). Для SAR: CBBA решает task-to-agent matching итеративно, с учётом расстояния и батареи.

**Тесты (категория 2):**
- `cbba_on_sar_finds_targets` — SAR + CBBA, хотя бы 1 цель найдена
- `cbba_on_emergency_mesh_maintains_availability` — EmergencyMesh + CBBA, network_availability > threshold

---

### Шаг 8 — Обновить README.md

- Добавить **Milestone 10** в `## Current Status`
- CBBA: distributed auction, message/round model, 5 стратегий в `strategy_comparison`
- Обновить пример вывода `strategy_comparison`

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run -p swarm-examples --bin strategy_comparison -- --json /tmp/cbba_results.json
cargo run -p swarm-examples --bin sar_scenario
cargo run -p swarm-examples --bin emergency_mesh_scenario
```

---

## Testing Strategy

### Категория 1 — Без рефакторинга (unit тесты)

**`swarm-alloc` — CBBA core (4 теста):**
- `cbba_config_defaults`
- `cbba_score_distance`
- `cbba_bundle_position_penalty`
- `cbba_bundle_capped_by_max_size`

**`swarm-alloc` — CBBA rounds (3 теста):**
- `cbba_round_assignments_converge` — N раундов → bundles консистентны
- `cbba_does_not_exceed_max_rounds` — convergence detection срабатывает до max_rounds
- `cbba_conflicting_bids_resolution` — два агента bid-ят на одну задачу, побеждает higher bid

**`swarm-runtime` — CBBA message (2 теста):**
- `cbba_message_serde_roundtrip`
- `dispatch_cbba_does_not_affect_heartbeat_senders`

**`swarm-metrics` — CBBA metrics (1 тест):**
- `cbba_metrics_populated`

**`swarm-alloc` — &mut self regression (0 новых тестов — проверяется существующими 160+)**

**Регрессия:** все существующие ~160 тестов должны пройти с `&mut self`.

### Категория 2 — Лёгкий рефакторинг (интеграционные)

- `cbba_vs_greedy_on_small_scenario` — CBBA даёт другой assignment чем Greedy (но не хуже)
- `cbba_on_sar_finds_targets` — SAR + CBBA, хотя бы 1 цель найдена
- `cbba_on_emergency_mesh_maintains_availability` — EmergencyMesh + CBBA
- `cbba_handles_partition` — partition во время CBBA раундов → convergence после heal

### Категория 3 — Тяжёлый (не для v0.10)

- 1000 seeds comparison: 5 стратегий × SAR × 1000 seeds. Через `strategy_comparison`.
- Property-based CBBA: случайные топологии. v0.11.
- TSP-ordering в bundles: полноценный sequential task ordering. v0.11.

### Покрытие gap

- **Gap**: CBBA с message loss (retransmission). Gossip канал наследует message loss из v0.4, CBBA получает degraded performance при loss > 0. Специфичный CBBA retransmission не реализован — v0.11.
- **Gap**: dynamic task injection во время CBBA rounds. Требует перезапуска раундов. v0.11.
- **Gap**: convergence proof (TLA+). Не в scope v0.10.

---

## Risks and Tradeoffs

**1. `&self` → `&mut self` для всего `Allocator` trait**

Breaking change затрагивает 28 call sites в 7 файлах. Но изменение механическое (добавить `mut`), без изменения логики. Все существующие тесты должны пройти.

**2. CBBA управляется ScenarioRunner, не AgentNode**

CBBA требует shared state и round coordination → не может быть per-agent allocator. Архитектурно: `ScenarioRunner` оркестрирует CBBA как отдельную фазу в tick loop. Это добавляет coupling между runner и CBBA, но изолирует CBBA от AgentNode.

**3. CBBA + movement interaction**

Агенты двигаются каждый тик → pose меняется → score меняется → bundles могут перестраиваться. Gossip handle changing bids — агенты обновляют winning_bids на каждом раунде.

**4. CBBA convergence при partition**

При network partition (v0.4) агенты не обмениваются CBBA сообщениями → bundles расходятся. После heal — gossip восстанавливает connectivity → CBBA пересчитывает consensus. Тест `cbba_handles_partition` проверяет этот сценарий.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| `&self` → `&mut self` во всех allocators | Не компилируются существующие стратегии | `cargo check --workspace` + `cargo test --workspace` |
| CBBA `allocate()` facade для strategy_comparison | `strategy_comparison` не видит CBBA или падает | `cargo run --bin strategy_comparison -- --json /tmp/test.json` |
| `Cbba` variant в `RuntimeMessage` | Gossip dispatch ломается | `dispatch_cbba_does_not_affect_heartbeat_senders` |
| CBBA round orchestration в runner | Не ломает существующие сценарии (без CBBA) | `cargo run --bin coverage_with_failure` |
| `Cargo.lock` изменился | Должен быть включён в commit | `git diff --stat` |

---

## Open Questions

1. **CBBA как отдельная фаза tick loop vs как allocator facade?**
   - Отдельная фаза: runner оркестрирует, CBBA state в runner. Чище, но добавляет coupling.
   - Allocator facade: CBBA выглядит как обычный allocator для `strategy_comparison`, но внутри `allocate()` делает single-shot round. Проще для бенчмарков.
   - Рекомендация: runner оркестрирует CBBA (отдельная фаза). Facade `allocate()` для `strategy_comparison` вызывает `run_round()` как single-shot.

2. **Нужен ли полный TSP-ordering в bundles для v0.10?**
   - Да — более точная модель, но сложная реализация.
   - Нет — marginal penalty по позиции в bundle (Step 1) достаточно для v0.10.
   - Рекомендация: marginal penalty для v0.10, TSP — v0.11.

3. **CBBA сообщения — push (broadcast) или pull (poll)?**
   - Push: агент отправляет bids всем peer_ids каждый раунд. Проще, но O(N²) messages.
   - Pull: агент запрашивает bids у конкретных peers. Сложнее.
   - Рекомендация: push через gossip (каждый агент broadcast-ит свои winning_bids).
