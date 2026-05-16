# План: Milestone 5 — Emergency Mesh (v0.5)

## Context

Текущее состояние проекта (после v0.4):

- **Milestone 1** (v0.1): heartbeat, membership, failure detection, task registry, greedy reallocation.
- **Milestone 2** (v0.2): dynamic tasks, auction allocator, capability matching, task expiration.
- **Milestone 3** (v0.3): pluggable transport (`InMemAgentTransport`, `UdpTransport`), multiprocess execution.
- **Milestone 4** (v0.4): partial connectivity, explicit partitions, gossip/anti-entropy, stale heartbeat protection, generation-based convergence.

Цель v0.5 — реализовать **второй reference scenario**: Emergency Mesh Network. Это уже не просто task allocation при отказе агента, а сценарий с неполной связностью, relay-агентами, деградацией сети и перестройкой mesh.

Сценарий:
- Базовая станция видит не всех агентов напрямую.
- Есть ground nodes (наземные узлы), которым нужна связь.
- Часть агентов — scout (разведка), часть — relay (ретрансляция).
- Связь ограничена дальностью (`comms_range`) и деградирует.
- Relay может погибнуть; runtime должен перераспределить relay-задачи.
- Измеряется network availability over time.

## Investigation context

`INVESTIGATION.md` отсутствует. Анализ проведён на основе:
- `DRONE_A.1.md` — reference scenario Emergency Mesh Network (стр. 228–248).
- `DRONE_B.1.md` — слои системы, фаза 2 Coordination Layer, метрики network availability.
- Текущего состояния кодовой базы (прочитаны все `.rs` файлы проекта).

## Affected components

| Crate | Изменения |
| --- | --- |
| `swarm-types` | `comms_range` у `Agent`; `required_role` у `Task`; новый тип `GroundNode`. |
| `swarm-comms` | `ConnectivityModel` — граф связности по дальности (ручной BFS); mesh reachability; интеграция в `InMemNetwork`. |
| `swarm-alloc` | `ConnectivityAwareAllocator` — учитывает связность при назначении relay-задач; расширение `Allocator` trait методом `allocate_with_connectivity`. |
| `swarm-runtime` | Поддержка `required_role` в `allocate_unassigned`; обработка relay-задач в `AgentNode`. |
| `swarm-metrics` | `network_availability`, `relay_reallocation_ticks`, `avg_hop_count`, `disconnected_agents_max`. |
| `swarm-sim` | `ScenarioRunner` обновляет позиции агентов по назначенным задачам и собирает метрики связности на каждом тике. |
| `swarm-scenarios` | Новый builder: `emergency_mesh.rs` — сценарий с базой, ground nodes, scout и relay. |
| `swarm-examples` | Новый бинарник `emergency_mesh_scenario.rs` — запускаемый reference scenario. |
| `README.md` | Актуализация статуса, добавление описания Milestone 5 и примера запуска. |
| `Cargo.toml` (root) | Без изменений: новый dependency не требуется (BFS на `Vec` + `HashMap`). |

## Implementation steps

### Шаг 1. Типы данных: связность, роли, наземные узлы

**1.1.** Добавить `comms_range: f64` в `Agent` (`crates/swarm-types/src/agent.rs`).
- По умолчанию `f64::INFINITY` (полная совместимость со старыми сценариями).
- Обновить конструкторы в тестах и сценариях.

**1.2.** Добавить `required_role: Option<Role>` в `Task` (`crates/swarm-types/src/task.rs`).
- Hard constraint: агент без совпадающей роли исключается из allocation.
- Аналогично `required_capabilities`, но проверяет `agent.role`.
- `preferred_role` остаётся soft constraint (cost bonus).

**1.3.** Добавить `GroundNode` (`crates/swarm-types/src/agent.rs` или новый файл `ground.rs`):
```rust
pub struct GroundNode {
    pub id: String,
    pub pose: Pose,
    pub comms_range: f64,
}
```
- Ground node — пассивный участник mesh: он не получает задачи, но участвует в графе связности.

**1.4.** Добавить `ground_nodes: Vec<GroundNode>` и `base_station: Option<Pose>` в `Scenario` (`crates/swarm-sim/src/scenario.rs`).
- Serde-совместимость: поля опциональны или имеют дефолты (`Vec::new()`, `None`).

### Шаг 2. Модель связности и mesh reachability

**2.1.** Создать `crates/swarm-comms/src/connectivity.rs`:
```rust
pub struct ConnectivitySnapshot {
    pub agent_entries: Vec<(AgentId, Pose, f64, Health)>, // id, pose, comms_range, health
    pub ground_nodes: Vec<(String, Pose, f64)>,          // id, pose, comms_range
    pub base_id: String,
    pub base_pose: Pose,
}

pub struct ConnectivityModel;
impl ConnectivityModel {
    /// Прямая связь: distance(a, b) <= min(comms_range_a, comms_range_b)
    pub fn direct_link(a: &Pose, range_a: f64, b: &Pose, range_b: f64) -> bool;

    /// Построить adjacency list (HashMap<String, Vec<String>>) по снимку.
    pub fn build_adjacency(snapshot: &ConnectivitySnapshot) -> HashMap<String, Vec<String>>;

    /// BFS от base до всех достижимых узлов. Возвращает HashMap<node_id, hop_count>.
    pub fn reachability_from_base(snapshot: &ConnectivitySnapshot) -> HashMap<String, usize>;

    /// Доля достижимых агентов от base = network availability.
    pub fn availability_fraction(reachability: &HashMap<String, usize>, agent_ids: &[AgentId]) -> f64;
}
```
- Реализовать через ручной BFS на `VecDeque` + `HashMap` (adjacency list). Новый dependency не требуется (N ≤ 20, аллокаций `Vec` достаточно).

**2.2.** Модифицировать `InMemNetwork` (`crates/swarm-comms/src/network.rs`):
- Добавить `connectivity: Option<ConnectivitySnapshot>` — снимок позиций и дальностей на текущий тик.
- В `send()` вместо (или в дополнение к) `partitions.contains(&pair)` проверять reachability через `ConnectivityModel`.
- Если `multi_hop` включён: сообщение доходит, если существует путь в графе связности (симуляция mesh routing без явных routing tables).
- Сохранить поддержку explicit `partitions` для обратной совместимости с v0.4 сценариями.

**2.3.** Добавить `latency_per_hop: u64` в `NetworkConfig`.
- Полная задержка = `base_latency_ticks + hop_count * latency_per_hop`.

### Шаг 3. Connectivity-aware allocator

**3.1.** Расширить `AllocationAgent` (`crates/swarm-alloc/src/allocator.rs`):
- Добавить `comms_range: f64`.

**3.2.** Расширить `AllocationTask`:
- Добавить `task_kind: TaskKind` (или использовать `required_role`).

**3.3.** Расширить `Allocator` trait (`crates/swarm-alloc/src/allocator.rs`):
```rust
pub struct ConnectivityContext {
    pub snapshot: ConnectivitySnapshot,
    pub base_id: AgentId,
}

pub trait Allocator {
    fn allocate(
        &self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
    ) -> Vec<(TaskId, AgentId)>;

    /// Новый метод для v0.5. Default-реализация делегирует `allocate`.
    fn allocate_with_connectivity(
        &self,
        tasks: &[AllocationTask<'_>],
        agents: &[AllocationAgent],
        _connectivity: &ConnectivityContext,
    ) -> Vec<(TaskId, AgentId)> {
        self.allocate(tasks, agents)
    }
}
```
- `GreedyAllocator` и `AuctionAllocator` используют default-реализацию (игнорируют connectivity).
- `ConnectivityAwareAllocator` (`crates/swarm-alloc/src/connectivity_aware.rs`) реализует `allocate_with_connectivity`:
  - Для **relay-задач**: среди relay-агентов выбирать того, чья позиция максимизирует число вновь достижимых узлов от base (симулируем назначение на каждого кандидата, строим reachability, выбираем лучший).
  - Для **scout-задач**: стандартный auction/greedy cost.
  - Hard constraint `required_role` фильтрует до вычисления cost.

**3.4.** Обновить `has_all_capabilities` + добавить `has_required_role`:
```rust
fn has_required_role(agent: &AllocationAgent, required: &Option<Role>) -> bool {
    required.as_ref().map_or(true, |r| &agent.role == r)
}
```
- Применять в `GreedyAllocator` и `AuctionAllocator` перед cost calculation.

### Шаг 4. Runtime: учёт связности, pose update, allocator call sites

**4.1.** Модифицировать `AgentNode::send_heartbeats` (`crates/swarm-runtime/src/node.rs`):
- Heartbeats и gossip отправляются всем `peer_ids`, но `InMemNetwork` фильтрует недостижимые на основе `ConnectivityModel`. Это минимальное изменение в `node.rs`.

**4.2.** Обновить `allocate_unassigned` (`crates/swarm-runtime/src/node.rs`):
- Передавать `required_role` в фильтр capability/role.
- Передавать `comms_range` в `AllocationAgent`.
- Передавать `ConnectivityContext` в `allocator.allocate_with_connectivity(...)` вместо `allocate(...)`.

**4.3.** Добавить pose update в `ScenarioRunner::run` (`crates/swarm-sim/src/runner.rs`):
- После фазы allocation на каждом тике обновлять позиции живых агентов в `MembershipView`:
  - Если агенту назначена задача с `pose: Some(target_pose)`, установить `agent.pose = target_pose` (мгновенное перемещение для целей миссионной симуляции уровня A).
  - Это изменяет граф связности на следующем тике, что позволяет восстановить reachability после reallocation relay-задачи.
- Обновлённые позиции сохраняются в `Coordinator.membership` и передаются в `ConnectivitySnapshot` на следующем тике.

### Шаг 5. Метрики сети

**5.1.** Расширить `RunMetrics` (`crates/swarm-metrics/src/metrics.rs`):
```rust
pub struct RunMetrics {
    // ... существующие поля ...
    pub network_availability: f64,        // средняя доля достижимых агентов
    pub relay_reallocation_ticks: Option<u64>,
    pub avg_hop_count: f64,
    pub disconnected_agents_max: u64,
    pub relay_tasks_assigned: u64,
    pub relay_tasks_reassigned: u64,
}
```

**5.2.** Расширить `AggregateMetrics`:
- `avg_network_availability`, `avg_relay_reallocation_ticks`, `avg_avg_hop_count`, `avg_disconnected_agents_max`.

**5.3.** В `ScenarioRunner::run` (`crates/swarm-sim/src/runner.rs`):
- На каждом тике:
  1. Построить `ConnectivitySnapshot` из текущих позиций агентов в `MembershipView`.
  2. Вычислить `reachability = ConnectivityModel::reachability_from_base(&snapshot)`.
  3. `availability_fraction` = доля `agent_ids`, присутствующих в `reachability`.
  4. `avg_hop_count_this_tick` = среднее hop_count по всем достижимым агентам.
  5. `disconnected_count` = количество агентов, отсутствующих в `reachability`.
  6. Сохранить `availability_fraction` в вектор `availability_per_tick` для построения time series.
- `network_availability` (итоговая метрика) = среднее `availability_per_tick` по всем тикам.
- `relay_reallocation_ticks`:
  - `detection_tick` = тик, когда `FailureDetector` впервые пометил relay агента как failed (совпадает с `detection_time_ticks`, но фиксируется отдельно для relay).
  - `reallocation_tick` = первый тик, на котором все relay-задачи, принадлежавшие мёртвому relay, назначены новому живому агенту (проверяется через `TaskRegistry` + `required_role: Some(Role::Relay)`).
  - `relay_reallocation_ticks = reallocation_tick.saturating_sub(detection_tick)`.
- `disconnected_agents_max` = максимум `disconnected_count` за всю симуляцию.

### Шаг 6. Emergency Mesh Scenario

**6.1.** Создать `crates/swarm-scenarios/src/emergency_mesh.rs`:
```rust
pub struct EmergencyMeshConfig {
    pub seed: u64,
    pub scout_count: usize,
    pub relay_count: usize,
    pub ground_node_count: usize,
    pub base_pose: Pose,
    pub area_size: f64,        // размер зоны катастрофы
    pub comms_range: f64,      // дальность связи
    pub failure_tick: u64,     // когда погибает relay
    pub max_ticks: u64,
    pub timeout_ticks: u64,
    pub gossip_interval_ticks: u64,
}
```
- Генерация агентов:
  - Base station в центре или на краю.
  - Scouts равномерно/случайно распределены в зоне.
  - Relays позиционируются между base и удалёнными scouts.
  - Ground nodes — фиксированные точки, которым нужна связь.
- Задачи:
  - Scout-задачи: `required_role: Some(Role::Scout)` — покрытие подзон.
  - Relay-задачи: `required_role: Some(Role::Relay)` — позиционирование в ключевых точках mesh.
- Отказ: `failure_tick` — один из relay агентов погибает.

**6.2.** Экспортировать из `swarm-scenarios/src/lib.rs`.

### Шаг 7. Пример запуска

**7.1.** Создать `crates/swarm-examples/src/bin/emergency_mesh_scenario.rs`:
- Запуск 1000 seeds.
- Для каждого seed:
  - Построить `EmergencyMeshConfig` с вариацией позиций.
  - Запустить `ScenarioRunner` с `ConnectivityAwareAllocator`.
  - Проверить инварианты:
    - `network_availability >= 0.8` (80% времени сеть связна).
    - `relay_reallocation_ticks` задано (перераспределение произошло).
    - Все scout-задачи назначены capable агентам.
- Вывести `AggregateMetrics`.
- Exit code `1` при нарушении инвариантов.

### Шаг 8. Актуализация README

**8.1.** Добавить раздел **Milestone 5** в `README.md`:
- Emergency Mesh scenario.
- `comms_range`, ground nodes, base station.
- Relay role и connectivity-aware allocation.
- Network availability metrics.
- Relay reallocation при потере.

**8.2.** Добавить команду запуска:
```bash
cargo run -p swarm-examples --bin emergency_mesh_scenario
```

## Testing strategy

### Category 1 — Pure Unit Tests (без рефакторинга существующих тестов)

| Тест | Что проверяется | Файл |
| --- | --- | --- |
| `connectivity_direct_link_within_range` | `ConnectivityModel::direct_link` возвращает true в пределах дальности. | `swarm-comms/src/connectivity.rs` |
| `connectivity_direct_link_beyond_range` | `direct_link` false за пределами. | `swarm-comms/src/connectivity.rs` |
| `connectivity_mesh_reachability_via_relay` | Путь от base до scout существует только через relay. | `swarm-comms/src/connectivity.rs` |
| `connectivity_hop_count_two_hops` | `hop_count` = 2 при двух ретрансляциях. | `swarm-comms/src/connectivity.rs` |
| `connectivity_availability_all_reachable` | `availability_fraction` = 1.0 при полной связности. | `swarm-comms/src/connectivity.rs` |
| `connectivity_availability_half_reachable` | `availability_fraction` = 0.5 при разделении. | `swarm-comms/src/connectivity.rs` |
| `allocator_required_role_blocks_scout` | `required_role: Some(Relay)` блокирует Scout-агента. | `swarm-alloc/src/allocator.rs` |
| `allocator_required_role_allows_relay` | Relay-агент проходит фильтр для relay-задачи. | `swarm-alloc/src/allocator.rs` |
| `connectivity_aware_prefers_relay_for_relay_task` | `ConnectivityAwareAllocator` назначает relay-задачу relay-агенту, а не scout. | `swarm-alloc/src/connectivity_aware.rs` |
| `pose_update_changes_agent_position` | После назначения задачи с `pose` позиция агента в `MembershipView` обновляется. | `swarm-sim/src/runner.rs` |

### Category 2 — In-Process Async Simulation (основная среда)

| Тест | Что проверяется | Файл |
| --- | --- | --- |
| `emergency_mesh_base_reaches_all_via_relay` | Base достигает всех scouts через 2 relay; network_availability = 1.0. | `swarm-examples/src/bin/emergency_mesh_scenario.rs` (вызов из теста) |
| `emergency_mesh_relay_death_causes_reallocation` | После гибели relay `relay_reallocation_ticks` задано, задачи переназначены. | `swarm-examples/src/bin/emergency_mesh_scenario.rs` |
| `emergency_mesh_network_degrades_when_relay_lost` | При потере relay availability падает, затем восстанавливается после reallocation + pose update нового relay. | `swarm-examples/src/bin/emergency_mesh_scenario.rs` |
| `emergency_mesh_1000_seeds_invariant` | 1000 seeds, все проходят порог `network_availability >= 0.8`. | `swarm-examples/src/bin/emergency_mesh_scenario.rs` |
| `partition_scenario_still_works` | Обратная совместимость: v0.4 partition scenario проходит без регрессий. | `swarm-examples/src/bin/partition_scenario.rs` |
| `coverage_scenario_still_works` | v0.1 coverage scenario проходит без регрессий (comms_range по умолчанию = INF). | `swarm-examples/src/bin/coverage_with_failure.rs` |
| `dynamic_auction_still_works` | v0.2 dynamic auction проходит без регрессий. | `swarm-examples/src/bin/dynamic_auction.rs` |
| `multiprocess_scenario_still_works` | v0.3 multiprocess scenario проходит без регрессий. | `swarm-examples/src/bin/multiprocess_scenario.rs` |

### Category 3 — Multi-Process / Heavy Integration

Для v0.5 не требуется новых multi-process тестов, поскольку mesh-логика реализована на уровне runtime + in-process network model. Существующий `multiprocess_scenario` (v0.3) должен продолжать проходить как регрессионный тест.

**Gap**: multi-process mesh с UDP loopback и реальным range-based packet loss — оставить на v0.6+ (расширение transport layer).

## Risks and tradeoffs

1. **Обратная совместимость Task serde**: добавление `required_role` может сломать десериализацию старых JSON-конфигов агентов (если такие есть вне репозитория). Внутри репозитория все конструкторы обновляются.
   - Mitigation: `#[serde(default)]` на новых полях.

2. **Производительность графа**: построение adjacency list + BFS на каждом тике для N=20 агентов — O(N²), приемлемо. Для N>100 потребуется оптимизация (инкрементальное обновление графа).
   - Mitigation: пока N мал, полный rebuild на каждом тике допустим.

3. **Mesh abstraction vs reality**: текущий план симулирует multi-hop на транспортном уровне (`InMemNetwork` проверяет reachability), а не через явные routing tables в `AgentNode`. Это допустимая абстракция для v0.5, но ограничивает fidelity.
   - Tradeoff: проще реализация, но нельзя тестировать протоколы маршрутизации.

4. **Сложность allocator'а**: `ConnectivityAwareAllocator` требует знания позиций и структуры сети. В плане выбрано расширение `Allocator` trait методом `allocate_with_connectivity` с default-реализацией, делегирующей `allocate`. Это сохраняет обратную совместимость для `GreedyAllocator` и `AuctionAllocator`.
   - Mitigation: все call sites в `node.rs` и `runner.rs` переходят на `allocate_with_connectivity`; `GreedyAllocator`/`AuctionAllocator` не требуют изменений.

5. **Determinism**: позиции агентов в сценарии должны быть seed-based. `EmergencyMeshConfig` использует `rand` с фиксированным `seed` для генерации позиций.

## Open questions

1. **Динамическая смена ролей**: должен ли агент-scout динамически становиться relay при гибели всех relay? Это усложняет state machine. Для v0.5 оставить фиксированные роли; reallocation — только переназначение задач между агентами одной роли.
2. **Движение агентов**: для v0.5 добавлен мгновенный pose update при назначении задачи с `pose`. Это упрощённая модель движения уровня A (mission simulation). Реальная кинематика (`position += velocity * dt`) — в будущем.
3. **Назначение relay-задач**: relay-задачи — это "стоять в точке X,Y" или "обеспечивать связь с узлом Y"? Для v0.5: позиционные relay-задачи (pose-based), allocator назначает relay-агента к точке.
4. **Ground nodes как задачи?** ground node — это отдельная сущность (не Task, не Agent). Она участвует в графе связности, но не получает allocation. Альтернатива: сделать ground nodes специальными Task (coverage точек). Решение: отдельная сущность для ясности.

## Что могло сломаться

### Поведение
- **Default `comms_range = INFINITY`** означает, что старые сценарии (coverage, auction, partition) продолжают работать в полносвязной сети. Если default случайно будет `0.0`, все старые сценарии сломаются (нет связи).
  - *Проверка*: запустить `coverage_with_failure`, `dynamic_auction`, `partition_scenario`, `multiprocess_scenario` — все должны показывать `success_rate = 1.0`.

### API / контракты
- **Изменение `Task` struct**: добавление `required_role` меняет публичный API `swarm-types`. Код вне репозитория, использующий `Task { ... }` без поля `required_role`, сломается при компиляции.
  - *Проверка*: `cargo build --workspace` должен проходить без ошибок; все конструкторы `Task` в репозитории обновлены.
- **Изменение `NetworkConfig`**: добавление `latency_per_hop`.
  - *Проверка*: все вызовы `NetworkConfig { ... }` в репозитории обновлены.

### Данные / сериализация
- **Serde backward compat**: старые сохранённые сценарии (JSON/YAML) без `comms_range` и `required_role` могут не десериализоваться.
  - *Проверка*: добавить `#[serde(default)]` на все новые поля; написать unit test на десериализацию старого JSON.

### Интеграции
- **Multiprocess scenario**: `UdpTransport` не знает о `ConnectivityModel`. В multi-process режиме связь остаётся полносвязной (UDP loopback не фильтруется по дальности).
  - *Проверка*: `multiprocess_scenario` должен продолжать проходить. Ограничение задокументировать в README: mesh reachability симулируется только в in-process режиме на v0.5.

### Производительность / ресурсы
- **Граф на каждом тике**: `ScenarioRunner` строит adjacency list и запускает BFS для N агентов + M ground nodes каждый тик. Для N=20, M=5, 1000 seeds × 50 ticks — ~1M запусков BFS. Ручная реализация на `VecDeque` справится, но стоит измерить.
  - *Проверка*: запустить `cargo bench` (если есть) или измерить время `emergency_mesh_scenario` для 1000 seeds; регрессия >20% — сигнал к оптимизации.

### Регрессии в метриках
- **Новые поля в `RunMetrics`**: `AggregateMetrics::from_runs` теперь должен учитывать новые поля. Если забыть обновить Display/aggregation, метрики будут неполными.
  - *Проверка*: unit tests на `AggregateMetrics` с заполненными новыми полями.
