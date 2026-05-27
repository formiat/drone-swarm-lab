# M32b — Benchmark Identity Hardening for `--mission all`

## Context

M32 добавил per-row `mission`/`scenario` в `AggregateMetrics` и обновил exporters. Это исправило основную проблему: строки для SAR/inspection/wildfire больше не получают `mission="coverage"`.

Однако осталась вторичная, но заметная проблема идентичности:

- `benchmark_run_id` merged report берётся из `first.benchmark_run_id`, а первая миссия в `--mission all` — coverage. Итог: `2026-05-27T143506Z_coverage_1_quick`.
- `run_id` строится как `{benchmark_run_id}_{strategy}_{profile}`, поэтому SAR-строка получает id вида `..._coverage_1_quick_greedy_ideal`, что вводит в заблуждение.
- Profile names в merged report не mission-scoped: если две миссии имеют одинаковый profile (например, "ideal"), строки конфликтуют в HashMap и перетирают друг друга.

## Investigation context

`INVESTIGATION.md` отсутствует. Анализ кода показал:

- `generate_benchmark_run_id` (crates/swarm-sim/src/benchmark.rs:314-343) создаёт id формата `{timestamp}_{mission}_{seed_count}_{mode}` или `{prefix}_{timestamp}_{mission}_{seed_count}_{mode}`.
- `merge_reports` (crates/swarm-examples/src/bin/strategy_comparison.rs:747-793) копирует `first.benchmark_run_id` без изменений.
- `export_json`/`export_csv` (crates/swarm-sim/src/report_export.rs:6-195) строят `row_id` как `{benchmark_run_id}_{strategy}_{profile}`.
- В merged report profile names собираются напрямую из `report.profile_names` без mission prefix.

## Affected components

| Компонент | Путь | Что меняется |
|---|---|---|
| Benchmark ID generation | `crates/swarm-sim/src/benchmark.rs` | Добавить `merged_benchmark_run_id` helper |
| Report merge | `crates/swarm-examples/src/bin/strategy_comparison.rs` | Использовать merged ID, mission-scoped profiles |
| JSON/CSV export | `crates/swarm-sim/src/report_export.rs` | Mission-aware `run_id` |
| Integration tests | `crates/swarm-examples/tests/benchmark_pack.rs` | Новый тест `--mission all` identity |

## Implementation steps

### 1. merged_benchmark_run_id helper

Файл: `crates/swarm-sim/src/benchmark.rs`

Добавить публичную функцию:

```rust
pub fn merged_benchmark_run_id(reports: &[ComparisonReport]) -> String {
    if reports.len() == 1 {
        return reports[0].benchmark_run_id.clone();
    }
    // Parse first id to extract timestamp and prefix
    let first_id = &reports[0].benchmark_run_id;
    let parts: Vec<&str> = first_id.split('_').collect();
    
    // Format: [prefix_]timestamp_mission_count_mode
    // Try to detect prefix (contains non-timestamp chars before timestamp)
    let (prefix, timestamp) = if parts.len() >= 5 {
        // Has prefix: prefix_timestamp_mission_count_mode
        (Some(parts[0]), parts[1])
    } else {
        // No prefix: timestamp_mission_count_mode
        (None, parts[0])
    };
    
    let seed_count = reports[0].total_runs_per_cell;
    let mode = if seed_count <= 1 {
        "smoke"
    } else if seed_count <= 10 {
        "quick"
    } else {
        "full"
    };
    
    if let Some(p) = prefix {
        format!("{}_{}_all_{}_{}", p, timestamp, seed_count, mode)
    } else {
        format!("{}_all_{}_{}", timestamp, seed_count, mode)
    }
}
```

### 2. Обновить merge_reports

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

- Заменить `benchmark_run_id: first.benchmark_run_id.clone()` на `benchmark_run_id: swarm_sim::merged_benchmark_run_id(reports)`.
- Сделать profile names mission-scoped при merge:
  ```rust
  let merged_profile = format!("{}/{}", metrics.mission, profile_name);
  merged_results.insert((strategy_name.clone(), merged_profile.clone()), metrics.clone());
  ```
- Собирать `all_profile_names` в mission-scoped виде.

### 3. Обновить row_id в exporters

Файл: `crates/swarm-sim/src/report_export.rs`

В `export_json` и `export_csv`:

```rust
let row_id = format!(
    "{}_{}_{}_{}",
    report.benchmark_run_id,
    metrics.mission,
    strategy_name,
    profile_name
);
```

Заменить `/` на `_` в `run_id` для path-safe:
```rust
let safe_profile = profile_name.replace('/', "_");
let row_id = format!(
    "{}_{}_{}_{}",
    report.benchmark_run_id,
    metrics.mission,
    strategy_name,
    safe_profile
);
```

### 4. Интеграционный тест

Файл: `crates/swarm-examples/tests/benchmark_pack.rs`

Добавить тест `strategy_comparison_mission_all_has_all_benchmark_id`:

1. Запустить `--smoke --mission all --jobs 4 --output-dir target/test-output/mission_all_identity`
2. Прочитать `results.json`, `results.csv`, `manifest.json`
3. Проверить:
   - `benchmark_run_id` содержит `_all_`
   - `benchmark_run_id` не содержит `_coverage_` (как mission name)
   - Есть строки с `mission == "sar"`, `mission == "wildfire"`
   - SAR row `run_id` содержит `sar`, не содержит `coverage`
   - Profile names mission-scoped (например, `sar/ideal`, `wildfire/small-static`)
   - CSV содержит аналогичные строки

### 5. Проверка backward compatibility

- Single-mission runs (`--mission coverage`) должны использовать оригинальный `benchmark_run_id` без изменений.
- `--run-id-prefix myrun` должен сохраняться в merged id: `myrun_..._all_1_smoke`.

## Testing strategy

### Категория 1 — без рефакторинга

- **Интеграционный тест**: `strategy_comparison_mission_all_has_all_benchmark_id`
  - Проверяет `benchmark_run_id`, `run_id`, `mission`, `scenario`, `profile` для mixed-mission output
- **Unit test для `merged_benchmark_run_id`**:
  - Один отчёт → возвращает оригинальный id
  - Несколько отчётов → содержит `_all_`
  - С prefix → сохраняет prefix

### Категория 2 — лёгкий рефакторинг

- Заменить `/tmp/...` пути в существующих тестах на `tempfile::TempDir`
- Добавить helper для парсинга JSON report rows

### Категория 3 — тяжёлый рефакторинг

- Не требуется для этого фикса

## Risks and tradeoffs

| Риск | Вероятность | Влияние | Митигация |
|---|---|---|---|
| `merged_benchmark_run_id` парсинг сломается на нестандартных id | Низкая | Среднее | Добавить fallback: если parse не удался, использовать `first.benchmark_run_id` + `_all` |
| Mission-scoped profile names ломают внешние парсеры | Средняя | Среднее | Это intentional fix; профили уже были scoped в старом merge_reports (M32 убрал это); теперь возвращаем scoped имена с правильной семантикой |
| `run_id` с `/` может сломать файловые пути | Низкая | Среднее | Заменить `/` на `_` в `run_id` |
| Single-mission run id изменяется | Низкая | Высокое | `merged_benchmark_run_id` возвращает оригинал для len==1 |

## Open questions

1. **Как обрабатывать custom mode (seed count > 10 и != 1000)?**
   - Предлагается: `seed_count <= 1` → smoke, `<= 10` → quick, `== 1000` → full, иначе `custom`.

2. **Нужно ли менять `generate_benchmark_run_id` для single mission smoke?**
   - Сейчас smoke (1 seed) получает mode = "quick" в `generate_benchmark_run_id`, потому что `end_seed - start_seed <= 10`. Это существующее поведение; трогать только если явно запрошено.

3. **Как вести себя с `total_runs_per_cell` при merge?**
   - Сейчас `total_runs_per_cell` берётся из `first.total_runs_per_cell`. Это корректно, потому что все миссии в `--mission all` используют одинаковый seed range.

## Что могло сломаться

- **Поведение**: `benchmark_run_id` для `--mission all` меняется с `..._coverage_...` на `..._all_...`. Внешние скрипты, парсившие id для извлечения mission, увидят `all`.
- **Поведение**: Profile names в merged report теперь mission-scoped (`sar/ideal` вместо `ideal`). Внешние инструменты, ожидавшие чистые profile names, могут сломаться.
- **API/контракты**: `merged_benchmark_run_id` — новая публичная функция в `swarm-sim`. Не ломает существующий API.
- **Данные**: Старые `results.json` десериализуются без изменений (только additive changes).
- **Производительность**: Нет значительных изменений.

## Критерии готовности

- [ ] `cargo test --workspace` проходит (включая новый интеграционный тест).
- [ ] `cargo clippy --all-targets -- -D warnings` проходит.
- [ ] `cargo fmt --all` не меняет код.
- [ ] `--smoke --mission all` создаёт `benchmark_run_id` с `_all_` и без `_coverage_`.
- [ ] JSON/CSV/table согласованы по row identity.
- [ ] Profile names в merged report mission-scoped.
- [ ] Single-mission runs не изменили `benchmark_run_id`.
- [ ] Локальный commit сделан.
