# PLAN: Phase 2 — Unified Experiment Runner

## Context

Phase 1 реализовал True Distributed CBBA с message-driven consensus. Теперь есть 5 стратегий на 3 reference missions (Coverage, EmergencyMesh, SAR). Но `strategy_comparison` binary использует только Coverage сценарий. Нет способа запустить бенчмарк на EmergencyMesh или SAR без ручного изменения кода.

**Phase 2** делает `strategy_comparison` unified experiment runner: `--mission` флаг, 3 миссии + `all`, SAR profiles, SAR метрики в экспорт, README с публикуемыми числовыми таблицами.

**Источники контекста:** `docs/DRONE_A.5.md`, `docs/DRONE_B.5.md`. INVESTIGATION.md отсутствует.

**Текущее состояние (v0.10 Phase 1):**
- `strategy_comparison` binary с 5 стратегиями
- `BenchmarkHarness` с `run_quick()` (10 seeds) и `run_full()` (100 seeds, CI)
- `StandardProfiles` для Coverage сценария
- `ComparisonReport` с markdown таблицей, key: `(strategy, profile)`
- JSON/CSV export, replay logs, `--full` флаг
- `RunMetrics` уже содержит SAR поля (`time_to_find`, `coverage_over_time`, etc.)
- `AggregateMetrics` НЕ содержит SAR агрегаций, `ReportRow` НЕ включает SAR колонки

**Критерий готовности:**
1. `--mission coverage|emergency-mesh|sar|all` флаг.
2. `--mission all` запускает бенчмарк на всех 3 миссиях, каждая со своими profiles.
3. SAR metrics агрегируются в `AggregateMetrics` и экспортируются в JSON/CSV.
4. `ComparisonReport` содержит колонки `mission` и `scenario` в каждой строке.
5. Каждая строка содержит `seed_range` / `total_runs`.
6. README содержит таблицу с реальными числами из бенчмарка.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.5.md` и `docs/DRONE_B.5.md`:
- DRONE_A.5: Phase 2 — unified experiment runner с mission support, единый output, README с таблицами.
- DRONE_B.5: unified benchmark для получения публикуемых результатов после CBBA.

---

## --mission all архитектура

`BenchmarkHarness` вызывается 3 раза (по разу на миссию). Каждый вызов производит `ComparisonReport` с 2-part key `(strategy, profile)`. Результаты сливаются в единый `ComparisonReport` с 3-part key `(mission, strategy, profile)`:

```
for mission in [Coverage, EmergencyMesh, Sar]:
    harness.run(strategies, mission.scenario_builder, profiles)
    for (strategy, profile) result:
        merged.results.insert((mission_name, strategy, profile), metrics)
```

`benchmark_run_id` — один на весь прогон (timestamp + prefix), стабильный внутри одного запуска. Все 3 миссии разделяют один `run_id`.

`ComparisonReport` структура после merge:
```rust
pub struct ComparisonReport {
    pub benchmark_run_id: String,
    pub seed_range_start: u64,
    pub seed_range_end: u64,
    pub total_runs_per_cell: u64, // seeds per (mission, strategy, profile)
    pub mission_names: Vec<String>,
    pub scenario_names: Vec<String>,    // NEW: scenario name per mission
    pub strategy_names: Vec<String>,
    pub profile_names: Vec<String>,
    pub results: HashMap<(String, String, String), AggregateMetrics>,
}
```

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | `--mission` флаг; mission builder selection; merge 3 harness results |
| `crates/swarm-sim/src/benchmark.rs` | `ComparisonReport`: 3-part key + seed_range + scenario column |
| `crates/swarm-sim/src/report_export.rs` | `ReportRow` добавить SAR колонки (time_to_find, probability_of_detection, targets_found) + mission/scenario колонки |
| `crates/swarm-metrics/src/metrics.rs` | `AggregateMetrics`: добавить `avg_time_to_find`, `avg_probability_of_detection`, `avg_targets_found`; `from_runs` агрегация |
| `crates/swarm-scenarios/src/sar_scenario.rs` | `SarProfile` enum + `StandardProfiles` |  
| `crates/swarm-scenarios/src/emergency_mesh.rs` | `EmergencyMeshProfile` enum + `StandardProfiles` |
| `crates/swarm-scenarios/src/lib.rs` | Re-export профилей |
| `README.md` | Таблица с числами из бенчмарка |

---

## Implementation Steps

### Шаг 1 — SAR + EmergencyMesh StandardProfiles

Файлы: `sar_scenario.rs`, `emergency_mesh.rs`

SAR profiles: `Ideal` (grid 6×6, 2 targets, PoD=1.0), `Standard` (grid 8×8, 3 targets, PoD=0.6), `Challenging` (grid 10×10, 5 targets, 10% packet loss), `BatteryConstrained` (grid 6×6, max_range=200, speed=5).

EmergencyMesh profiles: `Ideal` (no loss), `LowLoss` (5% loss), `MediumLoss` (10% loss), `SingleFailure` (relay agent fails).

### Шаг 2 — AggregateMetrics SAR агрегация

Файл: `crates/swarm-metrics/src/metrics.rs`

Добавить поля:
```rust
pub avg_time_to_find: f64,
pub avg_probability_of_detection: f64,  
pub avg_targets_found: f64,
```

В `from_runs()`: агрегировать из `RunMetrics::time_to_find`, `probability_of_detection`, `targets_found`.

В `Display`: добавить строки вывода.

### Шаг 3 — ReportRow + CSV/JSON SAR колонки

Файл: `crates/swarm-sim/src/report_export.rs`

`ReportRow` — добавить поля: `mission`, `scenario`, `seed_range_start`, `seed_range_end`, `time_to_find`, `probability_of_detection`, `targets_found`.

`export_csv`: записывать новые колонки в header и строки.

`export_json`: поля автоматически включаются через `serde`.

### Шаг 4 — ComparisonReport с mission/scenario/seed_range

Файл: `crates/swarm-sim/src/benchmark.rs`

Изменить key на `(mission, strategy, profile)`. Добавить поля `seed_range_start`, `seed_range_end`, `scenario_names`. Обновить `Display` таблицу: добавить колонки `Mission | Scenario | Seeds`.

`BenchmarkHarness::run_with_seeds` — добавить параметры `mission_name: &str`, `scenario_name: &str`, `seed_range_start: u64`, `seed_range_end: u64`.

### Шаг 5 — Mission enum + builder selection + merge

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

```rust
enum Mission { Coverage, EmergencyMesh, Sar }

fn build_scenario(mission: &Mission, seed: u64, profile: &str) -> (Scenario, RunConfig)
```

Merge logic:
```rust
let mut merged = ComparisonReport::new(run_id);
for mission in &cli.missions {
    let builder = mission_to_builder(mission);
    let report = harness.run(strategies, builder, profiles, mission);
    for (key, metrics) in report.results {
        merged.results.insert((mission_name, key.0, key.1), metrics);
    }
}
```

### Шаг 6 — CLI --mission флаг

`--mission coverage|emergency-mesh|sar|all`. Default: `coverage` (backward compat).

### Шаг 7 — README с таблицей

Таблица с результатами из прогона `--mission all --json /tmp/all.json`:

```
| Mission | Scenario | Strategy | Profile | Seeds | Success | TimeToFind | PoD | ... |
|---------|----------|----------|---------|-------|---------|------------|-----|-----|
| coverage | coverage_with_failure | greedy | ideal | 0..999 | 1.000 | - | - | ... |
| sar | sar_v1 | cbba | standard | 0..999 | 0.85 | 145.3 | 0.67 | ... |
```

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run -p swarm-examples --bin strategy_comparison --mission all --json /tmp/all.json
cargo run -p swarm-examples --bin strategy_comparison --mission sar --csv /tmp/sar.csv
```

---

## Testing Strategy

### Категория 1 — unit тесты

- `sar_ideal_profile_params` — проверка параметров Ideal профиля
- `sar_battery_constrained_profile_params` — проверка параметров
- `emergency_mesh_ideal_profile_params` — проверка параметров
- `aggregate_metrics_include_sar_fields` — SAR поля в AggregateMetrics
- `report_row_includes_sar_columns` — SAR колонки в ReportRow
- `parse_mission_all_returns_three` — парсинг --mission all
- `parse_mission_defaults_to_coverage` — без --mission
- `comparison_report_has_three_part_key` — новый key формат

### Категория 2 — integration

- `strategy_comparison_coverage_default` — backward compat
- `strategy_comparison_sar_runs` — --mission sar runs
- `strategy_comparison_all_merges_three_missions` — merge проверка
- `csv_export_includes_mission_column` — mission в CSV
- `json_export_includes_sar_metrics` — SAR метрики в JSON

### Категория 3 — manual

- `cargo run --bin strategy_comparison --mission all --json results.json` — полный benchmark

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| `ComparisonReport` 3-part key | Сломан `Display` и export | unit тесты + integration |
| `AggregateMetrics` новые поля | `from_runs` инициализация нулями для старых RunMetrics | unit тест |
| `ReportRow` новые колонки | CSV header mismatch | integration тест CSV export |
| `--mission` default coverage | Без флага падает | unit + integration тесты |
| SAR merge дублирует run_id | 3 прогона дают разные метрики с одним run_id | 3-part key различает миссии |

---

## Risks and Tradeoffs

**1. BenchmarkHarness вызывается 3 раза подряд**

Серийный запуск 3 миссий увеличивает общее время. Для `--full` (1000 seeds × 5 стратегий × 4 профиля × 3 миссии = 60,000 прогонов) время может быть значительным. Mitigation: `--quick` для малых прогонов.

**2. 3-part key ломает существующие скрипты**

JSON/CSV формат меняется. Новые колонки mission/scenario добавляются в начало строки, старые остаются — partial backward compat.

**3. SAR метрики не применимы к Coverage/EmergencyMesh**

`avg_time_to_find` = 0 для non-SAR миссий. В таблице помечается как `-` или `N/A`. Приемлемо.

---

## Open Questions

1. **Parallel execution для `--mission all`?** — Сейчас серийный. `rayon` для параллельных прогонов? v0.11.

2. **Как расширять профили в будущем?** — Current approach: `SarProfile::from_str(s) -> params`. Добавление нового варианта — +1 match arm и +1 config. Приемлемо.

3. **Версионирование формата JSON/CSV?** — Добавление колонок ломает парсеры. Добавить `format_version: "2"` в JSON root? v0.11.

4. **Should `ComparisonReport` store raw `RunMetrics` instead of only `AggregateMetrics`?** — `AggregateMetrics` достаточно для бенчмарка. Raw metrics для детального анализа — replay logs.
