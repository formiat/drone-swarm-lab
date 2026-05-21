# PLAN: CBBA Robustness (M15)

## Context

Stage 2 (SAR v2 / Uncertainty Map, M14) завершён. Платформа имеет:
- `CbbaAllocator` — распределённый алгоритм аукциона с bundle building + consensus.
- `CbbaConfig` — max_bundle_size, max_rounds, score weights.
- Proptest suite — 2 теста (no-panic, success-rate bounded) с фиксированными agent/task poses.
- `convergence_ticks` — метрика времени сходимости после heal (в runner).
- `PartitionEvent` — `{at_tick, until_tick, agents}`.
- `packet_loss_rate` в `RunConfig` — дроп сообщений на уровне `InMemNetwork`.
- 5 стратегий в benchmark matrix: greedy, auction, connectivity-aware, centralized, cbba.
- JSON/CSV/Markdown export через `report_export.rs`.

**Источники контекста:** `docs/DRONE_B.8.md` (Stage 3), `docs/DRONE_A.7.md` (Algorithmic Depth), `docs/DRONE_B.7.md`.

**Текущее состояние (после M14):**
- `CbbaAllocator` — Phase 1 (bundle building) + Phase 2 (consensus via remote bids).
- `check_convergence()` — stable winning bids for 2 consecutive rounds OR max_rounds reached.
- `current_round` — инкрементируется каждый тик в `allocate()`.
- `messages_exchanged` — счётчик remote bids.
- `bundle` — простой `Vec<TaskId>`, порядок = порядок добавления.
- `PartitionEvent.until_tick` — когда партиция заканчивается.
- `convergence_ticks` в `RunMetrics` — время до сходимости после heal.

**Критерий готовности:**
1. Расширенный proptest suite — случайные топологии, packet loss ∈ [0.0, 0.5], измерение convergence ticks.
2. Convergence time distribution — p50/p95/max в `AggregateMetrics`.
3. TSP-ordering в bundles — nearest-neighbour оптимизация порядка задач.
4. Retransmission policy — экспоненциальный backoff при высоком packet loss.
5. Partition healing — `heal_at_tick` в `PartitionEvent`, CBBA повторно сходится после heal.
6. 1000-seed publishable benchmark — `scenarios/cbba_stress.json`, README с таблицей.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_B.8.md` (Stage 3):
- CBBA robustness — ключевой шаг Algorithmic Depth.
- Нужно понять: где CBBA сходится быстро, где деградирует, как packet loss влияет на convergence.
- TSP-ordering + retransmission — практические улучшения для реального поведения.
- 1000-seed analysis — методологически сильный результат для публикации.

**Ключевое наблюдение:** текущий `CbbaAllocator`:
- Bundle order = порядок добавления, не оптимален для travel distance.
- Нет retransmission — lost bids остаются lost, сходимость может требовать больше раундов.
- `PartitionEvent` имеет `until_tick`, но нет явного `heal_at_tick` для автоматического восстановления.
- Proptest покрывает только no-panic и bounded success rate, не convergence time.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-alloc/src/cbba.rs` | TSP-ordering (`order_bundle_tsp`), retransmission policy, bundle travel metric |
| `crates/swarm-alloc/src/cbba.rs` | `CbbaConfig` — новые поля retransmission |
| `crates/swarm-sim/tests/proptest_cbba.rs` | Расширенные property-based тесты (топологии, packet loss, convergence) |
| `crates/swarm-metrics/src/metrics.rs` | `convergence_ticks_p50/p95/max`, `avg_bundle_travel_distance` |
| `crates/swarm-sim/src/runner.rs` | Convergence time tracking per tick, partition heal logic |
| `crates/swarm-sim/src/runner.rs` | `PartitionEvent` — добавить `heal_at_tick` |
| `crates/swarm-sim/src/report_export.rs` | Новые колонки в JSON/CSV export |
| `crates/swarm-sim/src/benchmark.rs` | Markdown table — новые колонки |
| `scenarios/cbba_stress.json` | **NEW** — 1000 seeds, разные packet loss rates |
| `README.md` | Таблица convergence distribution из 1000 seeds |

---

## Implementation Steps

### Шаг 1 — Расширенный proptest suite

Файл: `crates/swarm-sim/tests/proptest_cbba.rs`

Добавить 3 новых proptest:

```rust
#[test]
fn cbba_convergence_ticks_with_random_topology(
    agent_count in 3usize..=8,
    task_count in 3usize..=12,
    comms_range in 10.0f64..100.0,
    packet_loss in 0.0f64..0.5,
) {
    // Agents со случайными позициями в [0, 100]×[0, 100]
    // comms_range определяет связность графа (Erdős-Rényi-like)
    // Измерить convergence_ticks из metrics.cbba_rounds_to_convergence
    // Assert: converged == true для packet_loss < 0.3
}

#[test]
fn cbba_no_conflicts_after_convergence(
    agent_count in 2usize..=6,
    task_count in 2usize..=8,
    packet_loss in 0.0f64..0.4,
) {
    // После прогона проверить: нет task_id, назначенного >1 агенту
    // (через inspection registry или metrics.conflicting_assignments)
}

#[test]
fn cbba_convergence_time_bounded(
    agent_count in 2usize..=5,
    task_count in 2usize..=6,
    packet_loss in 0.0f64..0.5,
) {
    // convergence_ticks < max_ticks для packet_loss ∈ [0, 0.5]
}
```

### Шаг 2 — Convergence time distribution

Файл: `crates/swarm-metrics/src/metrics.rs`

Добавить в `AggregateMetrics`:
```rust
#[serde(default)]
pub convergence_ticks_p50: f64,
#[serde(default)]
pub convergence_ticks_p95: f64,
#[serde(default)]
pub convergence_ticks_max: f64,
#[serde(default)]
pub avg_bundle_travel_distance: f64,
```

Добавить в `RunMetrics` (для per-run tracking):
```rust
#[serde(default)]
pub cbba_convergence_tick: Option<u64>, // когда именно CBBA сошёлся
```

Файл: `crates/swarm-sim/src/runner.rs`

В `run()` — отслеживать тик, когда `cbba.converged` впервые стал true. Записывать в `RunMetrics.cbba_convergence_tick`.

`AggregateMetrics::from_runs()` — вычислять p50/p95/max из `cbba_convergence_tick` значений:
```rust
let mut convergence_ticks: Vec<u64> = runs
    .iter()
    .filter_map(|run| run.cbba_convergence_tick)
    .collect();
convergence_ticks.sort_unstable();
let p50 = percentile(&convergence_ticks, 0.5);
let p95 = percentile(&convergence_ticks, 0.95);
let max = convergence_ticks.last().copied().unwrap_or(0) as f64;
```

### Шаг 3 — TSP-ordering в task bundles

Файл: `crates/swarm-alloc/src/cbba.rs`

Добавить:
```rust
fn order_bundle_tsp(agent_pose: Pose, bundle: &[TaskId], tasks: &[Task]) -> Vec<TaskId> {
    // Greedy nearest-neighbour: начать с agent_pose, на каждом шаге — ближайшая непосещённая задача
    let mut ordered = Vec::new();
    let mut remaining: HashSet<TaskId> = bundle.iter().cloned().collect();
    let mut current_pos = agent_pose;
    
    while !remaining.is_empty() {
        let next = remaining
            .iter()
            .min_by_key(|tid| {
                let task = tasks.iter().find(|t| &t.id == *tid).unwrap();
                let pose = task.pose.unwrap_or(Pose { x: 0.0, y: 0.0 });
                (current_pos.distance_to(&pose) * 1000.0) as u64
            })
            .cloned()
            .unwrap();
        remaining.remove(&next);
        ordered.push(next);
        let task = tasks.iter().find(|t| t.id == next).unwrap();
        current_pos = task.pose.unwrap_or(Pose { x: 0.0, y: 0.0 });
    }
    ordered
}
```

В `build_bundles` — после добавления задачи, если bundle.len() > 1, переупорядочить через `order_bundle_tsp`.

Метрика `avg_bundle_travel_distance` — сумма расстояний между consecutive tasks в bundle + от agent до первой задачи.

Тест:
```rust
#[test]
fn order_bundle_tsp_returns_nearest_first() {
    let agent_pose = Pose { x: 0.0, y: 0.0 };
    let tasks = vec![
        task("t_far", 1, 100.0, 0.0),
        task("t_near", 1, 1.0, 0.0),
    ];
    let bundle = vec![TaskId::from("t_far".to_owned()), TaskId::from("t_near".to_owned())];
    let ordered = order_bundle_tsp(agent_pose, &bundle, &tasks);
    assert_eq!(ordered[0], TaskId::from("t_near".to_owned()));
}
```

### Шаг 4 — Retransmission policy

Файл: `crates/swarm-alloc/src/cbba.rs`

Расширить `CbbaConfig`:
```rust
pub retransmit_max_attempts: u32,   // default: 3
pub retransmit_backoff_ticks: u64,  // default: 2
pub retransmit_threshold_packet_loss: f64, // default: 0.1
```

В `CbbaAllocator` — отслеживать pending bids ( bids, которые были отправлены, но не получили acknowledgment от remote agents ):
```rust
pending_retransmits: HashMap<TaskId, (u32, u64)>, // (attempts, next_retry_tick)
```

В `allocate()` — если `packet_loss_rate > threshold` и есть pending bids, повторно включать их в `current_assignments()` (чтобы runner отправил их снова) с backoff.

**Упрощение для v0.15:** вместо полноценного ACK-механизма — при `packet_loss_rate > threshold` агент "переотправляет" свои winning bids каждые `retransmit_backoff_ticks` тиков, даже если они не изменились. Это увеличивает `messages_exchanged`, но повышает вероятность доставки.

```rust
// В allocate():
if packet_loss_rate > self.config.retransmit_threshold_packet_loss {
    if self.current_round % self.config.retransmit_backoff_ticks as u32 == 0 {
        // Force re-broadcast of all winning bids
        self.force_rebroadcast = true;
    }
}
```

### Шаг 5 — Partition healing

Файл: `crates/swarm-sim/src/runner.rs`

Расширить `PartitionEvent`:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartitionEvent {
    pub at_tick: u64,
    pub until_tick: Option<u64>,
    pub heal_at_tick: Option<u64>, // v0.15 — явное время heal
    pub agents: (AgentId, AgentId),
}
```

В `run()` — логика обработки `heal_at_tick`:
```rust
// Если heal_at_tick задан и наступил — сбросить партицию для этой пары
if let Some(heal) = event.heal_at_tick {
    if current_tick >= heal {
        network.remove_partition(&event.agents.0, &event.agents.1);
        // Сбросить CBBA convergence, чтобы повторно сойтись
        if let Some(ref mut cbba) = cbba_state {
            cbba.converged = false;
        }
    }
}
```

Тест:
```rust
#[test]
fn cbba_reconverges_after_partition_heal() {
    // Partition на тиках 50-100, heal на 100
    // К тику 150 — converged == true, conflicting_assignments == 0
}
```

### Шаг 6 — 1000-seed benchmark и README

Файл: `scenarios/cbba_stress.json`

Создать `ScenarioSuite` с 1000 entries — разные seeds, фиксированный профиль:
```json
{
  "name": "CBBA Stress 1000 seeds",
  "description": "CBBA convergence analysis across 1000 seeds with varying packet loss",
  "scenarios": [
    {"mission": "cbba-stress", "profile": "pl-0.0", "scenario": {...seed 0...}, "run_config": {"packet_loss_rate": 0.0, ...}},
    {"mission": "cbba-stress", "profile": "pl-0.1", "scenario": {...seed 0...}, "run_config": {"packet_loss_rate": 0.1, ...}},
    ...
  ]
}
```

**Упрощение для v0.15:** вместо 1000 seeds × ручного JSON — использовать `BenchmarkHarness` с `cbba` стратегией и профилями `pl-0.0`, `pl-0.1`, `pl-0.2`, `pl-0.3`, `pl-0.4`, `pl-0.5` (6 профилей × 1000 seeds = 6000 runs). Результаты экспортировать в JSON/CSV.

Файл: `README.md`

Добавить раздел **M15 — CBBA Robustness**:
- Описание TSP-ordering, retransmission, partition healing.
- Convergence distribution таблица (p50/p95/max по packet loss rate).
- Команды запуска: `cargo run -p swarm-examples --bin strategy_comparison -- --mission cbba-stress --full`

---

## Testing Strategy

### Категория 1 — unit тесты (swarm-alloc, swarm-metrics)

- `order_bundle_tsp_nearest_first` — nearest neighbour выбирается первым
- `order_bundle_tsp_all_tasks_included` — нет потерь задач
- `cbba_retransmission_increases_messages` — при high packet loss messages_exchanged растёт
- `cbba_config_retransmit_defaults` — default values для новых полей
- `percentile_calculation_p50_p95` — корректность percentile для convergence_ticks

### Категория 2 — integration (swarm-sim, swarm-alloc)

- `cbba_convergence_ticks_tracked` — `cbba_convergence_tick` заполнен после сходимости
- `cbba_partition_heal_reconverges` — после heal convergence восстанавливается
- `cbba_tsp_reduces_travel_distance` — с TSP < без TSP по `avg_bundle_travel_distance`
- `cbba_stress_pl_0_3_convergence_p95_under_100` — p95 < 100 при packet_loss=0.3
- `cbba_no_conflicts_after_convergence` — conflicting_assignments == 0

### Категория 3 — proptest / e2e (swarm-sim/tests)

- `cbba_convergence_ticks_with_random_topology` — convergence при случайных topologies
- `cbba_no_conflicts_after_convergence` — инвариант отсутствия конфликтов
- `cbba_convergence_time_bounded` — convergence_ticks < max_ticks
- E2E: `--mission cbba-stress --full` — 6000 runs, экспорт CSV/JSON

---

## Risks and Tradeoffs

**1. TSP-ordering изменяет порядок задач в bundle**

Может повлиять на convergence, если порядок влияет на marginal score. Митигация: TSP применяется только к уже назначенным задачам (bundle), не к bidding process. Marginal score считается до TSP.

**2. Retransmission увеличивает message count**

`messages_exchanged` может вырасти на 2-3x при high packet loss. Митигация: threshold-based activation (только при packet_loss > 0.1).

**3. PartitionEvent heal_at_tick backward compatibility**

Старые JSON с `PartitionEvent` без `heal_at_tick` должны загружаться. Митигация: `#[serde(default)]` на новом поле.

**4. Convergence time tracking для non-CBBA стратегий**

`cbba_convergence_tick` имеет смысл только для CBBA. Митигация: `Option<u64>`, None для non-distributed стратегий.

**5. 1000-seed benchmark время выполнения**

6000 runs (6 profiles × 1000 seeds) может занять минуты. Митигация: `--full` флаг для CI, quick run (10 seeds) для smoke test.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| `PartitionEvent` новое поле | JSON roundtrip старых сценариев | `cargo test` dsl::tests |
| `CbbaConfig` новые поля | Default impl, конструкторы | `cargo test` swarm-alloc |
| `RunMetrics` новое поле | AggregateMetrics::from_runs, export | `cargo test` swarm-metrics + report_export |
| TSP-ordering меняет bundle order | `cbba_round_assignments_converge` тест | `cargo test` cbba::tests |
| Retransmission увеличивает messages | proptest message count bounds | `cargo test` proptest_cbba |
| Convergence tracking в runner | `convergence_ticks` logic для partition | `cargo test` runner + partition_scenario |

---

## Open Questions

1. **TSP-ordering: жадный NN или полный TSP?** — v0.15: greedy nearest-neighbour (O(n²)). v0.16: 2-opt improvement.
2. **Retransmission: explicit ACK или periodic rebroadcast?** — v0.15: periodic rebroadcast (проще, не требует ACK протокола). v0.16: explicit ACK с таймаутом.
3. **Convergence tracking: по winning bids или по registry?** — v0.15: по `cbba.converged` flag. Альтернатива: проверять `conflicting_assignments == 0` через registry.
4. **Bundle travel distance: из poses или из grid_cell?** — v0.15: из `task.pose`. Для SAR задач pose есть всегда.

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo test -p swarm-alloc
cargo test -p swarm-metrics
cargo test --test proptest_cbba
cargo run -p swarm-examples --bin strategy_comparison -- \
  --mission cbba-stress --json /tmp/cbba_stress.json
cargo run -p swarm-examples --bin strategy_comparison -- \
  --mission cbba-stress --csv /tmp/cbba_stress.csv
```
