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
| `swarm-comms` | `ConnectivityModel` — граф связности по дальности; mesh reachability; интеграция в `InMemNetwork`. |
| `swarm-alloc` | `ConnectivityAwareAllocator` — учитывает связность при назначении relay-задач. |
| `swarm-runtime` | Поддержка `required_role` в `allocate_unassigned`; обработка relay-задач в `AgentNode`. |
| `swarm-metrics` | `network_availability`, `relay_reallocation_ticks`, `avg_hop_count`, `disconnected_agents_max`. |
| `swarm-sim` | `ScenarioRunner` собирает метрики связности на каждом тике. |
| `swarm-scenarios` | Новый builder: `emergency_mesh.rs` — сценарий с базой, ground nodes, scout и relay. |
| `swarm-examples` | Новый бинарник `emergency_mesh_scenario.rs` — запускаемый reference scenario. |
| `README.md` | Актуализация статуса, добавление описания Milestone 5 и примера запуска. |

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
pub struct ConnectivityModel;
impl ConnectivityModel {
    /// Прямая связь: distance(a, b) <= min(comms_range_a, comms_range_b)
    pub fn direct_link(a: &Pose, range_a: f64, b: &Pose, range_b: f64) -> bool;
    
    /// Построить граф связности по агентам + ground nodes + base.
    pub fn build_graph(agents: &[Agent], ground_nodes: &[GroundNode], base: Option<&Pose>) -> Graph;
    
    /// Есть ли путь от base до target через живых агентов и ground nodes?
    pub fn is_reachable(graph: &Graph, from: &str, to: &str) -> bool;
    
    /// Количество хопов кратчайшего пути (None = unreachable).
    pub fn hop_count(graph: &Graph, from: &str, to: &str) -> Option<usize>;
    
    /// Доля достижимых агентов от base = network availability.
    pub fn availability_fraction(graph: &Graph, base_id: &str, agent_ids: &[AgentId]) -> f64;
}
```
- Использовать `petgraph` (уже в `Cargo.toml` / `workspace.dependencies`).

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

**3.3.** Реализовать `ConnectivityAwareAllocator` (`crates/swarm-alloc/src/connectivity_aware.rs`):
```rust
pub struct ConnectivityAwareAllocator {
    pub base_allocator: Box<dyn Allocator>,
    pub weight_connectivity: f64, // бонус за улучшение связности
}
```
- Для **relay-задач**: выбирать агента так, чтобы его позиция максимизировала число вновь достижимых узлов от base (центральность, покрытие "дыр" в связности).
- Для **scout-задач**: стандартный auction/greedy cost, но с проверкой `required_role`.
- Hard constraint `required_role` фильтрует до вычисления cost.

**3.4.** Обновить `has_all_capabilities` + добавить `has_required_role`:
```rust
fn has_required_role(agent: &AllocationAgent, required: &Option<Role>) -> bool {
    required.as_ref().map_or(true, |r| &agent.role == r)
}
```
- Применять в `GreedyAllocator` и `AuctionAllocator` перед cost calculation.

### Шаг 4. Runtime: учёт связности в AgentNode и ScenarioRunner

**4.1.** Модифицировать `AgentNode::send_heartbeats` (`crates/swarm-runtime/src/node.rs`):
- Вместо отправки всем `peer_ids`, отправлять только достижимым соседям (direct links) или всем (broadcast через mesh, если сеть симулирует multi-hop на транспортном уровне).
- Для простоты v0.5: heartbeats и gossip отправляются всем `peer_ids`, но `InMemNetwork` фильтрует недостижимые на основе `ConnectivityModel`. Это минимальное изменение в `node.rs`.

**4.2.** Обновить `allocate_unassigned` (`crates/swarm-runtime/src/node.rs`):
- Передавать `required_role` в фильтр capability/role.
- Передавать `comms_range` в `AllocationAgent`.

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
- На каждом тике строить `ConnectivityModel`, вычислять:
  - `availability_fraction` (доля агентов, достижимых от base).
  - `hop_count` от base до каждого агента.
  - `disconnected_count`.
- Агрегировать по всем тикам для итоговых метрик.

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

### Category 2 — In-Process Async Simulation (основная среда)

| Тест | Что проверяется | Файл |
| --- | --- | --- |
| `emergency_mesh_base_reaches_all_via_relay` | Base достигает всех scouts через 2 relay; network_availability = 1.0. | `swarm-examples/src/bin/emergency_mesh_scenario.rs` (вызов из теста) |
| `emergency_mesh_relay_death_causes_reallocation` | После гибели relay `relay_reallocation_ticks` задано, задачи переназначены. | `swarm-examples/src/bin/emergency_mesh_scenario.rs` |
| `emergency_mesh_network_degrades_when_relay_lost` | При потере relay availability падает, затем восстанавливается после reallocation. | `swarm-examples/src/bin/emergency_mesh_scenario.rs` |
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

2. **Производительность графа**: построение `petgraph` на каждом тике для N=20 агентов — O(N²), приемлемо. Для N>100 потребуется оптимизация (инкрементальное обновление графа).
   - Mitigation: пока N мал, полный rebuild на каждом тике допустим.

3. **Mesh abstraction vs reality**: текущий план симулирует multi-hop на транспортном уровне (`InMemNetwork` проверяет reachability), а не через явные routing tables в `AgentNode`. Это допустимая абстракция для v0.5, но ограничивает fidelity.
   - Tradeoff: проще реализация, но нельзя тестировать протоколы маршрутизации.

4. **Сложность allocator'а**: `ConnectivityAwareAllocator` требует знания позиций всех агентов и структуры сети. Это нарушает чистоту `Allocator` trait (который сейчас stateless). Можно обойти, передавая `connectivity_snapshot` как параметр.
   - Mitigation: расширить `Allocator` trait новым методом `allocate_with_connectivity`, оставив старый метод для обратной совместимости; или обернуть allocator в структуру, хранящую снимок.

5. **Determinism**: позиции агентов в сценарии должны быть seed-based. `EmergencyMeshConfig` использует `rand` с фиксированным `seed` для генерации позиций.

## Open questions

1. **Динамическая смена ролей**: должен ли агент-scout динамически становиться relay при гибели всех relay? Это усложняет state machine. Для v0.5 оставить фиксированные роли; reallocation — только переназначение задач между агентами одной роли.
2. **Движение агентов**: должны ли агенты физически перемещаться к pose задачи? В текущей симуляции позиции статичны (Milestone A/B). Для v0.5 оставить статичные позиции; движение — в будущем (kinematic simulation).
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
- **Граф на каждом тике**: `ScenarioRunner` строит граф связности N агентов + M ground nodes каждый тик. Для N=20, M=5, 1000 seeds × 50 ticks — ~1M построений графа. `petgraph` справится, но стоит измерить.
  - *Проверка*: запустить `cargo bench` (если есть) или измерить время `emergency_mesh_scenario` для 1000 seeds; регрессия >20% — сигнал к оптимизации.

### Регрессии в метриках
- **Новые поля в `RunMetrics`**: `AggregateMetrics::from_runs` теперь должен учитывать новые поля. Если забыть обновить Display/aggregation, метрики будут неполными.
  - *Проверка*: unit tests на `AggregateMetrics` с заполненными новыми полями.
