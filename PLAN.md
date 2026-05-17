# PLAN: Milestone 8 — Kinematic + Battery Foundation

## Context

Milestones 1-4 построили coordination runtime: membership, failure detection, task allocation, pluggable transport, partitions, gossip/convergence.

**Milestone 8 (v0.8)** добавляет физическую модель движения и батареи, превращая абстрактных неподвижных агентов в движущиеся ресурсы с ограниченной энергией. Это — foundation для всех будущих reference missions (SAR, Inspection, Emergency Mesh).

**Источники контекста:** `DRONE_A.3.md` (Milestone 8: kinematic + battery), `DRONE_B.3.md` (SAR + kinematic model). INVESTIGATION.md отсутствует.

**Текущее состояние (v0.4):**
- `Agent.pose: Pose` — статичная позиция (не меняется во время симуляции)
- `Agent.battery: f64` — всегда 100.0 (статичная)
- `Velocity` тип существует, но не используется
- `Agent.comms_range: f64` — уже добавлен (по умолчанию INFINITY, backward compat)
- `ConnectivityModel` в `swarm-comms` — уже вычисляет link existence по range

**Критерий готовности:**
1. Агенты двигаются: `pose += velocity * dt` каждый тик при назначенной задаче с `pose`.
2. Батарея расходуется пропорционально пройденному расстоянию.
3. Агент с `battery = 0` не может быть назначен на новые задачи (capability gate).
4. При `comms_range < INFINITY`, движение меняет связность между агентами (link появляется/исчезает при сближении/расхождении).
5. Метрики: `final_battery_min`, `avg_distance_travelled`, `agents_exhausted`.
6. Все существующие тесты проходят (backward compat: `comms_range = INFINITY`, `speed = 0`).

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-types/src/agent.rs` | Добавить `speed: f64` (m/s), `max_range: f64` (m), `battery_drain_rate: f64` (%/m) в `Agent` |
| `crates/swarm-types/src/pose.rs` | Методы `Velocity::speed() -> f64`, `Pose::distance_to(&Pose) -> f64` |
| `crates/swarm-runtime/src/membership.rs` | `AgentEntry`: добавить `speed`, `max_range`, `battery_drain_rate`; метод `apply_movement(dt)` |
| `crates/swarm-runtime/src/node.rs` | `AgentNode`: вызывать `apply_movement` в tick loop; обновлять `AllocationAgent.battery` из membership |
| `crates/swarm-alloc/src/allocator.rs` | Battery gate: `AllocationAgent.battery > 0.0` как capability constraint в `allocate()` |
| `crates/swarm-sim/src/runner.rs` | Передавать `tick_duration_ms` в movement; метрики движения/батареи |
| `crates/swarm-metrics/src/metrics.rs` | Новые поля: `final_battery_min`, `avg_distance_travelled`, `agents_exhausted` |
| `crates/swarm-scenarios/src/coverage.rs` | Добавить `speed`, `range`, `battery_drain_rate` в агентов coverage-сценария |
| `crates/swarm-scenarios/src/partition.rs` | Добавить движение в partition-сценарий (опционально) |
| `README.md` | Обновить статус до Milestone 8 |

---

## Implementation Steps

### Шаг 1 — Расширить Agent/AgentEntry кинематическими полями

Файл: `crates/swarm-types/src/agent.rs`

```rust
pub struct Agent {
    // ... existing fields ...
    pub speed: f64,              // NEW: cruising speed (m/s)
    pub max_range: f64,          // NEW: max travel distance on full battery (m)
    pub battery_drain_rate: f64, // NEW: battery % per meter travelled (0.0..=1.0)
}
```

`max_range` — максимальная дистанция, которую агент может пройти на полной батарее.
`battery_drain_rate` = 100.0 / max_range (вычисляется автоматически при создании, если не задан явно).

`speed` по умолчанию 0.0 для backward compat.

Файл: `crates/swarm-runtime/src/membership.rs`

```rust
pub struct AgentEntry {
    // ... existing fields ...
    pub speed: f64,
    pub max_range: f64,
    pub battery_drain_rate: f64,
}
```

**Тесты (категория 1):**
- `agent_speed_defaults_to_zero` — backward compat

---

### Шаг 2 — Кинематика: position += velocity * dt

Новый метод в `MembershipView` (или отдельная функция в `node.rs`):

```rust
impl MembershipView {
    /// Move agents toward their assigned tasks. Updates pose and battery.
    /// Returns list of agents that exhausted their battery this tick.
    pub fn apply_movement(
        &mut self,
        registry: &TaskRegistry,
        tick_duration_ms: u64,
    ) -> (Vec<AgentId>, Vec<(AgentId, f64)>) // (exhausted, (agent_id, distance_moved))
}
```

Логика:
1. Для каждого alive агента с назначенной задачей:
   - Получить `task.pose` (если задача без pose — движение не применяется)
   - Вычислить направление к цели: `dx = task_pose.x - agent_pose.x`, `dy = task_pose.y - agent_pose.y`
   - `distance_to_target = sqrt(dx² + dy²)`
   - `max_step = speed * (tick_duration_ms / 1000.0)` — сколько метров можно пройти за тик
   - Если `distance_to_target <= max_step`:
     - `agent.pose = task.pose` (достигли цели)
     - Пройденное расстояние = `distance_to_target`
   - Иначе:
     - `agent.pose.x += dx / distance_to_target * max_step`
     - `agent.pose.y += dy / distance_to_target * max_step`
     - Пройденное расстояние = `max_step`
   - `battery_drain = distance_moved * battery_drain_rate`
   - `agent.battery = max(0.0, agent.battery - battery_drain)`
2. Вернуть список агентов с `battery <= 0` (exhausted) и расстояния.

**Тесты (категория 1):**
- `movement_toward_target_updates_pose` — агент движется к задаче
- `movement_reaches_target_snaps_pose` — агент достигает цели и останавливается
- `movement_drains_battery` — батарея уменьшается пропорционально расстоянию
- `movement_exhausts_battery` — батарея достигает 0, агент помечается exhausted
- `movement_no_target_no_movement` — агент без задачи не двигается
- `movement_speed_zero_no_movement` — speed=0 → без движения (backward compat)

---

### Шаг 3 — Battery gate в allocator

Файл: `crates/swarm-alloc/src/allocator.rs`

В `GreedyAllocator::allocate()` и `AuctionAllocator::allocate()`:

```rust
// Filter out agents with exhausted battery
let capable: Vec<&AllocationAgent> = agents
    .iter()
    .filter(|agent| has_all_capabilities(agent, &at.task.required_capabilities))
    .filter(|agent| agent.battery > 0.0) // NEW: battery gate
    .collect();
```

При `battery = 0`, агент исключается из allocation (как и при несовпадении capabilities).

**Тесты (категория 1):**
- `battery_exhausted_agent_excluded_from_allocation` — greedy не назначает задачи агенту с battery=0
- `battery_exhausted_agent_excluded_from_auction` — auction не назначает

---

### Шаг 4 — Интеграция движения в tick loop

Файл: `crates/swarm-runtime/src/node.rs`

В `process_inbox_and_allocate()` после `allocator`:

```rust
if self.config.enable_movement {
    let (exhausted, distances) = self.coordinator.membership.apply_movement(
        &self.coordinator.registry,
        self.config.tick_duration_ms,
    );
    // exhausted agents: release their tasks for reallocation
    for agent_id in &exhausted {
        self.coordinator.membership.mark_dead(agent_id);
        self.coordinator.registry.release_agent_tasks(agent_id);
    }
    // Record distances for metrics
    for (agent_id, distance) in &distances {
        self.movement_this_tick.push((agent_id.clone(), *distance));
    }
}
```

Добавить `NodeConfig`:

```rust
pub struct NodeConfig {
    pub tick_duration_ms: u64,  // milliseconds per tick
    pub enable_movement: bool,  // default: false (backward compat)
}
```

Передать `NodeConfig` в `AgentNode::new()`.

Файл: `crates/swarm-sim/src/runner.rs`

Добавить в `RunConfig`:
```rust
pub enable_movement: bool,
pub tick_duration_ms: u64,  // default: 100
```

Передавать в `AgentNode` при создании.

**Тесты (категория 1):**
- `movement_disabled_by_default_does_not_change_pose` — backward compat

---

### Шаг 5 — Влияние движения на связность

Движение уже влияет на связность автоматически через `ConnectivityModel::direct_link(a.pose, range_a, b.pose, range_b)` — при изменении `pose` расстояние пересчитывается каждый тик. При `comms_range = INFINITY` (default) связность не меняется (backward compat).

Дополнительно: при движении агент может выйти из `comms_range` другого агента и перестать с ним обмениваться heartbeats/gossip. Это эквивалентно динамическому partition.

**Тесты (категория 1):**
- `movement_changes_connectivity` — агент с `comms_range=10` уходит на расстояние 15 и теряет связь
- `movement_restores_connectivity` — агент возвращается в range и связь восстанавливается

---

### Шаг 6 — Метрики движения и батареи

Файл: `crates/swarm-metrics/src/metrics.rs`

Новые поля в `RunMetrics`:
```rust
pub final_battery_min: f64,           // minimum battery among all agents at end
pub avg_distance_travelled: f64,      // average distance per agent per run
pub agents_exhausted: u64,            // count of agents that reached battery=0
pub total_distance_travelled: f64,    // sum of all agent distances
```

Заполняются в `ScenarioRunner::run_with()`.

Обновить `AggregateMetrics` и `Display`.

**Тест (категория 1):**
- `movement_metrics_present` — после симуляции с движением метрики содержат ненулевые значения

---

### Шаг 7 — Обновить существующие сценарии

Файл: `crates/swarm-scenarios/src/coverage.rs`, `partition.rs`, `auction.rs`

Добавить `speed: 5.0`, `max_range: 500.0` к агентам в сценариях где это уместно.
Для backward compat: `speed: 0.0` где движение не нужно.

Файл: `crates/swarm-examples/src/bin/partition_scenario.rs`

Опционально: добавить `enable_movement: true` и проверить что convergence работает при движении.

---

### Шаг 8 — Обновить README.md

- Добавить Milestone 8 в `## Current Status`
- Описать kinematic/battery модель
- Документировать `speed`, `max_range`, `battery_drain_rate`

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run -p swarm-examples --bin coverage_with_failure
cargo run -p swarm-examples --bin dynamic_auction
cargo run -p swarm-examples --bin multiprocess_scenario
cargo run -p swarm-examples --bin partition_scenario
```

---

## Testing Strategy

### Категория 1 — Без рефакторинга

**`swarm-types` — поля Agent (1 тест):**
- `agent_speed_defaults_to_zero`

**`swarm-runtime` — kinematics (6 тестов):**
- `movement_toward_target_updates_pose`
- `movement_reaches_target_snaps_pose`
- `movement_drains_battery`
- `movement_exhausts_battery`
- `movement_no_target_no_movement`
- `movement_speed_zero_no_movement`

**`swarm-runtime` — backward compat (1 тест):**
- `movement_disabled_by_default_does_not_change_pose`

**`swarm-alloc` — battery gate (2 теста):**
- `battery_exhausted_agent_excluded_from_allocation`
- `battery_exhausted_agent_excluded_from_auction`

**`swarm-comms` — connectivity + movement (2 теста):**
- `movement_changes_connectivity`
- `movement_restores_connectivity`

**`swarm-metrics` — новые метрики (1 тест):**
- `movement_metrics_present`

**Регрессия:** все существующие ~130 тестов должны пройти (backward compat через default значения).

### Категория 2 — Лёгкий рефакторинг

- **Integration test с движением:** запустить in-process симуляцию с `speed > 0`, проверить что за N тиков агент приблизился к цели на ожидаемое расстояние.
- **Battery exhaustion integration:** агент с маленькой батареей не доходит до удалённой задачи, задача остаётся unassigned → success=false для этой конфигурации.

### Категория 3 — Тяжёлый (не для v0.8)

- **SAR mission с движением**: grid, hidden targets, multiple agents с разными скоростями и батареями. Отложен до v0.9.
- **Relay placement с движением**: агенты двигаются чтобы поддерживать mesh. v0.9.

### Покрытие gap

- **Gap**: wallclock battery drain vs simulation ticks. Текущая модель: drain за расстояние, не за время. Висение на месте не тратит батарею. Приемлемо для v0.8.
- **Gap**: нет модели возврата на базу (return-to-base). Агент может исчерпать батарею на полпути и стать dead. Для SAR это важный параметр — отложен до v0.9.
- **Gap**: нет charge/recharge модели. Батарея только тратится. Приемлемо для v0.8.

---

## Risks and Tradeoffs

**1. Breaking change: `Agent` новые поля**

`speed`, `max_range`, `battery_drain_rate` добавляются в `Agent`. Все конструкторы `Agent` должны быть обновлены. `#[serde(default)]` обеспечивает deserialization старых JSON-конфигов.

**2. Battery drain = f(distance), не f(time)**

Простая модель: батарея тратится только при движении. Не учитывает "standby drain" (расход на связь, сенсоры). Достаточно для v0.8; standby drain можно добавить позже как `battery_drain_rate * dt`.

**3. Movement в `MembershipView` vs отдельный модуль**

`apply_movement` в `MembershipView` изменяет mutable state `pose` и `battery`. Это нарушает SRP (MembershipView отвечает и за membership, и за физику). Но для v0.8 это простейший путь; рефакторинг в отдельный `KinematicModel` — v0.9.

**4. `speed = 0` backward compat**

По умолчанию `speed = 0.0` — без движения. Все существующие тесты не затрагиваются. `enable_movement = false` в `NodeConfig` — дополнительная защита.

**5. Battery=0 → mark_dead semantics**

При battery=0 агент помечается dead, его задачи освобождаются. Это консистентно с текущим failure detection: dead агент исключается из allocation и membership. Альтернатива: `Health::Exhausted` — но добавляет новый state без явной пользы для v0.8.

**6. Движение меняет аллокационные решения**

Поскольку battery теперь влияет на eligibility через allocator gate, а pose меняется — allocation decisions становятся нестатичными. Gossip/merge должен корректно обрабатывать случаи, когда агент «передумал» из-за движения. Это уже покрыто gossip convergence тестами из v0.4.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| Новые поля в `Agent` | Все конструкторы Agent (в тестах, сценариях, agent_process) не компилируются | `cargo check --workspace` |
| `serde(default)` для новых полей | Старые JSON-конфиги для `agent_process` не десериализуются | Тест `agent_comms_range_serde_default_infinity` — аналогично для speed/max_range |
| `apply_movement` меняет pose | Координатор/allocator работают с устаревшим pose из `AllocationAgent` | Pose обновляется в membership → при построении AllocationAgent используется актуальный pose |
| Battery drain при speed=0 | Не должно быть drain | Тест `movement_speed_zero_no_movement` |
| Partitions + movement | Агент выходит из comms_range → partition; должен быть обработан gossip | Существующие gossip тесты из v0.4 |
| `enable_movement = true` по умолчанию | Ломает все существующие тесты | `enable_movement = false` по умолчанию; тесты проверяют explicit opt-in |
| `Cargo.lock` изменился | Должен быть включён в commit | `git diff --stat` |

---

## Open Questions

1. **Куда поместить `apply_movement`?** Варианты: (a) метод `MembershipView`, (b) отдельный `KinematicModel` в `swarm-runtime`, (c) в `AgentNode`. Для v0.8 — (a) как самый простой. Для v0.9 — рефакторить в (b).

2. **`battery_drain_rate` вычислять или задавать?** `battery_drain_rate = 100.0 / max_range` если не задан явно. Упрощает конфигурацию сценариев.

3. **Battery drain per time?** Сейчас: drain per distance. Для SAR это правильно (основной расход — на полёт). Для relay/standby нужен drain per time. Добавить позже как `battery_drain_rate_idle: f64`.

4. **Нужен ли `Velocity` вектор в агенте?** Текущий `Velocity` тип не используется. Можно вычислять velocity из разницы позиций между тиками (`new_pose - old_pose`) делённой на dt. Не нужно хранить. Или хранить для сглаживания траектории. Для v0.8 — не хранить.
