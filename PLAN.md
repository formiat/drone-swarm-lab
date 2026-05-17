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
1. `CbbaAllocator` реализует `Allocator` trait — встраивается в существующий `ScenarioRunner` через тот же интерфейс.
2. CBBA сходится: после N раундов (тиков) все агенты приходят к одному assignment (bundles консистентны).
3. CBBA работает на SAR и EmergencyMesh сценариях.
4. `strategy_comparison` расширен: 5 стратегий (Greedy, Auction, ConnectivityAware, CentralizedPlanner, CBBA) на 2 миссиях.
5. Метрики: `cbba_rounds_to_convergence`, `cbba_messages_per_round`.
6. Все существующие тесты проходят.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.3.md` и `docs/DRONE_B.3.md`:
- DRONE_A.3: CBBA — это отдельная стратегия в `swarm-alloc`, message/round model, сравнение с другими стратегиями, запуск на SAR + EmergencyMesh.
- DRONE_B.3: после Milestone 9 у проекта появляется публикуемый результат — сравнение 4+ стратегий на reference missions с 1000 seeds. CBBA закрывает gap "настоящий распределённый алгоритм".

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-alloc/src/cbba.rs` | **NEW** — `CbbaAllocator` с round-based bidding |
| `crates/swarm-alloc/src/allocator.rs` | Расширить `AllocationAgent`: добавить `bundle: Vec<TaskId>`, `winning_bids` для CBBA |
| `crates/swarm-alloc/src/strategy.rs` | Добавить `CbbaAllocator` в `StrategyRegistry` |
| `crates/swarm-alloc/src/lib.rs` | Export `CbbaAllocator` |
| `crates/swarm-runtime/src/node.rs` | CBBA round integration: после allocate вызывать `cbba.run_round()` |
| `crates/swarm-metrics/src/metrics.rs` | Новые поля: `cbba_rounds_to_convergence`, `cbba_messages_per_round` |
| `crates/swarm-scenarios/src/sar_scenario.rs` | Опционально: SAR-specific CBBA parameters |
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | Добавить CBBA в реестр |
| `README.md` | Обновить статус до Milestone 10 |

---

## Implementation Steps

### Шаг 1 — CBBA типы и состояние

Файл: `crates/swarm-alloc/src/cbba.rs` (новый)

CBBA — итеративный аукцион с двумя фазами на каждом раунде:

**Фаза 1 — Bundle Building** (каждый агент локально):
- Из не-assigned задач выбрать ту, что даёт максимальный marginal score
- Marginal score = score(agent, task с учётом уже имеющегося bundle) - score без задачи
- Добавить задачу в bundle если маржинально выгодно и не превышен `max_bundle_size`

**Фаза 2 — Consensus** (между агентами):
- Обмен winning bids через gossip (reuse существующего gossip канала)
- Если remote bid на задачу выше локального → удалить задачу из bundle
- Если remote bid на задачу ниже → оставить (конфликт разрешён)
- После consensus — пересчитать bundle assignments

```rust
pub struct CbbaConfig {
    pub max_bundle_size: usize,    // max tasks per agent
    pub max_rounds: u32,           // convergence rounds before forced stop
    pub score_weight_distance: f64,
    pub score_weight_battery: f64,
}

impl Default for CbbaConfig {
    fn default() -> Self {
        Self {
            max_bundle_size: 5,
            max_rounds: 20,
            score_weight_distance: 1.0,
            score_weight_battery: 0.5,
        }
    }
}

pub struct CbbaAllocator {
    config: CbbaConfig,
    /// Per-agent state persisted across rounds: (agent_id -> bundle, winning_bids)
    bundles: HashMap<AgentId, Vec<TaskId>>,
    winning_bids: HashMap<TaskId, (AgentId, f64)>, // (winner, bid_value)
    current_round: u32,
    converged: bool,
}
```

**Тесты (категория 1):**
- `cbba_config_defaults` — проверка дефолтных значений
- `cbba_score_distance` — дальняя задача получает меньший score
- `cbba_bundle_capped_by_max_size` — bundle не превышает max_bundle_size

---

### Шаг 2 — CBBA score function

Файл: `crates/swarm-alloc/src/cbba.rs`

```rust
impl CbbaAllocator {
    fn score(&self, agent: &AllocationAgent, task: &Task, existing_bundle: &[TaskId]) -> f64 {
        // Distance cost: closer = higher score
        let task_pose = task.pose.unwrap_or(Pose { x: 0.0, y: 0.0 });
        let dist = agent.pose.distance_to(&task_pose);
        let distance_score = -self.config.score_weight_distance * dist;

        // Battery bonus: higher battery = higher score
        let battery_score = self.config.score_weight_battery * agent.battery;

        // Capability gate
        if !has_all_capabilities(agent, &task.required_capabilities)
            || !has_required_role(agent, &task.required_role)
            || agent.battery <= 0.0
        {
            return f64::NEG_INFINITY;
        }

        distance_score + battery_score
    }
}
```

---

### Шаг 3 — Интеграция CBBA в tick loop

Файл: `crates/swarm-runtime/src/node.rs`

CBBA работает **поверх** существующего tick loop. Каждый тик:
1. Heartbeat + gossip (уже есть)
2. Если `enable_cbba` и allocator — CBBA → запустить `cbba.run_round(tasks, agents)`
3. Вернуть decisions из CBBA как результаты allocation

```rust
impl Allocator for CbbaAllocator {
    fn allocate(
        &mut self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)> {
        self.run_round(tasks, agents)
    }
}
```

`run_round()`:
1. Если `current_round == 0` — инициализировать bundles (пустые)
2. **Фаза 1 (Bundle Building)**: для каждого агента добавить лучшую доступную задачу
3. **Фаза 2 (Consensus)**: синхронизировать winning_bids между агентами через gossip
4. `current_round += 1`
5. Если все bundles стабильны (не менялись 2 раунда подряд) → `converged = true`
6. Вернуть текущие assignment decisions

**Тесты (категория 1):**
- `cbba_round_assignments_converge` — после N раундов bundles консистентны

---

### Шаг 4 — CBBA через gossip канал

CBBA использует существующий gossip канал из v0.4 для обмена winning bids. Формат сообщения:

```json
{
  "type": "cbba",
  "round": 3,
  "winning_bids": { "task-0": ["agent-1", 42.5], "task-1": ["agent-2", 38.0] },
  "sender_bundle": ["task-3", "task-5"]
}
```

Добавить вариант `Cbba` в `RuntimeMessage` enum:
```rust
pub enum RuntimeMessage {
    Heartbeat { ... },
    Gossip { ... },
    Cbba { round: u32, winning_bids: HashMap<TaskId, (AgentId, f64)>, sender_bundle: Vec<TaskId> },
}
```

В `process_inbox_and_allocate()`: dispatch Cbba сообщений в `cbba.apply_remote_bids(...)`.

**Тесты (категория 1):**
- `cbba_message_serde_roundtrip`

---

### Шаг 5 — Метрики CBBA

Файл: `crates/swarm-metrics/src/metrics.rs`

```rust
#[serde(default)]
pub cbba_rounds_to_convergence: u64,   // rounds until all bundles stable
#[serde(default)]
pub cbba_messages_per_round: u64,      // CBBA messages exchanged per tick
#[serde(default)]
pub cbba_converged: bool,              // whether CBBA reached consensus
```

---

### Шаг 6 — CBBA в strategy_comparison

Файл: `crates/swarm-alloc/src/strategy.rs`

Добавить `impl Strategy for CbbaAllocator`:
```rust
impl Strategy for CbbaAllocator {
    fn name(&self) -> &'static str { "cbba" }
    fn description(&self) -> &'static str {
        "Consensus-Based Bundle Algorithm — distributed auction with bundle building"
    }
}
```

Добавить в `StrategyRegistry::default()`:
```rust
reg.register(Box::new(CbbaAllocator::default()));
```

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

CBBA автоматически включается через реестр. Проверить:
```bash
cargo run -p swarm-examples --bin strategy_comparison -- --json results.json
```

Ожидается: 5 стратегий в выводе (greedy, auction, connectivity-aware, centralized, cbba).

---

### Шаг 7 — CBBA на SAR и EmergencyMesh

CBBA работает через тот же `Allocator` trait — автоматически применим ко всем сценариям. Специфичных изменений в сценариях не требуется.

Для SAR: CBBA решает task-to-agent matching итеративно. Должен показывать лучшую `probability_of_detection` чем greedy (за счёт учёта расстояния и батареи в score).

Для EmergencyMesh: CBBA учитывает relay placement через connectivity-aware расширение `allocate_with_connectivity`.

**Тесты (категория 2):**
- `cbba_on_sar_finds_targets` — запуск SAR с CBBA, хотя бы 1 цель найдена
- `cbba_on_emergency_mesh_maintains_availability` — EmergencyMesh с CBBA, network_availability > threshold

---

### Шаг 8 — Обновить README.md

- Добавить **Milestone 10** в `## Current Status`
- Документировать CBBA: distributed auction, message/round model, 5 стратегий
- Обновить `strategy_comparison` пример вывода

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

**`swarm-alloc` — CBBA core (3 теста):**
- `cbba_config_defaults`
- `cbba_score_distance`
- `cbba_bundle_capped_by_max_size`

**`swarm-alloc` — CBBA rounds (1 тест):**
- `cbba_round_assignments_converge`

**`swarm-runtime` — CBBA message (1 тест):**
- `cbba_message_serde_roundtrip`

**`swarm-metrics` — CBBA metrics (1 тест):**
- `cbba_metrics_populated`

**Регрессия:** все существующие ~160 тестов должны пройти. CBBA не изменяет существующие allocators.

### Категория 2 — Лёгкий рефакторинг (интеграционные)

- `cbba_on_sar_finds_targets` — SAR + CBBA, проверка нахождения целей
- `cbba_on_emergency_mesh_maintains_availability` — EmergencyMesh + CBBA
- `cbba_vs_greedy_on_small_scenario` — сравнение результатов CBBA и Greedy на фиксированном seed

### Категория 3 — Тяжёлый (не для v0.10)

- **1000 seeds comparison**: 5 стратегий × SAR × 1000 seeds. Требует значительного времени выполнения. Реализовано через `strategy_comparison`, но не как автотест.
- **Property-based CBBA**: случайные топологии и task distributions. v0.11.

### Покрытие gap

- **Gap**: convergence proof (TLA+). Не в scope v0.10.
- **Gap**: CBBA с реальным message loss. Gossip канал уже обрабатывает message loss из v0.4, CBBA поверх него наследует эту обработку. Но специфичный retransmission для CBBA bids не реализован.
- **Gap**: dynamic task injection во время CBBA rounds. Текущий CBBA работает со статичным task set; динамические задачи требуют перезапуска раундов. v0.11.

---

## Risks and Tradeoffs

**1. CBBA vs gossip convergence**

CBBA требует обмена winning bids между агентами. Существующий gossip канал (v0.4) уже обменивается assignment maps. CBBA добавляет второй канал обмена (winning bids). Риск: дублирование функциональности. Митигация: gossip обменивается assignment maps для anti-entropy, CBBA bids — для distributed auction, разные цели.

**2. `Allocator::allocate(&self)` vs `&mut self`**

Существующие allocators (Greedy, Auction) — stateless (`&self`). CBBA требует mutable state (bundles, winning_bids). `Allocator::allocate` принимает `&self`. Требуется либо изменить сигнатуру trait на `&mut self`, либо использовать `RefCell`/`Mutex` внутри CBBA. Рекомендация: изменить trait на `&mut self` — breaking change для всех allocators, но минимальный (параметры не меняются).

**3. CBBA convergence time**

CBBA сходится за O(N²) раундов в худшем случае. При max_rounds=20 и 5 агентах — практично. При 20+ агентах может не успеть сойтись за max_ticks. Митигация: `max_rounds` конфигурируем, при превышении возвращаем best-effort assignment.

**4. CBBA + movement interaction**

Агенты двигаются каждый тик. Pose меняется → score меняется → bundles могут перестраиваться. Это ожидаемое поведение для SAR (агенты переоценивают задачи по мере движения). Gossip должен корректно обрабатывать changing bids.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| `Allocator::allocate` сигнатура меняется на `&mut self` | Все существующие allocators (Greedy, Auction, ConnectivityAware, CentralizedPlanner) требуют `&mut self` | `cargo check --workspace` + `cargo test --workspace` |
| `Cbba` вариант в `RuntimeMessage` | Gossip/Heartbeat dispatch не ломается | `dispatch_gossip_does_not_affect_heartbeat_senders` тест |
| CBBA добавляет latency в tick loop | Симуляция замедляется | benchmark: время SAR сценария не должно вырасти >50% |
| `Cargo.lock` изменился | Должен быть включён в commit | `git diff --stat` |

---

## Open Questions

1. **`&self` vs `&mut self` для `Allocator::allocate`?**
   - `&self` — сохраняет backward compat, но требует internal mutability для CBBA
   - `&mut self` — ломает все существующие allocators, но чище концептуально
   - Рекомендация: `&mut self` — CBBA естественно требует mutable state, существующие allocators просто добавляют `mut`

2. **CBBA через gossip или отдельный transport?**
   - Gossip — reuse существующей инфраструктуры, нет нового transport
   - Отдельный transport — изоляция, но overhead
   - Рекомендация: через gossip (добавить `Cbba` variant в `RuntimeMessage`)

3. **Нужен ли отдельный `CbbaMessage` тип или reuse `RawMessage`?**
   - `RawMessage` — уже используется для gossip/heartbeat
   - `CbbaMessage` — более типобезопасно
   - Рекомендация: добавить `Cbba` variant в `RuntimeMessage` enum (консистентно с `Heartbeat` и `Gossip`)

4. **`max_bundle_size` — жёсткий лимит или soft?**
   - Жёсткий: агент не может взять больше N задач (реалистично — battery/comms ограничивают)
   - Soft: агент может взять больше если выгодно
   - Рекомендация: жёсткий с default=5
