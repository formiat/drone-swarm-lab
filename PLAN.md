# PLAN: SAR v2 / Uncertainty Map (M14)

## Context

Milestone 13 (Safety Layer) завершён. Платформа имеет физические ограничения (geofence, no-fly zones, separation). Следующий шаг по roadmap из `docs/DRONE_B.8.md` — Stage 2: SAR v2 / Uncertainty Map (M14).

**SAR v2** делает поисково-спасательную миссию исследовательски содержательной: агенты работают с вероятностной картой уверенности (belief map), повторно сканируют ячейки с высокой неопределённостью, учитывают ложные срабатывания (false positives).

**Источники контекста:** `docs/DRONE_A.7.md`, `docs/DRONE_B.7.md`, `docs/DRONE_B.8.md`. INVESTIGATION.md отсутствует.

**Текущее состояние (Safety Layer v0.13 complete):**
- `SearchGrid` — дискретная сетка с `width`, `height`, `cell_size`.
- `HiddenTarget` — цель в ячейке (`cell_x`, `cell_y`).
- `SensorModel` — role-based PoD (`scout_pod`, `thermal_pod`, `relay_pod`).
- `GridState` — mutable scan progress: `cells: Vec<CellState>`, `targets_found`, `scan_count`.
- `scan_cell` — one-shot scan: if target present + RNG < PoD → TargetFound, else Visited.
- SAR scenarios: `sar.ideal.json` с 3 scout + 1 thermal + 1 relay, 6×6 grid, 2 targets.
- Metrics: `time_to_find`, `probability_of_detection`, `targets_found`, `scan_count`, `coverage_progress`.

**Критерий готовности:**
1. `BeliefMap` с Bayes-обновлением, entropy, highest_uncertainty_cells.
2. `SensorModel` v2 — `detection_probability` + `false_positive_rate` (не только role-based PoD).
3. Задачи получают динамический приоритет на основе entropy ячейки.
4. Повторные сканирования (confirmation scans) для ячеек с `posterior > threshold`.
5. Новые метрики: `belief_entropy_final`, `false_positive_rate`, `confirmation_scans`.
6. Сценарии: `sar.uncertain.json`, `sar.noisy.json` (высокий FPR).
7. README обновлён с документацией SAR v2.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_B.8.md`:
- SAR v2 — Stage 2 гибридного roadmap.
- BeliefMap с Bayes-обновлением — ключевой компонент.
- False positives + repeated scans — основные отличия от SAR v1.
- Uncertainty-driven prioritization влияет на поведение всех аллокаторов.

**Ключевое наблюдение:** текущий `SensorModel` role-based (`scout_pod`/`thermal_pod`/`relay_pod`) отличается от предложенного `SensorModel` v2 (`detection_probability`/`false_positive_rate`/`range_m`). Для v0.14 планируем **расширение**, а не замену: добавляем `false_positive_rate` и `detection_probability` к существующему role-based подходу. Role-based PoD остаётся для backward compatibility.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-types/src/grid.rs` | Добавить `BeliefCell`, `BeliefMap`; расширить `SensorModel` v2 |
| `crates/swarm-runtime/src/grid_state.rs` | Интегрировать `BeliefMap` в `GridState`; Bayes-обновление в `scan_cell` |
| `crates/swarm-sim/src/runner.rs` | Обновить scan logic: передавать detection result в BeliefMap; repeated scans |
| `crates/swarm-alloc/src/allocator.rs` | Добавить `sar_task_priority` или dynamic priority hook |
| `crates/swarm-metrics/src/metrics.rs` | Новые поля: `belief_entropy_final`, `false_positives`, `confirmation_scans` |
| `crates/swarm-sim/src/report_export.rs` | Новые колонки в JSON/CSV export |
| `crates/swarm-scenarios/src/sar_scenario.rs` | Новые профили: `Uncertain`, `Noisy`; обновить `SensorModel` создание |
| `scenarios/sar.uncertain.json` | **NEW** — средний PoD, moderate FPR |
| `scenarios/sar.noisy.json` | **NEW** — высокий FPR, требует repeated scans |
| `scenarios/sar.ideal.json` | Обновить до SAR v2 (belief-aware) |
| `README.md` | Документировать SAR v2, BeliefMap, новые метрики |

---

## Implementation Steps

### Шаг 1 — BeliefMap и SensorModel v2

Файл: `crates/swarm-types/src/grid.rs`

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BeliefCell {
    pub prior: f64,
    pub posterior: f64,
    pub scan_count: u32,
    pub last_scan_tick: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BeliefMap {
    pub grid: SearchGrid,
    pub cells: Vec<Vec<BeliefCell>>,
    pub false_positives: u32,
    pub confirmation_scans: u32,
}

impl BeliefMap {
    pub fn new(grid: &SearchGrid, prior: f64) -> Self {
        let cells = (0..grid.height)
            .map(|_| {
                (0..grid.width)
                    .map(|_| BeliefCell {
                        prior,
                        posterior: prior,
                        scan_count: 0,
                        last_scan_tick: None,
                    })
                    .collect()
            })
            .collect();
        Self {
            grid: grid.clone(),
            cells,
            false_positives: 0,
            confirmation_scans: 0,
        }
    }

    pub fn update(&mut self, cell: (u32, u32), detection: bool, sensor: &SensorModel) {
        let (x, y) = cell;
        let bc = &mut self.cells[y as usize][x as usize];
        bc.scan_count += 1;

        let p_d_given_t = sensor.detection_probability; // P(detection | target)
        let p_d_given_not_t = sensor.false_positive_rate; // P(detection | no target)
        let p_t = bc.posterior;
        let p_not_t = 1.0 - p_t;

        let p_d = p_d_given_t * p_t + p_d_given_not_t * p_not_t;
        if p_d > 0.0 {
            bc.posterior = if detection {
                p_d_given_t * p_t / p_d
            } else {
                (1.0 - p_d_given_t) * p_t / (1.0 - p_d)
            };
        }
        bc.posterior = bc.posterior.clamp(0.0, 1.0);
    }

    pub fn entropy(&self, cell: (u32, u32)) -> f64 {
        let (x, y) = cell;
        let p = self.cells[y as usize][x as usize].posterior;
        if p <= 0.0 || p >= 1.0 { return 0.0; }
        -p * p.log2() - (1.0 - p) * (1.0 - p).log2()
    }

    pub fn highest_uncertainty_cells(&self, n: usize) -> Vec<(u32, u32)> {
        let mut all: Vec<((u32, u32), f64)> = (0..self.grid.height)
            .flat_map(|y| (0..self.grid.width).map(move |x| ((x, y), self.entropy((x, y)))))
            .collect();
        all.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        all.into_iter().take(n).map(|(c, _)| c).collect()
    }

    pub fn mean_entropy(&self) -> f64 {
        let total: f64 = (0..self.grid.height)
            .flat_map(|y| (0..self.grid.width).map(move |x| self.entropy((x, y))))
            .sum();
        total / (self.grid.width * self.grid.height) as f64
    }
}
```

Расширить `SensorModel`:
```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SensorModel {
    pub scout_pod: f64,
    pub thermal_pod: f64,
    pub relay_pod: f64,
    // v0.14 — sensor model v2
    pub detection_probability: f64, // P(detect | target present)
    pub false_positive_rate: f64,   // P(detect | no target)
}
```

**Тесты (категория 1):**
- `belief_map_update_bayes_correct` — posterior после detection/no-detection
- `belief_map_entropy_zero_at_extremes` — H=0 при posterior=0/1
- `belief_map_entropy_max_at_half` — H максимум при posterior=0.5
- `belief_map_posterior_clamped` — posterior не выходит за [0,1]
- `belief_map_highest_uncertainty` — корректный порядок ячеек

### Шаг 2 — Интеграция BeliefMap в GridState и runner

Файл: `crates/swarm-runtime/src/grid_state.rs`

Добавить `belief_map: Option<BeliefMap>` в `GridState`. Обновить `scan_cell`:
1. После скана — вызвать `belief_map.update(cell, detection, sensor)`.
2. Если `detection==true` но target не найден (false positive) — инкремент `false_positives`.
3. Если `scan_count > 1` — инкремент `confirmation_scans`.
4. CellState остаётся `Visited` (не `TargetFound`) если posterior < threshold, позволяя repeated scans.

Файл: `crates/swarm-sim/src/runner.rs`

В SAR scan logic (после agent достигает cell center):
1. Вызывать `grid_state.scan_cell(...)` как сейчас.
2. Дополнительно обновлять `BeliefMap`.
3. Если `posterior > threshold` и `scan_count >= 1` — оставить задачу в пуле (не release task) для повторного сканирования.

Threshold: `0.95` для TargetFound, `0.05` для "confirmed empty".

**Тесты (категория 2):**
- `scan_updates_belief` — posterior изменяется после scan_cell
- `scan_false_positive_counted` — detection без target = FPR increment
- `scan_repeated_when_uncertain` — задача остаётся при posterior ∈ (0.05, 0.95)

### Шаг 3 — Uncertainty-driven prioritization

Файл: `crates/swarm-alloc/src/allocator.rs` или `crates/swarm-scenarios/src/sar_scenario.rs`

```rust
pub fn sar_task_priority(belief_map: &Option<BeliefMap>, cell: (u32, u32)) -> u8 {
    match belief_map {
        Some(bm) => {
            let entropy = bm.entropy(cell);
            let p = bm.cells[cell.1 as usize][cell.0 as usize].posterior;
            // Scale entropy * posterior to priority range [1, 10]
            let raw = entropy * p * 20.0;
            raw.clamp(1.0, 10.0) as u8
        }
        None => 1,
    }
}
```

В `build_sar_scenario` — при создании задач, устанавливать `priority` через `sar_task_priority` с начальным uniform prior (например, `prior = target_count / total_cells`).

В `runner.rs` — после каждого tick, обновлять `priority` назначенных задач на основе текущего BeliefMap. Или: пересоздавать задачи для highest-uncertainty cells каждые N ticks.

**Упрощение для v0.14:** static priority при создании сценария (initial entropy = max для всех ячеек). Dynamic priority — в `NodeTickOutput` или через `DynamicTaskEvent`.

**Тесты (категория 2):**
- `sar_task_priority_high_entropy_wins` — ячейка с posterior=0.5 получает больший priority
- `sar_task_priority_low_entropy_loses` — ячейка с posterior=0.99 получает priority=1

### Шаг 4 — Метрики

Файл: `crates/swarm-metrics/src/metrics.rs`

```rust
// RunMetrics
#[serde(default)]
pub belief_entropy_final: f64,
#[serde(default)]
pub false_positives: u32,
#[serde(default)]
pub confirmation_scans: u32,

// AggregateMetrics
#[serde(default)]
pub avg_belief_entropy_final: f64,
#[serde(default)]
pub avg_false_positive_rate: f64,
#[serde(default)]
pub avg_confirmation_scans: f64,
```

`avg_false_positive_rate` = `false_positives / scan_count` (или 0 если scan_count=0).

Файл: `crates/swarm-sim/src/report_export.rs` — добавить новые колонки.

### Шаг 5 — SAR v2 сценарии и README

Файл: `crates/swarm-scenarios/src/sar_scenario.rs`

Новые профили:
```rust
pub enum SarProfile {
    Ideal,
    Standard,
    Challenging,
    BatteryConstrained,
    Uncertain,   // moderate PoD, moderate FPR
    Noisy,       // high FPR, requires repeated scans
}
```

- `Uncertain`: `sensor = SensorModel::new(0.4, 0.7, 0.15, 0.5, 0.2)` (detection_probability=0.5, FPR=0.2)
- `Noisy`: `sensor = SensorModel::new(0.3, 0.6, 0.1, 0.4, 0.4)` (detection_probability=0.4, FPR=0.4)

Файлы: `scenarios/sar.uncertain.json`, `scenarios/sar.noisy.json`

Структура: аналогично `sar.ideal.json`, но с `sensor.detection_probability` и `sensor.false_positive_rate`.

Файл: `README.md`

Добавить раздел **M14 — SAR v2 / Uncertainty Map**:
- Описание BeliefMap + Bayes-обновление
- Пример SensorModel v2
- Новые метрики (belief_entropy, false_positive_rate, confirmation_scans)
- Команды запуска: `--scenario-suite scenarios/sar.uncertain.json`

---

## Testing Strategy

### Категория 1 — unit тесты (swarm-types, swarm-runtime)

- `belief_map_update_bayes_correct` — posterior после detection/no-detection
- `belief_map_entropy_zero_at_extremes` — H=0 при posterior=0/1
- `belief_map_entropy_max_at_half` — H максимум при posterior=0.5
- `belief_map_posterior_clamped` — posterior ∈ [0,1] для любых inputs
- `belief_map_highest_uncertainty_cells` — корректный порядок
- `sar_task_priority_high_entropy_wins` — priority ∝ entropy
- `sensor_model_v2_serde_roundtrip` — detection_probability + false_positive_rate

### Категория 2 — integration

- `scan_updates_belief_in_grid_state` — GridState.scan_cell обновляет BeliefMap
- `scan_false_positive_counted` — detection без target → false_positives += 1
- `scan_repeated_when_uncertain` — задача остаётся при posterior ∈ (0.05, 0.95)
- `sar_uncertain_scenario_pod_above_0_5` — после 200 тиков avg_pod > 0.5
- `sar_noisy_scenario_high_fpr` — false_positive_rate соответствует sensor model
- `belief_map_mean_entropy_decreases` — entropy падает по мере сканирования

### Категория 3 — proptest / e2e

- `belief_update_no_panic_random_sensor` — случайные detection_probability/FPR ∈ [0,1]
- `belief_posterior_always_in_range` — posterior ∈ [0,1] для любых sequences
- E2E: `--scenario-suite scenarios/sar.noisy.json` — завершается, экспортирует CSV

---

## Risks and Tradeoffs

**1. SensorModel v1 vs v2 compatibility**

Текущий `SensorModel` role-based (`scout_pod`/`thermal_pod`/`relay_pod`) используется в SAR v1. Добавление `detection_probability`/`false_positive_rate` требует решения: как role-based PoD мапится на Bayes-модель.

Митигация: `scan_cell` продолжает использовать role-based PoD для TargetFound/Visited decision. `BeliefMap.update` использует `detection_probability` как общий sensor characteristic. Role-based PoD влияет на `detection` флаг, а `detection_probability` — на Bayes-обновление.

**2. Performance repeated scans**

Repeated scans увеличивают `scan_count` и могут замедлить convergence. Митигация: threshold `0.95` для confirmation ensures не более 2-3 scans на ячейку.

**3. Task priority изменение в runtime**

Изменение `priority` после создания задачи требует доступа к `TaskRegistry` из runner. Митигация: static priority при создании сценария (v0.14). Dynamic priority через `DynamicTaskEvent` (v0.15).

**4. Backward compatibility SAR v1**

SAR v1 сценарии (`sar.ideal.json`) должны продолжать работать. Митигация: `BeliefMap` — `Option`, `SensorModel` v2 fields имеют `#[serde(default)]`. Если BeliefMap отсутствует — fallback на SAR v1 logic.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| `SensorModel` новые поля | JSON roundtrip старых сценариев | `cargo test` dsl::tests |
| `GridState.scan_cell` изменён | SAR v1 behavior (TargetFound/Visited) | `cargo test` grid_state::tests |
| `BeliefMap` добавлен в `GridState` | Конструкторы GridState в sar_scenario.rs | `cargo test` swarm-scenarios |
| `RunMetrics` новые поля | AggregateMetrics::from_runs, export | `cargo test` swarm-metrics + report_export |
| Task priority изменён | SAR scenario task creation | `cargo test` sar_scenario tests |
| Repeated scans не release task | Task pool не очищается, memory | `cargo test` runner + E2E |

---

## Open Questions

1. **Dynamic priority vs static priority?** — v0.14: static (initial entropy). v0.15: dynamic через `DynamicTaskEvent` или task re-prioritization.
2. **Threshold для confirmation scan?** — `0.95` для "found", `0.05` для "empty", остальное — repeated scan. Настраиваемое через `SarScenarioConfig`.
3. **BeliefMap в GridState или отдельно?** — В GridState для simplicity. Можно вынести в отдельный модуль если разрастётся.
4. **Как отличать SAR v1 и SAR v2 в benchmark?** — `BeliefMap` present → SAR v2 metrics. Absent → SAR v1 (backward compatible).

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo test -p swarm-types
cargo test -p swarm-runtime
cargo run -p swarm-examples --bin strategy_comparison -- \
  --scenario-suite scenarios/sar.uncertain.json --json /tmp/sar_v2.json
```
