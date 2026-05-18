# DRONE_A.4 — Milestone 11: True Distributed CBBA + Publishable Benchmark

> Сессия: май 2026. Синтез направлений A и B после завершения Milestone 10.

---

## Текущее состояние

Milestones 1–10 реализованы полностью. На сегодня:

- **5 стратегий:** Greedy, Auction, ConnectivityAware, CentralizedPlanner, CBBA
- **2 reference миссии:** SAR (Search and Rescue), EmergencyMesh
- **Инфраструктура:** proptest (50 случайных сценариев), replay, JSON/CSV export, kinematic+battery model
- **Тест-сьют:** 167 тестов, все зелёные

Текущий статус проекта: **runnable research prototype** с полной инфраструктурой и рабочими алгоритмами.
Следующий порог: **publishable result** — воспроизводимый benchmark с числовыми выводами.

---

## Главный незакрытый gap: CBBA — не по-настоящему распределённый

Согласно PLAN.md Milestone 10, CBBA должен был работать через
tick-loop orchestration с обменом bid-ами через `RuntimeMessage::Cbba`. Реализовано не было.

**Что есть сейчас:**

- `RuntimeMessage::Cbba` объявлен в `swarm-runtime/src/message.rs` (строки 23–28).
- В `swarm-runtime/src/node.rs` (строки 123–125) CBBA-сообщения **молча игнорируются**:
  ```rust
  Some(RuntimeMessage::Cbba { .. }) => {
      // CBBA messages handled at ScenarioRunner level
  }
  ```
- В `swarm-sim/src/runner.rs` никакой CBBA-оркестрации нет — только чтение метрик
  в конце симуляции (строки 738–741).
- `CbbaAllocator::apply_remote_bids()` реализован, но **никогда не вызывается**.
- `CbbaAllocator::allocate()` делает только локальный `build_bundles()` — фасад без
  настоящего консенсуса.

**Что это значит:** сейчас CBBA принимает решения без обмена bid-ами между агентами.
Каждый агент просто смотрит на задачи и грeedily добавляет их в свой bundle. Это не
отличается от Greedy на уровне алгоритма — только более сложный scoring.

**Что должно быть:** агенты широковещательно рассылают свои `winning_bids` через сеть,
конкурирующие bid-ы разрешаются через `apply_remote_bids()`, и только после консенсуса
(stable `winning_bids`) assignments применяются. Это делает CBBA по-настоящему
распределённым алгоритмом.

---

## Milestone 11: два направления, один roadmap

### Направление A — True Distributed CBBA

Реализовать tick-loop orchestration для CBBA:

1. **Phase 1 (Bundle Building):** runner вызывает `cbba.build_bundles()` — локальное
   решение каждого агента, без изменений.

2. **Phase 2 (Consensus broadcast):** для каждого живого агента runner формирует
   `RuntimeMessage::Cbba { round, winning_bids, sender_bundle }` и рассылает через
   `node.send_cbba_bids()` (новый метод в `AgentNode`).

3. **Network delivery:** сеть доставляет CBBA-сообщения через `bus.advance_tick()`
   (уже вызывается в начале тика).

4. **Phase 2 (Consensus apply):** runner собирает CBBA-сообщения из inbox каждого
   агента через `node.collect_cbba_messages()` (новый метод), агрегирует их в
   `remote_bids: Vec<(AgentId, HashMap<TaskId, (AgentId, f64)>)>`, вызывает
   `cbba.apply_remote_bids(&remote_bids)`.

5. **Assignment apply:** runner получает `cbba.current_assignments()` и применяет
   decisions к каждому `node.coordinator.registry` напрямую.

#### Архитектурные изменения

| Компонент | Изменение |
|---|---|
| `swarm-runtime/src/node.rs` | Добавить `send_cbba_bids(round, winning_bids)` и `collect_cbba_messages() -> Vec<RuntimeMessage>` |
| `swarm-sim/src/runner.rs` | CBBA-mode detection: если allocator реализует `CbbaOrchestrator` trait — войти в CBBA tick path |
| `swarm-alloc/src/cbba.rs` | Добавить `CbbaOrchestrator` trait (маркер) или отдельный `run_distributed_round()` |
| `swarm-alloc/src/allocator.rs` | Добавить опциональный метод `as_cbba_orchestrator()` |

**Почему trait, а не downcast:** `runner.rs` не должен знать о `CbbaAllocator` напрямую
(нарушение инверсии зависимостей). Вместо этого `Allocator` предоставляет
`fn is_distributed(&self) -> bool` и `fn distributed_assignments()`.

#### Convergence и assignment применение

При `cbba.converged == true` runner пропускает Phase 1/2 и продолжает с текущими
assignments. При новых задачах или отказе агента: `cbba.converged = false` (уже реализовано
в `allocate()`). Для distributed режима: runner выставляет `cbba.converged = false` явно
при обнаружении нового `DynamicTaskEvent` или `FailureEvent`.

---

### Направление B — Publishable Benchmark + Property-Based Tests

#### B.1 — Property-based тесты для CBBA

Текущий `swarm-sim/tests/proptest_runner.rs` использует только `GreedyAllocator` и
`CentralizedPlanner`. Добавить:

```rust
proptest! {
    #[test]
    fn cbba_does_not_panic_on_random_topology(
        agents in prop::collection::vec(agent_strategy(), 3..10),
        tasks in prop::collection::vec(task_strategy(), 3..15),
        packet_loss in 0.0f64..0.5f64,
    ) {
        let scenario = scenario_from_agents_tasks(agents, tasks);
        let mut config = default_run_config();
        config.packet_loss_rate = packet_loss;
        let _metrics = ScenarioRunner::run_with(&scenario, config, CbbaAllocator::default());
    }

    #[test]
    fn cbba_task_completion_rate_non_negative(/* ... */) { /* ... */ }

    #[test]
    fn cbba_converges_within_max_rounds(/* ... */) { /* ... */ }
}
```

Цель: 500+ случайных комбинаций (agents × tasks × packet_loss × topology), CBBA
не паникует и метрики в допустимых пределах.

#### B.2 — 1000-seed full benchmark

`strategy_comparison` уже поддерживает seed batches. Расширить:

1. Добавить SAR как отдельный benchmark context в `strategy_comparison`
   (сейчас только coverage/mesh профили).
2. Запустить: **5 стратегий × 2 миссии × 1000 seeds × 3 network profiles** =
   30 000 прогонов.
3. Зафиксировать числовые результаты в `README.md` — таблица и ключевые выводы.

Профили (уже есть в `swarm-scenarios/src/profiles.rs`):
- `ideal` (0% loss, 0 latency)
- `degraded` (~10% loss)
- `high_loss` (~30% loss)

Ожидаемый вывод в README (формат):

```
| Strategy     | SAR detection (ideal) | SAR detection (degraded) | Mesh availability |
|---|---|---|---|
| greedy       | 23.4 ticks            | 31.2 ticks               | 0.94              |
| auction      | 22.1 ticks            | 29.8 ticks               | 0.95              |
| cbba         | 19.8 ticks            | 26.1 ticks               | 0.97              |
| centralized  | 18.9 ticks            | 24.7 ticks               | 0.98              |
| conn-aware   | 21.3 ticks            | 27.5 ticks               | 0.99              |
```

---

## Порядок реализации (Milestone 11)

### Шаг 1 — `AgentNode` CBBA API

Файл: `crates/swarm-runtime/src/node.rs`

Добавить два метода:
- `send_cbba_bids(&mut self, round: u32, winning_bids: &HashMap<TaskId, (AgentId, f64)>)`
  — формирует `RuntimeMessage::Cbba` и рассылает по `peer_ids`.
- `collect_cbba_messages(&mut self) -> Vec<RuntimeMessage>`
  — вычитывает inbox, возвращает только `Cbba` variant, остальные кладёт обратно в
  буфер или игнорирует (зависит от порядка вызовов с `process_inbox_and_allocate`).

**Примечание:** `collect_cbba_messages` должен вызываться *после* того, как другой тик
доставил CBBA-сообщения. Т.е. в следующем тике агент подбирает CBBA-сообщения от соседей
из предыдущего тика — один раунд задержки. Это корректно для итеративного алгоритма.

**Тесты (категория 1):**
- `send_cbba_bids_delivers_to_peers` — после `send_cbba_bids`, peer получает `Cbba` message
- `collect_cbba_messages_filters_non_cbba` — heartbeats и gossip не попадают в результат

---

### Шаг 2 — `Allocator::is_distributed()` + runner CBBA path

Файл: `crates/swarm-alloc/src/allocator.rs`

```rust
pub trait Allocator {
    fn allocate(&mut self, ...) -> Vec<(TaskId, AgentId)>;
    fn allocation_metrics(&self) -> (u64, bool, u64) { (0, false, 0) }
    /// True if this allocator requires distributed tick-loop orchestration.
    fn is_distributed(&self) -> bool { false }
}
```

`CbbaAllocator` переопределяет `is_distributed()` → `true`.

Файл: `crates/swarm-sim/src/runner.rs`

В `run_internal()` после gossip phase:

```rust
if allocator.is_distributed() {
    // CBBA distributed phase
    let alive_nodes = nodes filtered by !crashed;
    // Phase 1: local bundle building
    let all_agents = collect_allocation_agents(&alive_nodes);
    let all_tasks = collect_allocation_tasks(&alive_nodes);
    allocator.build_bundles_distributed(&all_agents, &all_tasks);

    // Phase 2: broadcast winning_bids
    for (node, _) in alive_nodes {
        node.send_cbba_bids(current_round, cbba_winning_bids);
    }

    // Advance tick to deliver CBBA messages
    bus.borrow_mut().advance_tick();

    // Collect remote bids
    let remote_bids: Vec<(AgentId, ...)> = alive_nodes
        .flat_map(|(node, id)| node.collect_cbba_messages()
            .filter_map(|m| extract_cbba_bids(m).map(|bids| (id, bids))))
        .collect();
    allocator.apply_remote_bids_distributed(&remote_bids);

    // Apply assignments
    let assignments = allocator.distributed_assignments();
    apply_assignments_to_nodes(&mut nodes, assignments);
}
```

Для `build_bundles_distributed`, `apply_remote_bids_distributed`, `distributed_assignments`:
добавить в `Allocator` trait с default-реализацией panic/noop (вызываются только когда
`is_distributed() == true`). Альтернатива — отдельный `DistributedAllocator` trait. Выбор
при реализации.

**Тесты (категория 1):**
- `cbba_distributed_round_delivers_bids` — 3 агента, 2 раунда, `apply_remote_bids` вызван
- `cbba_convergence_stops_broadcasting` — после `converged`, send_cbba_bids не вызывается

---

### Шаг 3 — Property-based тесты для CBBA

Файл: `crates/swarm-sim/tests/proptest_runner.rs`

Добавить 3 proptest-случая (описаны в B.1 выше).

Файл: `crates/swarm-alloc/src/cbba.rs`

Добавить unit proptest:
- `cbba_score_monotone_in_battery` — выше battery → выше score при той же дистанции
- `cbba_no_double_assignment` — один task_id встречается в assignments не более одного раза

**Тесты (категория 1, без рефакторинга):** 2 новых unit + 3 новых proptest.

---

### Шаг 4 — SAR benchmark в strategy_comparison

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

Добавить `SarScenario` как контекст бенчмарка (рядом с EmergencyMesh). Параметры:
- 6×6 grid, 5 агентов, 5 hidden targets
- 1000 seeds
- профили: `ideal`, `degraded`, `high_loss`

Добавить в `ComparisonReport` и JSON/CSV экспорт метрики SAR:
`avg_time_to_find`, `avg_probability_of_detection`, `avg_targets_found`.

Файл: `crates/swarm-sim/src/report_export.rs` — добавить новые колонки.

**Тесты (категория 2):**
- `sar_benchmark_runs_all_strategies` — все 5 стратегий без паники, `total_runs == 1000`
- `sar_benchmark_csv_contains_cbba_row` — CSV содержит строку со strategy=`cbba`

---

### Шаг 5 — Обновить README.md

- Добавить **Milestone 11** в `## Current Status`
- Таблица сравнения 5 стратегий на 2 миссиях (числа из реального прогона)
- Секция "What makes CBBA different" — объяснение distributed consensus vs local decision

---

## Testing Strategy

### Категория 1 — Без рефакторинга (unit тесты)

| Тест | Файл |
|---|---|
| `send_cbba_bids_delivers_to_peers` | `swarm-runtime/src/node.rs` |
| `collect_cbba_messages_filters_non_cbba` | `swarm-runtime/src/node.rs` |
| `cbba_distributed_round_delivers_bids` | `swarm-sim/src/runner.rs` или test module |
| `cbba_convergence_stops_broadcasting` | `swarm-alloc/src/cbba.rs` |
| `cbba_score_monotone_in_battery` | `swarm-alloc/src/cbba.rs` (proptest) |
| `cbba_no_double_assignment` | `swarm-alloc/src/cbba.rs` (proptest) |

### Категория 2 — Лёгкий рефакторинг (интеграционные)

| Тест | Описание |
|---|---|
| `cbba_does_not_panic_on_random_topology` | proptest, `swarm-sim/tests/proptest_runner.rs` |
| `cbba_task_completion_rate_non_negative` | proptest, 500 случаев |
| `cbba_converges_within_max_rounds` | proptest, check `cbba_converged == true` |
| `sar_benchmark_runs_all_strategies` | `strategy_comparison` integration |
| `sar_benchmark_csv_contains_cbba_row` | export verification |

### Категория 3 — Тяжёлый (не для v0.11)

- **TSP-ordering в bundles:** полноценный sequential task ordering через nearest-neighbour.
  Требует отдельного алгоритма и перепроектирования scoring.
- **CBBA retransmission:** ретрансляция bid-ов при message loss > 30%. Требует
  sequence numbers и acknowledgment в CBBA protocol.
- **Dynamic task injection во время CBBA rounds:** перезапуск consensus при появлении
  нового task. Требует изменения convergence detection и bundle invalidation.
- **1M seeds comparison:** 5 стратегий × 2 миссии × 1000 seeds уже в scope; 100K+ seeds
  оставлено для будущего статистического анализа.

### Покрытие gap

- **Gap:** CBBA при message loss > 50%. Gossip канал деградирует → bundles расходятся
  без heal. Специфичный retransmission — v0.12.
- **Gap:** CBBA под network partition. Текущий тест `cbba_handles_partition` проверяет
  convergence после heal, но не время расхождения во время partition. Более детальный
  анализ — после числового benchmark.
- **Gap:** CBBA на > 20 агентах. Текущий `max_bundle_size=5` и `max_rounds=20` не
  тестированы при N > 10. Scaling analysis — v0.12.

---

## Что откладывается после M11

**TSP-ordering:** полноценный sequential task ordering — нужно накопить данные о том,
насколько упрощённый position penalty отличается от TSP на реальных SAR прогонах.
После числового benchmark это будет видно.

**Mission DSL (YAML/RON):** по-прежнему не приоритет. Текущие Rust-сценарии
достаточны для 2 reference missions. DSL нужен, когда появятся новые миссии.

**Sensor model / uncertainty map:** полноценный "уровень D" симуляции с probabilistic
detection map и sensor fusion. Входит в SAR как `probability_of_detection`, но
полноценная неопределённость в пространстве состояний — Milestone 12+.

**PX4 / Visualization:** за горизонтом текущего плана.

---

## Финальная рекомендация

Порядок реализации Milestone 11:

1. **Шаг 1–2** (Distributed CBBA): закрывает технический долг, делает CBBA честным
   алгоритмом. Без этого числа benchmark не имеют смысла — CBBA был бы просто Greedy+.
2. **Шаг 3** (proptest CBBA): закрывает coverage gap, обнаруженный в Milestone 7.
3. **Шаг 4–5** (SAR benchmark + README): даёт публикуемый результат — таблица из реальных
   1000-seed прогонов по 5 стратегиям и 2 миссиям.

Критерий готовности Milestone 11:

1. CBBA агенты обмениваются bid-ами через `RuntimeMessage::Cbba` в каждом тике.
2. `apply_remote_bids()` вызывается с реальными данными от соседей (не пустой срез).
3. proptest CBBA: 500+ случаев без паники, `cbba_converged` в допустимом диапазоне.
4. `strategy_comparison` включает SAR и выводит числа по всем 5 стратегиям.
5. README содержит таблицу с реальными числами из прогона benchmark.
6. Все 167+ существующих тестов проходят.
